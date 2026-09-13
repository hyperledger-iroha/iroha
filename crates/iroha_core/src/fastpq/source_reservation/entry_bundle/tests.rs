//! Complete-entry canonical sizing, fragment ownership and rollback regressions.

use super::*;
use crate::fastpq::{
    FastpqPublicInputsTemplate, measure_fastpq_source_statement_usage, poseidon_preimage_digest,
    quantity_materializer_invocations_for_testing, quantity_statement_from_finalized_transcripts,
};
use fastpq_prover::gadgets::public_transfer_statement::{
    PublicTransferLimits, TransferSmtBuildLimits,
};
use iroha_data_model::{
    asset::AssetDefinitionId,
    fastpq::{TransferDeltaTranscript, TransferSmtWitness},
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn context() -> ReservationContext {
    ReservationContext {
        height: 9,
        policy_digest: [5; 32],
        scope_tag: 4,
    }
}

fn caps() -> SourceUsage {
    SourceUsage {
        executed_entries: 8,
        transcripts: 32,
        deltas: 32,
        input_transcript_bytes: 1_000_000,
        max_statement_bytes: 1_000_000,
        total_statement_bytes: 4_000_000,
    }
}

fn construction() -> FastpqSourceStatementBuildLimits {
    FastpqSourceStatementBuildLimits {
        max_executed_entries: 8,
        max_transcripts: 32,
        max_deltas: 32,
        max_input_transcript_bytes: 1_000_000,
        max_statement_bytes: 1_000_000,
        max_total_statement_bytes: 4_000_000,
    }
}

fn ledger() -> EntryBundleReservationLedger {
    EntryBundleReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: caps(),
            block: caps(),
        },
        construction(),
    )
    .unwrap()
}

fn transcript(hash: Hash, amount: u32, before: u32, receiver: u32) -> TransferTranscript {
    let delta = TransferDeltaTranscript {
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
    };
    let digest = poseidon_preimage_digest(&delta, &hash);
    TransferTranscript {
        batch_hash: hash,
        deltas: vec![delta],
        authority_digest: Hash::new(b"reservation authority"),
        poseidon_preimage_digest: Some(digest),
    }
}

fn bundle(hash: Hash) -> Vec<TransferTranscript> {
    vec![transcript(hash, 3, 10, 0), transcript(hash, 2, 7, 3)]
}

fn reference(hash: Hash, bundle: &[TransferTranscript]) -> SourceUsage {
    let measured = measure_fastpq_source_statement_usage(
        1,
        &BTreeMap::from([(hash, bundle.to_vec())]),
        construction(),
    )
    .unwrap();
    SourceUsage {
        executed_entries: 1,
        transcripts: measured.transcripts.try_into().unwrap(),
        deltas: measured.deltas.try_into().unwrap(),
        input_transcript_bytes: measured.input_transcript_bytes.try_into().unwrap(),
        max_statement_bytes: measured.max_statement_bytes.try_into().unwrap(),
        total_statement_bytes: measured.total_statement_bytes.try_into().unwrap(),
    }
}

fn assert_consistent(tx: &EntryBundleReservationTransaction<'_>) {
    super::super::tests::assert_consistent(&tx.inner);
}

#[test]
fn complete_entry_replaces_the_frame_and_preserves_every_occurrence() {
    let hash = Hash::new(b"complete entry");
    let bundle = bundle(hash);
    let bytes_before = norito::encode_canonical(&bundle).unwrap();
    let first_usage = reference(hash, &bundle[..1]);
    let expected = reference(hash, &bundle);
    let private_calls = quantity_materializer_invocations_for_testing();
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    assert_eq!(
        tx.replace_bundle(&owner, &bundle[..1]).unwrap(),
        first_usage
    );
    assert_eq!(tx.replace_bundle(&owner, &bundle).unwrap(), expected);
    assert_consistent(&tx);
    assert_eq!(tx.usage(), expected);
    assert_eq!(expected.transcripts, 2);
    assert_eq!(expected.deltas, 2);
    assert_eq!(expected.max_statement_bytes, expected.total_statement_bytes);
    assert!(expected.total_statement_bytes < first_usage.total_statement_bytes * 2);
    assert_eq!(
        quantity_materializer_invocations_for_testing(),
        private_calls
    );
    assert_eq!(norito::encode_canonical(&bundle).unwrap(), bytes_before);
    assert_eq!(tx.commit(), expected);
    assert_eq!(ledger.usage(), expected);

    let statement = quantity_statement_from_finalized_transcripts(
        FastpqPublicInputsTemplate {
            dsid: [3; 16],
            slot: 81,
            old_root: [0; 32],
            new_root: [0; 32],
            perm_root: [7; 32],
        }
        .with_tx_set_hash([8; 32]),
        &bundle,
        PublicTransferLimits {
            max_transcripts: 32,
            max_deltas: 32,
            max_rows: 64,
            max_public_bytes: 4_000_000,
            max_unique_keys: 64,
            max_allocation_steps: 256,
        },
        TransferSmtBuildLimits::for_update_limit(64).unwrap(),
    )
    .unwrap();
    assert_eq!(
        norito::encode_canonical(statement.statement())
            .unwrap()
            .len() as u64,
        expected.total_statement_bytes
    );
    assert_eq!(
        bundle
            .iter()
            .map(|item| norito::encode_canonical(item).unwrap().len() as u64)
            .sum::<u64>(),
        expected.input_transcript_bytes
    );
}

