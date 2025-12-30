//! Governance body selector helpers (parliament seats + alternates) using VRF draws.

use std::collections::BTreeMap;

use iroha_config::parameters::actual::Governance;
use iroha_data_model::{
    ChainId,
    governance::types::{ParliamentBodies, ParliamentBody, ParliamentRoster},
};

use crate::governance::{
    draw::{self, Draw},
    parliament::CandidateRef,
    state::ParliamentTerm,
};

/// Select parliament members/alternates using VRF draw and governance config.
pub fn select_parliament<'a, I>(
    gov_cfg: &Governance,
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
) -> Draw
where
    I: IntoIterator<Item = CandidateRef<'a>>,
{
    let committee = gov_cfg.parliament_committee_size;
    let alternates = gov_cfg
        .parliament_alternate_size
        .unwrap_or(committee)
        .max(committee);
    draw::run_draw(chain_id, epoch, beacon, candidates, committee, alternates)
}

/// Derive per-body rosters from a council selection (shared membership).
pub fn derive_parliament_bodies(council: &ParliamentTerm) -> ParliamentBodies {
    let mut rosters: BTreeMap<ParliamentBody, ParliamentRoster> = BTreeMap::new();
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ] {
        let roster = ParliamentRoster {
            body,
            epoch: council.epoch,
            members: council.members.clone(),
            alternates: council.alternates.clone(),
            verified: council.verified,
            candidate_count: council.candidate_count,
            derived_by: council.derived_by,
        };
        rosters.insert(body, roster);
    }
    ParliamentBodies {
        selection_epoch: council.epoch,
        rosters,
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{account::AccountId, domain::DomainId};

    use super::*;

    fn mk_account(seed: u8) -> AccountId {
        use iroha_crypto::{Algorithm, KeyPair};
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        let domain: DomainId = "wonderland".parse().expect("domain id");
        AccountId::new(domain, public_key)
    }

    #[test]
    fn derive_bodies_clones_council_members() {
        let council = ParliamentTerm {
            epoch: 5,
            members: vec![mk_account(1), mk_account(2)],
            alternates: vec![mk_account(3)],
            verified: 2,
            candidate_count: 3,
            ..ParliamentTerm::default()
        };
        let bodies = derive_parliament_bodies(&council);
        assert_eq!(bodies.selection_epoch, 5);
        assert_eq!(bodies.rosters.len(), 7);
        for (body, roster) in &bodies.rosters {
            assert_eq!(roster.body, *body);
            assert_eq!(roster.epoch, 5);
            assert_eq!(roster.members, council.members);
            assert_eq!(roster.alternates, council.alternates);
            assert_eq!(roster.verified, council.verified);
            assert_eq!(roster.candidate_count, council.candidate_count);
        }
    }
}
