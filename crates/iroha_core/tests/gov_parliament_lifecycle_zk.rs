//! End-to-end SORA parliament lifecycle test for ZK voting.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]

mod zk_testkit;

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
    isi::{
        governance::{
            ApproveGovernanceProposal, AtWindow, CastZkBallot, CouncilDerivationKind,
            EnactReferendum, FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract,
            RegisterCitizen, VotingMode,
        },
        verifying_keys,
        zk::{CreateElection, FinalizeElection},
    },
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant, Transfer},
    proof::{ProofAttachment, ProofBox},
    smart_contract::manifest::{ContractManifest, ManifestProvenance},
};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanManageParliament, CanProposeContractDeployment,
    CanSubmitGovernanceBallot,
};
use iroha_primitives::{json::Json, numeric::Numeric};
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
fn sora_parliament_zk_lifecycle_with_20_citizens() {
    let (proposer_id, proposer_kp) = gen_account_in("sora");
    let (escrow_id, _escrow_kp) = gen_account_in("sora");
    let citizens: Vec<_> = (0..CITIZEN_COUNT)
        .map(|_| {
            let (id, _kp) = gen_account_in("sora");
            id
        })
        .collect();

    let domain_id: DomainId = "sora".parse().expect("domain");
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
        Account::new(proposer_id.clone()).build(&proposer_id);
    let escrow_account =
        Account::new(escrow_id.clone()).build(&proposer_id);
    let citizen_accounts = citizens
        .iter()
        .cloned()
        .map(|id| Account::new(id.clone()).build(&proposer_id));

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
    state.zk.halo2.enabled = true;

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
    gov_cfg.plain_voting_enabled = false;
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

    let bundle_ballot_1 = zk_testkit::add2inst_public_bundle(5, 8);
    let bundle_ballot_2 = zk_testkit::add2inst_public_bundle(6, 8);
    let bundle_tally = zk_testkit::add2inst_public_bundle(7, 3);

    let code_hash_hex = "cc".repeat(32);
    let abi_hash_hex = canonical_abi_hex();
    let manifest_provenance = manifest_provenance(&code_hash_hex, &abi_hash_hex, &proposer_kp);

    let proposal_contract_id = "parliament.lifecycle.zk.contract".to_string();

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

    let manage_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(manage_parliament, proposer_id.clone())
        .execute(&proposer_id, &mut stx_1)
        .expect("grant parliament permission");

    let manage_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(manage_vk, proposer_id.clone())
        .execute(&proposer_id, &mut stx_1)
        .expect("grant verifying key permission");

    for citizen in &citizens {
        let ballot_perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: "sora-parliament-zk-lifecycle".to_string(),
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
        mode: Some(VotingMode::Zk),
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

    verifying_keys::RegisterVerifyingKey {
        id: bundle_ballot_1.vk_id.clone(),
        record: bundle_ballot_1.vk_record.clone(),
    }
    .execute(&proposer_id, &mut stx_1)
    .expect("register voting key");

    CreateElection {
        election_id: referendum_id.clone(),
        options: 2,
        eligible_root: bundle_ballot_1.root_bytes(),
        start_ts: 0,
        end_ts: 0,
        vk_ballot: bundle_ballot_1.vk_id.clone(),
        vk_tally: bundle_ballot_1.vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    }
    .execute(&proposer_id, &mut stx_1)
    .expect("create election");

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

    let root_hint = hex::encode(bundle_ballot_1.root_bytes());

    let voter_a = citizens[0].clone();
    let inputs_a = norito::json::object([
        (
            "owner",
            norito::json::to_value(&voter_a.to_string()).expect("owner json"),
        ),
        (
            "amount",
            norito::json::to_value(&BALLOT_LOCK).expect("amount json"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&20_u64).expect("duration json"),
        ),
        (
            "direction",
            norito::json::to_value("Aye").expect("direction json"),
        ),
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("root hint json"),
        ),
    ])
    .expect("serialize first ballot inputs");

    CastZkBallot {
        election_id: referendum_id.clone(),
        proof_b64: bundle_ballot_1.proof_b64.clone(),
        public_inputs_json: norito::json::to_json(&inputs_a).expect("ballot json"),
    }
    .execute(&voter_a, &mut stx_1)
    .expect("first zk ballot");

    let voter_b = citizens[1].clone();
    let inputs_b = norito::json::object([
        (
            "owner",
            norito::json::to_value(&voter_b.to_string()).expect("owner json"),
        ),
        (
            "amount",
            norito::json::to_value(&BALLOT_LOCK).expect("amount json"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&20_u64).expect("duration json"),
        ),
        (
            "direction",
            norito::json::to_value("Nay").expect("direction json"),
        ),
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("root hint json"),
        ),
    ])
    .expect("serialize second ballot inputs");

    CastZkBallot {
        election_id: referendum_id.clone(),
        proof_b64: bundle_ballot_2.proof_b64.clone(),
        public_inputs_json: norito::json::to_json(&inputs_b).expect("ballot json"),
    }
    .execute(&voter_b, &mut stx_1)
    .expect("second zk ballot");

    stx_1.apply();
    block_1
        .commit()
        .expect("commit setup/approval/ballot block");

    let header_2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block_2 = state.block(header_2);
    let mut stx_2 = block_2.transaction();

    let tally_attachment = ProofAttachment::new_ref(
        bundle_tally.backend.to_string(),
        ProofBox::new(
            bundle_tally.backend.to_string(),
            bundle_tally.proof_bytes.clone(),
        ),
        bundle_ballot_1.vk_id.clone(),
    );

    FinalizeElection {
        election_id: referendum_id.clone(),
        tally: vec![7, 3],
        tally_proof: tally_attachment,
    }
    .execute(&proposer_id, &mut stx_2)
    .expect("finalize election");

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
