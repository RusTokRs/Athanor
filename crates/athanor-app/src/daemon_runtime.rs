use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use fs2::FileExt;

pub const DAEMON_TOKEN_BYTES: usize = 32;

/// Small deterministic LRU-like cache for daemon read-only query paths.
///
/// Synchronization belongs to the daemon state that owns the cache.
#[derive(Debug)]
pub(crate) struct BoundedCache<K, V> {
    capacity: usize,
    entries: VecDeque<(K, V)>,
}

impl<K: PartialEq, V: Clone> BoundedCache<K, V> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::new(),
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<V> {
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == &key)
        {
            self.entries.remove(index);
        }
        self.entries.push_back((key, value));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRuntimePaths {
    pub directory: PathBuf,
    pub endpoint: PathBuf,
    pub token: PathBuf,
    pub lock: PathBuf,
    pub log: PathBuf,
}

impl DaemonRuntimePaths {
    pub fn for_project(project_id: &str, override_dir: Option<&Path>) -> Result<Self> {
        let directory = match override_dir {
            Some(path) => path.to_path_buf(),
            None => default_runtime_root()?.join(project_id),
        };
        Ok(Self {
            endpoint: directory.join("endpoint.json"),
            token: directory.join("token"),
            lock: directory.join("lock"),
            log: directory.join("daemon.log"),
            directory,
        })
    }

    pub fn prepare(&self) -> Result<()> {
        fs::create_dir_all(&self.directory).with_context(|| {
            format!(
                "failed to create daemon runtime directory {}",
                self.directory.display()
            )
        })?;
        restrict_directory(&self.directory)
    }
}

pub fn default_runtime_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ATHANOR_RUNTIME_DIR") {
        if path.is_empty() {
            bail!("ATHANOR_RUNTIME_DIR must not be empty");
        }
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            anyhow::anyhow!(
                "cannot determine local application data directory; set ATHANOR_RUNTIME_DIR"
            )
        })?;
        Ok(PathBuf::from(local_app_data)
            .join("Athanor")
            .join("runtime"))
    }

    #[cfg(unix)]
    {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
            anyhow::anyhow!("XDG_RUNTIME_DIR is required; set ATHANOR_RUNTIME_DIR explicitly")
        })?;
        Ok(PathBuf::from(runtime).join("athanor"))
    }

    #[cfg(not(any(unix, windows)))]
    {
        bail!("daemon runtime directories are unsupported on this platform")
    }
}

pub struct DaemonRuntimeLock {
    file: File,
}

impl DaemonRuntimeLock {
    pub fn acquire(path: &Path, project_id: &str) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("failed to open daemon lock {}", path.display()))?;
        file.try_lock_exclusive().with_context(|| {
            format!(
                "daemon lock is held at {}; another daemon is running",
                path.display()
            )
        })?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "project_id": project_id,
                "pid": std::process::id(),
                "athanor_version": env!("CARGO_PKG_VERSION"),
            })
        )?;
        file.flush()?;
        restrict_file(path)?;
        Ok(Self { file })
    }
}

impl Drop for DaemonRuntimeLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn create_daemon_token(path: &Path) -> Result<String> {
    let mut bytes = [0_u8; DAEMON_TOKEN_BYTES];
    getrandom::fill(&mut bytes).context("failed to generate daemon authentication token")?;
    let token = encode_hex(&bytes);
    write_secret(path, token.as_bytes())?;
    Ok(token)
}

pub fn read_daemon_token(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open daemon token {}", path.display()))?;
    let mut token = String::new();
    file.read_to_string(&mut token)
        .with_context(|| format!("failed to read daemon token {}", path.display()))?;
    let token = token.trim().to_string();
    if token.len() != DAEMON_TOKEN_BYTES * 2 || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("daemon token is invalid");
    }
    Ok(token)
}

