// Cadenza stores nothing the window can edit. The original playground has no panel for it
// ("这个样式没有参数"); the picture uses the fixed envelopes in `envelopes.rs`.

use crate::tuning::Manifest;

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("cadenza_tuning", &["cadenze_tuning"], &[]);
    manifest.reset_key = Some("notifications.cadenzaReset");
    manifest
}
