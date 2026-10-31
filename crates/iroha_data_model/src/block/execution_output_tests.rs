//! Pure output encoding/identity controls; none of these fixtures authenticate execution.

#[path = "execution_output_ballot_tests.rs"]
mod anonymous_ballot;

use super::*;
use crate::{
    NetworkId, ValidationFail,
    account::AccountId,
    events::{
        data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferOutcome},
        time::TimeInterval,
    },
    transaction::{
        FeePaymentIntent, TransactionBuilder, error::TransactionRejectionReason,
        signed::SealedTransactionReveal,
    },
    trigger::DataTriggerStep,
};
use iroha_crypto::{Algorithm, KeyPair};

fn input() -> TransactionEntrypoint {
    let key = KeyPair::from_seed(vec![0x42; 32], Algorithm::Ed25519);
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"output fixture genesis",
    )));
    let mut builder = TransactionBuilder::new(
        network,
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(std::time::Duration::from_millis(123));
    TransactionEntrypoint::External(builder.sign(key.private_key()))
}

fn proposal() -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::new(b"immutable proposal identity"))
}

fn trigger() -> TriggerUseV1 {
    TriggerUseV1 {
        trigger_id: "output-root".parse().unwrap(),
        registered_at_height: 3,
        action_hash: Hash::new(b"untrusted model action digest, not State evidence"),
    }
}

fn step() -> ExecutionStep {
    ExecutionStep(iroha_primitives::const_vec::ConstVec::new_empty())
}

fn root_result(trigger: &TriggerUseV1) -> TransactionResult {
    TransactionResult::new(Ok(vec![DataTriggerStep {
        id: trigger.trigger_id.clone(),
        instructions: step(),
    }]))
}

fn pipeline_invocation() -> PipelineInvocationV1 {
    PipelineInvocationV1 {
        event: PipelineEventPositionV1::BlockApproved,
        candidate_index: 4,
        trigger: trigger(),
    }
}

fn time_invocation() -> TimeInvocationV1 {
    TimeInvocationV1 {
        schedule_index: 2,
        event: TimeEvent {
            interval: TimeInterval {
                since_ms: 100,
                length_ms: 20,
            },
        },
        trigger: trigger(),
    }
}

fn completion(trigger: &TriggerUseV1) -> InvocationCompletionV1 {
    InvocationCompletionV1 {
        callback_index: 0,
        trigger_id: trigger.trigger_id.clone(),
        outcome: TriggerCompletedOutcome::Success,
    }
}

fn pipeline() -> ExecutionOutputV1 {
    let invocation = pipeline_invocation();
    ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
        result: root_result(&invocation.trigger),
        completions: vec![completion(&invocation.trigger)],
        invocation,
        failure_root: None,
    })
}

fn time() -> ExecutionOutputV1 {
    let invocation = time_invocation();
    ExecutionOutputV1::Time(TimeExecutionOutputV1 {
        result: root_result(&invocation.trigger),
        completions: vec![completion(&invocation.trigger)],
        invocation,
        failure_root: None,
    })
}

fn network() -> ExecutionOutputV1 {
    ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: TransactionResult::new(Ok(Vec::new())),
        completions: Vec::new(),
    })
}

fn rejected() -> TransactionResult {
    TransactionResult::new(Err(TransactionRejectionReason::Validation(
        ValidationFail::NotPermitted("model rejection".into()),
    )))
}

fn receipt() -> AssetBatchTransferOutcome {
    let authority = input().authority_opt().expect("signed fixture").clone();
    AssetBatchTransferOutcome {
        leg_index: 0,
        leg_id: "owned-leg".into(),
        asset: crate::asset::AssetId::new(
            crate::asset::AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("output-test", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            authority.clone(),
        ),
        destination: authority,
        amount: iroha_primitives::numeric::Quantity::from(1_u32),
        status: AssetBatchTransferLegStatus::Applied,
    }
}

#[test]
fn execution_output_roundtrips_all_owners_diagnostics_and_exact_receipts() {
    let mut network = network();
    if let ExecutionOutputV1::Network(NetworkExecutionOutputV1 { result, .. }) = &mut network {
        result.set_batch_transfer_outcomes(vec![receipt()]);
    }
    let mut failed = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 {
        result,
        failure_root,
        completions,
        ..
    }) = &mut failed
    {
        *result = rejected();
        *failure_root = Some(TriggerFailureRootV1::ReturnedBeforeRollback(step()));
        completions[0].outcome = TriggerCompletedOutcome::Failure("rolled back".into());
    }
    let mut declared = failed.clone();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { failure_root, .. }) = &mut declared {
        *failure_root = Some(TriggerFailureRootV1::DeclaredInstructionProjection(step()));
    }
    let mut network_event = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { invocation, .. }) =
        &mut network_event
    {
        invocation.event = PipelineEventPositionV1::Network(0);
    }
    for row in [network, pipeline(), network_event, time(), failed, declared] {
        row.validate_structure(8, &[input()]).unwrap();
        let bytes = norito::encode_canonical(&row).unwrap();
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
            row
        );
        let json = norito::json::to_json(&row).unwrap();
        assert_eq!(
            norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
            row
        );
        assert_eq!(
            row.result().batch_transfer_outcomes(),
            row.result().1.as_slice()
        );
        assert_eq!(
            row.completions().len(),
            usize::from(!matches!(
                row,
                ExecutionOutputV1::Network(NetworkExecutionOutputV1 { .. })
            ))
        );
    }
}

#[test]
fn execution_output_json_requires_null_slot_and_rejects_unknown_owner_fields() {
    let original = norito::json::to_value(&time()).unwrap();
    let detail = original.as_object().unwrap().get("detail").unwrap();
    assert!(detail.as_object().unwrap().contains_key("failure_root"));
    let mut omitted = original.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .get_mut("detail")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("failure_root")
        .unwrap();
    assert!(norito::json::from_value::<ExecutionOutputV1>(omitted).is_err());
    let mut foreign = original.clone();
    foreign
        .as_object_mut()
        .unwrap()
        .get_mut("detail")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("entrypoint".into(), norito::json::Value::Null);
    assert!(norito::json::from_value::<ExecutionOutputV1>(foreign).is_err());
    let mut kind = original;
    kind.as_object_mut().unwrap().insert(
        "kind".into(),
        norito::json::Value::String("legacy_time_entrypoint".into()),
    );
    assert!(norito::json::from_value::<ExecutionOutputV1>(kind).is_err());
    let mut action = norito::json::to_value(&trigger()).unwrap();
    action
        .as_object_mut()
        .unwrap()
        .remove("action_hash")
        .unwrap();
    assert!(norito::json::from_value::<TriggerUseV1>(action).is_err());
}

#[test]
fn invocation_identity_commits_every_prebody_field_and_distinct_domains() {
    let original = pipeline_invocation();
    let hash = original.execution_call_hash(proposal()).unwrap();
    assert_eq!(
        hash,
        Hash::new_from_chunks(&[
            PIPELINE_CALL_DOMAIN,
            proposal().as_ref(),
            &norito::encode_canonical(&original).unwrap()
        ])
    );
    let mut mutations = Vec::new();
    let mut changed = original.clone();
    changed.candidate_index += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.event = PipelineEventPositionV1::Network(0);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.trigger.trigger_id = "other-root".parse().unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.trigger.registered_at_height += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.trigger.action_hash = Hash::new(b"other action");
    mutations.push(changed);
    // Authority is committed inside the actual State-owned action_hash. The Core
    // helper's rekey regression checks that preimage and both invocation calls;
    // there is no independent model authority claim to mutate here.
    for changed in mutations {
        assert_ne!(changed.execution_call_hash(proposal()).unwrap(), hash);
    }
    assert_ne!(
        original
            .execution_call_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"another proposal"
            )))
            .unwrap(),
        hash
    );
    let time = time_invocation();
    assert_eq!(
        time.execution_call_hash(proposal()).unwrap(),
        Hash::new_from_chunks(&[
            TIME_CALL_DOMAIN,
            proposal().as_ref(),
            &norito::encode_canonical(&time).unwrap()
        ])
    );
    assert_ne!(time.execution_call_hash(proposal()).unwrap(), hash);
    assert_ne!(
        invocation_hash(PIPELINE_CALL_DOMAIN, proposal(), &time).unwrap(),
        time.execution_call_hash(proposal()).unwrap()
    );
    let alternative =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _guard = norito::core::DecodeFlagsGuard::enter(alternative);
    assert_eq!(original.execution_call_hash(proposal()).unwrap(), hash);
}

#[test]
fn equal_time_displays_have_distinct_actual_occurrence_owners() {
    let first = time();
    let mut second = first.clone();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { invocation, .. }) = &mut second {
        invocation.schedule_index = 9;
    }
    assert_eq!(first.result(), second.result());
    assert_ne!(
        first.execution_call_hash(proposal(), &[]).unwrap(),
        second.execution_call_hash(proposal(), &[]).unwrap()
    );
    validate_execution_outputs_v1(&[first, second], proposal(), 8, &[]).unwrap();
    let original = time_invocation();
    let mut changed = original.clone();
    changed.event.interval.since_ms += 1;
    assert_ne!(
        original.execution_call_hash(proposal()).unwrap(),
        changed.execution_call_hash(proposal()).unwrap()
    );
    changed = original.clone();
    changed.event.interval.length_ms += 1;
    assert_ne!(
        original.execution_call_hash(proposal()).unwrap(),
        changed.execution_call_hash(proposal()).unwrap()
    );
}

