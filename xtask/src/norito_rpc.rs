//! Compatibility wrappers for Norito RPC fixture tooling.
//!
//! The implementation lives in `norito_codegen_exporter`; these adapters keep
//! the established `cargo xtask norito-rpc-*` command surface unchanged.

use eyre::Result;
pub use norito_codegen_exporter::FixtureOptions;
use norito_codegen_exporter::{
    JsonOutput, generate_fixtures as generate_fixtures_impl, run_verify as run_verify_impl,
};

use crate::JsonTarget;

/// Verify canonical Norito RPC fixtures and optionally write a JSON report.
pub fn run_verify(json_out: Option<JsonTarget>) -> Result<()> {
    run_verify_impl(json_out.map(json_output))
}

/// Regenerate canonical Norito RPC fixtures using the focused exporter crate.
pub fn generate_fixtures(options: FixtureOptions) -> Result<()> {
    generate_fixtures_impl(options)
}

fn json_output(target: JsonTarget) -> JsonOutput {
    match target {
        JsonTarget::Stdout => JsonOutput::Stdout,
        JsonTarget::File(path) => JsonOutput::File(path),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn output_target_adapter_preserves_variants() {
        assert_eq!(json_output(JsonTarget::Stdout), JsonOutput::Stdout);

        let path = PathBuf::from("artifacts/norito-rpc.json");
        assert_eq!(
            json_output(JsonTarget::File(path.clone())),
            JsonOutput::File(path)
        );
    }

    #[test]
    fn fixture_options_are_owned_by_exporter() {
        assert!(
            std::any::type_name::<FixtureOptions>()
                .starts_with("norito_codegen_exporter::norito_rpc::")
        );
    }

    #[test]
    fn fixture_generation_errors_are_forwarded() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let missing = temp_dir.path().join("missing.json");
        let options = FixtureOptions::new(
            Some(missing.clone()),
            None,
            Some(temp_dir.path().join("output")),
            None,
            false,
            true,
        );

        let error = generate_fixtures(options).expect_err("missing source fixture must fail");
        assert!(
            error.to_string().contains("fixtures JSON missing"),
            "unexpected delegated error: {error}"
        );
        assert!(
            error.to_string().contains(&missing.display().to_string()),
            "delegated error should retain the missing path: {error}"
        );
    }

    #[test]
    fn wrapper_remains_an_exporter_delegate() {
        let source = include_str!("norito_rpc.rs");
        let production = source
            .split_once("#[cfg(test)]")
            .map_or(source, |(production, _)| production);
        assert!(
            production.lines().count() < 50,
            "wrapper grew substantive logic"
        );
        assert!(production.contains("norito_codegen_exporter"));
        for implementation_marker in [
            "CANONICAL_MANIFEST",
            "SCHEMA_HASH_MANIFEST",
            "SIGNING_SEED_HEX",
            "SignedTransaction",
        ] {
            assert!(
                !production.contains(implementation_marker),
                "{implementation_marker} belongs in the exporter implementation"
            );
        }
    }
}
