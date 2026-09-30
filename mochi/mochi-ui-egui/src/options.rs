//! Small desktop entry-point contract; network lifecycle belongs to Kagami.

use std::{ffi::OsString, path::PathBuf};

pub(crate) const HELP: &str = "Mochi — Iroha developer workspace\n\nUsage: mochi [--workspace <DIRECTORY>]\n\nOptions:\n  --workspace <DIRECTORY>  Share Kagami contexts for this workspace (default: current directory)\n  -h, --help               Show this help\n  -V, --version            Show the installed version\n\nStart a persistent four-validator localnet from the desktop, or use `kagami localnet up`.\nNo supplied TOML files are required. Closing Mochi leaves your managed network running.\n";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Options {
    Help,
    Version,
    Open(Option<PathBuf>),
}

pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut arguments = arguments.into_iter();
    let mut workspace = None;
    while let Some(argument) = arguments.next() {
        if argument == "--help" || argument == "-h" {
            return Ok(Options::Help);
        }
        if argument == "--version" || argument == "-V" {
            return Ok(Options::Version);
        }
        if argument == "--workspace" {
            if workspace.is_some() {
                return Err("--workspace may be provided once".into());
            }
            let path = arguments
                .next()
                .filter(|value| !value.is_empty())
                .ok_or("--workspace requires a directory")?;
            workspace = Some(PathBuf::from(path));
        } else {
            return Err(format!(
                "Unknown argument: {}. Use mochi --help.",
                argument.to_string_lossy()
            ));
        }
    }
    Ok(Options::Open(workspace))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn workspace_is_explicit_and_supplied_configs_are_rejected() {
        assert_eq!(parse(args(&[])).unwrap(), Options::Open(None));
        assert_eq!(
            parse(args(&["--workspace", "/project"])).unwrap(),
            Options::Open(Some("/project".into()))
        );
        assert!(parse(args(&["sandbox", "serve"])).is_err());
        assert!(parse(args(&["--config", "config.toml"])).is_err());
        assert!(parse(args(&["--workspace"])).is_err());
        assert!(parse(args(&["--workspace", "a", "--workspace", "b"])).is_err());
    }
    #[test]
    fn help_and_version_need_no_runtime_or_gui() {
        assert_eq!(parse(args(&["--help"])).unwrap(), Options::Help);
        assert_eq!(parse(args(&["--version"])).unwrap(), Options::Version);
        assert!(HELP.contains("Usage: mochi [--workspace <DIRECTORY>]"));
        assert!(HELP.contains("kagami localnet up"));
    }
}
