//! Exact source measurement, finalized-input and pre-tree rejection regressions.

use super::super::tests::transaction_wire_hash;
use super::super::{
    FastpqSourceExecutionEntryV1, FastpqSourceExecutionKindV1, FastpqSourceRouteV1,
    FastpqSourceStatementContextV1, derive_fastpq_ordinary_source_manifest_v1,
};
use super::*;
use crate::fastpq::{
    FastpqPublicInputsTemplate, poseidon_preimage_digest,
    quantity_statement::quantity_materializer_invocations_for_testing,
    quantity_statement_from_finalized_transcripts,
};
use fastpq_prover::{
    ProofSemantics,
    gadgets::public_transfer_statement::{
        TransferSmtBuildLimits, public_claims_from_transcripts,
        quantity_rows_for_public_preparation,
    },
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    DomainId, NetworkId,
    asset::AssetDefinitionId,
    fastpq::{FastpqPublicTransferTranscriptV1, TransferDeltaTranscript, TransferSmtWitness},
    nexus::DataSpaceId,
};
use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn limits() -> FastpqSourceStatementBuildLimits {
    FastpqSourceStatementBuildLimits {
        max_executed_entries: 8,
        max_transcripts: 8,
        max_deltas: 16,
        max_input_transcript_bytes: 1_000_000,
        max_statement_bytes: 1_000_000,
        max_total_statement_bytes: 2_000_000,
    }
}

fn delta(amount: u32, before: u32, receiver: u32) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        amount: Quantity::from(amount),
        from_balance_before: Quantity::from(before),
        from_balance_after: Quantity::from(before - amount),
        to_balance_before: Quantity::from(receiver),
        to_balance_after: Quantity::from(receiver + amount),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

fn transcript(deltas: Vec<TransferDeltaTranscript>) -> TransferTranscript {
    let batch_hash = Hash::new(b"budget source call");
    let digest = match deltas.as_slice() {
        [delta] => Some(poseidon_preimage_digest(delta, &batch_hash)),
        _ => None,
    };
    TransferTranscript {
        batch_hash,
        deltas,
        authority_digest: Hash::new(b"authority"),
        poseidon_preimage_digest: digest,
    }
}

fn archive(transcripts: Vec<TransferTranscript>) -> BTreeMap<Hash, Vec<TransferTranscript>> {
    BTreeMap::from([(transcripts[0].batch_hash, transcripts)])
}

fn source() -> FastpqSourceStatementContextV1 {
    FastpqSourceStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"budget genesis",
        ))),
        height: 19,
    }
}

fn entries(hash: Hash) -> Vec<FastpqSourceExecutionEntryV1> {
    vec![
        FastpqSourceExecutionEntryV1 {
            entry_hash: Hash::new(b"non-transfer"),
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Unrouted,
            dataspace_id: DataSpaceId::new(4),
        },
        FastpqSourceExecutionEntryV1 {
            entry_hash: hash,
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Unrouted,
            dataspace_id: DataSpaceId::new(4),
        },
    ]
}

fn encoded_statement(transcript: &TransferTranscript, slot: u64, perm: [u8; 32]) -> usize {
    encoded_bundle(std::slice::from_ref(transcript), slot, perm)
}

fn encoded_bundle(bundle: &[TransferTranscript], slot: u64, perm: [u8; 32]) -> usize {
    let inputs = FastpqPublicInputsTemplate {
        dsid: [255; 16],
        slot,
        old_root: [0; 32],
        new_root: [0; 32],
        perm_root: perm,
    }
    .with_tx_set_hash([255; 32]);
    let statement = quantity_statement_from_finalized_transcripts(
        inputs,
        bundle,
        source_statement_public_limits(limits()).unwrap(),
        TransferSmtBuildLimits::for_update_limit(32).unwrap(),
    )
    .unwrap();
    norito::encode_canonical(statement.statement())
        .unwrap()
        .len()
}

fn exact(usage: FastpqSourceTranscriptUsage, e: u32) -> FastpqSourceStatementBuildLimits {
    FastpqSourceStatementBuildLimits {
        max_executed_entries: e,
        max_transcripts: usage.transcripts,
        max_deltas: usage.deltas,
        max_input_transcript_bytes: usage.input_transcript_bytes,
        max_statement_bytes: usage.max_statement_bytes,
        max_total_statement_bytes: usage.total_statement_bytes,
    }
}

