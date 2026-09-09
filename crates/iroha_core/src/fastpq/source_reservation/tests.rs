//! Reservation lifecycle and accounting regressions without transcript construction.

use super::*;

fn context() -> ReservationContext {
    ReservationContext {
        height: 7,
        policy_digest: [3; 32],
        scope_tag: 2,
    }
}

fn generous() -> SourceUsage {
    SourceUsage {
        executed_entries: 20,
        transcripts: 100,
        deltas: 1000,
        input_transcript_bytes: 10000,
        max_statement_bytes: 1000,
        total_statement_bytes: 10000,
    }
}

fn ledger() -> ReservationLedger {
    ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: generous(),
            block: generous(),
        },
    )
    .unwrap()
}

fn occurrence(deltas: u64, input: u64, statement: u64) -> OccurrenceUsage {
    OccurrenceUsage::new(deltas, input, statement).unwrap()
}

fn invariant(error: ReservationError, expected: ReservationInvariant) {
    assert_eq!(error, ReservationError::Invariant(expected));
}

fn assert_consistent(tx: &ReservationTransaction<'_>) {
    let mut block = SourceUsage::ZERO;
    let mut maxima = BTreeMap::new();
    let mut sum = [0_u128; 3];
    for owner in tx.ledger.owners.values() {
        let mut local = SourceUsage {
            executed_entries: 1,
            ..SourceUsage::ZERO
        };
        let mut local_sums = [0_u128; 3];
        let mut local_maxima = BTreeMap::new();
        for slot in owner.slots.values() {
            local.transcripts += 1;
            local.deltas += slot.usage.deltas;
            local_sums[0] += u128::from(slot.usage.input_bytes);
            local_sums[1] += u128::from(slot.usage.statement_bytes);
            local.max_statement_bytes = local.max_statement_bytes.max(slot.usage.statement_bytes);
            *local_maxima
                .entry(slot.usage.statement_bytes)
                .or_insert(0_u64) += 1;
        }
        local.input_transcript_bytes = local_sums[0].try_into().unwrap();
        local.total_statement_bytes = local_sums[1].try_into().unwrap();
        assert_eq!(owner.usage, local);
        assert_eq!(owner.maxima, local_maxima);
        block.executed_entries += 1;
        block.transcripts += local.transcripts;
        sum[0] += u128::from(local.deltas);
        sum[1] += u128::from(local.input_transcript_bytes);
        sum[2] += u128::from(local.total_statement_bytes);
        block.max_statement_bytes = block.max_statement_bytes.max(local.max_statement_bytes);
        *maxima.entry(local.max_statement_bytes).or_insert(0_u64) += 1;
    }
    block.deltas = sum[0].try_into().unwrap();
    block.input_transcript_bytes = sum[1].try_into().unwrap();
    block.total_statement_bytes = sum[2].try_into().unwrap();
    assert_eq!(tx.usage(), block);
    assert_eq!(tx.ledger.maxima, maxima);
}

#[test]
fn whole_prefix_replacement_grows_and_shrinks_without_counting_another_transcript() {
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_owner().unwrap();
    let first = tx.reserve(&owner, occurrence(1, 10, 12)).unwrap();
    let grown = tx.replace(&owner, &first, occurrence(3, 35, 40)).unwrap();
    assert_eq!(
        tx.owner_usage(&owner).unwrap(),
        SourceUsage {
            executed_entries: 1,
            transcripts: 1,
            deltas: 3,
            input_transcript_bytes: 35,
            max_statement_bytes: 40,
            total_statement_bytes: 40,
        }
    );
    invariant(
        tx.replace(&owner, &first, occurrence(1, 2, 3)).unwrap_err(),
        ReservationInvariant::StaleSlot,
    );
    tx.replace(&owner, &grown, occurrence(2, 15, 10)).unwrap();
    assert_eq!(
        tx.usage(),
        SourceUsage {
            executed_entries: 1,
            transcripts: 1,
            deltas: 2,
            input_transcript_bytes: 15,
            max_statement_bytes: 10,
            total_statement_bytes: 10,
        }
    );
    assert_consistent(&tx);
    let committed = tx.commit();
    assert_eq!(ledger.usage(), committed);
}

