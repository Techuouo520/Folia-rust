// Platform sessions kept in the system keyring: one vault entry for every provider, so a
// rebuilt (re-signed) app asks the keychain once instead of once per account.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::OmniError;

const VAULT_ACCOUNT: &str = "accounts-vault";

/// One keychain read per launch: every extra read is another authorization dialog.
static CACHE: Mutex<Option<(String, Vault)>> = Mutex::new(None);

#[derive(Clone, Default, Serialize, Deserialize)]
struct Vault {
    #[serde(default)]
    version: u8,
    #[serde(default)]
    entries: BTreeMap<String, String>,
}

fn legacy_entry(service: &str, provider: &str) -> Result<keyring::Entry, OmniError> {
    Ok(keyring::Entry::new(service, &format!("{provider}-session"))?)
}

fn vault_entry(service: &str) -> Result<keyring::Entry, OmniError> {
    Ok(keyring::Entry::new(service, VAULT_ACCOUNT)?)
}

/// An unreadable vault is an error, never an empty one: treating it as empty would drop every session.
fn read(service: &str) -> Result<Option<Vault>, OmniError> {
    if let Ok(cache) = CACHE.lock() {
        if let Some((cached_service, vault)) = cache.as_ref() {
            if cached_service == service {
                return Ok(Some(vault.clone()));
            }
        }
    }
    let parsed = match vault_entry(service)?.get_password() {
        Ok(raw) => match serde_json::from_str::<Vault>(&raw) {
            Ok(mut vault) => {
                vault.version = 1;
                Ok(Some(vault))
            }
            Err(err) => {
                tracing::warn!(error = %err, "account vault unreadable");
                Err(OmniError::Response(format!("account vault unreadable: {err}")))
            }
        },
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => return Err(err.into()),
    }?;
    if let (Ok(mut cache), Some(vault)) = (CACHE.lock(), parsed.as_ref()) {
        *cache = Some((service.to_owned(), vault.clone()));
    }
    Ok(parsed)
}

fn write(service: &str, data: &Vault) -> Result<(), OmniError> {
    let raw = serde_json::to_string(data).map_err(|err| OmniError::Response(err.to_string()))?;
    vault_entry(service)?.set_password(&raw)?;
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((service.to_owned(), data.clone()));
    }
    Ok(())
}

pub(crate) fn load(service: &str, provider: &str) -> Result<Option<String>, OmniError> {
    if let Some(vault) = read(service)? {
        if let Some(value) = vault.entries.get(provider) {
            return Ok(Some(value.clone()));
        }
    }
    // A session written before the vault existed: copy it in, confirm the copy, then drop it.
    let value = match legacy_entry(service, provider)?.get_password() {
        Ok(value) => value,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    save(service, provider, &value)?;
    match read(service)?.and_then(|vault| vault.entries.get(provider).cloned()) {
        Some(stored) if stored == value => match legacy_entry(service, provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => tracing::warn!(%err, provider, "could not remove the old keyring entry"),
        },
        _ => tracing::warn!(provider, "vault copy unconfirmed, keeping the old keyring entry"),
    }
    Ok(Some(value))
}

pub(crate) fn save(service: &str, provider: &str, value: &str) -> Result<(), OmniError> {
    let mut vault = read(service)?.unwrap_or_default();
    vault.version = 1;
    vault.entries.insert(provider.to_owned(), value.to_owned());
    write(service, &vault)?;
    tracing::info!(provider, "session kept in the system keyring");
    Ok(())
}

pub(crate) fn delete(service: &str, provider: &str) -> Result<(), OmniError> {
    if let Some(mut vault) = read(service)? {
        if vault.entries.remove(provider).is_some() {
            write(service, &vault)?;
        }
    }
    match legacy_entry(service, provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
