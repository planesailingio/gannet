use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::state::LinkMode;

/// The name of the command in bin/, with the platform's executable suffix.
pub fn link_file_name(bin_name: &str) -> String {
    if cfg!(windows) {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    }
}

/// Point `bin_dir/<bin_name>` at `target`, replacing whatever is there.
/// Returns the mode that was actually used (Windows may fall back to a copy
/// when symlinks need Developer Mode).
pub fn link(bin_dir: &Path, bin_name: &str, target: &Path) -> Result<LinkMode> {
    let dest = bin_dir.join(link_file_name(bin_name));
    platform_link(&dest, target).with_context(|| {
        format!(
            "could not link {} into {}",
            target.display(),
            bin_dir.display()
        )
    })
}

/// Remove the link (or copied binary) for `bin_name`, if present.
pub fn unlink(bin_dir: &Path, bin_name: &str) -> Result<()> {
    let dest = bin_dir.join(link_file_name(bin_name));
    match fs::remove_file(&dest) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("could not remove {}", dest.display())),
    }
}

/// Where the link currently points, if it is a symlink.
pub fn read_target(bin_dir: &Path, bin_name: &str) -> Option<std::path::PathBuf> {
    fs::read_link(bin_dir.join(link_file_name(bin_name))).ok()
}

#[cfg(unix)]
fn platform_link(dest: &Path, target: &Path) -> Result<LinkMode> {
    use std::os::unix::fs::symlink;
    // Atomic replace: symlink to a temp name, then rename over the real one.
    let tmp = dest.with_file_name(format!(
        ".{}.gannet-tmp",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("link")
    ));
    let _ = fs::remove_file(&tmp);
    symlink(target, &tmp)?;
    fs::rename(&tmp, dest)?;
    Ok(LinkMode::Symlink)
}

#[cfg(windows)]
fn platform_link(dest: &Path, target: &Path) -> Result<LinkMode> {
    use std::os::windows::fs::symlink_file;
    // A stale copy (or old symlink) may be in the way; a running exe can't
    // be deleted but can be renamed aside.
    if dest.exists() || fs::symlink_metadata(dest).is_ok() {
        let old = dest.with_extension("exe.old");
        let _ = fs::remove_file(&old);
        fs::rename(dest, &old).or_else(|_| fs::remove_file(dest))?;
    }
    match symlink_file(target, dest) {
        Ok(()) => Ok(LinkMode::Symlink),
        // 1314: ERROR_PRIVILEGE_NOT_HELD — no Developer Mode; copy instead.
        Err(e) if e.raw_os_error() == Some(1314) => {
            fs::copy(target, dest)?;
            Ok(LinkMode::Copy)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_link(_dest: &Path, _target: &Path) -> Result<LinkMode> {
    anyhow::bail!("unsupported platform");
}

/// Best-effort cleanup of `.exe.old` files left by in-use replacements on
/// Windows. No-op elsewhere.
pub fn sweep_old(bin_dir: &Path) {
    if !cfg!(windows) {
        return;
    }
    if let Ok(entries) = fs::read_dir(bin_dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(".exe.old") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}
