//! Lexical checkpoint epoch reuse with independent native, certificate and caller budgets.

use super::super::tests::Fixture;
use super::*;
use crate::{block::CommitCertificate, sumeragi::epoch::validation_counts};
use norito::core::{DecodeAttemptErrorKind, DecodeBudgetContext, DecodeResourceError};
use std::cell::Cell;

const CHAIN: &str = "portable-finality-test";
const TEST_ALLOCATION_CEILING: usize = 64 * 1024 * 1024;

fn selected(fixture: &Fixture) -> (SumeragiFinalityVerifier, SumeragiFinalityCheckpoint) {
    let mut verifier = fixture.verifier();
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&fixture.second).unwrap();
    let checkpoint = verifier.export_checkpoint(&fixture.second).unwrap();
    (verifier, checkpoint)
}

// These finite test counters measure native admission, not physical pool custody. Every
// production inner decoder retains its original input-derived limits beneath this layer.
fn caller_limits(allocation: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        1024 * 1024,
        MAX_FINALITY_CHECKPOINT_BYTES,
        8 * 1024 * 1024,
        allocation,
        64,
    )
}

#[test]
fn checkpoint_epoch_work_is_lexical_and_standalone_proofs_remain_independent() {
    let fixture = Fixture::new();
    let (verifier, checkpoint) = selected(&fixture);
    let original = checkpoint.encode_canonical().unwrap();
    let expected = verifier.verify_retained_decision(checkpoint.tip()).unwrap();
    assert!(!norito::core::decode_limits_active());
    for _ in 0..2 {
        let before = validation_counts::calls();
        let (imported, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
            &checkpoint,
            &fixture.network,
            CHAIN,
        )
        .unwrap();
        assert_eq!(
            validation_counts::calls() - before,
            2,
            "one importer workspace miss plus the independently reconstructed signed genesis"
        );
        assert_eq!(tip.context_id(), expected.context_id());
        assert_eq!(
            tip.block().encode_wire().unwrap(),
            checkpoint.tip.block_wire
        );
        assert_eq!(tip.commitment(), expected.commitment());
        assert_eq!(
            imported.export_checkpoint(checkpoint.tip()).unwrap(),
            checkpoint
        );
    }
    // Neither public structural reading nor public retained authentication receives an
    // importer workspace. Their original result/core and successor/config scopes remain.
    let before = validation_counts::calls();
    checkpoint.tip().decode_checked().unwrap();
    assert_eq!(validation_counts::calls() - before, 2);
    let before = validation_counts::calls();
    verifier.verify_retained_decision(checkpoint.tip()).unwrap();
    assert_eq!(validation_counts::calls() - before, 4);
    assert_eq!(checkpoint.encode_canonical().unwrap(), original);
}

#[test]
fn warmed_import_epoch_work_never_replaces_fresh_roster_or_quorum_authentication() {
    let fixture = Fixture::new();
    let (verifier, checkpoint) = selected(&fixture);
    let original = checkpoint.encode_canonical().unwrap();
    let mut wrong_roster = checkpoint.clone();
    wrong_roster.tip.committee[0].proof_of_possession[0] ^= 1;
    let mut wrong_quorum = checkpoint.clone();
    let mut block = decode_framed_signed_block(&wrong_quorum.tip.block_wire).unwrap();
    let certificate = block.commit_certificate().unwrap();
    let header = certificate.consensus_header().to_vec();
    let result = certificate.result_preimage().to_vec();
    let availability = certificate.availability().to_vec();
    let mut quorum: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
    quorum.agg_sig.0[0] ^= 1;
    block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        header.clone(),
        norito::encode_canonical(&quorum).unwrap(),
        result.clone(),
        availability.clone(),
    )));
    wrong_quorum.tip.block_wire = block.encode_wire().unwrap();
    assert_eq!(wrong_quorum.tip.block_header, checkpoint.tip.block_header);
    assert_ne!(wrong_quorum.tip.block_wire, checkpoint.tip.block_wire);
    assert_eq!(wrong_quorum.decisions, checkpoint.decisions);
    let changed = block.commit_certificate().unwrap();
    assert_eq!(changed.consensus_header(), header);
    assert_eq!(changed.result_preimage(), result);
    assert_eq!(changed.availability(), availability);

    let calls = Cell::new(0_u32);
    for changed in [&wrong_roster, &wrong_quorum] {
        let bytes = changed.encode_canonical().unwrap();
        let expected = verifier
            .verify_retained_decision(changed.tip())
            .unwrap_err();
        let before = validation_counts::calls();
        let refusal = SumeragiFinalityVerifier::from_trusted_checkpoint_with_consumer(
            changed,
            &fixture.network,
            CHAIN,
            |_, _, _| calls.set(calls.get() + 1),
        )
        .unwrap_err();
        let FinalityReadError::Invalid(actual) = refusal else {
            panic!("{refusal:?}");
        };
        assert_eq!(actual, expected, "the original witness gate still refuses");
        assert_eq!(validation_counts::calls() - before, 2);
        assert_eq!(calls.get(), 0);
        assert_eq!(changed.encode_canonical().unwrap(), bytes);
    }
    let (_, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
        &checkpoint,
        &fixture.network,
        CHAIN,
    )
    .unwrap();
    assert_eq!(
        tip.block().encode_wire().unwrap(),
        checkpoint.tip.block_wire
    );
    assert_eq!(
        verifier.export_checkpoint(checkpoint.tip()).unwrap(),
        checkpoint
    );
    assert_eq!(checkpoint.encode_canonical().unwrap(), original);
}