#[test]
fn exact_frames_and_all_six_caps_precede_every_private_constructor() {
    let map = archive(vec![
        transcript(vec![delta(3, 10, 0)]),
        transcript(vec![delta(3, 7, 3)]),
    ]);
    let entries = entries(*map.keys().next().unwrap());
    let calls = quantity_materializer_invocations_for_testing();
    let measured = measure_fastpq_source_statement_usage(2, &map, limits()).unwrap();
    assert_eq!(quantity_materializer_invocations_for_testing(), calls);
    assert_eq!(measured.transcripts, 2);
    assert_eq!(measured.deltas, 2);
    let expected_inputs: usize = map
        .values()
        .flatten()
        .map(|t| norito::encode_canonical(t).unwrap().len())
        .sum();
    assert_eq!(measured.input_transcript_bytes, expected_inputs);
    let sizes: Vec<_> = map
        .values()
        .map(|bundle| encoded_bundle(bundle, 0, [0; 32]))
        .collect();
    assert_eq!(measured.max_statement_bytes, *sizes.iter().max().unwrap());
    assert_eq!(measured.total_statement_bytes, sizes.iter().sum::<usize>());
    let bound = exact(measured, 2);
    let ordinary = derive_fastpq_ordinary_source_manifest_v1(
        source(),
        &entries,
        9,
        [7; 32],
        transaction_wire_hash(),
        &map,
        limits(),
    )
    .unwrap();
    let exact_result = derive_fastpq_ordinary_source_manifest_v1(
        source(),
        &entries,
        9,
        [7; 32],
        transaction_wire_hash(),
        &map,
        bound,
    )
    .unwrap();
    assert_eq!(ordinary, exact_result);
    for dimension in 0..6 {
        let mut low = bound;
        match dimension {
            0 => low.max_executed_entries -= 1,
            1 => low.max_transcripts -= 1,
            2 => low.max_deltas -= 1,
            3 => low.max_input_transcript_bytes -= 1,
            4 => low.max_statement_bytes -= 1,
            _ => low.max_total_statement_bytes -= 1,
        }
        let calls = quantity_materializer_invocations_for_testing();
        assert!(
            derive_fastpq_ordinary_source_manifest_v1(
                source(),
                &entries,
                9,
                [7; 32],
                transaction_wire_hash(),
                &map,
                low
            )
            .is_err(),
            "dimension {dimension}"
        );
        assert_eq!(
            quantity_materializer_invocations_for_testing(),
            calls,
            "dimension {dimension}"
        );
    }
}

#[test]
fn pending_none_digest_cannot_reserve_smaller_finalized_frames() {
    let finalized = transcript(vec![delta(3, 10, 0)]);
    let mut pending = finalized.clone();
    pending.poseidon_preimage_digest = None;
    assert!(
        norito::encode_canonical(&pending).unwrap().len()
            < norito::encode_canonical(&finalized).unwrap().len()
    );
    assert!(
        norito::encode_canonical(&FastpqPublicTransferTranscriptV1::from(&pending))
            .unwrap()
            .len()
            < norito::encode_canonical(&FastpqPublicTransferTranscriptV1::from(&finalized))
                .unwrap()
                .len()
    );
    let pending_map = archive(vec![pending]);
    let before = norito::encode_canonical(&pending_map).unwrap();
    let calls = quantity_materializer_invocations_for_testing();
    assert!(measure_fastpq_source_statement_usage(1, &pending_map, limits()).is_err());
    assert_eq!(quantity_materializer_invocations_for_testing(), calls);
    assert_eq!(norito::encode_canonical(&pending_map).unwrap(), before);
    let usage =
        measure_fastpq_source_statement_usage(1, &archive(vec![finalized.clone()]), limits())
            .unwrap();
    assert_eq!(
        usage.total_statement_bytes,
        encoded_statement(&finalized, u64::MAX, [255; 32])
    );
}

#[test]
fn context_field_values_never_change_canonical_frame_length() {
    let t = transcript(vec![delta(3, 10, 0)]);
    let expected = quantity_statement_frame_len_from_finalized_transcripts(
        std::slice::from_ref(&t),
        source_statement_public_limits(limits()).unwrap(),
    )
    .unwrap();
    for (slot, perm) in [
        (0, [0; 32]),
        (127, [1; 32]),
        (128, [127; 32]),
        (u64::MAX, [255; 32]),
    ] {
        assert_eq!(expected, encoded_statement(&t, slot, perm));
    }
}

