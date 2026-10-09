//! Tests of the TRON light client against the synthetic chain of `test_support::tron`.

use super::*;
use crate::{
    light_client::{
        SccpLcAdvanceV1, SccpLcEvidenceV1, SccpLcSegmentV1, SccpSourceProofV1,
        apply_advance_with_profiles, initialize_light_client_with_profiles, is_aged_with_profiles,
        profile::SccpChainProfilesV1, state::SccpLcMemoryStateV1,
        verify_equivocation_with_profiles, verify_proof_with_profiles,
    },
    test_support::tron::{
        SYNTHETIC_TRON_FIRST_PERIOD, SYNTHETIC_TRON_NEXT_BOUNDARY_HEIGHT, SyntheticTronChainV1,
        trigger_transaction,
    },
};
use iroha_data_model::sccp::light_client::SccpLcInitExpectationV1;

const P0: u64 = SYNTHETIC_TRON_FIRST_PERIOD;
const BOUNDARY: u64 = SYNTHETIC_TRON_NEXT_BOUNDARY_HEIGHT;
const CONTRACT: [u8; 21] = [
    0x41, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
    0x42, 0x42, 0x42, 0x42, 0x42,
];
const OWNER: [u8; 21] = [
    0x41, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07,
    0x07, 0x07, 0x07, 0x07, 0x07,
];

fn profiles(chain: &SyntheticTronChainV1) -> SccpChainProfilesV1 {
    SccpChainProfilesV1::genesis().with_tron(*chain.profile())
}

fn params() -> SccpLightClientParamsV1 {
    SccpLightClientParamsV1::defaults_for(NETWORK).expect("external")
}

/// Install a light client checkpointed at `height` and return the storage and Taira time.
fn installed(chain: &SyntheticTronChainV1, height: u64) -> (SccpLcMemoryStateV1, u64) {
    let now = chain.time_ms(height) + 60_000;
    let mut memory = SccpLcMemoryStateV1::new();
    let initial = initialize_light_client_with_profiles(
        &profiles(chain),
        &memory,
        NETWORK,
        SccpLcInitExpectationV1::Absent,
        &params(),
        &chain.bootstrap(height),
        now,
    )
    .expect("bootstrap verifies");
    memory.install(NETWORK, &initial);
    (memory, now)
}

fn advance(
    chain: &SyntheticTronChainV1,
    memory: &mut SccpLcMemoryStateV1,
    segments: Vec<TronSegmentV1>,
    now: u64,
) -> Result<SccpLcDeltaV1, SccpLcError> {
    let bytes = SccpLcAdvanceV1::Tron(TronLcAdvanceV1 { segments })
        .to_bytes()
        .expect("bounded");
    let delta = apply_advance_with_profiles(&profiles(chain), memory, NETWORK, &bytes, now)?;
    memory.apply(NETWORK, &delta);
    Ok(delta)
}

#[test]
fn bootstrap_installs_the_period_set_and_ages_with_the_period() {
    let chain = SyntheticTronChainV1::new([1; 32]);
    let (memory, now) = installed(&chain, 1_100);
    let light_client = memory.light_client(NETWORK).expect("installed");
    assert_eq!(light_client.head.latest_set_id, P0);
    assert_eq!(light_client.head.latest_finalized.source_height, 1_100);
    let deadline = weak_subjectivity_deadline_ms(chain.profile(), &light_client);
    let period_end = chain.profile().period_end_ms(P0).expect("end");
    assert_eq!(deadline, period_end + params().ws_bound_ms);
    assert!(!is_aged(chain.profile(), &light_client, now));
    assert_eq!(
        is_aged_with_profiles(&profiles(&chain), &memory, NETWORK, deadline),
        Ok(true)
    );
    assert_eq!(
        aged_supersessions(chain.profile(), &memory, &light_client)
            .expect("supersession")
            .len(),
        1
    );
    let mut wrong = TronLcBootstrapV1 {
        set: chain.witness_set(P0 - 1),
        checkpoint_header: chain.block(1_100).raw,
    };
    let bytes = crate::light_client::SccpLcBootstrapDataV1::Tron(wrong.clone())
        .to_bootstrap()
        .expect("frame");
    assert_eq!(
        crate::light_client::verify_bootstrap_with_profiles(
            &profiles(&chain),
            NETWORK,
            &params(),
            &bytes,
            now
        ),
        Err(TronLcError::InvalidBootstrap.into())
    );
    wrong.set.witnesses.truncate(10);
    assert!(check_set(&wrong.set).is_err());
}

