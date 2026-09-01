use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn rerun_if_exists(path: &str) {
    // A missing rerun-if-changed path makes cargo re-run the script on every
    // build, so only emit paths that actually exist.
    if Path::new(path).exists() {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn main() {
    if let Some(git_dir) = git(&["rev-parse", "--git-dir"]) {
        // HEAD changes on checkout/detached-HEAD moves; a commit on the
        // current branch moves the branch ref (loose file or packed-refs).
        rerun_if_exists(&format!("{git_dir}/HEAD"));
        if let Some(branch_ref) = git(&["symbolic-ref", "-q", "HEAD"]) {
            rerun_if_exists(&format!("{git_dir}/{branch_ref}"));
            rerun_if_exists(&format!("{git_dir}/packed-refs"));
        }
    }
    let sha = git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=GANNET_BUILD_SHA={sha}");
}
