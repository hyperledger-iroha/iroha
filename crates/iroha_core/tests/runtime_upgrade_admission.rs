//! Admission coverage for runtime upgrade manifests under the v1-only policy.
#![allow(clippy::items_after_statements)]

use std::{borrow::Cow, collections::BTreeSet};

use iroha_config::parameters::actual::RuntimeUpgradeProvenanceMode;
use iroha_core::smartcontracts::Execute; // bring trait for `.execute()` on ISIs
use iroha_core::{
    prelude::World,
    state::{State, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    isi::error::{InstructionExecutionError, InvalidParameterError},
    prelude::*,
    runtime::{RuntimeUpgradeRecord, RuntimeUpgradeStatus},
};
use iroha_primitives::json::Json;
use ivm::ProgramMetadata;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const TEST_GAS_LIMIT: u64 = 1_000_000;

fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
    // Program: HALT (minimal body)
    let mut code = Vec::new();
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
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

fn metadata_with_gas_limit(limit: u64) -> iroha_data_model::metadata::Metadata {
    let mut md = iroha_data_model::metadata::Metadata::default();
    let key: iroha_data_model::name::Name = "gas_limit".parse().expect("gas_limit key");
    md.insert(key, Json::new(limit));
    md
}

#[test]
fn runtime_upgrade_rejects_non_v1_manifest() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

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

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    use iroha_data_model::prelude::Grant;
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 2".to_string(),
        description: "Unsupported ABI version".to_string(),
        abi_version: 2,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 2,
        end_height: 10,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let manifest_bytes = manifest.canonical_bytes();
    let err = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx)
        .expect_err("non-v1 runtime upgrade must be rejected");

    match err {
        InstructionExecutionError::InvariantViolation(msg) => {
            assert!(
                msg.contains("unsupported abi_version 2") && msg.contains("expected 1"),
                "unexpected msg: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn propose_runtime_upgrade_allows_v1_when_v1_active() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};
    use iroha_data_model::prelude::Grant;

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

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    let active_manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activated ABI v1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 1,
        end_height: 5,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let active_id = active_manifest.id();
    stx1.world.runtime_upgrades_mut().insert(
        active_id,
        RuntimeUpgradeRecord {
            manifest: active_manifest,
            status: RuntimeUpgradeStatus::ActivatedAt(1),
            proposer: account_id.clone(),
            created_height: 1,
        },
    );
    stx1.apply();
    block1.commit().unwrap();

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Runtime upgrade without ABI bump".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let id = manifest.id();
    let manifest_bytes = manifest.canonical_bytes();
    iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx2)
        .expect("v1 runtime upgrade should be accepted even with v1 active");

    stx2.world
        .runtime_upgrades()
        .get(&id)
        .expect("record persisted");
}

#[test]
fn propose_runtime_upgrade_rejects_non_matching_abi_hash() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

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

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant permission");

    let mut manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: [0xAA; 32], // wrong hash: should be compute_abi_hash(AbiV1)
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    // Deliberately scramble abi_hash while keeping other fields valid.
    manifest.abi_hash[0] ^= 0xFF;
    let manifest_bytes = manifest.canonical_bytes();
    let err = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx)
        .expect_err("mismatched abi_hash must be rejected");

    assert!(matches!(
        err,
        InstructionExecutionError::InvariantViolation(_)
    ));
}

#[test]
fn propose_runtime_upgrade_rejects_incorrect_added_sets() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

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

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        // No new syscalls/types exist in this build, so added_* must be empty.
        added_syscalls: vec![9000],
        added_pointer_types: vec![0x00FF],
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let manifest_bytes = manifest.canonical_bytes();
    let err = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx)
        .expect_err("incorrect added_* sets must be rejected");

    assert!(matches!(
        err,
        InstructionExecutionError::InvariantViolation(_)
    ));
}

#[test]
fn propose_runtime_upgrade_is_idempotent_for_identical_manifest() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

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

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let manifest_bytes = manifest.canonical_bytes();
    iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade {
        manifest_bytes: manifest_bytes.clone(),
    }
    .execute(&account_id, &mut stx)
    .expect("first proposal succeeds");
    // Replay the same proposal; should be treated as a no-op.
    iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx)
        .expect("replaying identical proposal is idempotent");

    let id = manifest.id();
    let rec = stx
        .world
        .runtime_upgrades()
        .get(&id)
        .expect("record persisted");
    assert_eq!(rec.manifest, manifest);
    assert!(matches!(
        rec.status,
        iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed
    ));
}

