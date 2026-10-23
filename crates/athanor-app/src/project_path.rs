#[cfg(windows)]
use std::path::Prefix;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let mut components = path.components();

        if let Some(Component::Prefix(prefix)) = components.next() {
            match prefix.kind() {
                Prefix::VerbatimDisk(disk) => {
                    let drive = char::from(disk);
                    return PathBuf::from(format!("{drive}:\\")).join(components.as_path());
                }
                Prefix::VerbatimUNC(server, share) => {
                    return PathBuf::from(format!(
                        "\\\\{}\\{}",
                        server.to_string_lossy(),
                        share.to_string_lossy()
                    ))
                    .join(components.as_path());
                }
                _ => {}
            }
        }
    }

    path
}

/// Validates a user-requested project file and returns its portable relative path.
///
/// Both lexical escapes and existing symlinks that resolve outside the canonical project root are
/// rejected before changed-file validation can read or remove anything.
pub(crate) fn normalize_requested_file(root: &Path, file: &Path) -> Result<String> {
    let absolute = if file.is_absolute() {
        file.to_path_buf()
    } else {
        root.join(file)
    };
    let relative = absolute
        .strip_prefix(root)
        .with_context(|| format!("{} is outside {}", absolute.display(), root.display()))?;
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        anyhow::bail!("{} is outside {}", absolute.display(), root.display());
    }
    let candidate = root.join(relative);
    if candidate.exists() {
        let canonical = normalize_canonical_path(candidate.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize requested file {}",
                candidate.display()
            )
        })?);
        if !canonical.starts_with(root) {
            anyhow::bail!("{} is outside {}", candidate.display(), root.display());
        }
    }

    portable_relative_path(relative)
}

pub(crate) fn portable_relative_path(relative: &Path) -> Result<String> {
    let mut normalized = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().with_context(|| {
                    format!("{} is not a UTF-8 project path", relative.display())
                })?;
                normalized.push(part.to_string());
            }
            Component::CurDir => {}
            _ => anyhow::bail!("{} is not a portable relative path", relative.display()),
        }
    }
    if normalized.is_empty() {
        anyhow::bail!("{} is not a portable relative path", relative.display());
    }
    Ok(normalized.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_requested_relative_and_absolute_files() {
        let root = std::env::current_dir().unwrap();
        let relative = PathBuf::from("docs/README.md");
        let absolute = root.join("docs/development/agent-workflow.md");

        assert_eq!(
            normalize_requested_file(&root, &relative).unwrap(),
            "docs/README.md"
        );
        assert_eq!(
            normalize_requested_file(&root, &absolute).unwrap(),
            "docs/development/agent-workflow.md"
        );
    }

    #[test]
    fn rejects_requested_files_that_escape_the_project() {
        let root = std::env::current_dir().unwrap();
        let error = normalize_requested_file(&root, Path::new("../outside.rs"))
            .expect_err("parent-directory input must be rejected");

        assert!(error.to_string().contains("is outside"));
    }

    #[cfg(unix)]
    #[test]
    fn preserves_unix_backslashes_and_rejects_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let root = std::env::current_dir().unwrap();
        assert_eq!(
            normalize_requested_file(&root, Path::new("src/back\\slash.rs")).unwrap(),
            "src/back\\slash.rs"
        );
        let invalid = PathBuf::from(std::ffi::OsString::from_vec(vec![
            b's', b'r', b'c', b'/', 0xff,
        ]));
        let error = normalize_requested_file(&root, &invalid)
            .expect_err("non-UTF-8 requested paths must fail closed");
        assert!(error.to_string().contains("not a UTF-8"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_requested_symlinks_that_escape_the_project() {
        let root = std::env::temp_dir().join(format!(
            "athanor-validate-changed-root-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let outside = root.with_extension("outside.rs");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, "fn outside() {}\n").unwrap();
        let link = root.join("linked.rs");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let error = normalize_requested_file(&root, &link)
            .expect_err("a symlink outside the project must be rejected");

        assert!(error.to_string().contains("is outside"));
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_file(outside).unwrap();
    }
}
