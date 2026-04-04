//! Adversarial SORA parliament tests for wealthy Sybil-style sortition capture.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use core::num::NonZeroU64;
use std::collections::BTreeSet;

use iroha_core::{
    governance::{draw, state::ParliamentTerm},
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly, council_quorum_threshold},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::BlockHeader,
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
        ProposalKind,
    },
    isi::governance::ApproveGovernanceProposal,
    smart_contract::manifest::{ContractManifest, ManifestProvenance},
};
use mv::storage::StorageReadOnly;

const ATTACKER_BOND_XOR: u128 = 10_000;
const ATTACKER_COUNT: usize = 97;
const HONEST_COUNT: usize = 3;
const REFERENDUM_END: u64 = 500;
const MULTI_BEACON_SAMPLES: u64 = 32;

fn mk_account(seed: u8) -> AccountId {
    let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    let (public_key, _) = keypair.into_parts();
    AccountId::new(public_key)
}

fn canonical_abi_hash_bytes() -> [u8; 32] {
    ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)
}

fn manifest_provenance(
    code_hash_hex: &str,
    abi_hash_hex: &str,
    signer: &KeyPair,
) -> ManifestProvenance {
    fn parse_hex32(input: &str) -> [u8; 32] {
        let bytes = hex::decode(input).expect("hex should decode");
        let mut out = [0_u8; 32];
        out.copy_from_slice(&bytes);
        out
    }

    ContractManifest {
        code_hash: Some(Hash::prehashed(parse_hex32(code_hash_hex))),
        abi_hash: Some(Hash::prehashed(parse_hex32(abi_hash_hex))),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(signer)
    .provenance
    .expect("manifest should contain provenance")
}

fn seeded_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let mut cfg = state.gov.clone();
    cfg.parliament_term_blocks = 100;
    cfg.parliament_quorum_bps = 6_667;
    cfg.plain_voting_enabled = true;
    cfg.min_turnout = 0;
    cfg.approval_threshold_q_num = 1;
    cfg.approval_threshold_q_den = 2;
    cfg.min_enactment_delay = 0;
    cfg.window_span = 100;
    cfg.conviction_step_blocks = 1;
    cfg.max_conviction = 6;
    state.set_gov(cfg);
    state
}

fn seed_sortition_term_from_candidates(
    candidates: &[(AccountId, u128)],
    beacon: [u8; 32],
) -> (
    ChainId,
    [u8; 32],
    ParliamentTerm,
    iroha_data_model::governance::types::ParliamentBodies,
) {
    let chain: ChainId = "sora-adversarial-test-chain".into();
    let epoch = 0_u64;
    let gov_cfg = iroha_config::parameters::actual::Governance::default();
    let committee = gov_cfg.parliament_committee_size;
    let alternates = gov_cfg
        .parliament_alternate_size
        .unwrap_or(committee)
        .max(committee);
    let draw = draw::run_citizen_draw(
        &chain,
        epoch,
        &beacon,
        candidates.iter().map(|(id, bond)| (id, *bond)),
        committee,
        alternates,
    );
    let term = ParliamentTerm {
        epoch,
        members: draw.members,
        alternates: draw.alternates,
        verified: u32::try_from(draw.verified).expect("verified count should fit u32"),
        candidate_count: u32::try_from(candidates.len()).expect("candidate count should fit u32"),
        derived_by: iroha_data_model::isi::governance::CouncilDerivationKind::Vrf,
    };
    let bodies = draw::derive_parliament_bodies(&gov_cfg, &chain, epoch, &beacon, &term);
    (chain, beacon, term, bodies)
}

fn seed_sortition_term_with_beacon(
    attacker_accounts: &[AccountId],
    honest_accounts: &[AccountId],
    beacon: [u8; 32],
) -> (
    ChainId,
    [u8; 32],
    ParliamentTerm,
    iroha_data_model::governance::types::ParliamentBodies,
) {
    let mut candidates: Vec<(AccountId, u128)> = attacker_accounts
        .iter()
        .cloned()
        .map(|id| (id, ATTACKER_BOND_XOR))
        .collect();
    candidates.extend(
        honest_accounts
            .iter()
            .cloned()
            .map(|id| (id, ATTACKER_BOND_XOR)),
    );
    seed_sortition_term_from_candidates(&candidates, beacon)
}

