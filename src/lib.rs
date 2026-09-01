pub mod archive;
pub mod cli;
pub mod commands;
pub mod complete;
pub mod link;
pub mod paths;
pub mod platform;
pub mod provider;
pub mod select;
pub mod state;

use paths::Dirs;

/// Shared context threaded through every command.
pub struct Ctx {
    pub dirs: Dirs,
    pub verbose: bool,
}
