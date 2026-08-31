use anyhow::{Result, bail};

use crate::platform::{Arch, Os};

/// Why an asset was chosen, plus anything worth telling the user.
#[derive(Debug)]
pub struct Selection {
    pub index: usize,
    pub warning: Option<String>,
    /// One line per asset: either its score or why it was disqualified.
    pub log: Vec<String>,
}

/// Extensions that can never be the binary we want.
const DISQUALIFIED_EXTENSIONS: &[&str] = &[
    ".sha256",
    ".sha512",
    ".md5",
    ".sig",
    ".asc",
    ".pem",
    ".sbom",
    ".intoto",
    ".json",
    ".yml",
    ".yaml",
    ".txt",
    ".md",
    ".sh",
    ".ps1",
    ".deb",
    ".rpm",
    ".apk",
    ".msi",
    ".dmg",
    ".pkg",
    ".snap",
    ".appimage",
    ".tar.xz",
    ".txz",
    ".xz",
    ".tar.bz2",
    ".bz2",
    ".tar.zst",
    ".tzst",
    ".zst",
    ".7z",
];

/// Name fragments that mark non-binary or source artifacts.
const DISQUALIFIED_TOKENS: &[&str] = &[
    "checksums",
    "sha256sums",
    "sha512sums",
    "sbom",
    "src",
    "source",
    "sources",
    "debug",
    "symbols",
    "vendored",
];

const LINUX_TOKENS: &[&str] = &["linux"];
const MACOS_TOKENS: &[&str] = &["darwin", "macos", "mac", "osx", "apple"];
const WINDOWS_TOKENS: &[&str] = &["windows", "win", "win64", "win32", "msvc", "mingw"];

const X86_64_TOKENS: &[&str] = &["x86-64", "amd64", "x64"];
const AARCH64_TOKENS: &[&str] = &["aarch64", "arm64"];
/// Architectures gannet never targets; presence disqualifies outright.
const FOREIGN_ARCH_TOKENS: &[&str] = &[
    "i686",
    "i386",
    "386",
    "x86", // bare x86 (x86-64 is checked first) means 32-bit
    "arm", // bare arm (arm64 is checked first) means 32-bit ARM
    "armv6",
    "armv7",
    "armhf",
    "riscv64",
    "ppc64",
    "ppc64le",
    "s390x",
    "mips",
    "mips64",
    "loongarch64",
    "loong64",
];
const MACOS_UNIVERSAL_TOKENS: &[&str] = &["universal", "universal2", "all"];

/// Lowercase the name and collapse separators so token checks can look for
/// `-token-` with clean boundaries. `x86_64` becomes `-x86-64-`.
fn normalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    out.push('-');
    for c in name.chars() {
        match c {
            '-' | '_' | '.' | ' ' | '+' | '(' | ')' => out.push('-'),
            c => out.extend(c.to_lowercase()),
        }
    }
    out.push('-');
    out
}

fn has_token(normalized: &str, token: &str) -> bool {
    normalized.contains(&format!("-{token}-"))
}

fn has_any_token(normalized: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|t| has_token(normalized, t))
}

fn ends_with_any<'a>(name_lower: &str, suffixes: &[&'a str]) -> Option<&'a str> {
    suffixes.iter().find(|s| name_lower.ends_with(**s)).copied()
}

fn os_tokens(os: Os) -> &'static [&'static str] {
    match os {
        Os::Linux => LINUX_TOKENS,
        Os::Macos => MACOS_TOKENS,
        Os::Windows => WINDOWS_TOKENS,
    }
}

fn arch_tokens(arch: Arch) -> &'static [&'static str] {
    match arch {
        Arch::X86_64 => X86_64_TOKENS,
        Arch::Aarch64 => AARCH64_TOKENS,
    }
}