#[test]
fn output_result_and_receipt_mutation_never_changes_prebody_identity() {
    let mut row = pipeline();
    let original = row.clone();
    let call = row.execution_call_hash(proposal(), &[]).unwrap();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
        result,
        completions,
        ..
    }) = &mut row
    {
        result.set_batch_transfer_outcomes(vec![receipt()]);
        result.0.as_mut().unwrap().push(DataTriggerStep {
            id: "chained".parse().unwrap(),
            instructions: step(),
        });
        completions.push(InvocationCompletionV1 {
            callback_index: 3,
            trigger_id: "chained".parse().unwrap(),
            outcome: TriggerCompletedOutcome::Success,
        });
    }
    row.validate_structure(8, &[]).unwrap();
    assert_eq!(row.execution_call_hash(proposal(), &[]).unwrap(), call);
    assert_ne!(HashOf::new(&row), HashOf::new(&original));
    assert_ne!(
        norito::encode_canonical(&row).unwrap(),
        norito::encode_canonical(&original).unwrap()
    );
    // Structural acceptance of changed claims is deliberate; only actual Core
    // execution and exact global executed-wire finality authenticate their truth.
}

#[test]
fn execution_output_preserves_sealed_outer_source_and_inner_call_identity() {
    let TransactionEntrypoint::External(signed) = input() else {
        unreachable!()
    };
    let inner = signed.hash_as_entrypoint();
    let first = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"first commitment"),
        signed.clone(),
        [1; 32],
    ));
    let second = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"second commitment"),
        signed,
        [2; 32],
    ));
    assert_ne!(first.hash(), inner);
    assert_ne!(first.hash(), second.hash());
    assert_eq!(
        network()
            .execution_call_hash(proposal(), std::slice::from_ref(&first))
            .unwrap(),
        Hash::from(inner)
    );
    validate_execution_outputs_v1(&[network()], proposal(), 8, std::slice::from_ref(&first))
        .unwrap();
    let mut second_row = network();
    if let ExecutionOutputV1::Network(NetworkExecutionOutputV1 { input_index, .. }) =
        &mut second_row
    {
        *input_index = 1;
    }
    assert!(
        validate_execution_outputs_v1(&[network(), second_row], proposal(), 8, &[first, second])
            .unwrap_err()
            .contains("one execution call")
    );
}

#[test]
fn execution_output_rejects_foreign_sources_phases_and_duplicate_candidates() {
    let inputs = vec![input()];
    let valid = vec![network(), pipeline(), time()];
    validate_execution_outputs_v1(&valid, proposal(), 8, &inputs).unwrap();
    for rows in [
        vec![pipeline(), time()],
        vec![network(), network()],
        vec![pipeline(), network()],
        vec![network(), time(), pipeline()],
        vec![network(), pipeline(), pipeline()],
    ] {
        assert!(validate_execution_outputs_v1(&rows, proposal(), 8, &inputs).is_err());
    }
    let mut foreign = network();
    if let ExecutionOutputV1::Network(NetworkExecutionOutputV1 { input_index, .. }) = &mut foreign {
        *input_index = 1;
    }
    assert!(foreign.validate_structure(8, &inputs).is_err());
    let mut candidate = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { invocation, .. }) =
        &mut candidate
    {
        invocation.candidate_index += 1;
    }
    assert!(
        validate_execution_outputs_v1(&[network(), pipeline(), candidate], proposal(), 8, &inputs)
            .is_err()
    );
    let mut wrong_event = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { invocation, .. }) =
        &mut wrong_event
    {
        invocation.event = PipelineEventPositionV1::Network(9);
    }
    assert!(wrong_event.validate_structure(8, &inputs).is_err());
}

#[test]
fn execution_output_allows_real_candidate_gaps_but_not_event_order_reversal() {
    let inputs = vec![input()];
    let mut first = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { invocation, .. }) = &mut first {
        invocation.event = PipelineEventPositionV1::Network(0);
    }
    let last = pipeline();
    validate_execution_outputs_v1(
        &[network(), first.clone(), last.clone()],
        proposal(),
        8,
        &inputs,
    )
    .unwrap();
    assert!(
        validate_execution_outputs_v1(&[network(), last, first], proposal(), 8, &inputs).is_err()
    );
    validate_execution_outputs_v1(&[pipeline()], proposal(), 8, &[]).unwrap();
    validate_execution_outputs_v1(&[], proposal(), 8, &[]).unwrap();
}

#[test]
fn execution_output_rejects_root_diagnostic_and_rollback_receipt_substitutions() {
    let mut row = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { result, .. }) = &mut row {
        result.0 = Ok(Vec::new());
    }
    assert!(row.validate_structure(8, &[]).is_err());
    row = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { result, .. }) = &mut row {
        result.0.as_mut().unwrap()[0].id = "wrong-root".parse().unwrap();
    }
    assert!(row.validate_structure(8, &[]).is_err());
    row = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { failure_root, .. }) = &mut row {
        *failure_root = Some(TriggerFailureRootV1::ReturnedBeforeRollback(step()));
    }
    assert!(row.validate_structure(8, &[]).is_err());
    row = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { result, .. }) = &mut row {
        *result = rejected();
    }
    assert!(row.validate_structure(8, &[]).is_err());
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 {
        failure_root,
        completions,
        ..
    }) = &mut row
    {
        *failure_root = Some(TriggerFailureRootV1::DeclaredInstructionProjection(step()));
        completions[0].outcome = TriggerCompletedOutcome::Failure("whole invocation failed".into());
    }
    row.validate_structure(8, &[]).unwrap();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { result, .. }) = &mut row {
        result.set_batch_transfer_outcomes(vec![receipt()]);
    }
    assert!(row.validate_structure(8, &[]).is_err());
}

#[test]
fn rejected_network_output_cannot_retain_rolled_back_callback_completions() {
    let inputs = vec![input()];
    let mut row = network();
    let ExecutionOutputV1::Network(output) = &mut row else {
        unreachable!("Network fixture");
    };
    output.completions.push(completion(&trigger()));
    row.validate_structure(8, &inputs).unwrap();
    validate_execution_outputs_v1(std::slice::from_ref(&row), proposal(), 8, &inputs).unwrap();

    let ExecutionOutputV1::Network(output) = &mut row else {
        unreachable!("Network fixture");
    };
    output.result = rejected();
    assert!(row.validate_structure(8, &inputs).is_err());
    assert!(
        validate_execution_outputs_v1(std::slice::from_ref(&row), proposal(), 8, &inputs).is_err()
    );

    let ExecutionOutputV1::Network(output) = &mut row else {
        unreachable!("Network fixture");
    };
    output.completions[0].outcome = TriggerCompletedOutcome::Failure("rolled back".into());
    assert!(row.validate_structure(8, &inputs).is_err());
    assert!(
        validate_execution_outputs_v1(std::slice::from_ref(&row), proposal(), 8, &inputs).is_err()
    );

    let ExecutionOutputV1::Network(output) = &mut row else {
        unreachable!("Network fixture");
    };
    output.completions.clear();
    row.validate_structure(8, &inputs).unwrap();
    validate_execution_outputs_v1(&[row], proposal(), 8, &inputs).unwrap();
}

#[test]
fn rejected_internal_output_retains_only_the_whole_invocation_root_failure() {
    for row in [pipeline(), time()] {
        for diagnostic in [
            TriggerFailureRootV1::ReturnedBeforeRollback(step()),
            TriggerFailureRootV1::DeclaredInstructionProjection(step()),
        ] {
            let mut failed = row.clone();
            let (result, failure_root, completions) = match &mut failed {
                ExecutionOutputV1::Pipeline(output) => (
                    &mut output.result,
                    &mut output.failure_root,
                    &mut output.completions,
                ),
                ExecutionOutputV1::Time(output) => (
                    &mut output.result,
                    &mut output.failure_root,
                    &mut output.completions,
                ),
                ExecutionOutputV1::Network(_)
                | ExecutionOutputV1::NetworkWithLifecycle(_)
                | ExecutionOutputV1::PipelineWithLifecycle(_)
                | ExecutionOutputV1::TimeWithLifecycle(_) => unreachable!("internal fixture"),
            };
            *result = rejected();
            *failure_root = Some(diagnostic);
            completions[0].outcome =
                TriggerCompletedOutcome::Failure("whole invocation failed".into());
            failed.validate_structure(8, &[]).unwrap();
            validate_execution_outputs_v1(std::slice::from_ref(&failed), proposal(), 8, &[])
                .unwrap();

            for mutation in 0..6 {
                let mut invalid = failed.clone();
                let completions = match &mut invalid {
                    ExecutionOutputV1::Pipeline(output) => &mut output.completions,
                    ExecutionOutputV1::Time(output) => &mut output.completions,
                    ExecutionOutputV1::Network(_)
                    | ExecutionOutputV1::NetworkWithLifecycle(_)
                    | ExecutionOutputV1::PipelineWithLifecycle(_)
                    | ExecutionOutputV1::TimeWithLifecycle(_) => unreachable!("internal fixture"),
                };
                match mutation {
                    0 => completions[0].outcome = TriggerCompletedOutcome::Success,
                    1 => completions[0].callback_index = 1,
                    2 => completions[0].trigger_id = "different-root".parse().unwrap(),
                    3 => completions.clear(),
                    4 | 5 => completions.push(InvocationCompletionV1 {
                        callback_index: 1,
                        trigger_id: "rolled-back-nested-callback".parse().unwrap(),
                        outcome: if mutation == 4 {
                            TriggerCompletedOutcome::Success
                        } else {
                            TriggerCompletedOutcome::Failure("nested failure".into())
                        },
                    }),
                    _ => unreachable!(),
                }
                assert!(
                    invalid.validate_structure(8, &[]).is_err(),
                    "mutation {mutation}"
                );
                assert!(
                    validate_execution_outputs_v1(&[invalid], proposal(), 8, &[]).is_err(),
                    "mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn execution_output_rejects_invalid_chronology_intervals_and_completion_positions() {
    let mut row = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { invocation, .. }) = &mut row {
        invocation.trigger.registered_at_height = 8;
    }
    assert!(row.validate_structure(8, &[]).is_err());
    row = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { invocation, .. }) = &mut row {
        invocation.event.interval.since_ms = u64::MAX;
    }
    assert!(row.validate_structure(8, &[]).is_err());
    assert!(row.execution_call_hash(proposal(), &[]).is_err());
    row = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { completions, .. }) = &mut row {
        completions.push(completions[0].clone());
    }
    assert!(row.validate_structure(8, &[]).is_err());
    row = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { completions, .. }) = &mut row {
        completions[0].trigger_id = "foreign-root".parse().unwrap();
    }
    assert!(row.validate_structure(8, &[]).is_err());
    let mut invocation = pipeline_invocation();
    invocation.trigger.action_hash = Hash::prehashed([0; Hash::LENGTH]);
    assert!(invocation.execution_call_hash(proposal()).is_err());
    assert!(
        pipeline_invocation()
            .execution_call_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0; Hash::LENGTH]
            )))
            .is_err()
    );
    assert!(validate_execution_outputs_v1(&[], proposal(), 0, &[]).is_err());
    let mut other_interval = time();
    if let ExecutionOutputV1::Time(TimeExecutionOutputV1 { invocation, .. }) = &mut other_interval {
        invocation.schedule_index += 1;
        invocation.event.interval.length_ms += 1;
    }
    assert!(validate_execution_outputs_v1(&[time(), other_interval], proposal(), 8, &[]).is_err());
    assert!(validate_execution_outputs_v1(&[time(), time()], proposal(), 8, &[]).is_err());
}

