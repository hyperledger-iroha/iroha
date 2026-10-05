//! Generate or check the tracked IVM proof-coverage inventory. Usage:
//!   cargo run --locked -p ivm --features dev-tools --bin gen_proof_coverage_inventory -- --write
//!   cargo run --locked -p ivm --features dev-tools --bin gen_proof_coverage_inventory -- --check
//!   cargo run --locked -p ivm --features dev-tools --bin gen_proof_coverage_inventory -- --write --root /tmp/ivm-doc-stage
use std::path::{Path, PathBuf};
mod support;
use support::{GeneratedOutput, parse_generation_options, sync_generated_outputs};

const REGENERATE: &str =
    "cargo run --locked -p ivm --features dev-tools --bin gen_proof_coverage_inventory -- --write";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

/// Destination of the rendered inventory below a workspace or staging root.
fn artifact_path(root: &Path) -> PathBuf {
    root.join(ivm::proof_coverage::INVENTORY_ARTIFACT_PATH)
}

fn main() {
    let options = match parse_generation_options(std::env::args().skip(1), workspace_root()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let output = GeneratedOutput::exact(
        artifact_path(&options.root),
        ivm::proof_coverage::render_inventory_json(),
    )
    .unwrap_or_else(|error| panic!("render proof-coverage inventory: {error}"));
    let updated = sync_generated_outputs(&[output], options.mode, REGENERATE)
        .unwrap_or_else(|error| panic!("{error}"));
    for path in updated {
        eprintln!("updated: {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_path_is_the_tracked_inventory_below_the_selected_root() {
        assert_eq!(
            artifact_path(Path::new("/stage")),
            Path::new("/stage/crates/ivm/docs/proof_coverage_inventory.json")
        );
        assert!(
            artifact_path(&workspace_root())
                .parent()
                .is_some_and(Path::is_dir),
            "the tracked artifact lives in the crate docs directory"
        );
        assert!(REGENERATE.contains("gen_proof_coverage_inventory -- --write"));
    }
}
