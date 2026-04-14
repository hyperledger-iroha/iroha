#![doc = "ZK verify gating tests for `CoreHost`"]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! `CoreHost` ZK verify gating tests: ensure Unshield/ZkTransfer are rejected
//! without a prior successful `ZK_VERIFY_*` syscall.

use std::sync::Arc;

use iroha_core::{
    kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::host::CoreHost, state::State,
};
use iroha_data_model::prelude::*;
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, PointerType, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};
use nonzero_ext::nonzero;

const VM_GAS_LIMIT: u64 = 10_000_000;

fn with_core_host<R>(vm: &mut IVM, f: impl FnOnce(&mut CoreHost) -> R) -> R {
    CoreHost::with_host(vm, f)
}

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&type_id.to_be_bytes());
    v.push(1);
    v.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn small_proof_attachment() -> iroha_data_model::proof::ProofAttachment {
    let proof = iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xAB; 32]);
    iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        proof,
        iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "fixture"),
    )
}

fn store_tlv(vm: &mut IVM, cursor: &mut u64, tlv: &[u8]) -> u64 {
    vm.memory
        .input_write_aligned(cursor, tlv, 8)
        .expect("write TLV into INPUT")
}

#[test]
fn unshield_without_verify_is_rejected() {
    // Minimal state
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Authority and host
    let authority: AccountId = ALICE_ID.clone();
    let mut vm = IVM::new(VM_GAS_LIMIT);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    // Set any halo2 config (not used directly in this test but ensures host is configured)
    host.set_halo2_config(&iroha_config::parameters::actual::Halo2 {
        enabled: false,
        curve: iroha_config::parameters::actual::ZkCurve::Pallas,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 16,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..Default::default()
    });
    vm.set_host(host);

    // Build a Norito-encoded Unshield instruction and pass via the vendor syscall bridge.
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let unshield = iroha_data_model::isi::zk::Unshield {
        asset,
        to: authority.clone(),
        public_amount: 1u128,
        inputs: vec![[0u8; 32]],
        proof: small_proof_attachment(),
        root_hint: None,
    };
    let bytes = norito::to_bytes(&iroha_data_model::isi::InstructionBox::from(unshield))
        .expect("encode Unshield InstructionBox to Norito");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);

    // Program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    let scall = instruction::wide::system::SCALL;
    let sys = u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION).unwrap();
    code.extend_from_slice(&encoding::wide::encode_sys(scall, sys).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();

    // Run and then attempt to apply queued ISIs; expect rejection due to missing verify
    vm.run().unwrap();
    let err = with_core_host(&mut vm, |host| host.apply_queued(&mut stx, &authority))
        .expect_err("unshield must be gated by verify");
    match err {
        iroha_data_model::ValidationFail::NotPermitted(msg) => {
            assert!(
                msg.contains("missing ZK_VERIFY_UNSHIELD")
                    || msg.contains("missing ZK_VERIFY_TRANSFER")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn zktransfer_without_verify_is_rejected() {
    // Minimal state
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let authority: AccountId = ALICE_ID.clone();
    let mut vm = IVM::new(VM_GAS_LIMIT);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    host.set_halo2_config(&iroha_config::parameters::actual::Halo2 {
        enabled: true,
        curve: iroha_config::parameters::actual::ZkCurve::Pallas,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 16,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..Default::default()
    });
    vm.set_host(host);

    // Build ZkTransfer instruction
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "gold".parse().unwrap(),
    );
    let zkt = iroha_data_model::isi::zk::ZkTransfer {
        asset,
        inputs: vec![[1u8; 32]],
        outputs: vec![[2u8; 32]],
        proof: small_proof_attachment(),
        root_hint: None,
    };
    let bytes = norito::to_bytes(&iroha_data_model::isi::InstructionBox::from(zkt))
        .expect("encode ZkTransfer InstructionBox to Norito");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);

    // Program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    let scall = instruction::wide::system::SCALL;
    let sys = u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION).unwrap();
    code.extend_from_slice(&encoding::wide::encode_sys(scall, sys).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();

    vm.run().unwrap();
    let err = with_core_host(&mut vm, |host| host.apply_queued(&mut stx, &authority))
        .expect_err("zktranfer must be gated by verify");
    match err {
        iroha_data_model::ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("missing ZK_VERIFY_TRANSFER"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn submit_ballot_without_verify_is_rejected() {
    // Minimal state
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let authority: AccountId = ALICE_ID.clone();
    let mut vm = IVM::new(VM_GAS_LIMIT);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    host.set_halo2_config(&iroha_config::parameters::actual::Halo2 {
        enabled: true,
        curve: iroha_config::parameters::actual::ZkCurve::Pallas,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 16,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..Default::default()
    });
    vm.set_host(host);

    // Build SubmitBallot instruction
    let sb = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "election1".to_string(),
        ciphertext: vec![0u8; 32],
        ballot_proof: small_proof_attachment(),
        nullifier: [1u8; 32],
    };
    let bytes = norito::to_bytes(&iroha_data_model::isi::InstructionBox::from(sb))
        .expect("encode SubmitBallot InstructionBox to Norito");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);

    // Program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    let scall = instruction::wide::system::SCALL;
    let sys = u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION).unwrap();
    code.extend_from_slice(&encoding::wide::encode_sys(scall, sys).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();

    vm.run().unwrap();
    let err = with_core_host(&mut vm, |host| host.apply_queued(&mut stx, &authority))
        .expect_err("SubmitBallot must be gated by verify");
    match err {
        iroha_data_model::ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("missing ZK_VOTE_VERIFY_BALLOT"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn finalize_election_without_verify_is_rejected() {
    // Minimal state
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura, query);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let authority: AccountId = ALICE_ID.clone();
    let mut vm = IVM::new(VM_GAS_LIMIT);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    host.set_halo2_config(&iroha_config::parameters::actual::Halo2 {
        enabled: true,
        curve: iroha_config::parameters::actual::ZkCurve::Pallas,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 16,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..Default::default()
    });
    vm.set_host(host);

    // Build FinalizeElection instruction
    let fin = iroha_data_model::isi::zk::FinalizeElection {
        election_id: "election1".to_string(),
        tally: vec![1, 0, 0],
        tally_proof: small_proof_attachment(),
    };
    let bytes = norito::to_bytes(&iroha_data_model::isi::InstructionBox::from(fin))
        .expect("encode FinalizeElection InstructionBox to Norito");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);

    // Program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION)
                .expect("syscall id fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();

    vm.run().unwrap();
    let err = with_core_host(&mut vm, |host| host.apply_queued(&mut stx, &authority))
        .expect_err("FinalizeElection must be gated by verify");
    match err {
        iroha_data_model::ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("missing ZK_VOTE_VERIFY_TALLY"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