#[test]
fn physical_fee_fragments_reopen_the_same_entry_and_count_e_once() {
    let hash = Hash::new(b"fee owner");
    let bundle = bundle(hash);
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    let first_usage = tx.replace_bundle(&owner, &bundle[..1]).unwrap();
    tx.commit();

    let mut fee = ledger.transaction(context()).unwrap();
    let same = fee.open_entry(hash).unwrap();
    assert_eq!(same.inner.binding.id, owner.inner.binding.id);
    assert_eq!(fee.owner_usage(&owner).unwrap(), first_usage);
    let final_usage = fee.replace_bundle(&same, &bundle).unwrap();
    assert_consistent(&fee);
    assert_eq!(final_usage.executed_entries, 1);
    assert_eq!(
        fee.open_entry(hash).unwrap().inner.binding.id,
        owner.inner.binding.id
    );
    fee.commit();
    assert_eq!(ledger.usage(), reference(hash, &bundle));
}

#[test]
fn drop_restores_prior_fee_usage_and_discards_new_identity_bindings() {
    let hash = Hash::new(b"committed business");
    let other = Hash::new(b"discarded physical owner");
    let bundle = bundle(hash);
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    let committed = tx.replace_bundle(&owner, &bundle[..1]).unwrap();
    tx.commit();
    let discarded;
    {
        let mut fee = ledger.transaction(context()).unwrap();
        fee.replace_bundle(&owner, &bundle).unwrap();
        discarded = fee.open_entry(other).unwrap();
    }
    assert_eq!(ledger.usage(), committed);
    let mut retry = ledger.transaction(context()).unwrap();
    assert_eq!(
        retry.owner_usage(&discarded),
        Err(ReservationError::Invariant(
            ReservationInvariant::StaleOwner
        ))
    );
    let replacement = retry.open_entry(other).unwrap();
    assert_ne!(replacement.inner.binding.id, discarded.inner.binding.id);
    assert_eq!(retry.usage().executed_entries, 2);
    retry.replace_bundle(&owner, &bundle).unwrap();
    assert_consistent(&retry);
}

#[test]
fn savepoints_restore_whole_frames_and_prevent_identity_or_checkpoint_aba() {
    let hash = Hash::new(b"savepoint owner");
    let other = Hash::new(b"savepoint discarded");
    let bundle = bundle(hash);
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    let first = tx.replace_bundle(&owner, &bundle[..1]).unwrap();
    let ancestor = tx.checkpoint();
    let discarded = tx.open_entry(other).unwrap();
    tx.replace_bundle(&owner, &bundle).unwrap();
    let branch = tx.checkpoint();
    assert_eq!(tx.rollback(ancestor).unwrap(), first);
    assert_consistent(&tx);
    let recreated = tx.open_entry(other).unwrap();
    assert_ne!(recreated.inner.binding.id, discarded.inner.binding.id);
    tx.replace_bundle(&owner, &bundle).unwrap();
    let before = tx.usage();
    assert_eq!(
        tx.rollback(branch),
        Err(ReservationError::Invariant(
            ReservationInvariant::StaleCheckpoint
        ))
    );
    assert_eq!(tx.usage(), before);
    assert_consistent(&tx);
    assert_eq!(
        tx.open_entry(other).unwrap().inner.binding.id,
        recreated.inner.binding.id
    );
}

