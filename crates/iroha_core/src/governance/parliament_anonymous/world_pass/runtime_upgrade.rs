//! G3 runtime preparation and its distinct selected, bounded result owner (N14).
//!
//! Candidate certificates are never listed and are refused again before reading an effect.
//! The native output grammar has no G3 receipt. A production certificate's effect is
//! prepared in its own child transaction, and only the selected result owner may consume it:
//! that owner admits the complete receipt into the block's bounded
//! [`RuntimeEnactmentResultsV1`] custody first and applies the original transaction only
//! after admission. A root selects the owner through an exact composed services profile
//! (`runtime::catalog::parliament_runtime_enactment`); this release composes it into no
//! profile, so every root keeps the original unsupported-runtime refusal before effects.
//! The retained private preparation drops a deterministically rejected effect transaction
//! before a fresh failure child; local refusal keeps its original retry owner intact.
//!
//! The owner publishes each receipt as committed World State: it writes the receipt to
//! `world.parliament_anonymous_runtime_enactments` in the effect's own transaction, so the
//! certified World state root of the block's result (`R`) binds receipt, effect and terminal
//! status together, and the block start seals the admitted receipts against those rows. The
//! native block outputs stay closed. A root selects the owner only when it also installs that
//! table, so a root that installed an older family descriptor keeps the original refusal.
//!
//! Activation still needs, all together: the qualified ballot authority
//! (`PARLIAMENT_BALLOT_QUALIFICATION_V1`, P1) and the composed service.

use super::*;
use crate::execution_attempt::ExecutionAttemptError;
use iroha_data_model::governance::types::ProposalKind;

pub(super) fn items(
    attempts: &[(GovernanceAttemptId, &Attempt)],
    now_ms: u64,
) -> Vec<GovernancePassItemV1> {
    attempts
        .iter()
        .filter_map(|(id, attempt)| {
            if !matches!(attempt.proposal(), ProposalKind::RuntimeUpgrade(_)) {
                return None;
            }
            let due_ms = attempt.enactment_due_ms()?;
            (due_ms <= now_ms).then_some(GovernancePassItemV1::EnactParliament {
                due_ms,
                attempt: *id,
            })
        })
        .collect()
}

pub(super) fn live(attempt: &Attempt, due_ms: u64) -> bool {
    matches!(attempt.proposal(), ProposalKind::RuntimeUpgrade(_))
        && attempt.enactment_due_ms() == Some(due_ms)
}

/// The root selects no G3 result owner, so no current result profile can commit the
/// standalone G3 receipt. This is local unsupported-runtime recovery, never a rejection or an
/// applied item, and it is reached before any effect is prepared.
fn require_result_profile(state: &StateBlock<'_>) -> Result<core::convert::Infallible, PassError> {
    Err(PassError::Storage(
        StateStorageAdmissionError::UnsupportedRuntime {
            height: state._curr_block.height().get(),
            reason: "runtime enactment requires a distinct selected governed result profile".into(),
        },
    ))
}

/// Proof that this block's root selects the distinct G3 result owner: its committed selection
/// installs the anonymous family and its exact services profile composes the result service.
/// Outside tests only [`Self::of`] constructs it.
struct SelectedResultOwner(());

impl SelectedResultOwner {
    /// The owner when the root of `state` selects it.
    ///
    /// # Errors
    /// A selection this release cannot decide (an uncompiled services profile, the service
    /// under another closure, or storage it does not implement) is a local unsupported-runtime
    /// refusal, never "not selected".
    fn of(state: &StateBlock<'_>) -> Result<Option<Self>, PassError> {
        let selected = crate::runtime::catalog::parliament_runtime_enactment::selected(
            state.world.runtime_selection(),
        )
        .map_err(|reason| {
            PassError::Storage(StateStorageAdmissionError::UnsupportedRuntime {
                height: state._curr_block.height().get(),
                reason,
            })
        })?;
        Ok(selected.then_some(Self(())))
    }

    /// A selected owner for boundary controls of [`produce`]. Without a qualification record
    /// (P1) no production certificate exists, so no test can reach it through [`apply`].
    #[cfg(test)]
    const fn for_test() -> Self {
        Self(())
    }
}

