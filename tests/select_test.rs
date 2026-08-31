use gannet::platform::{Arch, Os};
use gannet::select::select_asset;

fn pick(names: &[&str], os: Os, arch: Arch, repo: &str) -> String {
    let owned: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    let sel = select_asset(&owned, os, arch, repo, None).expect("selection should succeed");
    owned[sel.index].clone()
}

/// sharkdp/fd style: every OS/arch combination, gnu and musl, deb noise.
const FD: &[&str] = &[
    "fd-v10.3.0-aarch64-apple-darwin.tar.gz",
    "fd-v10.3.0-aarch64-unknown-linux-gnu.tar.gz",
    "fd-v10.3.0-aarch64-unknown-linux-musl.tar.gz",
    "fd-v10.3.0-arm-unknown-linux-gnueabihf.tar.gz",
    "fd-v10.3.0-arm-unknown-linux-musleabihf.tar.gz",
    "fd-v10.3.0-i686-pc-windows-msvc.zip",
    "fd-v10.3.0-i686-unknown-linux-gnu.tar.gz",
    "fd-v10.3.0-i686-unknown-linux-musl.tar.gz",
    "fd-v10.3.0-x86_64-apple-darwin.tar.gz",
    "fd-v10.3.0-x86_64-pc-windows-gnu.zip",
    "fd-v10.3.0-x86_64-pc-windows-msvc.zip",
    "fd-v10.3.0-x86_64-unknown-linux-gnu.tar.gz",
    "fd-v10.3.0-x86_64-unknown-linux-musl.tar.gz",
    "fd_10.3.0_amd64.deb",
    "fd_10.3.0_arm64.deb",
];

#[test]
fn fd_macos_arm() {
    assert_eq!(
        pick(FD, Os::Macos, Arch::Aarch64, "fd"),
        "fd-v10.3.0-aarch64-apple-darwin.tar.gz"
    );
}

#[test]
fn fd_linux_prefers_musl() {
    assert_eq!(
        pick(FD, Os::Linux, Arch::X86_64, "fd"),
        "fd-v10.3.0-x86_64-unknown-linux-musl.tar.gz"
    );
    assert_eq!(
        pick(FD, Os::Linux, Arch::Aarch64, "fd"),
        "fd-v10.3.0-aarch64-unknown-linux-musl.tar.gz"
    );
}

#[test]
fn fd_windows_prefers_msvc() {
    assert_eq!(
        pick(FD, Os::Windows, Arch::X86_64, "fd"),
        "fd-v10.3.0-x86_64-pc-windows-msvc.zip"
    );
}

/// BurntSushi/ripgrep style: binary name differs from repo name.
const RIPGREP: &[&str] = &[
    "ripgrep-14.1.1-aarch64-apple-darwin.tar.gz",
    "ripgrep-14.1.1-aarch64-unknown-linux-gnu.tar.gz",
    "ripgrep-14.1.1-i686-pc-windows-msvc.zip",
    "ripgrep-14.1.1-x86_64-apple-darwin.tar.gz",
    "ripgrep-14.1.1-x86_64-pc-windows-gnu.zip",
    "ripgrep-14.1.1-x86_64-pc-windows-msvc.zip",
    "ripgrep-14.1.1-x86_64-unknown-linux-musl.tar.gz",
    "ripgrep_14.1.1-1_amd64.deb",
    "ripgrep-14.1.1-x86_64-unknown-linux-musl.tar.gz.sha256",
];

#[test]
fn ripgrep_ignores_checksums_and_debs() {
    assert_eq!(
        pick(RIPGREP, Os::Linux, Arch::X86_64, "ripgrep"),
        "ripgrep-14.1.1-x86_64-unknown-linux-musl.tar.gz"
    );
}

/// jqlang/jq style: bare binaries, plus a source tarball with no OS marker.
const JQ: &[&str] = &[
    "jq-1.7.1.tar.gz",
    "jq-1.7.1.zip",
    "jq-linux-amd64",
    "jq-linux-arm64",
    "jq-linux-i386",
    "jq-macos-amd64",
    "jq-macos-arm64",
    "jq-windows-amd64.exe",
    "jq-windows-i386.exe",
    "sha256sum.txt",
];

