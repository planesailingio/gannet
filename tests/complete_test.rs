use std::ffi::OsString;
use std::fs;
use std::path::Path;

use gannet::complete::{
    CACHE_TTL_SECS, Partial, gannet_dir_from_args, installed_keys, is_fresh, owner_candidates,
    parse_partial, repos_for_owner, safe_segment, tags_for_repo,
};
use gannet::state::{LinkMode, State};

fn fixture_root(dir: &Path) -> anyhow::Result<()> {
    let mut state = State::default();
    state.record_install(
        "sharkdp/fd",
        "github",
        "fd",
        LinkMode::Symlink,
        "v10.2.0",
        "fd.tar.gz",
        false,
        100,
    );
    state.record_install(
        "sharkdp/fd",
        "github",
        "fd",
        LinkMode::Symlink,
        "v10.5.0",
        "fd.tar.gz",
        false,
        200,
    );
    state.record_install(
        "BurntSushi/ripgrep",
        "github",
        "rg",
        LinkMode::Symlink,
        "15.2.0",
        "rg.tar.gz",
        false,
        300,
    );
    state.save(&dir.join("state.json"))
}

fn seed_repo_cache(root: &Path, owner: &str, fetched_at: u64, names: &[&str]) {
    let dir = root.join("cache").join("completion").join("repos");
    fs::create_dir_all(&dir).unwrap();
    let names: Vec<&str> = names.to_vec();
    let json = serde_json::json!({ "fetched_at": fetched_at, "names": names });
    fs::write(dir.join(format!("{owner}.json")), json.to_string()).unwrap();
}

#[test]
fn parse_partial_classifies_tokens() {
    assert_eq!(parse_partial(""), Partial::Owner(""));
    assert_eq!(parse_partial("shar"), Partial::Owner("shar"));
    assert_eq!(
        parse_partial("sharkdp/"),
        Partial::Repo {
            owner: "sharkdp",
            repo: ""
        }
    );
    assert_eq!(
        parse_partial("sharkdp/f"),
        Partial::Repo {
            owner: "sharkdp",
            repo: "f"
        }
    );
    assert_eq!(
        parse_partial("sharkdp/fd@"),
        Partial::Tag {
            owner: "sharkdp",
            repo: "fd",
            tag: ""
        }
    );
    assert_eq!(
        parse_partial("sharkdp/fd@v9"),
        Partial::Tag {
            owner: "sharkdp",
            repo: "fd",
            tag: "v9"
        }
    );
    // First-@ split, matching PackageSpec::from_str.
    assert_eq!(
        parse_partial("sharkdp/fd@v9@x"),
        Partial::Tag {
            owner: "sharkdp",
            repo: "fd",
            tag: "v9@x"
        }
    );
    assert_eq!(parse_partial("a/b/c"), Partial::Invalid);
    assert_eq!(parse_partial("foo@1"), Partial::Invalid);
    assert_eq!(parse_partial("@x"), Partial::Invalid);
    assert_eq!(parse_partial("/fd"), Partial::Invalid);
}

#[test]
fn is_fresh_boundaries() {
    assert!(is_fresh(1000, 1000 + CACHE_TTL_SECS, CACHE_TTL_SECS));
    assert!(!is_fresh(1000, 1001 + CACHE_TTL_SECS, CACHE_TTL_SECS));
    // A future stamp saturates instead of wrapping.
    assert!(is_fresh(2000, 1000, CACHE_TTL_SECS));
    assert!(is_fresh(0, 0, 0));
}

#[test]
fn safe_segment_accepts_and_rejects() {
    for ok in ["sharkdp", "fd.rs", "my_repo-2", "a", "conserve"] {
        assert!(safe_segment(ok), "{ok} should be safe");
    }
    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "a b",
        "CON",
        "con.json",
        "com3",
        "LPT9",
        "ünïcode",
    ] {
        assert!(!safe_segment(bad), "{bad} should be rejected");
    }
}

#[test]
fn gannet_dir_from_args_forms() {
    let args = |v: &[&str]| -> Vec<OsString> { v.iter().map(OsString::from).collect() };
    assert_eq!(
        gannet_dir_from_args(&args(&["gannet", "--gannet-dir", "/x", "install"])),
        Some("/x".into())
    );
    assert_eq!(
        gannet_dir_from_args(&args(&["gannet", "--gannet-dir=/y"])),
        Some("/y".into())
    );
    assert_eq!(gannet_dir_from_args(&args(&["gannet", "install"])), None);
    // Flag at the end with no value.
    assert_eq!(
        gannet_dir_from_args(&args(&["gannet", "--gannet-dir"])),
        None
    );
}

#[test]
fn installed_keys_reads_state() {
    let dir = tempfile::tempdir().unwrap();
    fixture_root(dir.path()).unwrap();
    assert_eq!(
        installed_keys(dir.path()),
        vec!["BurntSushi/ripgrep".to_string(), "sharkdp/fd".to_string()]
    );
}

