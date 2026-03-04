//! Governance instruction validation tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]

use core::convert::TryInto;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
};
use iroha_crypto::{KeyPair, blake2::Digest as _};
use iroha_data_model::{
    governance::types::{
        AbiVersion, AtWindow, ContractAbiHash, ContractCodeHash, DeployContractProposal,
        ProposalKind,
    },
    permission::Permission,
    prelude::*,
};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanProposeContractDeployment, CanSubmitGovernanceBallot,
};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn mk_state_and_authority() -> (State, iroha_data_model::account::AccountId) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let kp = KeyPair::random();
    let (pk, _) = kp.clone().into_parts();
    let domain_id: DomainId = "apps".parse().unwrap();
    let account_id = AccountId::of(domain_id.clone(), pk);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world =
        iroha_core::state::World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query);
    (state, account_id)
}

fn compute_proposal_id(
    namespace: &str,
    contract_id: &str,
    code_hex: &str,
    abi_hex: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::Blake2b512;
    let namespace = namespace.trim();
    let contract_id = contract_id.trim();
    let code = hex::decode(code_hex).expect("valid code hex");
    let abi = hex::decode(abi_hex).expect("valid abi hex");
    let ns_len: u32 = namespace
        .len()
        .try_into()
        .expect("namespace length fits u32");
    let cid_len: u32 = contract_id
        .len()
        .try_into()
        .expect("contract length fits u32");
    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + core::mem::size_of::<u32>() * 2
            + namespace.len()
            + contract_id.len()
            + code.len()
            + abi.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&ns_len.to_le_bytes());
    input.extend_from_slice(namespace.as_bytes());
    input.extend_from_slice(&cid_len.to_le_bytes());
    input.extend_from_slice(contract_id.as_bytes());
    input.extend_from_slice(&code);
    input.extend_from_slice(&abi);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

#[test]
fn propose_rejects_invalid_hex() {
    let (state, authority) = mk_state_and_authority();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm: Permission = CanProposeContractDeployment {
        contract_id: "demo.contract".into(),
    }
    .into();
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant propose");

    let result = iroha_data_model::isi::governance::ProposeDeployContract {
        namespace: "apps".into(),
        contract_id: "demo.contract".into(),
        code_hash_hex: "zz".into(),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".into(),
        window: None,
        mode: None,
        manifest_provenance: None,
    }
    .execute(&authority, &mut stx);

    assert!(result.is_err(), "invalid hex must be rejected");
}

#[test]
fn propose_window_override_applied() {
    let (state, authority) = mk_state_and_authority();
    let header = BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm: Permission = CanProposeContractDeployment {
        contract_id: "demo.contract".into(),
    }
    .into();
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant propose");

    let min_delay = stx.gov.min_enactment_delay;
    let lower = 5 + min_delay + 7;
    let upper = lower + 3;
    let code_hex = "11".repeat(32);
    let abi_hex = canonical_abi_hex();
    iroha_data_model::isi::governance::ProposeDeployContract {
        namespace: "apps".into(),
        contract_id: "demo.contract".into(),
        code_hash_hex: code_hex.clone(),
        abi_hash_hex: abi_hex.clone(),
        abi_version: "1".into(),
        window: Some(AtWindow { lower, upper }),
        mode: None,
        manifest_provenance: None,
    }
    .execute(&authority, &mut stx)
    .expect("propose");

    let pid = compute_proposal_id("apps", "demo.contract", &code_hex, &abi_hex);
    let rid = hex::encode(pid);
    let rec = stx
        .world
        .governance_referenda()
        .get(&rid)
        .copied()
        .expect("referendum record exists");
    assert_eq!(rec.h_start, lower);
    assert_eq!(rec.h_end, upper);
}

#[test]
fn propose_window_below_min_delay_rejected() {
    let (state, authority) = mk_state_and_authority();
    let header = BlockHeader::new(nonzero!(4_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm: Permission = CanProposeContractDeployment {
        contract_id: "demo.contract".into(),
    }
    .into();
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant propose");

    let min_delay = stx.gov.min_enactment_delay;
    let lower = 4 + min_delay - 1;
    let result = iroha_data_model::isi::governance::ProposeDeployContract {
        namespace: "apps".into(),
        contract_id: "demo.contract".into(),
        code_hash_hex: "11".repeat(32),
        abi_hash_hex: canonical_abi_hex(),
        abi_version: "1".into(),
        window: Some(AtWindow {
            lower,
            upper: lower + 5,
        }),
        mode: None,
        manifest_provenance: None,
    }
    .execute(&authority, &mut stx);

    assert!(result.is_err(), "window below min delay must fail");
}

#[test]
fn enact_requires_approved_status() {
    let (state, authority) = mk_state_and_authority();
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm: Permission = CanEnactGovernance.into();
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant enact");

    let pid = [0xAB; 32];
    stx.world.governance_proposals_mut().insert(
        pid,
        iroha_core::state::GovernanceProposalRecord {
            proposer: authority.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                namespace: "apps".into(),
                contract_id: "demo.contract".into(),
                code_hash_hex: ContractCodeHash::from_hex_str(&"11".repeat(32)).expect("code hash"),
                abi_hash_hex: ContractAbiHash::from_hex_str(&canonical_abi_hex())
                    .expect("abi hash"),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: 1,
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
            pipeline: iroha_core::state::GovernancePipeline::seeded(1, None, &stx.gov),
            parliament_snapshot: None,
        },
    );

    let res = iroha_data_model::isi::governance::EnactReferendum {
        referendum_id: pid,
        preimage_hash: [0u8; 32],
        at_window: AtWindow { lower: 1, upper: 1 },
    }
    .execute(&authority, &mut stx);

    assert!(res.is_err(), "must reject enactment before approval");
}

#[test]
fn proposal_record_exposes_deploy_payload() {
    let (_state, authority) = mk_state_and_authority();
    let record = iroha_core::state::GovernanceProposalRecord {
        proposer: authority.clone(),
        kind: ProposalKind::DeployContract(DeployContractProposal {
            namespace: "apps".into(),
            contract_id: "calc.v1".into(),
            code_hash_hex: ContractCodeHash::from_hex_str(&"11".repeat(32)).expect("code hash"),
            abi_hash_hex: ContractAbiHash::from_hex_str(&canonical_abi_hex()).expect("abi hash"),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        }),
        created_height: 42,
        status: iroha_core::state::GovernanceProposalStatus::Proposed,
        pipeline: iroha_core::state::GovernancePipeline::default(),
        parliament_snapshot: None,
    };
    let payload = record
        .as_deploy_contract()
        .expect("payload must be present");
    assert_eq!(payload.namespace, "apps");
    assert_eq!(payload.contract_id, "calc.v1");
}

#[test]
fn zk_ballot_rejects_oversized_proof() {
    let (mut state, authority) = mk_state_and_authority();
    state.gov.min_bond_amount = 0;
    let mut zk_cfg = state.view().zk.clone();
    zk_cfg.preverify_max_bytes = 4;
    state.set_zk(zk_cfg);

    let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: "election-1".into(),
    }
    .into();
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant ballot permission");

    let oversized = vec![0u8; 16];
    let proof_b64 = BASE64_STD.encode(oversized);
    let res = iroha_data_model::isi::governance::CastZkBallot {
        election_id: "election-1".into(),
        proof_b64,
        public_inputs_json: "{}".into(),
    }
    .execute(&authority, &mut stx);

    assert!(res.is_err(), "oversized proofs must be rejected");
}