#[test]
fn synthetic_time_entrypoint_cannot_decode_or_own_a_network_output() {
    // Reproduce the retired bytes locally; no production compatibility type or
    // decoder remains merely to keep this negative wire test constructible.
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::trigger::time::TimeTriggerEntrypoint")]
    struct RetiredTimeEntrypoint {
        id: TriggerId,
        instructions: ExecutionStep,
        authority: AccountId,
    }
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::transaction::signed::model::TransactionEntrypoint")]
    enum RetiredNetworkTime {
        #[codec(index = 3)]
        Time(RetiredTimeEntrypoint),
    }
    let old = RetiredTimeEntrypoint {
        id: trigger().trigger_id,
        authority: input().authority_opt().expect("signed fixture").clone(),
        instructions: step(),
    };
    assert!(
        norito::decode_canonical::<ExecutionOutputV1>(&norito::encode_canonical(&old).unwrap())
            .is_err()
    );
    let retired_network = norito::encode_canonical(&RetiredNetworkTime::Time(old)).unwrap();
    assert!(norito::decode_canonical::<TransactionEntrypoint>(&retired_network).is_err());
    // A scheduled output supplies no network input at all.
    assert!(network().validate_structure(8, &[]).is_err());
    assert!(network().execution_call_hash(proposal(), &[]).is_err());
    let mut bytes = norito::encode_canonical(&time()).unwrap();
    bytes.push(0);
    assert!(norito::decode_canonical::<ExecutionOutputV1>(&bytes).is_err());
}

#[test]
fn bounded_output_limit_terminals_roundtrip_and_preserve_invocation_identity() {
    let inputs = [input()];
    let cases = [
        (
            network(),
            ExecutionOutputV1::network_output_limit_rejection(0),
        ),
        (
            pipeline(),
            ExecutionOutputV1::pipeline_output_limit_rejection(pipeline_invocation()),
        ),
        (
            time(),
            ExecutionOutputV1::time_output_limit_rejection(time_invocation()),
        ),
    ];
    for (original, terminal) in cases {
        assert!(terminal.is_output_limit_rejection());
        terminal.validate_structure(8, &inputs).unwrap();
        assert_eq!(
            terminal.execution_call_hash(proposal(), &inputs).unwrap(),
            original.execution_call_hash(proposal(), &inputs).unwrap()
        );
        let bytes = norito::encode_canonical(&terminal).unwrap();
        assert_eq!(norito::canonical_frame_len(&terminal).unwrap(), bytes.len());
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
            terminal
        );
        assert!(terminal.result().batch_transfer_outcomes().is_empty());
    }
}

#[test]
fn omitted_failure_root_requires_exact_typed_terminal_and_only_root_completion() {
    let terminal = ExecutionOutputV1::pipeline_output_limit_rejection(pipeline_invocation());
    for fault in 0..6 {
        let mut altered = terminal.clone();
        let ExecutionOutputV1::Pipeline(output) = &mut altered else {
            unreachable!()
        };
        match fault {
            0 => {
                output.result = TransactionResult::new(Err(ValidationFail::NotPermitted(
                    EXECUTION_OUTPUT_LIMIT_REASON.into(),
                )
                .into()))
            }
            1 => output.completions.clear(),
            2 => output.completions[0].callback_index = 1,
            3 => output.completions[0].trigger_id = "foreign-root".parse().unwrap(),
            4 => output.completions[0].outcome = TriggerCompletedOutcome::Success,
            _ => output.completions.push(output.completions[0].clone()),
        }
        assert!(!altered.is_output_limit_rejection(), "fault {fault}");
        assert!(altered.validate_structure(8, &[]).is_err(), "fault {fault}");
    }
}

#[test]
fn bounded_terminal_does_not_hide_receipts_or_ordinary_failure_diagnostics() {
    let mut receipt_terminal =
        ExecutionOutputV1::pipeline_output_limit_rejection(pipeline_invocation());
    let ExecutionOutputV1::Pipeline(output) = &mut receipt_terminal else {
        unreachable!()
    };
    output.result.set_batch_transfer_outcomes(vec![receipt()]);
    assert!(!receipt_terminal.is_output_limit_rejection());
    assert!(receipt_terminal.validate_structure(8, &[]).is_err());

    let mut terminal = ExecutionOutputV1::time_output_limit_rejection(time_invocation());
    let ExecutionOutputV1::Time(output) = &mut terminal else {
        unreachable!()
    };
    output.failure_root = Some(TriggerFailureRootV1::ReturnedBeforeRollback(step()));
    assert!(!terminal.is_output_limit_rejection());

    let mut network_terminal = ExecutionOutputV1::network_output_limit_rejection(0);
    let ExecutionOutputV1::Network(output) = &mut network_terminal else {
        unreachable!()
    };
    output.completions.push(completion(&trigger()));
    assert!(!network_terminal.is_output_limit_rejection());

    let mut failed = pipeline();
    let ExecutionOutputV1::Pipeline(output) = &mut failed else {
        unreachable!()
    };
    output.failure_root = Some(TriggerFailureRootV1::OmittedByOutputLimit);
    assert!(failed.validate_structure(8, &[]).is_err());
}

#[test]
fn pipeline_network_event_requires_actual_signed_transaction_source_kind() {
    use crate::transaction::signed::{
        SealedTransactionCommitmentPayload, SignedSealedTransactionCommitment,
    };

    let key = KeyPair::from_seed(vec![0x24; 32], Algorithm::Ed25519);
    let payload = SealedTransactionCommitmentPayload::new(
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"sealed output fixture network",
        ))),
        AccountId::new(key.public_key().clone()),
        Hash::new(b"sealed output fixture commitment"),
        4,
        10,
        None,
    );
    let sealed = TransactionEntrypoint::SealedCommitment(SignedSealedTransactionCommitment::sign(
        payload,
        key.private_key(),
    ));
    validate_execution_outputs_v1(
        &[network(), pipeline()],
        proposal(),
        8,
        std::slice::from_ref(&sealed),
    )
    .unwrap();
    let mut event = pipeline();
    if let ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 { invocation, .. }) = &mut event {
        invocation.event = PipelineEventPositionV1::Network(0);
    }
    assert!(
        event
            .validate_structure(8, &[sealed])
            .unwrap_err()
            .contains("no signed transaction event")
    );
    let TransactionEntrypoint::External(signed) = input() else {
        unreachable!()
    };
    let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"revealed output fixture commitment"),
        signed,
        [9; 32],
    ));
    validate_execution_outputs_v1(&[network(), event], proposal(), 8, &[reveal]).unwrap();
}

#[test]
fn execution_inputs_borrow_slices_arrays_vectors_and_block_sources() {
    fn check<S: ExecutionInputs + ?Sized>(source: &S, expected: &TransactionEntrypoint) {
        assert_eq!(source.input_count(), 1);
        assert!(std::ptr::eq(source.input_at(0).unwrap(), expected));
        assert!(source.input_at(1).is_none());
        validate_execution_outputs_v1(&[network(), pipeline(), time()], proposal(), 8, source)
            .unwrap();
    }
    let array = [input()];
    check(&array, &array[0]);
    check(array.as_slice(), &array[0]);
    let vector = Vec::from(array);
    check(&vector, &vector[0]);

    let header = BlockHeader::new(8.try_into().unwrap(), None, None, 123, 0);
    let mut builder = crate::block::builder::BlockBuilder::new(header);
    let TransactionEntrypoint::External(signed) = input() else {
        unreachable!()
    };
    builder.push_transaction(signed);
    let block = builder.build(crate::block::BlockSignatures::default());
    check(&block, block.network_entrypoint_at(0).unwrap());
}

#[test]
fn execution_input_validation_uses_bounded_random_access() {
    struct CountedInput {
        entry: TransactionEntrypoint,
        lookups: std::cell::Cell<usize>,
    }
    impl ExecutionInputs for CountedInput {
        fn input_count(&self) -> usize {
            1
        }
        fn input_at(&self, index: usize) -> Option<&TransactionEntrypoint> {
            self.lookups.set(self.lookups.get() + 1);
            (index == 0).then_some(&self.entry)
        }
    }
    let source = CountedInput {
        entry: input(),
        lookups: std::cell::Cell::new(0),
    };
    let mut rows = vec![network()];
    for candidate_index in 0..32 {
        let mut row = pipeline();
        let ExecutionOutputV1::Pipeline(value) = &mut row else {
            unreachable!()
        };
        value.invocation.event = PipelineEventPositionV1::Network(0);
        value.invocation.candidate_index = candidate_index;
        value.invocation.trigger.trigger_id = format!("root-{candidate_index}").parse().unwrap();
        value.result = root_result(&value.invocation.trigger);
        value.completions = vec![completion(&value.invocation.trigger)];
        rows.push(row);
    }
    validate_execution_outputs_v1(&rows, proposal(), 8, &source).unwrap();
    assert_eq!(
        source.lookups.get(),
        34,
        "two network checks, one lookup per event"
    );
}