pub fn constant_time_token_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut difference = left.len() ^ right.len();
    let length = left.len().max(right.len());
    for index in 0..length {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

pub struct RuntimeFileGuard {
    paths: Vec<PathBuf>,
}

impl RuntimeFileGuard {
    pub fn new(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            paths: paths.into_iter().collect(),
        }
    }
}

impl Drop for RuntimeFileGuard {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

fn write_secret(path: &Path, content: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create daemon secret {}", path.display()))?;
    file.write_all(content)?;
    file.write_all(b"\n")?;
    file.flush()?;
    restrict_file(path)
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to restrict {}", path.display()))
}

#[cfg(windows)]
fn restrict_directory(path: &Path) -> Result<()> {
    restrict_windows_acl(path, true)
}

#[cfg(not(any(unix, windows)))]
fn restrict_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn restrict_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to restrict {}", path.display()))
}

#[cfg(windows)]
fn restrict_file(path: &Path) -> Result<()> {
    restrict_windows_acl(path, false)
}

#[cfg(not(any(unix, windows)))]
fn restrict_file(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn restrict_windows_acl(path: &Path, inherit_children: bool) -> Result<()> {
    use std::process::{Command, Stdio};

    let user = Command::new("whoami")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|identity| identity.trim().to_string())
        .filter(|identity| !identity.is_empty())
        .or_else(|| std::env::var("USERNAME").ok())
        .context("cannot determine current Windows identity for daemon runtime permissions")?;
    let path = path.to_string_lossy().into_owned();
    let grant = if inherit_children {
        format!("{user}:(OI)(CI)(F)")
    } else {
        format!("{user}:(F)")
    };
    let status = Command::new("icacls")
        .args([&path, "/inheritance:r", "/grant:r", &grant])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to run icacls for {path}"))?;
    if !status.success() {
        bail!("icacls failed while restricting {path}");
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_reads_and_compares_tokens() {
        let root =
            std::env::temp_dir().join(format!("athanor-runtime-token-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("token");

        let token = create_daemon_token(&path).unwrap();

        assert_eq!(token.len(), DAEMON_TOKEN_BYTES * 2);
        assert_eq!(read_daemon_token(&path).unwrap(), token);
        assert!(constant_time_token_eq(&token, &token));
        assert!(!constant_time_token_eq(&token, "wrong"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_lock_can_be_reacquired_after_drop() {
        let root =
            std::env::temp_dir().join(format!("athanor-runtime-lock-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("lock");
        let lock = DaemonRuntimeLock::acquire(&path, "alpha").unwrap();
        assert!(DaemonRuntimeLock::acquire(&path, "alpha").is_err());
        drop(lock);
        DaemonRuntimeLock::acquire(&path, "alpha").unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_lock_replaces_stale_metadata_when_reacquired() {
        let root =
            std::env::temp_dir().join(format!("athanor-runtime-stale-lock-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("lock");
        fs::write(
            &path,
            serde_json::json!({
                "project_id": "old-project",
                "pid": 999999,
                "athanor_version": "0.0.0-stale"
            })
            .to_string(),
        )
        .unwrap();

        {
            let _lock = DaemonRuntimeLock::acquire(&path, "beta").unwrap();
        }

        let metadata: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(metadata["project_id"], "beta");
        assert_eq!(metadata["pid"], serde_json::json!(std::process::id()));
        assert_eq!(metadata["athanor_version"], env!("CARGO_PKG_VERSION"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_file_guard_removes_endpoint_and_token_files_on_drop() {
        let root =
            std::env::temp_dir().join(format!("athanor-runtime-guard-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let endpoint = root.join("endpoint.json");
        let token = root.join("token");
        fs::write(&endpoint, "{}\n").unwrap();
        fs::write(&token, "secret\n").unwrap();

        {
            let _guard = RuntimeFileGuard::new([endpoint.clone(), token.clone()]);
            assert!(endpoint.is_file());
            assert!(token.is_file());
        }

        assert!(!endpoint.exists());
        assert!(!token.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
