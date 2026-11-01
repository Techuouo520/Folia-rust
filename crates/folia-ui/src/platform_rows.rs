// Shared provider ordering for home account entries, the switcher and the login window.

use folia_omni::Platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRowStatus {
    SignedIn,
    RanOut,
    SignedOut,
}

/// Orders provider rows by the active account, signed-in accounts, expired accounts, then registration order.
pub fn order_provider_rows(
    providers: impl IntoIterator<Item = (Platform, ProviderRowStatus)>,
    active: Option<&str>,
) -> Vec<Platform> {
    let mut rows: Vec<_> = providers.into_iter().collect();
    rows.sort_by_key(|(platform, status)| {
        if Some(platform.id) == active {
            return 0;
        }
        match status {
            ProviderRowStatus::SignedIn => 1,
            ProviderRowStatus::RanOut => 2,
            ProviderRowStatus::SignedOut => 3,
        }
    });
    rows.into_iter().map(|(platform, _)| platform).collect()
}
