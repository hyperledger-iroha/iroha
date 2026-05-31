//! Governance VRF draw utilities (members + alternates) for on-chain bodies.

use std::collections::{BTreeMap, BTreeSet};

use iroha_config::parameters::actual::Governance;
use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    governance::types::{ParliamentBodies, ParliamentBody, ParliamentRoster},
    isi::governance::CouncilDerivationKind,
};

use crate::governance::{
    parliament::{CandidateRef, CandidateVariant, build_input, compute_seed, derive_committee},
    sortition,
};

/// VRF draw result with winners and alternates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    /// Selected members in ranked order (front = highest VRF output).
    pub members: Vec<AccountId>,
    /// Alternates to replace members that decline or are ineligible.
    pub alternates: Vec<AccountId>,
    /// Count of candidates with verified VRF proofs.
    pub verified: usize,
}

/// Run a VRF draw for `committee_size + alternate_size` and split the top `committee_size` as members.
pub fn run_draw<'a, I>(
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
    committee_size: usize,
    alternate_size: usize,
) -> Draw
where
    I: IntoIterator<Item = CandidateRef<'a>>,
{
    let seed = compute_seed(chain_id, epoch, beacon);
    let total = committee_size.saturating_add(alternate_size);
    let derivation = derive_committee(chain_id, &seed, candidates, total);
    let mut members = Vec::new();
    let mut alternates = Vec::new();
    for (idx, member) in derivation.members.into_iter().enumerate() {
        if idx < committee_size {
            members.push(member.account_id);
        } else {
            alternates.push(member.account_id);
        }
    }
    Draw {
        members,
        alternates,
        verified: derivation.verified,
    }
}

/// Helper to build a `CandidateRef` from raw parts.
pub fn candidate_ref<'a>(
    account_id: &'a AccountId,
    variant: CandidateVariant,
    public_key: &'a [u8],
    proof: &'a [u8],
) -> CandidateRef<'a> {
    let _ = build_input(&[0u8; 64], account_id); // ensure codec is linked; caller supplies real seed in derive_committee
    CandidateRef {
        account_id,
        variant,
        public_key,
        proof,
    }
}

/// Replace a missing member with the next alternate. Returns `true` if replaced.
pub fn replace_with_alternate(
    members: &mut [AccountId],
    alternates: &mut Vec<AccountId>,
    missing: &AccountId,
) -> bool {
    if let Some(pos) = members.iter().position(|m| m == missing) {
        if let Some(next) = alternates.first().cloned() {
            members[pos] = next;
            alternates.remove(0);
            return true;
        }
    }
    false
}

/// Domain separator for citizen draws.
pub const CITIZEN_SEED_DOMAIN: &[u8] = b"gov:citizen:seed:v1";
/// Domain separator for citizen VRF inputs.
pub const CITIZEN_INPUT_DOMAIN: &[u8] = b"iroha:vrf:v1:citizen|";

fn scored_output(seed: &[u8; 64], input_domain: &[u8], account_id: &AccountId) -> [u8; 32] {
    let input = sortition::build_input(input_domain, seed, account_id);
    let digest = Blake2b512::digest(input);
    let mut output = [0u8; 32];
    output.copy_from_slice(&digest[..32]);
    output
}