#[test]
fn trigger_use_rejects_retired_parallel_authority_slot() {
    use norito::codec::DecodeAll as _;

    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::block::execution_output::TriggerUseV1")]
    struct RetiredTriggerUse {
        trigger_id: TriggerId,
        authority: AccountId,
        registered_at_height: u64,
        action_hash: Hash,
    }
    let current = trigger();
    let json = norito::json::to_value(&current).unwrap();
    assert!(!json.as_object().unwrap().contains_key("authority"));
    for value in [
        norito::json::Value::Null,
        norito::json::to_value(input().authority_opt().expect("signed fixture")).unwrap(),
    ] {
        let mut retired = json.clone();
        retired
            .as_object_mut()
            .unwrap()
            .insert("authority".into(), value);
        assert!(norito::json::from_value::<TriggerUseV1>(retired).is_err());
    }

    let retired = RetiredTriggerUse {
        trigger_id: current.trigger_id.clone(),
        authority: input().authority_opt().expect("signed fixture").clone(),
        registered_at_height: current.registered_at_height,
        action_hash: current.action_hash,
    };
    let payload = retired.encode();
    assert!(TriggerUseV1::decode_all(&mut payload.as_slice()).is_err());
    assert!(
        norito::decode_canonical::<TriggerUseV1>(&norito::encode_canonical(&retired).unwrap())
            .is_err()
    );
    let payload = current.encode();
    assert_eq!(
        TriggerUseV1::decode_all(&mut payload.as_slice()).unwrap(),
        current
    );
}

#[test]
fn bounded_terminals_roundtrip_maximum_trigger_name_and_scalar_positions() {
    let trigger = TriggerUseV1 {
        trigger_id: "x"
            .repeat(iroha_model_base::name::MAX_NAME_BYTES)
            .parse()
            .unwrap(),
        registered_at_height: u64::MAX - 1,
        action_hash: Hash::new(b"bounded descriptor test; no authority claim"),
    };
    assert!(
        "x".repeat(iroha_model_base::name::MAX_NAME_BYTES + 1)
            .parse::<TriggerId>()
            .is_err()
    );
    let rows = [
        ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
            event: PipelineEventPositionV1::Network(u32::MAX),
            candidate_index: u32::MAX,
            trigger: trigger.clone(),
        }),
        ExecutionOutputV1::time_output_limit_rejection(TimeInvocationV1 {
            schedule_index: u32::MAX,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: u64::MAX - 1,
                    length_ms: 1,
                },
            },
            trigger,
        }),
    ];
    // These are sizing/codec fixtures, not admissible source positions or a live
    // capacity policy. Model source validation still rejects absent Network input.
    for row in rows {
        assert!(row.is_output_limit_rejection());
        let bytes = norito::encode_canonical(&row).unwrap();
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
            row
        );
        let mut changed = row.clone();
        match &mut changed {
            ExecutionOutputV1::Pipeline(row) => {
                row.invocation.trigger.action_hash = Hash::new(b"different actual action digest");
            }
            ExecutionOutputV1::Time(row) => {
                row.invocation.trigger.action_hash = Hash::new(b"different actual action digest");
            }
            ExecutionOutputV1::Network(_)
            | ExecutionOutputV1::NetworkWithLifecycle(_)
            | ExecutionOutputV1::PipelineWithLifecycle(_)
            | ExecutionOutputV1::TimeWithLifecycle(_) => unreachable!(),
        }
        assert_eq!(
            norito::encode_canonical(&changed).unwrap().len(),
            bytes.len()
        );
        assert_ne!(changed, row);
    }
}

fn omitted_internal_rejection(mut row: ExecutionOutputV1) -> ExecutionOutputV1 {
    let (result, root, completions) = match &mut row {
        ExecutionOutputV1::Pipeline(row) => {
            (&mut row.result, &mut row.failure_root, &mut row.completions)
        }
        ExecutionOutputV1::Time(row) => {
            (&mut row.result, &mut row.failure_root, &mut row.completions)
        }
        ExecutionOutputV1::Network(_)
        | ExecutionOutputV1::NetworkWithLifecycle(_)
        | ExecutionOutputV1::PipelineWithLifecycle(_)
        | ExecutionOutputV1::TimeWithLifecycle(_) => panic!("internal fixture"),
    };
    *result = TransactionResult::new(Err(TransactionRejectionReason::LimitCheck(
        crate::transaction::error::TransactionLimitError {
            reason: INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.into(),
        },
    )));
    *root = Some(TriggerFailureRootV1::OmittedAfterRejection);
    completions[0].outcome =
        TriggerCompletedOutcome::Failure(INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.into());
    row
}

#[test]
fn omitted_internal_rejection_has_a_distinct_exact_roundtrip_shape() {
    for terminal in [
        ExecutionOutputV1::pipeline_output_limit_rejection(pipeline_invocation()),
        ExecutionOutputV1::time_output_limit_rejection(time_invocation()),
    ] {
        assert!(!terminal.is_internal_rejection_diagnostic_omitted());
        let row = omitted_internal_rejection(terminal.clone());
        assert!(row.is_internal_rejection_diagnostic_omitted());
        assert!(!row.is_output_limit_rejection());
        row.validate_structure(8, &[input()]).unwrap();
        assert_eq!(
            row.execution_call_hash(proposal(), &[input()]).unwrap(),
            terminal
                .execution_call_hash(proposal(), &[input()])
                .unwrap()
        );
        let bytes = norito::encode_canonical(&row).unwrap();
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
            row
        );
        assert!(bytes.len() <= norito::encode_canonical(&terminal).unwrap().len());
        let json = norito::json::to_json(&row).unwrap();
        assert!(json.contains("omitted_after_rejection"));
        assert_eq!(
            norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
            row
        );
    }
    assert!(!network().is_internal_rejection_diagnostic_omitted());
    let variant = TriggerFailureRootV1::OmittedAfterRejection;
    let bytes = norito::encode_canonical(&variant).unwrap();
    assert_eq!(
        norito::decode_canonical::<TriggerFailureRootV1>(&bytes).unwrap(),
        variant
    );
}

#[test]
fn omitted_internal_rejection_refuses_tampered_result_root_completion_and_receipts() {
    for terminal in [
        ExecutionOutputV1::pipeline_output_limit_rejection(pipeline_invocation()),
        ExecutionOutputV1::time_output_limit_rejection(time_invocation()),
    ] {
        let valid = omitted_internal_rejection(terminal);
        for fault in 0..12 {
            let mut invalid = valid.clone();
            let (result, root, completions) = match &mut invalid {
                ExecutionOutputV1::Pipeline(row) => {
                    (&mut row.result, &mut row.failure_root, &mut row.completions)
                }
                ExecutionOutputV1::Time(row) => {
                    (&mut row.result, &mut row.failure_root, &mut row.completions)
                }
                ExecutionOutputV1::Network(_)
                | ExecutionOutputV1::NetworkWithLifecycle(_)
                | ExecutionOutputV1::PipelineWithLifecycle(_)
                | ExecutionOutputV1::TimeWithLifecycle(_) => unreachable!(),
            };
            match fault {
                0 => *result = rejected(),
                1 => {
                    *result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.into()),
                    )))
                }
                2 => *result = TransactionResult::new(Ok(Vec::new())),
                3 => *root = None,
                4 => *root = Some(TriggerFailureRootV1::OmittedByOutputLimit),
                5 => completions.clear(),
                6 => completions[0].outcome = TriggerCompletedOutcome::Success,
                7 => {
                    completions[0].outcome =
                        TriggerCompletedOutcome::Failure("different diagnostic".into())
                }
                8 => completions[0].callback_index = 1,
                9 => completions[0].trigger_id = "foreign-root".parse().unwrap(),
                10 => completions.push(completions[0].clone()),
                _ => result.set_batch_transfer_outcomes(vec![receipt()]),
            }
            assert!(
                !invalid.is_internal_rejection_diagnostic_omitted(),
                "fault {fault}"
            );
            assert!(
                invalid.validate_structure(8, &[input()]).is_err(),
                "fault {fault}"
            );
        }
    }
}

// Test-only owned codec oracle: deliberately NOT a production bounded scanner.
// The production interface has no implementation in this candidate.
#[derive(Default)]
struct LifecycleActionCodecOracle {
    calls: usize,
}

