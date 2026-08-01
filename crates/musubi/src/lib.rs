//! Musubi package-manager library entrypoint.

mod atomic_io;
pub mod cache;
mod cli;
mod command;
mod lockfile;
pub mod manifest;
mod output;
mod package;
pub mod publish;
pub mod registry;
mod resolver;
pub mod workspace;

/// Run the Musubi command-line interface.
///
/// Return the stable process exit status after writing routed command output.
#[must_use]
pub fn run() -> i32 {
    cli::run()
}
