//! Deterministic governance sortition utilities for on-chain bodies.

use std::collections::{BTreeMap, BTreeSet};

use iroha_config::parameters::actual::Governance;
use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::types::{ParliamentBodies, ParliamentBody, ParliamentRoster},
    isi::governance::CouncilDerivationKind,
};
use iroha_primitives::numeric::Quantity;

use crate::governance::sortition;

/// Sortition result with winners and alternates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    /// Selected members in deterministic rank order.
    pub members: Vec<AccountId>,
    /// Alternates to replace members that decline or are ineligible.
    pub alternates: Vec<AccountId>,
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
/// Domain separator for citizen sortition inputs.
pub const CITIZEN_INPUT_DOMAIN: &[u8] = b"iroha:vrf:v1:citizen|";

fn scored_output(seed: &[u8; 64], input_domain: &[u8], account_id: &AccountId) -> [u8; 32] {
    let input = sortition::build_input(input_domain, seed, account_id);
    let digest = Blake2b512::digest(input);
    let mut output = [0u8; 32];
    output.copy_from_slice(&digest[..32]);
    output
}

/// Deterministic draw over bonded citizens.
pub fn run_citizen_draw<'a, I>(
    network_id: &NetworkId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
    committee_size: usize,
    alternate_size: usize,
) -> Draw
where
    I: IntoIterator<Item = (&'a AccountId, u128)>,
{
    let seed = sortition::compute_seed(network_id, epoch, beacon, CITIZEN_SEED_DOMAIN);
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
    }
}