#[test]
fn positive_outer_budget_keeps_original_independent_import_charges() {
    let fixture = Fixture::new();
    let (_, checkpoint) = selected(&fixture);
    let original = checkpoint.encode_canonical().unwrap();
    // An active owner deliberately selects the original independent path. Measure only
    // its full import; encoding/export and comparison remain outside both measured scopes.
    let baseline = DecodeBudgetContext::new(caller_limits(TEST_ALLOCATION_CEILING));
    let calls = Cell::new(0_u32);
    let consume = |_: &SumeragiFinalityCheckpoint,
                   verifier: SumeragiFinalityVerifier,
                   tip: VerifiedSumeragiBlock| {
        calls.set(calls.get() + 1);
        (verifier, tip)
    };
    let before = validation_counts::calls();
    let (expected_verifier, expected_tip) = baseline
        .with(|| {
            SumeragiFinalityVerifier::from_trusted_checkpoint_with_consumer(
                &checkpoint,
                &fixture.network,
                CHAIN,
                consume,
            )
        })
        .unwrap();
    assert_eq!(validation_counts::calls() - before, 7);
    let charge = baseline.consumed_allocated_bytes();
    assert!(charge > 0);
    let cap = usize::try_from(charge).unwrap();
    assert!(cap < TEST_ALLOCATION_CEILING);
    let strict = DecodeBudgetContext::new(caller_limits(cap));
    assert_eq!(calls.get(), 1);
    let before = validation_counts::calls();
    let (imported, tip) = strict
        .with(|| {
            SumeragiFinalityVerifier::from_trusted_checkpoint_with_consumer(
                &checkpoint,
                &fixture.network,
                CHAIN,
                consume,
            )
        })
        .unwrap();
    assert_eq!(validation_counts::calls() - before, 7);
    assert_eq!(strict.consumed_allocated_bytes(), charge);
    assert_eq!(calls.get(), 2);
    assert_eq!(tip.context_id(), expected_tip.context_id());
    assert_eq!(
        tip.block().encode_wire().unwrap(),
        checkpoint.tip.block_wire
    );
    assert_eq!(tip.commitment(), expected_tip.commitment());
    assert_eq!(
        imported.export_checkpoint(checkpoint.tip()).unwrap(),
        checkpoint
    );
    assert_eq!(
        expected_verifier
            .export_checkpoint(checkpoint.tip())
            .unwrap(),
        checkpoint
    );
    assert_eq!(checkpoint.encode_canonical().unwrap(), original);
}

#[test]
fn positive_outer_refusal_keeps_native_provenance_and_original_source_retry() {
    let fixture = Fixture::new();
    let (_, checkpoint) = selected(&fixture);
    let original = checkpoint.encode_canonical().unwrap();
    // Measure the actual original bounds prefix, including both optional memo insertions.
    // A total-success-minus-one cap could merely decline optional retention and still pass.
    let prefix = DecodeBudgetContext::new(caller_limits(TEST_ALLOCATION_CEILING));
    prefix.with(|| checkpoint.validate_bounds()).unwrap();
    let prefix_charge = prefix.consumed_allocated_bytes();
    assert!(prefix_charge > 0);
    let cap = usize::try_from(prefix_charge)
        .unwrap()
        .checked_add(1)
        .unwrap();
    assert!(cap < TEST_ALLOCATION_CEILING);
    let expected_budget = DecodeBudgetContext::new(caller_limits(cap));
    let expected = expected_budget
        .with(|| {
            checkpoint.validate_bounds().unwrap();
            need(
                checkpoint.network_id == fixture.network && checkpoint.chain_id == CHAIN,
                "checkpoint differs from independently selected network or chain",
            )
            .unwrap();
            norito::core::with_decode_limits_scope(
                norito::canonical_decode_limits(checkpoint.genesis_wire.len()),
                || decode_framed_signed_block(&checkpoint.genesis_wire),
            )
        })
        .unwrap_err();
    assert_eq!(expected.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    let expected_resource = expected.into_error().decode_resource_error().unwrap();
    assert!(matches!(
        expected_resource,
        DecodeResourceError::TotalAllocationExceeded { attempted, limit }
            if attempted > limit && limit == cap as u64
    ));
    let actual_budget = DecodeBudgetContext::new(caller_limits(cap));
    let calls = Cell::new(0_u32);
    let refusal = actual_budget
        .with(|| {
            SumeragiFinalityVerifier::from_trusted_checkpoint_with_consumer(
                &checkpoint,
                &fixture.network,
                CHAIN,
                |_, _, _| calls.set(calls.get() + 1),
            )
        })
        .unwrap_err();
    let FinalityReadError::DecodeResource(actual) = refusal else {
        panic!("{refusal:?}");
    };
    assert_eq!(actual.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(
        actual.into_error().decode_resource_error(),
        Some(expected_resource)
    );
    let consumed = actual_budget.consumed_allocated_bytes();
    assert!(consumed >= prefix_charge && consumed > 0);
    assert_eq!(consumed, expected_budget.consumed_allocated_bytes());
    assert_eq!(calls.get(), 0);
    assert_eq!(checkpoint.encode_canonical().unwrap(), original);
    let (imported, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_consumer(
        &checkpoint,
        &fixture.network,
        CHAIN,
        |_, verifier, tip| {
            calls.set(calls.get() + 1);
            (verifier, tip)
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        tip.block().encode_wire().unwrap(),
        checkpoint.tip.block_wire
    );
    assert_eq!(
        imported.export_checkpoint(checkpoint.tip()).unwrap(),
        checkpoint
    );
    assert_eq!(actual_budget.consumed_allocated_bytes(), consumed);
    assert_eq!(checkpoint.encode_canonical().unwrap(), original);
}
