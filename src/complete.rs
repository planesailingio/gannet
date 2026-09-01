//! Candidate generation for dynamic shell completion.
//!
//! Everything here runs on a TAB press, so the rules differ from the rest of
//! the crate: never error, never print, and never build a `Dirs` (its eager
//! `create_dir_all` has no business running mid-keystroke). Anything that
//! goes wrong collapses to an empty candidate list.
//!
//! Repo and tag lookups hit the GitHub API through a short-timeout client and
//! are cached under `<root>/cache/completion/` with a 1-hour TTL. Expired
//! entries are refreshed; if the refresh fails the stale names are used.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use clap_complete::CompletionCandidate;
use serde::{Deserialize, Serialize};

use crate::provider::github::Github;
use crate::state::{State, now_unix};

pub const CACHE_TTL_SECS: u64 = 3600;
const MAX_REPO_PAGES: usize = 3;

/// What the user has typed so far of a `<owner>/<repo>[@tag]` spec.
#[derive(Debug, PartialEq, Eq)]
pub enum Partial<'a> {
    /// No `/` yet: "" or "shar".
    Owner(&'a str),
    /// Owner complete, repo underway: "sharkdp/" or "sharkdp/f".
    Repo { owner: &'a str, repo: &'a str },
    /// Package complete, tag underway: "sharkdp/fd@" or "sharkdp/fd@v9".
    Tag {
        owner: &'a str,
        repo: &'a str,
        tag: &'a str,
    },
    /// Nothing sensible can be completed from this.
    Invalid,
}

/// Classify a partial spec token. Mirrors `PackageSpec::from_str`: the tag is
/// whatever follows the first `@`, and the package half needs exactly one `/`.
pub fn parse_partial(s: &str) -> Partial<'_> {
    let (pkg, tag) = match s.split_once('@') {
        Some((pkg, tag)) => (pkg, Some(tag)),
        None => (s, None),
    };
    let mut parts = pkg.split('/');
    match (parts.next(), parts.next(), parts.next(), tag) {
        (Some(owner), Some(repo), None, Some(tag)) if !owner.is_empty() && !repo.is_empty() => {
            Partial::Tag { owner, repo, tag }
        }
        (Some(owner), Some(repo), None, None) if !owner.is_empty() => Partial::Repo { owner, repo },
        (Some(owner), None, None, None) => Partial::Owner(owner),
        _ => Partial::Invalid,
    }
}

/// True when `fetched_at` is within `ttl` seconds of `now` (future stamps
/// count as fresh rather than panicking or wrapping).
pub fn is_fresh(fetched_at: u64, now: u64, ttl: u64) -> bool {
    now.saturating_sub(fetched_at) <= ttl
}

/// A string that is safe to use as a cache filename and a URL path segment.
/// GitHub owner/repo names fit this; anything else gets no candidates.
pub fn safe_segment(s: &str) -> bool {
    if s.is_empty() || s == "." || s == ".." {
        return false;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return false;
    }
    // Windows reserved device names, with or without an extension.
    let stem = s.split('.').next().unwrap_or(s).to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
}

/// Best-effort recovery of `--gannet-dir` from the raw completion argv
/// (the shell passes the full command words to the completer process).
pub fn gannet_dir_from_args(args: &[OsString]) -> Option<PathBuf> {
    let mut found = None;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--gannet-dir" {
            if let Some(value) = iter.peek() {
                found = Some(PathBuf::from(value));
            }
        } else if let Some(s) = arg.to_str()
            && let Some(value) = s.strip_prefix("--gannet-dir=")
        {
            found = Some(PathBuf::from(value));
        }
    }
    found
}

