//! Checked prefix publication, rollback and strict canonical frame parity.

use super::*;
use crate::fastpq::{
    poseidon_preimage_digest,
    quantity_statement::quantity_statement_frame_len_from_finalized_transcripts,
};
use fastpq_prover::gadgets::public_transfer_statement::PublicTransferLimits;
use iroha_data_model::{
    DomainId,
    asset::AssetDefinitionId,
    fastpq::{TransferSmtWitness, TransferTranscript},
};
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};

fn limits() -> PrefixLengthLimits {
    PrefixLengthLimits {
        max_deltas: 128,
        max_input_frame_bytes: 4_000_000,
        max_public_statement_frame_bytes: 4_000_000,
    }
}

fn hashes() -> (Hash, Hash) {
    (
        Hash::new(b"checked-prefix-call"),
        Hash::new(b"checked-prefix-authority"),
    )
}

fn quantity(mantissa: u32, scale: u32) -> Quantity {
    Quantity::try_from_numeric(Numeric::new(mantissa, scale)).unwrap()
}

fn delta(amount: Quantity, sender: Quantity, receiver: Quantity) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        from_balance_after: sender.try_sub(&amount).unwrap(),
        to_balance_after: receiver.try_add(&amount).unwrap(),
        amount,
        from_balance_before: sender,
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

fn checked() -> CheckedSourcePrefix {
    let (batch_hash, authority_digest) = hashes();
    CheckedSourcePrefix::new(batch_hash, authority_digest, limits()).unwrap()
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

fn snapshot(
    prefix: &CheckedSourcePrefix,
) -> (Option<PrefixFrameLengths>, [usize; 6], usize, usize) {
    (
        prefix.latest(),
        [
            prefix.lengths.private_deltas.count(),
            prefix.lengths.private_deltas.len(),
            prefix.lengths.public_deltas.count(),
            prefix.lengths.public_deltas.len(),
            prefix.lengths.rows.count(),
            prefix.lengths.rows.len(),
        ],
        prefix.semantics.count(),
        prefix.semantics.unique_keys(),
    )
}

#[test]
fn dropping_pending_updates_preserves_empty_and_nonempty_prefixes_before_valid_retry() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let second = delta(quantity(1, 2), quantity(9, 0), quantity(1, 0));
    let mut prefix = checked();
    assert_eq!(prefix.latest(), None);
    assert_eq!(prefix.semantics.count(), 0);
    assert_eq!(prefix.semantics.unique_keys(), 0);

    let mut deltas = Vec::new();
    for next in [&first, &second] {
        deltas.push(next.clone());
        let claim = transcript(deltas.clone());
        let expected = reference(&claim);
        let before = snapshot(&prefix);
        {
            let pending = prefix
                .prepare(next, claim.poseidon_preimage_digest)
                .unwrap();
            assert!(std::ptr::eq(pending.delta(), next));
            assert_eq!(pending.lengths(), expected);
            // A caller's failed reservation drops this prepared update without
            // publishing sizing counters or a new semantic comparison value.
        }
        assert_eq!(snapshot(&prefix), before);
        assert_eq!(
            prefix
                .prepare(next, claim.poseidon_preimage_digest)
                .unwrap()
                .commit(),
            expected
        );
        assert_eq!(prefix.latest(), Some(expected));
        assert_eq!(prefix.semantics.count(), deltas.len());
        assert_eq!(prefix.semantics.unique_keys(), 2);
    }
}

#[test]
fn wrong_first_digest_does_not_publish_sizing_or_semantics_and_valid_retry_succeeds() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let claim = transcript(vec![first.clone()]);
    let wrong = Some(Hash::new(b"unrelated checked-prefix digest"));
    assert_ne!(wrong, claim.poseidon_preimage_digest);
    let mut prefix = checked();
    let before = snapshot(&prefix);
    assert!(prefix.lengths.clone().append(&first, wrong).is_ok());
    assert!(matches!(
        prefix.append(&first, wrong),
        Err(CheckedPrefixError::Semantic(_))
    ));
    assert_eq!(snapshot(&prefix), before);
    assert_eq!(
        prefix
            .append(&first, claim.poseidon_preimage_digest)
            .unwrap(),
        reference(&claim)
    );
    assert_eq!(prefix.semantics.count(), 1);
    assert_eq!(prefix.semantics.unique_keys(), 2);
}

