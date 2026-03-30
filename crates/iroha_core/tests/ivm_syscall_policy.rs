//! Ensure `CoreHost` enforces syscall policy by `abi_version` header.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::cast_possible_truncation)]

use core::str::FromStr;

use iroha_core::smartcontracts::ivm::host::CoreHost;
use iroha_data_model::prelude::*;
use ivm::{IVM, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};

fn fixture_account(hex_public_key: &str) -> AccountId {
    let public_key = hex_public_key.parse().expect("public key");
    AccountId::new(public_key)
}

fn program_with_scall(sys: u8) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(instruction::wide::system::SCALL, sys).to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 10_000,
        abi_version: 1,
    };
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn metadata_with_gas_limit(limit: u64) -> iroha_data_model::metadata::Metadata {
    let mut md = iroha_data_model::metadata::Metadata::default();
    md.insert(
        iroha_data_model::name::Name::from_str("gas_limit").expect("gas_limit key"),
        iroha_primitives::json::Json::new(limit),
    );
    md
}

#[test]
fn deny_unlisted_syscall_in_current() {
    // Choose a syscall number that is not in the ABI v1 allowlist.
    let prog = program_with_scall(0xAB);
    let mut vm = IVM::new(u64::MAX);
    // Any authority is fine; it won't be used
    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    vm.set_host(CoreHost::new(authority));
    vm.load_program(&prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::UnknownSyscall(_)));
}

#[test]
fn allow_forwarded_alloc_in_current() {
    // ALLOC is forwarded by CoreHost and should be permitted.
    let prog = program_with_scall(ivm_sys::SYSCALL_ALLOC as u8);
    let mut vm = IVM::new(u64::MAX);
    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    vm.set_host(CoreHost::new(authority));
    vm.load_program(&prog).unwrap();
    // Set x10 = 16 for allocation size
    vm.set_register(10, 16);
    vm.run().expect("alloc should be allowed under policy");
}

#[test]
fn unknown_syscall_is_rejected_at_admission() {
    use std::borrow::Cow;

    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::cache::IvmCache,
        state::State, tx::AcceptedTransaction,
    };
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        block::BlockHeader,
        executor::ValidationFail,
        prelude::{AssetDefinition, TransactionBuilder},
        transaction::error::TransactionRejectionReason,
    };
    use nonzero_ext::nonzero;

    // Build a minimal world with a single authority account.
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world =
        iroha_core::state::World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    // Program calls an unknown syscall number (0xAB) before halting.
    let prog = program_with_scall(0xAB);
    let chain: ChainId = "chain".parse().expect("chain id");
    let tx = TransactionBuilder::new(chain, account_id.clone())
        .with_metadata(metadata_with_gas_limit(1_000_000))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

    // Validate the transaction in a block; admission should surface UnknownSyscall.
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = IvmCache::new();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
            assert!(
                msg.contains("unknown syscall number"),
                "expected UnknownSyscall, got {msg}"
            );
        }
        other => panic!("expected NotPermitted(UnknownSyscall), got {other:?}"),
    }
}
