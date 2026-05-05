//! Musubi package-manager library entrypoint.

mod cli;

/// Run the Musubi command-line interface.
///
/// # Errors
///
/// Returns an error when command parsing, manifest IO, packaging, or Kotodama
/// compilation fails.
pub fn run() -> eyre::Result<()> {
    cli::run()
}