impl TriggerLifecycleActionValidatorV1 for LifecycleActionCodecOracle {
    fn validate_canonical_action(
        &mut self,
        frame: &[u8],
        _payload: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, String> {
        use crate::events::EventFilterBox;
        self.calls += 1;
        let action: crate::trigger::action::Action =
            norito::decode_canonical(frame).map_err(|error| error.to_string())?;
        let filter_kind = match action.filter() {
            EventFilterBox::Data(_) => TriggerLifecycleStateKindV1::Data,
            EventFilterBox::ExecuteTrigger(_) => TriggerLifecycleStateKindV1::ByCall,
            EventFilterBox::Pipeline(_) => TriggerLifecycleStateKindV1::Pipeline,
            EventFilterBox::Time(_) => TriggerLifecycleStateKindV1::Time,
            _ => return Err("invalid trigger filter in test oracle".into()),
        };
        Ok(TriggerLifecycleActionSemanticsV1 {
            filter_kind,
            has_retry_policy: action.retry_policy().is_some(),
        })
    }
}

fn lifecycle_frame(action: &crate::trigger::action::Action) -> TriggerLifecycleActionFrameV1 {
    TriggerLifecycleActionFrameV1 {
        codec: TriggerLifecycleActionCodecV1::NoritoCanonicalV1,
        bytes: norito::encode_canonical(action).unwrap(),
    }
}

fn lifecycle_fixture_action(
    frame: &TriggerLifecycleActionFrameV1,
) -> crate::trigger::action::Action {
    norito::decode_canonical(&frame.bytes).unwrap()
}

fn lifecycle_fixture_limits() -> TriggerLifecycleActionFrameLimitsV1 {
    // Inert fixture bounds, never a selected production policy.
    TriggerLifecycleActionFrameLimitsV1 {
        max_frame_bytes: 1024 * 1024,
        max_total_frame_bytes: 8 * 1024 * 1024,
        max_frames: 64,
    }
}

fn lifecycle_fixture_validate_frames(trace: &TriggerLifecycleEffectsV1) -> Result<(), String> {
    trace.validate_action_frames(
        &mut TriggerLifecycleActionFrameBudgetV1::new(lifecycle_fixture_limits()),
        &mut LifecycleActionCodecOracle::default(),
    )
}

fn lifecycle_network() -> ExecutionOutputV1 {
    let ExecutionOutputV1::Network(execution) = network() else {
        unreachable!()
    };
    ExecutionOutputV1::NetworkWithLifecycle(NetworkExecutionOutputV2 {
        execution,
        trigger_lifecycle: TriggerLifecycleEffectsV1::Complete {
            effects: Vec::new(),
        },
    })
}
fn lifecycle_pipeline() -> ExecutionOutputV1 {
    let ExecutionOutputV1::Pipeline(execution) = pipeline() else {
        unreachable!()
    };
    ExecutionOutputV1::PipelineWithLifecycle(PipelineExecutionOutputV2 {
        execution,
        trigger_lifecycle: lifecycle_root(TriggerLifecycleStateKindV1::Pipeline),
    })
}
fn lifecycle_time() -> ExecutionOutputV1 {
    let ExecutionOutputV1::Time(execution) = time() else {
        unreachable!()
    };
    ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
        execution,
        trigger_lifecycle: lifecycle_root(TriggerLifecycleStateKindV1::Time),
    })
}

fn lifecycle_state(kind: TriggerLifecycleStateKindV1) -> TriggerLifecycleStateV1 {
    use crate::{
        events::{
            EventFilterBox,
            pipeline::{BlockEventFilter, PipelineEventFilterBox},
            time::{ExecutionTime, Schedule, TimeEventFilter},
        },
        transaction::Executable,
        trigger::action::{Action, Repeats, TimeTriggerRetryPolicy},
    };
    let filter = match kind {
        TriggerLifecycleStateKindV1::Pipeline => {
            EventFilterBox::Pipeline(PipelineEventFilterBox::Block(BlockEventFilter::new()))
        }
        TriggerLifecycleStateKindV1::Time => EventFilterBox::Time(TimeEventFilter::new(
            ExecutionTime::Schedule(Schedule::starting_at(std::time::Duration::from_millis(100))),
        )),
        _ => unreachable!("these fixtures exercise actual Pipeline/Time owners"),
    };
    let action = Action::new(
        Executable::Instructions(iroha_primitives::const_vec::ConstVec::new_empty()),
        Repeats::Exactly(1),
        input().authority_opt().unwrap().clone(),
        filter,
    )
    .unwrap();
    let action = if kind == TriggerLifecycleStateKindV1::Time {
        action
            .with_retry_policy(TimeTriggerRetryPolicy {
                max_retries: std::num::NonZeroU32::new(2).unwrap(),
                retry_after_ms: std::num::NonZeroU64::new(10).unwrap(),
            })
            .unwrap()
    } else {
        action
    };
    TriggerLifecycleStateV1 {
        kind,
        registered_at_height: Some(trigger().registered_at_height),
        action_hash: Some(trigger().action_hash),
        action: lifecycle_frame(&action),
        retry_state: None,
    }
}

fn lifecycle_use(
    kind: TriggerLifecycleStateKindV1,
    index: u32,
    id: TriggerId,
) -> TriggerLifecycleEffectV1 {
    TriggerLifecycleEffectV1 {
        ordinal: 0,
        trigger_id: id,
        origin: TriggerLifecycleOriginV1::Derived {
            callback_index: Some(index),
        },
        change: TriggerLifecycleChangeV1::Invoke {
            state: lifecycle_state(kind),
            callback_index: index,
        },
    }
}

fn lifecycle_root(kind: TriggerLifecycleStateKindV1) -> TriggerLifecycleEffectsV1 {
    TriggerLifecycleEffectsV1::Complete {
        effects: vec![lifecycle_use(kind, 0, trigger().trigger_id)],
    }
}

fn lifecycle_mut(row: &mut ExecutionOutputV1) -> &mut TriggerLifecycleEffectsV1 {
    match row {
        ExecutionOutputV1::NetworkWithLifecycle(row) => &mut row.trigger_lifecycle,
        ExecutionOutputV1::PipelineWithLifecycle(row) => &mut row.trigger_lifecycle,
        ExecutionOutputV1::TimeWithLifecycle(row) => &mut row.trigger_lifecycle,
        ExecutionOutputV1::Network(_)
        | ExecutionOutputV1::Pipeline(_)
        | ExecutionOutputV1::Time(_) => panic!("explicit successor fixture required"),
    }
}

fn lifecycle_insert() -> TriggerLifecycleEffectV1 {
    TriggerLifecycleEffectV1 {
        ordinal: 0,
        trigger_id: trigger().trigger_id,
        origin: TriggerLifecycleOriginV1::DirectNetworkInstruction { item_index: 0 },
        change: TriggerLifecycleChangeV1::Insert {
            after: lifecycle_state(TriggerLifecycleStateKindV1::Time),
        },
    }
}

#[test]
fn lifecycle_complete_empty_and_populated_successor_rows_roundtrip() {
    for row in [
        lifecycle_network(),
        lifecycle_pipeline(),
        lifecycle_time(),
        ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation()),
    ] {
        row.validate_structure(8, &[input()]).unwrap();
        let bytes = norito::encode_canonical(&row).unwrap();
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
            row
        );
        let json = norito::json::to_json(&row).unwrap();
        assert!(json.contains("trigger_lifecycle") && json.contains("complete"));
        assert_eq!(
            norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
            row
        );
    }
    assert!(
        lifecycle_network()
            .require_trigger_lifecycle()
            .unwrap()
            .effects()
            .is_empty()
    );
    assert_eq!(
        lifecycle_time()
            .require_trigger_lifecycle()
            .unwrap()
            .effects()
            .len(),
        1
    );
}

#[test]
fn lifecycle_missing_or_partial_coverage_never_becomes_complete_empty() {
    for row in [lifecycle_network(), lifecycle_pipeline(), lifecycle_time()] {
        let json = norito::json::to_json(&row).unwrap();
        let trace = norito::json::to_json(row.require_trigger_lifecycle().unwrap()).unwrap();
        let field = format!(",\"trigger_lifecycle\":{trace}");
        let missing = json.replace(&field, "");
        assert_ne!(missing, json, "the actual required member was removed");
        assert!(norito::json::from_str::<ExecutionOutputV1>(&missing).is_err());
    }
    for malformed in [
        r#"{"kind":"partial","detail":{"effects":[]}}"#,
        r#"{"kind":"complete","detail":{}}"#,
        r#"{"kind":"complete","detail":{"effects":[],"extra":true}}"#,
    ] {
        assert!(norito::json::from_str::<TriggerLifecycleEffectsV1>(malformed).is_err());
    }
}

#[test]
fn lifecycle_full_action_and_retry_state_survive_canonical_roundtrip() {
    let mut state = lifecycle_state(TriggerLifecycleStateKindV1::Time);
    state.retry_state = Some(TriggerLifecycleRetryStateV1 {
        attempt: 1,
        next_time_ms: 120,
    });
    let trace = TriggerLifecycleEffectsV1::Complete {
        effects: vec![TriggerLifecycleEffectV1 {
            change: TriggerLifecycleChangeV1::Insert {
                after: state.clone(),
            },
            ..lifecycle_insert()
        }],
    };
    trace
        .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], false)
        .unwrap();
    let bytes = norito::encode_canonical(&trace).unwrap();
    let decoded = norito::decode_canonical::<TriggerLifecycleEffectsV1>(&bytes).unwrap();
    assert_eq!(decoded, trace);
    let TriggerLifecycleChangeV1::Insert { after } = &decoded.effects()[0].change else {
        unreachable!()
    };
    assert_eq!(after.action.bytes, state.action.bytes);
    let after_action = lifecycle_fixture_action(&after.action);
    let state_action = lifecycle_fixture_action(&state.action);
    assert_eq!(after_action.executable(), state_action.executable());
    assert_eq!(after_action.filter(), state_action.filter());
    assert_eq!(after_action.authority(), state_action.authority());
    assert_eq!(after_action.repeats(), state_action.repeats());
    assert_eq!(after_action.metadata, state_action.metadata);
    assert_eq!(after_action.retry_policy(), state_action.retry_policy());
    lifecycle_fixture_validate_frames(&decoded).unwrap();
    assert_eq!(
        after.retry_state,
        Some(TriggerLifecycleRetryStateV1 {
            attempt: 1,
            next_time_ms: 120
        })
    );
    // Shape checks deliberately do not accept the fixture hash as State proof.
}

#[test]
fn lifecycle_order_preserves_remove_reinsert_and_equal_state_touches() {
    let state = lifecycle_state(TriggerLifecycleStateKindV1::Time);
    let mut effects = vec![lifecycle_insert(); 4];
    effects[1].change = TriggerLifecycleChangeV1::Update {
        before: state.clone(),
        after: state.clone(),
    };
    effects[2].change = TriggerLifecycleChangeV1::Remove {
        before: state.clone(),
    };
    for (index, effect) in effects.iter_mut().enumerate() {
        effect.ordinal = u32::try_from(index).unwrap();
    }
    let trace = TriggerLifecycleEffectsV1::Complete { effects };
    trace
        .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], false)
        .unwrap();
    assert_eq!(trace.effects().len(), 4);
    for fault in 0..4 {
        let mut changed = trace.clone();
        let TriggerLifecycleEffectsV1::Complete { effects } = &mut changed;
        match fault {
            0 => effects[1].ordinal = 0,
            1 => effects[1].ordinal = 2,
            2 => effects.swap(0, 1),
            _ => {
                if let TriggerLifecycleChangeV1::Remove { before } = &mut effects[2].change {
                    before.action_hash = Some(Hash::new(b"foreign preceding state"));
                }
            }
        }
        assert!(
            changed
                .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], false)
                .is_err(),
            "fault {fault}"
        );
    }
}