/// Deterministic draw over bonded citizens (no proofs; hash-ordered by VRF input).
pub fn run_citizen_draw<'a, I>(
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
    committee_size: usize,
    alternate_size: usize,
) -> Draw
where
    I: IntoIterator<Item = (&'a AccountId, u128)>,
{
    let seed = sortition::compute_seed(chain_id, epoch, beacon, CITIZEN_SEED_DOMAIN);
    let dedup: BTreeMap<AccountId, u128> =
        candidates
            .into_iter()
            .fold(BTreeMap::new(), |mut acc, (account_id, bond)| {
                acc.entry(account_id.clone())
                    .and_modify(|existing| *existing = (*existing).max(bond))
                    .or_insert(bond);
                acc
            });
    let mut scored: Vec<([u8; 32], AccountId)> = Vec::new();
    for account_id in dedup.keys() {
        let output = scored_output(&seed, CITIZEN_INPUT_DOMAIN, account_id);
        scored.push((output, account_id.clone()));
    }
    scored.sort_by(|a, b| {
        use core::cmp::Ordering;
        match b.0.cmp(&a.0) {
            Ordering::Equal => a.1.cmp(&b.1),
            other => other,
        }
    });
    scored.dedup_by(|a, b| a.1 == b.1);

    let total = committee_size.saturating_add(alternate_size);
    let mut members = Vec::new();
    let mut alternates = Vec::new();
    for (idx, (_, account_id)) in scored.into_iter().take(total).enumerate() {
        if idx < committee_size {
            members.push(account_id);
        } else {
            alternates.push(account_id);
        }
    }
    Draw {
        members,
        alternates,
        verified: 0,
    }
}

/// Deterministically derive parliament bodies directly from bonded citizen candidates.
///
/// Each body is sampled independently with body-specific domain tags. Bond amounts are used only
/// for eligibility before this function is called, so every bonded citizen has one draw.
pub fn derive_parliament_bodies_from_bonded_citizens<'a, I>(
    gov_cfg: &Governance,
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
    derived_by: CouncilDerivationKind,
) -> ParliamentBodies
where
    I: IntoIterator<Item = (&'a AccountId, u128)>,
{
    let dedup: BTreeMap<AccountId, u128> =
        candidates
            .into_iter()
            .fold(BTreeMap::new(), |mut acc, (account_id, bond)| {
                acc.entry(account_id.clone())
                    .and_modify(|existing| *existing = (*existing).max(bond))
                    .or_insert(bond);
                acc
            });
    let candidate_count = u32::try_from(dedup.len()).unwrap_or(u32::MAX);
    let candidates: Vec<(AccountId, u128)> = dedup.into_iter().collect();

    let alternates_per_body = gov_cfg
        .parliament_alternate_size
        .unwrap_or(gov_cfg.parliament_committee_size);
    let mut rosters = std::collections::BTreeMap::new();
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ] {
        let committee_size = body_committee_size(gov_cfg, body);
        let (members, alternates) = body_selection_from_bonded(
            chain_id,
            epoch,
            beacon,
            &candidates,
            committee_size,
            alternates_per_body,
            body,
        );
        rosters.insert(
            body,
            ParliamentRoster {
                body,
                epoch,
                members,
                alternates,
                verified: 0,
                candidate_count,
                derived_by,
            },
        );
    }
    ParliamentBodies {
        selection_epoch: epoch,
        rosters,
    }
}

/// Deterministically derive parliament rosters for all bodies from the persisted council draw.
///
/// Uses per-body domain separators to shuffle the combined member+alternate list into distinct
/// committees so each stage has an independent roster while remaining reproducible across peers.
pub fn derive_parliament_bodies(
    gov_cfg: &Governance,
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    council: &super::state::ParliamentTerm,
) -> ParliamentBodies {
    let mut candidates: Vec<AccountId> = Vec::new();
    candidates.extend(council.members.iter().cloned());
    candidates.extend(council.alternates.iter().cloned());
    let mut seen = BTreeSet::new();
    candidates.retain(|id| seen.insert(id.clone()));

    let alternates_per_body = gov_cfg
        .parliament_alternate_size
        .unwrap_or(gov_cfg.parliament_committee_size);

    let mut rosters = std::collections::BTreeMap::new();
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ] {
        let committee_size = body_committee_size(gov_cfg, body);
        let (members, alternates) = body_selection(
            chain_id,
            epoch,
            beacon,
            &candidates,
            committee_size,
            alternates_per_body,
            body,
        );
        let roster = ParliamentRoster {
            body,
            epoch,
            members,
            alternates,
            verified: council.verified,
            candidate_count: council.candidate_count,
            derived_by: council.derived_by,
        };
        rosters.insert(body, roster);
    }

    ParliamentBodies {
        selection_epoch: epoch,
        rosters,
    }
}

fn body_committee_size(cfg: &Governance, body: ParliamentBody) -> usize {
    match body {
        ParliamentBody::RulesCommittee => cfg.rules_committee_size,
        ParliamentBody::AgendaCouncil => cfg.agenda_council_size,
        ParliamentBody::InterestPanel => cfg.interest_panel_size,
        ParliamentBody::ReviewPanel => cfg.review_panel_size,
        ParliamentBody::PolicyJury => cfg.policy_jury_size,
        ParliamentBody::OversightCommittee => cfg.oversight_committee_size,
        ParliamentBody::FmaCommittee => cfg.fma_committee_size,
    }
}

fn body_seed_domain(body: ParliamentBody) -> &'static [u8] {
    match body {
        ParliamentBody::RulesCommittee => b"gov:parliament:body:rules:v1",
        ParliamentBody::AgendaCouncil => b"gov:parliament:body:agenda:v1",
        ParliamentBody::InterestPanel => b"gov:parliament:body:interest:v1",
        ParliamentBody::ReviewPanel => b"gov:parliament:body:review:v1",
        ParliamentBody::PolicyJury => b"gov:parliament:body:policy_jury:v1",
        ParliamentBody::OversightCommittee => b"gov:parliament:body:oversight:v1",
        ParliamentBody::FmaCommittee => b"gov:parliament:body:fma:v1",
    }
}

