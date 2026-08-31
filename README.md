<div align="center">

# 🐦 gannet

**Install any GitHub release binary with one command. Roll back with another.**

[![CI](https://github.com/planesailingio/gannet/actions/workflows/ci.yml/badge.svg)](https://github.com/planesailingio/gannet/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/planesailingio/gannet?display_name=tag)](https://github.com/planesailingio/gannet/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

*Named after the seabird that dives, grabs, and never comes up empty.*

</div>

---

Thousands of brilliant CLI tools ship as GitHub release binaries — and installing them means squinting at a releases page, guessing which of fourteen `.tar.gz` files matches your machine, extracting it, fishing the binary out from between the READMEs, and hoping you remember where you put it when the next version breaks something.

**gannet does all of that in one command.** And because it keeps the previous version on disk, "the next version broke something" is a one-command fix too:

```console
$ gannet install sharkdp/fd
downloading fd-v10.5.0-aarch64-apple-darwin.tar.gz (1.3 MiB)
installed sharkdp/fd v10.5.0 -> ~/.gannet/bin/fd

$ gannet rollback sharkdp/fd
rolled sharkdp/fd back to v10.4.0 (was v10.5.0)
```

That's it. No manifests, no daemon, no waiting for someone to package the tool for your distro. If it has a GitHub release, you can install it.

## ✨ Why gannet?

- ⚡ **One command, any repo** — `gannet install owner/repo`. gannet queries the GitHub API, scores every release asset against your OS and architecture, and picks the right one. Checksums, `.deb`s, `.msi`s and source tarballs are filtered out automatically.
- ⏪ **Fearless upgrades** — the current *and* previous version of every tool stay on disk. `rollback` swaps a symlink, instantly and offline. `use` pins any version you like, tenv/tfenv-style.
- 📦 **Archive-savvy** — tar.gz, tgz, zip, gz, or a bare binary; nested folders, missing exec bits, binaries named differently from the repo (`ripgrep` → `rg`) — gannet finds the executable and ignores the packing material.
- 🪶 **Genuinely lightweight** — a single static Rust binary. No database, no background service, no config format to learn. The entire state is one human-readable JSON file you can `cat`.
- 🖥️ **Cross-platform** — macOS, Linux, and Windows, on x86_64 and arm64. Symlinks where the OS allows, transparent copy fallback where it doesn't.
- 🔌 **Ready for more than GitHub** — release sources sit behind a small provider trait, so GitLab and friends can slot in without touching the install pipeline.

## 🚀 Install

**Homebrew** (macOS / Linux):

```sh
brew install planesailingio/tools/gannet
```

**From a release**: grab your platform's archive from the [releases page](https://github.com/planesailingio/gannet/releases) and put `gannet` on your PATH. (Yes, gannet can manage itself: `gannet install planesailingio/gannet`.)

**From source**:

```sh
make install   # cargo install --path .
```

Then add gannet's bin directory to your PATH:

```sh
# bash (~/.bashrc) / zsh (~/.zshrc)
export PATH="$HOME/.gannet/bin:$PATH"

# fish
fish_add_path ~/.gannet/bin
```

On Windows, add `%USERPROFILE%\.gannet\bin` to your PATH. Enabling [Developer Mode](https://learn.microsoft.com/en-us/windows/apps/get-started/enable-your-device-for-development) lets gannet use symlinks; without it, versions are copied instead — everything works, switching is just a touch slower.

> [!TIP]
> Set `GITHUB_TOKEN` (or `GH_TOKEN`) and gannet uses it for the GitHub API — 5,000 requests/hour instead of the anonymous 60.

## 🧭 Ninety seconds of gannet

```console
$ gannet install BurntSushi/ripgrep      # installs the `rg` binary
$ gannet install sharkdp/fd@v10.3.0      # pin an exact version
$ gannet list
PACKAGE            COMMAND  CURRENT  PREVIOUS  PINNED
BurntSushi/ripgrep rg       15.2.0   -         -
sharkdp/fd         fd       v10.3.0  -         yes

$ gannet use sharkdp/fd@v10.2.0          # switch versions (fetches if needed)
$ gannet rollback sharkdp/fd             # instant, offline undo
$ gannet upgrade --all                   # everything to latest
$ gannet uninstall sharkdp/fd            # gone, cleanly
```

## 📖 Commands

| Command | What it does |
| --- | --- |
| `install <owner>/<repo>[@tag]` | Install the latest release, or pin a tag |
| `uninstall <owner>/<repo>` | Remove the command and all installed versions |
| `list [owner/repo]` | Show installed packages, or details for one |
| `upgrade [owner/repo \| --all]` | Move to the latest release |
| `rollback <owner>/<repo>` | Switch back to the previous version |
| `use <owner>/<repo>@<tag>` | Switch to a specific version (fetches it if needed) |

When a release is awkward, `install` has escape hatches:

| Flag | Use it when |
| --- | --- |
| `--asset <substring>` | Auto-detection picks the wrong asset (e.g. you want the gnu build over musl) |
| `--bin <name>` | The archive ships several executables |
| `--as <name>` | You want the command under a different name, or two packages collide |
| `--force` | Reinstall the current version from scratch |

Global: `-v/--verbose` shows the full asset-scoring table — genuinely handy when you're curious *why* gannet picked what it picked. `--gannet-dir <path>` (or `GANNET_DIR`) relocates everything.

## 🔍 How it works

No magic, just a tidy directory and a symlink:

```text
~/.gannet/
├── state.json                      # the entire "database"
├── bin/                            # ← the only thing on your PATH
│   └── fd -> ../packages/sharkdp/fd/v10.5.0/fd
├── packages/
│   └── sharkdp/fd/
│       ├── v10.5.0/fd              # current
│       └── v10.2.0/fd              # previous — rollback always has a target
└── tmp/                            # staging during installs
```

- **Asset selection** scores each asset for your platform (`darwin`/`linux`/`windows`, `arm64`/`x86_64`, and all their aliases), preferring musl/static builds on Linux and msvc on Windows.
- **Retention** keeps exactly two versions per tool: the one you're using and the one you used before. Installing a third prunes the oldest — disk usage stays flat and rollback always works.
- **Atomicity**: downloads stage under `~/.gannet/tmp` and move into place with a rename; state writes go through temp-file-and-rename. A failed install leaves your working version untouched.
- **Safety**: archives that try to escape the extraction directory (zip-slip) are rejected outright.

## 🚧 Not yet (contributions welcome!)

- `.tar.xz`, `.zst`, `.bz2`, `.7z` assets — gannet currently tells you and lists the alternatives. *The top fast-follow.*
- Checksum/signature verification of downloads.
- Private repositories, and a GitLab provider (the trait is waiting).
- Locking for concurrent gannet invocations.

Spotted a release gannet mis-detects? [Open an issue](https://github.com/planesailingio/gannet/issues) with the repo name — asset-naming heuristics only get better with real-world counterexamples, and the scoring is table-driven and easy to extend.

## 🛠️ Developing

```sh
make check    # fmt + clippy + tests — what CI runs
make build
```

Releases are automated: push a `v*` tag and CI builds macOS (arm64/x86_64), Linux (musl arm64/x86_64) and Windows binaries, publishes a GitHub release with `SHA256SUMS`, and pushes an updated formula to [planesailingio/homebrew-tools](https://github.com/planesailingio/homebrew-tools). (Maintainers: the tap push needs a `HOMEBREW_TAP_TOKEN` secret — a fine-grained PAT with contents read/write on the tap repo.)

## 📄 License

[MIT](LICENSE) © Rhys Evans

---

<div align="center">

*If gannet saved you a trip to a releases page, consider giving it a ⭐*

</div>
