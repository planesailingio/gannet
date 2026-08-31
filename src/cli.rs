use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, Subcommand};

/// A package reference: `owner/repo` with an optional `@tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSpec {
    pub owner: String,
    pub repo: String,
    pub tag: Option<String>,
}

impl PackageSpec {
    /// The state key, e.g. "sharkdp/fd".
    pub fn key(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

impl fmt::Display for PackageSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)?;
        if let Some(tag) = &self.tag {
            write!(f, "@{tag}")?;
        }
        Ok(())
    }
}

impl FromStr for PackageSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (pkg, tag) = match s.split_once('@') {
            Some((pkg, "")) => return Err(format!("empty tag in '{pkg}@'")),
            Some((pkg, tag)) => (pkg, Some(tag.to_string())),
            None => (s, None),
        };
        let mut parts = pkg.split('/');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(owner), Some(repo), None) if !owner.is_empty() && !repo.is_empty() => {
                Ok(PackageSpec {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    tag,
                })
            }
            _ => Err(format!(
                "expected '<owner>/<repo>' or '<owner>/<repo>@<tag>', got '{s}'"
            )),
        }
    }
}

#[derive(Parser)]
#[command(name = "gannet", version, about = "A lightweight package manager for GitHub release binaries", long_about = None)]
pub struct Cli {
    /// Print extra detail (asset scoring, API calls)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Root directory (default: ~/.gannet; also via GANNET_DIR)
    #[arg(long, global = true, value_name = "PATH")]
    pub gannet_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install a package from its latest (or a pinned) GitHub release
    Install {
        /// Package as <owner>/<repo> or <owner>/<repo>@<tag>
        spec: PackageSpec,
        /// Binary to pick when the archive contains several executables
        #[arg(long, value_name = "NAME")]
        bin: Option<String>,
        /// Skip asset auto-detection; pick the asset whose name contains this substring
        #[arg(long, value_name = "SUBSTRING")]
        asset: Option<String>,
        /// Name for the installed command (default: the binary's own name)
        #[arg(long = "as", value_name = "NAME")]
        link_as: Option<String>,
        /// Reinstall even if this version is already current
        #[arg(long)]
        force: bool,
    },
    /// Remove a package and all of its installed versions
    Uninstall {
        /// Package as <owner>/<repo>
        spec: PackageSpec,
    },
    /// List installed packages (or show details for one)
    List {
        /// Package as <owner>/<repo>
        spec: Option<PackageSpec>,
    },
    /// Upgrade a package (or everything) to the latest release
    Upgrade {
        /// Package as <owner>/<repo>
        spec: Option<PackageSpec>,
        /// Upgrade every installed package
        #[arg(long, conflicts_with = "spec")]
        all: bool,
    },
    /// Switch back to the previously used version
    Rollback {
        /// Package as <owner>/<repo>
        spec: PackageSpec,
    },
    /// Switch to a specific version, downloading it if needed
    #[command(name = "use")]
    Use {
        /// Package as <owner>/<repo>@<tag> (or pass the tag as a second argument)
        spec: PackageSpec,
        /// Version tag (alternative to the @<tag> form)
        tag: Option<String>,
    },
}
