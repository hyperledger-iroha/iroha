use eyre::Result;

/// Runs subcommand
pub trait RunArgs {
    /// Runs command
    ///
    /// # Errors
    /// if inner command errors
    fn run(self) -> Result<()>;
}
