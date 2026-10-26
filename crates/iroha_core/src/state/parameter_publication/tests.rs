//! Actual transaction publication: source identity, two outputs and rollback.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    test_allocations::{allocations_during, refuse_one_layout_during},
};
use iroha_data_model::{block::BlockHeader, parameter::SumeragiParameter};
use mv::storage::StorageReadOnly;
fn state() -> State {
    State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}
fn header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, 8_000, 0)
}
fn change(value: u64) -> Parameter {
    Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(value))
}

#[test]
fn installation_moves_exact_graph_and_both_events_without_allocation() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let before = tx.world.parameters.get().clone();
    let prepared = tx
        .prepare_parameter_change(&change(17), None, None)
        .unwrap();
    assert!(before.same_version(tx.world.parameters.get()));
    assert!(tx.world.external_event_buf.is_empty() && tx.world.internal_event_buf.is_empty());
    assert_eq!(
        allocations_during(|| tx.world.install_parameter_change(prepared).unwrap()),
        0
    );
    assert_eq!(tx.world.parameters.get().sumeragi.max_clock_drift_ms, 17);
    assert_eq!(tx.world.external_event_buf.len(), 1);
    assert_eq!(tx.world.internal_event_buf.len(), 1);
    assert!(
        tx.world.internal_event_buf[0]
            .belongs_to(tx.world.sccp_committee_generations.allocation_budget())
    );
    assert_eq!(
        tx.world
            .parameters
            .get_before_block()
            .sumeragi
            .max_clock_drift_ms,
        before.sumeragi.max_clock_drift_ms
    );
    drop(tx);
    assert!(before.same_version(block.world.parameters.get()));
}
#[test]
fn applied_sibling_invalidates_prepared_version_without_touching_outputs() {
    let state = state();
    let mut block = state.block(header());
    let stale = {
        let tx = block.transaction();
        tx.prepare_parameter_change(&change(17), None, None)
            .unwrap()
    };
    let mut accepted = block.transaction();
    let prepared = accepted
        .prepare_parameter_change(&change(19), None, None)
        .unwrap();
    accepted.world.install_parameter_change(prepared).unwrap();
    accepted.apply();
    let mut next = block.transaction();
    let original = next.world.parameters.get().clone();
    let external = next.world.external_event_buf.len();
    let internal = next.world.internal_event_buf.len();
    assert!(matches!(
        next.world.install_parameter_change(stale),
        Err(ParameterChangeError::Ownership(
            ParameterRetentionError::StalePredecessor
        ))
    ));
    assert!(original.same_version(next.world.parameters.get()));
    assert_eq!(
        (
            next.world.external_event_buf.len(),
            next.world.internal_event_buf.len()
        ),
        (external, internal)
    );
}
#[test]
fn both_output_reservations_precede_any_parameter_or_event_mutation() {
    for layout in [
        Layout::array::<EventBox>(1).unwrap(),
        Layout::array::<SharedDataEvent>(1).unwrap(),
    ] {
        let state = state();
        let mut block = state.block(header());
        let tx = block.transaction();
        assert_eq!(tx.world.external_event_buf.capacity(), 0);
        assert_eq!(tx.world.internal_event_buf.capacity(), 0);
        let before = tx.world.parameters.get().clone();
        let pool = tx.world.sccp_committee_generations.allocation_budget();
        let held = pool.reserved_bytes();
        let (result, observed) = refuse_one_layout_during(layout, || {
            tx.prepare_parameter_change(&change(23), None, None)
        });
        assert!(observed);
        let ParameterChangeError::Deferred(original) = result.err().unwrap() else {
            panic!("original output allocator refusal required")
        };
        assert_eq!(
            original
                .allocator_refusal()
                .unwrap()
                .vec_reserve_error()
                .unwrap()
                .1,
            layout
        );
        assert!(before.same_version(tx.world.parameters.get()));
        assert!(tx.world.external_event_buf.is_empty() && tx.world.internal_event_buf.is_empty());
        assert_eq!(pool.reserved_bytes(), held);
        drop(
            tx.prepare_parameter_change(&change(23), None, None)
                .unwrap(),
        );
        assert_eq!(pool.reserved_bytes(), held);
    }
}
#[test]
fn source_event_geometry_change_refuses_before_first_parameter_write() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let before = tx.world.parameters.get().clone();
    let prepared = tx
        .prepare_parameter_change(&change(23), None, None)
        .unwrap();
    // A different source event was appended after preparation. The graph is
    // still the same, so output geometry must independently reject this plan.
    tx.world.internal_event_buf.push(prepared.event.clone());
    assert!(tx.world.install_parameter_change(prepared).is_err());
    assert!(before.same_version(tx.world.parameters.get()));
    assert_eq!(tx.world.internal_event_buf.len(), 1);
    assert_eq!(tx.world.external_event_buf.len(), 0);
}