#[test]
fn private_path_bytes_are_exact_and_never_enter_public_statement_size() {
    let baseline = transcript(vec![delta(3, 10, 0)]);
    let mut changed = baseline.clone();
    changed.deltas[0].from_smt_witness.siblings = vec![[9; 32]; 129];
    changed.deltas[0].to_smt_witness.path_bits = vec![255; 257];
    let a = measure_fastpq_source_statement_usage(1, &archive(vec![baseline]), limits()).unwrap();
    let map = archive(vec![changed.clone()]);
    let before = norito::encode_canonical(&map).unwrap();
    let b = measure_fastpq_source_statement_usage(1, &map, limits()).unwrap();
    assert!(b.input_transcript_bytes > a.input_transcript_bytes);
    assert_eq!(b.total_statement_bytes, a.total_statement_bytes);
    assert_eq!(
        b.input_transcript_bytes,
        norito::encode_canonical(&changed).unwrap().len()
    );
    assert_eq!(norito::encode_canonical(&map).unwrap(), before);
}

#[test]
fn multi_delta_repeated_keys_zero_and_self_transfers_keep_all_occurrences() {
    let multi = transcript(vec![delta(3, 10, 0), delta(3, 7, 3)]);
    let mut self_delta = delta(0, 4, 4);
    self_delta.to_account = self_delta.from_account.clone();
    let zero_self = transcript(vec![self_delta]);
    let map = archive(vec![multi.clone(), zero_self.clone(), zero_self.clone()]);
    let usage = measure_fastpq_source_statement_usage(1, &map, limits()).unwrap();
    assert_eq!(usage.transcripts, 3);
    assert_eq!(usage.deltas, 4);
    assert_eq!(
        usage.total_statement_bytes,
        encoded_bundle(&[multi, zero_self.clone(), zero_self], 9, [7; 32])
    );
    let malformed = transcript(vec![delta(3, 10, 0), delta(3, 10, 0)]);
    assert!(measure_fastpq_source_statement_usage(1, &archive(vec![malformed]), limits()).is_err());
}

#[test]
fn signed_512_maximum_and_decimal_scale_boundaries_match_final_producer() {
    let mut maximum_bytes = [255; 64];
    maximum_bytes[63] = 127;
    let maximum = BigInt::from_twos_bytes(&maximum_bytes).unwrap();
    for scale in [0, 28] {
        let max =
            Quantity::from_canonical_numeric(Numeric::try_new(maximum.clone(), scale).unwrap())
                .unwrap();
        let amount =
            Quantity::from_canonical_numeric(Numeric::try_new(1_u32, scale).unwrap()).unwrap();
        let mut d = delta(1, 2, 0);
        d.amount = amount.clone();
        d.from_balance_before = max.clone();
        d.from_balance_after = max.try_sub(&amount).unwrap();
        d.to_balance_before = Quantity::zero();
        d.to_balance_after = amount;
        let t = transcript(vec![d]);
        let usage =
            measure_fastpq_source_statement_usage(1, &archive(vec![t.clone()]), limits()).unwrap();
        assert_eq!(
            usage.total_statement_bytes,
            encoded_statement(&t, 9, [7; 32])
        );
    }
}