#[test]
fn empty_entry_or_cleared_bundle_retains_e_and_recomputes_other_maximum() {
    let hash = Hash::new(b"larger entry");
    let other = Hash::new(b"smaller entry");
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let first = tx.open_entry(hash).unwrap();
    let second = tx.open_entry(other).unwrap();
    let empty = tx.replace_bundle(&first, &[]).unwrap();
    assert_consistent(&tx);
    assert_eq!(
        empty,
        SourceUsage {
            executed_entries: 1,
            ..SourceUsage::ZERO
        }
    );
    tx.replace_bundle(&first, &bundle(hash)).unwrap();
    let smaller = tx
        .replace_bundle(&second, &[transcript(other, 3, 10, 0)])
        .unwrap();
    assert!(tx.usage().max_statement_bytes > smaller.max_statement_bytes);
    tx.replace_bundle(&first, &[]).unwrap();
    assert_consistent(&tx);
    assert_eq!(tx.usage().max_statement_bytes, smaller.max_statement_bytes);
    assert_eq!(
        tx.usage().total_statement_bytes,
        smaller.total_statement_bytes
    );
    assert_eq!(tx.usage().executed_entries, 2);
    assert_eq!(tx.usage().transcripts, 1);
}

#[test]
fn whole_bundle_intrinsic_limit_is_inclusive_and_failure_is_atomic() {
    let hash = Hash::new(b"intrinsic entry");
    let bundle = bundle(hash);
    let first = reference(hash, &bundle[..1]);
    let full = reference(hash, &bundle);
    let mut limited = EntryBundleReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: SourceUsage {
                max_statement_bytes: first.max_statement_bytes,
                ..caps()
            },
            block: caps(),
        },
        construction(),
    )
    .unwrap();
    let mut tx = limited.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    tx.replace_bundle(&owner, &bundle[..1]).unwrap();
    assert_eq!(
        tx.replace_bundle(&owner, &bundle),
        Err(EntryBundleReservationError::Reservation(
            ReservationError::Intrinsic {
                dimension: SourceDimension::IndividualStatementBytes,
                actual: full.max_statement_bytes,
                maximum: first.max_statement_bytes,
            }
        ))
    );
    assert_eq!(tx.usage(), first);
}

#[test]
fn remaining_block_capacity_excludes_the_owners_replaced_frame() {
    let hash = Hash::new(b"capacity owner");
    let other = Hash::new(b"capacity other");
    let bundle = bundle(hash);
    let first = reference(hash, &bundle[..1]);
    let full = reference(hash, &bundle);
    let mut limited = EntryBundleReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: SourceUsage {
                total_statement_bytes: full.total_statement_bytes,
                ..caps()
            },
            block: SourceUsage {
                total_statement_bytes: full.total_statement_bytes,
                ..caps()
            },
        },
        construction(),
    )
    .unwrap();
    let mut tx = limited.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    tx.replace_bundle(&owner, &bundle[..1]).unwrap();
    // Replacing within the same entry fits exactly; its old frame is not added.
    tx.replace_bundle(&owner, &bundle).unwrap();
    tx.replace_bundle(&owner, &[]).unwrap();
    let other_owner = tx.open_entry(other).unwrap();
    let other_usage = tx
        .replace_bundle(&other_owner, &[transcript(other, 3, 10, 0)])
        .unwrap();
    let before = tx.usage();
    assert_eq!(
        tx.replace_bundle(&owner, &bundle),
        Err(EntryBundleReservationError::Reservation(
            ReservationError::RemainingBlock {
                dimension: SourceDimension::TotalStatementBytes,
                required: full.total_statement_bytes,
                available: full.total_statement_bytes - other_usage.total_statement_bytes,
            }
        ))
    );
    assert_eq!(tx.usage(), before);
    assert_eq!(
        first.total_statement_bytes,
        other_usage.total_statement_bytes
    );
}

#[test]
fn malformed_or_wrong_hash_bundles_never_replace_a_live_contribution() {
    let hash = Hash::new(b"valid owner");
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    let original = tx
        .replace_bundle(&owner, &[transcript(hash, 3, 10, 0)])
        .unwrap();
    let mut missing = transcript(hash, 2, 7, 3);
    missing.poseidon_preimage_digest = None;
    for invalid in [
        vec![transcript(Hash::new(b"wrong identity"), 3, 10, 0)],
        vec![missing],
        vec![transcript(hash, 3, 10, 0), transcript(hash, 2, 9, 3)],
    ] {
        let before = norito::encode_canonical(&invalid).unwrap();
        let result = tx.replace_bundle(&owner, &invalid);
        assert!(matches!(
            result,
            Err(EntryBundleReservationError::Preparation(_))
        ));
        assert_eq!(tx.usage(), original);
        assert_eq!(norito::encode_canonical(&invalid).unwrap(), before);
    }
}

