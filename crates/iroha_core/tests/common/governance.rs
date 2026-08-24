//! Canonical governance fixtures shared by core integration tests.

use iroha_core::state::GovernanceParliamentSnapshot;
use iroha_data_model::{
    account::AccountId,
    governance::types::{ParliamentBodies, ParliamentBody, ParliamentRoster},
    isi::governance::CouncilDerivationKind,
};

/// Build a valid seven-body V1 proposal snapshot with one eligible citizen.
pub fn single_member_parliament_snapshot(
    member: &AccountId,
    selection_epoch: u64,
) -> GovernanceParliamentSnapshot {
    let rosters = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ]
    .into_iter()
    .map(|body| {
        (
            body,
            ParliamentRoster {
                body,
                epoch: selection_epoch,
                members: vec![member.clone()],
                alternates: Vec::new(),
                candidate_count: 1,
                derived_by: CouncilDerivationKind::Sortition,
            },
        )
    })
    .collect();
    GovernanceParliamentSnapshot::try_new(
        [0xA5; 32],
        ParliamentBodies {
            selection_epoch,
            rosters,
        },
    )
    .expect("canonical single-member Parliament snapshot")
}