#[test]
fn canonical_layout_and_inherited_flags_restore_after_measurement() {
    let t = transcript(vec![delta(3, 10, 0)]);
    let map = archive(vec![t]);
    let expected = measure_fastpq_source_statement_usage(1, &map, limits()).unwrap();
    for flags in 0..=norito::core::supported_header_flags() {
        if flags & !norito::core::supported_header_flags() != 0 {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let before = norito::core::effective_decode_flags();
        assert_eq!(
            measure_fastpq_source_statement_usage(1, &map, limits()).unwrap(),
            expected
        );
        assert_eq!(norito::core::effective_decode_flags(), before);
    }
}

#[test]
fn disjoint_entry_merge_preserves_maximum_and_requires_owned_entry_count() {
    let t = transcript(vec![delta(3, 10, 0)]);
    let single =
        measure_fastpq_source_statement_usage(1, &archive(vec![t.clone()]), limits()).unwrap();
    let merged = single.checked_add_disjoint_entries(single).unwrap();
    let mut other = t.clone();
    other.batch_hash = Hash::new(b"another complete entry");
    other.poseidon_preimage_digest = Some(poseidon_preimage_digest(
        &other.deltas[0],
        &other.batch_hash,
    ));
    let map = BTreeMap::from([(t.batch_hash, vec![t]), (other.batch_hash, vec![other])]);
    let both = measure_fastpq_source_statement_usage(2, &map, limits()).unwrap();
    assert_eq!(merged, both);
    assert_eq!(merged.max_statement_bytes, single.max_statement_bytes);
    // Adding a third non-transfer entry changes E without changing statement usage.
    merged.check_limits(2, exact(merged, 2)).unwrap();
    assert!(merged.check_limits(3, exact(merged, 2)).is_err());
    assert_eq!(
        FastpqSourceTranscriptUsage::default()
            .checked_add_disjoint_entries(merged)
            .unwrap(),
        merged
    );
}

#[test]
fn checked_merge_overflow_never_mutates_its_operands() {
    for dimension in 0..4 {
        let mut huge = FastpqSourceTranscriptUsage::default();
        let mut one = huge;
        match dimension {
            0 => {
                huge.transcripts = usize::MAX;
                one.transcripts = 1;
            }
            1 => {
                huge.deltas = usize::MAX;
                one.deltas = 1;
            }
            2 => {
                huge.input_transcript_bytes = usize::MAX;
                one.input_transcript_bytes = 1;
            }
            _ => {
                huge.total_statement_bytes = usize::MAX;
                one.total_statement_bytes = 1;
            }
        }
        let before = huge;
        assert!(huge.checked_add_disjoint_entries(one).is_err());
        assert_eq!(huge, before);
    }
    let huge = FastpqSourceTranscriptUsage {
        max_statement_bytes: usize::MAX,
        ..FastpqSourceTranscriptUsage::default()
    };
    assert_eq!(huge.checked_add_disjoint_entries(huge).unwrap(), huge);
}

#[test]
fn empty_usage_keeps_nontransfer_e_and_allocates_no_private_constructor() {
    let calls = quantity_materializer_invocations_for_testing();
    let zero = FastpqSourceStatementBuildLimits {
        max_executed_entries: 2,
        max_transcripts: 0,
        max_deltas: 0,
        max_input_transcript_bytes: 0,
        max_statement_bytes: 0,
        max_total_statement_bytes: 0,
    };
    assert_eq!(
        measure_fastpq_source_statement_usage(2, &BTreeMap::new(), zero).unwrap(),
        FastpqSourceTranscriptUsage::default()
    );
    assert!(measure_fastpq_source_statement_usage(3, &BTreeMap::new(), zero).is_err());
    assert_eq!(quantity_materializer_invocations_for_testing(), calls);
}

#[test]
fn shared_row_construction_keeps_preallocation_update_cap_and_public_semantics() {
    let t = transcript(vec![delta(3, 10, 0)]);
    let limits = source_statement_public_limits(limits()).unwrap();
    let claims = public_claims_from_transcripts(std::slice::from_ref(&t), limits).unwrap();
    let mut marked = [0; 32];
    marked[31] = 1;
    let inputs = fastpq_prover::PublicInputs {
        dsid: [0; 16],
        slot: 0,
        old_root: marked,
        new_root: marked,
        perm_root: [0; 32],
        tx_set_hash: [0; 32],
    };
    let mut invalid = claims.clone();
    invalid[0].deltas[0].from_balance_after = Quantity::zero();
    assert!(matches!(
        quantity_rows_for_public_preparation(&invalid, inputs, limits, 1),
        Err(fastpq_prover::Error::VerifierLimitExceeded {
            limit: "max_transfer_smt_updates",
            ..
        })
    ));
    assert!(quantity_rows_for_public_preparation(&invalid, inputs, limits, 2).is_err());
    let rows = quantity_rows_for_public_preparation(&claims, inputs, limits, 2).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        fastpq_prover::gadgets::public_transfer_statement::prepare_quantity_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::AxtTransferClaim,
            limits
        )
        .is_ok()
    );
    assert!(
        fastpq_prover::gadgets::public_transfer_statement::prepare_quantity_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::AxtOpaqueEffect,
            limits
        )
        .is_err()
    );
    assert!(
        quantity_rows_for_public_preparation(&[], inputs, limits, 0)
            .unwrap()
            .is_empty()
    );
    assert!(
        fastpq_prover::gadgets::public_transfer_statement::prepare_quantity_public_transfers(
            &[],
            &[],
            inputs,
            ProofSemantics::AxtTransferClaim,
            limits
        )
        .is_err()
    );
    assert!(
        fastpq_prover::gadgets::public_transfer_statement::prepare_quantity_public_transfers(
            &[],
            &[],
            inputs,
            ProofSemantics::StateTransition,
            limits
        )
        .is_ok()
    );
}

