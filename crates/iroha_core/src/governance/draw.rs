//! Deterministic future-beacon sortition utilities for on-chain bodies.
use crate::governance::sortition;
use iroha_config::parameters::actual::Governance;
use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::types::{ParliamentBodies, ParliamentBody, ParliamentRoster},
    isi::governance::CouncilDerivationKind,
};
use iroha_primitives::numeric::Quantity;
use std::collections::{BTreeMap, BTreeSet};
/// Sortition result with winners and alternates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    /// Selected members in deterministic rank order.
    pub members: Vec<AccountId>,
    /// Alternates to replace members that decline or are ineligible.
    pub alternates: Vec<AccountId>,
}
/// Bodies selected before a narrow Policy Jury result can trigger a fresh Confirmation Jury.
pub const PRIMARY_PARLIAMENT_BODIES_V1: [ParliamentBody; 9] = [
    ParliamentBody::RulesCommittee,
    ParliamentBody::AgendaCouncil,
    ParliamentBody::InterestPanel,
    ParliamentBody::ReviewPanel,
    ParliamentBody::CoordinationCouncil,
    ParliamentBody::MpcCommittee,
    ParliamentBody::FmaCommittee,
    ParliamentBody::OversightCommittee,
    ParliamentBody::PolicyJury,
];
/// Deterministic simultaneous body assignment and its binding concentration cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParliamentDrawPlan {
    /// Rosters derived from the committed candidate snapshot and future pulse.
    pub bodies: ParliamentBodies,
    /// Smallest feasible maximum number of primary bodies assigned to one citizen.
    pub assignment_cap: u32,
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
/// Domain separator for citizen sortition inputs derived from a finalized beacon pulse.
pub const CITIZEN_INPUT_DOMAIN: &[u8] = b"iroha:beacon-sortition:v1:citizen|";
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
/// Each body is sampled with body-specific domain tags. A citizen is unique within one body's
/// member/alternate roster, but may serve on multiple bodies when the electorate is too small for
/// complete separation.
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
    let candidates: Vec<AccountId> = dedup.into_keys().collect();
    derive_body_plan(
        gov_cfg,
        network_id,
        epoch,
        beacon,
        &candidates,
        candidate_count,
        derived_by,
        &PRIMARY_PARLIAMENT_BODIES_V1,
    )
    .bodies
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
    derive_body_plan(
        gov_cfg,
        network_id,
        epoch,
        beacon,
        &candidates,
        council.candidate_count,
        council.derived_by,
        &PRIMARY_PARLIAMENT_BODIES_V1,
    )
    .bodies
}
/// Derive a fresh Confirmation Jury from a later finalized pulse.
///
/// Policy Jury members are excluded unconditionally. If the remaining electorate is smaller than
/// the configured jury, the nonempty feasible roster is binding and its reduced size is visible in
/// the returned roster.
#[must_use]
pub fn derive_confirmation_jury(
    gov_cfg: &Governance,
    network_id: &NetworkId,
    pulse_height: u64,
    future_beacon: &[u8; 32],
    candidates: &[AccountId],
    policy_jury_members: &BTreeSet<AccountId>,
    derived_by: CouncilDerivationKind,
) -> ParliamentDrawPlan {
    let eligible: Vec<_> = candidates
        .iter()
        .filter(|candidate| !policy_jury_members.contains(*candidate))
        .cloned()
        .collect();
    let candidate_count = u32::try_from(eligible.len()).unwrap_or(u32::MAX);
    derive_body_plan(
        gov_cfg,
        network_id,
        pulse_height,
        future_beacon,
        &eligible,
        candidate_count,
        derived_by,
        &[ParliamentBody::ConfirmationJury],
    )
}