/// The selected result custody's capacity, observed immediately before the item is charged.
/// A live G3 enactment whose receipt would not fit stays due, uncharged, exactly like a native
/// item whose rows do not fit; every other item and occurrence is returned unchanged. A root
/// that selects no result owner never admits a receipt, so its custody always fits and its
/// items still reach the original refusal.
pub(super) fn inspect_result_capacity(
    item: GovernancePassItemV1,
    occurrence: GovernanceOccurrenceV1,
    results: &RuntimeEnactmentResultsV1,
) -> GovernanceOccurrenceV1 {
    // SPEC/N14: a G3 enactment is charged only if its receipt fits the selected custody.
    if matches!(item, GovernancePassItemV1::EnactParliament { .. })
        && occurrence == GovernanceOccurrenceV1::Live
        && !results.fits()
    {
        GovernanceOccurrenceV1::NoOutputCapacity
    } else {
        occurrence
    }
}

pub(super) fn apply(
    state: &mut StateBlock<'_>,
    id: GovernanceAttemptId,
    at: GovernanceAnchorV1,
    results: &mut RuntimeEnactmentResultsV1,
) -> Result<(), PassError> {
    // Retain the original purpose/time/source error ordering. Reading this disposable
    // child grants no business effects, status mutation or output publication.
    let tx = state.try_transaction().map_err(PassError::Storage)?;
    let attempt = stored_attempt(&tx, id)?;
    attempt.certificate_for_execution(id, at)?;
    drop(tx);
    // Only the distinctly selected result owner may consume a prepared effect.
    let Some(owner) = SelectedResultOwner::of(state)? else {
        return match require_result_profile(state) {
            Err(error) => Err(error),
            Ok(unavailable) => match unavailable {},
        };
    };
    with_prepared(state, id, at, |prepared| {
        produce(&owner, prepared, at, results)
    })
}

/// Disposable original effects plus their distinct, still-unpublished receipt.
/// No caller-supplied receipt, raw World or boolean can apply this owner: only [`produce`]
/// applies its transaction, and only after the receipt entered the selected custody.
struct PreparedRuntimeEnactment<'block, 'state> {
    transaction: StateTransaction<'block, 'state>,
    receipt: RuntimeEnactmentReceiptV1,
}

/// The selected G3 result owner: commit the complete original receipt into World within the
/// effect's own transaction, admit it into this block's bounded result custody, then apply that
/// one transaction. The certified World state root of the block's result binds the receipt
/// together with the effect and the attempt's terminal status. Every refusal drops the
/// transaction unapplied, so no business effect, terminal status or receipt survives it.
fn produce(
    _selected: &SelectedResultOwner,
    prepared: PreparedRuntimeEnactment<'_, '_>,
    at: GovernanceAnchorV1,
    results: &mut RuntimeEnactmentResultsV1,
) -> Result<(), PassError> {
    let PreparedRuntimeEnactment {
        mut transaction,
        receipt,
    } = prepared;
    if receipt.anchor != at {
        return Err(PassError::World(
            "a prepared runtime receipt names another pass position",
        ));
    }
    let id = receipt.governance_attempt_id;
    if transaction
        .world
        .parliament_anonymous_runtime_enactments
        .get(&id)
        .is_some()
    {
        return Err(PassError::World(
            "a runtime attempt already holds a committed enactment receipt",
        ));
    }
    // SPEC/N14: the receipt is published as committed World State in the effect's own
    // transaction; R's World state root certifies both or neither.
    transaction
        .world
        .parliament_anonymous_runtime_enactments
        .insert(id, receipt);
    transaction
        .require_storage_admission()
        .map_err(PassError::Storage)?;
    results.admit(receipt).map_err(PassError::Results)?;
    // SPEC/N14: the complete receipt is committed and in custody before any effect applies.
    transaction.apply();
    Ok(())
}

