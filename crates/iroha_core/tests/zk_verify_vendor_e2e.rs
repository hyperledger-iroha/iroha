#![doc = "End-to-end vendor bridge gating path for ZK verification"]
#![cfg(feature = "zk-tests")]
//! End-to-end gating path: ZK verify (mocked) -> vendor bridge -> `CoreHost` gating.
//!
//! This test avoids IPA math by forcing the verification flag on `CoreHost`
//! via test-only helpers. It demonstrates the expected gating behavior when
//! a contract enqueues a ZK ISI via the vendor bridge after a prior verify.

use std::sync::Arc;

use iroha_core::{
    kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::host::CoreHost, state::State,
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_crypto::Hash;
use iroha_data_model::{isi::BuiltInInstruction, prelude::*};
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, PointerType, encoding, instruction, syscalls as ivm_sys};
use nonzero_ext::nonzero;

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

#[test]
fn ballot_verify_then_vendor_bridge_gated_ok_when_flag_forced() {
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
    let mut vm = IVM::new(0);
    let host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    vm.set_host(host);

    // Build a Norito-encoded SubmitBallot instruction (dummy payload)
    let ballot_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let ballot_vk = ballot_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let sb = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "election1".to_string(),
        ciphertext: vec![0u8; 8],
        ballot_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ballot_fixture.proof_box("halo2/ipa"),
            ballot_vk,
        ),
        nullifier: [7u8; 32],
    };
    let sb_bytes = sb.encode_as_instruction_box();
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &sb_bytes);
    let mut cursor = 0;
    let ptr = store_tlv(&mut vm, &mut cursor, &tlv);
    vm.set_register(10, ptr);

    // Program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    let scall = instruction::wide::system::SCALL;
    let sys = u8::try_from(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION).unwrap();
    code.extend_from_slice(&encoding::wide::encode_sys(scall, sys).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    // Program header: magic + header + code (simple)
    let mut prog = Vec::new();
    prog.extend_from_slice(b"IVM\0");
    prog.extend_from_slice(&[1, 0, 0, 4]); // version 1.0, mode=0, vector_length=4
    prog.extend_from_slice(&0u64.to_le_bytes()); // gas
    prog.push(1); // abi_version=1
    prog.push(0); // vector len
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();

    // Run once — without verify flag, apply should be rejected
    let env_hash: [u8; 32] = Hash::new(&ballot_fixture.proof_bytes).into();
    vm.run().unwrap();
    CoreHost::with_host(&mut vm, |host| {
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
    });

    // Re-enqueue SubmitBallot via the vendor bridge and expect success.
    vm.set_register(10, ptr);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let applied = CoreHost::with_host(&mut vm, |host| host.apply_queued(&mut stx, &authority))
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
