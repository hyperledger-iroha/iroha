//! Admission-time rejection when on-chain manifest `abi_hash` mismatches node policy.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use core::str::FromStr;
use std::borrow::Cow;

use iroha_core::smartcontracts::Execute; // bring trait for `.execute()` on ISIs
use iroha_core::{
    prelude::World, smartcontracts::ivm::cache::IvmCache, state::State,
    tx::TransactionRejectionReason,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    executor::{IvmAdmissionError, ValidationFail},
    metadata::Metadata,
    prelude::*,
    smart_contract::manifest,
};
use iroha_primitives::json::Json;
use ivm::{ProgramMetadata, encoding};
use nonzero_ext::nonzero;

const TEST_GAS_LIMIT: u64 = 1_000_000;

fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
    // Program: HALT
    let mut code = Vec::new();
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version,
    };
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn minimal_ivm_program_with_syscall(abi_version: u8, syscall: u8) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(ivm::instruction::wide::system::SCALL, syscall).to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version,
    };
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn metadata_with_gas_limit(limit: u64) -> Metadata {
    let mut md = Metadata::default();
    let key = iroha_data_model::name::Name::from_str("gas_limit").expect("gas_limit key");
    md.insert(key, Json::new(limit));
    md
}

#[test]
fn ivm_manifest_mismatched_abi_hash_rejected_at_admission() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};
    use iroha_data_model::{
        isi::smart_contract_code::RegisterSmartContractCode,
        permission,
        transaction::{Executable, TransactionBuilder},
    };

    // Build world with a domain and an authority account
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account =
        Account::new(account_id.clone().to_account_id(domain_id.clone())).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    // Prepare a minimal IVM program and its hashes
    let prog = minimal_ivm_program(1);
    let parsed = ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
    let policy = ivm::SyscallPolicy::AbiV1;
    let correct_abi = ivm::syscalls::compute_abi_hash(policy);
    let mut wrong_abi = correct_abi;
    wrong_abi[0] ^= 0x5A; // flip a byte to make it wrong

    // Block 1: grant permission and register a manifest with wrong abi_hash under the code_hash
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();

    // Grant CanRegisterSmartContractCode to the authority
    let token = iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
    let perm: permission::Permission = token.into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    // Register manifest with wrong abi_hash
    let manifest = manifest::ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&account_id, &mut stx1)
        .expect("register manifest");
    stx1.apply();
    let _ = block1.commit();

    // Block 2: submit the IVM program; admission should reject due to abi_hash mismatch
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), account_id.clone())
        .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();

    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
            IvmAdmissionError::ManifestAbiHashMismatch(info),
        ))) => {
            assert_eq!(info.expected, iroha_crypto::Hash::prehashed(wrong_abi));
            assert_eq!(info.actual, iroha_crypto::Hash::prehashed(correct_abi));
        }
        other => panic!(
            "abi_hash mismatch must be rejected at admission with structured error, got {other:?}"
        ),
    }
}

#[test]
fn ivm_manifest_matching_abi_hash_accepted_at_admission() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};
    use iroha_data_model::{
        isi::smart_contract_code::RegisterSmartContractCode,
        permission,
        transaction::{Executable, TransactionBuilder},
    };

    // Build world with a domain and an authority account
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account =
        Account::new(account_id.clone().to_account_id(domain_id.clone())).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    // Prepare a minimal IVM program and its hashes
    let prog = minimal_ivm_program(1);
    let parsed = ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
    let policy = ivm::SyscallPolicy::AbiV1;
    let correct_abi = ivm::syscalls::compute_abi_hash(policy);

    // Block 1: grant permission and register a manifest with correct abi_hash under the code_hash
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();

    // Grant permission
    let token = iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
    let perm: permission::Permission = token.into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    // Register manifest with correct abi_hash
    let manifest = manifest::ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(iroha_crypto::Hash::prehashed(correct_abi)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&account_id, &mut stx1)
        .expect("register manifest");
    stx1.apply();
    let _ = block1.commit();

    // Block 2: submit the IVM program; admission should accept due to matching abi_hash
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), account_id.clone())
        .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();

    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_ok(),
        "matching manifest abi_hash should allow admission, got {result:?}"
    );
}

#[test]
fn ivm_manifest_without_abi_hash_allows_admission() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};
    use iroha_data_model::{
        isi::smart_contract_code::RegisterSmartContractCode,
        permission,
        transaction::{Executable, TransactionBuilder},
    };

    // Build world with a domain and an authority account
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account =
        Account::new(account_id.clone().to_account_id(domain_id.clone())).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    // Prepare a minimal IVM program (v1) and its hashes
    let prog = minimal_ivm_program(1);
    let parsed = ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

    // Block 1: grant permission and register a manifest with only code_hash (no abi_hash)
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();

    // Grant permission
    let token = iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
    let perm: permission::Permission = token.into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    // Register manifest with code_hash only
    let manifest = manifest::ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: None,
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&account_id, &mut stx1)
        .expect("register manifest");
    stx1.apply();
    let _ = block1.commit();

    // Block 2: submit the IVM program; admission should accept since abi_hash is not enforced when absent
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), account_id.clone())
        .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();

    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_ok(),
        "manifest with no abi_hash should not block admission, got {result:?}"
    );
}

#[test]
fn ivm_manifest_matching_abi_hash_v1_accepted_at_admission() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};
    use iroha_data_model::{
        isi::smart_contract_code::RegisterSmartContractCode,
        permission,
        transaction::{Executable, TransactionBuilder},
    };

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account =
        Account::new(account_id.clone().to_account_id(domain_id.clone())).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    let prog = minimal_ivm_program(1);
    let parsed = ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
    let abi_current = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);

    // Block 1: grant permission and register manifest with v1 abi_hash
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let token = iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
    let perm: permission::Permission = token.into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    let manifest = manifest::ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_current)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&account_id, &mut stx1)
        .expect("register manifest");
    stx1.apply();
    let _ = block1.commit();

    // Block 2: submit program; admission should accept
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), account_id.clone())
        .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();

    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_ok(),
        "v1 abi_hash should allow admission, got {result:?}"
    );
}

#[test]
fn ivm_manifest_unknown_syscall_rejected_before_execution() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};
    use iroha_data_model::{
        isi::smart_contract_code::RegisterSmartContractCode,
        permission,
        transaction::{Executable, TransactionBuilder},
    };

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account =
        Account::new(account_id.clone().to_account_id(domain_id.clone())).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    let prog = minimal_ivm_program_with_syscall(1, 0xAB);
    let parsed = ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
    let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);

    // Block 1: grant permission and register the manifest with the correct abi_hash.
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let token = iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
    let perm: permission::Permission = token.into();
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    let manifest = manifest::ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&kp);
    RegisterSmartContractCode { manifest }
        .execute(&account_id, &mut stx1)
        .expect("register manifest");
    stx1.apply();
    let _ = block1.commit();

    // Block 2: submit the program with an unknown syscall; admission should reject before execution.
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), account_id.clone())
        .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();

    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
            assert!(
                msg.contains("unknown syscall number 0xab"),
                "unknown syscall must be rejected during admission, got {msg}"
            );
        }
        other => panic!("unknown syscall should be rejected before host execution, got {other:?}"),
    }
}