fn seed_sortition_term(
    attacker_accounts: &[AccountId],
    honest_accounts: &[AccountId],
) -> (
    ChainId,
    [u8; 32],
    ParliamentTerm,
    iroha_data_model::governance::types::ParliamentBodies,
) {
    seed_sortition_term_with_beacon(attacker_accounts, honest_accounts, [0x7b; 32])
}

fn body_list() -> [ParliamentBody; 7] {
    [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ]
}

fn beacon_from_seed(seed: u64) -> [u8; 32] {
    let mut beacon = [0_u8; 32];
    beacon[..8].copy_from_slice(&seed.to_le_bytes());
    beacon[8..16].copy_from_slice(&seed.rotate_left(13).to_le_bytes());
    beacon[16..24].copy_from_slice(&(!seed).to_le_bytes());
    beacon[24..32].copy_from_slice(&seed.wrapping_mul(0x9E37_79B9_7F4A_7C15_u64).to_le_bytes());
    beacon
}

fn controlled_member_seats(
    bodies: &iroha_data_model::governance::types::ParliamentBodies,
    attacker_set: &BTreeSet<AccountId>,
) -> usize {
    body_list()
        .into_iter()
        .map(|body| {
            let roster = bodies.rosters.get(&body).expect("body roster should exist");
            roster
                .members
                .iter()
                .filter(|id| attacker_set.contains(*id))
                .count()
        })
        .sum()
}

fn seed_proposal_and_referendum(
    state: &mut State,
    proposal_id: [u8; 32],
    proposal_kind: ProposalKind,
    proposer: &AccountId,
) -> String {
    let referendum_id = hex::encode(proposal_id);
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let referendum = iroha_core::state::GovernanceReferendumRecord {
        h_start: 1,
        h_end: REFERENDUM_END,
        status: iroha_core::state::GovernanceReferendumStatus::Proposed,
        mode: iroha_core::state::GovernanceReferendumMode::Plain,
    };
    stx.world
        .governance_referenda_mut()
        .insert(referendum_id.clone(), referendum);
    let pipeline = iroha_core::state::GovernancePipeline::seeded(1, Some(&referendum), &stx.gov);
    stx.world.governance_proposals_mut().insert(
        proposal_id,
        iroha_core::state::GovernanceProposalRecord {
            proposer: proposer.clone(),
            kind: proposal_kind,
            created_height: 1,
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
            pipeline,
            parliament_snapshot: None,
        },
    );
    stx.apply();
    block.commit().expect("seed proposal block commit");
    referendum_id
}

fn seed_captured_parliament(
    state: &mut State,
    members: Vec<AccountId>,
    alternates: Vec<AccountId>,
) {
    let header = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let council = iroha_core::governance::state::ParliamentTerm {
        epoch: 0,
        members: members.clone(),
        alternates: alternates.clone(),
        verified: u32::try_from(members.len()).expect("member count should fit u32"),
        candidate_count: u32::try_from(members.len().saturating_add(alternates.len()))
            .expect("candidate count should fit u32"),
        derived_by: iroha_data_model::isi::governance::CouncilDerivationKind::Fallback,
    };
    stx.world.council_mut().insert(0, council.clone());
    let bodies = iroha_core::governance::draw::derive_parliament_bodies(
        &stx.gov,
        &ChainId::from("sora-adversarial-test-chain"),
        0,
        &[0x7b; 32],
        &council,
    );
    stx.world.parliament_bodies_mut().insert(0, bodies);
    stx.apply();
    block.commit().expect("seed parliament block commit");
}

