// Credential identity includes the protocol and normalized endpoint so changing hosts never reuses a key.

use folia_store::AiSettings;
use ring::digest::{SHA256, digest};

use crate::Error;

/// Compares the normalized credential scope used by saved keys and connection forms.
pub fn same_endpoint(left: &AiSettings, right: &AiSettings) -> bool {
    left.protocol == right.protocol
        && matches!((crate::client::endpoint(left), crate::client::endpoint(right)), (Ok(left), Ok(right)) if left == right)
}

fn entry(service: &str, settings: &AiSettings) -> Result<keyring::Entry, Error> {
    let endpoint = crate::client::endpoint(settings)?;
    let identity = format!("{:?}|{}", settings.protocol, endpoint);
    let hash = digest(&SHA256, identity.as_bytes());
    let key: String = hash.as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(keyring::Entry::new(service, &format!("ai-{key}"))?)
}

pub fn save_key(service: &str, settings: &AiSettings, key: &str) -> Result<(), Error> {
    let key = key.trim();
    if key.is_empty() {
        return Err(Error::MissingKey);
    }
    entry(service, settings)?.set_password(key)?;
    Ok(())
}

pub fn has_key(service: &str, settings: &AiSettings) -> Result<bool, Error> {
    Ok(load_key(service, settings)?.is_some())
}

pub fn remove_key(service: &str, settings: &AiSettings) -> Result<(), Error> {
    match entry(service, settings)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn load_key(service: &str, settings: &AiSettings) -> Result<Option<String>, Error> {
    match entry(service, settings)?.get_password() {
        Ok(key) => Ok((!key.trim().is_empty()).then_some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
