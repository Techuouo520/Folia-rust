// Native scrobble/v1 PLV/PLD uploads; follows api-enhanced (MIT, assets/netease-api/LICENSE).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use folia_core::{AudioQuality, Track};
use reqwest::header::{COOKIE, HeaderValue};
use serde_json::{Value, json};

use super::{Netease, eapi::Session, ncbl};
use crate::{OmniError, PlaybackReport};

const URL: &str = "https://clientlog3.music.163.com/api/clientlog/encrypt/upload?multiupload=true";
const VERSION: &str = "3.1.35";
const BUILD: &str = "205293";

pub(super) fn eligible(track: &Track) -> bool {
    track
        .online
        .as_ref()
        .is_some_and(|song| song.provider == "netease" && song.id.parse::<u64>().is_ok_and(|id| id > 0))
}

pub(super) async fn send(provider: &Netease, track: &Track, report: PlaybackReport) -> Result<(), OmniError> {
    let session = provider.client.session();
    if !session.signed_in() {
        return Err(OmniError::SignedOut);
    }
    let song = track.online.as_ref().filter(|_| eligible(track)).ok_or(OmniError::Unsupported("this track"))?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let meta = metadata(&session, now)?;
    let cookies = meta
        .as_object()
        .into_iter()
        .flat_map(|object| object.iter())
        .map(|(key, value)| format!("{key}={}", value.as_str().unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("; ");
    let mut cookies =
        HeaderValue::from_str(&cookies).map_err(|_| OmniError::Response("invalid session header".into()))?;
    cookies.set_sensitive(true);
    let total = if report.total_seconds.is_finite() && report.total_seconds > 0.0 {
        report.total_seconds
    } else {
        report.played_seconds
    };
    for action in ["_plv", "_pld"] {
        if provider.client.session().music_u != session.music_u {
            return Err(OmniError::SignedOut);
        }
        let body = format!("{}\u{1}{action}\u{1}{}", now / 1000, record(&song.id, report, total, now, action));
        let payload = ncbl::encode(meta.to_string().as_bytes(), body.as_bytes())?;
        upload(provider, payload, cookies.clone()).await?;
    }
    tracing::info!(song = %song.id, seconds = report.played_seconds.round(), "listening report accepted");
    Ok(())
}

fn metadata(session: &Session, now: u128) -> Result<Value, OmniError> {
    let cid = format!("{:06x}.{now}.01.0", u32::from_le_bytes(ncbl::random()?) & 0xffffff);
    Ok(json!({
        "JSESSIONID-WYYY": "", "MUSIC_U": session.music_u.as_deref().unwrap_or_default(), "NMTID": "",
        "WEVNSM": "1.0.0", "WNMCID": cid, "__csrf": session.csrf.as_deref().unwrap_or_default(),
        "_iuqxldmzr_": "33", "_ntes_nnid": ",", "_ntes_nuid": "", "appver": format!("{VERSION}.{BUILD}"),
        "channel": "netease", "clientSign": "", "deviceId": session.device_id, "mode": "",
        "ntes_kaola_ad": "1", "os": "pc", "osver": "Microsoft-Windows-10-Professional-build-19045-64bit"
    }))
}

fn record(id: &str, report: PlaybackReport, total: f64, now: u128, action: &str) -> Value {
    let (level, bitrate) = match report.quality {
        AudioQuality::Standard => ("standard", 128),
        AudioQuality::Lossless => ("lossless", 999),
        AudioQuality::Hires => ("hires", 1999),
        _ => ("exhigh", 320),
    };
    let mut data = json!({
        "mode": "circulation", "download": 0, "alg": "", "status": "front", "id": id,
        "bitrate": bitrate, "type": "song", "is_listentogether": 0, "source": "list", "is_heart": 0,
        "resource_ratio": "", "resource_time": total.round(), "musiceffect_id": "", "app_mode": 2,
        "bitrate_level": level, "vipType": "", "fee": 1, "file": 4, "rightSource": 0,
        "sourceId": id, "sourcetype": "track", "libra_abt": "", "channel": "netease", "curStartChannel": ""
    });
    if action == "_plv" {
        data["_addrefer"] = json!(format!(
            "[F:63][{now}#933#{VERSION}#{BUILD}#c9156c3][e][2][23][cell_pc_songlist_song:2|page_pc_songlist_songflow|page_mine_like_music][{id}:song:x:x|:::|{id}:list::]"
        ));
        data["_multirefers"] = json!([
            "[F:26][s][18][_ai]",
            "[F:26][s][12][_ai]",
            format!(
                "[F:63][{now}#933#{VERSION}#{BUILD}#c9156c3][e][2][8][cell_pc_main_tab_entrance:6|page_pc_main_tab][我喜欢的音乐:spm::|:::]"
            ),
            "[F:26][s][5][_ai]",
            "[F:26][s][0][_ai]"
        ]);
    } else {
        data["_addrefer"] = json!(format!(
            "[F:63][{now}#616#{VERSION}#{BUILD}#c9156c3][e][2][92][btn_pc_cover_play|cell_pc_songlist_song:6|page_pc_songlist_songflow|page_mine_like_music][:::|{id}:song:x:x|:::|{id}:list::]"
        ));
        data["_multirefers"] = json!([
            "[F:26][s][87][_ai]",
            "[F:26][s][81][_ai]",
            "[F:26][s][75][_ai]",
            "[F:26][s][69][_ai]",
            "[F:26][s][63][_ai]"
        ]);
        data["time"] = json!(report.played_seconds.min(total).round());
        data["realtime"] = data["time"].clone();
        data["musiceffect_id"] = json!("1001");
        data["app_mode"] = json!(1);
        data["lyriceffect"] = json!("default");
        data["displayMode"] = json!("classic");
        data["fee"] = json!(8);
        data["end"] = json!("interrupt");
    }
    data
}

async fn upload(provider: &Netease, payload: Vec<u8>, cookies: HeaderValue) -> Result<(), OmniError> {
    let boundary = format!("{:032x}", u128::from_le_bytes(ncbl::random()?));
    let file = format!("op_{}_0_{}", std::process::id(), u32::from_le_bytes(ncbl::random()?));
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file}\"\r\nContent-Type: multipart/form-data\r\n\r\n").into_bytes();
    body.extend(payload);
    body.extend(format!("\r\n--{boundary}--\r\n").bytes());
    let answer: Value = provider.client.http().post(URL)
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .header(COOKIE, cookies).header("Referer", "https://music.163.com/di")
        .header("User-Agent", format!("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Safari/537.36 Chrome/91.0.4472.164 NeteaseMusicDesktop/{VERSION}"))
        .header("Accept-Language", "zh-CN,zh;q=0.8")
        .timeout(Duration::from_secs(15)).body(body).send().await?.error_for_status()?.json().await?;
    if answer.get("code").and_then(Value::as_u64) != Some(200)
        || !answer
            .pointer("/data/successfiles")
            .and_then(Value::as_array)
            .is_some_and(|files| files.iter().any(|name| name.as_str() == Some(&file)))
    {
        return Err(OmniError::Response("listening log was not acknowledged".into()));
    }
    Ok(())
}
