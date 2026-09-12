//! Differential full-domain prefix semantics and pending-update atomicity.

use super::*;
use crate::{
    ProofSemantics, PublicInputs,
    gadgets::{
        public_transfer_statement::{
            PublicTransferLimits, PublicTransferTranscript, prepare_quantity_public_transfers,
            quantity_rows_for_public_preparation, quantity_tests,
        },
        transfer,
    },
};
use iroha_data_model::{asset::id::AssetDefinitionId, fastpq::TransferTranscript};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};

fn batch() -> Hash {
    Hash::new(b"quantity prefix occurrence")
}

fn limits() -> QuantityPrefixLimits {
    QuantityPrefixLimits {
        max_deltas: 64,
        max_unique_keys: 128,
    }
}

fn delta(sender: Quantity, receiver: Quantity, amount: Quantity) -> TransferDeltaTranscript {
    let d = quantity_tests::delta(sender, receiver, amount);
    TransferDeltaTranscript {
        from_account: d.from_account,
        to_account: d.to_account,
        asset_definition: d.asset_definition,
        amount: d.amount,
        from_balance_before: d.from_balance_before,
        from_balance_after: d.from_balance_after,
        to_balance_before: d.to_balance_before,
        to_balance_after: d.to_balance_after,
        from_smt_witness: Default::default(),
        to_smt_witness: Default::default(),
    }
}

fn integer_delta(sender: u32, receiver: u32, amount: u32) -> TransferDeltaTranscript {
    delta(sender.into(), receiver.into(), amount.into())
}

fn digest(deltas: &[TransferDeltaTranscript]) -> Option<Hash> {
    match deltas {
        [d] => Some(transfer::compute_poseidon_digest(d, &batch())),
        _ => None,
    }
}

fn inputs() -> PublicInputs {
    PublicInputs {
        old_root: Hash::new(b"prefix caller root before").into(),
        new_root: Hash::new(b"prefix caller root after").into(),
        slot: 17,
        dsid: [3; 16],
        perm_root: Hash::new(b"prefix permissions").into(),
        tx_set_hash: Hash::new(b"prefix transaction set").into(),
    }
}

fn public_limits() -> PublicTransferLimits {
    PublicTransferLimits {
        max_transcripts: 1,
        max_deltas: 64,
        max_rows: 128,
        max_public_bytes: 1 << 24,
        max_unique_keys: 128,
        max_allocation_steps: 512,
    }
}

fn claims(
    deltas: &[TransferDeltaTranscript],
    digest: Option<Hash>,
) -> Vec<PublicTransferTranscript> {
    vec![PublicTransferTranscript::from(&TransferTranscript {
        batch_hash: batch(),
        authority_digest: Hash::new(b"prefix caller authority"),
        poseidon_preimage_digest: digest,
        deltas: deltas.to_vec(),
    })]
}

fn full_accepts(deltas: &[TransferDeltaTranscript], digest: Option<Hash>) -> bool {
    let claims = claims(deltas, digest);
    let Ok(rows) = quantity_rows_for_public_preparation(&claims, inputs(), public_limits(), 128)
    else {
        return false;
    };
    prepare_quantity_public_transfers(
        &rows,
        &claims,
        inputs(),
        ProofSemantics::StateTransition,
        public_limits(),
    )
    .is_ok()
}

fn incremental_accepts(deltas: &[TransferDeltaTranscript], final_digest: Option<Hash>) -> bool {
    let mut validator = QuantityPrefixValidator::new(batch(), limits());
    for (index, delta) in deltas.iter().enumerate() {
        let current_digest = if index + 1 == deltas.len() {
            final_digest
        } else {
            digest(&deltas[..=index])
        };
        match validator.prepare(delta, current_digest) {
            Ok(pending) => pending.commit(),
            Err(_) => return false,
        }
    }
    !deltas.is_empty()
}

fn assert_every_prefix(deltas: &[TransferDeltaTranscript], expected: &[bool]) {
    assert_eq!(deltas.len(), expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let prefix = &deltas[..=index];
        let digest = digest(prefix);
        assert_eq!(
            full_accepts(prefix, digest),
            *expected,
            "full prefix {index}"
        );
        assert_eq!(
            incremental_accepts(prefix, digest),
            *expected,
            "incremental prefix {index}"
        );
    }
}

