pub mod install;
pub mod list;
pub mod rollback;
pub mod uninstall;
pub mod upgrade;
pub mod use_cmd;

use std::path::PathBuf;

use crate::Ctx;
use crate::link;
use crate::state::Package;

/// Absolute path of the binary for one installed version of a package.
pub fn version_binary(ctx: &Ctx, owner: &str, repo: &str, tag: &str, bin_name: &str) -> PathBuf {
    ctx.dirs
        .version_dir(owner, repo, tag)
        .join(link::link_file_name(bin_name))
}

/// Re-point the bin/ entry of `pkg` at the given tag's binary.
pub fn relink(
    ctx: &Ctx,
    owner: &str,
    repo: &str,
    tag: &str,
    pkg: &Package,
) -> anyhow::Result<crate::state::LinkMode> {
    let target = version_binary(ctx, owner, repo, tag, &pkg.bin_name);
    if !target.exists() {
        anyhow::bail!(
            "the binary for {owner}/{repo}@{tag} is missing from disk ({}); reinstall it with 'gannet use {owner}/{repo}@{tag}'",
            target.display()
        );
    }
    link::link(&ctx.dirs.bin, &pkg.bin_name, &target)
}
