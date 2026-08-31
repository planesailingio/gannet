use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// How many versions of a package are kept on disk (current + previous).
pub const KEEP_VERSIONS: usize = 2;

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub packages: BTreeMap<String, Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub provider: String,
    /// Name of the installed command (link in bin/ and file in the version dir).
    pub bin_name: String,
    pub link_mode: LinkMode,
    pub current: String,
    pub pinned: bool,
    /// Versions on disk, newest first. Invariants: contains `current`,
    /// length <= KEEP_VERSIONS.
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub tag: String,
    /// Unix timestamp (seconds).
    pub installed_at: u64,
    /// The release asset this version came from.
    pub asset: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkMode {
    Symlink,
    Copy,
}

impl Default for State {
    fn default() -> Self {
        State {
            schema_version: 1,
            packages: BTreeMap::new(),
        }
    }
}

/// Two tags refer to the same version if they only differ by a leading 'v'.
pub fn tags_match(a: &str, b: &str) -> bool {
    a.trim_start_matches('v') == b.trim_start_matches('v')
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl State {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).with_context(|| {
                format!(
                    "state file {} is corrupt; fix or remove it (installed packages are under the packages/ directory next to it)",
                    path.display()
                )
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(e).with_context(|| format!("could not read {}", path.display())),
        }
    }

    /// Atomic save: write a temp file next to the target, then rename over it.
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, text).with_context(|| format!("could not write {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("could not update {}", path.display()))?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&Package> {
        self.packages.get(key)
    }

    /// Record a freshly downloaded version as current. Returns the tags whose
    /// version directories should be deleted to honour the retention policy.
    #[allow(clippy::too_many_arguments)]
    pub fn record_install(
        &mut self,
        key: &str,
        provider: &str,
        bin_name: &str,
        link_mode: LinkMode,
        tag: &str,
        asset: &str,
        pinned: bool,
        now: u64,
    ) -> Vec<String> {
        let pkg = self.packages.entry(key.to_string()).or_insert(Package {
            provider: provider.to_string(),
            bin_name: bin_name.to_string(),
            link_mode,
            current: tag.to_string(),
            pinned,
            versions: Vec::new(),
        });
        pkg.bin_name = bin_name.to_string();
        pkg.link_mode = link_mode;
        pkg.current = tag.to_string();
        pkg.pinned = pinned;
        pkg.versions.retain(|v| v.tag != tag);
        pkg.versions.insert(
            0,
            VersionEntry {
                tag: tag.to_string(),
                installed_at: now,
                asset: asset.to_string(),
            },
        );
        pkg.versions
            .split_off(KEEP_VERSIONS.min(pkg.versions.len()))
            .into_iter()
            .map(|v| v.tag)
            .collect()
    }

    /// Make an already-on-disk version current (use/rollback).
    pub fn switch_current(&mut self, key: &str, tag: &str, pinned: bool) -> Result<()> {
        let pkg = self
            .packages
            .get_mut(key)
            .with_context(|| format!("{key} is not installed"))?;
        if !pkg.versions.iter().any(|v| v.tag == tag) {
            bail!("version {tag} of {key} is not on disk");
        }
        pkg.current = tag.to_string();
        pkg.pinned = pinned;
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Option<Package> {
        self.packages.remove(key)
    }
}

impl Package {
    /// The on-disk version that is not current, if any.
    pub fn previous(&self) -> Option<&VersionEntry> {
        self.versions.iter().find(|v| v.tag != self.current)
    }

    /// Find an on-disk version by tag, tolerating a leading-'v' mismatch.
    pub fn version_on_disk(&self, tag: &str) -> Option<&VersionEntry> {
        self.versions.iter().find(|v| tags_match(&v.tag, tag))
    }

    /// Drop a version from the on-disk list (self-heal when its directory
    /// has gone missing).
    pub fn forget_version(&mut self, tag: &str) {
        self.versions.retain(|v| v.tag != tag);
    }
}

/// Format a unix timestamp as a UTC calendar date (YYYY-MM-DD).
pub fn format_date(unix_secs: u64) -> String {
    // Howard Hinnant's civil_from_days algorithm.
    let days = (unix_secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