fn commit(validator: &mut QuantityPrefixValidator, d: &TransferDeltaTranscript) {
    let digest = (validator.count() == 0).then(|| transfer::compute_poseidon_digest(d, &batch()));
    validator.prepare(d, digest).unwrap().commit();
}

#[test]
fn explicit_identity_limits_and_empty_counts_are_retained() {
    let validator = QuantityPrefixValidator::new(batch(), limits());
    assert_eq!(validator.batch_hash(), batch());
    assert_eq!(validator.limits(), limits());
    assert_eq!(validator.count(), 0);
    assert_eq!(validator.unique_keys(), 0);
}

#[test]
fn pending_borrows_original_and_drop_preserves_empty_state() {
    let d = integer_delta(10, 0, 1);
    let mut validator = QuantityPrefixValidator::new(batch(), limits());
    {
        let pending = validator
            .prepare(&d, digest(std::slice::from_ref(&d)))
            .unwrap();
        assert!(std::ptr::eq(pending.delta(), &d));
        assert_eq!(pending.count(), 1);
        assert_eq!(pending.unique_keys(), 2);
    }
    assert_eq!(validator.count(), 0);
    assert_eq!(validator.unique_keys(), 0);
    commit(&mut validator, &d);
    assert_eq!(validator.count(), 1);
    assert_eq!(validator.unique_keys(), 2);
}

#[test]
fn dropping_later_pending_preserves_both_prior_balances() {
    let first = integer_delta(10, 0, 1);
    let next = integer_delta(9, 1, 2);
    let mut validator = QuantityPrefixValidator::new(batch(), limits());
    commit(&mut validator, &first);
    let before = validator.last_values.clone();
    {
        let pending = validator.prepare(&next, None).unwrap();
        assert_eq!(pending.count(), 2);
        assert_eq!(pending.unique_keys(), 2);
    }
    assert_eq!(validator.count(), 1);
    assert_eq!(validator.last_values, before);
    commit(&mut validator, &next);
    assert_eq!(validator.count(), 2);
    assert_ne!(validator.last_values, before);
}

#[test]
fn repeated_sender_and_receiver_disagreement_reject_without_mutation() {
    let first = integer_delta(10, 0, 1);
    let valid = integer_delta(9, 1, 2);
    let mut validator = QuantityPrefixValidator::new(batch(), limits());
    commit(&mut validator, &first);
    let before = validator.last_values.clone();
    for bad in [integer_delta(10, 1, 2), integer_delta(9, 2, 2)] {
        assert!(validator.prepare(&bad, None).is_err());
        assert_eq!(validator.count(), 1);
        assert_eq!(validator.last_values, before);
        assert_every_prefix(&[first.clone(), bad], &[true, false]);
    }
    commit(&mut validator, &valid);
    assert_every_prefix(&[first, valid], &[true, true]);
}

#[test]
fn later_fractional_before_balance_requires_chronology_even_when_arithmetic_passes() {
    let first = integer_delta(10, 0, 1);
    let bad = delta("9.1".parse().unwrap(), Quantity::one(), Quantity::zero());
    assert!(normalized_delta_values_for::<FastpqQuantityUnits>(DeltaView::from(&bad), 28).is_ok());
    assert_every_prefix(&[first, bad], &[true, false]);
}

