// Cadenza stores nothing the window can edit. The original playground has no panel for it
// ("这个样式没有参数"); the picture uses the fixed envelopes in `envelopes.rs`.

use crate::tuning::Manifest;

pub fn manifest() -> Manifest {
    Manifest { storage_key: "cadenza_tuning", older_keys: &["cadenze_tuning"], groups: &[] }
}