#[test]
fn foreign_owner_context_and_checkpoint_fail_before_source_preparation() {
    let hash = Hash::new(b"foreign capability");
    let mut first = ledger();
    let mut second = ledger();
    let mut tx = first.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    let checkpoint = tx.checkpoint();
    tx.commit();
    let private_calls = quantity_materializer_invocations_for_testing();
    let mut other = second.transaction(context()).unwrap();
    assert_eq!(
        other.replace_bundle(&owner, &bundle(hash)),
        Err(EntryBundleReservationError::Reservation(
            ReservationError::Invariant(ReservationInvariant::ForeignLedger)
        ))
    );
    assert_eq!(
        other.rollback(checkpoint),
        Err(ReservationError::Invariant(
            ReservationInvariant::ForeignLedger
        ))
    );
    assert_eq!(other.usage(), SourceUsage::ZERO);
    assert_eq!(
        quantity_materializer_invocations_for_testing(),
        private_calls
    );
    drop(other);
    assert_eq!(
        second
            .transaction(ReservationContext {
                height: 10,
                ..context()
            })
            .err(),
        Some(ReservationError::Invariant(
            ReservationInvariant::ContextMismatch
        ))
    );
}

#[test]
fn zero_entry_ceiling_and_incoherent_policy_remain_errors() {
    let mut empty = EntryBundleReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: SourceUsage::ZERO,
            block: SourceUsage::ZERO,
        },
        construction(),
    )
    .unwrap();
    let mut tx = empty.transaction(context()).unwrap();
    assert_eq!(
        tx.open_entry(Hash::new(b"zero limit")).err(),
        Some(ReservationError::Intrinsic {
            dimension: SourceDimension::ExecutedEntries,
            actual: 1,
            maximum: 0,
        })
    );
    assert_eq!(tx.usage(), SourceUsage::ZERO);
    assert_eq!(
        EntryBundleReservationLedger::new(
            context(),
            ReservationPolicy {
                intrinsic: caps(),
                block: SourceUsage::ZERO
            },
            construction(),
        )
        .err()
        .map(|error| error.to_string()),
        Some(
            ReservationError::Invariant(ReservationInvariant::InvalidPolicy {
                dimension: SourceDimension::ExecutedEntries,
            })
            .to_string()
        )
    );
}

#[test]
fn construction_failure_is_distinct_from_consensus_capacity_and_has_context() {
    let hash = Hash::new(b"construction cap");
    let mut limited = EntryBundleReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: caps(),
            block: caps(),
        },
        FastpqSourceStatementBuildLimits {
            max_deltas: 0,
            ..construction()
        },
    )
    .unwrap();
    let mut tx = limited.transaction(context()).unwrap();
    let owner = tx.open_entry(hash).unwrap();
    let before = tx.usage();
    let error = tx.replace_bundle(&owner, &bundle(hash)).unwrap_err();
    assert!(error.to_string().contains("complete-entry preparation"));
    assert!(matches!(error, EntryBundleReservationError::Preparation(_)));
    assert_eq!(tx.usage(), before);
    let capacity = EntryBundleReservationError::from(ReservationError::Invariant(
        ReservationInvariant::StaleOwner,
    ));
    assert!(capacity.to_string().contains("StaleOwner"));
}

#[test]
fn conversion_rejects_inconsistent_complete_entry_measurements() {
    let valid = FastpqSourceTranscriptUsage {
        transcripts: 2,
        deltas: 2,
        input_transcript_bytes: 30,
        max_statement_bytes: 20,
        total_statement_bytes: 20,
    };
    assert_eq!(occurrence_usage(valid).unwrap().usage().transcripts, 2);
    assert_eq!(
        occurrence_usage(FastpqSourceTranscriptUsage::default())
            .unwrap()
            .usage(),
        SourceUsage::ZERO
    );
    for invalid in [
        FastpqSourceTranscriptUsage {
            transcripts: 0,
            ..valid
        },
        FastpqSourceTranscriptUsage { deltas: 1, ..valid },
        FastpqSourceTranscriptUsage {
            input_transcript_bytes: 0,
            ..valid
        },
        FastpqSourceTranscriptUsage {
            max_statement_bytes: 19,
            ..valid
        },
        FastpqSourceTranscriptUsage {
            max_statement_bytes: 0,
            total_statement_bytes: 0,
            ..valid
        },
    ] {
        assert_eq!(
            occurrence_usage(invalid),
            Err(ReservationError::Invariant(
                ReservationInvariant::InvalidEntryBundleMeasurement
            ))
        );
    }
}
