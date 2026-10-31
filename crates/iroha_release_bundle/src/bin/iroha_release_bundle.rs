//! Native release publication command; the parent owns the continuous workspace source census.

#[path = "iroha_release_bundle/cli.rs"]
mod cli;

fn main() -> std::process::ExitCode {
    match cli::run(std::env::args_os().skip(1), &mut std::io::stdout().lock()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("release bundle: {error}");
            std::process::ExitCode::from(error.exit_code())
        }
    }
}
