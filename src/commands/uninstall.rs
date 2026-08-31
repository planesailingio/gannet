use std::fs;

use anyhow::{Context, Result};

use crate::cli::PackageSpec;
use crate::state::State;
use crate::{Ctx, link};

pub fn run(ctx: &Ctx, spec: &PackageSpec) -> Result<()> {
    let key = spec.key();
    let state_file = ctx.dirs.state_file();
    let mut state = State::load(&state_file)?;
    let pkg = state
        .remove(&key)
        .with_context(|| format!("{key} is not installed"))?;

    link::unlink(&ctx.dirs.bin, &pkg.bin_name)?;

    let pkg_dir = ctx.dirs.package_dir(&spec.owner, &spec.repo);
    if pkg_dir.exists() {
        fs::remove_dir_all(&pkg_dir)
            .with_context(|| format!("could not remove {}", pkg_dir.display()))?;
    } else {
        eprintln!("warning: {} was already gone", pkg_dir.display());
    }
    // Tidy the owner directory if this was its last package.
    if let Some(owner_dir) = pkg_dir.parent() {
        let _ = fs::remove_dir(owner_dir);
    }

    state.save(&state_file)?;
    println!("uninstalled {key} (removed command '{}')", pkg.bin_name);
    Ok(())
}