/// Deterministically derive parliament bodies directly from bonded citizen candidates.
///
/// Each body is sampled independently with body-specific domain tags. A citizen is unique within
/// one body's member/alternate roster, but may serve on multiple independently drawn bodies.
/// Bond amounts are used only for eligibility before this function is called, so every bonded
/// citizen has one draw per body. Proposal-time JIT sortition intentionally does not consume the
/// persisted per-epoch seat budget; that budget governs accepted, persisted service assignments.
pub fn derive_parliament_bodies_from_bonded_citizens<'a, I, B>(
    gov_cfg: &Governance,
    network_id: &NetworkId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
    derived_by: CouncilDerivationKind,
) -> ParliamentBodies
where
    I: IntoIterator<Item = (&'a AccountId, B)>,
    B: Into<Quantity>,
{
    let dedup: BTreeMap<AccountId, Quantity> =
        candidates
            .into_iter()
            .fold(BTreeMap::new(), |mut acc, (account_id, bond)| {
                let bond = bond.into();
                acc.entry(account_id.clone())
                    .and_modify(|existing| *existing = existing.clone().max(bond.clone()))
                    .or_insert(bond);
                acc
            });
    let candidate_count = u32::try_from(dedup.len()).unwrap_or(u32::MAX);
    let candidates: Vec<(AccountId, Quantity)> = dedup.into_iter().collect();

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
            network_id,
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
    network_id: &NetworkId,
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
            network_id,
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
    network_id: &NetworkId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: &[AccountId],
    committee_size: usize,
    alternate_size: usize,
    body: ParliamentBody,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let seed = sortition::compute_seed(network_id, epoch, beacon, body_seed_domain(body));
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
    network_id: &NetworkId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: &[(AccountId, Quantity)],
    committee_size: usize,
    alternate_size: usize,
    body: ParliamentBody,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let seed = sortition::compute_seed(network_id, epoch, beacon, body_seed_domain(body));
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

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId, account::AccountId, block::BlockHeader, governance::types::ParliamentBody,
    };

    use super::*;

    fn mk_account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive governance draw fixture account key");
        let (public_key, _) = keypair.into_parts();
        AccountId::new(public_key)
    }

    fn network_id(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            label,
        )))
    }

    #[test]
    fn mk_account_uses_checked_seed_derivation() {
        let account = mk_account(1);
        let expected = AccountId::new(
            KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519)
                .expect("derive expected governance draw fixture account key")
                .public_key()
                .clone(),
        );

        assert_eq!(account, expected);
    }

    #[test]
    fn citizen_draw_orders_without_rerolls() {
        let network_id = network_id(b"citizen-demo");
        let beacon = [5u8; 32];
        let epoch = 3u64;
        let accounts = [mk_account(1), mk_account(2), mk_account(3)];
        let bonds = [
            (&accounts[0], 150u128),
            (&accounts[1], 250u128),
            (&accounts[2], 350u128),
        ];
        let draw = run_citizen_draw(&network_id, epoch, &beacon, bonds, 2, 1);
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
        let network_id = network_id(b"citizen-demo");
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

        let floor_draw = run_citizen_draw(&network_id, epoch, &beacon, floor_bonds, 2, 2);
        let high_draw = run_citizen_draw(&network_id, epoch, &beacon, high_bonds, 2, 2);

        assert_eq!(floor_draw.members, high_draw.members);
        assert_eq!(floor_draw.alternates, high_draw.alternates);
    }

    #[test]
    fn citizen_draw_deduplicates_duplicate_accounts_without_extra_chances() {
        let network_id = network_id(b"citizen-demo");
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

        let baseline_draw = run_citizen_draw(&network_id, epoch, &beacon, baseline, 2, 2);
        let duplicated_draw = run_citizen_draw(&network_id, epoch, &beacon, duplicated_whale, 2, 2);

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
        let network_id = network_id(b"body-demo");
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
            &network_id,
            epoch,
            &beacon,
            baseline,
            CouncilDerivationKind::Manual,
        );
        let inflated_bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &network_id,
            epoch,
            &beacon,
            inflated,
            CouncilDerivationKind::Manual,
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
    fn one_bonded_citizen_fills_each_actual_body_and_sets_one_person_quorum() {
        let network_id = network_id(b"body-one-citizen-demo");
        let beacon = [0x1C; 32];
        let epoch = 17_u64;
        let citizen = mk_account(1);
        let cfg = Governance {
            parliament_quorum_bps: 6_667,
            rules_committee_size: 7,
            agenda_council_size: 9,
            interest_panel_size: 11,
            review_panel_size: 13,
            policy_jury_size: 25,
            oversight_committee_size: 7,
            fma_committee_size: 5,
            parliament_alternate_size: Some(25),
            ..Governance::default()
        };

        let bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &network_id,
            epoch,
            &beacon,
            [(&citizen, 10_000_u128)],
            CouncilDerivationKind::Sortition,
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
            let roster = bodies.rosters.get(&body).expect("one-citizen roster");
            assert_eq!(roster.members, [citizen.clone()]);
            assert!(roster.alternates.is_empty());
            assert_eq!(roster.candidate_count, 1);
            assert_eq!(
                crate::state::council_quorum_threshold(
                    roster.members.len(),
                    cfg.parliament_quorum_bps,
                ),
                1
            );
        }
    }

    #[test]
    fn body_rosters_are_independently_domain_separated() {
        let network_id = network_id(b"body-domain-demo");
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
            &network_id,
            epoch,
            &beacon,
            accounts.iter().map(|account| (account, 100u128)),
            CouncilDerivationKind::Manual,
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
    fn forty_six_citizens_fill_all_default_body_members_independently() {
        let network_id = network_id(b"body-readiness-demo");
        let beacon = [0x46; 32];
        let epoch = 46u64;
        let accounts: Vec<_> = (1..=46).map(mk_account).collect();
        let cfg = Governance::default();

        let bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &network_id,
            epoch,
            &beacon,
            accounts.iter().map(|account| (account, 100u128)),
            CouncilDerivationKind::Manual,
        );

        for (body, expected) in [
            (ParliamentBody::RulesCommittee, 7),
            (ParliamentBody::AgendaCouncil, 9),
            (ParliamentBody::InterestPanel, 11),
            (ParliamentBody::ReviewPanel, 13),
            (ParliamentBody::PolicyJury, 25),
            (ParliamentBody::OversightCommittee, 7),
            (ParliamentBody::FmaCommittee, 5),
        ] {
            let roster = bodies.rosters.get(&body).expect("default body roster");
            assert_eq!(roster.members.len(), expected, "{body:?} members");
            let within_body: BTreeSet<_> =
                roster.members.iter().chain(&roster.alternates).collect();
            assert_eq!(
                within_body.len(),
                roster.members.len() + roster.alternates.len(),
                "{body:?} must not repeat a citizen within its own roster"
            );
        }

        let total_member_seats: usize = bodies
            .rosters
            .values()
            .map(|roster| roster.members.len())
            .sum();
        let distinct_members: BTreeSet<_> = bodies
            .rosters
            .values()
            .flat_map(|roster| roster.members.iter())
            .collect();
        assert_eq!(total_member_seats, 77);
        assert!(
            distinct_members.len() <= accounts.len(),
            "independent body draws may reuse a citizen across bodies"
        );
    }

    #[test]
    fn bonded_body_draws_are_stable_under_candidate_order_permutation() {
        let network_id = network_id(b"body-order-demo");
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
            &network_id,
            epoch,
            &beacon,
            forward,
            CouncilDerivationKind::Manual,
        );
        let reverse_bodies = derive_parliament_bodies_from_bonded_citizens(
            &cfg,
            &network_id,
            epoch,
            &beacon,
            reverse,
            CouncilDerivationKind::Manual,
        );
        assert_eq!(
            forward_bodies, reverse_bodies,
            "candidate ordering supplied by an API must not bias parliament body draws"
        );
    }

    #[test]
    fn persisted_body_draws_deduplicate_member_and_alternate_overlap() {
        let network_id = network_id(b"body-demo");
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
            candidate_count: 8,
            derived_by: CouncilDerivationKind::Manual,
        };

        let bodies = derive_parliament_bodies(&cfg, &network_id, epoch, &beacon, &council);
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
