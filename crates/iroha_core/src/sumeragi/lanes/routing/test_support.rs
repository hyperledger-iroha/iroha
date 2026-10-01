//! Explicit immutable metadata fixtures; absence never implies global scope.

use super::SumeragiRootScope;
use crate::state::World;
use iroha_data_model::parameter::{
    Parameter,
    custom::CustomParameter,
    system::{
        ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata,
    },
};
use iroha_primitives::json::Json;

/// Valid genesis metadata carrying one explicit root scope.
pub(crate) fn metadata(scope: SumeragiRootScope) -> Parameter {
    let roster = crate::sumeragi::test_chain::fixture_validators()
        .into_iter()
        .map(
            |(validator, _)| iroha_data_model::block::consensus::ValidatorPower {
                validator,
                power: 1,
            },
        )
        .collect::<Vec<_>>();
    let mut context = crate::kagemusha_v1_test_fixtures::genesis_context_parameters();
    context.root_scope = scope;
    let metadata = ConsensusHandshakeMetadata {
        mode: SumeragiConsensusMode::Permissioned,
        block_cadence_ms: std::num::NonZeroU64::new(1_000).unwrap(),
        wire_protocol_version: u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION),
        consensus_fingerprint: ConsensusFingerprint::new([0xA5; 32]),
        kagemusha_mint_finality:
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
        sumeragi_context: context,
    };
    metadata.validate().unwrap();
    Parameter::Custom(CustomParameter::new(
        consensus_metadata::handshake_meta_id(),
        Json::from_norito_value_ref(&norito::json::value::to_value(&metadata).unwrap()).unwrap(),
    ))
}

/// Component World with explicit structural root metadata for the requested scope.
pub(crate) fn world(scope: SumeragiRootScope) -> World {
    let world = World::new();
    let mut parameters = world.parameters.block();
    parameters.set_parameter(metadata(scope));
    parameters.commit();
    world
}

/// Original signed genesis with a canonical committee and explicit root scope.
pub(crate) fn signed_genesis(scope: SumeragiRootScope) -> iroha_data_model::block::SignedBlock {
    iroha_genesis::init_instruction_registry();
    let Parameter::Custom(parameter) = metadata(scope) else {
        unreachable!()
    };
    let metadata = parameter
        .payload()
        .try_into_any::<ConsensusHandshakeMetadata>()
        .unwrap();
    iroha_genesis::GenesisBuilder::new_without_executor(
        iroha_model_base::chain::ChainId::from("root-scope-fixture"),
        ".",
    )
    .set_topology(
        crate::sumeragi::test_chain::fixture_validators()
            .into_iter()
            .map(|(peer, pop)| iroha_genesis::GenesisTopologyEntry::new(peer, pop))
            .collect::<Vec<_>>(),
    )
    .with_sumeragi_context_parameters(metadata.sumeragi_context)
    .with_kagemusha_mint_finality_genesis_parameters(metadata.kagemusha_mint_finality)
    .build_raw()
    .unwrap()
    .with_consensus_meta()
    .build_and_sign(&iroha_test_samples::ALICE_KEYPAIR)
    .unwrap()
    .0
}

/// Explicit closed native lane policy using the actual signed fixture's DA layout.
/// Component fixtures do not infer a post-genesis schedule from an empty World.
pub(crate) fn closed_native_lane_policy(scope: SumeragiRootScope) -> Parameter {
    let genesis = signed_genesis(scope);
    let epoch = crate::sumeragi::epoch::genesis_epoch(&genesis).unwrap();
    Parameter::Custom(
        iroha_data_model::sumeragi_lanes::SumeragiLanePolicy::for_chain(
            Default::default(),
            epoch.da_layout,
        )
        .into_custom_parameter(),
    )
}
