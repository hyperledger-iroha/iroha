//! `CoreHost`: verify that the host binds the last verify envelope hash
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! to subsequently enqueued ZK ISIs via the vendor bridge.

use std::sync::Arc;

use iroha_core::smartcontracts::ivm::host::CoreHost;
use iroha_data_model::{
    isi::InstructionBox,
    prelude::*,
    proof::{ProofBox, VerifyingKeyBox},
};
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, IVMHost, PointerType, ProgramMetadata, syscalls as ivm_sys};
use norito::to_bytes;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("payload length fits in u32")
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn store_tlv(vm: &mut IVM, tlv: &[u8]) -> u64 {
    vm.alloc_input_tlv(tlv).expect("write TLV into INPUT")
}

#[test]
fn envelope_hash_is_injected_into_enqueued_unshield() {
    // Authority and host
    let authority: AccountId = ALICE_ID.clone();
    let mut vm = IVM::new(0);
    let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
    // Seed the pending envelope hash that the vendor bridge injects into Unshield proofs.
    let expected_hash: [u8; 32] = [0x11; 32];
    host.__test_set_last_env_hash_unshield(expected_hash);
    let metadata = ProgramMetadata {
        abi_version: 1,
        ..ProgramMetadata::default()
    };
    vm.load_program(&metadata.encode()).expect("load metadata");

    // Build an Unshield instruction and pass via the vendor bridge
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let unshield = iroha_data_model::isi::zk::Unshield {
        asset,
        to: authority.clone(),
        public_amount: 5u128,
        inputs: vec![[0u8; 32]],
        proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x01, 0x02, 0x03]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
        ),
        root_hint: None,
    };
    let payload = to_bytes(&InstructionBox::from(unshield)).expect("encode instruction");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    let ptr = store_tlv(&mut vm, &tlv);
    vm.set_register(10, ptr);

    // Directly invoke the vendor bridge syscall and inspect queued instructions.
    host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("vendor bridge syscall succeeds");
    let executed = host.drain_instructions();

    // The executed instruction should carry the injected envelope_hash
    assert_eq!(executed.len(), 1);
    if let Some(any_ref) = executed[0]
        .as_any()
        .downcast_ref::<iroha_data_model::isi::zk::Unshield>()
    {
        assert_eq!(any_ref.proof.envelope_hash, Some(expected_hash));
    } else {
        panic!("expected Unshield instruction variant");
    }
}
