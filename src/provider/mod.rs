pub mod github;

use std::path::Path;

use anyhow::{Result, bail};

pub struct Release {
    pub tag: String,
    pub assets: Vec<Asset>,
}

pub struct Asset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

/// A source of releases. GitHub today; the same trait fits GitLab or any
/// other host that serves tagged releases with downloadable assets.
pub trait Provider {
    fn id(&self) -> &'static str;
    fn latest_release(&self, owner: &str, repo: &str) -> Result<Release>;
    /// Resolve a specific tag. Implementations may normalize near-miss tags
    /// (e.g. `1.2.3` vs `v1.2.3`); the returned Release carries the tag that
    /// actually exists.
    fn release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> Result<Release>;
    fn download(&self, asset: &Asset, dest: &Path) -> Result<()>;
}

pub fn get(id: &str) -> Result<Box<dyn Provider>> {
    match id {
        "github" => Ok(Box::new(github::Github::new())),
        other => bail!("unknown provider '{other}'"),
    }
}
