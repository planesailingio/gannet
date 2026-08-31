use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// The on-disk layout under the gannet root (default `~/.gannet`).
pub struct Dirs {
    pub root: PathBuf,
    pub bin: PathBuf,
    pub packages: PathBuf,
    pub tmp: PathBuf,
}

impl Dirs {
    /// Resolve the root from, in order: the --gannet-dir flag, the
    /// GANNET_DIR env var, then ~/.gannet. Creates the directory tree.
    pub fn new(override_root: Option<PathBuf>) -> Result<Self> {
        let root = match override_root {
            Some(p) => p,
            None => match std::env::var_os("GANNET_DIR") {
                Some(p) if !p.is_empty() => PathBuf::from(p),
                _ => dirs::home_dir()
                    .context("could not determine your home directory")?
                    .join(".gannet"),
            },
        };
        let dirs = Dirs {
            bin: root.join("bin"),
            packages: root.join("packages"),
            tmp: root.join("tmp"),
            root,
        };
        for d in [&dirs.root, &dirs.bin, &dirs.packages, &dirs.tmp] {
            fs::create_dir_all(d)
                .with_context(|| format!("could not create directory {}", d.display()))?;
        }
        Ok(dirs)
    }

    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.json")
    }

    /// packages/<owner>/<repo>
    pub fn package_dir(&self, owner: &str, repo: &str) -> PathBuf {
        self.packages.join(owner).join(repo)
    }

    /// packages/<owner>/<repo>/<tag>
    pub fn version_dir(&self, owner: &str, repo: &str, tag: &str) -> PathBuf {
        self.package_dir(owner, repo).join(tag)
    }
}
