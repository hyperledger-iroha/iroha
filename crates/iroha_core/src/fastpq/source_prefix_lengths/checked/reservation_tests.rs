//! Checked-prefix publication across real reservation-ledger capacity and rollback decisions.

use super::*;
use crate::fastpq::{
    poseidon_preimage_digest,
    quantity_statement::quantity_statement_frame_len_from_finalized_transcripts,
    source_reservation::{
        OccurrenceUsage, ReservationContext, ReservationError, ReservationLedger,
        ReservationPolicy, SourceDimension, SourceUsage,
    },
};
use fastpq_prover::gadgets::public_transfer_statement::PublicTransferLimits;
use iroha_data_model::{
    DomainId,
    asset::AssetDefinitionId,
    fastpq::{TransferSmtWitness, TransferTranscript},
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn context() -> ReservationContext {
    ReservationContext {
        height: 1,
        policy_digest: [7; 32],
        scope_tag: 2,
    }
}

fn ledger() -> ReservationLedger {
    let caps = SourceUsage {
        executed_entries: 10,
        transcripts: 10,
        deltas: 2,
        input_transcript_bytes: 2_000_000,
        max_statement_bytes: 2_000_000,
        total_statement_bytes: 4_000_000,
    };
    ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: caps,
            block: caps,
        },
    )
    .unwrap()
}

fn prefix(batch: Hash) -> CheckedSourcePrefix {
    CheckedSourcePrefix::new(
        batch,
        Hash::new(b"reservation-authority"),
        PrefixLengthLimits {
            max_deltas: 10,
            max_input_frame_bytes: 2_000_000,
            max_public_statement_frame_bytes: 2_000_000,
        },
    )
    .unwrap()
}

fn delta(sender: u32, receiver: u32) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        amount: Quantity::from(1_u32),
        from_balance_before: Quantity::from(sender),
        from_balance_after: Quantity::from(sender - 1),
        to_balance_before: Quantity::from(receiver),
        to_balance_after: Quantity::from(receiver + 1),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

