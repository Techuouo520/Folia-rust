// Validates the upstream cinema mesh assets and exposes the same room to the audio renderer.

use std::io::Read;
use std::sync::{Arc, OnceLock};

use folia_core::{AcousticMaterial, AcousticMesh, AcousticWorld};

pub(super) const ROOM: &[u8] = include_bytes!("../../../../assets/cinema/room.mesh");
pub(super) const SEAT: &[u8] = include_bytes!("../../../../assets/cinema/seat.mesh");
pub(super) const AUDIENCE: &[u8] = include_bytes!("../../../../assets/cinema/audience.mesh");
pub(super) const CURTAINS: &[u8] = include_bytes!("../../../../assets/cinema/curtains.mesh");
pub(super) const SEAT_MATRICES: &[u8] = include_bytes!("../../../../assets/cinema/seats.bin");
pub(super) const AUDIENCE_MATRICES: &[u8] = include_bytes!("../../../../assets/cinema/audience_instances.bin");
pub(super) const GLOWS: &[u8] = include_bytes!("../../../../assets/cinema/glows.bin");

pub(super) struct MeshData {
    pub vertices: &'static [u8],
    pub indices: &'static [u8],
    pub count: u32,
}

pub(super) fn mesh(bytes: &'static [u8], name: &'static str) -> Result<MeshData, CinemaError> {
    let mut asset = Asset { bytes, name };
    if asset.bytes::<4>()? != *b"FCRM" || asset.u32()? != 1 {
        return Err(CinemaError::Asset(name));
    }
    let vertices = asset.u32()?;
    let count = asset.u32()?;
    if 16 + u64::from(vertices) * 40 + u64::from(count) * 4 != bytes.len() as u64 {
        return Err(CinemaError::Asset(name));
    }
    let (vertices, indices) = asset.bytes.split_at(vertices as usize * 40);
    Ok(MeshData { vertices, indices, count })
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CinemaError {
    #[error("invalid embedded cinema asset {0}")]
    Asset(&'static str),
    #[error("cinema graphics initialization failed")]
    Graphics(#[source] Arc<wgpu::Error>),
}

struct Asset<'a> {
    bytes: &'a [u8],
    name: &'static str,
}

impl Asset<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], CinemaError> {
        let mut value = [0; N];
        self.bytes.read_exact(&mut value).map_err(|_| CinemaError::Asset(self.name))?;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, CinemaError> {
        self.bytes().map(u32::from_le_bytes)
    }

    fn f32(&mut self) -> Result<f32, CinemaError> {
        self.bytes().map(f32::from_le_bytes)
    }

    fn vector(&mut self) -> Result<[f32; 3], CinemaError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
}

pub fn acoustic_world() -> Result<Arc<AcousticWorld>, CinemaError> {
    static WORLD: OnceLock<Result<Arc<AcousticWorld>, CinemaError>> = OnceLock::new();
    WORLD.get_or_init(|| decode_acoustics().map(Arc::new)).clone()
}

fn decode_acoustics() -> Result<AcousticWorld, CinemaError> {
    const BYTES: &[u8] = include_bytes!("../../../../assets/cinema/acoustics.bin");
    let mut asset = Asset { bytes: BYTES, name: "acoustics" };
    if asset.bytes::<4>()? != *b"FCRA" || asset.u32()? != 1 {
        return Err(CinemaError::Asset(asset.name));
    }
    let vertices = asset.u32()?;
    let triangles = asset.u32()?;
    let materials = asset.u32()?;
    let expected = 20 + u64::from(vertices) * 12 + u64::from(triangles) * 14 + u64::from(materials) * 28;
    if expected != BYTES.len() as u64 {
        return Err(CinemaError::Asset(asset.name));
    }
    let positions = (0..vertices).map(|_| asset.vector()).collect::<Result<Vec<_>, _>>()?;
    let faces = (0..triangles)
        .map(|_| Ok([asset.u32()?, asset.u32()?, asset.u32()?]))
        .collect::<Result<Vec<_>, CinemaError>>()?;
    let triangle_materials =
        (0..triangles).map(|_| asset.bytes().map(u16::from_le_bytes)).collect::<Result<Vec<_>, _>>()?;
    if faces.iter().flatten().any(|index| *index >= vertices)
        || triangle_materials.iter().any(|index| u32::from(*index) >= materials)
    {
        return Err(CinemaError::Asset(asset.name));
    }
    let materials = (0..materials)
        .map(|_| {
            Ok(AcousticMaterial {
                absorption: asset.vector()?,
                scattering: asset.f32()?,
                transmission: asset.vector()?,
            })
        })
        .collect::<Result<Vec<_>, CinemaError>>()?;
    Ok(AcousticWorld { meshes: vec![AcousticMesh { positions, triangles: faces, triangle_materials }], materials })
}