/// Derive an attempt-local body plan from an exact precommitted candidate snapshot.
///
/// `pulse_height` and `future_beacon` must come from the finalized pulse named by
/// the corresponding sortition requests. The caller is responsible for checking
/// that `candidates` is the exact strictly ordered snapshot committed by every
/// request in `bodies`; this function defensively deduplicates without using the
/// caller's ordering as entropy.
#[must_use]
pub fn derive_attempt_body_plan_v1(
    gov_cfg: &Governance,
    network_id: &NetworkId,
    pulse_height: u64,
    future_beacon: &[u8; 32],
    candidates: &[AccountId],
    bodies: &[ParliamentBody],
) -> ParliamentDrawPlan {
    let candidates: Vec<_> = candidates
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let candidate_count = u32::try_from(candidates.len()).unwrap_or(u32::MAX);
    derive_body_plan(
        gov_cfg,
        network_id,
        pulse_height,
        future_beacon,
        &candidates,
        candidate_count,
        CouncilDerivationKind::Sortition,
        bodies,
    )
}

/// Return the smallest feasible simultaneous assignment cap for `bodies`.
#[must_use]
pub fn smallest_feasible_assignment_cap(
    gov_cfg: &Governance,
    candidate_count: usize,
    bodies: &[ParliamentBody],
) -> u32 {
    if candidate_count == 0 || bodies.is_empty() {
        return 0;
    }
    let required_seats = bodies.iter().fold(0usize, |total, body| {
        total.saturating_add(body_committee_size(gov_cfg, *body).min(candidate_count))
    });
    let cap = required_seats.div_ceil(candidate_count).min(bodies.len());
    u32::try_from(cap).unwrap_or(u32::MAX)
}

#[allow(clippy::too_many_arguments)]
fn derive_body_plan(
    gov_cfg: &Governance,
    network_id: &NetworkId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: &[AccountId],
    candidate_count: u32,
    derived_by: CouncilDerivationKind,
    bodies: &[ParliamentBody],
) -> ParliamentDrawPlan {
    let alternates_per_body = gov_cfg
        .parliament_alternate_size
        .unwrap_or(gov_cfg.parliament_committee_size);
    let assignment_cap = smallest_feasible_assignment_cap(gov_cfg, candidates.len(), bodies);
    let mut rankings = BTreeMap::new();
    for body in bodies {
        rankings.insert(
            *body,
            ranked_body_candidates(network_id, epoch, beacon, candidates, *body),
        );
    }
    let mut loads: BTreeMap<AccountId, u32> = candidates
        .iter()
        .cloned()
        .map(|candidate| (candidate, 0))
        .collect();
    let mut selected: BTreeMap<ParliamentBody, Vec<AccountId>> = BTreeMap::new();
    for body in bodies {
        let target = body_committee_size(gov_cfg, *body).min(candidates.len());
        let ranked = rankings
            .get(body)
            .expect("every requested body has a deterministic ranking");
        let mut eligible: Vec<_> = ranked
            .iter()
            .enumerate()
            .filter_map(|(rank, candidate)| {
                let load = loads.get(candidate).copied().unwrap_or(0);
                (load < assignment_cap).then_some((
                    load,
                    same_matter_overlap(*body, candidate, &selected),
                    rank,
                    candidate.clone(),
                ))
            })
            .collect();
        eligible.sort_by(|left, right| {
            (left.0, left.1, left.2, &left.3).cmp(&(right.0, right.1, right.2, &right.3))
        });
        let chosen_set: BTreeSet<_> = eligible
            .into_iter()
            .take(target)
            .map(|(_, _, _, candidate)| candidate)
            .collect();
        let members: Vec<_> = ranked
            .iter()
            .filter(|candidate| chosen_set.contains(*candidate))
            .cloned()
            .collect();
        for member in &members {
            let load = loads
                .get_mut(member)
                .expect("selected candidates originate in the frozen snapshot");
            *load = load.saturating_add(1);
        }
        selected.insert(*body, members);
    }

    let mut rosters = BTreeMap::new();
    for body in bodies {
        let members = selected.remove(body).unwrap_or_default();
        let member_set: BTreeSet<_> = members.iter().cloned().collect();
        let alternates = rankings
            .remove(body)
            .unwrap_or_default()
            .into_iter()
            .filter(|candidate| !member_set.contains(candidate))
            .filter(|candidate| loads.get(candidate).copied().unwrap_or(0) < assignment_cap)
            .take(alternates_per_body)
            .collect();
        rosters.insert(
            *body,
            ParliamentRoster {
                body: *body,
                epoch,
                members,
                alternates,
                candidate_count,
                derived_by,
            },
        );
    }
    ParliamentDrawPlan {
        bodies: ParliamentBodies {
            selection_epoch: epoch,
            rosters,
        },
        assignment_cap,
    }
}