fn approve(
    state: &mut State,
    height: u64,
    body: ParliamentBody,
    proposal_id: [u8; 32],
    signer: &AccountId,
) -> Result<(), iroha_data_model::isi::error::InstructionExecutionError> {
    let mut block = state.block(BlockHeader::new(
        NonZeroU64::new(height).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx = block.transaction();
    let result = ApproveGovernanceProposal { body, proposal_id }.execute(signer, &mut stx);
    match result {
        Ok(()) => {
            stx.apply();
            block.commit().expect("approve block commit");
            Ok(())
        }
        Err(err) => Err(err),
    }
}

#[test]
fn wealthy_sybil_can_capture_multibody_sortition_share_under_good_settings() {
    let attackers: Vec<AccountId> = (0..ATTACKER_COUNT)
        .map(|idx| mk_account(u8::try_from(idx + 1).expect("index should fit u8")))
        .collect();
    let honest: Vec<AccountId> = (0..HONEST_COUNT)
        .map(|idx| mk_account(u8::try_from(idx + ATTACKER_COUNT + 1).expect("index should fit u8")))
        .collect();

    let (_chain, _beacon, _term, bodies) = seed_sortition_term(&attackers, &honest);
    let attacker_set: BTreeSet<_> = attackers.iter().cloned().collect();
    let cfg = iroha_config::parameters::actual::Governance::default();

    for body in body_list() {
        let roster = bodies.rosters.get(&body).expect("body roster should exist");
        let controlled = roster
            .members
            .iter()
            .filter(|id| attacker_set.contains(*id))
            .count();
        let required = usize::try_from(council_quorum_threshold(
            roster.members.len(),
            cfg.parliament_quorum_bps,
        ))
        .expect("threshold should fit usize");
        assert!(
            controlled >= required,
            "attackers should satisfy quorum for {body:?}: controlled={controlled}, required={required}, roster_size={}",
            roster.members.len()
        );
    }
}

#[test]
fn wealthy_sybil_capture_is_stable_across_many_beacons() {
    let attackers: Vec<AccountId> = (0..ATTACKER_COUNT)
        .map(|idx| mk_account(u8::try_from(idx + 1).expect("index should fit u8")))
        .collect();
    let honest: Vec<AccountId> = (0..HONEST_COUNT)
        .map(|idx| mk_account(u8::try_from(idx + ATTACKER_COUNT + 1).expect("index should fit u8")))
        .collect();
    let attacker_set: BTreeSet<_> = attackers.iter().cloned().collect();
    let cfg = iroha_config::parameters::actual::Governance::default();
    let mut misses = Vec::new();

    for seed in 0..MULTI_BEACON_SAMPLES {
        let beacon = beacon_from_seed(seed);
        let (_chain, _beacon, _term, bodies) =
            seed_sortition_term_with_beacon(&attackers, &honest, beacon);
        for body in body_list() {
            let roster = bodies.rosters.get(&body).expect("body roster should exist");
            let controlled = roster
                .members
                .iter()
                .filter(|id| attacker_set.contains(*id))
                .count();
            let required = usize::try_from(council_quorum_threshold(
                roster.members.len(),
                cfg.parliament_quorum_bps,
            ))
            .expect("threshold should fit usize");
            if controlled < required {
                misses.push((seed, body, controlled, required, roster.members.len()));
            }
        }
    }

    const MAX_ALLOWED_MISSES: usize = 1;
    assert!(
        misses.len() <= MAX_ALLOWED_MISSES,
        "attackers should keep quorum for nearly all beacon seeds: misses={}/{} (max={MAX_ALLOWED_MISSES}) details={misses:?}",
        misses.len(),
        MULTI_BEACON_SAMPLES as usize * body_list().len()
    );
}

#[test]
fn splitting_budget_into_many_10k_identities_outcontrols_single_whale() {
    const SPLIT_ATTACKER_COUNT: usize = 20;
    const HONEST_BASELINE_COUNT: usize = 3;
    let split_attackers: Vec<AccountId> = (0..SPLIT_ATTACKER_COUNT)
        .map(|idx| mk_account(u8::try_from(idx + 100).expect("index should fit u8")))
        .collect();
    let whale_attacker = mk_account(160);
    let honest: Vec<AccountId> = (0..HONEST_BASELINE_COUNT)
        .map(|idx| mk_account(u8::try_from(idx + 170).expect("index should fit u8")))
        .collect();

    let total_attacker_budget = ATTACKER_BOND_XOR
        .saturating_mul(u128::try_from(SPLIT_ATTACKER_COUNT).expect("count should fit u128"));
    let split_candidates: Vec<(AccountId, u128)> = split_attackers
        .iter()
        .cloned()
        .map(|id| (id, ATTACKER_BOND_XOR))
        .chain(honest.iter().cloned().map(|id| (id, ATTACKER_BOND_XOR)))
        .collect();
    let whale_candidates: Vec<(AccountId, u128)> =
        std::iter::once((whale_attacker.clone(), total_attacker_budget))
            .chain(honest.iter().cloned().map(|id| (id, ATTACKER_BOND_XOR)))
            .collect();

    let split_set: BTreeSet<_> = split_attackers.iter().cloned().collect();
    let whale_set: BTreeSet<_> = std::iter::once(whale_attacker).collect();

    for seed in 0..MULTI_BEACON_SAMPLES {
        let beacon = beacon_from_seed(seed);
        let (_chain, _beacon, _term, split_bodies) =
            seed_sortition_term_from_candidates(&split_candidates, beacon);
        let split_control = controlled_member_seats(&split_bodies, &split_set);
        let (_chain, _beacon, _term, whale_bodies) =
            seed_sortition_term_from_candidates(&whale_candidates, beacon);
        let whale_control = controlled_member_seats(&whale_bodies, &whale_set);

        assert!(
            split_control > whale_control,
            "10k-account splitting should out-control a single whale with the same budget for beacon seed {seed}: split_control={split_control}, whale_control={whale_control}"
        );
    }
}

#[test]
fn attacker_control_seat_count_is_monotonic_with_identity_set_growth() {
    let all_accounts: Vec<AccountId> = (0..(ATTACKER_COUNT + HONEST_COUNT))
        .map(|idx| mk_account(u8::try_from(idx + 1).expect("index should fit u8")))
        .collect();
    let attackers_hi: Vec<AccountId> = all_accounts.iter().take(97).cloned().collect();
    let honest: Vec<AccountId> = all_accounts.iter().skip(97).cloned().collect();
    let (_chain, _beacon, _term, bodies) = seed_sortition_term(&attackers_hi, &honest);

    let attacker_lo: BTreeSet<_> = all_accounts.iter().take(10).cloned().collect();
    let attacker_mid: BTreeSet<_> = all_accounts.iter().take(50).cloned().collect();
    let attacker_hi: BTreeSet<_> = all_accounts.iter().take(97).cloned().collect();

    let controlled_total = |set: &BTreeSet<AccountId>| -> usize {
        body_list()
            .into_iter()
            .map(|body| {
                let roster = bodies.rosters.get(&body).expect("body roster should exist");
                roster.members.iter().filter(|id| set.contains(*id)).count()
            })
            .sum()
    };

    let lo = controlled_total(&attacker_lo);
    let mid = controlled_total(&attacker_mid);
    let hi = controlled_total(&attacker_hi);
    assert!(
        lo <= mid && mid <= hi,
        "controlled seat totals must be monotonic with attacker identity-set growth: lo={lo}, mid={mid}, hi={hi}"
    );
}

#[test]
fn duplicate_approvals_do_not_count_twice_for_quorum() {
    let mut state = seeded_state();
    let attacker_a = mk_account(1);
    let attacker_b = mk_account(2);
    let honest = mk_account(3);
    let proposal_id = [0xAA; 32];
    let rid = seed_proposal_and_referendum(
        &mut state,
        proposal_id,
        ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address"),
            code_hash_hex: ContractCodeHash::from_hex_str(&"11".repeat(32)).expect("code hash"),
            abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode(canonical_abi_hash_bytes()))
                .expect("abi hash"),
            abi_version: AbiVersion::new(1),
            manifest_provenance: Some(manifest_provenance(
                &"11".repeat(32),
                &hex::encode(canonical_abi_hash_bytes()),
                &KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519),
            )),
        }),
        &attacker_a,
    );

    seed_captured_parliament(
        &mut state,
        vec![attacker_a.clone(), attacker_b.clone(), honest.clone()],
        vec![],
    );

    approve(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        proposal_id,
        &attacker_a,
    )
    .expect("first rules approval");
    approve(
        &mut state,
        5,
        ParliamentBody::RulesCommittee,
        proposal_id,
        &attacker_a,
    )
    .expect("duplicate rules approval should be ignored");
    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("stage approvals should exist");
    let rules_stage = approvals
        .stages
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules stage should exist");
    assert_eq!(
        rules_stage.approvers.len(),
        1,
        "duplicate approval from same signer must not increase recorded approvers"
    );

    approve(
        &mut state,
        6,
        ParliamentBody::RulesCommittee,
        proposal_id,
        &attacker_b,
    )
    .expect("second unique rules approval");
    approve(
        &mut state,
        7,
        ParliamentBody::AgendaCouncil,
        proposal_id,
        &attacker_a,
    )
    .expect("first agenda approval");
    approve(
        &mut state,
        8,
        ParliamentBody::AgendaCouncil,
        proposal_id,
        &attacker_a,
    )
    .expect("duplicate agenda approval should be ignored");
    let referendum = state
        .view()
        .world()
        .governance_referenda()
        .get(&rid)
        .copied()
        .expect("referendum should exist");
    assert_eq!(
        referendum.status,
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "single-signer duplicates must not open referendum before unique quorum"
    );

    approve(
        &mut state,
        9,
        ParliamentBody::AgendaCouncil,
        proposal_id,
        &attacker_b,
    )
    .expect("second unique agenda approval");
    let approvals = state
        .view()
        .world()
        .governance_stage_approvals()
        .get(&rid)
        .cloned()
        .expect("stage approvals should exist");
    let agenda_stage = approvals
        .stages
        .get(&ParliamentBody::AgendaCouncil)
        .expect("agenda stage should exist");
    assert_eq!(
        agenda_stage.approvers.len(),
        2,
        "agenda stage should count unique approvers and ignore duplicates"
    );

    let referendum = state
        .view()
        .world()
        .governance_referenda()
        .get(&rid)
        .copied()
        .expect("referendum should exist");
    assert_eq!(
        referendum.status,
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "duplicated signatures must not accelerate progression; remaining body approvals are still required"
    );
}

