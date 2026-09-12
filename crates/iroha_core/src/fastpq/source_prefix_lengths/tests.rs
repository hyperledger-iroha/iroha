//! Prefix frame parity, atomic failure, canonical layout and scope regressions.

use super::*;
use crate::fastpq::{
    poseidon_preimage_digest,
    quantity_statement::quantity_statement_frame_len_from_finalized_transcripts,
    quantity_statement_from_finalized_transcripts,
};
use fastpq_prover::gadgets::public_transfer_statement::{
    PublicTransferLimits, TransferSmtBuildLimits,
};
use iroha_data_model::{asset::AssetDefinitionId, fastpq::TransferSmtWitness};
use iroha_model_base::domain::DomainId;
use iroha_primitives::{bigint::BigInt, numeric::Numeric};
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn limits() -> PrefixLengthLimits {
    PrefixLengthLimits {
        max_deltas: 128,
        max_input_frame_bytes: 4_000_000,
        max_public_statement_frame_bytes: 4_000_000,
    }
}

fn hashes() -> (Hash, Hash) {
    (Hash::new(b"prefix-call"), Hash::new(b"prefix-authority"))
}

fn quantity(mantissa: u32, scale: u32) -> Quantity {
    Quantity::try_from_numeric(Numeric::new(mantissa, scale)).unwrap()
}

fn delta(amount: Quantity, before: Quantity, receiver: Quantity) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        from_balance_after: before.try_sub(&amount).unwrap(),
        to_balance_after: receiver.try_add(&amount).unwrap(),
        amount,
        from_balance_before: before,
        to_balance_before: receiver,
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

fn transcript(deltas: Vec<TransferDeltaTranscript>) -> TransferTranscript {
    let (batch_hash, authority_digest) = hashes();
    TransferTranscript {
        batch_hash,
        authority_digest,
        poseidon_preimage_digest: match deltas.as_slice() {
            [delta] => Some(poseidon_preimage_digest(delta, &batch_hash)),
            _ => None,
        },
        deltas,
    }
}

fn sizer() -> SourcePrefixFrameSizer {
    let (batch, authority) = hashes();
    SourcePrefixFrameSizer::new(batch, authority, limits()).unwrap()
}

fn reference(prefix: &TransferTranscript) -> PrefixFrameLengths {
    PrefixFrameLengths {
        deltas: prefix.deltas.len(),
        input_frame_bytes: norito::encode_canonical(prefix).unwrap().len(),
        public_statement_frame_bytes: quantity_statement_frame_len_from_finalized_transcripts(
            std::slice::from_ref(prefix),
            PublicTransferLimits::default(),
        )
        .unwrap(),
    }
}

#[test]
fn every_prefix_matches_real_frames_across_scale_growth_and_sequence_size_boundaries() {
    let mut sizing = sizer();
    let mut deltas = Vec::new();
    let mut sender = Quantity::from(100_u32);
    let mut receiver = Quantity::zero();
    assert_eq!(sizing.latest(), None);
    for index in 0..40 {
        let next = delta(quantity(1, index % 29), sender, receiver);
        sender = next.from_balance_after.clone();
        receiver = next.to_balance_after.clone();
        deltas.push(next);
        let prefix = transcript(deltas.clone());
        let measured = sizing
            .append(deltas.last().unwrap(), prefix.poseidon_preimage_digest)
            .unwrap();
        assert_eq!(measured, reference(&prefix), "prefix {}", index + 1);
        assert_eq!(sizing.latest(), Some(measured));
        assert_eq!(sizing.private_deltas.count(), deltas.len());
        assert_eq!(sizing.public_deltas.count(), deltas.len());
        assert_eq!(sizing.rows.count(), 2 * deltas.len());
    }
    assert!(sizing.rows.len() > 16_384);
}

#[test]
fn final_frame_matches_the_complete_strict_materializer_with_real_roots() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let second = delta(quantity(1, 2), quantity(9, 0), quantity(1, 0));
    let prefix = transcript(vec![first.clone(), second.clone()]);
    let mut sizing = sizer();
    sizing
        .append(
            &first,
            transcript(vec![first.clone()]).poseidon_preimage_digest,
        )
        .unwrap();
    let measured = sizing.append(&second, None).unwrap();
    let inputs = FastpqPublicInputs {
        dsid: [255; 16],
        slot: u64::MAX,
        old_root: [0; 32],
        new_root: [0; 32],
        perm_root: [255; 32],
        tx_set_hash: [127; 32],
    };
    let actual = quantity_statement_from_finalized_transcripts(
        inputs,
        &[prefix],
        PublicTransferLimits::default(),
        TransferSmtBuildLimits::for_update_limit(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        measured.public_statement_frame_bytes,
        norito::encode_canonical(actual.statement()).unwrap().len()
    );
    assert_ne!(actual.statement().public_inputs.old_root, [0; 32]);
}