#[test]
fn arithmetic_failure_after_successful_sizing_keeps_both_prefixes_retryable() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let valid = delta(quantity(1, 0), quantity(9, 0), quantity(1, 0));
    let mut invalid = valid.clone();
    invalid.from_balance_after = quantity(7, 0);
    let mut prefix = checked();
    prefix
        .append(
            &first,
            transcript(vec![first.clone()]).poseidon_preimage_digest,
        )
        .unwrap();
    let before = snapshot(&prefix);
    assert!(prefix.lengths.clone().append(&invalid, None).is_ok());
    assert!(
        quantity_statement_frame_len_from_finalized_transcripts(
            &[transcript(vec![first.clone(), invalid.clone()])],
            PublicTransferLimits::default(),
        )
        .is_err()
    );
    assert!(matches!(
        prefix.append(&invalid, None),
        Err(CheckedPrefixError::Semantic(_))
    ));
    assert_eq!(snapshot(&prefix), before);
    let claim = transcript(vec![first, valid.clone()]);
    assert_eq!(prefix.append(&valid, None).unwrap(), reference(&claim));
    assert_eq!(prefix.semantics.count(), 2);
    assert_eq!(prefix.semantics.unique_keys(), 2);
}

#[test]
fn chronology_failure_does_not_insert_a_new_sender_or_publish_length_counters() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let mut valid = delta(quantity(1, 0), quantity(10, 0), quantity(1, 0));
    valid.from_account = (*CARPENTER_ID).clone();
    let mut invalid = valid.clone();
    invalid.to_balance_before = quantity(9, 0);
    invalid.to_balance_after = quantity(10, 0);
    let mut prefix = checked();
    prefix
        .append(
            &first,
            transcript(vec![first.clone()]).poseidon_preimage_digest,
        )
        .unwrap();
    let before = snapshot(&prefix);
    assert!(prefix.lengths.clone().append(&invalid, None).is_ok());
    assert!(
        quantity_statement_frame_len_from_finalized_transcripts(
            &[transcript(vec![first.clone(), invalid.clone()])],
            PublicTransferLimits::default(),
        )
        .is_err()
    );
    assert!(matches!(
        prefix.append(&invalid, None),
        Err(CheckedPrefixError::Semantic(_))
    ));
    assert_eq!(snapshot(&prefix), before);
    let claim = transcript(vec![first, valid.clone()]);
    assert_eq!(prefix.append(&valid, None).unwrap(), reference(&claim));
    assert_eq!(prefix.semantics.count(), 2);
    assert_eq!(prefix.semantics.unique_keys(), 3);
}

#[test]
fn each_byte_cap_rejects_before_semantic_publication_and_accepts_its_exact_boundary() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let mut second = delta(quantity(1, 0), quantity(9, 0), quantity(1, 0));
    second.from_smt_witness.siblings = vec![[7; 32]; 32];
    let claim = transcript(vec![first.clone(), second.clone()]);
    let expected = reference(&claim);
    for restrict_input in [true, false] {
        let mut prefix = checked();
        prefix
            .append(
                &first,
                transcript(vec![first.clone()]).poseidon_preimage_digest,
            )
            .unwrap();
        if restrict_input {
            prefix.lengths.limits.max_input_frame_bytes = expected.input_frame_bytes - 1;
        } else {
            prefix.lengths.limits.max_public_statement_frame_bytes =
                expected.public_statement_frame_bytes - 1;
        }
        let before = snapshot(&prefix);
        match prefix.append(&second, None) {
            Err(CheckedPrefixError::Length(PrefixLengthError::Input { actual, maximum }))
                if restrict_input =>
            {
                assert_eq!(actual, expected.input_frame_bytes);
                assert_eq!(maximum + 1, actual);
            }
            Err(CheckedPrefixError::Length(PrefixLengthError::Public { actual, maximum }))
                if !restrict_input =>
            {
                assert_eq!(actual, expected.public_statement_frame_bytes);
                assert_eq!(maximum + 1, actual);
            }
            result => panic!("wrong byte-cap result: {result:?}"),
        }
        assert_eq!(snapshot(&prefix), before);
        // Test-only access adjusts the same sizer's ceiling, so a retry checks
        // the semantic validator that was present during the failed attempt.
        prefix.lengths.limits.max_input_frame_bytes = expected.input_frame_bytes;
        prefix.lengths.limits.max_public_statement_frame_bytes =
            expected.public_statement_frame_bytes;
        assert_eq!(prefix.append(&second, None).unwrap(), expected);
        assert_eq!(prefix.semantics.count(), 2);
        assert_eq!(prefix.semantics.unique_keys(), 2);
    }
}

