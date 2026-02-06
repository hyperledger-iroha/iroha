//! Admission-time guard: reject IVM programs that invoke unknown syscalls under ABI v1.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{borrow::Cow, str::FromStr};

use iroha_core::{
    kura::Kura, prelude::World, query::store::LiveQueryStore, smartcontracts::ivm::cache::IvmCache,
    state::State,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    ValidationFail,
    metadata::Metadata,
    name::Name,
    prelude::*,
    transaction::{Executable, TransactionBuilder, error::TransactionRejectionReason},
};
use iroha_primitives::json::Json;
use ivm::{ProgramMetadata, encoding, instruction};
use nonzero_ext::nonzero;

const TEST_GAS_LIMIT: u64 = 10_000;

fn metadata_with_gas_limit(limit: u64) -> Metadata {
    let mut md = Metadata::default();
    let key = Name::from_str("gas_limit").expect("gas_limit key");
    md.insert(key, Json::new(limit));
    md
}

#[test]
fn unknown_syscall_number_rejected_during_ivm_admission() {
    // Minimal world with a single authority account.
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(domain_id.clone(), pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);

    // Build a tiny program with an unknown syscall (0xAB) followed by HALT.
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(instruction::wide::system::SCALL, 0xAB).to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 10,
        abi_version: 1,
    };
    let mut program = meta.encode();
    program.extend_from_slice(&code);

    // Submit the program; admission should fail before execution due to the unknown syscall.
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), account_id.clone())
        .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();

    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
            assert!(
                msg.contains("unknown syscall number"),
                "unexpected rejection message: {msg}"
            );
        }
        other => panic!("expected validation failure for unknown syscall, got {other:?}"),
    }
}
