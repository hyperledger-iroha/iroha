//! Musubi package-manager library entrypoint.

pub mod archive_fetch;
mod atomic_io;
pub mod cache;
mod cli;
mod command;
mod compiler;
mod graph;
mod lockfile;
pub mod manifest;
mod output;
mod package;
pub mod publication_runtime;
pub mod publish;
pub mod registry;
mod registry_cache;
mod resolver;
mod test_runner;
pub mod workspace;

/// Run the Musubi command-line interface.
///
/// Return the stable process exit status after writing routed command output.
#[must_use]
pub fn run() -> i32 {
    cli::run()
}
