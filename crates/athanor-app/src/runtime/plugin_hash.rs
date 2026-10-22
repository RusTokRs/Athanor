use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

pub(super) fn manifest(path: &Path) -> Result<String> {
    Ok(digest(path, "adapter manifest")?.0)
}

pub(super) fn executable(path: &Path) -> Result<String> {
    Ok(digest(path, "adapter executable")?.0)
}

pub(super) fn executable_size(path: &Path) -> Result<u64> {
    Ok(inspect_regular_file(path, "adapter executable")?.len())
}

pub(super) fn digest(path: &Path, label: &str) -> Result<(String, u64)> {
    let metadata = inspect_regular_file(path, label)?;
    let file =
        File::open(path).with_context(|| format!("failed to open {label} {}", path.display()))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to hash {label} {}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    if bytes != metadata.len() {
        bail!(
            "{label} changed while hashing {}: observed {bytes} bytes, expected {}",
            path.display(),
            metadata.len()
        );
    }
    Ok((hex_encode(&hasher.finalize()), bytes))
}

fn inspect_regular_file(path: &Path, label: &str) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("{label} must be a regular file: {}", path.display());
    }
    Ok(metadata)
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