#[test]
fn duplicate_owner_maxima_and_block_maxima_remain_exact_on_decreases() {
    let mut limits = generous();
    limits.max_statement_bytes = 30;
    let mut ledger = ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: limits,
            block: limits,
        },
    )
    .unwrap();
    let mut tx = ledger.transaction(context()).unwrap();
    let a = tx.open_owner().unwrap();
    let b = tx.open_owner().unwrap();
    let a0 = tx.reserve(&a, occurrence(1, 1, 30)).unwrap();
    let a1 = tx.reserve(&a, occurrence(1, 1, 30)).unwrap();
    let b0 = tx.reserve(&b, occurrence(1, 1, 25)).unwrap();
    tx.replace(&a, &a0, occurrence(1, 1, 10)).unwrap();
    assert_eq!(tx.usage().max_statement_bytes, 30);
    tx.replace(&a, &a1, occurrence(1, 1, 20)).unwrap();
    assert_eq!(tx.owner_usage(&a).unwrap().max_statement_bytes, 20);
    assert_eq!(tx.usage().max_statement_bytes, 25);
    tx.replace(&b, &b0, occurrence(1, 1, 5)).unwrap();
    assert_eq!(tx.usage().max_statement_bytes, 20);
    assert_eq!(tx.usage().total_statement_bytes, 35);
    assert_consistent(&tx);
}

#[test]
fn failed_capacity_replacement_is_atomic_and_excludes_the_whole_old_owner() {
    let mut limits = generous();
    limits.deltas = 5;
    let mut ledger = ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: limits,
            block: limits,
        },
    )
    .unwrap();
    let mut tx = ledger.transaction(context()).unwrap();
    let a = tx.open_owner().unwrap();
    let b = tx.open_owner().unwrap();
    let slot = tx.reserve(&a, occurrence(2, 4, 6)).unwrap();
    tx.reserve(&b, occurrence(2, 4, 6)).unwrap();
    let before = (tx.usage(), tx.journal.len(), tx.ledger.next_generation);
    assert_eq!(
        tx.replace(&a, &slot, occurrence(4, 8, 12)).unwrap_err(),
        ReservationError::RemainingBlock {
            dimension: SourceDimension::Deltas,
            required: 4,
            available: 3,
        }
    );
    assert_eq!(
        (tx.usage(), tx.journal.len(), tx.ledger.next_generation),
        before
    );
    tx.replace(&a, &slot, occurrence(3, 6, 9)).unwrap();
    assert_eq!(tx.usage().deltas, 5);
    assert_consistent(&tx);
}

#[test]
fn intrinsic_classification_uses_every_owner_occurrence_before_block_capacity() {
    let mut intrinsic = generous();
    intrinsic.deltas = 5;
    let mut block = generous();
    block.deltas = 6;
    let mut ledger =
        ReservationLedger::new(context(), ReservationPolicy { intrinsic, block }).unwrap();
    let mut tx = ledger.transaction(context()).unwrap();
    let a = tx.open_owner().unwrap();
    let b = tx.open_owner().unwrap();
    let a0 = tx.reserve(&a, occurrence(2, 2, 2)).unwrap();
    tx.reserve(&a, occurrence(2, 2, 2)).unwrap();
    tx.reserve(&b, occurrence(2, 2, 2)).unwrap();
    assert_eq!(
        tx.replace(&a, &a0, occurrence(4, 4, 4)).unwrap_err(),
        ReservationError::Intrinsic {
            dimension: SourceDimension::Deltas,
            actual: 6,
            maximum: 5,
        }
    );
    assert_eq!(tx.usage().deltas, 6);
    assert_consistent(&tx);
}

