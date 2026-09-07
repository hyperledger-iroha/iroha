//! Inventory unexpanded Rust source for reviewed Norito identity capture.
//!
//! No canonical names are inferred, no macros/cfg are expanded, and no source
//! files are modified. The JSON contains exact source hashes/spans and an
//! explicit unresolved review queue; compiler qualification remains separate.

#[path = "../source_inventory/mod.rs"]
mod source_inventory;

use clap::Parser;
use std::path::PathBuf;

/// Inputs selecting physical Rust source, independently of Cargo targets/features.
#[derive(Parser)]
#[command(about = "Inventory unexpanded Rust declarations for Norito capture review")]
#[command(group(clap::ArgGroup::new("selection").required(true).args(["sources", "crate_roots"])))]
struct Args {
    /// Repository root used for normalized report paths.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// File or directory to inventory; repeat to select additional source roots.
    /// Paths are relative to --root. Directories recursively select .rs files;
    /// skipped .git/target subdirectories are listed in the report. Explicitly
    /// selecting such a directory includes it. Includes/modules are unresolved
    /// references, never silently expanded or followed.
    #[arg(long = "source")]
    sources: Vec<PathBuf>,
    /// Crate-root file for the source-context graph; repeat for multiple roots.
    /// Follows ordinary modules and literal item includes within --root, keeping
    /// conditional/dynamic/ambiguous paths and fragment positions in a review queue.
    /// This mode does not infer crate names or evaluate Cargo features/cfgs.
    #[arg(long = "crate-root")]
    crate_roots: Vec<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let result = (|| -> anyhow::Result<(String, bool)> {
        if args.crate_roots.is_empty() {
            let report = source_inventory::inventory(&args.root, &args.sources)?;
            Ok((
                norito::json::to_json_pretty(&report)?,
                report.files.iter().any(|file| file.parse_error.is_some()),
            ))
        } else {
            let report =
                source_inventory::context_graph::inventory_contexts(&args.root, &args.crate_roots)?;
            Ok((
                norito::json::to_json_pretty(&report)?,
                report.invalid_sources,
            ))
        }
    })();
    match result {
        Ok((json, invalid)) => {
            println!("{json}");
            if invalid {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("source inventory rejected: {error:#}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_require_one_explicit_mode_and_allow_several_crate_roots() {
        assert!(Args::try_parse_from(["inventory"]).is_err());
        assert!(
            Args::try_parse_from(["inventory", "--source", "src", "--crate-root", "src/lib.rs"])
                .is_err()
        );
        let args = Args::try_parse_from([
            "inventory",
            "--crate-root",
            "src/lib.rs",
            "--crate-root",
            "src/main.rs",
        ])
        .unwrap();
        assert_eq!(args.crate_roots.len(), 2);
    }
}