/// The gannet root for completion: `--gannet-dir` from argv, else
/// `GANNET_DIR`, else `~/.gannet`. Read-only — nothing is created.
fn resolve_root() -> Option<PathBuf> {
    let args: Vec<OsString> = std::env::args_os().collect();
    if let Some(dir) = gannet_dir_from_args(&args) {
        return Some(dir);
    }
    if let Some(dir) = std::env::var_os("GANNET_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    dirs::home_dir().map(|h| h.join(".gannet"))
}

fn cache_dir(root: &Path) -> PathBuf {
    root.join("cache").join("completion")
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    fetched_at: u64,
    names: Vec<String>,
}

fn read_cache(path: &Path) -> Option<CacheFile> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Atomic, error-swallowing cache write. `NamedTempFile` (unique name per
/// call) keeps concurrent TAB presses from clobbering each other mid-write.
fn write_cache(path: &Path, cache: &CacheFile) {
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(text) = serde_json::to_string(cache) else {
        return;
    };
    if let Ok(tmp) = tempfile::NamedTempFile::new_in(parent)
        && fs::write(tmp.path(), text).is_ok()
    {
        let _ = tmp.persist(path);
    }
}

/// Serve `path` from cache when fresh; otherwise fetch, caching on success
/// and falling back to the stale names on failure.
fn cached_or_fetch(
    path: &Path,
    now: u64,
    fetch: impl FnOnce() -> anyhow::Result<Vec<String>>,
) -> Vec<String> {
    let cached = read_cache(path);
    if let Some(c) = &cached
        && is_fresh(c.fetched_at, now, CACHE_TTL_SECS)
    {
        return c.names.clone();
    }
    match fetch() {
        Ok(names) => {
            write_cache(
                path,
                &CacheFile {
                    fetched_at: now,
                    names: names.clone(),
                },
            );
            names
        }
        Err(_) => cached.map(|c| c.names).unwrap_or_default(),
    }
}

/// Repo names for an owner, via the cache. `fetch` is injected so tests
/// never touch the network.
pub fn repos_for_owner(
    root: &Path,
    owner: &str,
    now: u64,
    fetch: impl FnOnce() -> anyhow::Result<Vec<String>>,
) -> Vec<String> {
    if !safe_segment(owner) {
        return Vec::new();
    }
    let path = cache_dir(root).join("repos").join(format!("{owner}.json"));
    cached_or_fetch(&path, now, fetch)
}

/// Release tags for a repo, via the cache.
pub fn tags_for_repo(
    root: &Path,
    owner: &str,
    repo: &str,
    now: u64,
    fetch: impl FnOnce() -> anyhow::Result<Vec<String>>,
) -> Vec<String> {
    if !safe_segment(owner) || !safe_segment(repo) {
        return Vec::new();
    }
    let path = cache_dir(root)
        .join("tags")
        .join(format!("{owner}--{repo}.json"));
    cached_or_fetch(&path, now, fetch)
}

/// Installed `owner/repo` keys from state.json; empty on any problem.
pub fn installed_keys(root: &Path) -> Vec<String> {
    State::load(&root.join("state.json"))
        .map(|s| s.packages.into_keys().collect())
        .unwrap_or_default()
}

/// Owners worth suggesting before a `/` is typed: owners of installed
/// packages plus owners already in the repo cache, rendered as `owner/`.
pub fn owner_candidates(root: &Path) -> Vec<String> {
    let mut owners = BTreeSet::new();
    for key in installed_keys(root) {
        if let Some((owner, _)) = key.split_once('/') {
            owners.insert(owner.to_string());
        }
    }
    if let Ok(entries) = fs::read_dir(cache_dir(root).join("repos")) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if let Some(owner) = name.to_str().and_then(|n| n.strip_suffix(".json")) {
                owners.insert(owner.to_string());
            }
        }
    }
    owners.into_iter().map(|o| format!("{o}/")).collect()
}

/// Turn (value, help) pairs into candidates, preserving the given order.
fn to_candidates<'a>(
    values: impl IntoIterator<Item = (String, Option<&'a str>)>,
) -> Vec<CompletionCandidate> {
    values
        .into_iter()
        .enumerate()
        .map(|(i, (value, help))| {
            CompletionCandidate::new(value)
                .help(help.map(|h| h.to_string().into()))
                .display_order(Some(i))
        })
        .collect()
}