#[test]
fn committed_logical_owner_survives_fragments_and_aborted_business_scope() {
    let mut ledger = ledger();
    let mut seed = ledger.transaction(context()).unwrap();
    let owner = seed.open_owner().unwrap();
    seed.commit(); // Body-owned E survives ordinary business rejection.
    let mut fragment = ledger.transaction(context()).unwrap();
    let committed_slot = fragment.reserve(&owner, occurrence(2, 3, 4)).unwrap();
    fragment.commit();
    let committed = ledger.usage();
    {
        let mut business = ledger.transaction(context()).unwrap();
        business.reserve(&owner, occurrence(3, 5, 7)).unwrap();
        business
            .replace(&owner, &committed_slot, occurrence(4, 7, 9))
            .unwrap();
        assert_consistent(&business);
    }
    assert_eq!(ledger.usage(), committed);
    let mut fee_fragment = ledger.transaction(context()).unwrap();
    fee_fragment.reserve(&owner, occurrence(1, 2, 3)).unwrap();
    assert_eq!(fee_fragment.usage().executed_entries, 1);
    assert_eq!(fee_fragment.usage().transcripts, 2);
    assert_eq!(fee_fragment.usage().deltas, 3);
    assert_consistent(&fee_fragment);
    fee_fragment.commit();
}

#[test]
fn checkpoint_rollback_restores_old_handles_without_reviving_discarded_generations() {
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_owner().unwrap();
    let old = tx.reserve(&owner, occurrence(1, 2, 10)).unwrap();
    let checkpoint = tx.checkpoint();
    let discarded = tx.replace(&owner, &old, occurrence(2, 4, 20)).unwrap();
    let discarded_extra = tx.reserve(&owner, occurrence(1, 2, 30)).unwrap();
    let future_checkpoint = tx.checkpoint();
    tx.rollback(checkpoint).unwrap();
    let replacement = tx.replace(&owner, &old, occurrence(3, 6, 15)).unwrap();
    assert_ne!(replacement.generation, discarded.generation);
    invariant(
        tx.replace(&owner, &discarded, occurrence(1, 1, 1))
            .unwrap_err(),
        ReservationInvariant::StaleSlot,
    );
    let extra = tx.reserve(&owner, occurrence(1, 2, 5)).unwrap();
    assert_eq!(extra.id, discarded_extra.id);
    assert_ne!(extra.generation, discarded_extra.generation);
    invariant(
        tx.replace(&owner, &discarded_extra, occurrence(1, 1, 1))
            .unwrap_err(),
        ReservationInvariant::StaleSlot,
    );
    invariant(
        tx.rollback(future_checkpoint).unwrap_err(),
        ReservationInvariant::StaleCheckpoint,
    );
    assert_consistent(&tx);
    tx.commit();
}

#[test]
fn abort_does_not_reuse_owner_or_slot_generations_even_when_slot_ordinal_restarts() {
    let mut ledger = ledger();
    let mut seed = ledger.transaction(context()).unwrap();
    let stable = seed.open_owner().unwrap();
    seed.commit();
    let (discarded_owner, discarded_owner_slot, discarded_slot) = {
        let mut tx = ledger.transaction(context()).unwrap();
        let owner = tx.open_owner().unwrap();
        let own_slot = tx.reserve(&owner, occurrence(1, 1, 1)).unwrap();
        let slot = tx.reserve(&stable, occurrence(1, 1, 1)).unwrap();
        (owner, own_slot, slot)
    };
    let mut tx = ledger.transaction(context()).unwrap();
    let fresh_owner = tx.open_owner().unwrap();
    assert_ne!(
        fresh_owner.binding.generation,
        discarded_owner.binding.generation
    );
    invariant(
        tx.owner_usage(&discarded_owner).unwrap_err(),
        ReservationInvariant::StaleOwner,
    );
    invariant(
        tx.replace(&fresh_owner, &discarded_owner_slot, occurrence(1, 1, 1))
            .unwrap_err(),
        ReservationInvariant::ForeignSlotOwner,
    );
    let fresh = tx.reserve(&stable, occurrence(1, 1, 1)).unwrap();
    assert_eq!(fresh.id, discarded_slot.id);
    assert_ne!(fresh.generation, discarded_slot.generation);
    invariant(
        tx.replace(&stable, &discarded_slot, occurrence(1, 1, 1))
            .unwrap_err(),
        ReservationInvariant::StaleSlot,
    );
    assert_consistent(&tx);
}

