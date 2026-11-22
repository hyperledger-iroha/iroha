//! G3 runtime preparation; publication waits for a distinct selected result profile.
//!
//! Candidate certificates are never listed and are refused again before reading an effect.
//! The native output grammar has no G3 receipt, so actual apply refuses before effects.
//! The retained private preparation drops a deterministically rejected effect transaction
//! before a fresh failure child; local refusal keeps its original retry owner intact.

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

/// No current selected result profile can commit the standalone G3 receipt.
/// This is local unsupported-runtime recovery, never a rejection or an applied item.
/// TODO(N14): replace this refusal only with the original selected, bounded result owner.
fn require_result_profile(state: &StateBlock<'_>) -> Result<core::convert::Infallible, PassError> {
    Err(PassError::Storage(
        StateStorageAdmissionError::UnsupportedRuntime {
            height: state._curr_block.height().get(),
            reason: "runtime enactment requires a distinct selected governed result profile".into(),
        },
    ))
}

pub(super) fn apply(
    state: &mut StateBlock<'_>,
    id: GovernanceAttemptId,
    at: GovernanceAnchorV1,
    _rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    // Retain the original purpose/time/source error ordering. Reading this disposable
    // child grants no business effects, status mutation or output publication.
    let tx = state.try_transaction().map_err(PassError::Storage)?;
    let attempt = stored_attempt(&tx, id)?;
    attempt.certificate_for_execution(id, at)?;
    drop(tx);
    match require_result_profile(state) {
        Err(error) => Err(error),
        Ok(unavailable) => match unavailable {},
    }
}

/// Disposable original effects plus their distinct, still-unpublished receipt.
/// No caller-supplied receipt, raw World or boolean can apply this owner.
#[expect(
    dead_code,
    reason = "N14 selected governed result producer is not implemented; native outputs must stay closed"
)]
struct PreparedRuntimeEnactment<'block, 'state> {
    transaction: StateTransaction<'block, 'state>,
    receipt: iroha_data_model::governance::anonymous_ballot::RuntimeEnactmentReceiptV1,
}

/// Prepare the original effect and receipt within their original child borrow.
/// TODO(N14): only the genuinely selected result producer may consume this owner;
/// it must admit the complete receipt before applying the original transaction.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "retained effect preparation awaits a real selected output custody owner"
    )
)]
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
    let receipt = iroha_data_model::governance::anonymous_ballot::RuntimeEnactmentReceiptV1 {
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
    // Deliberately no tx.apply(): the distinct result owner does not exist yet.
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
        let mut rows = GovernanceStartOutputBudgetV1::new();
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
                apply(&mut block, driver.id(), at, &mut rows),
                Err(PassError::Reducer(_))
            ),
            "candidate purpose must refuse before the missing output profile"
        );
        assert_eq!(
            block.world.parliament_anonymous_attempts.get(&driver.id()),
            Some(&original)
        );
        assert!(block.world.runtime_change_sets.iter().next().is_none());
        assert!(rows.rows().is_empty());
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
        let failure = pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![item],
            |_, item| {
                Ok(inspect_native_capacity(
                    item,
                    GovernanceOccurrenceV1::Live,
                    &full.borrow(),
                ))
            },
            |state, _, at| apply(state, driver.id(), at, &mut full.borrow_mut()),
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
}
