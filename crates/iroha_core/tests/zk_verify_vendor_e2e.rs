#![doc = "End-to-end vendor bridge gating path for ZK verification"]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
#![cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
//! End-to-end gating path: ZK verify (mocked) -> vendor bridge -> `CoreHost` gating.
//!
//! This test avoids IPA math by forcing the verification flag on `CoreHost`
//! via test-only helpers. It demonstrates the expected gating behavior when
//! a contract enqueues a ZK ISI via the vendor bridge after a prior verify.
use iroha_core::smartcontracts::Execute;
use iroha_core::{
    kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::host::CoreHost, state::State,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::Account,
    asset::AssetDefinition,
    domain::Domain,
    isi::{
        smart_contract_code::{
            ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
        verifying_keys,
    },
    permission::Permission,
    prelude::*,
};
use iroha_executor_data_model::permission::governance::{
    CanManageParliament, CanSubmitGovernanceBallot,
};
use iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use ivm::{IVM, PointerType, host::IVMHost, syscalls as ivm_sys};
use nonzero_ext::nonzero;
use std::sync::Arc;
fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}
fn store_tlv(vm: &mut IVM, cursor: &mut u64, tlv: &[u8]) -> u64 {
    vm.memory
        .input_write_aligned(cursor, tlv, 8)
        .expect("write TLV into INPUT")
}
fn derive_ballot_nullifier(
    domain_tag: &str,
    network_id: &iroha_data_model::NetworkId,
    election_id: &str,
    commit: &[u8; 32],
) -> [u8; 32] {
    use blake2::{Blake2b512, Digest as _};
    fn push_len(buf: &mut Vec<u8>, len: usize) {
        let len_u64 = len as u64;
        buf.extend_from_slice(&len_u64.to_le_bytes());
    }
    let mut input = Vec::with_capacity(
        domain_tag.len() + network_id.as_bytes().len() + election_id.len() + commit.len() + 24,
    );
    push_len(&mut input, domain_tag.len());
    input.extend_from_slice(domain_tag.as_bytes());
    push_len(&mut input, network_id.as_bytes().len());
    input.extend_from_slice(network_id.as_bytes());
    push_len(&mut input, election_id.len());
    input.extend_from_slice(election_id.as_bytes());
    input.extend_from_slice(commit);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}
