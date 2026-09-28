//! SCCP governance through the SORA Parliament (`specs/sccp.md` §4.14.3). Owner: ws33.
//!
//! `ProposeSccpRouteGovernance` → 8-body Parliament → due certificate → atomic enactment. The
//! expected head is scoped per SCCP subject: for a proposal `P` with sorted subjects `S(P)`,
//! `version = 1 + Σ rev(s)` and `head_root = parliament_governance_head_root_v1(...)`. Attempt
//! creation additionally checks `base_revisions` and every `RegisterRoute` destination word.
//! Enactment applies every action in order and increments `rev(s)` for each subject inside the
//! same effect transaction.
//!
//! Until ws33 lands, every entry point fails closed with a message that states that SCCP v1
//! governance enactment is not wired yet.

use super::Error;
use crate::state::StateTransaction;
use iroha_data_model::{
    governance::types::GovernanceExpectedHeadV1, sccp::governance::SccpGovernanceProposalV1,
};

/// Build the fail-closed error of a governance entry point that is not implemented yet.
fn governance_not_wired(what: &str) -> Error {
    Error::InvariantViolation(
        format!(
            "SCCP v1 governance enactment is not wired yet: {what} not implemented yet (TODO(ws33))"
        )
        .into(),
    )
}

/// Return the Parliament expected head of `proposal` against current state (§4.14.3).
///
/// # Errors
///
/// Fails closed until ws33 implements per-subject heads.
pub fn expected_head(
    state_transaction: &StateTransaction<'_, '_>,
    proposal: &SccpGovernanceProposalV1,
) -> Result<GovernanceExpectedHeadV1, Error> {
    let _ = (state_transaction, proposal);
    // TODO(ws33): per-subject head from `sccp_governance_revisions` (§4.14.3).
    Err(governance_not_wired("the per-subject expected head"))
}

/// Check the SCCP preconditions of a new Parliament attempt: every `rev(s)` still equals
/// `base_revisions` and every `RegisterRoute` destination word is unused (§4.14.3 step 2).
///
/// # Errors
///
/// Fails closed until ws33 implements the preflight.
pub fn preflight_attempt(
    state_transaction: &StateTransaction<'_, '_>,
    proposal: &SccpGovernanceProposalV1,
) -> Result<(), Error> {
    let _ = (state_transaction, proposal);
    // TODO(ws33): base-revision and destination-word checks (§4.14.3).
    Err(governance_not_wired("the attempt preflight"))
}

/// Apply every action of the certified `proposal` in order and increment the revision of each
/// of its subjects (§4.14.3 step 4). Runs inside the isolated effect transaction.
///
/// # Errors
///
/// Fails closed until ws33 implements enactment.
pub fn enact(
    state_transaction: &mut StateTransaction<'_, '_>,
    proposal: &SccpGovernanceProposalV1,
    proposal_id: [u8; 32],
) -> Result<(), Error> {
    let _ = (state_transaction, proposal, proposal_id);
    // TODO(ws33): the v1 action executors and `SccpGovernanceEnacted` (§4.14.3).
    Err(governance_not_wired("enactment"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header, sample_proposal};

    #[test]
    fn governance_fails_closed_until_implemented() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let proposal = sample_proposal(stx.network_id);
        for error in [
            expected_head(&stx, &proposal).expect_err("skeleton"),
            preflight_attempt(&stx, &proposal).expect_err("skeleton"),
            enact(&mut stx, &proposal, [7; 32]).expect_err("skeleton"),
        ] {
            let message = error.to_string();
            assert!(
                message.contains("SCCP v1 governance enactment is not wired yet"),
                "{message}"
            );
            assert!(message.contains("TODO(ws33)"), "{message}");
        }
    }
}
