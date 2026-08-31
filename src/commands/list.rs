use anyhow::{Context, Result};

use crate::cli::PackageSpec;
use crate::state::{State, format_date};
use crate::{Ctx, link};

pub fn run(ctx: &Ctx, spec: Option<&PackageSpec>) -> Result<()> {
    let state = State::load(&ctx.dirs.state_file())?;

    if let Some(spec) = spec {
        let key = spec.key();
        let pkg = state
            .get(&key)
            .with_context(|| format!("{key} is not installed"))?;
        println!("{key} ({} provider)", pkg.provider);
        println!(
            "  command: {}",
            ctx.dirs
                .bin
                .join(link::link_file_name(&pkg.bin_name))
                .display()
        );
        for v in &pkg.versions {
            let marker = if v.tag == pkg.current { "*" } else { " " };
            println!(
                "  {marker} {}  installed {}  (from {})",
                v.tag,
                format_date(v.installed_at),
                v.asset
            );
        }
        if pkg.pinned {
            println!("  pinned: yes ('gannet upgrade {key}' moves to latest)");
        }
        return Ok(());
    }

    if state.packages.is_empty() {
        println!("nothing installed yet — try: gannet install sharkdp/fd");
        return Ok(());
    }
    let mut rows = vec![[
        "PACKAGE".to_string(),
        "COMMAND".to_string(),
        "CURRENT".to_string(),
        "PREVIOUS".to_string(),
        "PINNED".to_string(),
    ]];
    for (key, pkg) in &state.packages {
        rows.push([
            key.clone(),
            pkg.bin_name.clone(),
            pkg.current.clone(),
            pkg.previous()
                .map(|v| v.tag.clone())
                .unwrap_or_else(|| "-".into()),
            if pkg.pinned { "yes".into() } else { "-".into() },
        ]);
    }
    let widths: Vec<usize> = (0..5)
        .map(|i| rows.iter().map(|r| r[i].len()).max().unwrap_or(0))
        .collect();
    for row in rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| format!("{cell:<width$}", width = widths[i]))
            .collect();
        println!("{}", line.join("  ").trim_end());
    }
    Ok(())
}
