// Bounded Node-compatible buffer, crypto and zlib operations for the LX sandbox.

use std::io::{Read, Write};

use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use base64::{Engine, engine::general_purpose::STANDARD};
use md5::{Digest, Md5};
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::traits::PublicKeyParts;
use serde_json::{Value, json};

use super::SourceError;

pub(super) const BUFFER_LIMIT: usize = 4 * 1024 * 1024;

pub(super) fn dispatch(op: &str, args: &Value) -> Result<Value, SourceError> {
    let result = match op {
        "encode" => json!(encode(string(&args[0])?, string(&args[1])?)?),
        "decode" => json!(decode(&bytes(&args[0])?, string(&args[1])?)?),
        "md5" => json!(format!("{:x}", Md5::digest(bytes(&args[0])?))),
        "random" => {
            let size = args[0].as_u64().filter(|n| *n <= BUFFER_LIMIT as u64).ok_or(SourceError::Limit)?;
            let mut bytes = vec![0; size as usize];
            ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
                .map_err(|_| SourceError::Crypto)?;
            json!(bytes)
        }
        "aes" => json!(encrypt_aes(&bytes(&args[0])?, string(&args[1])?, &bytes(&args[2])?, &bytes(&args[3])?)?),
        "rsa" => json!(encrypt_rsa(&bytes(&args[0])?, string(&args[1])?)?),
        "inflate" => {
            let data = bytes(&args[0])?;
            let mut result = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .take(BUFFER_LIMIT as u64 + 1)
                .read_to_end(&mut result)
                .map_err(|_| SourceError::Compression)?;
            if result.len() > BUFFER_LIMIT {
                return Err(SourceError::Limit);
            }
            json!(result)
        }
        "deflate" => {
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&bytes(&args[0])?).map_err(|_| SourceError::Compression)?;
            json!(encoder.finish().map_err(|_| SourceError::Compression)?)
        }
        _ => return Err(SourceError::Protocol),
    };
    if result.as_array().is_some_and(|bytes| bytes.len() > BUFFER_LIMIT) {
        return Err(SourceError::Limit);
    }
    Ok(result)
}

pub(super) fn bytes(value: &Value) -> Result<Vec<u8>, SourceError> {
    let array = value.as_array().filter(|a| a.len() <= BUFFER_LIMIT).ok_or(SourceError::Limit)?;
    array.iter().map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()).ok_or(SourceError::Protocol)).collect()
}

fn string(value: &Value) -> Result<&str, SourceError> {
    value.as_str().filter(|s| s.len() <= BUFFER_LIMIT).ok_or(SourceError::Limit)
}

fn encode(text: &str, encoding: &str) -> Result<Vec<u8>, SourceError> {
    Ok(match encoding.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => text.as_bytes().to_vec(),
        "latin1" | "binary" | "ascii" => text.encode_utf16().map(|n| n as u8).collect(),
        "utf16le" | "utf-16le" | "ucs2" | "ucs-2" => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        "hex" => {
            let mut out = Vec::new();
            for pair in text.as_bytes().as_chunks::<2>().0 {
                let Some(a) = (pair[0] as char).to_digit(16) else { break };
                let Some(b) = (pair[1] as char).to_digit(16) else { break };
                out.push((a * 16 + b) as u8);
            }
            out
        }
        "base64" | "base64url" => {
            let mut input: String = text.chars().filter(|c| c.is_ascii_alphanumeric() || "+/-_".contains(*c)).collect();
            input = input.replace('-', "+").replace('_', "/");
            if input.len() % 4 == 1 {
                input.pop();
            }
            while !input.len().is_multiple_of(4) {
                input.push('=');
            }
            STANDARD.decode(input).map_err(|_| SourceError::Protocol)?
        }
        _ => return Err(SourceError::Protocol),
    })
}