#[test]
fn owner_context_ledger_and_slot_bindings_are_checked_before_accounting() {
    let mut first = ledger();
    let mut tx = first.transaction(context()).unwrap();
    let a = tx.open_owner().unwrap();
    let b = tx.open_owner().unwrap();
    let slot = tx.reserve(&a, occurrence(1, 2, 3)).unwrap();
    invariant(
        tx.replace(&b, &slot, occurrence(1, 1, 1)).unwrap_err(),
        ReservationInvariant::ForeignSlotOwner,
    );
    let mut stale_owner = a.clone();
    stale_owner.binding.generation += 1;
    invariant(
        tx.owner_usage(&stale_owner).unwrap_err(),
        ReservationInvariant::StaleOwner,
    );
    let mut wrong_context = a.clone();
    wrong_context.binding.context.scope_tag += 1;
    invariant(
        tx.owner_usage(&wrong_context).unwrap_err(),
        ReservationInvariant::ContextMismatch,
    );
    let mut stale_slot = slot.clone();
    stale_slot.generation += 1;
    invariant(
        tx.replace(&a, &stale_slot, occurrence(1, 1, 1))
            .unwrap_err(),
        ReservationInvariant::StaleSlot,
    );
    tx.commit();
    let mut second = ledger();
    let mut other = second.transaction(context()).unwrap();
    let foreign_slot_owner = other.open_owner().unwrap();
    invariant(
        other.owner_usage(&a).unwrap_err(),
        ReservationInvariant::ForeignLedger,
    );
    invariant(
        other
            .replace(&foreign_slot_owner, &slot, occurrence(1, 1, 1))
            .unwrap_err(),
        ReservationInvariant::ForeignLedger,
    );
    let before = first.next_generation;
    assert!(matches!(
        first.transaction(ReservationContext {
            height: 8,
            ..context()
        }),
        Err(ReservationError::Invariant(
            ReservationInvariant::ContextMismatch
        ))
    ));
    assert_eq!(first.next_generation, before);
}

#[test]
fn checkpoints_reject_foreign_transactions_ledgers_contexts_and_discarded_branches() {
    let mut ledger = ledger();
    let first = ledger.transaction(context()).unwrap();
    let old_transaction = first.checkpoint();
    first.commit();
    let mut tx = ledger.transaction(context()).unwrap();
    invariant(
        tx.rollback(old_transaction).unwrap_err(),
        ReservationInvariant::ForeignTransaction,
    );
    let mut other_ledger = ReservationLedger::new(context(), tx.ledger.policy).unwrap();
    let other = other_ledger.transaction(context()).unwrap();
    invariant(
        tx.rollback(other.checkpoint()).unwrap_err(),
        ReservationInvariant::ForeignLedger,
    );
    let mut wrong_context = tx.checkpoint();
    wrong_context.context.policy_digest[0] ^= 1;
    invariant(
        tx.rollback(wrong_context).unwrap_err(),
        ReservationInvariant::ContextMismatch,
    );
    let root = tx.checkpoint();
    let owner = tx.open_owner().unwrap();
    let inner = tx.checkpoint();
    tx.reserve(&owner, occurrence(1, 1, 1)).unwrap();
    let discarded = tx.checkpoint();
    tx.rollback(inner).unwrap();
    tx.reserve(&owner, occurrence(2, 2, 2)).unwrap(); // Refill the same offset.
    invariant(
        tx.rollback(discarded).unwrap_err(),
        ReservationInvariant::StaleCheckpoint,
    );
    tx.rollback(root).unwrap(); // The retained ancestor remains valid.
    assert_eq!(tx.usage(), SourceUsage::ZERO);
    assert_consistent(&tx);
}

