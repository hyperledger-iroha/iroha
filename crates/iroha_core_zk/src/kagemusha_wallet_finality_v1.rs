//! Bind the compact history anchor to an independently selected native genesis.
//!
//! The native verifier owns signature and original roster/PoP authentication.
//! This adapter reads only that owner's immutable signed-root values, requires
//! global scope and preserves the exact native initial lag-two schedule. The
//! wallet installation must independently pin the complete returned anchor digest
//! and original source catalog. The returned data is not a Load capability.

use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    sumeragi_finality::{
        ConsensusSchedule, GenesisReadError, ScheduleError, ScheduledSlot, SumeragiFinalityVerifier,
    },
};
use iroha_kagemusha_proof::finality::history::HistoryAnchor;

mod input;
pub use input::{FinalityInputError, block_witness, load_witness};

mod output;
pub use output::{FinalityRetentionError, retain_load_finality};

/// Server-only authenticated proof production and immutable recovery custody.
pub mod server;

/// Failure to derive the exact global signed-genesis policy.
#[derive(Debug, thiserror::Error)]
pub enum HistoryAnchorError {
    /// A valid private root cannot supply global ordinary Load authority.
    #[error("ordinary Load history requires independently selected global genesis")]
    Scope,
    /// Original signed metadata or explicit chain-parameter extraction failed.
    #[error(transparent)]
    Genesis(#[from] GenesisReadError),
    /// The original native epoch or initial schedule is invalid.
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
    /// The initial native Ready slots do not equal the selected signed policy.
    #[error("signed genesis does not yield the exact Ready slots at heights two and three")]
    InitialSlots,
}

/// Derive every fixed anchor field from the retained authenticated signed root.
///
/// The verifier's constructor authenticates the original signatures and exact
/// ordered BLS proofs of possession; its caller must independently select that
/// genesis and chain label. Neither decoded result-only genesis data, current
/// World configuration nor caller-supplied default parameters enter this adapter.
/// The installation owner must pin the whole anchor digest and source catalog
/// before qualifying a history source; computing this value grants no authority.
///
/// # Errors
/// Refuses a private root, absent or repeated signed parameters, invalid native
/// geometry, context encoding failure, or changed initial Ready slots. Original
/// metadata decoder errors remain typed inside [`HistoryAnchorError::Genesis`].
pub fn derive_history_anchor(
    verifier: &SumeragiFinalityVerifier,
) -> Result<HistoryAnchor, HistoryAnchorError> {
    if verifier.root_scope()? != SumeragiRootScope::Global {
        return Err(HistoryAnchorError::Scope);
    }
    let epoch = verifier.initial_epoch();
    let parameters = verifier.initial_chain_parameters()?;
    let schedule = ConsensusSchedule::from_genesis(epoch.clone(), parameters)?;
    for height in [2, 3] {
        if !matches!(schedule.get(height), Some(ScheduledSlot::Ready(slot))
            if slot.height == height && slot.epoch == *epoch && slot.params == parameters)
        {
            return Err(HistoryAnchorError::InitialSlots);
        }
    }
    Ok(HistoryAnchor {
        network: *epoch.network_id.as_bytes(),
        instance: verifier.instance().0,
        initial_context: epoch.context_id().map_err(ScheduleError::Epoch)?,
        initial_epoch: epoch.authorization.epoch,
        parameters: [
            parameters.block_time_ms,
            parameters.payload_retry_interval_ms,
            parameters.exec_budget_ms,
            parameters.apply_budget_ms,
            u64::from(parameters.max_block_bytes),
            parameters.epoch_length_blocks,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        parameter::system::SumeragiParameters,
        sumeragi_finality::{
            ChainParamsRecord, genesis_epoch, test_fixtures::NativeFinalityFixture,
        },
    };

    #[test]
    fn global_anchor_uses_original_signed_genesis_and_six_parameters() {
        let fixture = NativeFinalityFixture::new_with_explicit_parameters();
        let verifier = fixture.verifier();
        let anchor = derive_history_anchor(&verifier).unwrap();
        let epoch = genesis_epoch(fixture.genesis()).unwrap();
        assert_eq!(anchor.network, *fixture.network_id().as_bytes());
        assert_eq!(anchor.instance, verifier.instance().0);
        assert_eq!(anchor.initial_context, epoch.context_id().unwrap());
        assert_eq!(anchor.initial_epoch, 0);
        let p = ChainParamsRecord::from_parameters(&SumeragiParameters::default());
        assert_eq!(
            anchor.parameters,
            [
                p.block_time_ms,
                p.payload_retry_interval_ms,
                p.exec_budget_ms,
                p.apply_budget_ms,
                u64::from(p.max_block_bytes),
                p.epoch_length_blocks,
            ]
        );
        assert_eq!(anchor, derive_history_anchor(&verifier).unwrap());
    }

    #[test]
    fn valid_private_genesis_cannot_supply_a_global_anchor() {
        let global = NativeFinalityFixture::new_with_explicit_parameters();
        let private = NativeFinalityFixture::start_with_scope(
            global.chain_id(),
            SumeragiRootScope::Dataspace {
                parent_network_id: global.network_id(),
                dataspace_id: 9_u64.into(),
            },
        );
        assert!(matches!(
            derive_history_anchor(&private.verifier()),
            Err(HistoryAnchorError::Scope)
        ));
    }

    #[test]
    fn authenticated_genesis_with_omitted_parameters_cannot_inherit_defaults() {
        let fixture = NativeFinalityFixture::new();
        assert!(matches!(
            derive_history_anchor(&fixture.verifier()),
            Err(HistoryAnchorError::Genesis(_))
        ));
    }

    #[test]
    fn captured_receipt_anchor_is_derived_from_its_original_signed_genesis() {
        use iroha_data_model::{
            block::decode_framed_signed_block, sumeragi_finality::FinalityValidator,
        };

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
        let capture: norito::json::Value =
            norito::json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let wire = hex::decode(capture["signed_genesis_wire_hex"].as_str().unwrap()).unwrap();
        let genesis = decode_framed_signed_block(&wire).unwrap();
        let epoch = genesis_epoch(&genesis).unwrap();
        let roster = epoch
            .committee
            .iter()
            .map(|member| FinalityValidator {
                public_key: member.validator.public_key().clone(),
                proof_of_possession: member.proof_of_possession.clone(),
            })
            .collect();
        let verifier =
            SumeragiFinalityVerifier::new(&genesis, capture["chain_id"].as_str().unwrap(), roster)
                .unwrap();
        let anchor = derive_history_anchor(&verifier).unwrap();
        let expected = &capture["history_anchor"];
        for (name, actual) in [
            ("network_hex", anchor.network),
            ("instance_hex", anchor.instance),
            ("initial_context_hex", anchor.initial_context),
        ] {
            assert_eq!(
                hex::encode(actual),
                expected[name].as_str().unwrap(),
                "{name}"
            );
        }
        assert_eq!(
            anchor.initial_epoch,
            expected["initial_epoch"].as_u64().unwrap()
        );
        let parameters: Vec<_> = expected["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap())
            .collect();
        assert_eq!(anchor.parameters.as_slice(), parameters.as_slice());
        assert_eq!(anchor.network, *epoch.network_id.as_bytes());
        assert_eq!(anchor.initial_context, epoch.context_id().unwrap());
    }
}