#[test]
fn lifecycle_direct_origin_and_callback_zero_keep_distinct_owners() {
    let direct = TriggerLifecycleEffectsV1::Complete {
        effects: vec![lifecycle_insert()],
    };
    direct
        .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], false)
        .unwrap();
    assert!(
        direct
            .validate_structure(TriggerLifecycleOwnerV1::Time(&trigger()), 8, &[], false)
            .is_err()
    );
    let mut network = lifecycle_network();
    let ExecutionOutputV1::NetworkWithLifecycle(row) = &mut network else {
        unreachable!()
    };
    row.execution.completions.push(completion(&trigger()));
    row.trigger_lifecycle = lifecycle_root(TriggerLifecycleStateKindV1::Time);
    network.validate_structure(8, &[input()]).unwrap();
    let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut network);
    effects[0].origin = TriggerLifecycleOriginV1::DirectNetworkInstruction { item_index: 0 };
    assert!(network.validate_structure(8, &[input()]).is_err());
}

#[test]
fn lifecycle_invocations_require_exact_unique_successful_completions() {
    for fault in 0..7 {
        let mut row = lifecycle_time();
        let ExecutionOutputV1::TimeWithLifecycle(output) = &mut row else {
            unreachable!()
        };
        let TriggerLifecycleEffectsV1::Complete { effects } = &mut output.trigger_lifecycle;
        match fault {
            0 => effects.clear(),
            1 => {
                let mut duplicate = effects[0].clone();
                duplicate.ordinal = 1;
                effects.push(duplicate);
            }
            2 => {
                effects[0].origin = TriggerLifecycleOriginV1::Derived {
                    callback_index: None,
                }
            }
            3 => effects[0].trigger_id = "foreign-callback".parse().unwrap(),
            4 => {
                effects[0].origin = TriggerLifecycleOriginV1::Derived {
                    callback_index: Some(8),
                }
            }
            5 => {
                output.execution.completions[0].outcome =
                    TriggerCompletedOutcome::Failure("failed".into())
            }
            _ => {
                effects.clear();
                output.execution.completions.clear();
            }
        }
        assert!(row.validate_structure(8, &[]).is_err(), "fault {fault}");
    }
}

#[test]
fn lifecycle_rollback_and_output_limit_never_retain_body_effects() {
    let mut terminal = ExecutionOutputV1::time_lifecycle_output_limit_rejection(time_invocation());
    terminal.validate_structure(8, &[]).unwrap();
    *lifecycle_mut(&mut terminal) = lifecycle_root(TriggerLifecycleStateKindV1::Time);
    assert!(!terminal.is_output_limit_rejection());
    assert!(terminal.validate_structure(8, &[]).is_err());
    let trace = TriggerLifecycleEffectsV1::Complete {
        effects: vec![lifecycle_insert()],
    };
    assert!(
        trace
            .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], true)
            .is_err()
    );
}

#[test]
fn lifecycle_genuine_rejection_keeps_typed_retry_policy_with_full_state() {
    let mut row = lifecycle_time();
    let ExecutionOutputV1::TimeWithLifecycle(output) = &mut row else {
        unreachable!()
    };
    output.execution.result = rejected();
    output.execution.failure_root =
        Some(TriggerFailureRootV1::DeclaredInstructionProjection(step()));
    output.execution.completions[0].outcome =
        TriggerCompletedOutcome::Failure("actual native rejection".into());
    let before = lifecycle_state(TriggerLifecycleStateKindV1::Time);
    let mut after = before.clone();
    after.retry_state = Some(TriggerLifecycleRetryStateV1 {
        attempt: 1,
        next_time_ms: 130,
    });
    after.action_hash = Some(Hash::new(b"untrusted fixture retry action hash"));
    output.trigger_lifecycle = TriggerLifecycleEffectsV1::Complete {
        effects: vec![TriggerLifecycleEffectV1 {
            ordinal: 0,
            trigger_id: trigger().trigger_id,
            origin: TriggerLifecycleOriginV1::InvocationPolicy {
                callback_index: 0,
                policy: TriggerLifecyclePolicyV1::AdvanceTimeRetry,
            },
            change: TriggerLifecycleChangeV1::Update { before, after },
        }],
    };
    row.validate_structure(8, &[]).unwrap();
    assert!(!row.is_output_limit_rejection());
    for fault in 0..5 {
        let mut invalid = row.clone();
        let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut invalid);
        match fault {
            0 => effects[0].trigger_id = "foreign-policy".parse().unwrap(),
            1 => {
                effects[0].origin = TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 1,
                    policy: TriggerLifecyclePolicyV1::AdvanceTimeRetry,
                }
            }
            2 => {
                effects[0].origin = TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 0,
                    policy: TriggerLifecyclePolicyV1::ClearTimeRetry,
                }
            }
            3 => {
                effects[0].origin = TriggerLifecycleOriginV1::Derived {
                    callback_index: Some(0),
                }
            }
            _ => {
                if let TriggerLifecycleChangeV1::Update { before, .. } = &mut effects[0].change {
                    before.action_hash = Some(Hash::new(b"foreign use state"));
                }
            }
        }
        assert!(invalid.validate_structure(8, &[]).is_err(), "fault {fault}");
    }
}

#[test]
fn lifecycle_invalid_filter_height_retry_and_hash_shapes_refuse() {
    for fault in 0..5 {
        let mut effect = lifecycle_insert();
        let TriggerLifecycleChangeV1::Insert { after } = &mut effect.change else {
            unreachable!()
        };
        match fault {
            0 => after.action_hash = Some(Hash::prehashed([0; Hash::LENGTH])),
            1 => after.registered_at_height = None,
            2 => after.registered_at_height = Some(9),
            3 => after.kind = TriggerLifecycleStateKindV1::Pipeline,
            _ => {
                after.retry_state = Some(TriggerLifecycleRetryStateV1 {
                    attempt: 0,
                    next_time_ms: 120,
                })
            }
        }
        let trace = TriggerLifecycleEffectsV1::Complete {
            effects: vec![effect],
        };
        assert!(
            trace
                .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], false)
                .and_then(|()| lifecycle_fixture_validate_frames(&trace))
                .is_err(),
            "fault {fault}"
        );
    }
}

#[test]
fn lifecycle_successor_trace_does_not_change_execution_call_identity() {
    let mut row = lifecycle_network();
    let call = row.execution_call_hash(proposal(), &[input()]).unwrap();
    let before = norito::encode_canonical(&row).unwrap();
    *lifecycle_mut(&mut row) = TriggerLifecycleEffectsV1::Complete {
        effects: vec![lifecycle_insert()],
    };
    row.validate_structure(8, &[input()]).unwrap();
    assert_eq!(
        row.execution_call_hash(proposal(), &[input()]).unwrap(),
        call
    );
    assert_ne!(norito::encode_canonical(&row).unwrap(), before);
}

#[test]
fn lifecycle_historical_row_payloads_preserved_but_never_claim_coverage() {
    // Exact original enum shape and schema name. These controls are required
    // before activation; they are not a substitute for captured historical
    // block/proof/checkpoint replay under the actual old/new native decoders.
    #[derive(Encode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::block::execution_output::ExecutionOutputV1")]
    #[norito(
        tag = "kind",
        content = "detail",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    enum HistoricalExecutionOutputV1 {
        Network(NetworkExecutionOutputV1),
        Pipeline(PipelineExecutionOutputV1),
        Time(TimeExecutionOutputV1),
    }
    for row in [network(), pipeline(), time()] {
        let prior = match &row {
            ExecutionOutputV1::Network(value) => {
                HistoricalExecutionOutputV1::Network(value.clone())
            }
            ExecutionOutputV1::Pipeline(value) => {
                HistoricalExecutionOutputV1::Pipeline(value.clone())
            }
            ExecutionOutputV1::Time(value) => HistoricalExecutionOutputV1::Time(value.clone()),
            _ => unreachable!("historical fixture"),
        };
        assert_eq!(row.encode(), prior.encode());
        let retained_layout = norito::encode_canonical(&prior).unwrap();
        assert_eq!(norito::encode_canonical(&row).unwrap(), retained_layout);
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&retained_layout).unwrap(),
            row
        );
        row.validate_structure(8, &[input()]).unwrap();
        assert!(row.trigger_lifecycle().is_none());
        assert!(row.require_trigger_lifecycle().is_err());
    }
}

#[test]
fn lifecycle_complete_range_refuses_historical_mixed_and_duplicate_rows() {
    let complete = vec![lifecycle_network(), lifecycle_pipeline(), lifecycle_time()];
    validate_execution_outputs_with_lifecycle_v1(
        &complete,
        proposal(),
        8,
        &[input()],
        lifecycle_fixture_limits(),
        &mut LifecycleActionCodecOracle::default(),
    )
    .unwrap();
    for index in 0..3 {
        let mut mixed = complete.clone();
        mixed[index] = [network(), pipeline(), time()][index].clone();
        // Ordinary historical shape is valid, but cannot establish complete coverage.
        validate_execution_outputs_v1(&mixed, proposal(), 8, &[input()]).unwrap();
        assert!(
            validate_execution_outputs_with_lifecycle_v1(
                &mixed,
                proposal(),
                8,
                &[input()],
                lifecycle_fixture_limits(),
                &mut LifecycleActionCodecOracle::default()
            )
            .is_err()
        );
    }
    let mut duplicate = complete.clone();
    duplicate.insert(1, lifecycle_network());
    assert!(
        validate_execution_outputs_with_lifecycle_v1(
            &duplicate,
            proposal(),
            8,
            &[input()],
            lifecycle_fixture_limits(),
            &mut LifecycleActionCodecOracle::default()
        )
        .is_err()
    );
    let mut unordered = complete;
    unordered.swap(1, 2);
    assert!(
        validate_execution_outputs_with_lifecycle_v1(
            &unordered,
            proposal(),
            8,
            &[input()],
            lifecycle_fixture_limits(),
            &mut LifecycleActionCodecOracle::default()
        )
        .is_err()
    );
}

