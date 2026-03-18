//! Governance enactment wires manifest insertion and status update.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(unused_imports)]
#![allow(clippy::items_after_statements)]

use core::convert::TryInto;

use iroha_core::{
    kura::Kura,
    prelude::*,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
    },
    prelude::*,
    smart_contract::manifest::ContractManifest,
};
use mv::storage::StorageReadOnly;

fn mk_world_with_account() -> (State, iroha_data_model::account::AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone().to_account_id(domain_id)).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);
    (state, account_id, kp)
}

fn compute_proposal_id(
    namespace: &str,
    contract_id: &str,
    code_hex: &str,
    abi_hex: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
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
        .expect("contract_id length fits u32");
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

#[test]
#[allow(clippy::too_many_lines)]
fn enact_inserts_manifest_and_marks_enacted() {
    let (state, account_id, kp) = mk_world_with_account();
    // Block header
    let header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Grant governance permissions
    let p_enact: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::governance::CanEnactGovernance.into();
    Grant::account_permission(p_enact, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant CanEnactGovernance");
    let p_prop: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::governance::CanProposeContractDeployment {
            contract_id: "demo.contract".into(),
        }
        .into();
    Grant::account_permission(p_prop, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant CanProposeContractDeployment");
    // Propose a contract deployment to create a proposal record
    let code_hex = "aa".repeat(32);
    let code_bytes = hex::decode(&code_hex).unwrap();
    let mut code_arr = [0u8; 32];
    code_arr.copy_from_slice(&code_bytes);
    let key = iroha_crypto::Hash::prehashed(code_arr);
    let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let abi_hex = hex::encode(abi_hash_bytes);
    let manifest_provenance = ContractManifest {
        code_hash: Some(key),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash_bytes)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp)
    .provenance
    .expect("manifest should be signed");
    iroha_data_model::isi::governance::ProposeDeployContract {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        code_hash_hex: code_hex.clone(),
        abi_hash_hex: abi_hex.clone(),
        abi_version: "1".to_string(),
        window: None,
        mode: None,
        manifest_provenance: Some(manifest_provenance),
    }
    .execute(&account_id, &mut stx)
    .expect("propose");

    let pid = compute_proposal_id("apps", "demo.contract", &code_hex, &abi_hex);
    {
        let mut rec = stx
            .world
            .governance_proposals()
            .get(&pid)
            .cloned()
            .expect("proposal record present");
        rec.status = iroha_core::state::GovernanceProposalStatus::Approved;
        stx.world.governance_proposals_mut().insert(pid, rec);
    }
    // Enact
    let instr = iroha_data_model::isi::governance::EnactReferendum {
        referendum_id: pid,
        preimage_hash: [0u8; 32],
        at_window: iroha_data_model::governance::types::AtWindow { lower: 1, upper: 2 },
    };
    instr
        .execute(&account_id, &mut stx)
        .expect("enact should succeed");
    assert!(
        stx.world.contract_manifests().get(&key).is_some(),
        "manifest inserted in transaction"
    );
    stx.apply();

    // Verify manifest inserted using block world snapshot
    use iroha_core::state::WorldReadOnly;
    use mv::storage::StorageReadOnly;
    let man = block.world.contract_manifests().get(&key).cloned();
    assert!(man.is_some(), "manifest should be present");
    let man = man.unwrap();
    let abi_bytes = hex::decode(&abi_hex).unwrap();
    let mut abi_arr = [0u8; 32];
    abi_arr.copy_from_slice(&abi_bytes);
    assert_eq!(
        man.abi_hash.unwrap(),
        iroha_crypto::Hash::prehashed(abi_arr)
    );

    // Verify proposal status updated
    let p = block
        .world
        .governance_proposals()
        .get(&pid)
        .cloned()
        .unwrap();
    assert!(matches!(
        p.status,
        iroha_core::state::GovernanceProposalStatus::Enacted
    ));
    // Verify instance mapping inserted
    let inst = block
        .world
        .contract_instances()
        .get(&("apps".to_string(), "demo.contract".to_string()))
        .copied();
    assert_eq!(inst, Some(key));
}

#[test]
fn enact_rejects_on_conflicting_existing_manifest() {
    let (state, account_id, kp) = mk_world_with_account();
    let header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Grant CanEnactGovernance
    let token = iroha_executor_data_model::permission::governance::CanEnactGovernance;
    let perm: iroha_data_model::permission::Permission = token.into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant CanEnactGovernance");

    // Prepare code/abi hex and conflicting abi for preexisting manifest
    let pid = [0xCDu8; 32];
    let code_hex = "11".repeat(32);
    let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let abi_hex = hex::encode(abi_hash_bytes);
    let mut abi_conflict_bytes = abi_hash_bytes;
    abi_conflict_bytes[0] ^= 0xFF;
    let abi_hex_conflict = hex::encode(abi_conflict_bytes);

    // Seed proposal
    stx.world.governance_proposals_mut().insert(
        pid,
        iroha_core::state::GovernanceProposalRecord {
            proposer: account_id.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                namespace: "apps".to_string(),
                contract_id: "demo.contract2".to_string(),
                code_hash_hex: ContractCodeHash::from_hex_str(&code_hex).expect("code hash"),
                abi_hash_hex: ContractAbiHash::from_hex_str(&abi_hex).expect("abi hash"),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: 1,
            status: iroha_core::state::GovernanceProposalStatus::Approved,
            pipeline: iroha_core::state::GovernancePipeline::seeded(1, None, &stx.gov),
            parliament_snapshot: None,
        },
    );

    // Pre-insert conflicting manifest via instruction
    let code_b = hex::decode(&code_hex).unwrap();
    let mut code_arr = [0u8; 32];
    code_arr.copy_from_slice(&code_b);
    let abi_b = hex::decode(&abi_hex_conflict).unwrap();
    let mut abi_arr = [0u8; 32];
    abi_arr.copy_from_slice(&abi_b);
    let key = iroha_crypto::Hash::prehashed(code_arr);
    // Grant permission to register manifest
    let p_reg: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    Grant::account_permission(p_reg, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant CanRegisterSmartContractCode");
    let man = iroha_data_model::smart_contract::manifest::ContractManifest {
        code_hash: Some(key),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_arr)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode { manifest: man }
        .execute(&account_id, &mut stx)
        .expect("register conflicting manifest");

    // Enact should fail due to abi_hash mismatch
    let instr = iroha_data_model::isi::governance::EnactReferendum {
        referendum_id: pid,
        preimage_hash: [0u8; 32],
        at_window: iroha_data_model::governance::types::AtWindow { lower: 1, upper: 2 },
    };
    let err = instr.execute(&account_id, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("existing manifest abi_hash mismatch"));
}

#[test]
fn propose_rejects_mismatched_abi_hash() {
    let (state, account_id, _kp) = mk_world_with_account();
    let header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::governance::CanProposeContractDeployment {
            contract_id: "demo.contract".into(),
        }
        .into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant CanProposeContractDeployment");

    let code_hex = "aa".repeat(32);
    // Intentionally mismatched abi hash (not canonical for v1)
    let abi_hex = "bb".repeat(32);
    let res = iroha_data_model::isi::governance::ProposeDeployContract {
        namespace: "apps".to_string(),
        contract_id: "demo.contract".to_string(),
        code_hash_hex: code_hex,
        abi_hash_hex: abi_hex,
        abi_version: "1".to_string(),
        window: None,
        mode: None,
        manifest_provenance: None,
    }
    .execute(&account_id, &mut stx);
    assert!(res.is_err(), "proposal with mismatched abi hash must fail");
    let err = res.unwrap_err();
    match err {
        iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(msg),
        ) => {
            assert!(
                msg.contains("abi_hash does not match canonical hash"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
