use iroha_crypto::Hash;
use ivm::{CoreHost, IVM, Memory, PointerType, encoding, instruction::wide, syscalls};

fn assemble(code: &[u8]) -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 2,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let mut out = meta.encode();
    out.extend_from_slice(code);
    out
}

fn encode_prog_syscall(num: u32) -> Vec<u8> {
    let scall = wide::system::SCALL;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&encoding::wide::encode_sys(scall, num as u8).to_le_bytes());
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    assemble(&bytes)
}

fn make_tlv(type_id: u16, version: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(version);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn set_account_detail_validates_tlvs() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    // Prepare TLVs in INPUT
    let acc = make_tlv(PointerType::AccountId as u16, 1, b"alice@wonderland");
    let key = make_tlv(PointerType::Name as u16, 1, b"cursor");
    let val = make_tlv(PointerType::Json as u16, 1, br#"{"k":1}"#);
    let mut off = 0u64;
    vm.memory.preload_input(off, &acc).expect("preload input");
    let p_acc = Memory::INPUT_START + off;
    off += acc.len() as u64 + 16; // pad between items
    vm.memory.preload_input(off, &key).expect("preload input");
    let p_key = Memory::INPUT_START + off;
    off += key.len() as u64 + 16;
    vm.memory.preload_input(off, &val).expect("preload input");
    let p_val = Memory::INPUT_START + off;

    vm.set_register(10, p_acc);
    vm.set_register(11, p_key);
    vm.set_register(12, p_val);

    let prog = encode_prog_syscall(syscalls::SYSCALL_SET_ACCOUNT_DETAIL);
    vm.load_program(&prog).unwrap();
    vm.run().expect("set_account_detail tlvs should validate");
}

#[test]
fn set_account_detail_rejects_wrong_type() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    // Place AccountId in r11 (expected Name)
    let acc = make_tlv(PointerType::AccountId as u16, 1, b"alice@wonderland");
    vm.memory.preload_input(0, &acc).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START); // wrong type here
    // Minimal JSON for r12
    let json = make_tlv(PointerType::Json as u16, 1, br#"{}"#);
    vm.memory
        .preload_input(acc.len() as u64 + 16, &json)
        .expect("preload input");
    vm.set_register(12, Memory::INPUT_START + acc.len() as u64 + 16);

    let prog = encode_prog_syscall(syscalls::SYSCALL_SET_ACCOUNT_DETAIL);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn nft_mint_asset_validates_tlvs() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let nft = make_tlv(PointerType::NftId as u16, 1, b"rose:uuid:0123");
    vm.memory.preload_input(0, &nft).expect("preload input");
    let acc = make_tlv(PointerType::AccountId as u16, 1, b"alice@wonderland");
    vm.memory
        .preload_input(nft.len() as u64 + 8, &acc)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + nft.len() as u64 + 8);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_MINT_ASSET);
    vm.load_program(&prog).unwrap();
    vm.run().expect("nft_mint_asset tlvs should validate");
}

#[test]
fn transfer_asset_validates_tlvs() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let from = make_tlv(PointerType::AccountId as u16, 1, b"alice@wonderland");
    vm.memory.preload_input(0, &from).expect("preload input");
    let to = make_tlv(PointerType::AccountId as u16, 1, b"bob@wonderland");
    vm.memory
        .preload_input(from.len() as u64 + 8, &to)
        .expect("preload input");
    let asset = make_tlv(PointerType::AssetDefinitionId as u16, 1, b"rose#wonderland");
    vm.memory
        .preload_input(from.len() as u64 + to.len() as u64 + 16, &asset)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + from.len() as u64 + 8);
    vm.set_register(
        12,
        Memory::INPUT_START + from.len() as u64 + to.len() as u64 + 16,
    );
    vm.set_register(13, 42);
    let prog = encode_prog_syscall(syscalls::SYSCALL_TRANSFER_ASSET);
    vm.load_program(&prog).unwrap();
    vm.run().expect("transfer_asset tlvs should validate");
}

#[test]
fn transfer_asset_rejects_wrong_asset_type() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    // Put Name instead of AssetDefinitionId in r12
    let from = make_tlv(PointerType::AccountId as u16, 1, b"alice@wonderland");
    let to = make_tlv(PointerType::AccountId as u16, 1, b"bob@wonderland");
    let wrong = make_tlv(PointerType::Name as u16, 1, b"not-an-asset-id");
    vm.memory.preload_input(0, &from).expect("preload input");
    vm.memory
        .preload_input(from.len() as u64 + 8, &to)
        .expect("preload input");
    vm.memory
        .preload_input(from.len() as u64 + to.len() as u64 + 16, &wrong)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + from.len() as u64 + 8);
    vm.set_register(
        12,
        Memory::INPUT_START + from.len() as u64 + to.len() as u64 + 16,
    );
    vm.set_register(13, 1);
    let prog = encode_prog_syscall(syscalls::SYSCALL_TRANSFER_ASSET);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn nft_set_metadata_validates_tlvs() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let nft = make_tlv(PointerType::NftId as u16, 1, b"rose:uuid:0123");
    vm.memory.preload_input(0, &nft).expect("preload input");
    let json = make_tlv(PointerType::Json as u16, 1, br#"{"k":"v"}"#);
    vm.memory
        .preload_input(nft.len() as u64 + 8, &json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + nft.len() as u64 + 8);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_SET_METADATA);
    vm.load_program(&prog).unwrap();
    vm.run().expect("nft_set_metadata tlvs should validate");
}

#[test]
fn nft_set_metadata_rejects_wrong_type() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let nft = make_tlv(PointerType::NftId as u16, 1, b"rose:uuid:0123");
    vm.memory.preload_input(0, &nft).expect("preload input");
    // Put Name instead of Json in r11
    let wrong = make_tlv(PointerType::Name as u16, 1, b"not-json");
    vm.memory
        .preload_input(nft.len() as u64 + 8, &wrong)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + nft.len() as u64 + 8);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_SET_METADATA);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn nft_burn_asset_validates_tlv() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let nft = make_tlv(PointerType::NftId as u16, 1, b"rose:uuid:deadbeef");
    vm.memory.preload_input(0, &nft).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_BURN_ASSET);
    vm.load_program(&prog).unwrap();
    vm.run().expect("nft_burn_asset tlv should validate");
}

#[test]
fn nft_burn_asset_rejects_wrong_type() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    // Put AccountId instead of NftId in r10
    let wrong = make_tlv(PointerType::AccountId as u16, 1, b"alice@wonderland");
    vm.memory.preload_input(0, &wrong).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_BURN_ASSET);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn nft_burn_asset_rejects_name_type() {
    // Pass &Name where &NftId is required
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let wrong = make_tlv(PointerType::Name as u16, 1, b"not-an-nft-id");
    vm.memory.preload_input(0, &wrong).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_BURN_ASSET);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn nft_burn_asset_rejects_blob_type() {
    // Pass &Blob where &NftId is required
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let wrong = make_tlv(PointerType::Blob as u16, 1, b"opaque-bytes");
    vm.memory.preload_input(0, &wrong).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog = encode_prog_syscall(syscalls::SYSCALL_NFT_BURN_ASSET);
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}