#[test]
fn lifecycle_successor_keeps_exact_result_and_prebody_call_identity() {
    for (historical, complete) in [
        (network(), lifecycle_network()),
        (pipeline(), lifecycle_pipeline()),
        (time(), lifecycle_time()),
    ] {
        assert_eq!(historical.result(), complete.result());
        assert_eq!(historical.completions(), complete.completions());
        assert_eq!(
            historical
                .execution_call_hash(proposal(), &[input()])
                .unwrap(),
            complete
                .execution_call_hash(proposal(), &[input()])
                .unwrap()
        );
        assert_ne!(historical.encode(), complete.encode());
    }
}

fn lifecycle_renumber(trace: &mut TriggerLifecycleEffectsV1) {
    let TriggerLifecycleEffectsV1::Complete { effects } = trace;
    for (index, effect) in effects.iter_mut().enumerate() {
        effect.ordinal = u32::try_from(index).unwrap();
    }
}

#[test]
fn lifecycle_callback_mutation_requires_an_encountered_invoke() {
    let mut row = lifecycle_time();
    let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut row);
    let mut mutation = lifecycle_insert();
    mutation.trigger_id = "separate-body-created".parse().unwrap();
    mutation.origin = TriggerLifecycleOriginV1::Derived {
        callback_index: Some(0),
    };
    effects.insert(0, mutation);
    lifecycle_renumber(lifecycle_mut(&mut row));
    // Independent IDs ensure before-state continuity cannot mask this ordering bug.
    assert_eq!(
        row.validate_structure(8, &[]).unwrap_err(),
        "callback lifecycle effect precedes its Invoke"
    );
    let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut row);
    effects.swap(0, 1);
    lifecycle_renumber(lifecycle_mut(&mut row));
    row.validate_structure(8, &[]).unwrap();
}

#[test]
fn lifecycle_success_policy_requires_an_encountered_invoke() {
    let mut row = lifecycle_time();
    let mut depleted = lifecycle_state(TriggerLifecycleStateKindV1::Time);
    let mut depleted_action = lifecycle_fixture_action(&depleted.action);
    depleted_action.repeats = crate::trigger::action::Repeats::Exactly(0);
    assert!(depleted_action.repeats().is_depleted());
    depleted.action = lifecycle_frame(&depleted_action);
    let policy = TriggerLifecycleEffectV1 {
        ordinal: 0,
        trigger_id: "separate-depleted-entry".parse().unwrap(),
        origin: TriggerLifecycleOriginV1::InvocationPolicy {
            callback_index: 0,
            policy: TriggerLifecyclePolicyV1::RemoveDepleted,
        },
        change: TriggerLifecycleChangeV1::Remove { before: depleted },
    };
    let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut row);
    effects.insert(0, policy);
    lifecycle_renumber(lifecycle_mut(&mut row));
    assert_eq!(
        row.validate_structure(8, &[]).unwrap_err(),
        "successful lifecycle policy precedes its Invoke"
    );
    let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut row);
    effects.swap(0, 1);
    lifecycle_renumber(lifecycle_mut(&mut row));
    row.validate_structure(8, &[]).unwrap();
}

#[test]
fn lifecycle_network_root_effects_do_not_need_a_callback_invoke() {
    for origin in [
        TriggerLifecycleOriginV1::DirectNetworkInstruction { item_index: 0 },
        TriggerLifecycleOriginV1::Derived {
            callback_index: None,
        },
    ] {
        let mut row = lifecycle_network();
        let ExecutionOutputV1::NetworkWithLifecycle(output) = &mut row else {
            unreachable!()
        };
        output.execution.completions.push(completion(&trigger()));
        let mut first = lifecycle_insert();
        first.trigger_id = "network-root-before".parse().unwrap();
        first.origin = origin.clone();
        let mut last = first.clone();
        last.trigger_id = "network-root-after".parse().unwrap();
        output.trigger_lifecycle = TriggerLifecycleEffectsV1::Complete {
            effects: vec![first, lifecycle_by_call_use(0, trigger().trigger_id), last],
        };
        lifecycle_renumber(lifecycle_mut(&mut row));
        row.validate_structure(8, &[input()]).unwrap();
    }
}

fn lifecycle_by_call_use(index: u32, id: TriggerId) -> TriggerLifecycleEffectV1 {
    use crate::{
        events::execute_trigger::ExecuteTriggerEventFilter,
        transaction::Executable,
        trigger::action::{Action, Repeats},
    };
    let action = Action::new(
        Executable::Instructions(iroha_primitives::const_vec::ConstVec::new_empty()),
        Repeats::Exactly(1),
        input().authority_opt().unwrap().clone(),
        ExecuteTriggerEventFilter::new().for_trigger(id.clone()),
    )
    .unwrap();
    TriggerLifecycleEffectV1 {
        ordinal: 0,
        trigger_id: id,
        origin: TriggerLifecycleOriginV1::Derived {
            callback_index: Some(index),
        },
        change: TriggerLifecycleChangeV1::Invoke {
            callback_index: index,
            state: TriggerLifecycleStateV1 {
                kind: TriggerLifecycleStateKindV1::ByCall,
                registered_at_height: None,
                action_hash: None,
                action: lifecycle_frame(&action),
                retry_state: None,
            },
        },
    }
}

#[test]
fn lifecycle_invoke_encounter_order_preserves_allocation_gaps_and_nested_work() {
    for nested_index in [1, 3] {
        let mut row = lifecycle_time();
        let ExecutionOutputV1::TimeWithLifecycle(output) = &mut row else {
            unreachable!()
        };
        let nested_id: TriggerId = "nested-by-call".parse().unwrap();
        output.execution.completions.push(InvocationCompletionV1 {
            callback_index: nested_index,
            trigger_id: nested_id.clone(),
            outcome: TriggerCompletedOutcome::Success,
        });
        let Ok(steps) = &mut output.execution.result.0 else {
            unreachable!()
        };
        steps.push(DataTriggerStep {
            id: nested_id.clone(),
            instructions: step(),
        });
        let TriggerLifecycleEffectsV1::Complete { effects } = &mut output.trigger_lifecycle;
        effects.push(lifecycle_by_call_use(nested_index, nested_id));
        let mut nested_work = lifecycle_insert();
        nested_work.trigger_id = "nested-created-entry".parse().unwrap();
        nested_work.origin = TriggerLifecycleOriginV1::Derived {
            callback_index: Some(nested_index),
        };
        effects.push(nested_work);
        lifecycle_renumber(lifecycle_mut(&mut row));
        row.validate_structure(8, &[]).unwrap();

        let mut reversed = row.clone();
        let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut reversed);
        effects.swap(0, 1);
        lifecycle_renumber(lifecycle_mut(&mut reversed));
        assert_eq!(
            reversed.validate_structure(8, &[]).unwrap_err(),
            "lifecycle Invoke order differs from callback allocation"
        );

        let TriggerLifecycleEffectsV1::Complete { effects } = lifecycle_mut(&mut row);
        effects.swap(1, 2);
        lifecycle_renumber(lifecycle_mut(&mut row));
        assert_eq!(
            row.validate_structure(8, &[]).unwrap_err(),
            "callback lifecycle effect precedes its Invoke"
        );
    }
}

#[test]
fn lifecycle_rejected_policy_only_traces_do_not_require_an_invoke() {
    for (mut row, policy) in [
        (lifecycle_time(), TriggerLifecyclePolicyV1::AdvanceTimeRetry),
        (
            lifecycle_pipeline(),
            TriggerLifecyclePolicyV1::QuarantinePipeline,
        ),
    ] {
        let (result, failure_root, completions, trace, kind) = match &mut row {
            ExecutionOutputV1::TimeWithLifecycle(output) => (
                &mut output.execution.result,
                &mut output.execution.failure_root,
                &mut output.execution.completions,
                &mut output.trigger_lifecycle,
                TriggerLifecycleStateKindV1::Time,
            ),
            ExecutionOutputV1::PipelineWithLifecycle(output) => (
                &mut output.execution.result,
                &mut output.execution.failure_root,
                &mut output.execution.completions,
                &mut output.trigger_lifecycle,
                TriggerLifecycleStateKindV1::Pipeline,
            ),
            _ => unreachable!("internal rejection fixture"),
        };
        *result = rejected();
        *failure_root = Some(TriggerFailureRootV1::DeclaredInstructionProjection(step()));
        completions[0].outcome = TriggerCompletedOutcome::Failure("actual failure fixture".into());
        let before = lifecycle_state(kind);
        let change = if kind == TriggerLifecycleStateKindV1::Time {
            let mut after = before.clone();
            after.retry_state = Some(TriggerLifecycleRetryStateV1 {
                attempt: 1,
                next_time_ms: 130,
            });
            after.action_hash = Some(Hash::new(b"untrusted fixture retry action hash"));
            TriggerLifecycleChangeV1::Update { before, after }
        } else {
            TriggerLifecycleChangeV1::Remove { before }
        };
        *trace = TriggerLifecycleEffectsV1::Complete {
            effects: vec![TriggerLifecycleEffectV1 {
                ordinal: 0,
                trigger_id: trigger().trigger_id,
                origin: TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 0,
                    policy,
                },
                change,
            }],
        };
        assert!(
            row.require_trigger_lifecycle()
                .unwrap()
                .effects()
                .iter()
                .all(|effect| !matches!(&effect.change, TriggerLifecycleChangeV1::Invoke { .. }))
        );
        row.validate_structure(8, &[]).unwrap();
    }
}

