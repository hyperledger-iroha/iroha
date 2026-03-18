//! End-to-end SORA parliament lifecycle test for plain voting.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    Registrable,
    account::Account,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::{Domain, DomainId},
    governance::types::ParliamentBody,
    isi::governance::{
        ApproveGovernanceProposal, AtWindow, CastPlainBallot, CouncilDerivationKind,
        EnactReferendum, FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract,
        RegisterCitizen, VotingMode,
    },
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant, Transfer},
    smart_contract::manifest::{ContractManifest, ManifestProvenance},
};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanProposeContractDeployment, CanSubmitGovernanceBallot,
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const CITIZEN_COUNT: usize = 20;
const CITIZEN_FUND: u128 = 15_000;
const CITIZEN_BOND: u128 = 10_000;
const BALLOT_LOCK: u128 = CITIZEN_FUND - CITIZEN_BOND;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

fn parse_hex32(input: &str) -> [u8; 32] {
    let bytes = hex::decode(input).expect("hex should decode");
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

fn manifest_provenance(
    code_hash_hex: &str,
    abi_hash_hex: &str,
    signer: &KeyPair,
) -> ManifestProvenance {
    let code_hash = Hash::prehashed(parse_hex32(code_hash_hex));
    let abi_hash = Hash::prehashed(parse_hex32(abi_hash_hex));
    ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(abi_hash),
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

#[test]
fn sora_parliament_plain_lifecycle_with_20_citizens() {
    let domain_id: DomainId = "sora".parse().expect("domain");
    let (proposer_id, proposer_kp) = gen_account_in("sora");
    let (escrow_id, _escrow_kp) = gen_account_in("sora");
    let citizens: Vec<_> = (0..CITIZEN_COUNT)
        .map(|_| {
            let (id, _kp) = gen_account_in("sora");
            id
        })
        .collect();

    let domain = Domain::new(domain_id.clone()).build(&proposer_id);
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "sora".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&proposer_id);

    let proposer_asset = Asset::new(
        AssetId::new(asset_def_id.clone(), proposer_id.clone()),
        Numeric::new(1_000_000, 0),
    );
    let escrow_asset = Asset::new(
        AssetId::new(asset_def_id.clone(), escrow_id.clone()),
        Numeric::new(0, 0),
    );

    let proposer_account =
        Account::new(proposer_id.clone().to_account_id(domain_id.clone())).build(&proposer_id);
    let escrow_account =
        Account::new(escrow_id.clone().to_account_id(domain_id.clone())).build(&proposer_id);
    let citizen_accounts = citizens
        .iter()
        .cloned()
        .map(|id| Account::new(id.to_account_id(domain_id.clone())).build(&proposer_id));

    let world = World::with_assets(
        [domain],
        std::iter::once(proposer_account)
            .chain(std::iter::once(escrow_account))
            .chain(citizen_accounts)
            .collect::<Vec<_>>(),
        [asset_def],
        [proposer_asset, escrow_asset],
        [],
    );

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.voting_asset_id = asset_def_id.clone();
    gov_cfg.citizenship_asset_id = asset_def_id.clone();
    gov_cfg.citizenship_bond_amount = CITIZEN_BOND;
    gov_cfg.citizenship_escrow_account = escrow_id.clone();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = escrow_id.clone();
    gov_cfg.min_bond_amount = 0;
    gov_cfg.conviction_step_blocks = 1;
    gov_cfg.min_enactment_delay = 0;
    gov_cfg.window_span = 10;
    gov_cfg.min_turnout = 0;
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.parliament_term_blocks = 100;
    gov_cfg.rules_committee_size = 1;
    gov_cfg.agenda_council_size = 1;
    gov_cfg.interest_panel_size = 1;
    gov_cfg.review_panel_size = 1;
    gov_cfg.policy_jury_size = 1;
    gov_cfg.oversight_committee_size = 1;
    gov_cfg.fma_committee_size = 1;
    gov_cfg.parliament_alternate_size = Some(1);
    state.set_gov(gov_cfg);

    let code_hash_hex = "aa".repeat(32);
    let abi_hash_hex = canonical_abi_hex();
    let manifest_provenance = manifest_provenance(&code_hash_hex, &abi_hash_hex, &proposer_kp);

    let proposal_contract_id = "parliament.lifecycle.contract".to_string();

    let header_1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block_1 = state.block(header_1);
    let mut stx_1 = block_1.transaction();

    let propose_perm: Permission = CanProposeContractDeployment {
        contract_id: proposal_contract_id.clone(),
    }
    .into();
    Grant::account_permission(propose_perm, proposer_id.clone())
        .execute(&proposer_id, &mut stx_1)
        .expect("grant proposer permission");

    let enact_perm: Permission = CanEnactGovernance.into();
    Grant::account_permission(enact_perm, proposer_id.clone())
        .execute(&proposer_id, &mut stx_1)
        .expect("grant enact permission");

    for citizen in &citizens {
        let ballot_perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: "sora-parliament-lifecycle".to_string(),
        }
        .into();
        Grant::account_permission(ballot_perm, citizen.clone())
            .execute(&proposer_id, &mut stx_1)
            .expect("grant ballot permission");
    }

    for citizen in &citizens {
        Transfer::asset_numeric(
            AssetId::new(asset_def_id.clone(), proposer_id.clone()),
            Numeric::new(CITIZEN_FUND, 0),
            citizen.clone(),
        )
        .execute(&proposer_id, &mut stx_1)
        .expect("fund citizen account");

        RegisterCitizen {
            owner: citizen.clone(),
            amount: CITIZEN_BOND,
        }
        .execute(citizen, &mut stx_1)
        .expect("bond citizenship");
    }

    PersistCouncilForEpoch {
        epoch: 0,
        members: citizens[..10].to_vec(),
        alternates: citizens[10..].to_vec(),
        verified: 0,
        candidates_count: u32::try_from(CITIZEN_COUNT).expect("count fits in u32"),
        derived_by: CouncilDerivationKind::Fallback,
    }
    .execute(&proposer_id, &mut stx_1)
    .expect("persist council");

    ProposeDeployContract {
        namespace: "sora".to_string(),
        contract_id: proposal_contract_id.clone(),
        code_hash_hex: code_hash_hex.clone(),
        abi_hash_hex: abi_hash_hex.clone(),
        abi_version: "1".to_string(),
        window: None,
        mode: Some(VotingMode::Plain),
        manifest_provenance: Some(manifest_provenance),
    }
    .execute(&proposer_id, &mut stx_1)
    .expect("propose contract deployment");

    let (proposal_id, _) = stx_1
        .world
        .governance_proposals()
        .iter()
        .next()
        .expect("proposal should exist");
    let proposal_id = *proposal_id;
    let referendum_id = hex::encode(proposal_id);

    for citizen in &citizens {
        let citizen_balance = stx_1
            .world
            .assets()
            .get(&AssetId::new(asset_def_id.clone(), citizen.clone()))
            .cloned()
            .expect("citizen balance should exist")
            .0;
        assert_eq!(
            citizen_balance,
            Numeric::new(CITIZEN_FUND.saturating_sub(CITIZEN_BOND), 0)
        );
    }

    let escrow_balance = stx_1
        .world
        .assets()
        .get(&AssetId::new(asset_def_id.clone(), escrow_id.clone()))
        .cloned()
        .expect("escrow balance should exist")
        .0;
    assert_eq!(
        escrow_balance,
        Numeric::new(
            CITIZEN_BOND.saturating_mul(u128::try_from(CITIZEN_COUNT).expect("count fits in u128")),
            0,
        )
    );

    let stage_bodies = stx_1
        .world
        .parliament_bodies()
        .get(&0)
        .cloned()
        .expect("parliament bodies for epoch 0");
    let rules_signer = stage_bodies
        .rosters
        .get(&ParliamentBody::RulesCommittee)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("rules committee signer");
    let agenda_signer = stage_bodies
        .rosters
        .get(&ParliamentBody::AgendaCouncil)
        .and_then(|roster| roster.members.first())
        .cloned()
        .expect("agenda council signer");

    ApproveGovernanceProposal {
        body: ParliamentBody::RulesCommittee,
        proposal_id,
    }
    .execute(&rules_signer, &mut stx_1)
    .expect("rules approval");

    ApproveGovernanceProposal {
        body: ParliamentBody::AgendaCouncil,
        proposal_id,
    }
    .execute(&agenda_signer, &mut stx_1)
    .expect("agenda approval");

    for (idx, citizen) in citizens.iter().enumerate() {
        let direction = if idx < 12 { 0 } else { 1 };
        CastPlainBallot {
            referendum_id: referendum_id.clone(),
            owner: citizen.clone(),
            amount: BALLOT_LOCK,
            duration_blocks: 20,
            direction,
        }
        .execute(citizen, &mut stx_1)
        .expect("cast plain ballot");
    }

    stx_1.apply();
    block_1
        .commit()
        .expect("commit setup/approval/ballot block");

    let referendum_after_ballots = state
        .view()
        .world()
        .governance_referenda()
        .get(&referendum_id)
        .copied()
        .expect("referendum record after ballots");
    assert_eq!(
        referendum_after_ballots.status,
        iroha_core::state::GovernanceReferendumStatus::Open
    );

    let header_2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block_2 = state.block(header_2);
    let mut stx_2 = block_2.transaction();

    FinalizeReferendum {
        referendum_id: referendum_id.clone(),
        proposal_id,
    }
    .execute(&proposer_id, &mut stx_2)
    .expect("finalize referendum");

    stx_2.apply();
    block_2.commit().expect("commit finalize block");

    let proposal_after_finalize = state
        .view()
        .world()
        .governance_proposals()
        .get(&proposal_id)
        .cloned()
        .expect("proposal record after finalize");
    assert!(matches!(
        proposal_after_finalize.status,
        iroha_core::state::GovernanceProposalStatus::Approved
    ));

    let referendum_window = state
        .view()
        .world()
        .governance_referenda()
        .get(&referendum_id)
        .copied()
        .expect("referendum exists before enact");

    let header_3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block_3 = state.block(header_3);
    let mut stx_3 = block_3.transaction();

    EnactReferendum {
        referendum_id: proposal_id,
        preimage_hash: [0; 32],
        at_window: AtWindow {
            lower: referendum_window.h_start,
            upper: referendum_window.h_end,
        },
    }
    .execute(&proposer_id, &mut stx_3)
    .expect("enact referendum");

    stx_3.apply();
    block_3.commit().expect("commit enact block");

    let proposal_after_enact = state
        .view()
        .world()
        .governance_proposals()
        .get(&proposal_id)
        .cloned()
        .expect("proposal record after enact");
    assert!(matches!(
        proposal_after_enact.status,
        iroha_core::state::GovernanceProposalStatus::Enacted
    ));

    let code_hash = Hash::prehashed(parse_hex32(&code_hash_hex));
    assert!(
        state
            .view()
            .world()
            .contract_instances()
            .get(&("sora".to_string(), proposal_contract_id))
            .is_some_and(|bound| *bound == code_hash),
        "contract instance should be bound to enacted hash"
    );
}
