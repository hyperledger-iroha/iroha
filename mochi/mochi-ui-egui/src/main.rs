//! Mochi desktop entry point over Kagami's shared managed workspace.
#[cfg(feature = "gui")]
mod gui;
mod options;

fn main() -> std::process::ExitCode {
    match options::parse(std::env::args_os().skip(1)) {
        Ok(options::Options::Help) => {
            print!("{}", options::HELP);
            std::process::ExitCode::SUCCESS
        }
        Ok(options::Options::Version) => {
            println!("mochi {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Ok(options::Options::Open(workspace)) => launch(workspace),
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "gui")]
fn launch(workspace: Option<std::path::PathBuf>) -> std::process::ExitCode {
    let path = workspace.map(Ok).unwrap_or_else(std::env::current_dir);
    match path
        .map_err(|error| error.to_string())
        .and_then(|path| gui::run(path).map_err(|error| error.to_string()))
    {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "gui"))]
fn launch(_workspace: Option<std::path::PathBuf>) -> std::process::ExitCode {
    eprintln!(
        "This build has no desktop UI. Install the Mochi bundle or build with `-p mochi-ui --features gui`."
    );
    std::process::ExitCode::FAILURE
}