fn same_matter_overlap(
    body: ParliamentBody,
    candidate: &AccountId,
    selected: &BTreeMap<ParliamentBody, Vec<AccountId>>,
) -> u8 {
    let related: &[ParliamentBody] = match body {
        ParliamentBody::CoordinationCouncil => &[ParliamentBody::ReviewPanel],
        ParliamentBody::MpcCommittee | ParliamentBody::FmaCommittee => &[
            ParliamentBody::ReviewPanel,
            ParliamentBody::CoordinationCouncil,
        ],
        ParliamentBody::OversightCommittee => &[
            ParliamentBody::ReviewPanel,
            ParliamentBody::CoordinationCouncil,
            ParliamentBody::MpcCommittee,
            ParliamentBody::FmaCommittee,
        ],
        ParliamentBody::PolicyJury => &[
            ParliamentBody::ReviewPanel,
            ParliamentBody::CoordinationCouncil,
            ParliamentBody::MpcCommittee,
            ParliamentBody::FmaCommittee,
            ParliamentBody::OversightCommittee,
        ],
        ParliamentBody::ConfirmationJury => &[ParliamentBody::PolicyJury],
        ParliamentBody::RulesCommittee
        | ParliamentBody::AgendaCouncil
        | ParliamentBody::InterestPanel
        | ParliamentBody::ReviewPanel => &[],
    };
    related.iter().fold(0u8, |overlap, related_body| {
        overlap.saturating_add(u8::from(
            selected
                .get(related_body)
                .is_some_and(|members| members.contains(candidate)),
        ))
    })
}
pub(crate) fn body_committee_size(cfg: &Governance, body: ParliamentBody) -> usize {
    match body {
        ParliamentBody::RulesCommittee => cfg.rules_committee_size,
        ParliamentBody::AgendaCouncil => cfg.agenda_council_size,
        ParliamentBody::InterestPanel => cfg.interest_panel_size,
        ParliamentBody::ReviewPanel => cfg.review_panel_size,
        ParliamentBody::CoordinationCouncil => cfg.coordination_council_size,
        ParliamentBody::MpcCommittee => cfg.mpc_committee_size,
        ParliamentBody::FmaCommittee => cfg.fma_committee_size,
        ParliamentBody::OversightCommittee => cfg.oversight_committee_size,
        ParliamentBody::PolicyJury => cfg.policy_jury_size,
        ParliamentBody::ConfirmationJury => cfg.confirmation_jury_size,
    }
}
fn body_seed_domain(body: ParliamentBody) -> &'static [u8] {
    match body {
        ParliamentBody::RulesCommittee => b"gov:parliament:body:rules:v1",
        ParliamentBody::AgendaCouncil => b"gov:parliament:body:agenda:v1",
        ParliamentBody::InterestPanel => b"gov:parliament:body:interest:v1",
        ParliamentBody::ReviewPanel => b"gov:parliament:body:review:v1",
        ParliamentBody::CoordinationCouncil => b"gov:parliament:body:coordination:v1",
        ParliamentBody::MpcCommittee => b"gov:parliament:body:mpc:v1",
        ParliamentBody::FmaCommittee => b"gov:parliament:body:fma:v1",
        ParliamentBody::OversightCommittee => b"gov:parliament:body:oversight:v1",
        ParliamentBody::PolicyJury => b"gov:parliament:body:policy_jury:v1",
        ParliamentBody::ConfirmationJury => b"gov:parliament:body:confirmation_jury:v1",
    }
}
fn body_input_domain(body: ParliamentBody) -> &'static [u8] {
    match body {
        ParliamentBody::RulesCommittee => b"iroha:beacon-sortition:v1:parliament:rules|",
        ParliamentBody::AgendaCouncil => b"iroha:beacon-sortition:v1:parliament:agenda|",
        ParliamentBody::InterestPanel => b"iroha:beacon-sortition:v1:parliament:interest|",
        ParliamentBody::ReviewPanel => b"iroha:beacon-sortition:v1:parliament:review|",
        ParliamentBody::CoordinationCouncil => {
            b"iroha:beacon-sortition:v1:parliament:coordination|"
        }
        ParliamentBody::MpcCommittee => b"iroha:beacon-sortition:v1:parliament:mpc|",
        ParliamentBody::FmaCommittee => b"iroha:beacon-sortition:v1:parliament:fma|",
        ParliamentBody::OversightCommittee => b"iroha:beacon-sortition:v1:parliament:oversight|",
        ParliamentBody::PolicyJury => b"iroha:beacon-sortition:v1:parliament:policy_jury|",
        ParliamentBody::ConfirmationJury => {
            b"iroha:beacon-sortition:v1:parliament:confirmation_jury|"
        }
    }
}
fn ranked_body_candidates(
    network_id: &NetworkId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: &[AccountId],
    body: ParliamentBody,
) -> Vec<AccountId> {
    let seed = sortition::compute_seed(network_id, epoch, beacon, body_seed_domain(body));
    let mut scored: Vec<([u8; 32], AccountId)> = Vec::new();
    for account_id in candidates {
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
    scored
        .into_iter()
        .map(|(_, account_id)| account_id)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId, account::AccountId, block::BlockHeader, governance::types::ParliamentBody,
    };
    use std::collections::BTreeSet;
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
        for body in PRIMARY_PARLIAMENT_BODIES_V1 {
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
        for body in PRIMARY_PARLIAMENT_BODIES_V1 {
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
        let distinct_member_lists: BTreeSet<_> = PRIMARY_PARLIAMENT_BODIES_V1
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
    fn forty_six_citizens_bind_each_primary_body_to_its_feasible_size() {
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
            (ParliamentBody::RulesCommittee, 46),
            (ParliamentBody::AgendaCouncil, 46),
            (ParliamentBody::InterestPanel, 12),
            (ParliamentBody::ReviewPanel, 46),
            (ParliamentBody::CoordinationCouncil, 46),
            (ParliamentBody::MpcCommittee, 46),
            (ParliamentBody::FmaCommittee, 46),
            (ParliamentBody::OversightCommittee, 46),
            (ParliamentBody::PolicyJury, 46),
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
        assert_eq!(total_member_seats, 380);
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

    #[test]
    fn simultaneous_primary_draw_uses_the_smallest_feasible_overlap_cap() {
        let network_id = network_id(b"body-cap-demo");
        let beacon = [0xD4; 32];
        let accounts: Vec<_> = (1..=3).map(mk_account).collect();
        let cfg = Governance {
            rules_committee_size: 2,
            agenda_council_size: 2,
            interest_panel_size: 2,
            review_panel_size: 2,
            coordination_council_size: 2,
            mpc_committee_size: 2,
            fma_committee_size: 2,
            oversight_committee_size: 2,
            policy_jury_size: 2,
            parliament_alternate_size: Some(0),
            ..Governance::default()
        };
        let plan = derive_body_plan(
            &cfg,
            &network_id,
            19,
            &beacon,
            &accounts,
            3,
            CouncilDerivationKind::Sortition,
            &PRIMARY_PARLIAMENT_BODIES_V1,
        );
        assert_eq!(plan.assignment_cap, 6);
        let mut loads: BTreeMap<AccountId, u32> = BTreeMap::new();
        for roster in plan.bodies.rosters.values() {
            assert_eq!(roster.members.len(), 2);
            for member in &roster.members {
                *loads.entry(member.clone()).or_default() += 1;
            }
        }
        assert_eq!(loads.values().copied().max(), Some(plan.assignment_cap));
        assert_eq!(loads.values().copied().min(), Some(plan.assignment_cap));
    }

    #[test]
    fn attempt_body_plan_uses_only_snapshot_set_and_finalized_pulse() {
        let network_id = network_id(b"attempt-body-plan-demo");
        let accounts: Vec<_> = (1..=8).map(mk_account).collect();
        let mut reordered_with_duplicate = accounts.iter().rev().cloned().collect::<Vec<_>>();
        reordered_with_duplicate.push(accounts[3].clone());
        let cfg = Governance {
            rules_committee_size: 3,
            agenda_council_size: 3,
            parliament_alternate_size: Some(2),
            ..Governance::default()
        };
        let bodies = [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
        ];
        let expected =
            derive_attempt_body_plan_v1(&cfg, &network_id, 31, &[0xA7; 32], &accounts, &bodies);
        let reordered = derive_attempt_body_plan_v1(
            &cfg,
            &network_id,
            31,
            &[0xA7; 32],
            &reordered_with_duplicate,
            &bodies,
        );
        assert_eq!(expected, reordered);
        assert_ne!(
            expected,
            derive_attempt_body_plan_v1(&cfg, &network_id, 32, &[0xA8; 32], &accounts, &bodies,)
        );
    }

    #[test]
    fn enough_citizens_make_all_primary_assignments_disjoint() {
        let network_id = network_id(b"body-disjoint-demo");
        let beacon = [0xE5; 32];
        let accounts: Vec<_> = (1..=18).map(mk_account).collect();
        let cfg = Governance {
            rules_committee_size: 2,
            agenda_council_size: 2,
            interest_panel_size: 2,
            review_panel_size: 2,
            coordination_council_size: 2,
            mpc_committee_size: 2,
            fma_committee_size: 2,
            oversight_committee_size: 2,
            policy_jury_size: 2,
            parliament_alternate_size: Some(0),
            ..Governance::default()
        };
        let plan = derive_body_plan(
            &cfg,
            &network_id,
            20,
            &beacon,
            &accounts,
            18,
            CouncilDerivationKind::Sortition,
            &PRIMARY_PARLIAMENT_BODIES_V1,
        );
        assert_eq!(plan.assignment_cap, 1);
        let members: Vec<_> = plan
            .bodies
            .rosters
            .values()
            .flat_map(|roster| roster.members.iter())
            .collect();
        let unique: BTreeSet<_> = members.iter().copied().collect();
        assert_eq!(members.len(), unique.len());
    }

    #[test]
    fn confirmation_jury_uses_a_fresh_pulse_and_excludes_policy_jurors() {
        let network_id = network_id(b"confirmation-jury-demo");
        let candidates: Vec<_> = (1..=8).map(mk_account).collect();
        let policy_jury_members: BTreeSet<_> = candidates[..3].iter().cloned().collect();
        let cfg = Governance {
            confirmation_jury_size: 4,
            parliament_alternate_size: Some(1),
            ..Governance::default()
        };
        let first = derive_confirmation_jury(
            &cfg,
            &network_id,
            21,
            &[0xF1; 32],
            &candidates,
            &policy_jury_members,
            CouncilDerivationKind::Sortition,
        );
        let second = derive_confirmation_jury(
            &cfg,
            &network_id,
            22,
            &[0xF2; 32],
            &candidates,
            &policy_jury_members,
            CouncilDerivationKind::Sortition,
        );
        for plan in [&first, &second] {
            assert_eq!(plan.assignment_cap, 1);
            let roster = plan
                .bodies
                .rosters
                .get(&ParliamentBody::ConfirmationJury)
                .expect("confirmation roster");
            assert_eq!(roster.members.len(), 4);
            assert!(
                roster
                    .members
                    .iter()
                    .all(|member| !policy_jury_members.contains(member))
            );
        }
        assert_ne!(first.bodies, second.bodies);
    }
}