#[test]
fn wealthy_non_members_cannot_open_referendum_without_sortition_capture() {
    let mut state = seeded_state();
    let attacker_a = mk_account(31);
    let attacker_b = mk_account(32);
    let honest_a = mk_account(33);
    let honest_b = mk_account(34);
    let honest_c = mk_account(35);
    let proposal_id = [0xDD; 32];
    let rid = seed_proposal_and_referendum(
        &mut state,
        proposal_id,
        ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address"),
            code_hash_hex: ContractCodeHash::from_hex_str(&"33".repeat(32)).expect("code hash"),
            abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode(canonical_abi_hash_bytes()))
                .expect("abi hash"),
            abi_version: AbiVersion::new(1),
            manifest_provenance: Some(manifest_provenance(
                &"33".repeat(32),
                &hex::encode(canonical_abi_hash_bytes()),
                &KeyPair::from_seed(vec![19; 32], Algorithm::Ed25519),
            )),
        }),
        &honest_a,
    );
    seed_captured_parliament(
        &mut state,
        vec![honest_a.clone(), honest_b.clone(), honest_c.clone()],
        vec![],
    );

    let err_rules = approve(
        &mut state,
        4,
        ParliamentBody::RulesCommittee,
        proposal_id,
        &attacker_a,
    )
    .expect_err("non-member rules approval must fail");
    assert!(
        err_rules.to_string().contains("only seated members"),
        "unexpected rules error: {err_rules:?}"
    );
    let err_agenda = approve(
        &mut state,
        5,
        ParliamentBody::AgendaCouncil,
        proposal_id,
        &attacker_b,
    )
    .expect_err("non-member agenda approval must fail");
    assert!(
        err_agenda.to_string().contains("only seated members"),
        "unexpected agenda error: {err_agenda:?}"
    );

    let referendum = state
        .view()
        .world()
        .governance_referenda()
        .get(&rid)
        .copied()
        .expect("referendum should exist");
    assert_eq!(
        referendum.status,
        iroha_core::state::GovernanceReferendumStatus::Proposed,
        "wealth alone without sortition capture must not open the referendum"
    );
}
