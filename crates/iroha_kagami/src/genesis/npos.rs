//! Helpers for NPoS-specific genesis validation.

use color_eyre::eyre::{Result, eyre};
use iroha_data_model::parameter::system::SumeragiNposParameters;
use iroha_genesis::RawGenesisTransaction;

/// Check that the provided genesis manifest carries `sumeragi_npos_parameters`.
///
/// # Errors
///
/// Returns an error if the NPoS parameter is missing. Callers should surface
/// this early so operators regenerate genesis with `--consensus-mode npos`.
pub fn ensure_npos_parameters(manifest: &RawGenesisTransaction) -> Result<()> {
    if has_npos_parameters(manifest) {
        Ok(())
    } else {
        Err(eyre!(
            "NPoS consensus requires `sumeragi_npos_parameters` in genesis; regenerate with `kagami genesis generate --consensus-mode npos` or embed the parameter manually"
        ))
    }
}

/// Determine whether `sumeragi_npos_parameters` is present in the manifest.
pub fn has_npos_parameters(manifest: &RawGenesisTransaction) -> bool {
    let params = manifest.effective_parameters();
    let npos_param_id = SumeragiNposParameters::parameter_id();
    params
        .custom()
        .get(&npos_param_id)
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .is_some()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use iroha_data_model::{
        isi::Grant,
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiParameter},
        },
        prelude::*,
    };
    use iroha_genesis::{GenesisBuilder, RawGenesisTransaction};
    use iroha_test_samples::ALICE_ID;
    use tempfile::NamedTempFile;

    use super::*;

    fn manifest_with_params(
        params: Parameter,
        consensus_mode: SumeragiConsensusMode,
    ) -> RawGenesisTransaction {
        let builder =
            GenesisBuilder::new_without_executor(ChainId::from("npos-genesis"), PathBuf::from("."));
        builder
            .append_parameter(params)
            .build_raw()
            .with_consensus_mode(consensus_mode)
    }

    #[test]
    fn detects_present_npos_parameters() {
        let manifest = roundtrip_manifest(&manifest_with_params(
            Parameter::Custom(SumeragiNposParameters::default().into_custom_parameter()),
            SumeragiConsensusMode::Npos,
        ));
        assert!(has_npos_parameters(&manifest));
        assert!(ensure_npos_parameters(&manifest).is_ok());
    }

    #[test]
    fn rejects_manifest_without_npos_parameters() {
        let manifest = roundtrip_manifest(&manifest_with_params(
            Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Permissioned,
            )),
            SumeragiConsensusMode::Permissioned,
        ));
        assert!(!has_npos_parameters(&manifest));
        let err = ensure_npos_parameters(&manifest).expect_err("missing params should fail");
        assert!(
            err.to_string().contains("sumeragi_npos_parameters"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn ignores_unrelated_set_parameter() {
        let mut builder =
            GenesisBuilder::new_without_executor(ChainId::from("npos-genesis"), PathBuf::from("."));
        let grant = Grant::account_permission(
            iroha_executor_data_model::permission::parameter::CanSetParameters,
            ALICE_ID.clone(),
        );
        builder = builder.append_instruction(grant);
        let manifest = roundtrip_manifest(
            &builder
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Permissioned),
        );
        assert!(!has_npos_parameters(&manifest));
    }

    fn roundtrip_manifest(manifest: &RawGenesisTransaction) -> RawGenesisTransaction {
        let tmp = NamedTempFile::new().expect("create temp file");
        let json = norito::json::to_json_pretty(&manifest).expect("serialize manifest");
        fs::write(tmp.path(), json).expect("write manifest");
        RawGenesisTransaction::from_path(tmp.path()).expect("re-read manifest through parser")
    }
}