#[test]
fn segments_make_their_newest_built_on_header_solid() {
    let chain = SyntheticTronChainV1::new([2; 32]);
    let (mut memory, _) = installed(&chain, 1_100);
    let now = chain.time_ms(1_160) + 1_000;
    let delta =
        advance(&chain, &mut memory, vec![chain.segment(1_100, 1_160)], now).expect("advances");
    // 19 distinct witnesses build on 1 141 (1 142..=1 160), none more on 1 142.
    let head = delta.head.expect("moved");
    assert_eq!(head.latest_finalized.source_height, 1_141);
    assert_eq!(head.latest_set_id, P0);
    assert!(delta.new_sets.is_empty());
    let again =
        advance(&chain, &mut memory, vec![chain.segment(1_100, 1_160)], now).expect("idempotent");
    assert!(again.is_empty());
    assert_eq!(
        advance(&chain, &mut memory, vec![chain.segment(1_100, 1_115)], now),
        Err(TronLcError::NotSolid {
            source_height: 1_100
        }
        .into())
    );
    let mut broken = chain.segment(1_100, 1_160);
    broken.headers.remove(5);
    assert_eq!(
        advance(&chain, &mut memory, vec![broken], now),
        Err(TronLcError::AncestryBroken { index: 5 }.into())
    );
}

#[test]
fn maintenance_boundaries_teach_the_next_set_and_evict_absent_witnesses() {
    let chain = SyntheticTronChainV1::new([3; 32]).with_roster(P0 + 1, 3, Some(0));
    let (mut memory, _) = installed(&chain, BOUNDARY - 100);
    let now = chain.time_ms(BOUNDARY + 90) + 1_000;
    // A segment entering the next period without its maintenance block cannot learn it.
    assert_eq!(
        advance(
            &chain,
            &mut memory,
            vec![chain.segment(BOUNDARY + 1, BOUNDARY + 90)],
            now
        ),
        Err(TronLcError::BoundaryNotObserved { period: P0 + 1 }.into())
    );
    assert_eq!(
        advance(
            &chain,
            &mut memory,
            vec![chain.segment(BOUNDARY - 10, BOUNDARY + 20)],
            now
        ),
        Err(TronLcError::IncompleteLearningWindow { period: P0 + 1 }.into())
    );
    let delta = advance(
        &chain,
        &mut memory,
        vec![chain.segment(BOUNDARY - 10, BOUNDARY + 90)],
        now,
    )
    .expect("learns the next set");
    assert_eq!(delta.new_sets.len(), 1);
    let learned = decode_set(&delta.new_sets[0]).expect("decodes");
    assert_eq!(learned, chain.witness_set(P0 + 1));
    assert_eq!(delta.new_sets[0].valid_from_source_height, BOUNDARY);
    assert_eq!(delta.superseded_sets[0].set_id, P0);
    let head = delta.head.expect("moved");
    assert_eq!(head.latest_set_id, P0 + 1);
    assert_eq!(head.latest_finalized.source_height, BOUNDARY + 71);
}

#[test]
fn periods_cannot_be_skipped() {
    let chain = SyntheticTronChainV1::new([4; 32]);
    let (mut memory, _) = installed(&chain, 1_100);
    let far = BOUNDARY + 7_200;
    let now = chain.time_ms(far + 90) + 1_000;
    assert_eq!(
        advance(
            &chain,
            &mut memory,
            vec![chain.segment(far - 10, far + 90)],
            now
        ),
        Err(TronLcError::UnlearnedPeriod { period: P0 + 1 }.into())
    );
}