fn zk_instruction(
    policy: iroha_data_model::zk_validity_policy::ZkValidityPolicyV1,
) -> iroha_data_model::isi::SetParameter {
    iroha_data_model::isi::SetParameter::new(Parameter::Custom(policy.into_custom_parameter()))
}

#[test]
fn actual_set_parameter_keeps_original_work_and_allocation_refusal_unpublished_and_retries() {
    use crate::smartcontracts::Execute;
    use iroha_data_model::zk_validity_policy::ZkValidityPolicyV1;
    use iroha_test_samples::ALICE_ID;
    for limits in [
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX),
    ] {
        let state = state();
        let mut block = state.block(header());
        let policy = ZkValidityPolicyV1::protocol_default();
        let instruction = zk_instruction(policy.clone());
        let retry_instruction = instruction.clone();
        {
            let mut tx = block.transaction();
            let predecessor = tx.world.parameters.get().clone();
            let pool = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let held = pool.reserved_bytes();
            norito::with_decode_limits_scope(limits, || instruction.execute(&ALICE_ID, &mut tx))
                .expect_err("real SetParameter cannot publish a refused original decode");
            assert!(tx.is_execution_deferred());
            assert!(predecessor.same_version(tx.world.parameters.get()));
            assert!(
                tx.world.internal_event_buf.is_empty() && tx.world.external_event_buf.is_empty()
            );
            assert_eq!(pool.reserved_bytes(), held);
            let owner = tx
                .take_execution_deferral()
                .expect("State retained the actual refusal");
            let original = owner
                .decode_refusal()
                .expect("original decoder owner")
                .clone();
            assert_eq!(
                original.kind(),
                norito::core::DecodeLocalRefusalKind::EnclosingLimit
            );
            let transported = crate::execution_attempt::ExecutionDeferred::try_from_vm_error(
                owner.into_vm_error(),
            )
            .unwrap();
            assert_eq!(transported.decode_refusal(), Some(&original));
        }
        let mut retry = block.transaction();
        retry_instruction.execute(&ALICE_ID, &mut retry).unwrap();
        assert!(!retry.is_execution_deferred());
        assert_eq!(retry.world.parameters.get().zk_policy(), &policy);
        assert_eq!(
            (
                retry.world.internal_event_buf.len(),
                retry.world.external_event_buf.len()
            ),
            (1, 1)
        );
    }
}