fn decode(data: &[u8], encoding: &str) -> Result<String, SourceError> {
    Ok(match encoding.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => String::from_utf8_lossy(data).into_owned(),
        "latin1" | "binary" => data.iter().map(|b| char::from(*b)).collect(),
        "ascii" => data.iter().map(|b| char::from(b & 0x7f)).collect(),
        "utf16le" | "utf-16le" | "ucs2" | "ucs-2" => String::from_utf16_lossy(
            &data.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect::<Vec<_>>(),
        ),
        "base64" => STANDARD.encode(data),
        "base64url" => base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data),
        "hex" => data.iter().map(|b| format!("{b:02x}")).collect(),
        _ => return Err(SourceError::Protocol),
    })
}

fn encrypt_rsa(data: &[u8], pem: &str) -> Result<Vec<u8>, SourceError> {
    if data.len() > 128 {
        return Err(SourceError::Crypto);
    }
    let key = rsa::RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| rsa::RsaPublicKey::from_pkcs1_pem(pem))
        .map_err(|_| SourceError::Crypto)?;
    // LX explicitly pads to 128 bytes before RSA_NO_PADDING, so other key sizes are rejected too.
    if key.size() != 128 {
        return Err(SourceError::Crypto);
    }
    let modulus = num_bigint::BigUint::from_bytes_be(&key.n().to_bytes_be());
    let exponent = num_bigint::BigUint::from_bytes_be(&key.e().to_bytes_be());
    let message = num_bigint::BigUint::from_bytes_be(data);
    if message >= modulus {
        return Err(SourceError::Crypto);
    }
    let result = message.modpow(&exponent, &modulus).to_bytes_be();
    let mut padded = vec![0; 128 - result.len()];
    padded.extend(result);
    Ok(padded)
}

fn encrypt_aes(data: &[u8], mode: &str, key: &[u8], iv: &[u8]) -> Result<Vec<u8>, SourceError> {
    match key.len() {
        16 => cipher::<aes::Aes128>(data, mode, key, iv, "aes-128-"),
        24 => cipher::<aes::Aes192>(data, mode, key, iv, "aes-192-"),
        32 => cipher::<aes::Aes256>(data, mode, key, iv, "aes-256-"),
        _ => Err(SourceError::Crypto),
    }
}

fn cipher<C: BlockEncrypt + KeyInit + aes::cipher::BlockSizeUser<BlockSize = aes::cipher::consts::U16>>(
    data: &[u8],
    mode: &str,
    key: &[u8],
    iv: &[u8],
    prefix: &str,
) -> Result<Vec<u8>, SourceError> {
    let mode = mode.strip_prefix(prefix).ok_or(SourceError::Crypto)?;
    if !matches!(mode, "ecb" | "cbc" | "ctr" | "cfb" | "ofb")
        || (mode != "ecb" && iv.len() != 16)
        || (mode == "ecb" && !iv.is_empty())
    {
        return Err(SourceError::Crypto);
    }
    let cipher = C::new_from_slice(key).map_err(|_| SourceError::Crypto)?;
    let mut state = [0_u8; 16];
    if !iv.is_empty() {
        state.copy_from_slice(iv);
    }
    let mut result = data.to_vec();
    if matches!(mode, "cbc" | "ecb") {
        let padding = 16 - result.len() % 16;
        result.resize(result.len() + padding, padding as u8);
    }
    for chunk in result.chunks_mut(16) {
        if matches!(mode, "cbc" | "ecb") {
            if mode == "cbc" {
                for (byte, previous) in chunk.iter_mut().zip(state) {
                    *byte ^= previous;
                }
            }
            cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
            state.copy_from_slice(chunk);
        } else {
            let mut stream = state;
            cipher.encrypt_block(GenericArray::from_mut_slice(&mut stream));
            for (byte, mask) in chunk.iter_mut().zip(stream) {
                *byte ^= mask;
            }
            match mode {
                "cfb" => state[..chunk.len()].copy_from_slice(chunk),
                "ofb" => state = stream,
                _ => {
                    for byte in state.iter_mut().rev() {
                        *byte = byte.wrapping_add(1);
                        if *byte != 0 {
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}
