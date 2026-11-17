// Bounded native Gemini, Anthropic and OpenAI-compatible requests with endpoint-bound credentials.

use std::time::Duration;

use folia_store::{AiProtocol, AiSettings, DualTheme};
use reqwest::{Url, header::HeaderValue};
use serde_json::{Value, json};

use crate::Error;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

fn official_openai(url: &Url) -> bool {
    url.host_str().is_some_and(|host| host == "api.openai.com" || host.ends_with(".api.openai.com"))
}

pub fn temperature_supported(settings: &AiSettings) -> bool {
    match settings.protocol {
        AiProtocol::Gemini => true,
        AiProtocol::Anthropic => false,
        AiProtocol::Openai => endpoint(settings).map_or(true, |url| !official_openai(&url)),
    }
}

#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    settings: AiSettings,
    key: HeaderValue,
    base: Url,
}

pub(crate) fn endpoint(settings: &AiSettings) -> Result<Url, Error> {
    let mut url = Url::parse(settings.endpoint.trim()).map_err(|_| Error::Configuration("invalid endpoint URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Configuration("endpoint must be HTTP(S) without credentials, query or fragment"));
    }
    let mut path = url.path().trim_end_matches('/').to_owned();
    for suffix in ["/chat/completions", "/messages"] {
        if let Some(base) = path.strip_suffix(suffix) {
            path = base.into();
            break;
        }
    }
    if path.is_empty() {
        path = match settings.protocol {
            AiProtocol::Gemini => "/v1beta",
            AiProtocol::Openai | AiProtocol::Anthropic => "/v1",
        }
        .into();
    }
    url.set_path(&path);
    Ok(url)
}

impl Client {
    /// Reads credentials on the caller's background task, never on the UI or audio thread.
    pub async fn from_store(service: String, settings: AiSettings) -> Result<Self, Error> {
        let snapshot = settings.clone();
        let key = tokio::task::spawn_blocking(move || crate::credentials::load_key(&service, &snapshot))
            .await??
            .ok_or(Error::MissingKey)?;
        Self::with_key(settings, key)
    }

    /// Uses an unsaved form key for a connection test. This constructor does not persist it.
    pub fn with_key(settings: AiSettings, key: String) -> Result<Self, Error> {
        let base = endpoint(&settings)?;
        if key.trim().is_empty() {
            return Err(Error::MissingKey);
        }
        if settings.model.trim().is_empty() || settings.model.len() > 256 || settings.model.contains(['\r', '\n']) {
            return Err(Error::Configuration("select a model"));
        }
        let key =
            if settings.protocol == AiProtocol::Openai { format!("Bearer {}", key.trim()) } else { key.trim().into() };
        let mut key = HeaderValue::from_str(&key).map_err(|_| Error::Configuration("invalid API key"))?;
        key.set_sensitive(true);
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none());
        if !settings.use_system_proxy {
            builder = builder.no_proxy();
        }
        Ok(Self { http: builder.build()?, settings, key, base })
    }

    pub async fn test_connection(&self) -> Result<(), Error> {
        let answer = self.complete("Reply to the greeting briefly.", "Hello", false).await?;
        if answer.trim().is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(())
    }

    pub async fn generate_theme(&self, title: &str, lyrics: &str) -> Result<DualTheme, Error> {
        let prompt = crate::theme::source_prompt(title, lyrics);
        let answer = self.complete(include_str!("theme_prompt.txt"), &prompt, true).await?;
        crate::parse_theme(&answer)
    }

    fn address(&self, suffix: &str) -> Url {
        let mut url = self.base.clone();
        url.set_path(&format!("{}{suffix}", self.base.path()));
        url
    }

    async fn complete(&self, system: &str, prompt: &str, structured: bool) -> Result<String, Error> {
        let model = self.settings.model.trim();
        let budget = if structured { 24_576 } else { 4096 };
        let temperature = f64::from(self.settings.temperature.clamp(0.0, 2.0));
        let request = match self.settings.protocol {
            AiProtocol::Gemini => {
                let model = model.strip_prefix("models/").unwrap_or(model);
                if !model.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')) {
                    return Err(Error::Configuration("invalid Gemini model"));
                }
                let mut config = json!({ "temperature": temperature, "maxOutputTokens": budget });
                if structured {
                    config["responseMimeType"] = json!("application/json");
                }
                self.http.post(self.address(&format!("/models/{model}:generateContent"))).header("x-goog-api-key", self.key.clone()).json(&json!({
                    "systemInstruction": {"parts": [{"text": system}]},
                    "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                    "generationConfig": config
                }))
            }
            AiProtocol::Anthropic => self.http.post(self.address("/messages")).header("x-api-key", self.key.clone()).header("anthropic-version", "2023-06-01").json(&json!({
                "model": model, "system": system, "messages": [{"role": "user", "content": prompt}], "max_tokens": budget
            })),
            AiProtocol::Openai => {
                let mut body = json!({"model": model, "messages": [{"role": "system", "content": system}, {"role": "user", "content": prompt}], "stream": false});
                let official = official_openai(&self.base);
                if official {
                    body["max_completion_tokens"] = json!(budget);
                } else {
                    body["max_tokens"] = json!(budget);
                    body["temperature"] = json!(temperature);
                }
                if structured {
                    body["response_format"] = json!({"type": "json_object"});
                }
                self.http.post(self.address("/chat/completions")).header(reqwest::header::AUTHORIZATION, self.key.clone()).json(&body)
            }
        };
        let payload = self.response(request).await?;
        extract_text(self.settings.protocol, &payload)
    }

    async fn response(&self, request: reqwest::RequestBuilder) -> Result<Value, Error> {
        let mut response = request.send().await?;
        if !response.status().is_success() {
            return Err(Error::Status(response.status().as_u16()));
        }
        if response.content_length().is_some_and(|size| size > MAX_RESPONSE_BYTES as u64) {
            return Err(Error::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(Error::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)
    }
}

fn extract_text(protocol: AiProtocol, payload: &Value) -> Result<String, Error> {
    let text = match protocol {
        AiProtocol::Openai => {
            let choice = &payload["choices"][0];
            if matches!(choice["finish_reason"].as_str(), Some("length" | "content_filter"))
                || choice["message"]["refusal"].as_str().is_some_and(|value| !value.is_empty())
            {
                return Err(Error::InvalidResponse);
            }
            let content = &choice["message"]["content"];
            content.as_str().map(str::to_owned).unwrap_or_else(|| text_parts(content, "text"))
        }
        AiProtocol::Gemini => {
            let candidate = &payload["candidates"][0];
            if candidate["finishReason"].as_str().is_some_and(|reason| reason != "STOP") {
                return Err(Error::InvalidResponse);
            }
            text_parts(&candidate["content"]["parts"], "text")
        }
        AiProtocol::Anthropic => {
            if payload["stop_reason"].as_str() == Some("max_tokens") {
                return Err(Error::InvalidResponse);
            }
            text_parts(&payload["content"], "text")
        }
    };
    if text.trim().is_empty() { Err(Error::InvalidResponse) } else { Ok(text) }
}

fn text_parts(parts: &Value, field: &str) -> String {
    parts
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| part["thought"] != true)
        .filter_map(|part| part[field].as_str())
        .collect()
}