#[test]
fn actual_set_parameter_rejects_intrinsic_and_pending_zk_failures_without_publication() {
    use crate::smartcontracts::Execute;
    use iroha_data_model::{
        isi::error::{InstructionExecutionError, InvalidParameterError},
        zk_validity_policy::ZkValidityPolicyV1,
    };
    use iroha_test_samples::ALICE_ID;
    let mut state = state();
    // Startup rebuilds this derived index; seed its component fixture afterward.
    state
        .world
        .confidential_policy_transition_counts
        .insert(41, 3);
    let mut block = state.block(header());
    for intrinsic in [true, false] {
        let mut policy = ZkValidityPolicyV1::protocol_default();
        if intrinsic {
            policy.sccp.max_proofs_per_transaction = 0;
        } else {
            policy.confidential.policy_transition_max_per_height = 2;
        }
        let mut tx = block.transaction();
        let predecessor = tx.world.parameters.get().clone();
        let error = zk_instruction(policy)
            .execute(&ALICE_ID, &mut tx)
            .unwrap_err();
        assert!(
            !tx.is_execution_deferred(),
            "intrinsic and retained-count failures are deterministic"
        );
        if !intrinsic {
            let InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message),
            ) = &error
            else {
                panic!("expected deterministic pending-transition rejection, got {error:?}")
            };
            assert!(
                message.contains("committed confidential-policy transitions at height 41 (3)"),
                "expected original pending-transition detail, got {error:?}"
            );
        }
        assert!(predecessor.same_version(tx.world.parameters.get()));
        assert!(tx.world.internal_event_buf.is_empty() && tx.world.external_event_buf.is_empty());
    }
    let mut policy = ZkValidityPolicyV1::protocol_default();
    policy.confidential.policy_transition_max_per_height = 3;
    let mut tx = block.transaction();
    zk_instruction(policy.clone())
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    assert_eq!(tx.world.parameters.get().zk_policy(), &policy);
    assert_eq!(
        tx.world.confidential_policy_transition_counts.get(&41),
        Some(&3)
    );
}

#[test]
fn actual_genesis_handshake_refusal_preserves_cadence_and_metadata_together() {
    use crate::smartcontracts::Execute;
    use iroha_data_model::{
        block::consensus::SumeragiGenesisContextParameters,
        parameter::{
            CustomParameter,
            system::{ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode},
        },
    };
    use iroha_test_samples::ALICE_ID;
    // Direct component instruction coverage; a header does not grant signed
    // genesis/finality authority. Authentication is covered by the source owner.
    let metadata = ConsensusHandshakeMetadata {
        mode: SumeragiConsensusMode::Npos,
        block_cadence_ms: NonZeroU64::new(1024).unwrap(),
        wire_protocol_version: u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION),
        consensus_fingerprint: ConsensusFingerprint::new([0xab; 32]),
        sumeragi_context: SumeragiGenesisContextParameters::recommended(),
    };
    let parameter = Parameter::Custom(CustomParameter::new(
        "consensus_handshake_meta".parse().unwrap(),
        iroha_primitives::json::Json::new(metadata),
    ));
    let state = state();
    let mut block = state.block(BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        0,
        0,
    ));
    let instruction = iroha_data_model::isi::SetParameter::new(parameter.clone());
    {
        let mut tx = block.transaction();
        let original = tx.world.parameters.get().clone();
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX);
        norito::with_decode_limits_scope(limits, || instruction.execute(&ALICE_ID, &mut tx))
            .unwrap_err();
        assert!(tx.is_execution_deferred());
        assert!(
            tx.take_execution_deferral()
                .unwrap()
                .decode_refusal()
                .is_some()
        );
        assert!(original.same_version(tx.world.parameters.get()));
        assert!(tx.world.internal_event_buf.is_empty() && tx.world.external_event_buf.is_empty());
    }
    let mut retry = block.transaction();
    iroha_data_model::isi::SetParameter::new(parameter)
        .execute(&ALICE_ID, &mut retry)
        .unwrap();
    assert_eq!(
        retry.world.parameters.get().sumeragi().block_cadence_ms,
        metadata.block_cadence_ms
    );
    let id = iroha_data_model::parameter::system::consensus_metadata::handshake_meta_id();
    assert_eq!(
        retry
            .world
            .parameters
            .get()
            .custom()
            .get(&id)
            .unwrap()
            .payload(),
        &iroha_primitives::json::Json::new(metadata)
    );
    assert_eq!(
        (
            retry.world.internal_event_buf.len(),
            retry.world.external_event_buf.len()
        ),
        (1, 1)
    );
}