#[test]
fn inclusive_caps_zero_caps_and_nonempty_occurrence_shapes_are_exact() {
    let cap = SourceUsage {
        executed_entries: 1,
        transcripts: 1,
        deltas: 1,
        input_transcript_bytes: 10,
        max_statement_bytes: 20,
        total_statement_bytes: 20,
    };
    let mut ledger = ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: cap,
            block: cap,
        },
    )
    .unwrap();
    let mut tx = ledger.transaction(context()).unwrap();
    let owner = tx.open_owner().unwrap();
    tx.reserve(&owner, occurrence(1, 10, 20)).unwrap();
    assert_eq!(tx.usage(), cap);
    assert_eq!(
        tx.open_owner().unwrap_err(),
        ReservationError::RemainingBlock {
            dimension: SourceDimension::ExecutedEntries,
            required: 1,
            available: 0,
        }
    );
    assert_consistent(&tx);
    let mut empty = ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic: SourceUsage::ZERO,
            block: SourceUsage::ZERO,
        },
    )
    .unwrap();
    let mut empty_tx = empty.transaction(context()).unwrap();
    assert_eq!(
        empty_tx.open_owner().unwrap_err(),
        ReservationError::Intrinsic {
            dimension: SourceDimension::ExecutedEntries,
            actual: 1,
            maximum: 0,
        }
    );
    for values in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
        invariant(
            OccurrenceUsage::new(values.0, values.1, values.2).unwrap_err(),
            ReservationInvariant::EmptyOccurrence,
        );
    }
}

#[test]
fn fixed_width_overflow_and_underflow_are_invariants_in_dimension_order() {
    for dimension in DIMENSIONS {
        if dimension == SourceDimension::IndividualStatementBytes {
            continue;
        }
        let mut maximum = SourceUsage::ZERO;
        maximum.set(dimension, u64::MAX);
        let mut one = SourceUsage::ZERO;
        one.set(dimension, 1);
        invariant(
            maximum.replaced(SourceUsage::ZERO, one, 0).unwrap_err(),
            ReservationInvariant::UsageOverflow { dimension },
        );
        invariant(
            SourceUsage::ZERO
                .replaced(one, SourceUsage::ZERO, 0)
                .unwrap_err(),
            ReservationInvariant::UsageUnderflow { dimension },
        );
    }
    let all = SourceUsage {
        executed_entries: u64::MAX,
        transcripts: u64::MAX,
        deltas: u64::MAX,
        input_transcript_bytes: u64::MAX,
        max_statement_bytes: u64::MAX,
        total_statement_bytes: u64::MAX,
    };
    for large in [
        occurrence(u64::MAX, 1, 1),
        occurrence(1, u64::MAX, 1),
        occurrence(1, 1, u64::MAX),
    ] {
        let mut ledger = ReservationLedger::new(
            context(),
            ReservationPolicy {
                intrinsic: all,
                block: all,
            },
        )
        .unwrap();
        let mut tx = ledger.transaction(context()).unwrap();
        let owner = tx.open_owner().unwrap();
        tx.reserve(&owner, large).unwrap();
        let before = tx.usage();
        let dimension = if large.deltas == u64::MAX {
            SourceDimension::Deltas
        } else if large.input_bytes == u64::MAX {
            SourceDimension::InputTranscriptBytes
        } else {
            SourceDimension::TotalStatementBytes
        };
        invariant(
            tx.reserve(&owner, occurrence(1, 1, 1)).unwrap_err(),
            ReservationInvariant::UsageOverflow { dimension },
        );
        assert_eq!(tx.usage(), before);
        assert_consistent(&tx);
    }
}

#[test]
fn inconsistent_intrinsic_profiles_are_rejected_without_invented_defaults() {
    for dimension in DIMENSIONS {
        let block = generous();
        let mut intrinsic = block;
        intrinsic.set(dimension, block.get(dimension) + 1);
        assert!(
            matches!(ReservationLedger::new(context(), ReservationPolicy { intrinsic, block }),
            Err(ReservationError::Invariant(ReservationInvariant::InvalidPolicy { dimension: actual })) if actual == dimension)
        );
    }
}

