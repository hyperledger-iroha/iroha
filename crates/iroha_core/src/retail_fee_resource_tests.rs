//! Original decode refusals remain local and retry the exact protected DATA bytes.
//! These fixtures exercise custody and math only; they establish no Native grant.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::block::BlockHeader;
const START: u64 = 1_793_451_600_000;
fn local_limits(dimension: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        usize::MAX,
        if dimension == 0 { 0 } else { usize::MAX },
        if dimension == 1 { 0 } else { usize::MAX },
        if dimension == 2 { 0 } else { usize::MAX },
        usize::MAX,
    )
}
#[test]
fn protected_lineage_and_account_bytes_retry_after_original_local_decode_refusal() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(
        std::num::NonZeroU64::MIN,
        None,
        None,
        START,
        0,
    ));
    let mut stx = block.transaction();
    let owner = iroha_test_samples::ALICE_ID.clone();
    let original = vec![owner.clone()];
    let bytes = norito::to_bytes(&original).unwrap();
    let lineage_key = rekey_path(&owner, "lineage");
    stx.world
        .smart_contract_state
        .insert(lineage_key.clone(), bytes.clone());
    for dimension in 0..3 {
        let error = norito::with_decode_limits_scope(local_limits(dimension), || {
            predecessors(&stx.world, &owner)
        })
        .unwrap_err();
        assert!(
            matches!(error, ExecutionAttemptError::Deferred(_)),
            "dimension {dimension}: {error}"
        );
        assert_eq!(
            stx.world.smart_contract_state.get(&lineage_key),
            Some(&bytes)
        );
        assert_eq!(predecessors(&stx.world, &owner).unwrap(), original);
    }
    let record = RetailFeeAccountStateV1::enroll(owner.clone(), START, 1_000).unwrap();
    let record_bytes = norito::to_bytes(&record).unwrap();
    stx.world
        .smart_contract_state
        .insert(key(&owner), record_bytes.clone());
    let error =
        norito::with_decode_limits_scope(local_limits(0), || account_state(&stx.world, &owner))
            .unwrap_err();
    assert!(matches!(error, ExecutionAttemptError::Deferred(_)));
    assert_eq!(account_state(&stx.world, &owner).unwrap(), Some(record));
    stx.world
        .smart_contract_state
        .insert(lineage_key, vec![0xFF; 8]);
    let malformed =
        norito::with_decode_limits_scope(local_limits(2), || predecessors(&stx.world, &owner))
            .unwrap_err();
    assert!(
        matches!(malformed, ExecutionAttemptError::Rejected(_)),
        "malformed DATA is a completed rejection"
    );
}
#[test]
fn assessment_marker_keeps_local_refusal_distinct_from_wrong_grammar() {
    let assessment = RetailFeeAssessmentV1 {
        account_id: iroha_test_samples::ALICE_ID.clone(),
        retail_enrolled: false,
        billing_month_start_ms: START,
        policy_revision: 1,
        payments_used_before: 0,
        qualifying_payments: 1,
        fee_minor: 10,
        state_commitment: [7; 32],
        intent_hash: [9; 32],
        expires_at_ms: START + 1_000,
    };
    let original = norito::to_bytes(&assessment).unwrap();
    let marker = Log::new(
        Level::TRACE,
        format!("{ASSESSMENT_MARKER_PREFIX}{}", hex::encode(&original)),
    );
    let error =
        norito::with_decode_limits_scope(local_limits(0), || decode_assessment_marker(&marker))
            .unwrap_err();
    assert!(matches!(error, ExecutionAttemptError::Deferred(_)));
    assert_eq!(decode_assessment_marker(&marker).unwrap(), Some(assessment));
    let wrong_role = Log::new(Level::INFO, marker.msg.clone());
    let malformed = Log::new(Level::TRACE, format!("{ASSESSMENT_MARKER_PREFIX}ff"));
    for wrong in [wrong_role, malformed] {
        let error =
            norito::with_decode_limits_scope(local_limits(2), || decode_assessment_marker(&wrong))
                .unwrap_err();
        assert!(matches!(error, ExecutionAttemptError::Rejected(_)));
    }
}

#[test]
fn assessment_marker_bounds_the_complete_message_before_original_decode() {
    // Prefix plus the largest even hex body below the complete 4096-byte limit.
    let exact_hex_capacity = (4096 - ASSESSMENT_MARKER_PREFIX.len()) / 2;
    let bounded = Log::new(
        Level::TRACE,
        format!(
            "{ASSESSMENT_MARKER_PREFIX}{}",
            "00".repeat(exact_hex_capacity)
        ),
    );
    assert!(bounded.msg.len() <= 4096);
    let bounded_error = decode_assessment_marker(&bounded).unwrap_err();
    assert!(matches!(bounded_error, ExecutionAttemptError::Rejected(_)));
    assert!(
        matches!(
            &bounded_error,
            ExecutionAttemptError::Rejected(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(message)
            )) if message.contains("not canonical")
        ),
        "bounded malformed DATA reaches the original canonical decoder"
    );
    let oversized = Log::new(Level::TRACE, format!("{}00", bounded.msg));
    assert!(oversized.msg.len() > 4096);
    let oversized_error =
        norito::with_decode_limits_scope(local_limits(0), || decode_assessment_marker(&oversized))
            .unwrap_err();
    assert!(matches!(
        oversized_error,
        ExecutionAttemptError::Rejected(_)
    ));
    assert!(
        matches!(
            &oversized_error,
            ExecutionAttemptError::Rejected(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(message)
            )) if message.contains("bounded canonical lowercase hex")
        ),
        "oversized DATA is rejected before the local allocation scope"
    );
    let exact_limit = Log::new(Level::TRACE, format!("{}0", bounded.msg));
    assert_eq!(exact_limit.msg.len(), 4096);
    assert!(
        matches!(
            decode_assessment_marker(&exact_limit),
            Err(ExecutionAttemptError::Rejected(_))
        ),
        "the full limit does not waive even-length canonical hex"
    );
    assert_eq!(
        decode_assessment_marker(&Log::new(
            Level::TRACE,
            "ordinary application log".repeat(1000)
        ))
        .unwrap(),
        None,
        "non-marker logs remain outside this protocol"
    );
}