#[test]
fn actual_npos_refusal_keeps_original_policy_and_allows_same_source_retry() {
    use crate::smartcontracts::Execute;
    use iroha_data_model::parameter::system::SumeragiNposParameters;
    use iroha_test_samples::ALICE_ID;
    let policy = SumeragiNposParameters::default();
    let parameter = Parameter::Custom(policy.clone().into_custom_parameter());
    let mut world = World::default();
    world.set_parameter_for_testing(parameter.clone()).unwrap();
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(header());
    let instruction = iroha_data_model::isi::SetParameter::new(parameter.clone());
    {
        let mut tx = block.transaction();
        let original = tx.world.parameters.get().clone();
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX);
        norito::with_decode_limits_scope(limits, || instruction.execute(&ALICE_ID, &mut tx))
            .unwrap_err();
        assert!(tx.is_execution_deferred());
        assert!(
            tx.take_execution_deferral()
                .unwrap()
                .decode_refusal()
                .is_some()
        );
        assert!(original.same_version(tx.world.parameters.get()));
        assert!(tx.world.internal_event_buf.is_empty() && tx.world.external_event_buf.is_empty());
    }
    let mut retry = block.transaction();
    iroha_data_model::isi::SetParameter::new(parameter)
        .execute(&ALICE_ID, &mut retry)
        .unwrap();
    let id = SumeragiNposParameters::parameter_id();
    assert_eq!(
        retry
            .world
            .parameters
            .get()
            .custom()
            .get(&id)
            .unwrap()
            .payload(),
        policy.into_custom_parameter().payload()
    );
    assert_eq!(
        (
            retry.world.internal_event_buf.len(),
            retry.world.external_event_buf.len()
        ),
        (1, 1)
    );
}

fn assert_actual_policy_rejection(
    offered: Parameter,
    installed: Option<Parameter>,
    genesis: bool,
    expected: String,
) {
    use crate::smartcontracts::Execute;
    use iroha_data_model::isi::error::{InstructionExecutionError, InvalidParameterError};
    use iroha_test_samples::ALICE_ID;

    let state = state();
    let block_header = if genesis {
        BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0)
    } else {
        header()
    };
    let mut block = state.block(block_header);
    let mut tx = block.transaction();
    if let Some(installed) = installed {
        // Install malformed component input after startup's independent validation.
        tx.world.set_parameter_for_testing(installed).unwrap();
    }
    let predecessor = tx.world.parameters.get().clone();
    let error = iroha_data_model::isi::SetParameter::new(offered)
        .execute(&ALICE_ID, &mut tx)
        .unwrap_err();
    let InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) =
        error
    else {
        panic!("expected original deterministic parameter rejection")
    };
    assert_eq!(message, expected);
    assert!(!tx.is_execution_deferred());
    assert!(predecessor.same_version(tx.world.parameters.get()));
    assert!(tx.world.internal_event_buf.is_empty() && tx.world.external_event_buf.is_empty());
}

#[test]
fn actual_handshake_rejections_keep_payload_and_semantic_context_after_classification() {
    use iroha_data_model::{
        block::consensus::SumeragiGenesisContextParameters,
        parameter::{
            CustomParameter,
            system::{ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode},
        },
    };
    use iroha_primitives::json::Json;

    let payload = Json::from_str_norito("{}").unwrap();
    let old_payload_error = payload
        .try_into_any::<ConsensusHandshakeMetadata>()
        .unwrap_err();
    assert_actual_policy_rejection(
        Parameter::Custom(CustomParameter::new(
            "consensus_handshake_meta".parse().unwrap(),
            payload,
        )),
        None,
        true,
        format!("invalid signed consensus handshake metadata payload: {old_payload_error}"),
    );
    let metadata = ConsensusHandshakeMetadata {
        mode: SumeragiConsensusMode::Npos,
        block_cadence_ms: NonZeroU64::new(1024).unwrap(),
        wire_protocol_version: 0,
        consensus_fingerprint: ConsensusFingerprint::new([0xab; 32]),
        sumeragi_context: SumeragiGenesisContextParameters::recommended(),
    };
    let old_semantic_error = metadata.validate().unwrap_err();
    assert_actual_policy_rejection(
        Parameter::Custom(CustomParameter::new(
            "consensus_handshake_meta".parse().unwrap(),
            Json::new(metadata),
        )),
        None,
        true,
        format!("invalid signed consensus handshake metadata: {old_semantic_error}"),
    );
}

