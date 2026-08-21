//! Focused wrappers for Norito RPC fixture tooling.
//!
//! The implementation lives in `norito_codegen_exporter`; these adapters keep
//! the workspace command dispatcher thin.
mod alias_setup_fixture;
use crate::JsonTarget;
use eyre::Result;
pub use norito_codegen_exporter::FixtureOptions;
use norito_codegen_exporter::{
    JsonOutput, generate_fixtures as generate_fixtures_impl, run_verify as run_verify_impl,
};
/// Verify canonical Norito RPC fixtures and optionally write a JSON report.
pub fn run_verify(json_out: Option<JsonTarget>) -> Result<()> {
    let alias_setup_fixture = alias_setup_fixture::render()?;
    run_verify_impl(&alias_setup_fixture, json_out.map(json_output))
}
/// Regenerate canonical Norito RPC fixtures using the focused exporter crate.
pub fn generate_fixtures(options: FixtureOptions) -> Result<()> {
    let alias_setup_fixture = alias_setup_fixture::render()?;
    generate_fixtures_impl(options, &alias_setup_fixture)
}
fn json_output(target: JsonTarget) -> JsonOutput {
    match target {
        JsonTarget::Stdout => JsonOutput::Stdout,
        JsonTarget::File(path) => JsonOutput::File(path),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
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
    fn fixture_generation_preserves_create_only_output_root_errors() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_root = temp_dir.path().join("output");
        fs::create_dir(&output_root).expect("create output root");
        let options = FixtureOptions::new(Some(output_root));
        let error = generate_fixtures(options).expect_err("existing output root must fail");
        assert!(
            error.to_string().contains("already exists"),
            "unexpected delegated error: {error}"
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
    #[test]
    fn alias_setup_owner_is_source_driven_and_has_no_identity_fallback() {
        let source = include_str!("norito_rpc/alias_setup_fixture.rs");
        let production = source
            .split_once("#[cfg(test)]")
            .map_or(source, |(production, _)| production);
        assert!(production.contains("account_onboarding_test_fixture::receipt_v1()"));
        assert!(production.contains("AliasSetupFixtureBytes::try_new"));
        for forbidden in [
            "include_str!",
            "include_bytes!",
            "fallback",
            ".or_else(",
            "unwrap_or_else",
            "ChainId",
            "\"chain\"",
            "\"chainId\"",
            "\"chain_id\"",
        ] {
            assert!(
                !production.contains(forbidden),
                "production alias fixture owner must not contain `{forbidden}`"
            );
        }
    }
}
