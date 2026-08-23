//! Kagemusha catalog loading and runtime-projection startup gates.

use std::time::Duration;

use iroha_config::parameters::actual::Root as Config;
use iroha_core::{
    smartcontracts::isi::offline::{
        KagemushaCatalogQualificationSealV1, KagemushaReleaseCatalogV4,
        KagemushaValidatorQualificationCatalogCaptureV1,
        VerifiedKagemushaV4RuntimeEffectiveConfigV1,
    },
    state::State,
};
use iroha_data_model::{
    block::consensus_v2::SnapshotV2BootstrapRecord,
    offline::KagemushaV4ValidatorQualificationSealV1,
};
use iroha_genesis::GenesisBlock;

use super::kagemusha_validator_qualification_command;

pub(super) fn load_configured_kagemusha_release_catalog(
    config: &Config,
) -> Result<KagemushaReleaseCatalogV4, String> {
    KagemushaReleaseCatalogV4::from_offline_config(&config.settlement.offline)
        .map_err(|error| format!("failed to authenticate Kagemusha V4 release catalog: {error}"))
}

#[allow(dead_code)]
fn load_and_build_configured_kagemusha_catalog_qualification_seal(
    config: &Config,
) -> Result<
    (
        KagemushaReleaseCatalogV4,
        KagemushaCatalogQualificationSealV1,
    ),
    String,
> {
    let capture = load_and_build_configured_kagemusha_validator_qualification_capture(config)?;
    let seal = capture.catalog_qualification_seal().clone();
    Ok((capture.into_catalog(), seal))
}

pub(super) fn load_and_build_configured_kagemusha_validator_qualification_capture(
    config: &Config,
) -> Result<KagemushaValidatorQualificationCatalogCaptureV1, String> {
    match (
        config
            .settlement
            .offline
            .kagemusha_release_policy_path
            .as_deref(),
        config.settlement.offline.kagemusha_artifact_dir.as_deref(),
    ) {
        (Some(policy_path), Some(artifact_dir)) => {
            KagemushaReleaseCatalogV4::load_and_build_validator_qualification_capture(
                policy_path,
                artifact_dir,
                config.settlement.offline.kagemusha_max_decoded_bytes,
            )
            .map_err(|error| {
                format!("failed to authenticate the complete Kagemusha V4 release catalog: {error}")
            })
        }
        (None, None) => Err(
            "cannot qualify a Kagemusha V4 catalog without a release policy and artifact directory"
                .to_owned(),
        ),
        _ => Err(
            "Kagemusha V4 release policy and artifact directory must be configured together"
                .to_owned(),
        ),
    }
}

pub(super) fn install_configured_kagemusha_release_catalog(
    state: &mut State,
    config: &Config,
) -> Result<(), String> {
    state.set_kagemusha_release_catalog(load_configured_kagemusha_release_catalog(config)?);
    Ok(())
}

/// Install the process-local digest required by staged or enabled lifecycle execution.
pub(super) fn install_runtime_effective_config(
    config: &Config,
    state: &State,
    authenticated_snapshot_bootstrap: Option<&SnapshotV2BootstrapRecord>,
    authenticated_block_cadence: Duration,
    effective_genesis: Option<&GenesisBlock>,
) -> Result<(), String> {
    if !state.kagemusha_release_catalog.is_configured() {
        return Ok(());
    }
    install_runtime_effective_config_with_validator_seal_reader(
        config,
        state,
        authenticated_snapshot_bootstrap,
        authenticated_block_cadence,
        effective_genesis,
        |config| {
            kagemusha_validator_qualification_command::read_configured_kagemusha_validator_qualification_seal(config)
                .map_err(|error| error.to_string())
        },
    )
}

pub(super) fn install_runtime_effective_config_with_validator_seal_reader(
    config: &Config,
    state: &State,
    authenticated_snapshot_bootstrap: Option<&SnapshotV2BootstrapRecord>,
    authenticated_block_cadence: Duration,
    effective_genesis: Option<&GenesisBlock>,
    read_configured_kagemusha_validator_qualification_seal: impl FnOnce(
        &Config,
    ) -> Result<KagemushaV4ValidatorQualificationSealV1, String>,
) -> Result<(), String> {
    let runtime_effective_config = if let Some(bootstrap) = authenticated_snapshot_bootstrap {
        let Some(_) = config
            .settlement
            .offline
            .kagemusha_validator_qualification_seal_path
            .as_ref()
        else {
            iroha_logger::warn!(
                "authenticated-snapshot Kagemusha startup has no configured local validator qualification seal; staging and active-lifecycle output remain fail-closed"
            );
            return Ok(());
        };
        let seal = read_configured_kagemusha_validator_qualification_seal(config)?;
        if seal.body.validator_id != config.common.peer.id {
            return Err(
                "configured Kagemusha validator qualification seal belongs to a different local peer"
                    .to_owned(),
            );
        }
        let derived =
            VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive_from_authenticated_snapshot(
                config,
                bootstrap,
                authenticated_block_cadence,
                seal.body.runtime_effective_config.genesis_context,
            )
            .map_err(|error| {
                format!("failed to derive the local Kagemusha runtime-effective config: {error}")
            })?;
        if derived.projection() != &seal.body.runtime_effective_config {
            return Err(
                "effective snapshot runtime differs from the configured Kagemusha validator qualification seal"
                    .to_owned(),
            );
        }
        derived
    } else {
        VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive_from_signed_genesis(
            config,
            effective_genesis.ok_or_else(|| {
                "normal Kagemusha startup has no signed genesis metadata".to_owned()
            })?,
        )
        .map_err(|error| {
            format!("failed to derive the local Kagemusha runtime-effective config: {error}")
        })?
    };
    let digest = runtime_effective_config
        .projection()
        .consensus_sha256()
        .map_err(|error| {
            format!("failed to commit the local Kagemusha runtime-effective config: {error}")
        })?;
    state.install_kagemusha_runtime_effective_config_sha256(digest)
}
