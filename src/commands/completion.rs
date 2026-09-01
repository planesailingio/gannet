use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::env::Shells;

use crate::cli::Cli;

/// Names accepted by `gannet completion <shell>`, for clap's value parser.
pub const SHELLS: [&str; 5] = ["bash", "elvish", "fish", "powershell", "zsh"];

/// The shell name from a `$SHELL`-style value: the basename of the path.
pub fn shell_from_path(shell: &str) -> Option<&str> {
    let name = shell.rsplit(['/', '\\']).next()?.trim();
    (!name.is_empty()).then_some(name)
}

/// Print the completion registration script for `shell`, or for the shell
/// named by `$SHELL` when none is given.
pub fn run(shell: Option<&str>) -> Result<()> {
    let env_shell = std::env::var("SHELL").ok();
    let name = match shell {
        Some(s) => s,
        None => env_shell
            .as_deref()
            .and_then(shell_from_path)
            .context("could not detect your shell from $SHELL; pass it explicitly, e.g. 'gannet completion zsh'")?,
    };
    let shells = Shells::builtins();
    let Some(completer) = shells.completer(name) else {
        bail!(
            "unsupported shell '{name}' (supported: {})",
            SHELLS.join(", ")
        );
    };

    // How the registration script should invoke gannet for candidates:
    // keep a bare invocation bare so it resolves on PATH at completion
    // time, but absolutize a relative path (mirrors clap_complete).
    let mut bin = PathBuf::from(
        std::env::args_os()
            .next()
            .unwrap_or_else(|| "gannet".into()),
    );
    if bin.components().count() > 1
        && !bin.is_absolute()
        && let Ok(cwd) = std::env::current_dir()
    {
        bin = cwd.join(bin);
    }

    let cmd = Cli::command();
    completer
        .write_registration(
            "COMPLETE",
            cmd.get_name(),
            cmd.get_name(),
            &bin.to_string_lossy(),
            &mut std::io::stdout(),
        )
        .context("could not write the completion script")?;
    Ok(())
}
