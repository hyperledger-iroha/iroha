//! Genuine native exact-quorum certificates over a synthetic complete ballot interval.

use super::*;
use crate::{
    corpus::{ClosureAnchorsV1, CorpusAccumulatorV1, build_closure_record_v1},
    test_support::{proposal_content_id, signed_fixture_ballot},
};
use iroha_data_model::{
    block::{
        BlockHeader, SignedBlock, decode_framed_signed_block,
        execution_output::NetworkExecutionOutputV1, output_budget::ExecutionOutputLimits,
    },
    governance::anonymous_ballot::{
        AnonymousBallotCorpusEntryV1, AnonymousBallotRejectionV1, CastingClockEstablishedV1,
        CastingOpenedV1, Digest512, GovernanceBlockStartOutputV1, NativeSlotAdvancedV1,
        SealedCredentialListBytesV1, SealedCredentialListPublishedV1, digests::envelope_digest_v1,
    },
    testing::native_finality::NativeFinalityFixture,
};

fn anchor(height: u64, timestamp_ms: u64) -> GovernanceAnchorV1 {
    GovernanceAnchorV1 {
        height,
        position: 0,
        timestamp_ms,
    }
}

fn positioned(at: GovernanceAnchorV1, event: Event) -> Vec<ExecutionOutputV1> {
    let receipt = Event::NativeSlotAdvanced(NativeSlotAdvancedV1 {
        target_height: at.height,
        cursor_height: at.height - 1,
        ticket_height: at.height,
    });
    [receipt, event]
        .into_iter()
        .map(|event| {
            ExecutionOutputV1::GovernanceBlockStart(GovernanceBlockStartOutputV1 {
                anchor: Anchor::Pass(at),
                event,
            })
        })
        .collect()
}

fn block_at(fixture: &NativeFinalityFixture, at: u64) -> SignedBlock {
    let mut header: BlockHeader = fixture.next_header();
    header.creation_time_ms = at;
    fixture.block_with_submitted_work(header)
}

fn certify(
    fixture: &mut NativeFinalityFixture,
    mut block: SignedBlock,
    mut rows: Vec<ExecutionOutputV1>,
) -> SumeragiFinalityProof {
    for index in 0..block.network_entrypoint_count() {
        rows.push(ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index: u32::try_from(index).unwrap(),
            result: TransactionResult::new(Ok(Vec::new())),
            completions: Vec::new(),
        }));
    }
    set_outputs(&mut block, rows);
    fixture.certify(block)
}

fn set_outputs(block: &mut SignedBlock, rows: Vec<ExecutionOutputV1>) {
    block
        .set_execution_outputs(
            rows,
            0,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            &ExecutionOutputLimits {
                max_outputs: 6,
                max_output_bytes: 128 * 1024,
                max_total_output_bytes: 256 * 1024,
                max_executed_wire_bytes: 2 * 1024 * 1024,
            },
        )
        .unwrap();
}