#[test]
fn actual_npos_rejections_keep_incoming_installed_and_zero_context_after_classification() {
    use iroha_data_model::parameter::{CustomParameter, system::SumeragiNposParameters};
    use iroha_primitives::json::Json;

    let payload = Json::from_str_norito("{}").unwrap();
    let value = norito::json::from_str::<norito::json::Value>(payload.get()).unwrap();
    let old_incoming_error =
        norito::json::value::from_value::<SumeragiNposParameters>(value).unwrap_err();
    let malformed = CustomParameter::new(SumeragiNposParameters::parameter_id(), payload);
    assert_actual_policy_rejection(
        Parameter::Custom(malformed.clone()),
        None,
        false,
        format!("invalid signed NPoS parameter payload: {old_incoming_error}"),
    );
    let old_installed_error =
        SumeragiNposParameters::from_custom_parameter(&malformed).unwrap_err();
    assert_actual_policy_rejection(
        Parameter::Custom(SumeragiNposParameters::default().into_custom_parameter()),
        Some(Parameter::Custom(malformed)),
        false,
        old_installed_error.to_string(),
    );
    let mut zero = SumeragiNposParameters::default();
    zero.finality_margin_blocks = 0;
    assert_actual_policy_rejection(
        Parameter::Custom(zero.into_custom_parameter()),
        None,
        false,
        "SumeragiNposParameters.reconfig.finality_margin_blocks must be greater than zero"
            .to_owned(),
    );
}

#[test]
fn native_fx_suffix_reserves_final_event_without_relaxing_other_callers() {
    use iroha_data_model::isi::settlement::FxCorridorPolicyRegistry;
    for count in [6, 8, 10] {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        let next = Parameter::Custom(FxCorridorPolicyRegistry::default().into_custom_parameter());
        let prepared = tx.prepare_fx_usage_parameter_change(&next).unwrap();
        // Exercise the output geometry independently of the native asset producer.
        for _ in 0..count {
            tx.world
                .external_event_buf
                .push(EventBox::Data(prepared.event.clone()));
            tx.world.internal_event_buf.push(prepared.event.clone());
        }
        assert_eq!(
            allocations_during(|| tx.world.install_parameter_change(prepared).unwrap()),
            0
        );
        assert_eq!(tx.world.external_event_buf.len(), count + 1);
        assert_eq!(tx.world.internal_event_buf.len(), count + 1);
    }
    let state = state();
    let mut block = state.block(header());
    let tx = block.transaction();
    assert!(matches!(
        tx.prepare_fx_usage_parameter_change(&change(7)),
        Err(ParameterChangeError::Invalid(_))
    ));
}

#[test]
fn native_fx_suffix_refuses_incomplete_excessive_or_unpaired_outputs_before_write() {
    use iroha_data_model::isi::settlement::FxCorridorPolicyRegistry;
    for (external, internal) in [(5, 5), (11, 11), (6, 7)] {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        let before = tx.world.parameters.get().clone();
        let next = Parameter::Custom(FxCorridorPolicyRegistry::default().into_custom_parameter());
        let prepared = tx.prepare_fx_usage_parameter_change(&next).unwrap();
        for _ in 0..external {
            tx.world
                .external_event_buf
                .push(EventBox::Data(prepared.event.clone()));
        }
        for _ in 0..internal {
            tx.world.internal_event_buf.push(prepared.event.clone());
        }
        assert!(matches!(
            tx.world.install_parameter_change(prepared),
            Err(ParameterChangeError::Ownership(
                ParameterRetentionError::StalePredecessor
            ))
        ));
        assert!(before.same_version(tx.world.parameters.get()));
        assert_eq!(tx.world.external_event_buf.len(), external);
        assert_eq!(tx.world.internal_event_buf.len(), internal);
    }
}