#[test]
fn private_paths_change_only_original_frame_size_and_are_never_retained() {
    let original = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let mut changed = original.clone();
    changed.from_smt_witness.siblings = vec![[7; 32]; 128];
    changed.to_smt_witness.path_bits = vec![9; 16_384];
    let before = norito::encode_canonical(&changed).unwrap();
    let digest = transcript(vec![original.clone()]).poseidon_preimage_digest;
    let a = sizer().append(&original, digest).unwrap();
    let b = sizer().append(&changed, digest).unwrap();
    assert!(b.input_frame_bytes > a.input_frame_bytes);
    assert_eq!(
        b.public_statement_frame_bytes,
        a.public_statement_frame_bytes
    );
    assert_eq!(b, reference(&transcript(vec![changed.clone()])));
    assert_eq!(norito::encode_canonical(&changed).unwrap(), before);
}

#[test]
fn fixed_quantity_frames_cover_every_scale_and_high_limbs_while_original_claims_vary() {
    let mut bytes = [255_u8; 64];
    bytes[63] = 127;
    let maximum =
        Quantity::try_from_numeric(Numeric::new(BigInt::from_twos_bytes(&bytes).unwrap(), 0))
            .unwrap();
    for value in [Quantity::zero(), quantity(1, 0), maximum.clone()] {
        for scale in 0..=MAX_DECIMAL_SCALE {
            let units = FastpqQuantityUnits::from_quantity(&value, scale).unwrap();
            assert_eq!(encode_quantity_units_v1(&units).unwrap().len(), 141);
        }
    }
    let small = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let large = delta(quantity(1, 0), maximum, Quantity::zero());
    let small_prefix = transcript(vec![small.clone()]);
    let large_prefix = transcript(vec![large.clone()]);
    let a = sizer()
        .append(&small, small_prefix.poseidon_preimage_digest)
        .unwrap();
    let b = sizer()
        .append(&large, large_prefix.poseidon_preimage_digest)
        .unwrap();
    assert!(b.input_frame_bytes > a.input_frame_bytes);
    assert!(b.public_statement_frame_bytes > a.public_statement_frame_bytes);
    assert_eq!(a, reference(&small_prefix));
    assert_eq!(b, reference(&large_prefix));
}

#[test]
fn digest_shape_failures_and_delta_limits_leave_the_complete_sizing_state_unchanged() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let digest = transcript(vec![first.clone()]).poseidon_preimage_digest;
    let mut sizing = sizer();
    assert!(matches!(
        sizing.append(&first, None),
        Err(PrefixLengthError::DigestShape)
    ));
    assert_eq!(sizing.latest(), None);
    assert_eq!(sizing.private_deltas.count(), 0);
    let accepted = sizing.append(&first, digest).unwrap();
    assert!(matches!(
        sizing.append(&first, digest),
        Err(PrefixLengthError::DigestShape)
    ));
    sizing.limits.max_deltas = 1;
    assert!(matches!(
        sizing.append(&first, None),
        Err(PrefixLengthError::Deltas {
            actual: 2,
            maximum: 1
        })
    ));
    assert_eq!(sizing.latest(), Some(accepted));
    assert_eq!(sizing.private_deltas.count(), 1);
    assert_eq!(sizing.public_deltas.count(), 1);
    assert_eq!(sizing.rows.count(), 2);
}

