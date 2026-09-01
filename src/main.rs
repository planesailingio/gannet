use clap::{CommandFactory, Parser};
use gannet::cli::{Cli, Command};
use gannet::commands::install::InstallOpts;
use gannet::{Ctx, commands, link, paths};

fn main() {
    // Handles shell completion requests (COMPLETE=<shell>) and exits; a
    // no-op otherwise. Must run before parse so TAB presses never touch
    // the filesystem via Dirs::new/sweep_old below.
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    // No Ctx for `completion`: printing a script shouldn't create ~/.gannet.
    if let Command::Completion { shell } = &cli.command {
        return commands::completion::run(shell.as_deref());
    }

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
        Command::Completion { .. } => unreachable!("handled before Ctx is built"),
    }
}