#[test]
fn full_native_interval_authenticates_the_seal_physical_inputs_clock_and_closure() {
    let mut native = NativeFinalityFixture::start_at("parliament-selective", 1000);
    let reader = || NativeFinalityFixture::start_at("parliament-selective", 1000).verifier();
    let (context, ballot) = signed_fixture_ballot(native.network_id());
    let h2 = native.next_header().creation_time_ms;
    let h3 = h2 + context.policy().binding.vote_notice_ms + 1;
    let h4 = h3 + 1;
    let h5 = h4 + context.policy().binding.casting_window_ms;
    let sealed_at = anchor(2, h2);
    let anchors = ClosureAnchorsV1 {
        opening: anchor(3, h3),
        casting_clock_height: 4,
        casting_clock_timestamp_ms: h4,
        closure: anchor(5, h5),
    };
    let seal = SealedCredentialListPublishedV1 {
        body_context: *context.body_context(),
        body_instance_id: *context.body_instance_id(),
        roster_root: Digest512::new([0x0a; 64]),
        list_digest: *context.list_digest(),
        registration_mapping_digest: Digest512::new([0x12; 64]),
        sealed_list_bytes: SealedCredentialListBytesV1::from_list(
            &context.context().sealed_list().unwrap(),
        )
        .unwrap(),
    };
    let entry = AnonymousBallotCorpusEntryV1::from_envelope(
        &ballot.envelope().unwrap(),
        envelope_digest_v1(ballot.as_bytes()).unwrap(),
        3,
        1,
    )
    .unwrap();
    let mut corpus = CorpusAccumulatorV1::new(&context).unwrap();
    corpus.append(&entry).unwrap();
    let closure = build_closure_record_v1(
        &context,
        proposal_content_id(),
        seal.registration_mapping_digest,
        &anchors,
        &corpus,
    )
    .unwrap();
    let request = FullVerificationRequestV1 {
        context: context.clone(),
        seal: seal.clone(),
        other_scopes: Vec::new(),
        proposal_content_id: proposal_content_id(),
        anchors,
        certified_result_root: Some(closure.result_root().unwrap()),
        closure_record: closure.clone(),
    };
    let mut proofs = vec![native.genesis_proof().clone()];
    let block = block_at(&native, h2);
    proofs.push(certify(
        &mut native,
        block,
        positioned(sealed_at, Event::SealedCredentialListPublished(seal)),
    ));
    let mut block = block_at(&native, h3);
    let mut inputs = block.external_entrypoints_slice().to_vec();
    inputs.push(TransactionEntrypoint::AnonymousBallot(ballot));
    block.set_external_entrypoints(inputs);
    let opened = CastingOpenedV1 {
        body_context: *context.body_context(),
        body_instance_id: *context.body_instance_id(),
        scope: *context.scope(),
        policy_hash: *context.policy_hash(),
        opening_at_ms: h3,
    };
    let mut early = opened.clone();
    early.opening_at_ms = h2;
    let early_opening = certify(
        &mut native.clone(),
        block.clone(),
        positioned(anchors.opening, Event::CastingOpened(early)),
    );
    proofs.push(certify(
        &mut native,
        block,
        positioned(anchors.opening, Event::CastingOpened(opened)),
    ));
    let clock = ExecutionOutputV1::GovernanceBlockStart(GovernanceBlockStartOutputV1 {
        anchor: Anchor::CastingClockPrelude {
            height: 4,
            timestamp_ms: h4,
        },
        event: Event::CastingClockEstablished(CastingClockEstablishedV1 {
            scope: *context.scope(),
            casting_window_ms: context.policy().binding.casting_window_ms,
            closing_ms: h5,
        }),
    });
    let block = block_at(&native, h4);
    proofs.push(certify(&mut native, block, vec![clock]));
    let block = block_at(&native, h5);
    proofs.push(certify(
        &mut native,
        block,
        positioned(anchors.closure, Event::BallotClosed(closure)),
    ));
    let fetch = |height: u64| Ok(proofs[usize::try_from(height - 1).unwrap()].clone());
    let verdict =
        verify_finalized_ballot_interval_v1(reader(), &request, sealed_at, fetch).unwrap();
    let FullVerificationV1::IndependentFullVerification(report) = verdict else {
        panic!("unexpected verdict: {verdict:?}")
    };
    assert_eq!(report.blocks, 3);
    assert_eq!(report.accepted, 1);
    assert_eq!(report.ballots_verified, 1);
    let wrong = NativeFinalityFixture::start("other-ballot-chain").verifier();
    assert!(matches!(
        verify_finalized_ballot_interval_v1(wrong, &request, sealed_at, |_| panic!(
            "foreign root must refuse before retrieval"
        )),
        Err(FinalizedIntervalErrorV1::Evidence { .. })
    ));
    let missing = verify_finalized_ballot_interval_v1(reader(), &request, sealed_at, |height| {
        if height == 4 {
            Err(ReplaySourceErrorV1::Missing { height })
        } else {
            fetch(height)
        }
    });
    assert!(matches!(
        missing,
        Err(FinalizedIntervalErrorV1::Source(
            ReplaySourceErrorV1::Missing { height: 4 }
        ))
    ));
    let skipped = verify_finalized_ballot_interval_v1(reader(), &request, sealed_at, |height| {
        fetch(if height == 4 { 5 } else { height })
    });
    assert!(matches!(
        skipped,
        Err(FinalizedIntervalErrorV1::Finality(_))
    ));
    // Replace the actual physical ballot output with another structurally valid Network
    // row while keeping the genuine original certificate, input commitment and header.
    let mut forged = proofs[2].clone();
    let mut altered = decode_framed_signed_block(&forged.block_wire).unwrap();
    let mut rows = altered.execution_outputs().to_vec();
    let Some(ExecutionOutputV1::Network(ballot_row)) = rows.last_mut() else {
        panic!("expected the ballot's Network output");
    };
    assert_eq!(ballot_row.input_index, 1);
    ballot_row.result = TransactionResult::new(Err(TransactionRejectionReason::AnonymousBallot(
        AnonymousBallotRejectionV1::DuplicateNullifier,
    )));
    set_outputs(&mut altered, rows);
    forged.block_wire = altered.encode_wire().unwrap();
    assert!(matches!(
        verify_finalized_ballot_interval_v1(reader(), &request, sealed_at, |height| {
            if height == 3 {
                Ok(forged.clone())
            } else {
                fetch(height)
            }
        }),
        Err(FinalizedIntervalErrorV1::Finality(_))
    ));
    assert!(matches!(
        verify_finalized_ballot_interval_v1(reader(), &request, sealed_at, |height| {
            if height == 3 {
                Ok(early_opening.clone())
            } else {
                fetch(height)
            }
        }),
        Err(FinalizedIntervalErrorV1::Evidence { height: 3, .. })
    ));
    let mut changed = request.clone();
    changed.certified_result_root = Some(Digest512::new([0x22; 64]));
    assert!(matches!(
        verify_finalized_ballot_interval_v1(reader(), &changed, sealed_at, fetch),
        Ok(FullVerificationV1::InvalidEvidence(
            crate::replay::InvalidEvidenceV1::ResultRoot
        ))
    ));
    let mut changed = request.clone();
    changed.seal.registration_mapping_digest = Digest512::new([0x44; 64]);
    assert!(matches!(
        verify_finalized_ballot_interval_v1(reader(), &changed, sealed_at, fetch),
        Err(FinalizedIntervalErrorV1::Evidence { height: 2, .. })
    ));
    let mut changed = request.clone();
    changed.anchors.closure.position += 1;
    assert!(matches!(
        verify_finalized_ballot_interval_v1(reader(), &changed, sealed_at, fetch),
        Ok(FullVerificationV1::InvalidEvidence(_)) | Err(FinalizedIntervalErrorV1::Evidence { .. })
    ));
}