fn occurrence(lengths: PrefixFrameLengths) -> OccurrenceUsage {
    OccurrenceUsage::new(
        u64::try_from(lengths.deltas).unwrap(),
        u64::try_from(lengths.input_frame_bytes).unwrap(),
        u64::try_from(lengths.public_statement_frame_bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn capacity_failure_drops_unpublished_semantics_and_lengths_then_allows_exact_retry() {
    let a_hash = Hash::new(b"reservation-entry-a");
    let b_hash = Hash::new(b"reservation-entry-b");
    let first = delta(10, 0);
    let second = delta(9, 1);
    let mut a = prefix(a_hash);
    let mut b = prefix(b_hash);
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let a_owner = tx.open_owner().unwrap();
    let pending = a
        .prepare(&first, Some(poseidon_preimage_digest(&first, &a_hash)))
        .unwrap();
    let a_slot = tx.reserve(&a_owner, occurrence(pending.lengths())).unwrap();
    let a_first = pending.commit();
    let before_other = tx.checkpoint();
    let b_owner = tx.open_owner().unwrap();
    let pending = b
        .prepare(&first, Some(poseidon_preimage_digest(&first, &b_hash)))
        .unwrap();
    tx.reserve(&b_owner, occurrence(pending.lengths())).unwrap();
    pending.commit();
    let before_failure = tx.usage();
    {
        let pending = a.prepare(&second, None).unwrap();
        assert_eq!(pending.lengths().deltas, 2);
        assert_eq!(
            tx.replace(&a_owner, &a_slot, occurrence(pending.lengths()))
                .unwrap_err(),
            ReservationError::RemainingBlock {
                dimension: SourceDimension::Deltas,
                required: 2,
                available: 1
            }
        );
        // No checked commit: the owner rejected admission of this complete prefix.
    }
    assert_eq!(a.latest(), Some(a_first));
    assert_eq!(a.semantics.count(), 1);
    assert_eq!(a.semantics.unique_keys(), 2);
    assert_eq!(tx.usage(), before_failure);
    tx.rollback(before_other).unwrap();
    drop(b); // The abandoned execution owner also discards its local prefix.
    let pending = a.prepare(&second, None).unwrap();
    tx.replace(&a_owner, &a_slot, occurrence(pending.lengths()))
        .unwrap();
    let final_lengths = pending.commit();
    let original = TransferTranscript {
        batch_hash: a_hash,
        authority_digest: Hash::new(b"reservation-authority"),
        deltas: vec![first, second],
        poseidon_preimage_digest: None,
    };
    assert_eq!(
        final_lengths.input_frame_bytes,
        norito::encode_canonical(&original).unwrap().len()
    );
    assert_eq!(
        final_lengths.public_statement_frame_bytes,
        quantity_statement_frame_len_from_finalized_transcripts(
            &[original],
            PublicTransferLimits::default()
        )
        .unwrap()
    );
    assert_eq!(
        tx.usage(),
        SourceUsage {
            executed_entries: 1,
            transcripts: 1,
            deltas: 2,
            input_transcript_bytes: u64::try_from(final_lengths.input_frame_bytes).unwrap(),
            max_statement_bytes: u64::try_from(final_lengths.public_statement_frame_bytes).unwrap(),
            total_statement_bytes: u64::try_from(final_lengths.public_statement_frame_bytes)
                .unwrap(),
        }
    );
    assert_eq!(a.semantics.count(), 2);
    let committed = tx.commit();
    assert_eq!(ledger.usage(), committed);
}

#[test]
fn invalid_transfer_cannot_reach_reservation_or_publish_either_prefix() {
    let batch = Hash::new(b"invalid-entry");
    let mut invalid = delta(10, 0);
    invalid.from_balance_after = Quantity::zero();
    let mut checked = prefix(batch);
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    tx.open_owner().unwrap();
    let before = tx.usage();
    assert!(matches!(
        checked.prepare(&invalid, Some(poseidon_preimage_digest(&invalid, &batch))),
        Err(CheckedPrefixError::Semantic(_))
    ));
    assert_eq!(checked.latest(), None);
    assert_eq!(checked.semantics.count(), 0);
    assert_eq!(checked.semantics.unique_keys(), 0);
    assert_eq!(tx.usage(), before);
}

#[test]
fn abandoned_business_scope_discards_checked_prefix_and_reservations_but_retains_seeded_e() {
    let batch = Hash::new(b"discarded-business");
    let first = delta(10, 0);
    let mut ledger = ledger();
    let mut seed = ledger.transaction(context()).unwrap();
    let owner = seed.open_owner().unwrap();
    seed.commit();
    let seeded = ledger.usage();
    {
        let mut checked = prefix(batch);
        let mut business = ledger.transaction(context()).unwrap();
        let pending = checked
            .prepare(&first, Some(poseidon_preimage_digest(&first, &batch)))
            .unwrap();
        business
            .reserve(&owner, occurrence(pending.lengths()))
            .unwrap();
        pending.commit();
        assert_eq!(business.usage().transcripts, 1);
        // The real State owner must drop matching WSV/source side effects too.
    }
    assert_eq!(ledger.usage(), seeded);
    assert_eq!(
        seeded,
        SourceUsage {
            executed_entries: 1,
            ..SourceUsage::ZERO
        }
    );
    let mut retry = prefix(batch);
    let mut business = ledger.transaction(context()).unwrap();
    let pending = retry
        .prepare(&first, Some(poseidon_preimage_digest(&first, &batch)))
        .unwrap();
    business
        .reserve(&owner, occurrence(pending.lengths()))
        .unwrap();
    pending.commit();
    assert_eq!(business.usage().executed_entries, 1);
    assert_eq!(business.usage().transcripts, 1);
    business.commit();
}