fn burn_chain(
    seed: u8,
    height: u64,
    contract_ret: u64,
) -> (SyntheticTronChainV1, TronTransactionProofV1) {
    let call = TransferToTairaCallV1 {
        taira_recipient: vec![1; 40],
        token_amount: 5_000_000_000,
        expected_nonce: 3,
    };
    let transactions = vec![
        trigger_transaction(&OWNER, &CONTRACT, &[0xde, 0xad], 1, 0),
        trigger_transaction(&OWNER, &CONTRACT, &call.calldata(), contract_ret, 0),
        trigger_transaction(
            &OWNER,
            &CONTRACT,
            &crate::v1::evm_abi::void_frozen_calldata(4, 2),
            1,
            0,
        ),
    ];
    let chain = SyntheticTronChainV1::new([seed; 32]).with_transactions(height, transactions);
    let proof = chain.transaction_proof(height, 1);
    (chain, proof)
}

#[test]
fn solid_proofs_yield_the_transfer_call_and_void_calls() {
    let (chain, transaction) = burn_chain(5, 1_120, 1);
    let (memory, _) = installed(&chain, 1_100);
    let now = chain.time_ms(1_160) + 1_000;
    let proof = TronSourceProofV1 {
        anchor: TronProofAnchorV1::Solid(chain.segment(1_120, 1_140)),
        transaction,
    };
    let bytes = SccpSourceProofV1::Tron(proof.clone())
        .to_bytes()
        .expect("bounded");
    let verified = verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now)
        .expect("proof verifies");
    let SccpNormalizedEventV1::TransferCall {
        emitter,
        caller,
        call,
        locator,
    } = verified.event
    else {
        panic!("transfer call expected");
    };
    assert_eq!(emitter, SccpSourceEmitterV1::Tron(CONTRACT));
    assert_eq!(caller, OWNER[1..]);
    assert_eq!(call.expected_nonce, 3);
    assert_eq!(locator.source_height, 1_120);
    assert_eq!(locator.index_in_block, 1);
    assert_eq!(verified.checkpoints.len(), 1);
    let mut void = proof.clone();
    void.transaction = chain.transaction_proof(1_120, 2);
    let bytes = SccpSourceProofV1::Tron(void).to_bytes().expect("bounded");
    let verified = verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now)
        .expect("void verifies");
    assert!(matches!(
        verified.event,
        SccpNormalizedEventV1::Void {
            kind: SccpVoidKindV1::Frozen,
            first_nonce: 4,
            count: 2,
            ..
        }
    ));
    let mut junk = proof.clone();
    junk.transaction = chain.transaction_proof(1_120, 0);
    let bytes = SccpSourceProofV1::Tron(junk).to_bytes().expect("bounded");
    assert!(matches!(
        verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now),
        Err(SccpLcError::Tron(TronLcError::NotSccpCall(_)))
    ));
    let mut short = proof;
    short.anchor = TronProofAnchorV1::Solid(chain.segment(1_120, 1_130));
    let bytes = SccpSourceProofV1::Tron(short).to_bytes().expect("bounded");
    assert_eq!(
        verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now),
        Err(TronLcError::NotSolid {
            source_height: 1_120
        }
        .into())
    );
}