#[test]
fn exhausted_generations_fail_atomically_and_drop_rollback_needs_no_new_generation() {
    let mut ledger = ledger();
    let mut seed = ledger.transaction(context()).unwrap();
    let owner = seed.open_owner().unwrap();
    let slot = seed.reserve(&owner, occurrence(1, 2, 3)).unwrap();
    seed.commit();
    let committed = ledger.usage();
    {
        let mut tx = ledger.transaction(context()).unwrap();
        let checkpoint = tx.checkpoint();
        tx.reserve(&owner, occurrence(1, 2, 3)).unwrap();
        tx.ledger.next_generation = u64::MAX;
        let before = (tx.usage(), tx.journal.len());
        invariant(
            tx.replace(&owner, &slot, occurrence(2, 4, 6)).unwrap_err(),
            ReservationInvariant::GenerationOverflow,
        );
        assert_eq!((tx.usage(), tx.journal.len()), before);
        assert_eq!(tx.rollback(checkpoint).unwrap(), committed);
        assert_eq!(tx.ledger.next_generation, u64::MAX);
        assert_consistent(&tx);
    }
    assert_eq!(ledger.usage(), committed);
    assert!(matches!(
        ledger.transaction(context()),
        Err(ReservationError::Invariant(
            ReservationInvariant::GenerationOverflow
        ))
    ));
}

#[test]
fn nested_inner_rollback_then_outer_entry_rollback_preserves_prior_shared_transaction_work() {
    let mut ledger = ledger();
    let mut tx = ledger.transaction(context()).unwrap();
    let prior = tx.open_owner().unwrap();
    tx.reserve(&prior, occurrence(1, 2, 3)).unwrap();
    let prior_usage = tx.usage();
    let outer = tx.checkpoint();
    let entry = tx.open_owner().unwrap();
    let original = tx.reserve(&entry, occurrence(1, 2, 30)).unwrap();
    let inner = tx.checkpoint();
    tx.replace(&entry, &original, occurrence(2, 4, 40)).unwrap();
    tx.rollback(inner).unwrap();
    assert_eq!(tx.owner_usage(&entry).unwrap().max_statement_bytes, 30);
    tx.rollback(outer).unwrap();
    assert_eq!(tx.usage(), prior_usage);
    invariant(
        tx.owner_usage(&entry).unwrap_err(),
        ReservationInvariant::StaleOwner,
    );
    assert_consistent(&tx);
    tx.commit();
    assert_eq!(ledger.usage(), prior_usage);
}

#[test]
fn preseeded_entry_keeps_one_e_across_rejected_business_penalty_and_fee_fragments() {
    let mut intrinsic = generous();
    intrinsic.deltas = 3;
    let mut ledger = ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic,
            block: generous(),
        },
    )
    .unwrap();
    let mut seed = ledger.transaction(context()).unwrap();
    let owner = seed.open_owner().unwrap();
    seed.commit();
    {
        let mut business = ledger.transaction(context()).unwrap();
        business.reserve(&owner, occurrence(3, 30, 40)).unwrap();
    }
    assert_eq!(
        ledger.usage(),
        SourceUsage {
            executed_entries: 1,
            ..SourceUsage::ZERO
        }
    );
    let mut penalty = ledger.transaction(context()).unwrap();
    penalty.reserve(&owner, occurrence(2, 20, 12)).unwrap();
    penalty.commit();
    let mut fee = ledger.transaction(context()).unwrap();
    assert_eq!(
        fee.reserve(&owner, occurrence(2, 15, 18)).unwrap_err(),
        ReservationError::Intrinsic {
            dimension: SourceDimension::Deltas,
            actual: 4,
            maximum: 3,
        }
    );
    fee.reserve(&owner, occurrence(1, 15, 18)).unwrap();
    assert_eq!(
        fee.owner_usage(&owner).unwrap(),
        SourceUsage {
            executed_entries: 1,
            transcripts: 2,
            deltas: 3,
            input_transcript_bytes: 35,
            max_statement_bytes: 18,
            total_statement_bytes: 30,
        }
    );
    assert_consistent(&fee);
    let final_usage = fee.commit();
    assert_eq!(ledger.usage(), final_usage);
}