#[test]
fn public_failures_and_output_caps_have_explicit_canonical_map_precedence() {
    let mut a = transcript(vec![delta(3, 10, 0)]);
    let mut b = a.clone();
    b.batch_hash = Hash::new(b"second budget source call");
    a.poseidon_preimage_digest = None;
    b.poseidon_preimage_digest = None;
    let map = BTreeMap::from([(a.batch_hash, vec![a]), (b.batch_hash, vec![b])]);
    let mut ordered = entries(*map.keys().next().unwrap());
    ordered[0].entry_hash = *map.keys().next_back().unwrap();
    let before = norito::encode_canonical(&map).unwrap();
    let calls = quantity_materializer_invocations_for_testing();
    let error = derive_fastpq_ordinary_source_manifest_v1(
        source(),
        &ordered,
        9,
        [7; 32],
        transaction_wire_hash(),
        &map,
        limits(),
    )
    .unwrap_err();
    assert!(error.contains(&format!("source bundle {}", map.keys().next().unwrap())));
    ordered.reverse();
    assert_eq!(
        derive_fastpq_ordinary_source_manifest_v1(
            source(),
            &ordered,
            9,
            [7; 32],
            transaction_wire_hash(),
            &map,
            limits()
        )
        .unwrap_err(),
        error
    );
    assert_eq!(quantity_materializer_invocations_for_testing(), calls);
    assert_eq!(norito::encode_canonical(&map).unwrap(), before);
}

#[test]
fn invalid_digest_policy_and_public_arithmetic_fail_before_private_constructor() {
    for change in 0..3 {
        let mut t = transcript(vec![delta(3, 10, 0)]);
        match change {
            0 => t.poseidon_preimage_digest = Some(Hash::new(b"wrong preimage")),
            1 => t.deltas[0].from_balance_after = Quantity::zero(),
            _ => t.deltas.push(delta(3, 7, 3)), // Multi-delta Some must not be repaired.
        }
        let map = archive(vec![t]);
        let entries = entries(*map.keys().next().unwrap());
        let before = norito::encode_canonical(&map).unwrap();
        let calls = quantity_materializer_invocations_for_testing();
        assert!(
            derive_fastpq_ordinary_source_manifest_v1(
                source(),
                &entries,
                9,
                [7; 32],
                transaction_wire_hash(),
                &map,
                limits()
            )
            .is_err()
        );
        assert_eq!(quantity_materializer_invocations_for_testing(), calls);
        assert_eq!(norito::encode_canonical(&map).unwrap(), before);
    }
}

#[test]
fn multisig_full_key_lengths_are_measured_without_fixed_account_size_assumptions() {
    use iroha_data_model::account::{AccountId, MultisigMember, MultisigPolicy};
    let mut d = delta(3, 10, 0);
    let policy = MultisigPolicy::new(
        1,
        vec![
            MultisigMember::new(ALICE_ID.controller().expect_single_signatory().clone(), 1)
                .unwrap(),
            MultisigMember::new(BOB_ID.controller().expect_single_signatory().clone(), 1).unwrap(),
        ],
    )
    .unwrap();
    d.from_account = AccountId::new_multisig(policy);
    let t = transcript(vec![d]);
    let usage =
        measure_fastpq_source_statement_usage(1, &archive(vec![t.clone()]), limits()).unwrap();
    assert_eq!(
        usage.total_statement_bytes,
        encoded_statement(&t, 9, [7; 32])
    );
}

#[test]
fn same_entry_fragments_require_complete_frame_measurement_before_private_work() {
    let first = transcript(vec![delta(3, 10, 0)]);
    let second = transcript(vec![delta(3, 7, 3)]);
    let singleton_max =
        encoded_statement(&first, 9, [7; 32]).max(encoded_statement(&second, 9, [7; 32]));
    let map = archive(vec![first, second]);
    let bundle_bytes = encoded_bundle(map.values().next().unwrap(), 9, [7; 32]);
    assert!(bundle_bytes > singleton_max);
    let usage = measure_fastpq_source_statement_usage(2, &map, limits()).unwrap();
    assert_eq!(usage.max_statement_bytes, bundle_bytes);
    assert_eq!(usage.total_statement_bytes, bundle_bytes);
    let before = norito::encode_canonical(&map).unwrap();
    let calls = quantity_materializer_invocations_for_testing();
    let low = FastpqSourceStatementBuildLimits {
        max_statement_bytes: singleton_max,
        ..limits()
    };
    let error = derive_fastpq_ordinary_source_manifest_v1(
        source(),
        &entries(*map.keys().next().unwrap()),
        9,
        [7; 32],
        transaction_wire_hash(),
        &map,
        low,
    )
    .unwrap_err();
    assert!(
        error.contains("canonical individual statement bytes"),
        "{error}"
    );
    assert_eq!(quantity_materializer_invocations_for_testing(), calls);
    assert_eq!(norito::encode_canonical(&map).unwrap(), before);
}