#[test]
fn non_canonical_or_out_of_range_calls_are_not_sccp_calls() {
    let transfer = TransferToTairaCallV1 {
        taira_recipient: vec![1; 40],
        token_amount: 5_000_000_000,
        expected_nonce: 3,
    }
    .calldata();
    let frozen = crate::v1::evm_abi::void_frozen_calldata(4, 2);
    let with_trailing_byte = |calldata: &[u8]| {
        let mut calldata = calldata.to_vec();
        calldata.push(0);
        calldata
    };
    let cases = [
        (with_trailing_byte(&transfer), AbiError::BadLength),
        (with_trailing_byte(&frozen), AbiError::BadLength),
        (
            crate::v1::evm_abi::void_frozen_calldata(4, 0),
            AbiError::BadVoidRange,
        ),
        (
            crate::v1::evm_abi::void_frozen_calldata(4, 257),
            AbiError::BadVoidRange,
        ),
        (
            crate::v1::evm_abi::void_frozen_calldata(u64::MAX, 2),
            AbiError::BadVoidRange,
        ),
    ];
    let height = 1_120;
    let transactions = cases
        .iter()
        .map(|(calldata, _)| trigger_transaction(&OWNER, &CONTRACT, calldata, 1, 0))
        .collect();
    let chain = SyntheticTronChainV1::new([7; 32]).with_transactions(height, transactions);
    let (memory, _) = installed(&chain, 1_100);
    let now = chain.time_ms(1_160) + 1_000;
    for (index, (_, error)) in cases.into_iter().enumerate() {
        let proof = TronSourceProofV1 {
            anchor: TronProofAnchorV1::Solid(chain.segment(height, 1_140)),
            transaction: chain.transaction_proof(height, index),
        };
        let bytes = SccpSourceProofV1::Tron(proof).to_bytes().expect("bounded");
        assert_eq!(
            verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now),
            Err(TronLcError::NotSccpCall(error).into()),
            "case {index}"
        );
    }
}

#[test]
fn failed_or_value_moving_calls_are_refused() {
    let (chain, transaction) = burn_chain(6, 1_120, 2);
    let (memory, _) = installed(&chain, 1_100);
    let now = chain.time_ms(1_160) + 1_000;
    let proof = TronSourceProofV1 {
        anchor: TronProofAnchorV1::Solid(chain.segment(1_120, 1_140)),
        transaction,
    };
    let bytes = SccpSourceProofV1::Tron(proof).to_bytes().expect("bounded");
    assert_eq!(
        verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now),
        Err(TronLcError::TransactionFailed.into())
    );
    let paying = trigger_transaction(&OWNER, &CONTRACT, &[1], 1, 5);
    assert!(matches!(
        decode_trigger_call(&paying),
        Err(TronLcError::TransactionFailed)
    ));
    assert!(matches!(
        decode_trigger_call(&[0x0a]),
        Err(TronLcError::MalformedTransaction)
    ));
}

#[test]
fn backfill_and_checkpoint_anchored_proofs_reach_older_blocks() {
    let (chain, transaction) = burn_chain(7, 1_050, 1);
    let (mut memory, now) = installed(&chain, 1_100);
    let bytes = SccpLcAdvanceV1::Backfill {
        segment: SccpLcSegmentV1::Tron(chain.raw_segment(1_060, 1_100)),
    }
    .to_bytes()
    .expect("bounded");
    let delta = apply_advance_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now)
        .expect("backfill");
    assert_eq!(delta.checkpoints[0].data.source_height, 1_060);
    memory.apply(NETWORK, &delta);
    let proof = TronSourceProofV1 {
        anchor: TronProofAnchorV1::Checkpoint(chain.raw_segment(1_050, 1_060)),
        transaction,
    };
    let bytes = SccpSourceProofV1::Tron(proof).to_bytes().expect("bounded");
    let verified = verify_proof_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now)
        .expect("checkpoint-anchored proof");
    assert_eq!(verified.event.locator().source_height, 1_050);
    let bytes = SccpLcAdvanceV1::Backfill {
        segment: SccpLcSegmentV1::Tron(chain.raw_segment(1_060, 1_099)),
    }
    .to_bytes()
    .expect("bounded");
    assert_eq!(
        apply_advance_with_profiles(&profiles(&chain), &memory, NETWORK, &bytes, now),
        Err(SccpLcError::UnknownCheckpoint {
            source_height: 1_099
        })
    );
}

