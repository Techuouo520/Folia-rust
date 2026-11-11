// LX HTTP requests use the facade client and return bounded callback payloads without exposing local files.

use std::time::Duration;

use reqwest::{Client, Method};
use serde_json::{Value, json};

use super::{SourceError, crypto};

pub(super) async fn request(client: Client, url: &str, options: &Value) -> Result<Value, SourceError> {
    validate_url(url)?;
    let method = options["method"].as_str().unwrap_or("get").to_ascii_uppercase();
    let method = Method::from_bytes(method.as_bytes()).map_err(|_| SourceError::Protocol)?;
    let timeout = options["timeout"].as_f64().filter(|n| *n > 0.0).unwrap_or(60_000.0).min(60_000.0);
    let mut url = reqwest::Url::parse(url).map_err(|_| SourceError::Protocol)?;
    let headers = options["headers"].as_object();
    let content_type =
        headers.and_then(|h| h.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-type"))).map(|(_, v)| v);
    let body = options.get("body").filter(|v| truthy(v));
    let json = body.is_some() && content_type.and_then(Value::as_str) == Some("application/json");
    let data = body
        .or_else(|| options.get("form").filter(|v| truthy(v)))
        .or_else(|| options.get("formData").filter(|v| truthy(v)));
    let bytes = match data {
        Some(data) if options["__raw"] == true => Some(crypto::bytes(data)?),
        Some(_) if method == Method::GET && !json => {
            url.set_query(Some(options["__data"].as_str().ok_or(SourceError::Protocol)?));
            None
        }
        Some(_) => Some(options["__data"].as_str().ok_or(SourceError::Protocol)?.as_bytes().to_vec()),
        None => None,
    };
    let mut request = client.request(method, url).timeout(Duration::from_secs_f64(timeout / 1000.0));
    if let Some(headers) = options["headers"].as_object() {
        for (name, value) in headers {
            if let Some(values) = value.as_array() {
                for value in values {
                    request = request.header(name, text(value));
                }
            } else {
                request = request.header(name, text(value));
            }
        }
    }
    if let Some(bytes) = bytes {
        if content_type.is_none() {
            request = request.header("content-type", "application/x-www-form-urlencoded");
        }
        request = request.body(bytes);
    }
    let mut response = request.send().await.map_err(SourceError::Http)?;
    let status = response.status();
    let headers: serde_json::Map<String, Value> =
        response.headers().iter().map(|(k, v)| (k.to_string(), json!(String::from_utf8_lossy(v.as_bytes())))).collect();
    if response.content_length().is_some_and(|n| n > crypto::BUFFER_LIMIT as u64) {
        return Err(SourceError::Limit);
    }
    let mut raw = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(SourceError::Http)? {
        if raw.len() + chunk.len() > crypto::BUFFER_LIMIT {
            return Err(SourceError::Limit);
        }
        raw.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&raw);
    let body = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!(body));
    Ok(json!({"statusCode":status.as_u16(),"statusMessage":status.canonical_reason().unwrap_or(""),
        "headers":headers,"bytes":raw.len(),"raw":raw,"body":body}))
}

fn text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string())
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
        _ => true,
    }
}

pub(super) fn validate_url(url: &str) -> Result<(), SourceError> {
    let parsed = reqwest::Url::parse(url).map_err(|_| SourceError::Protocol)?;
    if matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some() {
        Ok(())
    } else {
        Err(SourceError::Protocol)
    }
}