// Deliberate output edits below test the actual parameter publisher's negative
// guards. They do not stand in for authenticated FX transfer success coverage.
#[test]
fn immediate_install_rejects_balanced_intervening_events_without_mutation() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    tx.world.external_event_buf = Vec::with_capacity(32);
    tx.world.internal_event_buf = Vec::with_capacity(32);
    let original = tx.world.parameters.get().clone();
    let prepared = tx
        .prepare_parameter_change(&change(31), None, None)
        .unwrap();
    tx.world
        .external_event_buf
        .push(EventBox::Data(prepared.event.clone()));
    tx.world.internal_event_buf.push(prepared.event.clone());
    let external = tx.world.external_event_buf.clone();
    let internal = tx.world.internal_event_buf.clone();
    assert!(tx.world.external_event_buf.capacity() > external.len());
    assert!(tx.world.internal_event_buf.capacity() > internal.len());
    assert!(matches!(
        tx.world.install_parameter_change(prepared),
        Err(ParameterChangeError::Ownership(
            ParameterRetentionError::StalePredecessor
        ))
    ));
    assert!(original.same_version(tx.world.parameters.get()));
    assert_eq!(tx.world.external_event_buf, external);
    assert_eq!(tx.world.internal_event_buf, internal);
    drop(tx);
    assert!(original.same_version(block.world.parameters.get()));
}

#[test]
fn native_fx_install_rejects_invalid_event_geometry_without_mutation() {
    use iroha_data_model::isi::settlement::FxCorridorPolicyRegistry;
    for (external_count, internal_count) in [(0, 0), (5, 5), (11, 11), (6, 7), (7, 6)] {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        // Keep all tested counts below both real capacities. In particular the
        // eleven-event case must reach the sequence guard, not fail for space.
        tx.world.external_event_buf = Vec::with_capacity(32);
        tx.world.internal_event_buf = Vec::with_capacity(32);
        let original = tx.world.parameters.get().clone();
        let next = Parameter::Custom(FxCorridorPolicyRegistry::default().into_custom_parameter());
        let prepared = tx.prepare_fx_usage_parameter_change(&next).unwrap();
        for _ in 0..external_count {
            tx.world
                .external_event_buf
                .push(EventBox::Data(prepared.event.clone()));
        }
        for _ in 0..internal_count {
            tx.world.internal_event_buf.push(prepared.event.clone());
        }
        let external = tx.world.external_event_buf.clone();
        let internal = tx.world.internal_event_buf.clone();
        assert_eq!(external.len(), external_count);
        assert_eq!(internal.len(), internal_count);
        assert!(tx.world.external_event_buf.capacity() > external.len());
        assert!(tx.world.internal_event_buf.capacity() > internal.len());
        assert!(matches!(
            tx.world.install_parameter_change(prepared),
            Err(ParameterChangeError::Ownership(
                ParameterRetentionError::StalePredecessor
            ))
        ));
        assert!(original.same_version(tx.world.parameters.get()));
        assert_eq!(tx.world.external_event_buf, external);
        assert_eq!(tx.world.internal_event_buf, internal);
        drop(tx);
        assert!(original.same_version(block.world.parameters.get()));
    }
}

