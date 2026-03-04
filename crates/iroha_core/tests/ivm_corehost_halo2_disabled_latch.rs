//! Ensure `CoreHost` threads `iroha_config.zk.halo2.enabled=false` to the underlying
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! `DefaultHost` and does not set ZK verification latches on verify calls.

#![cfg(feature = "zk-tests")]
#![cfg(feature = "zk-ipa-native")]

use std::sync::Arc;

use iroha_config::parameters::defaults;
use iroha_core::{
    kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::host::CoreHost, state::State,
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_data_model::{
    isi::InstructionBox,
    prelude::*,
    proof::{ProofBox, VerifyingKeyBox},
};
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, IVMHost, PointerType, ProgramMetadata, syscalls as ivm_sys};
use nonzero_ext::nonzero;
use norito::to_bytes;

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

fn store_tlv(vm: &mut IVM, tlv: &[u8]) -> u64 {
    vm.alloc_input_tlv(tlv).expect("write TLV into INPUT")
}

#[test]
fn halo2_disabled_verify_does_not_set_latch_and_gates_isi() {
    // Minimal node state (unused, but preserves test pattern for executor staging)
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
    let mut vm = IVM::new(100_000);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    host.set_halo2_config(&iroha_config::parameters::actual::Halo2 {
        enabled: false, // disabled
        curve: iroha_config::parameters::actual::ZkCurve::Pallas,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        verifier_worker_threads: defaults::zk::halo2::VERIFIER_WORKER_THREADS,
        verifier_queue_cap: defaults::zk::halo2::VERIFIER_QUEUE_CAP,
        verifier_enqueue_wait_ms: defaults::zk::halo2::VERIFIER_ENQUEUE_WAIT_MS,
        verifier_retry_ring_cap: defaults::zk::halo2::VERIFIER_RETRY_RING_CAP,
        verifier_retry_max_attempts: defaults::zk::halo2::VERIFIER_RETRY_MAX_ATTEMPTS,
        verifier_retry_tick_ms: defaults::zk::halo2::VERIFIER_RETRY_TICK_MS,
        max_envelope_bytes: defaults::zk::halo2::MAX_ENVELOPE_BYTES,
        max_proof_bytes: defaults::zk::halo2::MAX_PROOF_BYTES,
        max_transcript_label_len: defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN,
        enforce_transcript_label_ascii: defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII,
    });

    let meta = ProgramMetadata {
        abi_version: 1,
        ..ProgramMetadata::default()
    };
    vm.load_program(&meta.encode()).expect("load metadata");

    // Prepare a ZK_VERIFY_TRANSFER envelope TLV with a compact mock payload.
    let env = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]).proof_bytes;
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &env);
    let ptr_verify = store_tlv(&mut vm, &tlv);
    vm.memory
        .validate_tlv(ptr_verify)
        .expect("verify TLV readable");
    vm.set_register(10, ptr_verify);
    host.syscall(ivm_sys::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("verify syscall");
    assert_eq!(vm.register(10), 0, "verify must return 0 when disabled");

    // Now enqueue an Unshield via vendor syscall and ensure apply_queued rejects
    let asset: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let unshield = iroha_data_model::isi::zk::Unshield {
        asset,
        to: authority.clone(),
        public_amount: 1u128,
        inputs: vec![[0u8; 32]],
        proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xAA, 0xBB]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
        ),
        root_hint: None,
    };
    let payload = to_bytes(&InstructionBox::from(unshield)).expect("encode unshield");
    let tlv2 = make_tlv(PointerType::NoritoBytes as u16, &payload);
    let ptr_unshield = store_tlv(&mut vm, &tlv2);
    vm.memory
        .validate_tlv(ptr_unshield)
        .expect("unshield TLV readable");
    vm.set_register(10, ptr_unshield);
    host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("enqueue unshield via vendor syscall");
    let err = host
        .apply_queued(&mut stx, &authority)
        .expect_err("Unshield must be gated when verify returned 0");
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
