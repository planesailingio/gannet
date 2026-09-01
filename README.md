# gannet

[![CI](https://github.com/planesailingio/gannet/actions/workflows/ci.yml/badge.svg)](https://github.com/planesailingio/gannet/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/planesailingio/gannet?display_name=tag)](https://github.com/planesailingio/gannet/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Installs CLI tools straight from GitHub releases, and lets you roll back when a new version breaks something.

Loads of good tools only ship as binaries on a GitHub releases page. Installing them by hand means working out which of the fourteen tar.gz files is for your machine, extracting it, digging the binary out, sticking it somewhere on your PATH, and then doing it all again next release. gannet does that bit for you:

```console
$ gannet install sharkdp/fd
downloading fd-v10.5.0-aarch64-apple-darwin.tar.gz (1.3 MiB)
installed sharkdp/fd v10.5.0 -> ~/.gannet/bin/fd

$ gannet rollback sharkdp/fd
rolled sharkdp/fd back to v10.4.0 (was v10.5.0)
```

It keeps the previous version of everything on disk, so rollback works instantly and offline. No daemon, no config files, and you don't have to wait for someone to package the tool for your distro. If it has a GitHub release you can install it.

Named after the seabird. They dive, they grab, they rarely miss.

## What it does

- `gannet install owner/repo` asks the GitHub API for the latest release, scores every asset against your OS and architecture, and picks the right one. Checksums, .debs, .msis and source tarballs get filtered out automatically.
- Handles tar.gz, tgz, zip, gz, and bare binaries. Copes with nested folders, missing exec bits, and binaries named differently from the repo (ripgrep installs as `rg`).
- Keeps the current and previous version of each tool. `rollback` just swaps a symlink. `use` pins whatever version you want, a bit like tenv/tfenv.
- It's a single static Rust binary. No database, no background service. All the state lives in one JSON file you can just cat.
- Works on macOS, Linux and Windows, x86_64 and arm64. Uses symlinks where the OS allows and falls back to copying where it doesn't.
- Release sources sit behind a small provider trait, so a GitLab provider (or whatever else) could be added without touching the install pipeline.

## Install

Homebrew (macOS / Linux):

```sh
brew install planesailingio/tools/gannet
```

Or grab your platform's archive from the [releases page](https://github.com/planesailingio/gannet/releases) and put `gannet` somewhere on your PATH. gannet can manage itself after that: `gannet install planesailingio/gannet`.

Or from source:

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

On Windows, add `%USERPROFILE%\.gannet\bin` to your PATH. If you turn on [Developer Mode](https://learn.microsoft.com/en-us/windows/apps/get-started/enable-your-device-for-development) gannet can use symlinks; without it versions get copied instead, which works fine, switching is just a bit slower.

Tip: set `GITHUB_TOKEN` (or `GH_TOKEN`) and gannet will use it for GitHub API calls. That gets you 5,000 requests an hour instead of the anonymous 60.

### Shell completion

Tab completion is dynamic: `gannet install sharkdp/<TAB>` actually asks GitHub for sharkdp's repos, `gannet install sharkdp/fd@<TAB>` lists release tags, and `gannet uninstall <TAB>` offers what you have installed. Lookups are cached for an hour under `~/.gannet/cache/` so repeat tabs are instant, and `GITHUB_TOKEN` raises the rate limit here just like it does for installs. If you use fzf-tab in zsh the candidates come up in the fuzzy menu automatically.

If you installed with Homebrew the completions are already in place (bash users may need `brew install bash-completion@2`). Otherwise it's one line in your shell config. `gannet completion` detects your shell from `$SHELL`, or you can name one (`bash`, `zsh`, `fish`, `powershell`, `elvish`):

```sh
# bash (~/.bashrc) / zsh (~/.zshrc)
source <(gannet completion)

# fish (~/.config/fish/config.fish)
gannet completion fish | source
```

One caveat: completion honours `GANNET_DIR` reliably, but `--gannet-dir` on the command line only on a best-effort basis.

## Quick tour

```console
$ gannet install BurntSushi/ripgrep      # installs the `rg` binary
$ gannet install sharkdp/fd@v10.3.0      # pin an exact version
$ gannet list
PACKAGE            COMMAND  CURRENT  PREVIOUS  PINNED
BurntSushi/ripgrep rg       15.2.0   -         -
sharkdp/fd         fd       v10.3.0  -         yes

$ gannet use sharkdp/fd@v10.2.0          # switch versions (fetches if needed)
$ gannet rollback sharkdp/fd             # undo, instantly, offline
$ gannet upgrade --all                   # everything to latest
$ gannet uninstall sharkdp/fd            # gone
```

## Commands

| Command | What it does |
| --- | --- |
| `install <owner>/<repo>[@tag]` | Install the latest release, or pin a tag |
| `uninstall <owner>/<repo>` | Remove the command and all installed versions |
| `list [owner/repo]` | Show installed packages, or details for one |
| `upgrade [owner/repo \| --all]` | Move to the latest release |
| `rollback <owner>/<repo>` | Switch back to the previous version |
| `use <owner>/<repo>@<tag>` | Switch to a specific version (fetches it if needed) |

For awkward releases, `install` has some escape hatches:

| Flag | Use it when |
| --- | --- |
| `--asset <substring>` | Auto-detection picks the wrong asset (say you want the gnu build over musl) |
| `--bin <name>` | The archive ships several executables |
| `--as <name>` | You want the command under a different name, or two packages collide |
| `--force` | Reinstall the current version from scratch |

Globally, `-v/--verbose` prints the full asset-scoring table, which is handy when you want to know why gannet picked what it picked. `--gannet-dir <path>` (or `GANNET_DIR`) moves everything somewhere else.

## How it works

There's no magic here, just a directory and a symlink:

```text
~/.gannet/
├── state.json                      # the entire "database"
├── bin/                            # the only thing on your PATH
│   └── fd -> ../packages/sharkdp/fd/v10.5.0/fd
├── packages/
│   └── sharkdp/fd/
│       ├── v10.5.0/fd              # current
│       └── v10.2.0/fd              # previous, so rollback always has a target
└── tmp/                            # staging during installs
```

Asset selection scores each release asset for your platform (darwin/linux/windows, arm64/x86_64, plus all the aliases people use in filenames), preferring musl/static builds on Linux and msvc on Windows.

gannet keeps exactly two versions per tool: the one you're on and the one before it. Installing a third prunes the oldest, so disk usage stays flat and rollback always works.

Downloads stage under `~/.gannet/tmp` and move into place with a rename, and state writes go through a temp file and rename too. A failed install leaves your working version alone. Archives that try to escape the extraction directory (zip-slip) are rejected.

## Not done yet

- .tar.xz, .zst, .bz2 and .7z assets. Right now gannet tells you and lists the alternatives. This is top of the list.
- Checksum/signature verification of downloads.
- Private repos, and a GitLab provider (the trait is there waiting).
- Locking for concurrent gannet runs.

If gannet picks the wrong asset for some repo, [open an issue](https://github.com/planesailingio/gannet/issues) with the repo name. The scoring is table driven and easy to extend, and the heuristics only get better with real counterexamples.

## Developing

```sh
make hooks    # once after cloning: enables the pre-commit hook (fmt + tests)
make check    # fmt + clippy + tests, same as CI
make build
```

`make build-release` produces an optimized local build.

Releases are cut with `make release VERSION=X.Y.Z`, which runs `make check`, bumps `Cargo.toml`/`Cargo.lock`, commits, tags `vX.Y.Z`, and pushes branch and tag together. From there everything is automated: push a `v*` tag and CI builds macOS (arm64/x86_64), Linux (musl arm64/x86_64) and Windows binaries, publishes a GitHub release with `SHA256SUMS`, and pushes an updated formula to [planesailingio/homebrew-tools](https://github.com/planesailingio/homebrew-tools). Maintainers: the tap push needs a `HOMEBREW_TAP_TOKEN` secret, a fine-grained PAT with contents read/write on the tap repo.

## License

[MIT](LICENSE) © Rhys Evans