#[test]
fn intrinsic_dimensions_reject_in_source_order_before_any_failed_slot_mutation() {
    for dimension in [
        SourceDimension::Transcripts,
        SourceDimension::Deltas,
        SourceDimension::InputTranscriptBytes,
        SourceDimension::IndividualStatementBytes,
        SourceDimension::TotalStatementBytes,
    ] {
        let mut intrinsic = generous();
        intrinsic.set(dimension, 1);
        let mut ledger = ReservationLedger::new(
            context(),
            ReservationPolicy {
                intrinsic,
                block: generous(),
            },
        )
        .unwrap();
        let mut tx = ledger.transaction(context()).unwrap();
        let owner = tx.open_owner().unwrap();
        let old = tx.reserve(&owner, occurrence(1, 1, 1)).unwrap();
        let before = (tx.usage(), tx.journal.len(), tx.ledger.next_generation);
        let result = match dimension {
            SourceDimension::Transcripts | SourceDimension::TotalStatementBytes => {
                tx.reserve(&owner, occurrence(1, 1, 1))
            }
            SourceDimension::Deltas => tx.replace(&owner, &old, occurrence(2, 2, 2)),
            SourceDimension::InputTranscriptBytes => tx.replace(&owner, &old, occurrence(1, 2, 2)),
            SourceDimension::IndividualStatementBytes => {
                tx.replace(&owner, &old, occurrence(1, 1, 2))
            }
            SourceDimension::ExecutedEntries => unreachable!(),
        };
        assert_eq!(
            result.unwrap_err(),
            ReservationError::Intrinsic {
                dimension,
                actual: 2,
                maximum: 1
            }
        );
        assert_eq!(
            (tx.usage(), tx.journal.len(), tx.ledger.next_generation),
            before
        );
        assert_consistent(&tx);
    }
}

#[test]
fn failed_incremental_replacement_keeps_live_prefix_until_whole_attempt_drop() {
    let mut intrinsic = generous();
    intrinsic.deltas = 3;
    let mut ledger = ReservationLedger::new(
        context(),
        ReservationPolicy {
            intrinsic,
            block: generous(),
        },
    )
    .unwrap();
    let mut seed = ledger.transaction(context()).unwrap();
    let owner = seed.open_owner().unwrap();
    seed.reserve(&owner, occurrence(1, 2, 5)).unwrap();
    seed.commit();
    let before_loop = ledger.usage();
    {
        let mut attempt = ledger.transaction(context()).unwrap();
        let first = attempt.reserve(&owner, occurrence(1, 2, 10)).unwrap();
        let second = attempt
            .replace(&owner, &first, occurrence(2, 4, 20))
            .unwrap();
        let before_failure = attempt.usage();
        assert_eq!(
            attempt
                .replace(&owner, &second, occurrence(3, 6, 30))
                .unwrap_err(),
            ReservationError::Intrinsic {
                dimension: SourceDimension::Deltas,
                actual: 4,
                maximum: 3,
            }
        );
        assert_eq!(attempt.usage(), before_failure);
        assert!(attempt.validate_slot(&owner, &second).is_ok());
        assert_consistent(&attempt);
        // The adapter must unwind the whole attempt, not classify this as a rejected leg.
    }
    assert_eq!(ledger.usage(), before_loop);
}

#[test]
fn predeclared_time_e_survives_failure_but_lazy_dynamic_work_disappears_on_drop() {
    let mut ledger = ledger();
    let mut seed = ledger.transaction(context()).unwrap();
    let time_owner = seed.open_owner().unwrap();
    seed.commit();
    {
        let mut failed_time = ledger.transaction(context()).unwrap();
        failed_time
            .reserve(&time_owner, occurrence(1, 2, 3))
            .unwrap();
    }
    let time_e = SourceUsage {
        executed_entries: 1,
        ..SourceUsage::ZERO
    };
    assert_eq!(ledger.usage(), time_e);
    {
        let mut failed_dynamic = ledger.transaction(context()).unwrap();
        let dynamic = failed_dynamic.open_owner().unwrap();
        failed_dynamic
            .reserve(&dynamic, occurrence(1, 2, 3))
            .unwrap();
    }
    assert_eq!(ledger.usage(), time_e);
    // A transcript-free dynamic/quarantine fragment never mints another owner.
    let empty_dynamic = ledger.transaction(context()).unwrap();
    assert_eq!(empty_dynamic.commit(), time_e);
    assert_eq!(ledger.usage(), time_e);
}