#[test]
fn forks_signed_by_the_active_set_freeze_the_light_client() {
    let chain = SyntheticTronChainV1::new([8; 32]);
    let fork = chain.forked_at(1_130);
    let (memory, _) = installed(&chain, 1_100);
    let now = chain.time_ms(1_180) + 1_000;
    let honest = SccpLcEvidenceV1::Tron(TronLcEvidenceV1 {
        segment: chain.segment(1_120, 1_180),
    })
    .to_bytes()
    .expect("bounded");
    let forked = SccpLcEvidenceV1::Tron(TronLcEvidenceV1 {
        segment: fork.segment(1_120, 1_180),
    })
    .to_bytes()
    .expect("bounded");
    let reason = verify_equivocation_with_profiles(
        &profiles(&chain),
        &memory,
        NETWORK,
        &honest,
        &forked,
        now,
    )
    .expect("conflict");
    assert!(matches!(reason, SccpLcFreezeReasonV1::Equivocation(_)));
    assert_eq!(
        verify_equivocation_with_profiles(
            &profiles(&chain),
            &memory,
            NETWORK,
            &honest,
            &honest,
            now
        ),
        Err(SccpLcError::EvidenceNotConflicting)
    );
    let earlier = SccpLcEvidenceV1::Tron(TronLcEvidenceV1 {
        segment: chain.segment(1_100, 1_150),
    })
    .to_bytes()
    .expect("bounded");
    assert_eq!(
        verify_equivocation_with_profiles(
            &profiles(&chain),
            &memory,
            NETWORK,
            &honest,
            &earlier,
            now
        ),
        Err(SccpLcError::EvidenceNotConflicting)
    );
}

#[test]
fn protobuf_signature_and_merkle_codecs_are_strict() {
    let chain = SyntheticTronChainV1::new([9; 32]);
    let block = chain.block(10);
    let header = decode_header(&block.raw).expect("decodes");
    assert_eq!(header.id, block.id);
    assert_eq!(header_summary(&block.raw).expect("summary").number, 10);
    assert!(header_signer(&block.raw, &block.signature).is_ok());
    let mut unknown = block.raw.clone();
    unknown.extend_from_slice(&[0x60, 0x01]);
    assert_eq!(decode_header(&unknown), Err(TronLcError::MalformedHeader));
    let mut duplicate = block.raw.clone();
    duplicate.extend_from_slice(&[0x38, 0x01]);
    assert_eq!(decode_header(&duplicate), Err(TronLcError::MalformedHeader));
    let mut cursor = 0;
    assert_eq!(read_varint(&[0xac, 0x02], &mut cursor), Some(300));
    let mut cursor = 0;
    assert_eq!(read_varint(&[0x80, 0x00], &mut cursor), None);
    let mut signature = block.signature.clone();
    signature[64] = 2;
    assert_eq!(
        header_signer(&block.raw, &signature),
        Err(TronLcError::MalformedSignature)
    );
    let leaves = [[1_u8; 32], [2; 32], [3; 32]];
    let (root, branch) = merkle_root_and_branch(&leaves, 2).expect("tree");
    assert_eq!(merkle_root([3; 32], 2, 3, &branch), Some(root));
    assert_eq!(
        root,
        merkle_node(&merkle_node(&[1; 32], &[2; 32]), &[3; 32]),
        "the odd node is promoted, not duplicated"
    );
    assert_eq!(merkle_root([3; 32], 3, 3, &branch), None);
    let (single, empty) = merkle_root_and_branch(&[[5; 32]], 0).expect("tree");
    assert_eq!((single, empty.len()), ([5; 32], 0));
    let work = advance_work(&TronLcAdvanceV1 {
        segments: vec![chain.segment(1, 4)],
    });
    assert_eq!(work.secp256k1_recoveries, 4);
    assert_eq!(segment_work(&chain.raw_segment(1, 3)).native_headers, 3);
}