/// Prepare the original effect and receipt within their original child borrow. Only the
/// selected result owner ([`produce`]) consumes this owner; it admits the complete receipt
/// before applying the original transaction.
fn with_prepared<'state, R>(
    state: &mut StateBlock<'state>,
    id: GovernanceAttemptId,
    at: GovernanceAnchorV1,
    consume: impl for<'block> FnOnce(PreparedRuntimeEnactment<'block, 'state>) -> Result<R, PassError>,
) -> Result<R, PassError> {
    let mut tx = state.try_transaction().map_err(PassError::Storage)?;
    let mut attempt = stored_attempt(&tx, id)?;
    // This existing owner verifies purpose, certification and exact governance time before
    // any caller-provided proposal, compare-and-set observation or runtime effect is used.
    attempt.certificate_for_execution(id, at)?;
    let ProposalKind::RuntimeUpgrade(proposal) = attempt.proposal() else {
        return Err(PassError::World(
            "G3 runtime item names another proposal kind",
        ));
    };
    let observed =
        crate::runtime::change_set::anonymous::expected_head(proposal, &tx).map_err(|error| {
            match error {
                ExecutionAttemptError::Deferred(original) => PassError::Execution(original),
                ExecutionAttemptError::Rejected(_) => {
                    PassError::World("runtime certificate subject head is invalid")
                }
            }
        })?;
    if observed != attempt.expected_head() {
        attempt.mark_superseded(id, at, observed)?;
    } else {
        match crate::runtime::change_set::anonymous::enact(&mut tx, proposal) {
            Ok(()) => attempt.mark_enacted(id, at)?,
            Err(ExecutionAttemptError::Deferred(original)) => {
                return Err(PassError::Execution(original));
            }
            Err(ExecutionAttemptError::Rejected(_)) => {
                // No partial business effect can survive a caught semantic rejection.
                drop(tx);
                tx = state.try_transaction().map_err(PassError::Storage)?;
                attempt.mark_execution_failed(id, at)?;
            }
        }
    }
    let receipt = RuntimeEnactmentReceiptV1 {
        anchor: at,
        governance_attempt_id: attempt.attempt().id,
        status: attempt.attempt().status,
        stage: attempt.attempt().stage,
        attempt_state_digest: attempt_state_digest_v1(&attempt).map_err(PassError::Receipt)?,
    };
    receipt
        .validate()
        .map_err(|_| PassError::World("runtime enactment produced an invalid receipt"))?;
    store_attempt(&mut tx.world, id, attempt);
    tx.require_storage_admission().map_err(PassError::Storage)?;
    // Deliberately no tx.apply(): only the selected result owner applies it, after admission.
    consume(PreparedRuntimeEnactment {
        transaction: tx,
        receipt,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Rows, header};
    use super::*;
    use crate::governance::parliament_anonymous::{AnonymousBallotDecisionV1, test_driver::Driver};
    use iroha_data_model::governance::types::{ParliamentBody, RiskTierV1};

    fn full_native_partition() -> GovernanceStartOutputBudgetV1 {
        let mut rows = GovernanceStartOutputBudgetV1::new();
        rows.close_prelude();
        let mut position = 0;
        while rows.fits(GovernanceStartItemCeilingV1::Receipt) {
            rows.admit_item(
                GovernanceStartItemCeilingV1::Receipt,
                vec![GovernanceBlockStartOutputV1 {
                    anchor: GovernanceOutputAnchorV1::Pass(GovernanceAnchorV1 {
                        height: 5,
                        position,
                        timestamp_ms: 300_000,
                    }),
                    event: GovernanceBlockStartEventV1::NativeSlotAdvanced(NativeSlotAdvancedV1 {
                        target_height: 5,
                        cursor_height: 4,
                        ticket_height: 5,
                    }),
                }],
            )
            .unwrap();
            position += 1;
        }
        rows
    }

    // Exercise the actual shared cursor at the result-owner boundary. `Live` in these
    // boundary controls is an occurrence input, never a manufactured qualified certificate.
    // The separate candidate test below exercises the original certificate owner itself.
    fn cursor_before_g3(block: &mut StateBlock<'_>, exhaust: bool) -> SharedGovernancePassV1 {
        use GovernancePassSegmentV1 as Segment;
        let mut pass = SharedGovernancePassV1::begin::<PassError>(block).unwrap();
        pass.run_segment::<PassError>(
            block,
            Segment::G1,
            vec![GovernancePassItemV1::HeightProgress],
            |_, _| Ok(GovernanceOccurrenceV1::Live),
            |state, _, _| {
                let mut tx = state.try_transaction().map_err(PassError::Storage)?;
                tx.world
                    .smart_contract_state
                    .insert("g3-earlier-pass-effect".parse().unwrap(), vec![7]);
                tx.apply();
                Ok(())
            },
        )
        .unwrap();
        let mut work = Vec::new();
        if exhaust {
            let unit = AnonymousLifecycleItemV1::StartQualification.cost();
            for i in 0..pass.remaining() / unit {
                work.push(GovernancePassItemV1::Lifecycle(AnonymousDueItemV1 {
                    due_ms: u64::from(i),
                    governance_attempt_id: GovernanceAttemptId::new([3; 32]),
                    item: AnonymousLifecycleItemV1::StartQualification,
                }));
            }
        }
        pass.run_segment::<PassError>(
            block,
            Segment::G2a,
            work,
            |_, _| Ok(GovernanceOccurrenceV1::Live),
            |_, _, _| Ok(()),
        )
        .unwrap();
        for segment in [Segment::G2b, Segment::G2c] {
            pass.run_segment::<PassError>(
                block,
                segment,
                Vec::new(),
                |_, _| unreachable!("empty segment"),
                |_, _, _| unreachable!("empty segment"),
            )
            .unwrap();
        }
        pass
    }

    #[test]
    fn full_native_partition_cannot_hide_the_g3_result_owner_refusal() {
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let mut block = state.block(header(5, 300_000));
        let mut pass = cursor_before_g3(&mut block, false);
        let rows = full_native_partition();
        let held_rows = rows.rows().len();
        let remaining = pass.remaining();
        let item = GovernancePassItemV1::EnactParliament {
            due_ms: 0,
            attempt: GovernanceAttemptId::new([4; 32]),
        };
        assert_eq!(output_ceiling(item), None);
        let failure = pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![item],
            |_, item| {
                Ok(inspect_native_capacity(
                    item,
                    GovernanceOccurrenceV1::Live,
                    &rows,
                ))
            },
            |state, _, at| {
                assert_eq!(
                    at,
                    GovernanceAnchorV1 {
                        height: 5,
                        position: 1,
                        timestamp_ms: 300_000
                    }
                );
                match require_result_profile(state) {
                    Err(error) => Err(error),
                    Ok(unavailable) => match unavailable {},
                }
            },
        );
        assert!(matches!(
            failure,
            Err(GovernancePassErrorV1::Owner(PassError::Storage(
                StateStorageAdmissionError::UnsupportedRuntime { height: 5, .. }
            )))
        ));
        assert_eq!(pass.processed(), 1);
        assert_eq!(pass.remaining(), remaining);
        assert_eq!(rows.rows().len(), held_rows);
        assert!(matches!(
            pass.run_segment::<PassError>(
                &mut block,
                GovernancePassSegmentV1::G3,
                vec![item],
                |_, _| panic!("failed owner poisoned the cursor"),
                |_, _, _| panic!("failed owner poisoned the cursor"),
            ),
            Err(GovernancePassErrorV1::Segment)
        ));
        let key: iroha_model_base::state_path::StatePath =
            "g3-earlier-pass-effect".parse().unwrap();
        assert_eq!(block.world.smart_contract_state.get(&key), Some(&vec![7]));
        drop(block);
        assert!(
            state.world.smart_contract_state.view().get(&key).is_none(),
            "abandoning the failed pass must discard even its earlier applied child"
        );
        assert!(
            iroha_data_model::governance::anonymous_ballot::PARLIAMENT_BALLOT_QUALIFICATION_V1
                .is_none()
        );
    }

    #[test]
    fn full_native_partition_preserves_lapsed_and_ordinary_occurrences() {
        let rows = full_native_partition();
        let item = GovernancePassItemV1::EnactParliament {
            due_ms: 0,
            attempt: GovernanceAttemptId::new([4; 32]),
        };
        assert_eq!(
            inspect_native_capacity(item, GovernanceOccurrenceV1::Lapsed, &rows),
            GovernanceOccurrenceV1::Lapsed
        );
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let mut block = state.block(header(5, 300_000));
        assert!(
            matches!(
                inspect(&block, item, 0, &rows),
                Err(PassError::World("an item names no stored attempt"))
            ),
            "a full partition must preserve the original occurrence error"
        );
        let mut pass = cursor_before_g3(&mut block, false);
        let remaining = pass.remaining();
        pass.run_segment::<PassError>(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![item],
            |_, item| {
                Ok(inspect_native_capacity(
                    item,
                    GovernanceOccurrenceV1::Lapsed,
                    &rows,
                ))
            },
            |_, _, _| panic!("a lapsed item cannot reach any result owner"),
        )
        .unwrap();
        assert_eq!(pass.processed(), 1);
        assert_eq!(pass.remaining(), remaining);
        drop(block);

        let mut block = state.block(header(5, 300_000));
        let mut pass = SharedGovernancePassV1::begin::<PassError>(&block).unwrap();
        let remaining = pass.remaining();
        let ordinary = GovernancePassItemV1::HeightProgress;
        assert_eq!(
            inspect(&block, ordinary, 0, &rows).unwrap(),
            GovernanceOccurrenceV1::NoOutputCapacity
        );
        pass.run_segment::<PassError>(
            &mut block,
            GovernancePassSegmentV1::G1,
            vec![ordinary],
            |state, item| inspect(state, item, 0, &rows),
            |_, _, _| panic!("ordinary native capacity still defers"),
        )
        .unwrap();
        assert_eq!(pass.processed(), 0);
        assert_eq!(pass.remaining(), remaining);
    }

    #[test]
    fn g3_without_a_native_ceiling_still_obeys_the_shared_work_budget() {
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let mut block = state.block(header(5, 300_000));
        let mut pass = cursor_before_g3(&mut block, true);
        let rows = full_native_partition();
        let item = GovernancePassItemV1::EnactParliament {
            due_ms: 0,
            attempt: GovernanceAttemptId::new([4; 32]),
        };
        assert!(pass.remaining() < item.cost());
        let before = (pass.processed(), pass.remaining());
        pass.run_segment::<PassError>(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![item],
            |_, item| {
                Ok(inspect_native_capacity(
                    item,
                    GovernanceOccurrenceV1::Live,
                    &rows,
                ))
            },
            |_, _, _| panic!("the original work budget stops G3 before apply"),
        )
        .unwrap();
        assert_eq!((pass.processed(), pass.remaining()), before);
    }

    #[test]
    fn block_start_keeps_the_original_reducer_codec_refusal() {
        let error = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || norito::core::classify_decode_attempt(|| norito::core::reserve_decode_allocation(1)),
        )
        .unwrap_err();
        let original = error.local_refusal().unwrap();
        let pass = PassError::from(Error::from(error));
        let PassError::Execution(deferred) = pass else {
            panic!("the original codec refusal must retain its block retry owner");
        };
        assert_eq!(deferred.decode_refusal(), Some(&original));
    }

    #[test]
    fn missing_g3_result_owner_is_typed_local_refusal_before_any_effect() {
        use crate::state::{State, World};
        let state = State::new_for_testing(
            World::new(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let block = state.block(header(5, 300_000));
        let attempt_count = block.world.parliament_anonymous_attempts.iter().count();
        let scheduled_count = block.world.runtime_change_sets.iter().count();
        let selection = block.world.runtime_selection.get().clone();
        let error = require_result_profile(&block).unwrap_err();
        assert!(error.is_local());
        assert!(matches!(
            &error,
            PassError::Storage(StateStorageAdmissionError::UnsupportedRuntime { height: 5, .. })
        ));
        let mapped = crate::state::governance_start::parliament_error::<()>(error);
        assert!(matches!(
            mapped,
            crate::state::StateBlockStartError::Storage(
                StateStorageAdmissionError::UnsupportedRuntime { height: 5, .. }
            )
        ));
        assert_eq!(
            block.world.parliament_anonymous_attempts.iter().count(),
            attempt_count
        );
        assert_eq!(
            block.world.runtime_change_sets.iter().count(),
            scheduled_count
        );
        assert_eq!(block.world.runtime_selection.get(), &selection);
        assert!(block.governance_start_rows().is_none());
        drop(block);
        assert_eq!(
            state
                .world
                .parliament_anonymous_attempts
                .view()
                .iter()
                .count(),
            0
        );
    }

    #[test]
    fn candidate_certificate_never_lists_a_runtime_enactment() {
        use iroha_data_model::{
            governance::types::RuntimeUpgradeProposal,
            runtime::lifecycle::{ChangeSetId, RuntimeNoticeV1, RuntimeUpgradeActionV1},
        };
        let proposal = ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
            proposal_operator: crate::governance::parliament_anonymous::test_driver::operator(),
            action: RuntimeUpgradeActionV1::Cancel(ChangeSetId([8; 32])),
            notice: RuntimeNoticeV1 {
                blocks: 2,
                ms: 30_000,
                approval_cushion_ms: 0,
            },
            proposed_ms: 1_000_000,
        });
        let mut driver =
            Driver::for_proposal(proposal, RiskTierV1::Standard).drive_to_policy_reflection();
        assert_eq!(
            driver.close_ballot(ParliamentBody::PolicyJury, (20, 5, 0)),
            Ok(AnonymousBallotDecisionV1::Certified)
        );
        let original = driver.state.clone();
        assert!(items(&[(driver.id(), &driver.state)], u64::MAX).is_empty());
        assert!(!live(&driver.state, u64::MAX));
        // Even a manually routed item has to pass the same certificate purpose gate.
        assert!(
            driver
                .state
                .certificate_for_execution(
                    driver.id(),
                    GovernanceAnchorV1 {
                        height: 900_000,
                        position: 0,
                        timestamp_ms: u64::MAX / 2,
                    }
                )
                .is_err()
        );
        assert_eq!(driver.state, original);
        let mut source = Rows::default();
        source.attempts.push((driver.id(), driver.state.clone()));
        let state = source.state();
        let at = GovernanceAnchorV1 {
            height: 900_000,
            position: 0,
            timestamp_ms: u64::MAX / 2,
        };
        let mut block = state.block(header(at.height, at.timestamp_ms));
        let mut results = RuntimeEnactmentResultsV1::new(at.height, at.timestamp_ms);
        assert!(
            matches!(
                with_prepared(&mut block, driver.id(), at, |_| -> Result<(), PassError> {
                    panic!("candidate certificates grant no prepared result owner")
                }),
                Err(PassError::Reducer(_))
            ),
            "scoped preparation must retain the original certificate-purpose gate"
        );
        assert!(
            matches!(
                apply(&mut block, driver.id(), at, &mut results),
                Err(PassError::Reducer(_))
            ),
            "candidate purpose must refuse before the result owner is selected or consulted"
        );
        assert_eq!(
            block.world.parliament_anonymous_attempts.get(&driver.id()),
            Some(&original)
        );
        assert!(block.world.runtime_change_sets.iter().next().is_none());
        assert!(results.is_empty());
        let full = RefCell::new(full_native_partition());
        let item = GovernancePassItemV1::EnactParliament {
            due_ms: 0,
            attempt: driver.id(),
        };
        assert_eq!(
            inspect(&block, item, 0, &full.borrow()).unwrap(),
            GovernanceOccurrenceV1::Lapsed,
            "the original candidate occurrence cannot become production work"
        );
        // Manually routed work still checks the actual candidate certificate before the
        // unsupported result owner. This supplies no production qualification or effect.
        let mut pass = cursor_before_g3(&mut block, false);
        let original_rows = full.borrow().rows().len();
        let results = RefCell::new(RuntimeEnactmentResultsV1::new(at.height, at.timestamp_ms));
        let failure = pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![item],
            |_, item| {
                Ok(inspect_result_capacity(
                    item,
                    inspect_native_capacity(item, GovernanceOccurrenceV1::Live, &full.borrow()),
                    &results.borrow(),
                ))
            },
            |state, _, at| apply(state, driver.id(), at, &mut results.borrow_mut()),
        );
        assert!(
            matches!(
                failure,
                Err(GovernancePassErrorV1::Owner(PassError::Reducer(_)))
            ),
            "a full native partition must not mask the original candidate-purpose error"
        );
        assert_eq!(pass.processed(), 1);
        assert_eq!(full.borrow().rows().len(), original_rows);
        assert!(results.borrow().is_empty());
        assert_eq!(
            block.world.parliament_anonymous_attempts.get(&driver.id()),
            Some(&original)
        );
        assert!(block.world.runtime_change_sets.iter().next().is_none());
        assert!(
            iroha_data_model::governance::anonymous_ballot::PARLIAMENT_BALLOT_QUALIFICATION_V1
                .is_none()
        );
    }

    // ---- N14 selected result owner -------------------------------------------------------
    //
    // With no qualification record (P1) no production certificate exists, so `with_prepared`
    // cannot hand these controls a prepared owner. They build the owner directly with a real
    // child transaction of a real block and exercise the actual consumer, custody and cursor.

    fn effect_key() -> iroha_model_base::state_path::StatePath {
        "g3-selected-effect".parse().unwrap()
    }

    /// A terminal receipt of attempt `tag` at `at`.
    fn receipt_at(at: GovernanceAnchorV1, tag: u8) -> RuntimeEnactmentReceiptV1 {
        RuntimeEnactmentReceiptV1 {
            anchor: at,
            governance_attempt_id: GovernanceAttemptId::new([tag; 32]),
            status: GovernanceAttemptStatusV1::Enacted,
            stage: iroha_data_model::governance::types::GovernanceStageV1::default(),
            attempt_state_digest: [tag; 64].into(),
        }
    }

    /// A prepared owner whose original child transaction writes `value` as its effect.
    fn prepared<'block, 'state>(
        block: &'block mut StateBlock<'state>,
        receipt: RuntimeEnactmentReceiptV1,
        value: u8,
    ) -> PreparedRuntimeEnactment<'block, 'state> {
        let mut transaction = block.try_transaction().unwrap();
        transaction
            .world
            .smart_contract_state
            .insert(effect_key(), vec![value]);
        PreparedRuntimeEnactment {
            transaction,
            receipt,
        }
    }

    const AT: GovernanceAnchorV1 = GovernanceAnchorV1 {
        height: 5,
        position: 3,
        timestamp_ms: 300_000,
    };

    /// The committed receipt row of attempt `tag`, if any.
    fn committed(block: &StateBlock<'_>, tag: u8) -> Option<RuntimeEnactmentReceiptV1> {
        block
            .world
            .parliament_anonymous_runtime_enactments
            .get(&GovernanceAttemptId::new([tag; 32]))
            .copied()
    }

    #[test]
    fn selected_owner_commits_the_receipt_with_its_effect_before_applying() {
        // A fixture root whose selection installs the anonymous family and its receipt table.
        let state = Rows::default().state();
        let mut block = state.block(header(AT.height, AT.timestamp_ms));
        let mut results = RuntimeEnactmentResultsV1::new(AT.height, AT.timestamp_ms);
        let receipt = receipt_at(AT, 4);
        produce(
            &SelectedResultOwner::for_test(),
            prepared(&mut block, receipt, 9),
            AT,
            &mut results,
        )
        .unwrap();
        assert_eq!(results.receipts(), &[receipt]);
        assert_eq!(committed(&block, 4), Some(receipt));
        assert_eq!(
            block.world.smart_contract_state.get(&effect_key()),
            Some(&vec![9])
        );
        // The block start's seal accepts exactly the committed row.
        assert!(
            crate::state::governance_start::require_committed_enactments(
                &block.world,
                results.receipts()
            )
            .is_ok()
        );
        drop(block);
        assert!(
            state
                .world
                .smart_contract_state
                .view()
                .get(&effect_key())
                .is_none(),
            "the block was never committed"
        );
        assert!(
            state
                .world
                .parliament_anonymous_runtime_enactments
                .view()
                .get(&receipt.governance_attempt_id)
                .is_none(),
            "an abandoned block keeps no receipt"
        );
    }

    #[test]
    fn a_refused_receipt_discards_the_whole_prepared_effect() {
        use iroha_data_model::governance::anonymous_ballot::{
            RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1, RuntimeEnactmentResultsErrorV1 as Refusal,
        };
        let state = Rows::default().state();
        let mut block = state.block(header(AT.height, AT.timestamp_ms));
        let owner = SelectedResultOwner::for_test();
        let mut results = RuntimeEnactmentResultsV1::new(AT.height, AT.timestamp_ms);
        let effect =
            |block: &StateBlock<'_>| block.world.smart_contract_state.get(&effect_key()).cloned();

        // A receipt prepared for another pass position never enters World or custody.
        let elsewhere = GovernanceAnchorV1 { position: 2, ..AT };
        assert!(matches!(
            produce(
                &owner,
                prepared(&mut block, receipt_at(elsewhere, 4), 1),
                AT,
                &mut results
            ),
            Err(PassError::World(_))
        ));
        // Neither does one the custody of another block refuses: its World row is discarded.
        let mut foreign = RuntimeEnactmentResultsV1::new(AT.height + 1, AT.timestamp_ms);
        assert!(matches!(
            produce(
                &owner,
                prepared(&mut block, receipt_at(AT, 4), 2),
                AT,
                &mut foreign
            ),
            Err(PassError::Results(Refusal::OtherBlock))
        ));
        assert!(results.is_empty() && foreign.is_empty());
        assert_eq!(effect(&block), None);
        assert_eq!(committed(&block, 4), None);

        produce(
            &owner,
            prepared(&mut block, receipt_at(AT, 4), 3),
            AT,
            &mut results,
        )
        .unwrap();
        assert_eq!(effect(&block), Some(vec![3]));
        assert_eq!(committed(&block, 4), Some(receipt_at(AT, 4)));

        // An attempt that already holds a committed receipt is refused; its effect is discarded
        // and the committed row is unchanged.
        let next = GovernanceAnchorV1 { position: 4, ..AT };
        assert!(matches!(
            produce(
                &owner,
                prepared(&mut block, receipt_at(next, 4), 4),
                next,
                &mut results
            ),
            Err(PassError::World(_))
        ));
        assert_eq!(effect(&block), Some(vec![3]));
        assert_eq!(committed(&block, 4), Some(receipt_at(AT, 4)));
        assert_eq!(results.receipts().len(), 1);

        // A custody at its bound refuses before the effect (its owner checks `fits` first), and
        // the receipt row its transaction wrote is discarded with it.
        let mut position = next.position;
        let mut tag = 10_u8;
        while results.fits() {
            results
                .admit(receipt_at(GovernanceAnchorV1 { position, ..AT }, tag))
                .unwrap();
            position += 1;
            tag += 1;
        }
        assert_eq!(
            results.receipts().len(),
            usize::try_from(RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1).unwrap()
        );
        let last = GovernanceAnchorV1 { position, ..AT };
        assert!(matches!(
            produce(
                &owner,
                prepared(&mut block, receipt_at(last, 0xee), 5),
                last,
                &mut results
            ),
            Err(PassError::Results(Refusal::Capacity))
        ));
        assert_eq!(effect(&block), Some(vec![3]));
        assert_eq!(committed(&block, 0xee), None);
        assert!(!PassError::Results(Refusal::Capacity).is_local());
        assert!(PassError::Results(Refusal::Allocation).is_local());
    }

    #[test]
    fn a_full_result_custody_defers_live_g3_uncharged() {
        use iroha_data_model::governance::anonymous_ballot::RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1;
        let item = GovernancePassItemV1::EnactParliament {
            due_ms: 0,
            attempt: GovernanceAttemptId::new([4; 32]),
        };
        let mut results = RuntimeEnactmentResultsV1::new(5, 300_000);
        assert_eq!(
            inspect_result_capacity(item, GovernanceOccurrenceV1::Live, &results),
            GovernanceOccurrenceV1::Live
        );
        for position in 0..RUNTIME_ENACTMENT_RESULTS_PER_BLOCK_V1 {
            let tag = 100 + u8::try_from(position).unwrap();
            results
                .admit(receipt_at(GovernanceAnchorV1 { position, ..AT }, tag))
                .unwrap();
        }
        assert_eq!(
            inspect_result_capacity(item, GovernanceOccurrenceV1::Live, &results),
            GovernanceOccurrenceV1::NoOutputCapacity
        );
        assert_eq!(
            inspect_result_capacity(item, GovernanceOccurrenceV1::Lapsed, &results),
            GovernanceOccurrenceV1::Lapsed
        );
        assert_eq!(
            inspect_result_capacity(
                GovernancePassItemV1::HeightProgress,
                GovernanceOccurrenceV1::Live,
                &results
            ),
            GovernanceOccurrenceV1::Live,
            "only G3 enactments read the result custody"
        );
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let mut block = state.block(header(5, 300_000));
        let mut pass = cursor_before_g3(&mut block, false);
        let before = (pass.processed(), pass.remaining());
        pass.run_segment::<PassError>(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![item],
            |_, item| {
                Ok(inspect_result_capacity(
                    item,
                    GovernanceOccurrenceV1::Live,
                    &results,
                ))
            },
            |_, _, _| panic!("a full result custody stops G3 before any owner effect"),
        )
        .unwrap();
        assert_eq!((pass.processed(), pass.remaining()), before);
    }

    #[test]
    fn unselected_roots_keep_the_original_g3_refusal_before_effects() {
        // A root that installs the anonymous family but composes no result service.
        let state = Rows::default().state();
        let block = state.block(header(5, 300_000));
        assert!(
            crate::runtime::parliament_anonymous_installed(block.world.runtime_selection())
                .unwrap()
        );
        assert!(SelectedResultOwner::of(&block).unwrap().is_none());
        assert!(matches!(
            require_result_profile(&block),
            Err(PassError::Storage(
                StateStorageAdmissionError::UnsupportedRuntime { height: 5, .. }
            ))
        ));
        drop(block);
        // A root without the family selects nothing either.
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let block = state.block(header(5, 300_000));
        assert!(SelectedResultOwner::of(&block).unwrap().is_none());
    }
}
