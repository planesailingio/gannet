use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cli::PackageSpec;
use crate::state::{LinkMode, State, now_unix, tags_match};
use crate::{Ctx, archive, link, select};

#[derive(Default)]
pub struct InstallOpts<'a> {
    pub bin: Option<&'a str>,
    pub asset: Option<&'a str>,
    pub link_as: Option<&'a str>,
    pub force: bool,
}

/// The shared pipeline behind install, upgrade and use-with-download:
/// resolve → select → download → extract → discover → commit → link → prune.
pub fn run(ctx: &Ctx, spec: &PackageSpec, opts: &InstallOpts) -> Result<()> {
    let key = spec.key();
    let state_file = ctx.dirs.state_file();
    let mut state = State::load(&state_file)?;

    let provider_id = state
        .get(&key)
        .map(|p| p.provider.clone())
        .unwrap_or_else(|| "github".to_string());
    let provider = crate::provider::get(&provider_id)?;

    let pinned = spec.tag.is_some();
    let release = match &spec.tag {
        Some(tag) => provider.release_by_tag(&spec.owner, &spec.repo, tag)?,
        None => provider.latest_release(&spec.owner, &spec.repo)?,
    };
    if ctx.verbose {
        eprintln!("resolved {key} to release {}", release.tag);
    }

    // Already current, or already on disk? Then this is a switch, not a
    // download.
    if let Some(pkg) = state.get(&key) {
        if tags_match(&pkg.current, &release.tag) && !opts.force {
            println!("{key} {} is already installed and current", release.tag);
            return Ok(());
        }
        if !opts.force {
            if let Some(on_disk) = pkg.version_on_disk(&release.tag) {
                let tag = on_disk.tag.clone();
                let bin_dir = crate::commands::version_binary(
                    ctx,
                    &spec.owner,
                    &spec.repo,
                    &tag,
                    &pkg.bin_name,
                );
                if bin_dir.exists() {
                    crate::commands::relink(ctx, &spec.owner, &spec.repo, &tag, pkg)?;
                    state.switch_current(&key, &tag, pinned)?;
                    state.save(&state_file)?;
                    println!("switched {key} to {tag} (already on disk)");
                    return Ok(());
                }
                // Self-heal: state thinks it's on disk but the file is gone.
                state
                    .packages
                    .get_mut(&key)
                    .expect("package present")
                    .forget_version(&tag);
            }
        }
    }

    let names: Vec<String> = release.assets.iter().map(|a| a.name.clone()).collect();
    let (os, arch) = crate::platform::current()?;
    let selection = select::select_asset(&names, os, arch, &spec.repo, opts.asset)?;
    if ctx.verbose {
        for line in &selection.log {
            eprintln!("  {line}");
        }
    }
    if let Some(warning) = &selection.warning {
        eprintln!("warning: {warning}");
    }
    let asset = &release.assets[selection.index];

    // Stage under ~/.gannet/tmp so the final rename stays on one filesystem.
    let staging =
        tempfile::tempdir_in(&ctx.dirs.tmp).context("could not create a staging directory")?;
    let safe_name = Path::new(&asset.name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("asset")
        .to_string();
    let download_path = staging.path().join(&safe_name);
    println!("downloading {} ({})", asset.name, human_size(asset.size));
    provider.download(asset, &download_path)?;

    let extract_dir = staging.path().join("extracted");
    fs::create_dir_all(&extract_dir)?;
    let format = archive::detect_format(&safe_name, &download_path)?;
    if ctx.verbose {
        eprintln!("extracting as {format:?}");
    }
    archive::extract(&download_path, format, &extract_dir, &safe_name)?;
    let found = archive::discover(&extract_dir, os, &spec.repo, opts.bin)?;

    let discovered_stem = found
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&spec.repo)
        .to_string();
    let bin_name = match opts.link_as {
        Some(name) => name.to_string(),
        // A stem full of platform markers (bare assets like jq-macos-arm64)
        // makes a poor command name; fall back to the repo name.
        None if select::looks_platform_specific(&discovered_stem) => spec.repo.clone(),
        None => discovered_stem,
    };

    // Refuse to shadow a command owned by a different package.
    if let Some((other, _)) = state
        .packages
        .iter()
        .find(|(k, p)| **k != key && p.bin_name == bin_name)
    {
        bail!(
            "'{bin_name}' is already provided by {other}; install with --as <name> to use a different command name"
        );
    }

    let version_dir = ctx.dirs.version_dir(&spec.owner, &spec.repo, &release.tag);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("could not create {}", version_dir.display()))?;
    let dest = version_dir.join(link::link_file_name(&bin_name));
    if fs::rename(&found, &dest).is_err() {
        fs::copy(&found, &dest)
            .with_context(|| format!("could not move the binary into {}", dest.display()))?;
    }
    archive::ensure_executable(&dest)?;

    let mode = link::link(&ctx.dirs.bin, &bin_name, &dest)?;
    if mode == LinkMode::Copy {
        eprintln!(
            "note: symlinks are unavailable (enable Windows Developer Mode to use them); installed a copy instead"
        );
    }

    let pruned = state.record_install(
        &key,
        provider.id(),
        &bin_name,
        mode,
        &release.tag,
        &asset.name,
        pinned,
        now_unix(),
    );
    state.save(&state_file)?;
    for tag in pruned {
        let dir = ctx.dirs.version_dir(&spec.owner, &spec.repo, &tag);
        if ctx.verbose {
            eprintln!("pruning {}", dir.display());
        }
        let _ = fs::remove_dir_all(dir);
    }

    println!(
        "installed {key} {} -> {}",
        release.tag,
        ctx.dirs.bin.join(link::link_file_name(&bin_name)).display()
    );
    Ok(())
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
