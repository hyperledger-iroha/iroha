//! G3 runtime effects of production-qualified anonymous Parliament certificates.
//!
//! Candidate certificates are never listed and are refused again before reading an effect.
//! A deterministic rejection drops the effect transaction before a fresh transaction records
//! failure. A local refusal abandons the block with its original retry owner intact.

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

pub(super) fn apply(
    state: &mut StateBlock<'_>,
    id: GovernanceAttemptId,
    at: GovernanceAnchorV1,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
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
    let row = receipt(
        at,
        GovernanceLifecycleItemV1::EnactRuntimeUpgrade,
        &attempt,
        None,
    )?;
    store_attempt(&mut tx.world, id, attempt);
    tx.require_storage_admission().map_err(PassError::Storage)?;
    rows.admit_item(GovernanceStartItemCeilingV1::Receipt, vec![row])
        .map_err(PassError::Output)?;
    tx.apply();
    // Final boundary preparation may latch an original storage refusal. The block keeps
    // that owner even when the disposable transaction declines to publish its overlay.
    state
        .require_storage_admission()
        .map_err(PassError::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::parliament_anonymous::{AnonymousBallotDecisionV1, test_driver::Driver};
    use iroha_data_model::governance::types::{ParliamentBody, RiskTierV1};

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
        assert!(
            iroha_data_model::governance::anonymous_ballot::PARLIAMENT_BALLOT_QUALIFICATION_V1
                .is_none()
        );
    }
}
