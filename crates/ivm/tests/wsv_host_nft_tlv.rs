use std::collections::HashMap;

use iroha_crypto::Hash;
use ivm::{
    IVM, Memory, PointerType,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let payload = PointerType::from_u16(type_id)
        .map(|pty| common::payload_for_type(pty, payload))
        .unwrap_or_else(|| payload.to_vec());
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    let h: [u8; 32] = Hash::new(&payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn create_transfer_set_nft_with_tlv() {
    let alice: AccountId =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774@domain"
            .parse()
            .unwrap();
    let bob: AccountId =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4@domain"
            .parse()
            .unwrap();
    let carol: AccountId =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4@wonder"
            .parse()
            .unwrap();

    let mut wsv = MockWorldStateView::new();
    // Register accounts in mock
    wsv.add_account_unchecked(alice.clone());
    wsv.add_account_unchecked(bob.clone());
    wsv.add_account_unchecked(carol.clone());
    let host = WsvHost::new(wsv, alice.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Create NFT nft0 for alice
    let nft0 = "rose:uuid:0000$domain";
    let tlv_nft = make_tlv(PointerType::NftId as u16, nft0.as_bytes());
    let tlv_owner = make_tlv(PointerType::AccountId as u16, alice.to_string().as_bytes());
    vm.memory.preload_input(0, &tlv_nft).expect("preload input");
    vm.memory
        .preload_input(tlv_nft.len() as u64 + 8, &tlv_owner)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_nft.len() as u64 + 8);
    let prog_create = assemble_syscalls(&[syscalls::SYSCALL_NFT_MINT_ASSET as u8]);
    vm.load_program(&prog_create).unwrap();
    vm.run().expect("create nft via tlv failed");

    // Set NFT data as owner
    let tlv_nft = make_tlv(PointerType::NftId as u16, nft0.as_bytes());
    let tlv_json = make_tlv(PointerType::Json as u16, br#"{"k":"v"}"#);
    vm.memory.preload_input(0, &tlv_nft).expect("preload input");
    vm.memory
        .preload_input(tlv_nft.len() as u64 + 8, &tlv_json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_nft.len() as u64 + 8);
    let prog_set = assemble_syscalls(&[syscalls::SYSCALL_NFT_SET_METADATA as u8]);
    vm.load_program(&prog_set).unwrap();
    vm.run().expect("set nft data via tlv failed");

    // Transfer NFT to bob (caller=alice)
    let tlv_from = make_tlv(PointerType::AccountId as u16, alice.to_string().as_bytes());
    let tlv_nft = make_tlv(PointerType::NftId as u16, nft0.as_bytes());
    let tlv_to = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    vm.memory
        .preload_input(0, &tlv_from)
        .expect("preload input");
    vm.memory
        .preload_input(tlv_from.len() as u64 + 8, &tlv_nft)
        .expect("preload input");
    vm.memory
        .preload_input(tlv_from.len() as u64 + tlv_nft.len() as u64 + 16, &tlv_to)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_from.len() as u64 + 8);
    vm.set_register(
        12,
        Memory::INPUT_START + tlv_from.len() as u64 + tlv_nft.len() as u64 + 16,
    );
    let prog_xfer = assemble_syscalls(&[syscalls::SYSCALL_NFT_TRANSFER_ASSET as u8]);
    vm.load_program(&prog_xfer).unwrap();
    vm.run().expect("transfer nft via tlv failed");

    // Switch caller to an unrelated account before trying to mutate metadata again
    if let Some(any) = vm.host_mut_any() {
        let host = any.downcast_mut::<WsvHost>().expect("downcast WsvHost");
        host.caller = carol.clone();
    }

    // Set NFT data as non-owner/non-issuer should now fail (caller=carol, owner=bob, issuer=alice)
    let tlv_nft = make_tlv(PointerType::NftId as u16, nft0.as_bytes());
    let tlv_json = make_tlv(PointerType::Json as u16, br#"{"k":"v2"}"#);
    vm.memory.preload_input(0, &tlv_nft).expect("preload input");
    vm.memory
        .preload_input(tlv_nft.len() as u64 + 8, &tlv_json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_nft.len() as u64 + 8);
    vm.load_program(&prog_set).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));
}
