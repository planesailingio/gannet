use clap::Parser;
use gannet::cli::{Cli, Command};
use gannet::commands::install::InstallOpts;
use gannet::{Ctx, commands, link, paths};

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let ctx = Ctx {
        dirs: paths::Dirs::new(cli.gannet_dir)?,
        verbose: cli.verbose,
    };
    link::sweep_old(&ctx.dirs.bin);

    match &cli.command {
        Command::Install {
            spec,
            bin,
            asset,
            link_as,
            force,
        } => commands::install::run(
            &ctx,
            spec,
            &InstallOpts {
                bin: bin.as_deref(),
                asset: asset.as_deref(),
                link_as: link_as.as_deref(),
                force: *force,
            },
        ),
        Command::Uninstall { spec } => commands::uninstall::run(&ctx, spec),
        Command::List { spec } => commands::list::run(&ctx, spec.as_ref()),
        Command::Upgrade { spec, all } => commands::upgrade::run(&ctx, spec.as_ref(), *all),
        Command::Rollback { spec } => commands::rollback::run(&ctx, spec),
        Command::Use { spec, tag } => commands::use_cmd::run(&ctx, spec, tag.as_deref()),
    }
}