/// Reason an asset can't be used, or its score if it can.
enum Verdict {
    Disqualified(&'static str),
    Score(i64),
}

fn judge(name: &str, os: Os, arch: Arch, repo: &str) -> Verdict {
    let lower = name.to_lowercase();
    let norm = normalize(name);

    if let Some(ext) = ends_with_any(&lower, DISQUALIFIED_EXTENSIONS) {
        return Verdict::Disqualified(match ext {
            ".tar.xz" | ".txz" | ".xz" | ".tar.bz2" | ".bz2" | ".tar.zst" | ".tzst" | ".zst"
            | ".7z" => "unsupported archive format",
            _ => "not a binary asset",
        });
    }
    if has_any_token(&norm, DISQUALIFIED_TOKENS) {
        return Verdict::Disqualified("checksum/source/debug artifact");
    }

    // Wrong OS disqualifies. A bare `.exe` counts as a Windows marker.
    for other in [Os::Linux, Os::Macos, Os::Windows] {
        if other != os && has_any_token(&norm, os_tokens(other)) {
            return Verdict::Disqualified("different operating system");
        }
    }
    if os != Os::Windows && lower.ends_with(".exe") {
        return Verdict::Disqualified("different operating system");
    }
    if has_any_token(
        &norm,
        &["freebsd", "netbsd", "openbsd", "android", "illumos"],
    ) {
        return Verdict::Disqualified("different operating system");
    }

    // Wrong or foreign arch disqualifies — except `universal` builds on macOS.
    let is_universal = os == Os::Macos && has_any_token(&norm, MACOS_UNIVERSAL_TOKENS);
    let matches_arch = has_any_token(&norm, arch_tokens(arch));
    if !matches_arch && !is_universal {
        for other in [Arch::X86_64, Arch::Aarch64] {
            if other != arch && has_any_token(&norm, arch_tokens(other)) {
                return Verdict::Disqualified("different architecture");
            }
        }
        if has_any_token(&norm, FOREIGN_ARCH_TOKENS) {
            return Verdict::Disqualified("different architecture");
        }
    }

    let mut score: i64 = 0;
    if has_any_token(&norm, os_tokens(os)) || (os == Os::Windows && lower.ends_with(".exe")) {
        score += 100;
    }
    if matches_arch {
        score += 50;
    } else if is_universal {
        score += 40;
    }
    match os {
        Os::Linux => {
            if has_any_token(&norm, &["musl", "static"]) {
                score += 20;
            } else if has_any_token(&norm, &["gnu", "glibc"]) {
                score += 10;
            }
        }
        Os::Windows => {
            if has_token(&norm, "msvc") {
                score += 20;
            }
        }
        Os::Macos => {}
    }
    let is_zip = lower.ends_with(".zip");
    let is_targz = lower.ends_with(".tar.gz") || lower.ends_with(".tgz");
    if is_targz {
        score += if os == Os::Windows { 10 } else { 15 };
    } else if is_zip {
        score += if os == Os::Windows { 15 } else { 10 };
    } else if lower.ends_with(".gz") {
        score += 8;
    } else {
        // Bare binary (no recognised archive extension).
        score += 5;
    }
    if has_token(&norm, &repo.to_lowercase()) {
        score += 3;
    }
    Verdict::Score(score)
}

/// True when a file name carries OS/arch markers (e.g. a bare-binary asset
/// like `jq-macos-arm64`) — a sign it shouldn't be used as a command name.
pub fn looks_platform_specific(name: &str) -> bool {
    let norm = normalize(name);
    [
        LINUX_TOKENS,
        MACOS_TOKENS,
        WINDOWS_TOKENS,
        X86_64_TOKENS,
        AARCH64_TOKENS,
    ]
    .iter()
    .any(|set| has_any_token(&norm, set))
        || has_any_token(&norm, FOREIGN_ARCH_TOKENS)
}

/// Pick the release asset for this platform. `override_substr` (--asset)
/// bypasses the heuristics entirely.
pub fn select_asset(
    names: &[String],
    os: Os,
    arch: Arch,
    repo: &str,
    override_substr: Option<&str>,
) -> Result<Selection> {
    if names.is_empty() {
        bail!("this release has no assets");
    }

    if let Some(substr) = override_substr {
        let lower = substr.to_lowercase();
        let matches: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect();
        return match matches.as_slice() {
            [i] => Ok(Selection {
                index: *i,
                warning: None,
                log: vec![format!("--asset '{substr}' matched {}", names[*i])],
            }),
            [] => bail!(
                "--asset '{substr}' matched no assets; available:\n  {}",
                names.join("\n  ")
            ),
            many => bail!(
                "--asset '{substr}' matched {} assets; be more specific:\n  {}",
                many.len(),
                many.iter()
                    .map(|i| names[*i].as_str())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ),
        };
    }

    let mut log = Vec::with_capacity(names.len());
    let mut best: Option<(usize, i64)> = None;
    let mut survivors = 0usize;
    for (i, name) in names.iter().enumerate() {
        match judge(name, os, arch, repo) {
            Verdict::Disqualified(reason) => log.push(format!("skip  {name}: {reason}")),
            Verdict::Score(score) => {
                log.push(format!("{score:>5} {name}"));
                survivors += 1;
                let better = match best {
                    None => true,
                    Some((bi, bs)) => {
                        score > bs
                            || (score == bs
                                && (name.len(), name.as_str())
                                    < (names[bi].len(), names[bi].as_str()))
                    }
                };
                if better {
                    best = Some((i, score));
                }
            }
        }
    }

    match best {
        Some((index, score)) if score >= 100 => Ok(Selection {
            index,
            warning: None,
            log,
        }),
        Some((index, _)) if survivors == 1 => Ok(Selection {
            index,
            warning: Some(format!(
                "no OS marker in any asset name; guessing '{}'",
                names[index]
            )),
            log,
        }),
        _ => bail!(
            "could not find an asset for your platform; use --asset <substring> to pick one:\n  {}",
            names.join("\n  ")
        ),
    }
}
