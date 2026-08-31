use anyhow::{Result, bail};
use std::str::FromStr;

use crate::Ctx;
use crate::cli::PackageSpec;
use crate::commands::install::{self, InstallOpts};
use crate::state::State;

pub fn run(ctx: &Ctx, spec: Option<&PackageSpec>, all: bool) -> Result<()> {
    let state_file = ctx.dirs.state_file();
    let state = State::load(&state_file)?;

    let keys: Vec<String> = match (spec, all) {
        (Some(spec), false) => {
            if spec.tag.is_some() {
                bail!(
                    "upgrade always targets the latest release; use 'gannet use' for a specific version"
                );
            }
            vec![spec.key()]
        }
        (None, true) => state.packages.keys().cloned().collect(),
        (None, false) => bail!("give a package to upgrade, or --all"),
        (Some(_), true) => unreachable!("clap forbids spec with --all"),
    };
    if keys.is_empty() {
        println!("nothing installed yet");
        return Ok(());
    }

    let mut failures = Vec::new();
    for key in &keys {
        let was_pinned = state.get(key).map(|p| p.pinned).unwrap_or(false);
        let spec = PackageSpec::from_str(key).expect("state keys are valid specs");
        if was_pinned {
            println!("{key} was pinned; upgrading to latest and unpinning");
        }
        if let Err(e) = install::run(ctx, &spec, &InstallOpts::default()) {
            eprintln!("error upgrading {key}: {e:#}");
            failures.push(key.clone());
        }
    }
    if !failures.is_empty() {
        bail!(
            "{} of {} upgrades failed: {}",
            failures.len(),
            keys.len(),
            failures.join(", ")
        );
    }
    Ok(())
}