#[test]
fn field_readers_reject_only_another_wire_type() {
    let map = BTreeMap::from([
        (1, Wire::Varint(7)),
        (2, Wire::Bytes(b"ab")),
        (3, Wire::Fixed),
    ]);
    let malformed = || TronLcError::MalformedTransaction;
    assert_eq!(varint_of(&map, 1, malformed), Ok(Some(7)));
    assert_eq!(varint_of(&map, 4, malformed), Ok(None));
    for field in [2, 3] {
        assert_eq!(varint_of(&map, field, malformed), Err(malformed()));
    }
    assert_eq!(bytes_of(&map, 2, malformed), Ok(Some(&b"ab"[..])));
    assert_eq!(bytes_of(&map, 4, malformed), Ok(None));
    for field in [1, 3] {
        assert_eq!(bytes_of(&map, field, malformed), Err(malformed()));
    }
}

/// The high-`s` twin of a `v ∈ {0, 1}` signature: `(r, n − s)` with the parity flipped.
fn high_s(signature: &[u8]) -> Vec<u8> {
    let mut twin = signature.to_vec();
    let s: [u8; 32] = signature[32..64].try_into().expect("s");
    twin[32..64].copy_from_slice(&order_minus(&s));
    twin[64] ^= 1;
    twin
}

#[test]
fn java_tron_signature_forms_recover_the_same_witness() {
    let chain = SyntheticTronChainV1::new([10; 32]);
    let block = chain.block(1_105);
    let signer = header_signer(&block.raw, &block.signature).expect("canonical");
    let twin = high_s(&block.signature);
    assert!(
        twin[32..64] > SECP256K1_HALF_ORDER[..],
        "the twin is high-s"
    );
    assert_eq!(
        order_minus(&order_minus(&twin[32..64].try_into().expect("s"))).to_vec(),
        twin[32..64].to_vec()
    );
    for offset in [0_u8, 4, 27, 31] {
        for signature in [&block.signature, &twin] {
            let mut form = signature.clone();
            form[64] += offset;
            assert_eq!(header_signer(&block.raw, &form), Ok(signer), "v + {offset}");
        }
    }
    let malformed = Err(TronLcError::MalformedSignature);
    for v in [2_u8, 3, 6, 7, 8, 26, 29, 30, 33, 34, 35, 0xff] {
        let mut form = block.signature.clone();
        form[64] = v;
        assert_eq!(header_signer(&block.raw, &form), malformed, "v = {v}");
    }
    let mut zero_s = block.signature.clone();
    zero_s[32..64].fill(0);
    let mut order_s = block.signature.clone();
    order_s[32..64].copy_from_slice(&SECP256K1_ORDER);
    let mut order_r = block.signature.clone();
    order_r[..32].copy_from_slice(&SECP256K1_ORDER);
    for form in [zero_s, order_s, order_r, block.signature[..64].to_vec()] {
        assert_eq!(header_signer(&block.raw, &form), malformed);
    }

    // A segment in which every other witness signed with a high `s` advances exactly like the
    // canonical one.
    let (mut memory, _) = installed(&chain, 1_100);
    let now = chain.time_ms(1_160) + 1_000;
    let mut segment = chain.segment(1_100, 1_160);
    for header in segment.headers.iter_mut().step_by(2) {
        header.witness_signature = high_s(&header.witness_signature);
    }
    let delta = advance(&chain, &mut memory, vec![segment], now).expect("high-s headers count");
    assert_eq!(
        delta.head.expect("moved").latest_finalized.source_height,
        1_141
    );
}

/// The witness set `learned` lacks relative to `full`.
fn evicted(full: &TronWitnessSetV1, learned: &TronWitnessSetV1) -> Vec<Vec<u8>> {
    full.witnesses
        .iter()
        .filter(|witness| !learned.witnesses.contains(witness))
        .map(|witness| witness.account_address.clone())
        .collect()
}

