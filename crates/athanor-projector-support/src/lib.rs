use std::fs;
use std::path::Path;

use athanor_core::{CoreError, CoreResult};
use athanor_domain::{Diagnostic, Entity, Fact, Relation};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct NewDirectoryPublication {
    target: std::path::PathBuf,
    staging: std::path::PathBuf,
    output_kind: String,
    published: bool,
}

impl NewDirectoryPublication {
    pub fn new(target: impl Into<std::path::PathBuf>, output_kind: &str) -> CoreResult<Self> {
        let target = target.into();
        if target.exists() {
            return Err(adapter_error(format!(
                "immutable {output_kind} already exists: {}",
                target.display()
            )));
        }
        let parent = target.parent().ok_or_else(|| {
            adapter_error(format!(
                "{output_kind} target has no parent: {}",
                target.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(io_error("create output parent", parent))?;
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                adapter_error(format!(
                    "invalid {output_kind} target: {}",
                    target.display()
                ))
            })?;
        let staging = parent.join(format!(".{name}.tmp-{}", std::process::id()));
        remove_path_if_exists(&staging)?;
        fs::create_dir_all(&staging).map_err(io_error("create staging directory", &staging))?;

        Ok(Self {
            target,
            staging,
            output_kind: output_kind.to_string(),
            published: false,
        })
    }

    pub fn staging_path(&self) -> &Path {
        &self.staging
    }

    pub fn publish(mut self) -> CoreResult<()> {
        if self.target.exists() {
            return Err(adapter_error(format!(
                "immutable {} appeared during publication: {}",
                self.output_kind,
                self.target.display()
            )));
        }
        fs::rename(&self.staging, &self.target)
            .map_err(io_error("publish immutable output directory", &self.target))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for NewDirectoryPublication {
    fn drop(&mut self) {
        if !self.published {
            let _ = remove_path_if_exists(&self.staging);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalProjectionPayload {
    pub schema: String,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn publish_staged_directory(
    target: &Path,
    output_kind: &str,
    build: impl FnOnce(&Path) -> CoreResult<()>,
) -> CoreResult<()> {
    let parent = target.parent().ok_or_else(|| {
        adapter_error(format!(
            "{output_kind} target has no parent: {}",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(io_error("create output parent", parent))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            adapter_error(format!(
                "invalid {output_kind} target: {}",
                target.display()
            ))
        })?;
    let suffix = std::process::id();
    let staging = parent.join(format!(".{name}.tmp-{suffix}"));
    let backup = parent.join(format!(".{name}.backup-{suffix}"));
    remove_path_if_exists(&staging)?;
    remove_path_if_exists(&backup)?;
    fs::create_dir_all(&staging).map_err(io_error("create staging directory", &staging))?;

    if let Err(error) = build(&staging) {
        let _ = remove_path_if_exists(&staging);
        return Err(error);
    }

    if target.exists() {
        fs::rename(target, &backup).map_err(io_error("stage previous output", target))?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if backup.exists() {
            let _ = fs::rename(&backup, target);
        }
        return Err(adapter_error(format!(
            "replace {output_kind} directory {}: {error}",
            target.display()
        )));
    }
    remove_path_if_exists(&backup)
}

pub fn write_output_file(path: &Path, content: &str) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create output directory", parent))?;
    }
    fs::write(path, content).map_err(io_error("write output file", path))
}

pub fn replace_output_file(target: &Path, content: &str, output_kind: &str) -> CoreResult<()> {
    let parent = target.parent().ok_or_else(|| {
        adapter_error(format!(
            "{output_kind} target has no parent: {}",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(io_error("create output parent", parent))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            adapter_error(format!(
                "invalid {output_kind} target: {}",
                target.display()
            ))
        })?;
    let suffix = std::process::id();
    let staging = parent.join(format!(".{name}.tmp-{suffix}"));
    let backup = parent.join(format!(".{name}.backup-{suffix}"));
    remove_path_if_exists(&staging)?;
    remove_path_if_exists(&backup)?;
    fs::write(&staging, content).map_err(io_error("write staged output file", &staging))?;

    if target.exists() {
        fs::rename(target, &backup).map_err(io_error("stage previous output file", target))?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if backup.exists() {
            let _ = fs::rename(&backup, target);
        }
        let _ = remove_path_if_exists(&staging);
        return Err(adapter_error(format!(
            "replace {output_kind} file {}: {error}",
            target.display()
        )));
    }
    remove_path_if_exists(&backup)
}

pub fn safe_filename(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            output.push(char::from(byte));
        } else {
            output.push('~');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    output
}

fn remove_path_if_exists(path: &Path) -> CoreResult<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(io_error("remove output directory", path))?;
    } else if path.exists() {
        fs::remove_file(path).map_err(io_error("remove output file", path))?;
    }
    Ok(())
}

fn io_error<'a>(
    action: &'static str,
    path: &'a Path,
) -> impl FnOnce(std::io::Error) -> CoreError + 'a {
    move |error| adapter_error(format!("{action} {}: {error}", path.display()))
}

fn adapter_error(message: String) -> CoreError {
    CoreError::Adapter(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_complete_directory_and_removes_stale_files() {
        let root = std::env::temp_dir().join(format!(
            "athanor-projector-support-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("report");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("stale.txt"), "stale").unwrap();

        publish_staged_directory(&target, "test", |staging| {
            write_output_file(&staging.join("index.txt"), "complete")
        })
        .unwrap();

        assert!(!target.join("stale.txt").exists());
        assert_eq!(
            fs::read_to_string(target.join("index.txt")).unwrap(),
            "complete"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_filenames_are_collision_free_for_escaped_bytes() {
        assert_ne!(safe_filename("a/b"), safe_filename("a_b"));
        assert_eq!(safe_filename("entity_1"), "entity_1");
    }

    #[test]
    fn publishes_immutable_directory_once() {
        let root = std::env::temp_dir().join(format!(
            "athanor-immutable-publication-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("generations/00000001");
        let publication = NewDirectoryPublication::new(&target, "generation").unwrap();
        write_output_file(
            &publication.staging_path().join("manifest.json"),
            "complete",
        )
        .unwrap();
        publication.publish().unwrap();

        assert_eq!(
            fs::read_to_string(target.join("manifest.json")).unwrap(),
            "complete"
        );
        assert!(NewDirectoryPublication::new(&target, "generation").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaces_pointer_file() {
        let root = std::env::temp_dir().join(format!(
            "athanor-pointer-publication-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("current.json");
        replace_output_file(&target, "one", "pointer").unwrap();
        replace_output_file(&target, "two", "pointer").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "two");
        fs::remove_dir_all(root).unwrap();
    }
}
