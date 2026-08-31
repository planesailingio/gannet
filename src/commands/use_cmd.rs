use anyhow::{Result, bail};

use crate::Ctx;
use crate::cli::PackageSpec;
use crate::commands::install::{self, InstallOpts};
use crate::state::{State, tags_match};

pub fn run(ctx: &Ctx, spec: &PackageSpec, extra_tag: Option<&str>) -> Result<()> {
    let tag = match (&spec.tag, extra_tag) {
        (Some(t), None) => t.clone(),
        (None, Some(t)) => t.to_string(),
        (Some(_), Some(_)) => {
            bail!("give the version either as @<tag> or as a second argument, not both")
        }
        (None, None) => bail!(
            "which version? use '{0}@<tag>' (e.g. gannet use {0}@v1.2.3)",
            spec.key()
        ),
    };
    let key = spec.key();
    let state_file = ctx.dirs.state_file();
    let mut state = State::load(&state_file)?;

    if let Some(pkg) = state.get(&key) {
        if tags_match(&pkg.current, &tag) {
            println!("{key} is already using {}", pkg.current);
            return Ok(());
        }
        if let Some(on_disk) = pkg.version_on_disk(&tag) {
            let real_tag = on_disk.tag.clone();
            let binary = crate::commands::version_binary(
                ctx,
                &spec.owner,
                &spec.repo,
                &real_tag,
                &pkg.bin_name,
            );
            if binary.exists() {
                crate::commands::relink(ctx, &spec.owner, &spec.repo, &real_tag, pkg)?;
                state.switch_current(&key, &real_tag, true)?;
                state.save(&state_file)?;
                println!("switched {key} to {real_tag}");
                return Ok(());
            }
            // Self-heal: recorded but missing on disk — fall through to a
            // fresh download.
            state
                .packages
                .get_mut(&key)
                .expect("package present")
                .forget_version(&real_tag);
            state.save(&state_file)?;
        }
    }

    let pinned_spec = PackageSpec {
        owner: spec.owner.clone(),
        repo: spec.repo.clone(),
        tag: Some(tag),
    };
    install::run(ctx, &pinned_spec, &InstallOpts::default())
}