fn body_input_domain(body: ParliamentBody) -> &'static [u8] {
    match body {
        ParliamentBody::RulesCommittee => b"iroha:vrf:v1:parliament:rules|",
        ParliamentBody::AgendaCouncil => b"iroha:vrf:v1:parliament:agenda|",
        ParliamentBody::InterestPanel => b"iroha:vrf:v1:parliament:interest|",
        ParliamentBody::ReviewPanel => b"iroha:vrf:v1:parliament:review|",
        ParliamentBody::PolicyJury => b"iroha:vrf:v1:parliament:policy_jury|",
        ParliamentBody::OversightCommittee => b"iroha:vrf:v1:parliament:oversight|",
        ParliamentBody::FmaCommittee => b"iroha:vrf:v1:parliament:fma|",
    }
}

fn body_selection(
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: &[AccountId],
    committee_size: usize,
    alternate_size: usize,
    body: ParliamentBody,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let seed = sortition::compute_seed(chain_id, epoch, beacon, body_seed_domain(body));
    let mut scored: Vec<([u8; 32], AccountId)> = Vec::new();
    for account_id in candidates {
        let input = sortition::build_input(body_input_domain(body), &seed, account_id);
        let digest = Blake2b512::digest(input);
        let mut output = [0u8; 32];
        output.copy_from_slice(&digest[..32]);
        scored.push((output, account_id.clone()));
    }
    scored.sort_by(|a, b| {
        use core::cmp::Ordering;
        match b.0.cmp(&a.0) {
            Ordering::Equal => a.1.cmp(&b.1),
            other => other,
        }
    });
    scored.dedup_by(|a, b| a.1 == b.1);

    let total_alternates = alternate_size.min(scored.len().saturating_sub(committee_size));
    let mut members = Vec::with_capacity(committee_size.min(scored.len()));
    let mut alternates = Vec::with_capacity(total_alternates);
    for (idx, (_, account_id)) in scored.into_iter().enumerate() {
        if idx < committee_size {
            members.push(account_id);
        } else if alternates.len() < total_alternates {
            alternates.push(account_id);
        } else {
            break;
        }
    }
    (members, alternates)
}