#[test]
fn singleton_digest_requires_original_identity_amount_and_batch() {
    let d = integer_delta(10, 0, 1);
    let expected = digest(std::slice::from_ref(&d));
    for wrong in [None, Some(Hash::new(b"wrong prefix digest"))] {
        assert!(!full_accepts(std::slice::from_ref(&d), wrong));
        assert!(!incremental_accepts(std::slice::from_ref(&d), wrong));
    }
    let mut wrong_batch = QuantityPrefixValidator::new(Hash::new(b"wrong batch"), limits());
    assert!(wrong_batch.prepare(&d, expected).is_err());
    assert_eq!(wrong_batch.count(), 0);
    let mut wrong_account = d.clone();
    wrong_account.to_account = (*CARPENTER_ID).clone();
    let mut wrong_asset = d.clone();
    wrong_asset.asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("another", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let wrong_amount = integer_delta(10, 0, 2);
    for changed in [wrong_account, wrong_asset, wrong_amount] {
        assert!(!full_accepts(std::slice::from_ref(&changed), expected));
        assert!(!incremental_accepts(
            std::slice::from_ref(&changed),
            expected
        ));
    }
}

#[test]
fn multiple_prefix_some_digest_rejects_and_none_can_retry() {
    let first = integer_delta(10, 0, 1);
    let second = integer_delta(9, 1, 2);
    let mut validator = QuantityPrefixValidator::new(batch(), limits());
    commit(&mut validator, &first);
    let before = validator.last_values.clone();
    let stale_digest = digest(std::slice::from_ref(&first));
    assert!(validator.prepare(&second, stale_digest).is_err());
    assert_eq!(validator.count(), 1);
    assert_eq!(validator.last_values, before);
    assert!(!full_accepts(
        &[first.clone(), second.clone()],
        stale_digest
    ));
    assert!(!incremental_accepts(&[first, second.clone()], stale_digest));
    commit(&mut validator, &second);
    assert_eq!(validator.count(), 2);
}

#[test]
fn full_mantissa_and_scale_extrema_match_complete_preparation() {
    for d in [
        delta(u128::MAX.into(), Quantity::zero(), Quantity::one()),
        delta(
            quantity_tests::maximum(),
            Quantity::zero(),
            quantity_tests::maximum(),
        ),
        delta(
            quantity_tests::maximum().try_sub(&Quantity::one()).unwrap(),
            quantity_tests::tiny(),
            Quantity::one(),
        ),
        delta(2_u32.into(), Quantity::zero(), quantity_tests::tiny()),
    ] {
        assert_every_prefix(&[d], &[true]);
    }
}

#[test]
fn subtraction_addition_and_canonical_mantissa_domain_fail_closed() {
    let mut underflow = integer_delta(0, 0, 0);
    underflow.amount = Quantity::one();
    underflow.to_balance_after = Quantity::one();
    let mut overflow = integer_delta(1, 0, 1);
    overflow.to_balance_before = quantity_tests::maximum();
    overflow.to_balance_after = quantity_tests::maximum();
    let mut nonrepresentable = integer_delta(1, 0, 0);
    nonrepresentable.amount = quantity_tests::tiny();
    nonrepresentable.from_balance_before = quantity_tests::maximum();
    nonrepresentable.from_balance_after = quantity_tests::maximum();
    nonrepresentable.to_balance_after = quantity_tests::tiny();
    for bad in [underflow, overflow, nonrepresentable] {
        let mut validator = QuantityPrefixValidator::new(batch(), limits());
        assert!(
            validator
                .prepare(&bad, digest(std::slice::from_ref(&bad)))
                .is_err()
        );
        assert_eq!(validator.count(), 0);
        assert_eq!(validator.unique_keys(), 0);
        assert_every_prefix(&[bad], &[false]);
    }
}

#[test]
fn new_amount_scale_preserves_chronology_but_changes_selected_scale_rows() {
    let first = integer_delta(10, 0, 1);
    let second = delta(9_u32.into(), Quantity::one(), quantity_tests::tiny());
    assert_every_prefix(&[first.clone(), second.clone()], &[true, true]);
    let first_claims = claims(
        std::slice::from_ref(&first),
        digest(std::slice::from_ref(&first)),
    );
    let full_claims = claims(&[first, second], None);
    let first_rows =
        quantity_rows_for_public_preparation(&first_claims, inputs(), public_limits(), 128)
            .unwrap();
    let full_rows =
        quantity_rows_for_public_preparation(&full_claims, inputs(), public_limits(), 128).unwrap();
    let first_pre = &first_rows[0].pre_value;
    let same_key_first_row = full_rows
        .iter()
        .find(|row| row.key == first_rows[0].key)
        .unwrap();
    assert_ne!(first_pre, &same_key_first_row.pre_value);
    let first_units = super::super::decode_quantity_units_v1(first_pre).unwrap();
    let full_units = super::super::decode_quantity_units_v1(&same_key_first_row.pre_value).unwrap();
    assert_eq!(first_units.scale(), 0);
    assert_eq!(full_units.scale(), 28);
    assert_eq!(first_units.to_quantity(), full_units.to_quantity());
    let a = prepare_quantity_public_transfers(
        &first_rows,
        &first_claims,
        inputs(),
        ProofSemantics::StateTransition,
        public_limits(),
    )
    .unwrap();
    let b = prepare_quantity_public_transfers(
        &full_rows,
        &full_claims,
        inputs(),
        ProofSemantics::StateTransition,
        public_limits(),
    )
    .unwrap();
    assert_ne!(
        a.pairs()[0].updates[0].old_leaf,
        b.pairs()[0].updates[0].old_leaf
    );
}

#[test]
fn new_participant_and_separate_asset_scales_match_every_prefix() {
    let first = integer_delta(10, 0, 1);
    let mut next = delta(9_u32.into(), "0.1".parse().unwrap(), Quantity::one());
    next.to_account = (*CARPENTER_ID).clone();
    let mut other_asset = delta(2_u32.into(), Quantity::zero(), quantity_tests::tiny());
    other_asset.asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("another", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    assert_every_prefix(
        &[first.clone(), next.clone(), other_asset.clone()],
        &[true, true, true],
    );
    let mut validator = QuantityPrefixValidator::new(batch(), limits());
    commit(&mut validator, &first);
    commit(&mut validator, &next);
    assert_eq!(validator.unique_keys(), 3);
    commit(&mut validator, &other_asset);
    assert_eq!(validator.unique_keys(), 5);
}

#[test]
fn zero_and_nonzero_self_transfers_commit_credit_final_balance_once() {
    for amount in [0, 1] {
        let mut same = integer_delta(10, 10 - amount, amount);
        same.to_account = (*ALICE_ID).clone();
        let next = integer_delta(10, 0, 1);
        assert_every_prefix(&[same.clone(), next.clone()], &[true, true]);
        let mut validator = QuantityPrefixValidator::new(batch(), limits());
        commit(&mut validator, &same);
        assert_eq!(validator.unique_keys(), 1);
        assert_eq!(
            validator.last_values.values().next().unwrap().to_quantity(),
            Some(10_u32.into())
        );
        commit(&mut validator, &next);
        assert_eq!(validator.unique_keys(), 2);
    }
}

#[test]
fn mathematically_valid_but_false_self_legs_reject() {
    let mut same = integer_delta(10, 10, 1);
    same.to_account = same.from_account.clone();
    assert_every_prefix(&[same], &[false]);
}

#[test]
fn later_funded_participant_and_zero_updates_preserve_leg_order() {
    let first = integer_delta(2, 0, 1);
    let mut reverse = integer_delta(1, 1, 1);
    reverse.from_account = (*BOB_ID).clone();
    reverse.to_account = (*ALICE_ID).clone();
    let zero = integer_delta(2, 0, 0);
    assert_every_prefix(&[first, reverse, zero], &[true, true, true]);
}

#[test]
fn inclusive_count_and_key_caps_allow_existing_keys_at_the_boundary() {
    let first = integer_delta(10, 0, 1);
    let second = integer_delta(9, 1, 1);
    let third = integer_delta(8, 2, 1);
    let caps = QuantityPrefixLimits {
        max_deltas: 2,
        max_unique_keys: 2,
    };
    let mut validator = QuantityPrefixValidator::new(batch(), caps);
    commit(&mut validator, &first);
    commit(&mut validator, &second);
    let before = validator.last_values.clone();
    assert!(validator.prepare(&third, None).is_err());
    assert_eq!(validator.count(), 2);
    assert_eq!(validator.unique_keys(), 2);
    assert_eq!(validator.last_values, before);
}

#[test]
fn key_cap_failure_never_installs_the_first_participant() {
    let first = integer_delta(10, 0, 1);
    let mut validator = QuantityPrefixValidator::new(
        batch(),
        QuantityPrefixLimits {
            max_deltas: 2,
            max_unique_keys: 1,
        },
    );
    assert!(
        validator
            .prepare(&first, digest(std::slice::from_ref(&first)))
            .is_err()
    );
    assert_eq!(validator.count(), 0);
    assert!(validator.last_values.is_empty());
    let mut same = integer_delta(10, 9, 1);
    same.to_account = same.from_account.clone();
    commit(&mut validator, &same);
    assert_eq!(validator.unique_keys(), 1);
}

#[test]
fn zero_caps_and_count_overflow_reject_without_mutation() {
    let d = integer_delta(10, 0, 1);
    for caps in [
        QuantityPrefixLimits {
            max_deltas: 0,
            max_unique_keys: 2,
        },
        QuantityPrefixLimits {
            max_deltas: 1,
            max_unique_keys: 0,
        },
    ] {
        let mut validator = QuantityPrefixValidator::new(batch(), caps);
        assert!(
            validator
                .prepare(&d, digest(std::slice::from_ref(&d)))
                .is_err()
        );
        assert_eq!(validator.count(), 0);
        assert_eq!(validator.unique_keys(), 0);
    }
    let mut validator = QuantityPrefixValidator::new(
        batch(),
        QuantityPrefixLimits {
            max_deltas: usize::MAX,
            max_unique_keys: usize::MAX,
        },
    );
    validator.count = usize::MAX;
    assert!(validator.prepare(&d, None).is_err());
    assert_eq!(validator.count(), usize::MAX);
    assert_eq!(validator.unique_keys(), 0);
}

#[test]
fn separate_occurrence_reset_does_not_impose_cross_occurrence_chaining() {
    let d = integer_delta(10, 0, 1);
    let mut first = QuantityPrefixValidator::new(batch(), limits());
    commit(&mut first, &d);
    assert!(first.prepare(&d, None).is_err());
    let mut separate = QuantityPrefixValidator::new(batch(), limits());
    commit(&mut separate, &d);
    assert_eq!(separate.count(), 1);
    assert_eq!(separate.unique_keys(), 2);
}

#[test]
fn borrowed_normalization_matches_selected_scale_public_delta_helper() {
    for d in [
        integer_delta(10, 0, 1),
        delta(2_u32.into(), Quantity::zero(), quantity_tests::tiny()),
        delta(quantity_tests::maximum(), Quantity::zero(), Quantity::one()),
    ] {
        let public = super::super::PublicTransferDelta::from(&d);
        for scale in 0..=28 {
            let borrowed =
                normalized_delta_values_for::<FastpqQuantityUnits>(DeltaView::from(&d), scale);
            let original =
                super::super::normalized_values_for::<FastpqQuantityUnits>(&public, scale);
            match (borrowed, original) {
                (Ok(left), Ok(right)) => assert_eq!(left, right),
                (Err(left), Err(right)) => assert_eq!(left.to_string(), right.to_string()),
                _ => panic!("borrowed selected-scale normalization diverged at {scale}"),
            }
        }
    }
}

#[test]
fn borrowed_digest_factoring_preserves_empty_singleton_and_multiple_branches() {
    let d = integer_delta(10, 0, 1);
    let view = DeltaView::from(&d);
    let empty = claims(&[], None);
    assert!(super::super::check_digest_policy(&empty[0]).is_err());
    assert!(check_delta_digest_policy(&batch(), None, 0, view).is_err());
    for deltas in [vec![d.clone()], vec![d.clone(), integer_delta(9, 1, 1)]] {
        for provided in [
            None,
            digest(std::slice::from_ref(&d)),
            Some(Hash::new(b"wrong digest")),
        ] {
            let public = claims(&deltas, provided);
            let original = super::super::check_digest_policy(&public[0]);
            let borrowed = check_delta_digest_policy(&batch(), provided, deltas.len(), view);
            match (borrowed, original) {
                (Ok(()), Ok(())) => {}
                (Err(left), Err(right)) => assert_eq!(left.to_string(), right.to_string()),
                _ => panic!("borrowed digest policy diverged"),
            }
        }
    }
}
