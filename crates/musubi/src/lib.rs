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

/// Return an already rendered Kotodama diagnostic document carried by a CLI error.
///
/// The binary uses this to preserve pure JSON or SARIF output without adding an
/// `eyre` prefix while still exiting unsuccessfully.
#[must_use]
pub fn rendered_diagnostics(error: &eyre::Report) -> Option<&str> {
    cli::rendered_diagnostics(error)
}
