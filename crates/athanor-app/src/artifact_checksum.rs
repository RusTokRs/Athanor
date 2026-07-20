use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_projector_support::replace_output_file;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) const SHA256_PREFIX: &str = "sha256:";
const MANIFEST_NAME: &str = "manifest.json";
const READ_MODEL_FILES: [&str; 4] = [
    "diagnostics.jsonl",
    "entities.jsonl",
    "facts.jsonl",
    "relations.jsonl",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadModelChecksums {
    algorithm: String,
    files: BTreeMap<String, String>,
}

pub(crate) fn seal_read_model(directory: &Path) -> Result<String> {
    let files = read_model_files(directory)?;
    let mut checksums = BTreeMap::new();
    for (name, path) in files {
        checksums.insert(name, sha256_file(&path)?);
    }

    let manifest_path = directory.join(MANIFEST_NAME);
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path)
            .with_context(|| format!("failed to read manifest {}", manifest_path.display()))?,
    )
    .with_context(|| format!("failed to parse manifest {}", manifest_path.display()))?;
    let object = manifest.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "read-model manifest {} is not an object",
            manifest_path.display()
        )
    })?;
    object.insert(
        "checksums".to_string(),
        serde_json::to_value(ReadModelChecksums {
            algorithm: "sha256".to_string(),
            files: checksums,
        })?,
    );
    let content = serde_json::to_string_pretty(&manifest)?;
    replace_output_file(&manifest_path, &content, "checksummed read-model manifest")
        .map_err(anyhow::Error::new)?;
    sha256_file(&manifest_path)
}

pub(crate) fn validate_read_model(directory: &Path, expected_manifest_digest: &str) -> Result<()> {
    validate_digest_format(expected_manifest_digest)?;
    let manifest_path = directory.join(MANIFEST_NAME);
    validate_file_digest(
        &manifest_path,
        expected_manifest_digest,
        "read-model manifest",
    )?;

    let manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path)
            .with_context(|| format!("failed to read manifest {}", manifest_path.display()))?,
    )
    .with_context(|| format!("failed to parse manifest {}", manifest_path.display()))?;
    let checksums: ReadModelChecksums = serde_json::from_value(
        manifest
            .get("checksums")
            .cloned()
            .context("read-model manifest has no checksum set")?,
    )
    .context("read-model manifest has an invalid checksum set")?;
    if checksums.algorithm != "sha256" {
        bail!(
            "read-model manifest uses unsupported checksum algorithm `{}`",
            checksums.algorithm
        );
    }

    let actual_files = read_model_files(directory)?;
    let actual_names = actual_files.keys().cloned().collect::<BTreeSet<_>>();
    let expected_names = checksums.files.keys().cloned().collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        bail!(
            "read-model file set does not match manifest checksums: actual={actual_names:?}, expected={expected_names:?}"
        );
    }
    for (name, path) in actual_files {
        let expected = checksums
            .files
            .get(&name)
            .expect("validated checksum file set");
        validate_file_digest(&path, expected, &format!("read-model file `{name}`"))?;
    }
    Ok(())
}

pub(crate) fn validate_read_model_matches(source: &Path, target: &Path) -> Result<()> {
    let source_files = read_model_files(source)?;
    let target_files = read_model_files(target)?;
    for name in READ_MODEL_FILES {
        validate_file_matches(
            source_files
                .get(name)
                .expect("validated source read-model file set"),
            target_files
                .get(name)
                .expect("validated target read-model file set"),
            &format!("read-model file `{name}`"),
        )?;
    }
    Ok(())
}

pub(crate) fn validate_file_matches(source: &Path, target: &Path, label: &str) -> Result<()> {
    let source_digest = sha256_file(source)?;
    let target_digest = sha256_file(target)?;
    if source_digest != target_digest {
        bail!(
            "{label} differs between source {} and immutable target {}",
            source.display(),
            target.display()
        );
    }
    Ok(())
}

pub(crate) fn validate_file_digest(path: &Path, expected: &str, label: &str) -> Result<()> {
    validate_digest_format(expected)?;
    let actual = sha256_file(path)?;
    if actual != expected {
        bail!(
            "{label} {} checksum mismatch: expected {expected}, actual {actual}",
            path.display()
        );
    }
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect checksum target {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("checksum target is not a regular file: {}", path.display());
    }
    let file = File::open(path)
        .with_context(|| format!("failed to open checksum target {}", path.display()))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to hash {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{SHA256_PREFIX}{:x}", digest.finalize()))
}

fn read_model_files(directory: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let metadata = fs::symlink_metadata(directory).with_context(|| {
        format!(
            "failed to inspect read-model directory {}",
            directory.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "read-model path is not a regular directory: {}",
            directory.display()
        );
    }

    let mut files = BTreeMap::new();
    let mut manifest_found = false;
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to inspect read model {}", directory.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to inspect read model {}", directory.display()))?;
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == MANIFEST_NAME {
            if file_type.is_symlink() || !file_type.is_file() {
                bail!(
                    "read-model manifest is not a regular file: {}",
                    entry.path().display()
                );
            }
            manifest_found = true;
            continue;
        }
        if file_type.is_symlink() || !file_type.is_file() {
            bail!(
                "read-model checksum contract permits only direct regular files: {}",
                entry.path().display()
            );
        }
        files.insert(name, entry.path());
    }
    if !manifest_found {
        bail!("read model {} has no manifest", directory.display());
    }

    let actual = files.keys().cloned().collect::<BTreeSet<_>>();
    let expected = READ_MODEL_FILES
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        bail!("read-model data file set is invalid: actual={actual:?}, expected={expected:?}");
    }
    Ok(files)
}

fn validate_digest_format(digest: &str) -> Result<()> {
    let Some(hex) = digest.strip_prefix(SHA256_PREFIX) else {
        bail!("unsupported checksum value `{digest}`");
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid SHA-256 checksum value `{digest}`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seals_and_detects_read_model_tampering() {
        let root = test_root("tamper");
        write_read_model(&root);

        let manifest_digest = seal_read_model(&root).unwrap();
        validate_read_model(&root, &manifest_digest).unwrap();

        fs::write(root.join("entities.jsonl"), "tampered\n").unwrap();
        assert!(
            validate_read_model(&root, &manifest_digest)
                .unwrap_err()
                .to_string()
                .contains("checksum mismatch")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_extra_files_and_mismatched_recovery_targets() {
        let source = test_root("source");
        let target = test_root("target");
        write_read_model(&source);
        write_read_model(&target);
        validate_read_model_matches(&source, &target).unwrap();

        fs::write(target.join("substituted.jsonl"), "{}\n").unwrap();
        assert!(validate_read_model_matches(&source, &target).is_err());
        fs::remove_file(target.join("substituted.jsonl")).unwrap();
        fs::write(target.join("facts.jsonl"), "tampered\n").unwrap();
        assert!(validate_read_model_matches(&source, &target).is_err());

        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(target).unwrap();
    }

    fn write_read_model(root: &Path) {
        fs::create_dir_all(root).unwrap();
        for name in READ_MODEL_FILES {
            fs::write(root.join(name), "{}\n").unwrap();
        }
        fs::write(root.join(MANIFEST_NAME), r#"{"schema":"test"}"#).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "athanor-artifact-checksum-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
