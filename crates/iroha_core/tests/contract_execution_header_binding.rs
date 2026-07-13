//! Adversarial admission tests for signed contract execution headers.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    isi::smart_contract_code::{RegisterSmartContractBytes, RegisterSmartContractCode},
    prelude::*,
};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn contract_artifact() -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "HeaderBinding".to_owned(),
        compiler_fingerprint: "core-header-binding-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "run".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("ExecuteHeaderBinding".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    ivm::verify_contract_artifact(&artifact).expect("valid contract fixture");
    artifact
}

fn execution_header_mutations(original: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    assert!(
        original.len() >= ivm::HEADER_SIZE,
        "fixture must contain the fixed IVM header"
    );

    let mut mutations = Vec::new();

    for index in 0..ivm::METADATA_MAGIC.len() {
        let mut magic = original.to_vec();
        magic[index] ^= 0xff;
        mutations.push(("magic", magic));
    }

    let mut version_major = original.to_vec();
    version_major[4] = 2;
    mutations.push(("version_major", version_major));

    let mut version_minor = original.to_vec();
    version_minor[5] = 0;
    mutations.push(("version_minor", version_minor));

    let mut mode = original.to_vec();
    mode[6] = ivm::ivm_mode::ZK;
    mutations.push(("mode", mode));

    let mut vector_length = original.to_vec();
    vector_length[7] = 1;
    mutations.push(("vector_length", vector_length));

    let mut max_cycles = original.to_vec();
    max_cycles[8..16].copy_from_slice(&2_u64.to_le_bytes());
    mutations.push(("max_cycles", max_cycles));

    let mut abi_version = original.to_vec();
    abi_version[16] = 0;
    mutations.push(("abi_version", abi_version));

    for index in 17..ivm::HEADER_SIZE {
        let mut abi_hash = original.to_vec();
        abi_hash[index] ^= 0xff;
        mutations.push(("abi_hash", abi_hash));
    }

    mutations
}

fn state_with_authority() -> (State, AccountId, KeyPair) {
    let key_pair = KeyPair::try_random().expect("generate checked manifest signing key");
    let authority = AccountId::of(key_pair.public_key().clone());
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], core::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    (state, authority, key_pair)
}

#[test]
fn signed_and_registered_contract_rejects_every_execution_header_mutation() {
    let (state, authority, key_pair) = state_with_authority();
    let mut block = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    let permission: Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    Grant::account_permission(permission, authority.clone())
        .execute(&authority, &mut transaction)
        .expect("grant contract lifecycle authority");

    let original = contract_artifact();
    let verified = ivm::verify_contract_artifact(&original).expect("verify original artifact");
    let original_hash = verified.code_hash;
    let signed_manifest = verified.manifest.signed(&key_pair);

    RegisterSmartContractBytes {
        code_hash: original_hash,
        code: original.clone(),
    }
    .execute(&authority, &mut transaction)
    .expect("register original bytecode");
    RegisterSmartContractCode {
        manifest: signed_manifest.clone(),
    }
    .execute(&authority, &mut transaction)
    .expect("register original signed manifest");

    let mutations = execution_header_mutations(&original);
    assert_eq!(
        mutations.len(),
        ivm::METADATA_MAGIC.len() + 6 + ivm::HEADER_SIZE - 17,
        "cover every fixed-header field and every physical ABI-digest byte"
    );
    for (field, mutated) in mutations {
        let mutated_hash = ivm::contract_code_hash(&mutated);
        assert_ne!(
            mutated_hash, original_hash,
            "{field} mutation must change the canonical artifact hash"
        );

        let error = RegisterSmartContractBytes {
            code_hash: original_hash,
            code: mutated.clone(),
        }
        .execute(&authority, &mut transaction)
        .expect_err("mutated bytes must not replace content under the signed hash");
        assert!(
            !error.to_string().is_empty(),
            "{field} mutation must produce an admission error"
        );
        assert_eq!(
            transaction
                .world
                .contract_code()
                .get(&original_hash)
                .map(Vec::as_slice),
            Some(original.as_slice()),
            "{field} mutation changed registered bytecode"
        );

        let Ok(mut mutated_verified) = ivm::verify_contract_artifact(&mutated) else {
            // Invalid field values are rejected while registering the bytecode,
            // before a forged manifest can reach provenance verification.
            continue;
        };

        RegisterSmartContractBytes {
            code_hash: mutated_hash,
            code: mutated,
        }
        .execute(&authority, &mut transaction)
        .unwrap_or_else(|error| panic!("{field} structurally valid mutation: {error}"));

        mutated_verified.manifest.provenance = signed_manifest.provenance.clone();
        let error = RegisterSmartContractCode {
            manifest: mutated_verified.manifest,
        }
        .execute(&authority, &mut transaction)
        .expect_err("the original signature must not authorize a mutated header");
        assert!(
            error
                .to_string()
                .contains("manifest signature verification failed"),
            "{field} mutation returned the wrong provenance error: {error}"
        );
        assert!(
            transaction
                .world
                .contract_manifests()
                .get(&mutated_hash)
                .is_none(),
            "{field} mutation registered a forged manifest"
        );
    }

    assert_eq!(
        transaction.world.contract_manifests().get(&original_hash),
        Some(&signed_manifest),
        "adversarial attempts changed the original manifest"
    );
}