#[test]
fn digest_presence_failures_leave_both_prefixes_unchanged() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let second = delta(quantity(1, 0), quantity(9, 0), quantity(1, 0));
    let mut prefix = checked();
    let empty = snapshot(&prefix);
    assert!(matches!(
        prefix.append(&first, None),
        Err(CheckedPrefixError::Length(PrefixLengthError::DigestShape))
    ));
    assert_eq!(snapshot(&prefix), empty);
    let first_claim = transcript(vec![first.clone()]);
    prefix
        .append(&first, first_claim.poseidon_preimage_digest)
        .unwrap();
    let accepted = snapshot(&prefix);
    assert!(matches!(
        prefix.append(&second, first_claim.poseidon_preimage_digest),
        Err(CheckedPrefixError::Length(PrefixLengthError::DigestShape))
    ));
    assert_eq!(snapshot(&prefix), accepted);
    assert_eq!(
        prefix.append(&second, None).unwrap(),
        reference(&transcript(vec![first, second]))
    );
}

#[test]
fn inclusive_delta_limit_does_not_publish_an_over_limit_semantic_append() {
    let (batch_hash, authority_digest) = hashes();
    let mut prefix = CheckedSourcePrefix::new(
        batch_hash,
        authority_digest,
        PrefixLengthLimits {
            max_deltas: 1,
            ..limits()
        },
    )
    .unwrap();
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let second = delta(quantity(1, 0), quantity(9, 0), quantity(1, 0));
    let claim = transcript(vec![first.clone()]);
    assert_eq!(
        prefix
            .append(&first, claim.poseidon_preimage_digest)
            .unwrap(),
        reference(&claim)
    );
    let before = snapshot(&prefix);
    assert!(matches!(
        prefix.append(&second, None),
        Err(CheckedPrefixError::Length(PrefixLengthError::Deltas {
            actual: 2,
            maximum: 1
        }))
    ));
    assert_eq!(snapshot(&prefix), before);
}

#[test]
fn self_zero_and_later_funded_prefixes_match_strict_core_frame_lengths() {
    let mut self_transfer = delta(quantity(1, 0), quantity(10, 0), quantity(9, 0));
    self_transfer.to_account = self_transfer.from_account.clone();
    let zero = delta(Quantity::zero(), quantity(10, 0), Quantity::zero());
    let fund = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let mut spend_later = delta(quantity(1, 1), quantity(1, 0), Quantity::zero());
    spend_later.from_account = (*BOB_ID).clone();
    spend_later.to_account = (*CARPENTER_ID).clone();
    let mut return_later = delta(quantity(1, 2), quantity(1, 1), quantity(9, 0));
    return_later.from_account = (*CARPENTER_ID).clone();
    return_later.to_account = (*ALICE_ID).clone();

    let mut prefix = checked();
    let mut deltas = Vec::new();
    for (next, expected_keys) in [
        (self_transfer, 1),
        (zero, 2),
        (fund, 2),
        (spend_later, 3),
        (return_later, 3),
    ] {
        deltas.push(next);
        let claim = transcript(deltas.clone());
        let expected = reference(&claim);
        assert_eq!(
            prefix
                .append(deltas.last().unwrap(), claim.poseidon_preimage_digest)
                .unwrap(),
            expected
        );
        assert_eq!(prefix.latest(), Some(expected));
        assert_eq!(prefix.semantics.count(), deltas.len());
        assert_eq!(prefix.semantics.unique_keys(), expected_keys);
    }
}

#[test]
fn distinct_occurrence_instances_reset_local_chronology_even_with_identical_headers() {
    let first = delta(quantity(1, 0), quantity(10, 0), Quantity::zero());
    let independent = delta(quantity(1, 0), quantity(100, 0), quantity(7, 0));
    let first_claim = transcript(vec![first.clone()]);
    let independent_claim = transcript(vec![independent.clone()]);
    let mut first_prefix = checked();
    first_prefix
        .append(&first, first_claim.poseidon_preimage_digest)
        .unwrap();
    let before = snapshot(&first_prefix);
    assert!(matches!(
        first_prefix.append(&independent, None),
        Err(CheckedPrefixError::Semantic(_))
    ));
    assert_eq!(snapshot(&first_prefix), before);

    let mut second_prefix = checked();
    assert_eq!(second_prefix.latest(), None);
    assert_eq!(second_prefix.semantics.count(), 0);
    assert_eq!(second_prefix.semantics.unique_keys(), 0);
    assert_eq!(
        second_prefix
            .append(&independent, independent_claim.poseidon_preimage_digest)
            .unwrap(),
        reference(&independent_claim)
    );
    assert_eq!(second_prefix.semantics.count(), 1);
    assert_eq!(second_prefix.semantics.unique_keys(), 2);
    assert_eq!(snapshot(&first_prefix), before);
    // The local reset does not authorize inconsistent occurrences together:
    // the complete statement constructor must still check their shared keys.
    assert!(
        quantity_statement_frame_len_from_finalized_transcripts(
            &[first_claim, independent_claim],
            PublicTransferLimits::default(),
        )
        .is_err()
    );
}