/// Completer for `gannet install <spec>`: owners, then an owner's repos
/// (installed ones first), then release tags after `@`.
pub fn complete_install_spec(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(cur) = current.to_str() else {
        return Vec::new();
    };
    let Some(root) = resolve_root() else {
        return Vec::new();
    };
    let now = now_unix();
    match parse_partial(cur) {
        Partial::Owner(prefix) => to_candidates(
            owner_candidates(&root)
                .into_iter()
                .filter(|o| o.starts_with(prefix))
                .map(|o| (o, None)),
        ),
        Partial::Repo { owner, .. } => {
            let installed: BTreeSet<String> = installed_keys(&root)
                .into_iter()
                .filter_map(|k| {
                    let (o, r) = k.split_once('/')?;
                    (o == owner).then(|| r.to_string())
                })
                .collect();
            let gh = Github::for_completion();
            let remote =
                repos_for_owner(&root, owner, now, || gh.list_repos(owner, MAX_REPO_PAGES));
            let installed_first = installed
                .iter()
                .map(|r| (r.clone(), Some("installed")))
                .chain(
                    remote
                        .into_iter()
                        .filter(|r| !installed.contains(r))
                        .map(|r| (r, None)),
                );
            to_candidates(
                installed_first
                    .map(|(r, help)| (format!("{owner}/{r}"), help))
                    .filter(|(v, _)| v.starts_with(cur)),
            )
        }
        Partial::Tag { owner, repo, .. } => {
            let gh = Github::for_completion();
            let tags = tags_for_repo(&root, owner, repo, now, || {
                gh.list_release_tags(owner, repo)
            });
            to_candidates(
                tags.into_iter()
                    .map(|t| (format!("{owner}/{repo}@{t}"), None))
                    .filter(|(v, _)| v.starts_with(cur)),
            )
        }
        Partial::Invalid => Vec::new(),
    }
}

/// Completer for `gannet use <spec>`: installed packages before the `@`,
/// then on-disk versions first and remote release tags after it.
pub fn complete_use_spec(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(cur) = current.to_str() else {
        return Vec::new();
    };
    let Some(root) = resolve_root() else {
        return Vec::new();
    };
    match parse_partial(cur) {
        Partial::Owner(_) | Partial::Repo { .. } => to_candidates(
            installed_keys(&root)
                .into_iter()
                .filter(|k| k.starts_with(cur))
                .map(|k| (k, None)),
        ),
        Partial::Tag { owner, repo, .. } => to_candidates(
            spec_tags(&root, owner, repo)
                .into_iter()
                .map(|(t, help)| (format!("{owner}/{repo}@{t}"), help))
                .filter(|(v, _)| v.starts_with(cur)),
        ),
        Partial::Invalid => Vec::new(),
    }
}

/// Completer for the optional second `<tag>` argument of `gannet use`:
/// digs the `owner/repo` out of the words already on the command line.
pub fn complete_use_tag(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(cur) = current.to_str() else {
        return Vec::new();
    };
    let Some(root) = resolve_root() else {
        return Vec::new();
    };
    let args: Vec<OsString> = std::env::args_os().collect();
    let Some((owner, repo)) = args.iter().find_map(|a| {
        let s = a.to_str()?;
        match parse_partial(s) {
            Partial::Repo { owner, repo } if !repo.is_empty() => {
                Some((owner.to_string(), repo.to_string()))
            }
            _ => None,
        }
    }) else {
        return Vec::new();
    };
    to_candidates(
        spec_tags(&root, &owner, &repo)
            .into_iter()
            .filter(|(t, _)| t.starts_with(cur)),
    )
}

/// Tags to offer for one package: versions already on disk first, then
/// remote release tags (cached).
fn spec_tags<'a>(root: &Path, owner: &str, repo: &str) -> Vec<(String, Option<&'a str>)> {
    let key = format!("{owner}/{repo}");
    let on_disk: Vec<String> = State::load(&root.join("state.json"))
        .ok()
        .and_then(|s| s.packages.get(&key).cloned())
        .map(|p| p.versions.into_iter().map(|v| v.tag).collect())
        .unwrap_or_default();
    let gh = Github::for_completion();
    let remote = tags_for_repo(root, owner, repo, now_unix(), || {
        gh.list_release_tags(owner, repo)
    });
    on_disk
        .iter()
        .map(|t| (t.clone(), Some("on disk")))
        .chain(
            remote
                .into_iter()
                .filter(|t| !on_disk.contains(t))
                .map(|t| (t, None)),
        )
        .collect()
}

/// Candidates for args taking an installed package (uninstall, rollback,
/// upgrade, list). The completion engine prefix-filters these itself.
pub fn installed_spec_candidates() -> Vec<CompletionCandidate> {
    let Some(root) = resolve_root() else {
        return Vec::new();
    };
    to_candidates(installed_keys(&root).into_iter().map(|k| (k, None)))
}
