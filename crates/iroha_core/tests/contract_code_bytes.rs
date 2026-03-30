//! Tests for registering on-chain contract code bytes.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
};
use mv::storage::StorageReadOnly;

fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let meta = ivm::ProgramMetadata {
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

#[test]
fn register_contract_code_bytes_stores_and_idempotent() {
    use iroha_core::smartcontracts::Execute;
    use iroha_data_model::{isi::smart_contract_code::RegisterSmartContractBytes, prelude::*};

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let kp = iroha_crypto::KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let dom: DomainId = "wonderland".parse().unwrap();
    let auth = AccountId::of(pubkey);
    let domain = Domain::new(dom.clone()).build(&auth);
    let account = Account::new(auth.clone()).build(&auth);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query);

    let header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Prepare program and code hash
    let prog = minimal_ivm_program(1);
    let parsed = ivm::ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

    // Register bytes
    RegisterSmartContractBytes {
        code_hash,
        code: prog.clone(),
    }
    .execute(&auth, &mut stx)
    .expect("register code bytes");
    stx.apply();

    // Verify stored (uncommitted block scope)
    let got = block.world.contract_code().get(&code_hash).cloned();
    assert_eq!(got.as_deref(), Some(prog.as_slice()));
    block.commit().expect("commit initial block");

    // Idempotent re-register
    let mut block2 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(2_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx2 = block2.transaction();
    RegisterSmartContractBytes {
        code_hash,
        code: prog.clone(),
    }
    .execute(&auth, &mut stx2)
    .expect("idempotent");
}

#[test]
fn register_contract_code_bytes_respects_size_cap() {
    use iroha_core::smartcontracts::Execute;
    use iroha_data_model::{
        isi::smart_contract_code::RegisterSmartContractBytes,
        parameter::custom::{CustomParameter, CustomParameterId},
        prelude::*,
    };

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let kp = iroha_crypto::KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let dom: DomainId = "wonderland".parse().unwrap();
    let auth = AccountId::of(pubkey);
    let domain = Domain::new(dom.clone()).build(&auth);
    let account = Account::new(auth.clone()).build(&auth);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query);

    let header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    // Set cap to a tiny value (8 bytes) via custom parameter
    let id = CustomParameterId("max_contract_code_bytes".parse().unwrap());
    let cap = CustomParameter::new(id, iroha_primitives::json::Json::new(8u64));
    SetParameter::new(Parameter::Custom(cap))
        .execute(&auth, &mut stx)
        .expect("set cap");

    // Prepare a minimal IVM program which should exceed 8 bytes overall
    let prog = minimal_ivm_program(1);
    let parsed = ivm::ProgramMetadata::parse(&prog).expect("valid header");
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);

    // Register should fail due to cap
    let err = RegisterSmartContractBytes {
        code_hash,
        code: prog,
    }
    .execute(&auth, &mut stx)
    .unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("code bytes exceed cap"));
}
