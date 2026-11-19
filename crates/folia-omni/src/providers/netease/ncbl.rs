// NCBL v3 framing from api-enhanced/util/ncbl.js (MIT, assets/netease-api/LICENSE).

use std::io::Write;

use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use flate2::{Compression, write::GzEncoder};
use rsa::BigUint;
use rsa::rand_core::{OsRng, RngCore};

use crate::OmniError;

pub(super) fn random<const N: usize>() -> Result<[u8; N], OmniError> {
    let mut bytes = [0; N];
    OsRng.try_fill_bytes(&mut bytes).map_err(|_| OmniError::Response("report entropy unavailable".into()))?;
    Ok(bytes)
}

pub(super) fn encode(meta: &[u8], body: &[u8]) -> Result<Vec<u8>, OmniError> {
    let mut key = random::<32>()?;
    key[0] = key[0].min(0xa2);
    let mut uuid = random::<16>()?;
    uuid[6] = (uuid[6] & 0x0f) | 0x40;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    frame(meta, body, key, uuid, u16::from_le_bytes(random()?) as u32)
}

fn frame(meta: &[u8], body: &[u8], key: [u8; 32], uuid: [u8; 16], sequence: u32) -> Result<Vec<u8>, OmniError> {
    let modulus = BigUint::parse_bytes(b"fd90bd466ff9bc8a3fec2fbcf263b90d5c564879fa5d7aab89b31c1d5cb4139d", 16)
        .ok_or_else(|| OmniError::Response("invalid NCBL modulus".into()))?;
    let wrapped = BigUint::from_bytes_be(&key).modpow(&BigUint::from(65537_u32), &modulus).to_bytes_be();
    let mut wrapped_key = [0; 32];
    wrapped_key[32 - wrapped.len()..].copy_from_slice(&wrapped);
    let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
    compressed.write_all(body).map_err(compression_error)?;
    let compressed = compressed.finish().map_err(compression_error)?;
    let meta_len =
        u16::try_from(meta.len() + 74).map_err(|_| OmniError::Response("report metadata too large".into()))?;
    let mut output = Vec::with_capacity(meta_len as usize + compressed.len() + 12);
    output.extend_from_slice(b"NCBL");
    output.extend_from_slice(&3_u32.to_le_bytes());
    output.extend_from_slice(&meta_len.to_le_bytes());
    output.extend_from_slice(&uuid);
    output.extend_from_slice(&wrapped_key);
    let count = compressed.len().div_ceil(0x8000) as u32;
    output.extend_from_slice(&sequence.to_le_bytes());
    output.extend_from_slice(&(sequence + count - 1).to_le_bytes());
    output.extend_from_slice(&(compressed.len() as u32 + count * 6).to_le_bytes());
    output.extend_from_slice(&0x4343_u16.to_le_bytes());
    output.extend_from_slice(&(meta.len() as u16).to_le_bytes());
    encrypt_into(&mut output, meta, &wrapped_key, &uuid)?;
    for (index, chunk) in compressed.chunks(0x8000).enumerate() {
        output.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
        output.extend_from_slice(&(sequence + index as u32).to_le_bytes());
        encrypt_into(&mut output, chunk, &key, &uuid)?;
    }
    Ok(output)
}

fn encrypt_into(output: &mut Vec<u8>, bytes: &[u8], key: &[u8; 32], uuid: &[u8; 16]) -> Result<(), OmniError> {
    let offset = output.len();
    output.extend_from_slice(bytes);
    let counter = u32::from_le_bytes([uuid[12], uuid[13], uuid[14], uuid[15]]) >> 2;
    let mut cipher = chacha20::ChaCha20::new_from_slices(key, &uuid[..12])
        .map_err(|_| OmniError::Response("invalid NCBL key".into()))?;
    cipher.try_seek(u64::from(counter) * 64).map_err(|_| OmniError::Response("invalid NCBL counter".into()))?;
    cipher.try_apply_keystream(&mut output[offset..]).map_err(|_| OmniError::Response("NCBL frame too large".into()))
}

fn compression_error(error: std::io::Error) -> OmniError {
    OmniError::Io { path: "NCBL report".into(), source: error }
}
