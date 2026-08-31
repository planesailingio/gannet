use anyhow::{Context, Result, bail};

use crate::Ctx;
use crate::cli::PackageSpec;
use crate::state::State;

pub fn run(ctx: &Ctx, spec: &PackageSpec) -> Result<()> {
    let key = spec.key();
    let state_file = ctx.dirs.state_file();
    let mut state = State::load(&state_file)?;
    let pkg = state
        .get(&key)
        .with_context(|| format!("{key} is not installed"))?
        .clone();
    let Some(previous) = pkg.previous() else {
        bail!(
            "no previous version of {key} on disk; fetch a specific one with 'gannet use {key}@<tag>'"
        );
    };
    let tag = previous.tag.clone();
    crate::commands::relink(ctx, &spec.owner, &spec.repo, &tag, &pkg)?;
    state.switch_current(&key, &tag, pkg.pinned)?;
    state.save(&state_file)?;
    println!("rolled {key} back to {tag} (was {})", pkg.current);
    Ok(())
}