#[test]
fn exact_inclusive_caps_pass_and_failed_input_or_public_counts_are_atomic() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let prefix = transcript(vec![first.clone()]);
    let exact = reference(&prefix);
    let (batch, authority) = hashes();
    let cap = PrefixLengthLimits {
        max_deltas: 1,
        max_input_frame_bytes: exact.input_frame_bytes,
        max_public_statement_frame_bytes: exact.public_statement_frame_bytes,
    };
    let mut accepted = SourcePrefixFrameSizer::new(batch, authority, cap).unwrap();
    assert_eq!(
        accepted
            .append(&first, prefix.poseidon_preimage_digest)
            .unwrap(),
        exact
    );
    for input in [true, false] {
        let mut low = cap;
        if input {
            low.max_input_frame_bytes -= 1;
        } else {
            low.max_public_statement_frame_bytes -= 1;
        }
        let mut failed = SourcePrefixFrameSizer::new(batch, authority, low).unwrap();
        let error = failed
            .append(&first, prefix.poseidon_preimage_digest)
            .unwrap_err();
        assert!(matches!(error, PrefixLengthError::Input { .. }) == input);
        assert!(matches!(error, PrefixLengthError::Public { .. }) != input);
        assert_eq!(failed.latest(), None);
        assert_eq!(failed.private_deltas.count(), 0);
        assert_eq!(failed.public_deltas.count(), 0);
        assert_eq!(failed.rows.count(), 0);
        failed.limits = cap;
        assert_eq!(
            failed
                .append(&first, prefix.poseidon_preimage_digest)
                .unwrap(),
            exact
        );
    }
}

#[test]
fn canonical_layout_is_explicit_and_ambient_layout_is_restored() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let prefix = transcript(vec![first.clone()]);
    let expected = reference(&prefix);
    for flags in [0, header_flags::PACKED_SEQ | header_flags::PACKED_STRUCT] {
        let _ambient = DecodeFlagsGuard::enter(flags);
        let before = norito::core::encoded_payload_len(&vec![1_u32, 2]).unwrap();
        let actual = sizer()
            .append(&first, prefix.poseidon_preimage_digest)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            norito::core::encoded_payload_len(&vec![1_u32, 2]).unwrap(),
            before
        );
        assert!(matches!(
            canonical_flags(flags),
            Err(PrefixLengthError::UnsupportedLayout)
        ));
    }
}

#[test]
fn sizing_a_malformed_chronology_does_not_grant_semantic_validation() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let malformed = delta(Quantity::zero(), quantity(91, 1), quantity(1, 0));
    let mut sizing = sizer();
    sizing
        .append(
            &first,
            transcript(vec![first.clone()]).poseidon_preimage_digest,
        )
        .unwrap();
    let counted = sizing.append(&malformed, None).unwrap();
    let prefix = transcript(vec![first, malformed]);
    assert_eq!(
        counted.input_frame_bytes,
        norito::encode_canonical(&prefix).unwrap().len()
    );
    assert!(
        quantity_statement_frame_len_from_finalized_transcripts(
            &[prefix],
            PublicTransferLimits::default()
        )
        .is_err()
    );
}

#[test]
fn clone_checkpoint_restores_only_sizing_state_and_self_legs_keep_two_rows() {
    let mut self_delta = delta(quantity(1, 0), quantity(10, 0), quantity(9, 0));
    self_delta.to_account = self_delta.from_account.clone();
    let prefix = transcript(vec![self_delta.clone()]);
    let mut sizing = sizer();
    let empty = sizing.clone();
    assert_eq!(
        sizing
            .append(&self_delta, prefix.poseidon_preimage_digest)
            .unwrap(),
        reference(&prefix)
    );
    assert_eq!(sizing.rows.count(), 2);
    sizing = empty;
    assert_eq!(sizing.latest(), None);
    assert_eq!(sizing.rows.count(), 0);
    assert_eq!(
        sizing
            .append(&self_delta, prefix.poseidon_preimage_digest)
            .unwrap(),
        reference(&prefix)
    );
}

#[test]
fn checked_field_spans_follow_codec_prefix_boundaries_and_reject_overflow() {
    let flags = header_flags::COMPACT_LEN;
    for (payload, expected) in [
        (0, 1),
        (127, 128),
        (128, 130),
        (16_383, 16_385),
        (16_384, 16_387),
    ] {
        assert_eq!(field_span(payload, flags).unwrap(), expected);
    }
    assert!(matches!(
        field_span(usize::MAX, flags),
        Err(PrefixLengthError::Overflow)
    ));
    assert!(matches!(
        add(usize::MAX, 1),
        Err(PrefixLengthError::Overflow)
    ));
    assert!(matches!(subtract(0, 1), Err(PrefixLengthError::Overflow)));
    let maximum_deltas = (u32::MAX / 2) as usize;
    assert_eq!(checked_row_count(maximum_deltas).unwrap(), u32::MAX - 1);
    assert!(matches!(
        checked_row_count(maximum_deltas + 1),
        Err(PrefixLengthError::RowCount)
    ));
    assert!(matches!(
        checked_row_count(usize::MAX),
        Err(PrefixLengthError::RowCount)
    ));
}
