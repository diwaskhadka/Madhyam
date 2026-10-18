//! Where Madhyam keeps its files: $XDG_DATA_HOME/Madhyam, normally ~/.local/share/Madhyam.
//! The folder name is fixed (like %LOCALAPPDATA%\Madhyam on Windows): renaming it would lose
//! everyone's playlists and settings.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn data_dir() -> &'static Path {
    DATA_DIR.get_or_init(|| {
        let base = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local").join("share"));
        base.join("Madhyam")
    })
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Create the data folder, readable by this user only (it holds the catalog and secrets).
pub fn ensure_data_dir() {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
}

/// Write to a temporary file, then rename it over the target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// A fresh file name in the system temp folder.
pub fn temp_file(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("madhyam-{name}-{}-{n}.tmp", std::process::id()))
}
