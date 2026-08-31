use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;

use crate::platform::Os;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    TarGz,
    Zip,
    Gz,
    Bare,
}

/// Decide how to unpack an asset: by extension first, then magic bytes for
/// extensionless downloads.
pub fn detect_format(asset_name: &str, path: &Path) -> Result<Format> {
    let lower = asset_name.to_lowercase();
    if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        return Ok(Format::TarGz);
    }
    if lower.ends_with(".zip") {
        return Ok(Format::Zip);
    }
    if lower.ends_with(".gz") {
        return Ok(Format::Gz);
    }
    let mut magic = [0u8; 4];
    let n = File::open(path)?.read(&mut magic).unwrap_or(0);
    if n >= 2 && magic[..2] == [0x1f, 0x8b] {
        // Gzip with no telling extension: could be a tarball or a bare
        // gzipped binary. Peek at the decompressed stream for a tar header.
        let mut decoder = GzDecoder::new(File::open(path)?);
        let mut head = vec![0u8; 262];
        let read = decoder.read(&mut head).unwrap_or(0);
        if read >= 262 && &head[257..262] == b"ustar" {
            return Ok(Format::TarGz);
        }
        return Ok(Format::Gz);
    }
    if n >= 4 && magic == [0x50, 0x4b, 0x03, 0x04] {
        return Ok(Format::Zip);
    }
    Ok(Format::Bare)
}

/// Unpack `archive` into `staging`. All entry paths are confined to the
/// staging directory; archives that try to escape it are rejected.
pub fn extract(archive: &Path, format: Format, staging: &Path, asset_name: &str) -> Result<()> {
    match format {
        Format::TarGz => {
            let tar = GzDecoder::new(File::open(archive)?);
            let mut ar = tar::Archive::new(tar);
            for entry in ar.entries().context("could not read the tar archive")? {
                let mut entry = entry.context("corrupt tar entry")?;
                let kind = entry.header().entry_type();
                if !(kind.is_file() || kind.is_dir()) {
                    // Skip symlinks and special files; release archives
                    // don't rely on them for the binary itself.
                    continue;
                }
                // unpack_in refuses paths that would escape the directory.
                entry
                    .unpack_in(staging)
                    .context("could not unpack a tar entry")?;
            }
        }
        Format::Zip => {
            let mut zip = zip::ZipArchive::new(File::open(archive)?)
                .context("could not read the zip archive")?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).context("corrupt zip entry")?;
                let Some(rel) = entry.enclosed_name() else {
                    bail!(
                        "zip entry '{}' has an unsafe path; refusing to extract",
                        entry.name()
                    );
                };
                let dest = staging.join(rel);
                if entry.is_dir() {
                    fs::create_dir_all(&dest)?;
                    continue;
                }
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = File::create(&dest)
                    .with_context(|| format!("could not create {}", dest.display()))?;
                std::io::copy(&mut entry, &mut out)?;
                #[cfg(unix)]
                if let Some(mode) = entry.unix_mode() {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&dest, fs::Permissions::from_mode(mode))?;
                }
            }
        }
        Format::Gz => {
            let stem = asset_name
                .strip_suffix(".gz")
                .unwrap_or(asset_name)
                .rsplit('/')
                .next()
                .unwrap_or(asset_name);
            let mut decoder = GzDecoder::new(File::open(archive)?);
            let mut out = File::create(staging.join(stem))?;
            std::io::copy(&mut decoder, &mut out).context("could not decompress the .gz file")?;
        }
        Format::Bare => {
            fs::copy(archive, staging.join(asset_name))?;
        }
    }
    Ok(())
}

/// Names that are clearly documentation or support material, not the binary.
const DOC_PREFIXES: &[&str] = &[
    "readme",
    "license",
    "licence",
    "changelog",
    "notice",
    "copying",
    "authors",
    "contributing",
    "code_of_conduct",
];
const DOC_EXTENSIONS: &[&str] = &[
    ".md", ".txt", ".html", ".pdf", ".1", ".5", ".8", ".man", ".json", ".yml", ".yaml", ".toml",
    ".fish", ".bash", ".zsh", ".ps1", ".sh", ".bat", ".nu", ".elv", ".sig", ".sha256", ".d",
    ".rst", ".adoc",
];
const DOC_DIRS: &[&str] = &[
    "completions",
    "completion",
    "complete",
    "autocomplete",
    "man",
    "doc",
    "docs",
    "examples",
];

fn is_doc(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if DOC_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return true;
    }
    if DOC_EXTENSIONS.iter().any(|e| name.ends_with(e)) {
        return true;
    }
    path.iter().any(|part| {
        let part = part.to_str().unwrap_or("").to_lowercase();
        DOC_DIRS.contains(&part.as_str())
    })
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    if depth > 3 {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            walk(&path, depth + 1, out)?;
        } else if file_type.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    false
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn listing(candidates: &[PathBuf], root: &Path) -> String {
    candidates
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// Find the binary inside the extracted tree.
pub fn discover(staging: &Path, os: Os, repo: &str, bin_override: Option<&str>) -> Result<PathBuf> {
    let mut files = Vec::new();
    walk(staging, 0, &mut files).context("could not scan the extracted files")?;
    let candidates: Vec<PathBuf> = files.into_iter().filter(|p| !is_doc(p)).collect();
    if candidates.is_empty() {
        bail!("no binary found in the downloaded asset (only documentation and support files)");
    }

    // Explicit pick beats everything.
    if let Some(want) = bin_override {
        let want = want.to_lowercase();
        let matched: Vec<&PathBuf> = candidates
            .iter()
            .filter(|p| stem_of(p) == want || file_name_of(p) == want)
            .collect();
        return match matched.as_slice() {
            [] => bail!(
                "--bin '{want}' does not match any file in the archive; found:\n  {}",
                listing(&candidates, staging)
            ),
            [one, ..] => Ok((*one).clone()),
        };
    }

    let repo_lower = repo.to_lowercase();
    if let Some(named) = candidates
        .iter()
        .find(|p| stem_of(p) == repo_lower || file_name_of(p) == repo_lower)
    {
        return Ok(named.clone());
    }

    let executables: Vec<&PathBuf> = match os {
        Os::Windows => candidates
            .iter()
            .filter(|p| file_name_of(p).ends_with(".exe"))
            .collect(),
        _ => candidates.iter().filter(|p| is_executable(p)).collect(),
    };
    match executables.as_slice() {
        [one] => return Ok((*one).clone()),
        [] => {}
        many => bail!(
            "the archive contains several executables; pick one with --bin <name>:\n  {}",
            listing(
                &many.iter().map(|p| (*p).clone()).collect::<Vec<_>>(),
                staging
            )
        ),
    }

    // No exec bits at all (bare download, gunzipped file, zip built on
    // Windows): a single candidate is unambiguous, otherwise take the
    // largest file.
    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    candidates
        .iter()
        .max_by_key(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .cloned()
        .context("no binary found in the downloaded asset")
}

/// Ensure the committed binary is runnable (bare downloads and zips built on
/// Windows carry no exec bit).
pub fn ensure_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)?.permissions().mode();
        if mode & 0o111 == 0 {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("could not chmod {}", path.display()))?;
        }
    }
    let _ = path;
    Ok(())
}