#[test]
fn jq_bare_binaries() {
    assert_eq!(pick(JQ, Os::Macos, Arch::Aarch64, "jq"), "jq-macos-arm64");
    assert_eq!(pick(JQ, Os::Linux, Arch::X86_64, "jq"), "jq-linux-amd64");
    assert_eq!(
        pick(JQ, Os::Windows, Arch::X86_64, "jq"),
        "jq-windows-amd64.exe"
    );
}

/// junegunn/fzf style: underscore separators, zip on mac/windows.
const FZF: &[&str] = &[
    "fzf-0.55.0-darwin_amd64.tar.gz",
    "fzf-0.55.0-darwin_arm64.tar.gz",
    "fzf-0.55.0-linux_amd64.tar.gz",
    "fzf-0.55.0-linux_arm64.tar.gz",
    "fzf-0.55.0-windows_amd64.zip",
    "fzf_0.55.0_checksums.txt",
];

#[test]
fn fzf_underscore_names() {
    assert_eq!(
        pick(FZF, Os::Macos, Arch::Aarch64, "fzf"),
        "fzf-0.55.0-darwin_arm64.tar.gz"
    );
    assert_eq!(
        pick(FZF, Os::Windows, Arch::X86_64, "fzf"),
        "fzf-0.55.0-windows_amd64.zip"
    );
}

/// cli/cli (gh) style: mixed case, msi/deb/rpm noise.
const GH: &[&str] = &[
    "gh_2.55.0_checksums.txt",
    "gh_2.55.0_linux_386.deb",
    "gh_2.55.0_linux_amd64.deb",
    "gh_2.55.0_linux_amd64.rpm",
    "gh_2.55.0_linux_amd64.tar.gz",
    "gh_2.55.0_linux_arm64.tar.gz",
    "gh_2.55.0_macOS_amd64.zip",
    "gh_2.55.0_macOS_arm64.zip",
    "gh_2.55.0_windows_amd64.msi",
    "gh_2.55.0_windows_amd64.zip",
];

#[test]
fn gh_mixed_case_and_noise() {
    assert_eq!(
        pick(GH, Os::Macos, Arch::Aarch64, "cli"),
        "gh_2.55.0_macOS_arm64.zip"
    );
    assert_eq!(
        pick(GH, Os::Linux, Arch::X86_64, "cli"),
        "gh_2.55.0_linux_amd64.tar.gz"
    );
    assert_eq!(
        pick(GH, Os::Windows, Arch::X86_64, "cli"),
        "gh_2.55.0_windows_amd64.zip"
    );
}

#[test]
fn macos_universal_is_accepted() {
    let assets = &[
        "tool-1.0-macos-universal.tar.gz",
        "tool-1.0-linux-amd64.tar.gz",
    ];
    assert_eq!(
        pick(assets, Os::Macos, Arch::Aarch64, "tool"),
        "tool-1.0-macos-universal.tar.gz"
    );
}

#[test]
fn single_unmarked_asset_is_guessed_with_warning() {
    let assets: Vec<String> = vec!["tool-1.2.3.tar.gz".to_string()];
    let sel = select_asset(&assets, Os::Linux, Arch::X86_64, "tool", None).unwrap();
    assert_eq!(sel.index, 0);
    assert!(sel.warning.is_some());
}

#[test]
fn no_matching_platform_fails_with_asset_hint() {
    let assets: Vec<String> = vec![
        "tool-linux-amd64.tar.gz".to_string(),
        "tool-windows-amd64.zip".to_string(),
    ];
    let err = select_asset(&assets, Os::Macos, Arch::Aarch64, "tool", None).unwrap_err();
    assert!(err.to_string().contains("--asset"));
}

#[test]
fn asset_override_bypasses_scoring() {
    let owned: Vec<String> = FD.iter().map(|s| s.to_string()).collect();
    let sel = select_asset(&owned, Os::Linux, Arch::X86_64, "fd", Some("windows-gnu")).unwrap();
    assert_eq!(owned[sel.index], "fd-v10.3.0-x86_64-pc-windows-gnu.zip");
}

#[test]
fn ambiguous_asset_override_fails() {
    let owned: Vec<String> = FD.iter().map(|s| s.to_string()).collect();
    assert!(select_asset(&owned, Os::Linux, Arch::X86_64, "fd", Some("linux")).is_err());
}
