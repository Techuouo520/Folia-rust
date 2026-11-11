// LX header parsing follows userApi/utils.ts, including per-field UTF-16 limits.

use folia_store::SourceMetadata;

use super::SourceError;

pub fn parse_metadata(script: &str) -> Result<SourceMetadata, SourceError> {
    let header = script.strip_prefix("/*").and_then(|text| text.split_once("*/").map(|(head, _)| head));
    let header = header.ok_or(SourceError::Header)?;
    if header.is_empty() {
        return Err(SourceError::Header);
    }
    let mut info = SourceMetadata::default();
    for line in header.lines() {
        let line = strip_one_space(line);
        let Some(line) = line.strip_prefix('*') else { continue };
        let Some(line) = strip_one_space(line).strip_prefix('@') else { continue };
        let Some(at) = line.find(char::is_whitespace) else { continue };
        let (key, value) = line.split_at(at);
        let value = value.trim();
        let (field, limit) = match key {
            "name" => (&mut info.name, 24),
            "description" => (&mut info.description, 36),
            "version" => (&mut info.version, 36),
            "author" => (&mut info.author, 56),
            "homepage" => (&mut info.homepage, 1024),
            _ => continue,
        };
        *field = truncate(value, limit);
    }
    if info.name.is_empty() {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        info.name = format!("user_api_{}", time.as_secs());
    }
    Ok(info)
}

fn strip_one_space(text: &str) -> &str {
    text.strip_prefix(char::is_whitespace).unwrap_or(text)
}

pub(super) fn truncate(text: &str, limit: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() > limit { format!("{}...", String::from_utf16_lossy(&units[..limit])) } else { text.to_owned() }
}
