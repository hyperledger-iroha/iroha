use std::collections::HashMap;

use iroha_crypto::{Hash, PublicKey};
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, Memory, PointerType,
    mock_wsv::{AssetDefinitionId, DomainId, MockWorldStateView, ScopedAccountId, WsvHost},
    syscalls,
};
use norito::to_bytes;

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let payload = PointerType::from_u16(type_id)
        .map(|pty| common::payload_for_type(pty, payload))
        .unwrap_or_else(|| payload.to_vec());
    make_raw_tlv(type_id, &payload)
}

fn make_raw_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn account(domain: &str, public_key: &str) -> ScopedAccountId {
    let domain: DomainId = domain.parse().unwrap();
    let public_key: PublicKey = public_key.parse().unwrap();
    ScopedAccountId::new(domain, public_key)
}

fn make_account_tlv(account: &ScopedAccountId) -> Vec<u8> {
    let buf = to_bytes(account).expect("encode account into Norito");
    make_tlv(PointerType::AccountId as u16, &buf)
}

#[test]
fn grant_revoke_permission_with_tlv() {
    // alice has the balance; bob is the subject for grant/revoke
    let alice_input = account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    );
    let alice_literal_value = norito::json::to_value(&alice_input).expect("serialize account id");
    let alice_literal = alice_literal_value
        .as_str()
        .expect("account id string")
        .to_owned();
    let alice =
        ScopedAccountId::parse_encoded(&alice_literal).expect("canonical account id must parse");
    let bob = account(
        "wonderland",
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
    );
    let asset: AssetDefinitionId = "asset#wonderland".parse().unwrap();

    let wsv = MockWorldStateView::with_balances(&[(
        (alice.clone(), asset.clone()),
        Numeric::from(50_u64),
    )]);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&bob.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Step 1: grant ReadAccountAssets(alice) to bob via Name TLV
    let subj = make_account_tlv(&bob);
    vm.memory.preload_input(0, &subj).expect("preload input");
    let perm_name = make_raw_tlv(
        PointerType::Name as u16,
        format!("read_assets:{alice_literal}").as_bytes(),
    );
    vm.memory
        .preload_input(subj.len() as u64 + 8, &perm_name)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + subj.len() as u64 + 8);
    let prog_grant = assemble_syscalls(&[syscalls::SYSCALL_GRANT_PERMISSION as u8]);
    vm.load_program(&prog_grant).unwrap();
    vm.run().expect("grant permission failed");

    // Step 2: verify balance with granted permission via TLVs for alice & asset
    let acc = make_account_tlv(&alice);
    vm.memory.preload_input(0, &acc).expect("preload input");
    let asset_tlv = make_tlv(
        PointerType::AssetDefinitionId as u16,
        asset.to_string().as_bytes(),
    );
    vm.memory
        .preload_input(acc.len() as u64 + 8, &asset_tlv)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acc.len() as u64 + 8);
    let prog_bal = assemble_syscalls(&[syscalls::SYSCALL_GET_ACCOUNT_BALANCE as u8]);
    vm.load_program(&prog_bal).unwrap();
    vm.run().expect("balance after grant failed");
    let tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("balance tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Numeric = norito::decode_from_bytes(tlv.payload).expect("decode balance");
    assert_eq!(value, Numeric::from(50_u64));

    // Step 3: revoke the same permission via Json TLV
    let subj = make_account_tlv(&bob);
    let perm_json = make_tlv(
        PointerType::Json as u16,
        format!("{{\"type\":\"read_assets\",\"target\":\"{alice_literal}\"}}").as_bytes(),
    );
    vm.memory.preload_input(0, &subj).expect("preload input");
    vm.memory
        .preload_input(subj.len() as u64 + 8, &perm_json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + subj.len() as u64 + 8);
    let prog_revoke = assemble_syscalls(&[syscalls::SYSCALL_REVOKE_PERMISSION as u8]);
    vm.load_program(&prog_revoke).unwrap();
    vm.run().expect("revoke permission failed");

    // Step 4: balance call should now fail (no permission)
    let acc = make_account_tlv(&alice);
    vm.memory.preload_input(0, &acc).expect("preload input");
    let asset_tlv = make_tlv(
        PointerType::AssetDefinitionId as u16,
        asset.to_string().as_bytes(),
    );
    vm.memory
        .preload_input(acc.len() as u64 + 8, &asset_tlv)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acc.len() as u64 + 8);
    vm.load_program(&prog_bal).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));
}
