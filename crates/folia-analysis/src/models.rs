// Validates the pinned MIT model exports and their packed tensor manifests before GPU upload.
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use crate::AnalysisError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    BeatThis,
    Htdemucs,
}

impl ModelKind {
    pub fn file(self) -> &'static str {
        match self {
            Self::BeatThis => "beat_this-wgsl.bin",
            Self::Htdemucs => "htdemucs-wgsl.bin",
        }
    }

    pub fn specification(self) -> Result<Specification, AnalysisError> {
        #[derive(Deserialize)]
        struct Package {
            models: Vec<Specification>,
        }
        let package: Package = toml::from_str(include_str!("../../../assets/models/manifest.toml"))?;
        package
            .models
            .into_iter()
            .find(|model| {
                model.file == self.file()
                    && model.bytes >= 16
                    && model.sha256.len() == 64
                    && model.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| AnalysisError::Model(format!("missing pinned specification for {}", self.file())))
    }
}

#[derive(Debug, Deserialize)]
pub struct Specification {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct Tensor {
    pub byte_offset: u64,
    pub shape: Vec<usize>,
    pub dtype: Dtype,
    pub stride: Option<usize>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dtype {
    F32,
    F16,
}

impl Tensor {
    pub fn bytes(&self) -> Option<u64> {
        let elements = if let Some(stride) = self.stride {
            self.shape.first()?.checked_mul(stride)?
        } else {
            self.shape.iter().try_fold(1usize, |size, dimension| size.checked_mul(*dimension))?
        };
        let bytes = elements.checked_mul(if self.dtype == Dtype::F16 { 2 } else { 4 })?;
        Some(bytes.div_ceil(4) as u64 * 4)
    }
}

#[derive(Debug, Deserialize)]
pub struct HyperParameters {
    pub transformer_dim: usize,
    pub n_layers: usize,
    pub stem_dim: usize,
    pub ff_mult: usize,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub bytes: usize,
    pub tensors: BTreeMap<String, Tensor>,
    pub segment: Option<usize>,
    pub hyper_parameters: Option<HyperParameters>,
}

pub struct Weights {
    pub(crate) data: Vec<u8>,
    pub(crate) manifest: Manifest,
}

impl Weights {
    pub fn load(path: &Path, kind: ModelKind) -> Result<Self, AnalysisError> {
        let specification = kind.specification()?;
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() != specification.bytes as u64 {
            return Err(AnalysisError::Model(format!("{} has an unexpected byte count", kind.file())));
        }
        let mut data = Vec::with_capacity(specification.bytes);
        file.take(specification.bytes as u64 + 1).read_to_end(&mut data)?;
        if data.len() != specification.bytes {
            return Err(AnalysisError::Model(format!("{} has an unexpected byte count", kind.file())));
        }
        let digest = ring::digest::digest(&ring::digest::SHA256, &data);
        let digest: String = digest.as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
        if digest != specification.sha256 {
            return Err(AnalysisError::Model(format!("{} failed SHA256 verification", kind.file())));
        }
        let tail = data.len() - 16;
        if &data[tail..tail + 8] != b"FOLIABTW" {
            return Err(AnalysisError::Model("missing FOLIABTW trailer".to_owned()));
        }
        let mut offset = [0u8; 8];
        offset.copy_from_slice(&data[tail + 8..]);
        let offset = usize::try_from(u64::from_le_bytes(offset))
            .map_err(|_| AnalysisError::Model("tensor manifest offset overflow".to_owned()))?;
        if offset == 0 || offset >= tail {
            return Err(AnalysisError::Model("tensor manifest is outside the file".to_owned()));
        }
        let manifest: Manifest = serde_json::from_slice(&data[offset..tail])?;
        if manifest.bytes != offset {
            return Err(AnalysisError::Model("tensor bytes disagree with the manifest".to_owned()));
        }
        for (name, tensor) in &manifest.tensors {
            let length = tensor.bytes().ok_or_else(|| AnalysisError::Model(format!("invalid tensor {name}")))?;
            if length == 0 || tensor.byte_offset.checked_add(length).is_none_or(|end| end > offset as u64) {
                return Err(AnalysisError::Model(format!("tensor {name} is outside the weights")));
            }
        }
        data.truncate(offset);
        Ok(Self { data, manifest })
    }
}