#[test]
#[allow(clippy::too_many_lines)]
fn ballot_verify_then_vendor_bridge_gated_ok_when_flag_forced() {
    // Minimal state
    let authority: AccountId = ALICE_ID.clone();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = iroha_core::state::World::with([domain], [account], Vec::<AssetDefinition>::new());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state.zk.halo2.enabled = true;
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    // Authority and host
    let mut vm = IVM::new(10_000_000);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    let ballot_bundle = super::zk_testkit::vote_merkle8_bundle();
    let vk_commitment = ballot_bundle.vk_record.commitment;
    let vk_id = ballot_bundle.vk_id.clone();
    let vk_record = ballot_bundle.vk_record.clone();
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    let perm_parliament: Permission = CanManageParliament.into();
    let perm_submit: Permission = CanSubmitGovernanceBallot {
        referendum_id: "election1".to_string(),
    }
    .into();
    Grant::account_permission(perm_vk, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant vk permission");
    Grant::account_permission(perm_parliament, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant parliament permission");
    Grant::account_permission(perm_submit, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant submit ballot permission");
    let contract_call_permission =
        Permission::new("CanUseVendorBridgeTest".to_owned(), Json::new(()));
    Grant::account_permission(contract_call_permission, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant vendor-bridge contract permission");
    let lifecycle_permission: Permission = CanRegisterSmartContractCode.into();
    Grant::account_permission(lifecycle_permission, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant contract lifecycle permission");
    let (contract_program, _) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku VendorBridgeGate {
    kotoage fn execute(bytes instruction) authorize("CanUseVendorBridgeTest") {
        ledger::governance::submit_ballot(instruction);
    }
}
"#,
        )
        .expect("compile admitted vendor-bridge contract");
    let verified_contract =
        ivm::verify_contract_artifact(&contract_program).expect("verify vendor-bridge contract");
    let contract_code_hash = verified_contract.code_hash;
    RegisterSmartContractBytes {
        code_hash: contract_code_hash,
        code: contract_program.clone(),
    }
    .execute(&authority, &mut stx)
    .expect("register vendor-bridge contract bytes");
    RegisterSmartContractCode {
        manifest: verified_contract.manifest.signed(&ALICE_KEYPAIR),
    }
    .execute(&authority, &mut stx)
    .expect("register vendor-bridge contract manifest");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive vendor-bridge contract address");
    Register::account(Account::new(contract_address.subject_id()))
        .execute(&authority, &mut stx)
        .expect("register vendor-bridge contract subject");
    let contract_submit_permission: Permission = CanSubmitGovernanceBallot {
        referendum_id: "election1".to_owned(),
    }
    .into();
    Grant::account_permission(contract_submit_permission, contract_address.subject_id())
        .execute(&authority, &mut stx)
        .expect("grant ballot submission to the contract effect authority");
    ActivateContractInstance {
        contract_address: contract_address.clone(),
        expected_revision: 1,
        code_hash: contract_code_hash,
    }
    .execute(&authority, &mut stx)
    .expect("activate vendor-bridge contract");
    let prepared_contract = ivm::prepare_contract(Arc::<[u8]>::from(contract_program))
        .expect("prepare vendor-bridge contract");
    host.bind_authorized_deployed_contract_runtime_context(
        &stx,
        &contract_address,
        None,
        &prepared_contract,
        "execute",
    )
    .expect("bind admitted vendor-bridge contract");
    verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record,
    }
    .execute(&authority, &mut stx)
    .expect("register vk");
    let commit_bytes = ballot_bundle.commit_bytes();
    let root_bytes = ballot_bundle.root_bytes();
    // Seed the already-created election so this fixture remains focused on the
    // vendor-bridge verification latch.
    stx.world.elections_mut().insert(
        "election1".to_owned(),
        iroha_core::state::ElectionState {
            options: 1,
            eligible_root: root_bytes,
            start_ts: 0,
            end_ts: 0,
            finalized: false,
            tally: vec![0],
            ballot_nullifiers: std::collections::BTreeSet::new(),
            ciphertexts: Vec::new(),
            vk_ballot: Some(vk_id.clone()),
            vk_ballot_commitment: Some(vk_commitment),
            vk_tally: Some(vk_id.clone()),
            vk_tally_commitment: Some(vk_commitment),
            domain_tag: "zkvote".to_owned(),
        },
    );
    // Build a Norito-encoded SubmitBallot instruction (valid payload)
    let nullifier =
        derive_ballot_nullifier("zkvote", &state.network_id, "election1", &commit_bytes);
    let sb = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "election1".to_string(),
        ciphertext: commit_bytes.to_vec(),
        ballot_proof: iroha_data_model::proof::ProofAttachment::new_ref(
            ballot_bundle.backend.into(),
            iroha_data_model::proof::ProofBox::new(
                ballot_bundle.backend.into(),
                ballot_bundle.proof_bytes.clone(),
            ),
            vk_id.clone(),
        ),
        nullifier,
    };
    let sb_bytes = norito::to_bytes(&InstructionBox::from(sb))
        .expect("encode SubmitBallot instruction box to Norito");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &sb_bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);
    vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
    // Run once — without verify flag, apply should be rejected
    let env_hash: [u8; 32] = Hash::new(&ballot_bundle.proof_bytes).into();
    host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("queue ballot through the vendor bridge");
    let err = host
        .apply_queued(&mut stx, &authority)
        .expect_err("missing verify must reject");
    match err {
        iroha_data_model::ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("missing ZK_VOTE_VERIFY_BALLOT"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    // Seed ballot verification latch with the expected envelope hash to simulate
    // a prior successful `ZK_VOTE_VERIFY_BALLOT`.
    host.__test_seed_ballot_latch(env_hash);
    // Re-enqueue SubmitBallot via the vendor bridge and expect success.
    vm.set_register(10, ptr);
    host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("requeue ballot through the vendor bridge");
    let applied = host
        .apply_queued(&mut stx, &authority)
        .expect("apply queued after simulated verify");
    assert_eq!(applied.len(), 1, "expected exactly one queued instruction");
    let instr: &dyn iroha_data_model::isi::Instruction = &*applied[0];
    assert!(
        instr
            .as_any()
            .downcast_ref::<iroha_data_model::isi::zk::SubmitBallot>()
            .is_some(),
        "queued instruction should be SubmitBallot"
    );
}