#[test]
fn activate_runtime_upgrade_is_idempotent_at_start_height() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

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

    // Block 1: grant permission and propose upgrade starting at height 5
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 5,
        end_height: 15,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let id = manifest.id();
    let manifest_bytes = manifest.canonical_bytes();
    iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx1)
        .expect("propose manifest");
    stx1.apply();
    block1.commit().unwrap();

    // Block 5: activate twice at the same height; the second invocation should be a no-op
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();

    iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade { id }
        .execute(&account_id, &mut stx2)
        .expect("first activation succeeds");
    iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade { id }
        .execute(&account_id, &mut stx2)
        .expect("second activation at same height is a no-op");

    let rec = stx2
        .world
        .runtime_upgrades()
        .get(&id)
        .expect("record present after activation");
    assert!(matches!(
        rec.status,
        iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(5)
    ));
}

#[test]
fn activation_allows_v1_in_same_block() {
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::cache::IvmCache,
    };

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

    let prog_v1 = minimal_ivm_program(1);
    let chain: ChainId = "chain".parse().unwrap();

    // Block 1: grant permission and propose upgrade [2, 10)
    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx1)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 2,
        end_height: 10,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let id = manifest.id();
    let manifest_bytes = manifest.canonical_bytes();
    iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx1)
        .expect("propose manifest");
    stx1.apply();
    block1.commit().unwrap();

    // Block 2: activate upgrade, then validate a v1 program in the same block
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();

    iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade { id }
        .execute(&account_id, &mut stx2)
        .expect("activate upgrade at scheduled height");
    stx2.apply();

    let tx =
        iroha_data_model::transaction::TransactionBuilder::new(chain.clone(), account_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog_v1)))
            .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_ok(), "program with ABI v1 should validate");
}

#[test]
fn active_manifest_hash_mismatch_rejects_contracts() {
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::ivm::cache::IvmCache,
        tx::TransactionRejectionReason,
    };
    use iroha_data_model::executor::{IvmAdmissionError, ValidationFail};

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

    // Seed an activated upgrade record with an incorrect abi_hash.
    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: [0x11; 32], // deliberately wrong
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 1,
        end_height: 5,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let id = manifest.id();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.runtime_upgrades_mut().insert(
        id,
        RuntimeUpgradeRecord {
            manifest: manifest.clone(),
            status: RuntimeUpgradeStatus::ActivatedAt(1),
            proposer: account_id.clone(),
            created_height: 1,
        },
    );
    stx.apply();
    block.commit().unwrap();

    // Submitting an ABI v1 program should be rejected due to abi_hash mismatch.
    let prog_v1 = minimal_ivm_program(1);
    let chain: ChainId = "chain".parse().unwrap();
    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let tx =
        iroha_data_model::transaction::TransactionBuilder::new(chain.clone(), account_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog_v1)))
            .sign(kp.private_key());
    let mut ivm_cache = IvmCache::new();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
            IvmAdmissionError::ManifestAbiHashMismatch(info),
        ))) => {
            assert_eq!(info.expected, Hash::prehashed(manifest.abi_hash));
        }
        other => panic!("Expected ManifestAbiHashMismatch for tampered manifest, got {other:?}"),
    }
}

#[test]
fn propose_runtime_upgrade_rejects_missing_provenance_when_required() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random();
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(domain_id.clone(), pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let mut state = State::new_for_testing(world, kura, query_handle);
    state.gov.runtime_upgrade_provenance.mode = RuntimeUpgradeProvenanceMode::Required;

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let manifest_bytes = manifest.canonical_bytes();
    let err = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx)
        .expect_err("missing provenance must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
            assert!(
                msg.contains("runtime_upgrade_provenance:missing_provenance"),
                "unexpected msg: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn propose_runtime_upgrade_rejects_untrusted_signer() {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let trusted = KeyPair::random();
    let untrusted = KeyPair::random();
    let (pubkey, _) = trusted.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(domain_id.clone(), pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let mut state = State::new_for_testing(world, kura, query_handle);
    state.gov.runtime_upgrade_provenance.mode = RuntimeUpgradeProvenanceMode::Required;
    state.gov.runtime_upgrade_provenance.signature_threshold = 1;
    state.gov.runtime_upgrade_provenance.trusted_signers =
        BTreeSet::from([trusted.public_key().clone()]);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm = Permission::new("CanManageRuntimeUpgrades".to_string(), Json::new(()));
    Grant::account_permission(perm, account_id.clone())
        .execute(&account_id, &mut stx)
        .expect("grant permission");

    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "ABI version 1".to_string(),
        description: "Activate ABI version 1".to_string(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: vec![],
        added_pointer_types: vec![],
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    }
    .signed(&untrusted);

    let manifest_bytes = manifest.canonical_bytes();
    let err = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }
        .execute(&account_id, &mut stx)
        .expect_err("untrusted signer must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
            assert!(
                msg.contains("runtime_upgrade_provenance:untrusted_signer"),
                "unexpected msg: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