#[test]
fn lifecycle_action_frame_retains_full_original_ivm_metadata_and_retry_content() {
    let mut state = lifecycle_state(TriggerLifecycleStateKindV1::Time);
    let mut original = lifecycle_fixture_action(&state.action);
    // Stored bytecode source fixture, never an assertion of valid/executed IVM code.
    original.executable =
        crate::transaction::Executable::Ivm(crate::transaction::IvmBytecode::from_compiled(vec![
            0xaa, 0xbb, 0xcc, 0xdd,
        ]));
    original.repeats = crate::trigger::action::Repeats::Exactly(7);
    original.metadata.insert(
        "complete-original".parse().unwrap(),
        iroha_primitives::json::Json::new("full metadata, not its hash"),
    );
    state.action = lifecycle_frame(&original);
    state.retry_state = Some(TriggerLifecycleRetryStateV1 {
        attempt: 1,
        next_time_ms: 120,
    });
    let trace = TriggerLifecycleEffectsV1::Complete {
        effects: vec![TriggerLifecycleEffectV1 {
            change: TriggerLifecycleChangeV1::Insert {
                after: state.clone(),
            },
            ..lifecycle_insert()
        }],
    };
    lifecycle_fixture_validate_frames(&trace).unwrap();
    let bytes = norito::encode_canonical(&trace).unwrap();
    let decoded: TriggerLifecycleEffectsV1 = norito::decode_canonical(&bytes).unwrap();
    let TriggerLifecycleChangeV1::Insert { after } = &decoded.effects()[0].change else {
        unreachable!()
    };
    assert_eq!(
        after.action.bytes,
        norito::encode_canonical(&original).unwrap()
    );
    assert_eq!(lifecycle_fixture_action(&after.action), original);
    assert_eq!(
        after.retry_state,
        Some(TriggerLifecycleRetryStateV1 {
            attempt: 1,
            next_time_ms: 120
        })
    );
    let json = norito::json::to_json(&state.action).unwrap();
    assert_eq!(
        norito::json::from_str::<TriggerLifecycleActionFrameV1>(&json).unwrap(),
        state.action
    );
    let codec_json = norito::json::to_json(&state.action.codec).unwrap();
    let bytes_json = norito::json::to_json(&state.action.bytes).unwrap();
    for malformed in [
        json.replace(&codec_json, r#"{"kind":"unknown"}"#),
        json.replace(&format!("\"codec\":{codec_json},"), ""),
        json.replace(&format!(",\"bytes\":{bytes_json}"), ""),
    ] {
        assert_ne!(
            malformed, json,
            "mutate an actual serialized required field"
        );
        assert!(norito::json::from_str::<TriggerLifecycleActionFrameV1>(&malformed).is_err());
    }
}

#[test]
fn lifecycle_action_frame_refuses_bad_envelope_and_noncanonical_payload_layout() {
    let original =
        lifecycle_fixture_action(&lifecycle_state(TriggerLifecycleStateKindV1::Time).action);
    let canonical = norito::encode_canonical(&original).unwrap();
    let alternative = {
        let _fixed = norito::core::DecodeFlagsGuard::enter(0);
        norito::core::to_bytes(&original).unwrap()
    };
    assert_ne!(canonical, alternative);
    let mut suffix = canonical.clone();
    suffix.push(0);
    let mut corrupt = canonical.clone();
    *corrupt.last_mut().unwrap() ^= 0x80;
    for (bytes, oracle_calls) in [
        (Vec::new(), 0),
        (canonical[..canonical.len() - 1].to_vec(), 0),
        (suffix, 0),
        (corrupt, 0),
        (norito::encode_canonical(&42_u64).unwrap(), 0),
        // Valid alternate-layout envelope alone must never be sufficient.
        (alternative, 1),
    ] {
        let mut effect = lifecycle_insert();
        let TriggerLifecycleChangeV1::Insert { after } = &mut effect.change else {
            unreachable!()
        };
        after.action.bytes = bytes;
        let trace = TriggerLifecycleEffectsV1::Complete {
            effects: vec![effect],
        };
        let mut oracle = LifecycleActionCodecOracle::default();
        assert!(
            trace
                .validate_action_frames(
                    &mut TriggerLifecycleActionFrameBudgetV1::new(lifecycle_fixture_limits()),
                    &mut oracle,
                )
                .is_err()
        );
        assert_eq!(oracle.calls, oracle_calls);
    }
}

#[test]
fn lifecycle_action_frame_limits_precede_payload_validation_and_span_rows() {
    let effect = lifecycle_insert();
    let TriggerLifecycleChangeV1::Insert { after } = &effect.change else {
        unreachable!()
    };
    let bytes = after.action.bytes.len() as u64;
    let trace = TriggerLifecycleEffectsV1::Complete {
        effects: vec![effect],
    };
    for limits in [
        TriggerLifecycleActionFrameLimitsV1 {
            max_frame_bytes: bytes - 1,
            max_total_frame_bytes: bytes,
            max_frames: 1,
        },
        TriggerLifecycleActionFrameLimitsV1 {
            max_frame_bytes: bytes,
            max_total_frame_bytes: bytes - 1,
            max_frames: 1,
        },
        TriggerLifecycleActionFrameLimitsV1 {
            max_frame_bytes: bytes,
            max_total_frame_bytes: bytes,
            max_frames: 0,
        },
    ] {
        let mut oracle = LifecycleActionCodecOracle::default();
        assert!(
            trace
                .validate_action_frames(
                    &mut TriggerLifecycleActionFrameBudgetV1::new(limits),
                    &mut oracle
                )
                .is_err()
        );
        assert_eq!(oracle.calls, 0);
    }
    let mut oracle = LifecycleActionCodecOracle::default();
    let mut budget =
        TriggerLifecycleActionFrameBudgetV1::new(TriggerLifecycleActionFrameLimitsV1 {
            max_frame_bytes: bytes,
            max_total_frame_bytes: bytes,
            max_frames: 1,
        });
    trace
        .validate_action_frames(&mut budget, &mut oracle)
        .unwrap();
    assert_eq!(oracle.calls, 1);
    assert!(
        trace
            .validate_action_frames(&mut budget, &mut oracle)
            .is_err()
    );
    assert_eq!(oracle.calls, 1);
    assert!(
        TriggerLifecycleEffectsV1::Complete {
            effects: Vec::new()
        }
        .validate_action_frames(&mut budget, &mut oracle)
        .is_err(),
        "a refused range cannot recover by substituting Complete[]"
    );
    let complete = vec![lifecycle_network(), lifecycle_pipeline(), lifecycle_time()];
    let mut oracle = LifecycleActionCodecOracle::default();
    let mut limits = lifecycle_fixture_limits();
    limits.max_frames = 1;
    assert!(
        validate_execution_outputs_with_lifecycle_v1(
            &complete,
            proposal(),
            8,
            &[input()],
            limits,
            &mut oracle,
        )
        .is_err()
    );
    assert_eq!(
        oracle.calls, 1,
        "the block range must not reset its frame counter per row"
    );
}

#[test]
fn lifecycle_action_frame_semantics_and_validator_refusals_cannot_be_bypassed() {
    for wrong_kind in [true, false] {
        let mut effect = lifecycle_insert();
        let TriggerLifecycleChangeV1::Insert { after } = &mut effect.change else {
            unreachable!()
        };
        if wrong_kind {
            after.kind = TriggerLifecycleStateKindV1::Pipeline;
        } else {
            let mut action = lifecycle_fixture_action(&after.action);
            action.retry_policy = None;
            after.action = lifecycle_frame(&action);
            after.retry_state = Some(TriggerLifecycleRetryStateV1 {
                attempt: 1,
                next_time_ms: 120,
            });
        }
        let trace = TriggerLifecycleEffectsV1::Complete {
            effects: vec![effect],
        };
        // Outer shape alone is explicitly non-admitting for Action semantics.
        trace
            .validate_structure(TriggerLifecycleOwnerV1::Network, 8, &[], false)
            .unwrap();
        assert!(lifecycle_fixture_validate_frames(&trace).is_err());
    }
    struct Refuse;
    impl TriggerLifecycleActionValidatorV1 for Refuse {
        fn validate_canonical_action(
            &mut self,
            _: &[u8],
            _: &[u8],
        ) -> Result<TriggerLifecycleActionSemanticsV1, String> {
            Err("actual bounded scanner refusal".into())
        }
    }
    let complete = vec![lifecycle_network(), lifecycle_pipeline(), lifecycle_time()];
    assert_eq!(
        validate_execution_outputs_with_lifecycle_v1(
            &complete,
            proposal(),
            8,
            &[input()],
            lifecycle_fixture_limits(),
            &mut Refuse,
        )
        .unwrap_err(),
        "actual bounded scanner refusal"
    );
}

#[test]
fn lifecycle_retry_json_requires_both_exact_fields_and_rejects_unknowns() {
    for json in [
        "{}",
        r#"{"attempt":1}"#,
        r#"{"next_time_ms":120}"#,
        r#"{"attempt":1,"next_time_ms":120,"extra":0}"#,
        r#"{"attempt":null,"next_time_ms":120}"#,
        r#"{"attempt":1,"next_time_ms":null}"#,
        r#"{"attempt":-1,"next_time_ms":120}"#,
        r#"{"attempt":1,"next_time_ms":-1}"#,
        r#"[1,120]"#,
    ] {
        assert!(
            norito::json::from_str::<TriggerLifecycleRetryStateV1>(json).is_err(),
            "{json}"
        );
    }
    let actual = TriggerLifecycleRetryStateV1 {
        attempt: 1,
        next_time_ms: 120,
    };
    let json = norito::json::to_json(&actual).unwrap();
    assert_eq!(
        norito::json::from_str::<TriggerLifecycleRetryStateV1>(&json).unwrap(),
        actual
    );
}
