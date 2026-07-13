use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub(super) fn manifest(path: &Path) -> Result<String> {
    hash_file(path, "failed to read")
}

pub(super) fn executable(path: &Path) -> Result<String> {
    hash_file(path, "failed to read adapter executable")
}

pub(super) fn executable_size(path: &Path) -> Result<u64> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .with_context(|| format!("failed to inspect adapter executable {}", path.display()))
}

fn hash_file(path: &Path, context: &str) -> Result<String> {
    let content = fs::read(path).with_context(|| format!("{context} {}", path.display()))?;
    let digest = Sha256::digest(&content);
    Ok(hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