#[test]
fn complete_bundle_cumulative_cap_rejects_before_any_private_tree() {
    let first = vec![
        transcript(vec![delta(3, 10, 0)]),
        transcript(vec![delta(3, 7, 3)]),
    ];
    let second_hash = Hash::new(b"second cumulative complete entry");
    let mut second = first.clone();
    for item in &mut second {
        item.batch_hash = second_hash;
        item.poseidon_preimage_digest =
            Some(poseidon_preimage_digest(&item.deltas[0], &second_hash));
    }
    let first_hash = first[0].batch_hash;
    let expected_bytes = encoded_bundle(&first, 0, [0; 32]) + encoded_bundle(&second, 0, [0; 32]);
    let map = BTreeMap::from([(first_hash, first), (second_hash, second)]);
    let mut complete_entries = entries(first_hash);
    complete_entries.push(FastpqSourceExecutionEntryV1 {
        entry_hash: second_hash,
        ..complete_entries[1]
    });
    let usage = measure_fastpq_source_statement_usage(3, &map, limits()).unwrap();
    assert_eq!(usage.transcripts, 4);
    assert_eq!(usage.total_statement_bytes, expected_bytes);
    assert!(usage.max_statement_bytes < usage.total_statement_bytes);
    let bound = exact(usage, 3);
    let (manifest, leaves) = derive_fastpq_ordinary_source_manifest_v1(
        source(),
        &complete_entries,
        9,
        [7; 32],
        transaction_wire_hash(),
        &map,
        bound,
    )
    .unwrap();
    assert_eq!(
        (manifest.executed_entry_count, manifest.statement_count),
        (3, 2)
    );
    assert_eq!(
        leaves
            .iter()
            .map(|leaf| (leaf.entry_index, leaf.entry_transcript_count))
            .collect::<Vec<_>>(),
        vec![(1, 2), (2, 2)]
    );
    let calls = quantity_materializer_invocations_for_testing();
    let low = FastpqSourceStatementBuildLimits {
        max_total_statement_bytes: expected_bytes - 1,
        ..bound
    };
    let error = derive_fastpq_ordinary_source_manifest_v1(
        source(),
        &complete_entries,
        9,
        [7; 32],
        transaction_wire_hash(),
        &map,
        low,
    )
    .unwrap_err();
    assert!(error.contains("canonical total statement bytes"), "{error}");
    assert_eq!(quantity_materializer_invocations_for_testing(), calls);
}

#[test]
fn cross_transcript_discontinuity_order_and_duplicate_fail_before_private_work() {
    let original = archive(vec![
        transcript(vec![delta(3, 10, 0)]),
        transcript(vec![delta(3, 7, 3)]),
    ]);
    let original_hash = *original.keys().next().unwrap();
    for mutation in 0..3 {
        let mut map = original.clone();
        let bundle = map.get_mut(&original_hash).unwrap();
        match mutation {
            0 => bundle.swap(0, 1),
            1 => bundle.push(bundle[0].clone()),
            _ => {
                bundle[1] = transcript(vec![delta(3, 8, 3)]);
            }
        }
        let before = norito::encode_canonical(&map).unwrap();
        let calls = quantity_materializer_invocations_for_testing();
        let error = derive_fastpq_ordinary_source_manifest_v1(
            source(),
            &entries(original_hash),
            9,
            [7; 32],
            transaction_wire_hash(),
            &map,
            limits(),
        )
        .unwrap_err();
        assert!(
            error.contains("public repeated-key balances do not chain"),
            "mutation {mutation}: {error}"
        );
        assert_eq!(quantity_materializer_invocations_for_testing(), calls);
        assert_eq!(norito::encode_canonical(&map).unwrap(), before);
    }
}
