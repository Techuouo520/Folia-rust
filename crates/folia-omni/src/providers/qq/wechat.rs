// WeChat web OAuth for QQ Music, using the protocol documented by qq-music-api 3.3.3.
use std::time::Duration;

use super::musicu::{Client, Credential};
use crate::{LoginCode, LoginState, OmniError, QrContent};

const APP_ID: &str = "wx48db31d50e334801";
const CONNECT: &str = "https://open.weixin.qq.com/connect/qrconnect";
const POLL: &str = "https://lp.open.weixin.qq.com/connect/l/qrconnect";

pub async fn login_code(http: &reqwest::Client) -> Result<LoginCode, OmniError> {
    let url = reqwest::Url::parse_with_params(
        CONNECT,
        [
            ("appid", APP_ID),
            ("redirect_uri", "https://y.qq.com/portal/wx_redirect.html?login_type=2&surl=https://y.qq.com/"),
            ("response_type", "code"),
            ("scope", "snsapi_login"),
            ("state", "STATE"),
            ("href", "https://y.qq.com/mediastyle/music_v17/src/css/popup_wechat.css#wechat_redirect"),
        ],
    )
    .map_err(|err| OmniError::Response(err.to_string()))?;
    let page = http.get(url).timeout(Duration::from_secs(20)).send().await?.error_for_status()?.text().await?;
    let uuid = page
        .split_once("uuid=")
        .map(|(_, rest)| rest.split(['"', '\'', '&', '<', '>', ' ', '\n', '\r']).next().unwrap_or_default())
        .filter(|uuid| valid_token(uuid))
        .ok_or_else(|| OmniError::Response("WeChat QR identifier is missing".into()))?;
    let response = http
        .get(format!("https://open.weixin.qq.com/connect/qrcode/{uuid}"))
        .header(reqwest::header::REFERER, CONNECT)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(private_error)?
        .error_for_status()
        .map_err(private_error)?;
    let bytes = response.bytes().await.map_err(private_error)?;
    let picture = image::load_from_memory(&bytes).map_err(|err| OmniError::QrImage(err.to_string()))?;
    let mut image = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut image, image::ImageFormat::Png).map_err(|err| OmniError::QrImage(err.to_string()))?;
    Ok(LoginCode { key: uuid.to_owned(), content: QrContent::Png(image.into_inner()) })
}

pub async fn check_login(
    client: &Client,
    http: &reqwest::Client,
    uuid: &str,
) -> Result<(LoginState, Option<Credential>), OmniError> {
    if !valid_token(uuid) {
        return Err(OmniError::Response("WeChat QR identifier is invalid".into()));
    }
    let url = reqwest::Url::parse_with_params(POLL, [("uuid", uuid), ("_", &super::musicu::now_millis().to_string())])
        .map_err(|_| OmniError::Response("WeChat poll URL is invalid".into()))?;
    let body = http
        .get(url)
        .header(reqwest::header::REFERER, "https://open.weixin.qq.com/")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(private_error)?
        .error_for_status()
        .map_err(private_error)?
        .text()
        .await
        .map_err(private_error)?;
    let status = assignment(&body, "window.wx_errcode")
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| OmniError::Response("WeChat poll response is malformed".into()))?;
    let state = match status {
        408 => LoginState::Waiting,
        404 => LoginState::Scanned,
        402 => LoginState::Expired,
        405 => {
            let code = assignment(&body, "window.wx_code")
                .map(|value| value.trim_matches(['\'', '"']))
                .filter(|code| valid_token(code))
                .ok_or_else(|| OmniError::Response("WeChat authorization code is missing".into()))?;
            let credential = client.wechat_login(code, APP_ID).await?;
            return Ok((LoginState::Confirmed, Some(credential)));
        }
        code => LoginState::Failed { code, message: None },
    };
    Ok((state, None))
}

fn assignment<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    body.split(';').find_map(|statement| {
        let (key, value) = statement.trim().split_once('=')?;
        (key.trim() == name).then(|| value.trim())
    })
}

fn valid_token(value: &str) -> bool {
    (8..=128).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

fn private_error(error: reqwest::Error) -> OmniError {
    OmniError::Http(error.without_url())
}