fn body_selection_from_bonded(
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: &[(AccountId, u128)],
    committee_size: usize,
    alternate_size: usize,
    body: ParliamentBody,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let seed = sortition::compute_seed(chain_id, epoch, beacon, body_seed_domain(body));
    let mut scored: Vec<([u8; 32], AccountId)> = Vec::new();
    for (account_id, _bond) in candidates {
        let output = scored_output(&seed, body_input_domain(body), account_id);
        scored.push((output, account_id.clone()));
    }
    scored.sort_by(|a, b| {
        use core::cmp::Ordering;
        match b.0.cmp(&a.0) {
            Ordering::Equal => a.1.cmp(&b.1),
            other => other,
        }
    });
    scored.dedup_by(|a, b| a.1 == b.1);

    let total_alternates = alternate_size.min(scored.len().saturating_sub(committee_size));
    let mut members = Vec::with_capacity(committee_size.min(scored.len()));
    let mut alternates = Vec::with_capacity(total_alternates);
    for (idx, (_, account_id)) in scored.into_iter().enumerate() {
        if idx < committee_size {
            members.push(account_id);
        } else if alternates.len() < total_alternates {
            alternates.push(account_id);
        } else {
            break;
        }
    }
    (members, alternates)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, governance::types::ParliamentBody};

    use super::*;

    fn mk_account(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        AccountId::new(public_key)
    }

    #[test]
    fn citizen_draw_orders_without_rerolls() {
        let chain_id: ChainId = "citizen-demo".into();
        let beacon = [5u8; 32];
        let epoch = 3u64;
        let accounts = [mk_account(1), mk_account(2), mk_account(3)];
        let bonds = [
            (&accounts[0], 150u128),
            (&accounts[1], 250u128),
            (&accounts[2], 350u128),
        ];
        let draw = run_citizen_draw(&chain_id, epoch, &beacon, bonds, 2, 1);
        assert_eq!(draw.members.len(), 2);
        assert_eq!(draw.alternates.len(), 1);
        let mut combined = Vec::new();
        combined.extend(draw.members.iter().cloned());
        combined.extend(draw.alternates.iter().cloned());
        let unique: BTreeSet<_> = combined.iter().collect();
        assert_eq!(unique.len(), 3, "draw must not re-roll candidates");
    }

    #[test]
    fn citizen_draw_ignores_bond_amounts() {
        let chain_id: ChainId = "citizen-demo".into();
        let beacon = [7u8; 32];
        let epoch = 9u64;
        let accounts = [mk_account(1), mk_account(2), mk_account(3), mk_account(4)];
        let floor_bonds = [
            (&accounts[0], 150u128),
            (&accounts[1], 150u128),
            (&accounts[2], 150u128),
            (&accounts[3], 150u128),
        ];
        let high_bonds = [
            (&accounts[0], 150u128),
            (&accounts[1], 15_000u128),
            (&accounts[2], 150_000u128),
            (&accounts[3], 1_500_000u128),
        ];

        let floor_draw = run_citizen_draw(&chain_id, epoch, &beacon, floor_bonds, 2, 2);
        let high_draw = run_citizen_draw(&chain_id, epoch, &beacon, high_bonds, 2, 2);

        assert_eq!(floor_draw.members, high_draw.members);
        assert_eq!(floor_draw.alternates, high_draw.alternates);
    }

    #[test]
    fn citizen_draw_deduplicates_duplicate_accounts_without_extra_chances() {
        let chain_id: ChainId = "citizen-demo".into();
        let beacon = [9u8; 32];
        let epoch = 12u64;
        let accounts = [mk_account(1), mk_account(2), mk_account(3), mk_account(4)];
        let baseline = [
            (&accounts[0], 100u128),
            (&accounts[1], 100u128),
            (&accounts[2], 100u128),
            (&accounts[3], 100u128),
        ];
        let duplicated_whale = [
            (&accounts[0], 100u128),
            (&accounts[1], 1_000_000u128),
            (&accounts[1], 2_000_000u128),
            (&accounts[1], 3_000_000u128),
            (&accounts[2], 100u128),
            (&accounts[3], 100u128),
        ];

        let baseline_draw = run_citizen_draw(&chain_id, epoch, &beacon, baseline, 2, 2);
        let duplicated_draw = run_citizen_draw(&chain_id, epoch, &beacon, duplicated_whale, 2, 2);

        assert_eq!(baseline_draw.members, duplicated_draw.members);
        assert_eq!(baseline_draw.alternates, duplicated_draw.alternates);
        let combined: Vec<_> = duplicated_draw
            .members
            .iter()
            .chain(duplicated_draw.alternates.iter())
            .collect();
        let unique: BTreeSet<_> = combined.iter().copied().collect();
        assert_eq!(
            combined.len(),
            unique.len(),
            "duplicate citizen entries must not create duplicate seats"
        );
    }

    #[test]
    fn bonded_body_draws_deduplicate_candidates_and_ignore_whale_bonds() {
        let chain_id: ChainId = "body-demo".into();
        let beacon = [0xA5; 32];
        let epoch = 14u64;
        let accounts = [
            mk_account(1),
            mk_account(2),
            mk_account(3),
            mk_account(4),
            mk_account(5),
        ];
        let cfg = Governance {
            rules_committee_size: 2,
            agenda_council_size: 2,
            interest_panel_size: 2,
            review_panel_size: 2,
            policy_jury_size: 2,
            oversight_committee_size: 2,
            fma_committee_size: 2,
            parliament_alternate_size: Some(2),
            ..Governance::default()
        };
        let baseline = accounts.iter().map(|account| (account, 100u128));
        let inflated = [
            (&accounts[0], 100u128),
            (&accounts[1], 10_000_000u128),
            (&accounts[1], 20_000_000u128),
            (&accounts[2], 100u128),
            (&accounts[3], 100u128),
            (&accounts[4], 100u128),
            (&accounts[4], 40_000_000u128),
        ];

        let baseline_bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &chain_id,
            epoch,
            &beacon,
            baseline,
            CouncilDerivationKind::Fallback,
        );
        let inflated_bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &chain_id,
            epoch,
            &beacon,
            inflated,
            CouncilDerivationKind::Fallback,
        );

        for body in [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
            ParliamentBody::FmaCommittee,
        ] {
            let baseline = baseline_bodies.rosters.get(&body).expect("baseline roster");
            let inflated = inflated_bodies.rosters.get(&body).expect("inflated roster");
            assert_eq!(baseline.members, inflated.members, "{body:?} members");
            assert_eq!(
                baseline.alternates, inflated.alternates,
                "{body:?} alternates"
            );
            assert_eq!(inflated.candidate_count, 5);
            let combined: Vec<_> = inflated
                .members
                .iter()
                .chain(inflated.alternates.iter())
                .collect();
            let unique: BTreeSet<_> = combined.iter().copied().collect();
            assert_eq!(
                combined.len(),
                unique.len(),
                "{body:?} duplicate bonded candidates must not create duplicate seats"
            );
        }
    }

    #[test]
    fn body_rosters_are_independently_domain_separated() {
        let chain_id: ChainId = "body-domain-demo".into();
        let beacon = [0xC3; 32];
        let epoch = 16u64;
        let accounts = [
            mk_account(1),
            mk_account(2),
            mk_account(3),
            mk_account(4),
            mk_account(5),
            mk_account(6),
            mk_account(7),
            mk_account(8),
            mk_account(9),
            mk_account(10),
        ];
        let cfg = Governance {
            rules_committee_size: 3,
            agenda_council_size: 3,
            interest_panel_size: 3,
            review_panel_size: 3,
            policy_jury_size: 3,
            oversight_committee_size: 3,
            fma_committee_size: 3,
            parliament_alternate_size: Some(2),
            ..Governance::default()
        };
        let bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &chain_id,
            epoch,
            &beacon,
            accounts.iter().map(|account| (account, 100u128)),
            CouncilDerivationKind::Fallback,
        );

        let distinct_member_lists: BTreeSet<_> = [
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
            bodies
                .rosters
                .get(&body)
                .expect("body roster")
                .members
                .clone()
        })
        .collect();
        assert!(
            distinct_member_lists.len() > 1,
            "body draws must not clone one shared membership list across all parliament bodies"
        );
    }

    #[test]
    fn bonded_body_draws_are_stable_under_candidate_order_permutation() {
        let chain_id: ChainId = "body-order-demo".into();
        let beacon = [0xB7; 32];
        let epoch = 17u64;
        let accounts = [
            mk_account(1),
            mk_account(2),
            mk_account(3),
            mk_account(4),
            mk_account(5),
            mk_account(6),
        ];
        let cfg = Governance {
            rules_committee_size: 2,
            agenda_council_size: 2,
            interest_panel_size: 2,
            review_panel_size: 2,
            policy_jury_size: 2,
            oversight_committee_size: 2,
            fma_committee_size: 2,
            parliament_alternate_size: Some(2),
            ..Governance::default()
        };
        let forward = accounts.iter().map(|account| (account, 100u128));
        let reverse = accounts.iter().rev().map(|account| (account, 100u128));

        let forward_bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &chain_id,
            epoch,
            &beacon,
            forward,
            CouncilDerivationKind::Fallback,
        );
        let reverse_bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &chain_id,
            epoch,
            &beacon,
            reverse,
            CouncilDerivationKind::Fallback,
        );
        assert_eq!(
            forward_bodies, reverse_bodies,
            "candidate ordering supplied by an API must not bias parliament body draws"
        );
    }

    #[test]
    fn persisted_body_draws_deduplicate_member_and_alternate_overlap() {
        let chain_id: ChainId = "body-demo".into();
        let beacon = [0x5A; 32];
        let epoch = 15u64;
        let accounts = [
            mk_account(1),
            mk_account(2),
            mk_account(3),
            mk_account(4),
            mk_account(5),
        ];
        let cfg = Governance {
            rules_committee_size: 3,
            agenda_council_size: 3,
            interest_panel_size: 3,
            review_panel_size: 3,
            policy_jury_size: 3,
            oversight_committee_size: 3,
            fma_committee_size: 3,
            parliament_alternate_size: Some(2),
            ..Governance::default()
        };
        let council = super::super::state::ParliamentTerm {
            epoch,
            members: vec![
                accounts[0].clone(),
                accounts[1].clone(),
                accounts[1].clone(),
                accounts[2].clone(),
            ],
            alternates: vec![
                accounts[2].clone(),
                accounts[3].clone(),
                accounts[4].clone(),
                accounts[4].clone(),
            ],
            verified: 5,
            candidate_count: 8,
            derived_by: CouncilDerivationKind::Fallback,
        };

        let bodies = derive_parliament_bodies(&cfg, &chain_id, epoch, &beacon, &council);
        for (body, roster) in bodies.rosters {
            let combined: Vec<_> = roster
                .members
                .iter()
                .chain(roster.alternates.iter())
                .collect();
            let unique: BTreeSet<_> = combined.iter().copied().collect();
            assert_eq!(
                combined.len(),
                unique.len(),
                "{body:?} member/alternate overlap must be deduplicated before body selection"
            );
        }
    }
}