#[test]
fn installed_keys_soft_fails() {
    let dir = tempfile::tempdir().unwrap();
    assert!(installed_keys(dir.path()).is_empty());
    fs::write(dir.path().join("state.json"), "not json").unwrap();
    assert!(installed_keys(dir.path()).is_empty());
}

#[test]
fn owner_candidates_merges_and_dedups() {
    let dir = tempfile::tempdir().unwrap();
    fixture_root(dir.path()).unwrap();
    seed_repo_cache(dir.path(), "sharkdp", 0, &["fd"]);
    seed_repo_cache(dir.path(), "junegunn", 0, &["fzf"]);
    assert_eq!(
        owner_candidates(dir.path()),
        vec![
            "BurntSushi/".to_string(),
            "junegunn/".to_string(),
            "sharkdp/".to_string()
        ]
    );
}

#[test]
fn cache_fresh_hit_skips_fetch() {
    let dir = tempfile::tempdir().unwrap();
    seed_repo_cache(dir.path(), "sharkdp", 5000, &["fd", "bat"]);
    let names = repos_for_owner(dir.path(), "sharkdp", 5000 + CACHE_TTL_SECS, || {
        panic!("fetch must not run on a fresh cache")
    });
    assert_eq!(names, vec!["fd", "bat"]);
}

#[test]
fn cache_expired_refetches_and_rewrites() {
    let dir = tempfile::tempdir().unwrap();
    seed_repo_cache(dir.path(), "sharkdp", 0, &["old"]);
    let now = CACHE_TTL_SECS + 1;
    let names = repos_for_owner(dir.path(), "sharkdp", now, || Ok(vec!["new".to_string()]));
    assert_eq!(names, vec!["new"]);
    // The rewritten file is fresh again at `now`.
    let again = repos_for_owner(dir.path(), "sharkdp", now, || {
        panic!("fetch must not run after the rewrite")
    });
    assert_eq!(again, vec!["new"]);
}

#[test]
fn cache_expired_with_failing_fetch_serves_stale() {
    let dir = tempfile::tempdir().unwrap();
    seed_repo_cache(dir.path(), "sharkdp", 0, &["stale"]);
    let names = repos_for_owner(dir.path(), "sharkdp", CACHE_TTL_SECS + 1, || {
        anyhow::bail!("network down")
    });
    assert_eq!(names, vec!["stale"]);
}

#[test]
fn cache_corrupt_is_treated_as_absent() {
    let dir = tempfile::tempdir().unwrap();
    let repos = dir.path().join("cache").join("completion").join("repos");
    fs::create_dir_all(&repos).unwrap();
    fs::write(repos.join("sharkdp.json"), "{{{").unwrap();
    let names = repos_for_owner(dir.path(), "sharkdp", 1000, || Ok(vec!["fd".to_string()]));
    assert_eq!(names, vec!["fd"]);
}

#[test]
fn missing_cache_with_failing_fetch_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let names = tags_for_repo(dir.path(), "sharkdp", "fd", 1000, || {
        anyhow::bail!("network down")
    });
    assert!(names.is_empty());
}

#[test]
fn unsafe_owner_skips_cache_and_fetch() {
    let dir = tempfile::tempdir().unwrap();
    let names = repos_for_owner(dir.path(), "../etc", 1000, || {
        panic!("fetch must not run for an unsafe segment")
    });
    assert!(names.is_empty());
    assert!(!dir.path().join("cache").exists());
}

#[test]
fn tags_cache_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let names = tags_for_repo(dir.path(), "sharkdp", "fd", 1000, || {
        Ok(vec!["v10.5.0".to_string(), "v10.4.0".to_string()])
    });
    assert_eq!(names, vec!["v10.5.0", "v10.4.0"]);
    assert!(
        dir.path()
            .join("cache")
            .join("completion")
            .join("tags")
            .join("sharkdp--fd.json")
            .exists()
    );
    let cached = tags_for_repo(dir.path(), "sharkdp", "fd", 1000, || {
        panic!("fetch must not run on a fresh cache")
    });
    assert_eq!(cached, names);
}

#[test]
fn shell_from_path_forms() {
    use gannet::commands::completion::shell_from_path;
    assert_eq!(shell_from_path("/bin/zsh"), Some("zsh"));
    assert_eq!(shell_from_path("/usr/local/bin/fish"), Some("fish"));
    assert_eq!(shell_from_path("bash"), Some("bash"));
    assert_eq!(
        shell_from_path(r"C:\Program Files\PowerShell\pwsh.exe"),
        Some("pwsh.exe")
    );
    assert_eq!(shell_from_path(""), None);
    assert_eq!(shell_from_path("/bin/"), None);
}