#[test]
fn a_witness_missing_one_slot_of_the_window_is_still_learned() {
    // The witness scheduled at slot 8 203 misses its first-round slot and produces in the
    // second round (slot 8 230).
    let chain = SyntheticTronChainV1::new([11; 32])
        .with_roster(P0 + 1, 2, None)
        .with_missed_slots(&[BOUNDARY + 3]);
    let (mut memory, _) = installed(&chain, BOUNDARY - 100);
    let now = chain.time_ms(BOUNDARY + 100) + 1_000;
    let delta = advance(
        &chain,
        &mut memory,
        vec![chain.segment(BOUNDARY - 5, BOUNDARY + 100)],
        now,
    )
    .expect("learns the next set");
    let learned = decode_set(&delta.new_sets[0]).expect("decodes");
    assert_eq!(learned, chain.witness_set(P0 + 1));
    assert_eq!(delta.head.expect("moved").latest_set_id, P0 + 1);
}

#[test]
fn a_witness_missing_both_rounds_of_the_window_is_evicted() {
    let missing = BOUNDARY + 3;
    let chain = SyntheticTronChainV1::new([12; 32])
        .with_roster(P0 + 1, 2, None)
        .with_missed_slots(&[missing, missing + 27]);
    let absent = chain.scheduled_account(P0 + 1, missing);
    let (mut memory, _) = installed(&chain, BOUNDARY - 100);
    let now = chain.time_ms(BOUNDARY + 100) + 1_000;
    // The window spans 56 slots after the maintenance block: that witness's first two slots lie
    // inside it and its third outside.
    let window_slots = chain.profile().learning_window_ms() / 3_000;
    assert!(missing + 27 <= BOUNDARY + window_slots && missing + 54 > BOUNDARY + window_slots);
    let delta = advance(
        &chain,
        &mut memory,
        vec![chain.segment(BOUNDARY - 5, BOUNDARY + 100)],
        now,
    )
    .expect("learns the next set");
    let learned = decode_set(&delta.new_sets[0]).expect("decodes");
    assert_eq!(
        evicted(&chain.witness_set(P0 + 1), &learned),
        vec![absent.to_vec()]
    );
    assert_eq!(learned.witnesses.len(), 26);
}

#[test]
fn the_newest_window_header_names_a_witness_key() {
    let chain = SyntheticTronChainV1::new([13; 32]);
    let (memory, _) = installed(&chain, BOUNDARY - 100);
    let params = params();
    let ctx = Ctx {
        profile: chain.profile(),
        params: &params,
        now: chain.time_ms(BOUNDARY + 100) + 1_000,
        newest: P0,
    };
    let sets = Sets::new(&memory);
    let mut signed = ctx
        .signed(
            &chain.segment(BOUNDARY - 5, BOUNDARY + 100),
            "segment headers",
        )
        .expect("segment");
    // The first-round header of one witness recovers to an older key; its second-round header
    // carries the key it uses for the rest of the period.
    let first_round = 5 + 4;
    let stale_key = [0x41; ADDRESS_BYTES];
    let witness = signed[first_round].header.witness;
    let current_key = signed[first_round + 27].signer;
    assert_eq!(signed[first_round + 27].header.witness, witness);
    signed[first_round].signer = stale_key;
    let (set, valid_from, _) = learn_next(&ctx, &sets, &signed).expect("learns");
    assert_eq!(valid_from, BOUNDARY);
    let entry = set
        .witnesses
        .iter()
        .find(|entry| entry.account_address == witness)
        .expect("learned");
    assert_eq!(entry.signing_address, current_key.to_vec());
    // Reversed, the stale key is the newest one and wins.
    let mut reversed = signed.clone();
    reversed[first_round].signer = current_key;
    reversed[first_round + 27].signer = stale_key;
    let (set, _, _) = learn_next(&ctx, &sets, &reversed).expect("learns");
    let entry = set
        .witnesses
        .iter()
        .find(|entry| entry.account_address == witness)
        .expect("learned");
    assert_eq!(entry.signing_address, stale_key.to_vec());
}