#[test]
fn native_fx_install_keeps_original_parameter_version_guard() {
    use iroha_data_model::isi::settlement::FxCorridorPolicyRegistry;
    let state = state();
    let mut block = state.block(header());
    let (stale, predecessor) = {
        let tx = block.transaction();
        assert!(tx.world.external_event_buf.is_empty() && tx.world.internal_event_buf.is_empty());
        let next = Parameter::Custom(FxCorridorPolicyRegistry::default().into_custom_parameter());
        (
            tx.prepare_fx_usage_parameter_change(&next).unwrap(),
            tx.world.parameters.get().clone(),
        )
    };
    let mut sibling = block.transaction();
    let accepted = sibling
        .prepare_parameter_change(&change(37), None, None)
        .unwrap();
    sibling.world.install_parameter_change(accepted).unwrap();
    sibling.apply();
    let mut tx = block.transaction();
    let original = tx.world.parameters.get().clone();
    assert!(!predecessor.same_version(&original));
    assert_eq!(original.sumeragi.max_clock_drift_ms, 37);
    assert!(tx.world.external_event_buf.is_empty() && tx.world.internal_event_buf.is_empty());
    // A permitted count cannot authorize a successor of the stale graph.
    for _ in 0..6 {
        tx.world
            .external_event_buf
            .push(EventBox::Data(stale.event.clone()));
        tx.world.internal_event_buf.push(stale.event.clone());
    }
    let external = tx.world.external_event_buf.clone();
    let internal = tx.world.internal_event_buf.clone();
    assert_eq!((external.len(), internal.len()), (6, 6));
    assert!(matches!(
        tx.world.install_parameter_change(stale),
        Err(ParameterChangeError::Ownership(
            ParameterRetentionError::StalePredecessor
        ))
    ));
    assert!(original.same_version(tx.world.parameters.get()));
    assert_eq!(tx.world.external_event_buf, external);
    assert_eq!(tx.world.internal_event_buf, internal);
    drop(tx);
    assert!(original.same_version(block.world.parameters.get()));
}

#[test]
fn native_fx_install_rejects_foreign_original_pool_without_mutation() {
    use iroha_data_model::isi::settlement::FxCorridorPolicyRegistry;
    let source = state();
    let target = state();
    let mut source_block = source.block(header());
    let mut target_block = target.block(header());
    let source_original = source_block.world.parameters.get().clone();
    let source_pool = source_block
        .world
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    let foreign = {
        let tx = source_block.transaction();
        assert!(tx.world.external_event_buf.is_empty() && tx.world.internal_event_buf.is_empty());
        let next = Parameter::Custom(FxCorridorPolicyRegistry::default().into_custom_parameter());
        tx.prepare_fx_usage_parameter_change(&next).unwrap()
    };
    let mut tx = target_block.transaction();
    let original = tx.world.parameters.get().clone();
    assert!(source_original.belongs_to(&source_pool));
    assert!(original.belongs_to(tx.world.sccp_committee_generations.allocation_budget()));
    assert!(!source_pool.same_pool(tx.world.sccp_committee_generations.allocation_budget()));
    assert!(tx.world.external_event_buf.is_empty() && tx.world.internal_event_buf.is_empty());
    // Pass output geometry to exercise the actual retained graph's pool check.
    for _ in 0..6 {
        tx.world
            .external_event_buf
            .push(EventBox::Data(foreign.event.clone()));
        tx.world.internal_event_buf.push(foreign.event.clone());
    }
    let external = tx.world.external_event_buf.clone();
    let internal = tx.world.internal_event_buf.clone();
    assert_eq!((external.len(), internal.len()), (6, 6));
    assert!(matches!(
        tx.world.install_parameter_change(foreign),
        Err(ParameterChangeError::Ownership(
            ParameterRetentionError::ForeignPool
        ))
    ));
    assert!(original.same_version(tx.world.parameters.get()));
    assert!(source_original.same_version(source_block.world.parameters.get()));
    assert_eq!(tx.world.external_event_buf, external);
    assert_eq!(tx.world.internal_event_buf, internal);
    drop(tx);
    assert!(original.same_version(target_block.world.parameters.get()));
}
