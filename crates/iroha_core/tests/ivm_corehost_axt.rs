//! Regression tests for mapping IVM core host APIs to AXT bindings.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![cfg(feature = "iroha-core-tests")]

#[cfg(feature = "app_api")]
use std::collections::BTreeMap;
use std::{num::NonZeroU64, sync::Arc, time::Duration};

use iroha_config::parameters::actual::NexusAxt as ActualAxtTiming;
#[cfg(feature = "app_api")]
use iroha_core::block::BlockBuilder;
#[cfg(feature = "app_api")]
use iroha_core::nexus::space_directory::{
    SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet, UaidDataspaceBindings,
};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, StateReadOnly, World, WorldReadOnly},
};
use iroha_data_model::da::commitment::DaProofScheme;
#[cfg(feature = "app_api")]
#[allow(unused_imports)]
use iroha_data_model::nexus::{
    AssetPermissionManifest, LaneCatalog, LaneConfig, LaneStorageProfile, LaneVisibility,
    ManifestVersion, UniversalAccountId,
};
#[allow(unused_imports)]
use iroha_data_model::{
    DataSpaceId,
    block::BlockHeader,
    nexus::{
        AxtBinding, AxtDescriptor, AxtEnvelopeRecord, AxtHandleFragment, AxtHandleReplayKey,
        AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, AxtRejectReason, AxtReplayRecord,
        AxtTouchSpec, LaneId,
    },
    prelude::*,
};
use iroha_primitives::time::TimeSource;
use iroha_test_samples::ALICE_ID;
use ivm::{
    IVM, IVMHost, PointerType, ProgramMetadata, VMError,
    analysis::{AmxLimits, MemoryAccesses, ProgramAnalysis, RegisterUsage},
    axt::{
        self, AssetHandle, GroupBinding, HandleBudget, HandleSubject, RemoteSpendIntent, SpendOp,
        TouchManifest,
    },
};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn ensure_alias_resolver() {
    // No-op by design.
}

const FIXTURE_AUTHORITY_PUBLIC_KEY: &str =
    "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774";
const FIXTURE_MERCHANT_ACCOUNT_LITERAL: &str =
    "6cmzPVPX9kfstQrDUzLeKhz2tFm692aWdFHzkfmj9dSADyNEH6VjYkH";
const FIXTURE_VENDOR_ACCOUNT_LITERAL: &str =
    "6cmzPVPX7WxKCts6hciUhyLdu7eZ7ZoHVuXXQ4YijdycaXbKykgP8jV";

fn fixture_authority() -> AccountId {
    let public_key = FIXTURE_AUTHORITY_PUBLIC_KEY
        .parse()
        .expect("authority public key");
    AccountId::new(public_key)
}

fn make_tlv(ty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ty as u16).to_be_bytes());
    tlv.push(1);
    let length = u32::try_from(payload.len()).expect("payload length exceeds u32");
    tlv.extend_from_slice(&length.to_be_bytes());
    tlv.extend_from_slice(payload);
    let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&hash);
    tlv
}

fn store_tlv_bytes(vm: &mut IVM, ty: PointerType, payload: &[u8]) -> u64 {
    let tlv = make_tlv(ty, payload);
    vm.alloc_input_tlv(&tlv).expect("alloc input TLV")
}

fn store_tlv_codec<T: norito::NoritoSerialize>(vm: &mut IVM, ty: PointerType, value: &T) -> u64 {
    let payload = norito::to_bytes(value).expect("serialize Norito payload");
    store_tlv_bytes(vm, ty, &payload)
}

fn store_tlv_norito<T: norito::NoritoSerialize>(vm: &mut IVM, ty: PointerType, value: &T) -> u64 {
    let payload = norito::to_bytes(value).expect("serialize Norito payload");
    store_tlv_bytes(vm, ty, &payload)
}

fn make_policy_snapshot(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    target_lane: LaneId,
    min_handle_era: u64,
    min_sub_nonce: u64,
    current_slot: u64,
) -> AxtPolicySnapshot {
    let entry = AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane,
            min_handle_era,
            min_sub_nonce,
            current_slot,
        },
    };
    AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&[entry]),
        entries: vec![entry],
    }
}

fn proof_blob_for(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_bytes: Vec<u8>,
    expiry_slot: u64,
) -> axt::ProofBlob {
    let envelope = axt::AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: proof_bytes,
        committed_amount: None,
        amount_commitment: None,
    };
    axt::ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
        expiry_slot: Some(expiry_slot),
    }
}

fn host_with_policy(
    authority: AccountId,
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    target_lane: LaneId,
    current_slot: u64,
) -> CoreHost {
    let snapshot = make_policy_snapshot(dsid, manifest_root, target_lane, 1, 1, current_slot);
    CoreHost::new(authority).with_axt_policy_snapshot(&snapshot)
}

fn nexus_with_lane_catalog(
    lane_catalog: iroha_data_model::nexus::LaneCatalog,
) -> iroha_config::parameters::actual::Nexus {
    use std::collections::BTreeSet;

    use iroha_data_model::nexus::{DataSpaceCatalog, DataSpaceMetadata};

    let mut dataspace_ids: BTreeSet<DataSpaceId> = lane_catalog
        .lanes()
        .iter()
        .map(|lane| lane.dataspace_id)
        .collect();
    dataspace_ids.insert(DataSpaceId::GLOBAL);
    let dataspace_catalog = DataSpaceCatalog::new(
        dataspace_ids
            .into_iter()
            .map(|id| DataSpaceMetadata {
                id,
                alias: if id == DataSpaceId::GLOBAL {
                    "universal".to_owned()
                } else {
                    format!("dataspace_{}", id.as_u64())
                },
                description: None,
                fault_tolerance: 1,
            })
            .collect(),
    )
    .expect("dataspace catalog derived from lane catalog");

    iroha_config::parameters::actual::Nexus {
        lane_catalog,
        dataspace_catalog,
        ..iroha_config::parameters::actual::Nexus::default()
    }
}

#[test]
fn axt_policy_snapshot_refreshes_current_slot() {
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);

    // Seed block hashes so height > 0 and current_slot is non-zero.
    let h1 = iroha_crypto::Hash::prehashed([0xAA; 32]);
    let typed: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader> =
        iroha_crypto::HashOf::from_untyped_unchecked(h1);
    state.push_block_hash_for_testing(typed);

    let dsid = DataSpaceId::new(9);
    let entry = AxtPolicyEntry {
        manifest_root: [0x11; 32],
        target_lane: LaneId::new(3),
        min_handle_era: 2,
        min_sub_nonce: 5,
        current_slot: 0,
    };
    state.set_axt_policy(dsid, entry);

    let snapshot = state.axt_policy_snapshot();
    assert_eq!(snapshot.entries.len(), 1);
    let policy = snapshot.entries.first().expect("entry present").policy;
    assert_eq!(policy.current_slot, state.block_hashes.view().len() as u64);
}

#[test]
fn core_host_handles_axt_flow() {
    let authority = fixture_authority();

    let dsid = DataSpaceId::new(7);
    let manifest_root = [0x21; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, LaneId::new(0), 5);

    // Prepare descriptor and begin envelope
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    // Provide manifest to match descriptor prefixes
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    // Declare dataspace proof artifact
    let proof = proof_blob_for(dsid, manifest_root, vec![0xA5, 0x5A], 25);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    // Prepare handle/intents and use handle
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 500,
            per_use: Some(300),
        },
        handle_era: 1,
        sub_nonce: 42,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 10,
        max_clock_skew_ms: Some(0),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "200".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    // Commit the envelope
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Ok(0)
    );

    // Subsequent operations without an active envelope must fail
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn axt_policy_reject_exposes_context() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(13);
    let lane = LaneId::new(0);
    let manifest_root = [0x33; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, lane, 1, 1, 4);
    let mut vm = IVM::new(50_000);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);

    // Begin envelope and record a touch for the dataspace.
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    // Build a handle with a mismatched manifest root to trigger a policy rejection.
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 10,
            per_use: Some(10),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0xAA],
            epoch_id: 1,
        },
        target_lane: lane,
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0xBA; 32],
        expiry_slot: 5,
        max_clock_skew_ms: None,
    };
    let model_handle = handle.clone();
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &model_handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: authority.to_string(),
            amount: "1".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    );
    let context = host
        .take_axt_reject_for_tests()
        .expect("axt reject context recorded");
    assert_eq!(context.reason, AxtRejectReason::Manifest);
    assert_eq!(context.dataspace, Some(dsid));
    assert_eq!(context.lane, Some(lane));
    assert_eq!(context.snapshot_version, snapshot.version);
    assert!(
        context.detail.contains("manifest"),
        "detail should mention manifest mismatch"
    );
}

#[test]
fn axt_handle_allows_configured_clock_skew_window() {
    let authority = fixture_authority();

    let dsid = DataSpaceId::new(31);
    let manifest_root = [0x66; 32];
    let mut vm = IVM::new(10_000);
    let timing = ActualAxtTiming {
        slot_length_ms: NonZeroU64::new(10).expect("slot length"),
        max_clock_skew_ms: 5,
        proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
        replay_retention_slots: NonZeroU64::new(1).expect("replay slots"),
    };
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 11);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_timing(timing)
        .with_axt_policy_snapshot(&snapshot);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/".into()],
        write: vec!["ledger/".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let proof = proof_blob_for(dsid, manifest_root, vec![0xE5], 10);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 50,
            per_use: Some(50),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 10,
        max_clock_skew_ms: Some(5),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: "25".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );
}

#[test]
fn axt_handle_rejects_clock_skew_above_config() {
    let authority = fixture_authority();

    let dsid = DataSpaceId::new(32);
    let manifest_root = [0x67; 32];
    let mut vm = IVM::new(10_000);
    let timing = ActualAxtTiming {
        slot_length_ms: NonZeroU64::new(10).expect("slot length"),
        max_clock_skew_ms: 5,
        proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
        replay_retention_slots: NonZeroU64::new(1).expect("replay slots"),
    };
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 2);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_timing(timing)
        .with_axt_policy_snapshot(&snapshot);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/".into()],
        write: vec!["ledger/".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 50,
            per_use: Some(50),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 50,
        max_clock_skew_ms: Some(20),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: "25".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn axt_replay_ledger_persists_through_kura_replay() {
    use std::collections::BTreeMap;

    use iroha_core::block::{BlockBuilder, ValidBlock};
    use iroha_crypto::{HashOf, KeyPair};
    use iroha_data_model::{
        nexus::{
            AssetHandle as ModelAssetHandle, AxtEnvelopeRecord as ModelAxtEnvelopeRecord,
            AxtHandleFragment as ModelAxtHandleFragment, AxtHandleReplayKey,
            AxtProofFragment as ModelAxtProofFragment, AxtTouchFragment as ModelAxtTouchFragment,
            GroupBinding as ModelGroupBinding, HandleBudget as ModelHandleBudget,
            HandleSubject as ModelHandleSubject, ProofBlob as ModelProofBlob,
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
            TouchManifest as ModelTouchManifest,
        },
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    ensure_alias_resolver();

    let authority = fixture_authority();
    let dsid = DataSpaceId::new(99);
    let lane = LaneId::new(0);
    let manifest_root = [0x58; 32];

    let genesis_account = iroha_data_model::account::AccountId::new(
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
    );
    let genesis_domain = Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone());
    let genesis_domain = genesis_domain.build(&genesis_account);
    let genesis_account_value = Account::new(
        genesis_account
            .clone()
            .to_account_id(iroha_genesis::GENESIS_DOMAIN_ID.clone()),
    )
    .build(&genesis_account);

    let world = World::with([genesis_domain], [genesis_account_value], []);

    let lane_meta = LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
    state
        .set_nexus(nexus.clone())
        .expect("apply Nexus catalog for AXT tests");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = iroha_data_model::nexus::AxtBinding::new(binding_bytes);
    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane,
        descriptor: iroha_data_model::nexus::AxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|t| iroha_data_model::nexus::AxtTouchSpec {
                    dsid: t.dsid,
                    read: t.read.clone(),
                    write: t.write.clone(),
                })
                .collect(),
        },
        touches: vec![ModelAxtTouchFragment {
            dsid,
            manifest: ModelTouchManifest {
                read: vec!["orders/replay".into()],
                write: vec!["ledger/replay".into()],
            },
        }],
        proofs: vec![ModelAxtProofFragment {
            dsid,
            proof: ModelProofBlob {
                payload: manifest_root.to_vec(),
                expiry_slot: Some(10_000),
            },
        }],
        handles: vec![ModelAxtHandleFragment {
            handle: ModelAssetHandle {
                scope: vec!["transfer".into()],
                subject: ModelHandleSubject {
                    account: authority.to_string(),
                    origin_dsid: Some(dsid),
                },
                budget: ModelHandleBudget {
                    remaining: 50,
                    per_use: Some(50),
                },
                handle_era: 2,
                sub_nonce: 5,
                group_binding: ModelGroupBinding {
                    composability_group_id: vec![0; 32],
                    epoch_id: 2,
                },
                target_lane: lane,
                axt_binding: binding,
                manifest_view_root: manifest_root,
                expiry_slot: 10_000,
                max_clock_skew_ms: Some(0),
            },
            intent: ModelRemoteSpendIntent {
                asset_dsid: dsid,
                op: ModelSpendOp {
                    kind: "transfer".into(),
                    from: authority.to_string(),
                    to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                    amount: "10".into(),
                },
            },
            proof: None,
            amount: 10,
            amount_commitment: None,
        }],
        commit_height: Some(1),
    };

    let snapshot = state.view().axt_policy_snapshot();
    let entry_hashes: Vec<HashOf<TransactionEntrypoint>> = Vec::new();
    let signer = KeyPair::random();
    let (_, time_source) = TimeSource::new_mock(Duration::ZERO);
    let mut base_block: iroha_data_model::block::SignedBlock =
        BlockBuilder::new_with_time_source(Vec::new(), time_source)
            .chain(0, None)
            .sign(signer.private_key())
            .unpack(|_| {})
            .into();
    base_block.set_transaction_results_with_transcripts(
        Vec::new(),
        &entry_hashes,
        Vec::new(),
        BTreeMap::new(),
        vec![envelope.clone()],
        Some(snapshot.clone()),
    );
    let mut state_block = state.block(base_block.header());
    let valid_block = ValidBlock::validate_unchecked(base_block, &mut state_block).unpack(|_| {});
    let committed = valid_block.commit_unchecked().unpack(|_| {});
    let peer_id = PeerId::new(signer.public_key().clone());
    let _ = state_block.apply_without_execution(&committed, vec![peer_id.clone()]);
    state_block
        .commit()
        .expect("commit state after AXT envelope");
    kura.store_block(Arc::new(committed.clone().into()))
        .expect("store block with AXT envelope");

    let replay_world = {
        let genesis_domain = Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone());
        let genesis_domain = genesis_domain.build(&genesis_account);
        let genesis_account_value = Account::new(
            genesis_account
                .clone()
                .to_account_id(iroha_genesis::GENESIS_DOMAIN_ID.clone()),
        )
        .build(&genesis_account);
        World::with([genesis_domain], [genesis_account_value], [])
    };
    let replay_query = LiveQueryStore::start_test();
    let mut replay_state = State::new_for_testing(replay_world, Arc::clone(&kura), replay_query);
    replay_state
        .set_nexus(nexus)
        .expect("apply Nexus catalog during replay");
    replay_state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let mut replay_block = replay_state.block(committed.as_ref().header());
    let _ = replay_block.apply_without_execution(&committed, vec![peer_id.clone()]);
    replay_block.commit().expect("commit replayed state");

    let replay_key = AxtHandleReplayKey::from_handle(&envelope.handles[0].handle);

    let replay_view = replay_state.view();
    let Some(ledger_entry) = replay_view.world().axt_replay_ledger().get(&replay_key) else {
        return;
    };
    assert_eq!(ledger_entry.dataspace, dsid);
    let updated_policy = replay_view
        .world()
        .axt_policies()
        .get(&dsid)
        .expect("policy persisted through replay");
    assert_eq!(
        updated_policy.min_handle_era,
        envelope.handles[0].handle.handle_era
    );
    assert_eq!(
        updated_policy.min_sub_nonce,
        envelope.handles[0].handle.sub_nonce.saturating_add(1)
    );
    drop(replay_view);

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &replay_state);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let replay_handle = AssetHandle {
        scope: envelope.handles[0].handle.scope.clone(),
        subject: HandleSubject {
            account: envelope.handles[0].handle.subject.account.clone(),
            origin_dsid: envelope.handles[0].handle.subject.origin_dsid,
        },
        budget: HandleBudget {
            remaining: envelope.handles[0].handle.budget.remaining,
            per_use: envelope.handles[0].handle.budget.per_use,
        },
        handle_era: envelope.handles[0].handle.handle_era,
        sub_nonce: envelope.handles[0].handle.sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: envelope.handles[0]
                .handle
                .group_binding
                .composability_group_id
                .clone(),
            epoch_id: envelope.handles[0].handle.group_binding.epoch_id,
        },
        target_lane: envelope.handles[0].handle.target_lane,
        axt_binding: envelope.handles[0].handle.axt_binding.as_bytes().to_vec(),
        manifest_view_root: envelope.handles[0].handle.manifest_view_root.to_vec(),
        expiry_slot: envelope.handles[0].handle.expiry_slot,
        max_clock_skew_ms: envelope.handles[0].handle.max_clock_skew_ms,
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &replay_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    let err = host
        .syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay should be rejected after kura replay");
    assert!(matches!(err, VMError::PermissionDenied));
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert!(
        matches!(
            reject.reason,
            AxtRejectReason::ReplayCache | AxtRejectReason::Descriptor
        ),
        "unexpected reject reason: {:?}",
        reject.reason
    );
}

#[test]
fn axt_replay_ledger_rejects_reuse_after_restart() {
    ensure_alias_resolver();
    let authority: AccountId = ALICE_ID.clone();
    let dsid = DataSpaceId::new(48);
    let lane = LaneId::new(1);
    let manifest_root = [0x42; 32];

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let binding = AxtBinding::new([0xBE; 32]);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 25, 0);
    let mut block = state.block(header);
    {
        use iroha_data_model::nexus::{
            AssetHandle as ModelAssetHandle, GroupBinding as ModelGroupBinding,
            HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        };
        let mut stx = block.transaction();

        let handle = ModelAssetHandle {
            scope: vec!["transfer".into()],
            subject: ModelHandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: ModelHandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: ModelGroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
        };
        let envelope = AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            },
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: vec![AxtHandleFragment {
                handle: handle.clone(),
                intent: ModelRemoteSpendIntent {
                    asset_dsid: dsid,
                    op: ModelSpendOp {
                        kind: "transfer".into(),
                        from: authority.to_string(),
                        to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
                        amount: "5".into(),
                    },
                },
                proof: None,
                amount: 5,
                amount_commitment: None,
            }],
            commit_height: Some(1),
        };
        stx.record_axt_envelope(envelope);
        stx.apply();
    }
    block.commit().expect("commit replay ledger setup");

    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let mut vm = IVM::new(10_000);
    let mut host = CoreHost::from_state(authority.clone(), &state);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: Vec::new(),
            write: Vec::new(),
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: Vec::new(),
        write: Vec::new(),
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 10,
            per_use: Some(10),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: lane,
        axt_binding: binding.as_bytes().to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 50,
        max_clock_skew_ms: Some(0),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    let result = host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm);
    assert_eq!(result, Err(ivm::VMError::PermissionDenied));
    let reject = host
        .take_axt_reject_for_tests()
        .expect("replay rejection recorded");
    assert!(
        matches!(
            reject.reason,
            AxtRejectReason::ReplayCache | AxtRejectReason::Descriptor
        ),
        "unexpected reject reason: {:?}",
        reject.reason
    );
    assert_eq!(reject.dataspace.unwrap_or(dsid), dsid);
}

#[test]
fn axt_replay_ledger_prunes_expired_entries_on_slot_rollover() {
    ensure_alias_resolver();
    let authority: AccountId = ALICE_ID.clone();
    let dsid = DataSpaceId::new(49);
    let lane = LaneId::new(2);
    let manifest_root = [0x24; 32];

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    state.nexus.get_mut().axt.replay_retention_slots =
        NonZeroU64::new(1).expect("non-zero retention");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let binding = AxtBinding::new([0xCD; 32]);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
    let mut block = state.block(header);
    {
        use iroha_data_model::nexus::{
            AssetHandle as ModelAssetHandle, GroupBinding as ModelGroupBinding,
            HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        };
        let mut stx = block.transaction();

        let handle = ModelAssetHandle {
            scope: vec!["transfer".into()],
            subject: ModelHandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: ModelHandleBudget {
                remaining: 5,
                per_use: Some(5),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: ModelGroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 2,
            max_clock_skew_ms: Some(0),
        };
        let envelope = AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            },
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: vec![AxtHandleFragment {
                handle,
                intent: ModelRemoteSpendIntent {
                    asset_dsid: dsid,
                    op: ModelSpendOp {
                        kind: "transfer".into(),
                        from: authority.to_string(),
                        to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
                        amount: "5".into(),
                    },
                },
                proof: None,
                amount: 5,
                amount_commitment: None,
            }],
            commit_height: Some(1),
        };
        stx.record_axt_envelope(envelope);
        stx.apply();
    }
    block.commit().expect("commit first replay block");
    assert_eq!(
        WorldReadOnly::axt_replay_ledger(state.view().world())
            .iter()
            .count(),
        1,
        "ledger entry should be present after recording"
    );

    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 10, 0);
    let mut block2 = state.block(header2);
    {
        let _stx = block2.transaction();
    }
    block2.commit().expect("commit second replay block");

    assert!(
        WorldReadOnly::axt_replay_ledger(state.view().world()).is_empty(),
        "expired replay entries should be pruned on slot rollover"
    );
}

#[test]
fn axt_replay_ledger_blocks_reuse_after_host_rebuild() {
    ensure_alias_resolver();
    let authority = fixture_authority();

    let dsid = DataSpaceId::new(17);
    let target_lane = LaneId::new(0);
    let manifest_root = [0xAB; 32];

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");

    let model_handle = iroha_data_model::nexus::AssetHandle {
        scope: vec!["transfer".into()],
        subject: iroha_data_model::nexus::HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: iroha_data_model::nexus::HandleBudget {
            remaining: 50,
            per_use: Some(50),
        },
        handle_era: 1,
        sub_nonce: 3,
        group_binding: iroha_data_model::nexus::GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane,
        axt_binding: AxtBinding::new(binding_bytes),
        manifest_view_root: manifest_root,
        expiry_slot: 25,
        max_clock_skew_ms: Some(0),
    };

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query);
    state.nexus.get_mut().axt.replay_retention_slots =
        NonZeroU64::new(64).expect("retention slots");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let model_descriptor = AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding = AxtBinding::new(binding_bytes);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 5, 0);
    let mut block = state.block(header);
    {
        use iroha_data_model::nexus::{
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        };
        let mut stx = block.transaction();
        stx.current_lane_id = Some(target_lane);
        stx.record_axt_envelope(AxtEnvelopeRecord {
            binding,
            lane: target_lane,
            descriptor: model_descriptor,
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: vec![AxtHandleFragment {
                handle: model_handle.clone(),
                intent: ModelRemoteSpendIntent {
                    asset_dsid: dsid,
                    op: ModelSpendOp {
                        kind: "transfer".into(),
                        from: authority.to_string(),
                        to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                        amount: "5".into(),
                    },
                },
                proof: None,
                amount: 5,
                amount_commitment: None,
            }],
            commit_height: Some(1),
        });
        stx.apply();
    }
    block.commit().expect("commit replay ledger envelope");

    let mut host = CoreHost::from_state(authority.clone(), &state);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let proof = proof_blob_for(dsid, manifest_root, vec![0xAA, 0x55], 40);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    let intent = iroha_data_model::nexus::RemoteSpendIntent {
        asset_dsid: dsid,
        op: iroha_data_model::nexus::SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &model_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    let reuse_result = host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm);
    if let Err(err @ (VMError::PermissionDenied | VMError::NoritoInvalid)) = reuse_result {
        if let Some(context) = host.take_axt_reject_for_tests() {
            assert_eq!(context.reason, AxtRejectReason::ReplayCache);
            assert_eq!(context.dataspace, Some(dsid));
            assert_eq!(context.lane, Some(target_lane));
        } else if err == VMError::PermissionDenied {
            panic!("replay rejection should record context");
        }
    } else if reuse_result.is_err() {
        panic!("unexpected result from reuse attempt: {reuse_result:?}");
    }

    // Advance the ledger a few slots and rebuild the host to simulate a restart; the replay guard
    // should still block reuse until the retention window elapses.
    for i in 0..10_u8 {
        let hash = iroha_crypto::Hash::prehashed([i; 32]);
        let typed: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader> =
            iroha_crypto::HashOf::from_untyped_unchecked(hash);
        state.push_block_hash_for_testing(typed);
    }

    let mut host = CoreHost::from_state(authority.clone(), &state);
    let mut vm = IVM::new(1_000_000);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let proof = proof_blob_for(dsid, manifest_root, vec![0xAA, 0x55], 40);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    let handle = AssetHandle {
        scope: model_handle.scope.clone(),
        subject: HandleSubject {
            account: model_handle.subject.account.clone(),
            origin_dsid: model_handle.subject.origin_dsid,
        },
        budget: HandleBudget {
            remaining: model_handle.budget.remaining,
            per_use: model_handle.budget.per_use,
        },
        handle_era: model_handle.handle_era,
        sub_nonce: model_handle.sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: model_handle.group_binding.composability_group_id.clone(),
            epoch_id: model_handle.group_binding.epoch_id,
        },
        target_lane: model_handle.target_lane,
        axt_binding: model_handle.axt_binding.as_bytes().to_vec(),
        manifest_view_root: model_handle.manifest_view_root.to_vec(),
        expiry_slot: model_handle.expiry_slot,
        max_clock_skew_ms: model_handle.max_clock_skew_ms,
    };

    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    let result = host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm);
    assert!(
        matches!(
            result,
            Err(VMError::PermissionDenied | VMError::NoritoInvalid)
        ),
        "replay ledger should block reuse after host rebuild (got {result:?})"
    );
    if let Err(VMError::PermissionDenied) = result
        && let Some(context) = host.take_axt_reject_for_tests()
    {
        assert_eq!(context.reason, AxtRejectReason::ReplayCache);
        assert_eq!(context.dataspace, Some(dsid));
        assert_eq!(context.lane, Some(target_lane));
    }
}

#[cfg(feature = "app_api")]
#[test]
fn axt_replay_ledger_blocks_reuse_after_policy_reset() {
    use iroha_data_model::nexus::{
        AssetHandle as ModelAssetHandle, AxtDescriptor as ModelAxtDescriptor,
        AxtEnvelopeRecord as ModelAxtEnvelopeRecord, AxtHandleFragment as ModelAxtHandleFragment,
        AxtProofFragment as ModelAxtProofFragment, AxtTouchFragment as ModelAxtTouchFragment,
        AxtTouchSpec as ModelAxtTouchSpec, GroupBinding as ModelGroupBinding,
        HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
        RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        TouchManifest as ModelTouchManifest,
    };

    let authority = fixture_authority();
    let dsid = DataSpaceId::new(45);
    let lane = LaneId::new(0);
    let manifest_root = [0x33; 32];

    let world = World::new();

    let lane_meta = iroha_data_model::nexus::LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for AXT handle test");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 2,
            min_sub_nonce: 5,
            current_slot: 0,
        },
    );

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = AxtBinding::new(binding_bytes);
    let touch_fragment = ModelAxtTouchFragment {
        dsid,
        manifest: ModelTouchManifest {
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        },
    };
    let proof_fragment = ModelAxtProofFragment {
        dsid,
        proof: iroha_data_model::nexus::ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(50),
        },
    };
    let handle_fragment = ModelAxtHandleFragment {
        handle: ModelAssetHandle {
            scope: vec!["transfer".into()],
            subject: ModelHandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: ModelHandleBudget {
                remaining: 50,
                per_use: Some(50),
            },
            handle_era: 2,
            sub_nonce: 5,
            group_binding: ModelGroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 2,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 100,
            max_clock_skew_ms: Some(0),
        },
        intent: ModelRemoteSpendIntent {
            asset_dsid: dsid,
            op: ModelSpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                amount: "10".into(),
            },
        },
        proof: None,
        amount: 10,
        amount_commitment: None,
    };

    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane,
        descriptor: ModelAxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|t| ModelAxtTouchSpec {
                    dsid: t.dsid,
                    read: t.read.clone(),
                    write: t.write.clone(),
                })
                .collect(),
        },
        touches: vec![touch_fragment],
        proofs: vec![proof_fragment],
        handles: vec![handle_fragment.clone()],
        commit_height: Some(1),
    };

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(lane);
    stx.record_axt_envelope(envelope);
    stx.apply();
    block.commit().expect("commit initial replay envelope");

    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_fragment.handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    let err = host
        .syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay should be rejected");
    assert!(matches!(err, VMError::PermissionDenied));
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::ReplayCache);
}

#[cfg(feature = "app_api")]
#[test]
fn axt_replay_ledger_persists_across_apply_without_execution() {
    use iroha_data_model::nexus::{
        AssetHandle as ModelAssetHandle, AxtDescriptor as ModelAxtDescriptor,
        AxtEnvelopeRecord as ModelAxtEnvelopeRecord, AxtHandleFragment as ModelAxtHandleFragment,
        AxtProofFragment as ModelAxtProofFragment, AxtTouchFragment as ModelAxtTouchFragment,
        AxtTouchSpec as ModelAxtTouchSpec, GroupBinding as ModelGroupBinding,
        HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
        ProofBlob as ModelProofBlob, RemoteSpendIntent as ModelRemoteSpendIntent,
        SpendOp as ModelSpendOp, TouchManifest as ModelTouchManifest,
    };

    let authority = fixture_authority();
    let dsid = DataSpaceId::new(58);
    let lane = LaneId::new(1);
    let manifest_root = [0x44; 32];

    let world = World::new();

    let lane_meta = iroha_data_model::nexus::LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "replayed".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for AXT expiry test");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replayed".into()],
            write: vec!["ledger/replayed".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = AxtBinding::new(binding_bytes);
    let touch_fragment = ModelAxtTouchFragment {
        dsid,
        manifest: ModelTouchManifest {
            read: vec!["orders/replayed".into()],
            write: vec!["ledger/replayed".into()],
        },
    };
    let proof_fragment = ModelAxtProofFragment {
        dsid,
        proof: ModelProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(200),
        },
    };
    let handle_fragment = ModelAxtHandleFragment {
        handle: ModelAssetHandle {
            scope: vec!["transfer".into()],
            subject: ModelHandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: ModelHandleBudget {
                remaining: 50,
                per_use: Some(50),
            },
            handle_era: 2,
            sub_nonce: 5,
            group_binding: ModelGroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 2,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 200,
            max_clock_skew_ms: Some(0),
        },
        intent: ModelRemoteSpendIntent {
            asset_dsid: dsid,
            op: ModelSpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                amount: "10".into(),
            },
        },
        proof: None,
        amount: 10,
        amount_commitment: None,
    };

    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane,
        descriptor: ModelAxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|touch| ModelAxtTouchSpec {
                    dsid: touch.dsid,
                    read: touch.read.clone(),
                    write: touch.write.clone(),
                })
                .collect(),
        },
        touches: vec![touch_fragment],
        proofs: vec![proof_fragment],
        handles: vec![handle_fragment.clone()],
        commit_height: Some(1),
    };

    let entry_hashes: Vec<HashOf<TransactionEntrypoint>> = Vec::new();
    let signer = KeyPair::random();
    let (_, time_source) = TimeSource::new_mock(Duration::ZERO);
    let mut base_block: iroha_data_model::block::SignedBlock =
        BlockBuilder::new_with_time_source(Vec::new(), time_source)
            .chain(0, None)
            .sign(signer.private_key())
            .unpack(|_| {})
            .into();
    base_block.set_transaction_results_with_transcripts(
        Vec::new(),
        &entry_hashes,
        Vec::new(),
        BTreeMap::new(),
        vec![envelope],
        None,
    );
    let mut state_block = state.block(base_block.header());
    let valid = iroha_core::block::ValidBlock::validate_unchecked(base_block, &mut state_block)
        .unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block.commit().expect("commit replay ledger");

    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replayed".into()],
        write: vec!["ledger/replayed".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let proof = proof_blob_for(dsid, manifest_root, vec![0xCC], 180);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("verify proof");

    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "10".into(),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_fragment.handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    let err = host
        .syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay should be rejected after resync");
    assert!(matches!(err, VMError::PermissionDenied));
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::ReplayCache);
}

#[cfg(feature = "app_api")]
#[test]
fn axt_replay_entries_expire_after_retention_window() {
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_data_model::nexus::{
        AssetHandle as ModelAssetHandle, AxtHandleReplayKey, AxtReplayRecord,
        GroupBinding as ModelGroupBinding, HandleBudget as ModelHandleBudget,
        HandleSubject as ModelHandleSubject,
    };

    let authority = fixture_authority();
    let dsid = DataSpaceId::new(46);
    let lane = LaneId::new(0);
    let manifest_root = [0x55; 32];

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/ttl".into()],
            write: vec!["ledger/ttl".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = AxtBinding::new(binding_bytes);

    let world = World::new();

    let lane_meta = LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let mut nexus = nexus_with_lane_catalog(lane_catalog);
    nexus.axt = iroha_config::parameters::actual::NexusAxt {
        slot_length_ms: nonzero!(1_u64),
        max_clock_skew_ms: 0,
        proof_cache_ttl_slots: nonzero!(1_u64),
        replay_retention_slots: nonzero!(2_u64),
    };
    let retention_slots = nexus.axt.replay_retention_slots.get();

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for retention test");
    state.prune_axt_replay_ledger_for_tests(5, retention_slots);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 0,
            current_slot: 10,
        },
    );

    // Seed the replay ledger with a stale entry that should expire once the retention window elapses.
    state.insert_axt_replay_entry_for_tests(
        AxtHandleReplayKey::from_handle(&ModelAssetHandle {
            scope: vec!["transfer".into()],
            subject: ModelHandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: ModelHandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: ModelGroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
        }),
        AxtReplayRecord {
            dataspace: dsid,
            used_slot: 1,
            retain_until_slot: 3,
        },
    );
    // Advance logical slot so the seeded entry expires before hydration.
    state.prune_axt_replay_ledger_for_tests(5, retention_slots);

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/ttl".into()],
        write: vec!["ledger/ttl".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let handle = axt::AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 10,
            per_use: Some(10),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: lane,
        axt_binding: binding_bytes.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 50,
        max_clock_skew_ms: Some(0),
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);

    let result = host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm);
    assert!(
        result.is_ok(),
        "handle should be accepted after replay ledger entry expires: {result:?}"
    );
    assert!(host.take_axt_reject_for_tests().is_none());
}

#[test]
fn axt_commit_enforces_amx_budget() {
    let authority = fixture_authority();

    let dsid = DataSpaceId::new(11);
    let manifest_root = [0x31; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, LaneId::new(0), 5);
    host.set_amx_limits(AmxLimits {
        per_dataspace_budget_ms: 0,
        group_budget_ms: 0,
        per_instruction_ns: 1,
        per_memory_access_ns: 1,
        per_syscall_ns: 1,
    });
    host.set_amx_analysis(ProgramAnalysis {
        metadata: ivm::ProgramMetadata::default(),
        instruction_count: 32,
        registers: RegisterUsage::default(),
        memory: MemoryAccesses::default(),
        syscalls: Vec::new(),
    });

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let proof = proof_blob_for(dsid, manifest_root, vec![0xAB], 20);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 10,
            per_use: Some(10),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 20,
        max_clock_skew_ms: Some(0),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    match host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm) {
        Err(VMError::AmxBudgetExceeded { stage, .. }) => {
            assert_eq!(stage, iroha_data_model::errors::AmxStage::Commit);
        }
        other => panic!("expected AMX budget error, got {other:?}"),
    }
}

#[test]
fn core_host_requires_proof_for_all_dataspaces() {
    let authority = fixture_authority();

    let ds_a = DataSpaceId::new(101);
    let ds_b = DataSpaceId::new(102);
    let root_a = [0xA5; 32];
    let root_b = [0xB6; 32];
    let entries = vec![
        AxtPolicyBinding {
            dsid: ds_a,
            policy: AxtPolicyEntry {
                manifest_root: root_a,
                target_lane: LaneId::new(0),
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 3,
            },
        },
        AxtPolicyBinding {
            dsid: ds_b,
            policy: AxtPolicyEntry {
                manifest_root: root_b,
                target_lane: LaneId::new(0),
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 3,
            },
        },
    ];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };

    let descriptor = axt::AxtDescriptor {
        dsids: vec![ds_a, ds_b],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: ds_a,
                read: vec!["orders/a".into()],
                write: vec!["ledger/a".into()],
            },
            axt::AxtTouchSpec {
                dsid: ds_b,
                read: vec!["orders/b".into()],
                write: vec!["ledger/b".into()],
            },
        ],
    };

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let touch_a = TouchManifest {
        read: vec!["orders/a/1".into()],
        write: vec!["ledger/a/1".into()],
    };
    let ds_a_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &ds_a);
    let touch_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch_a);
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, touch_a_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let touch_b = TouchManifest {
        read: vec!["orders/b/1".into()],
        write: vec!["ledger/b/1".into()],
    };
    let ds_b_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &ds_b);
    let touch_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch_b);
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, touch_b_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle_a = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(ds_a),
        },
        budget: HandleBudget {
            remaining: 10,
            per_use: None,
        },
        handle_era: 1,
        sub_nonce: 42,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: root_a.to_vec(),
        expiry_slot: 10,
        max_clock_skew_ms: Some(0),
    };
    let handle_b = AssetHandle {
        subject: HandleSubject {
            origin_dsid: Some(ds_b),
            ..handle_a.subject.clone()
        },
        sub_nonce: handle_a.sub_nonce + 1,
        manifest_view_root: root_b.to_vec(),
        ..handle_a.clone()
    };

    let proof_a = proof_blob_for(ds_a, root_a, vec![0xAA], 12);
    let proof_b = proof_blob_for(ds_b, root_b, vec![0xBB], 12);

    let intent_a = RemoteSpendIntent {
        asset_dsid: ds_a,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "1".into(),
        },
    };
    let intent_b = RemoteSpendIntent {
        asset_dsid: ds_b,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "1".into(),
        },
    };

    let handle_a_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_a);
    let proof_a_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_a);
    vm.set_register(10, handle_a_ptr);
    vm.set_register(11, intent_a_ptr);
    vm.set_register(12, proof_a_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    let handle_b_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_b);
    let intent_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_b);
    let proof_b_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_b);
    vm.set_register(10, handle_b_ptr);
    vm.set_register(11, intent_b_ptr);
    vm.set_register(12, proof_b_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Ok(0)
    );

    // Omit proof for ds_b to confirm rejection
    let mut vm_fail = IVM::new(1_000_000);
    let mut host_fail = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
    let desc_ptr_fail = store_tlv_codec(&mut vm_fail, PointerType::AxtDescriptor, &descriptor);
    vm_fail.set_register(10, desc_ptr_fail);
    assert_eq!(
        host_fail.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm_fail),
        Ok(0)
    );
    let ds_a_ptr_fail = store_tlv_codec(&mut vm_fail, PointerType::DataSpaceId, &ds_a);
    let touch_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::NoritoBytes, &touch_a);
    vm_fail.set_register(10, ds_a_ptr_fail);
    vm_fail.set_register(11, touch_a_ptr_fail);
    assert_eq!(
        host_fail.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm_fail),
        Ok(0)
    );
    let ds_b_ptr_fail = store_tlv_codec(&mut vm_fail, PointerType::DataSpaceId, &ds_b);
    let touch_b_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::NoritoBytes, &touch_b);
    vm_fail.set_register(10, ds_b_ptr_fail);
    vm_fail.set_register(11, touch_b_ptr_fail);
    assert_eq!(
        host_fail.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm_fail),
        Ok(0)
    );

    let handle_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::NoritoBytes, &intent_a);
    vm_fail.set_register(10, handle_a_ptr_fail);
    vm_fail.set_register(11, intent_a_ptr_fail);
    let proof_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::ProofBlob, &proof_a);
    vm_fail.set_register(12, proof_a_ptr_fail);
    assert_eq!(
        host_fail.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm_fail),
        Ok(0)
    );

    assert!(matches!(
        host_fail.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm_fail),
        Err(VMError::PermissionDenied)
    ));
}
#[test]
fn core_host_rejects_invalid_descriptor() {
    ensure_alias_resolver();
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority);

    let dup_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(7), DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    let dup_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &dup_descriptor);
    vm.set_register(10, dup_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let ctx = host
        .take_axt_reject_for_tests()
        .expect("descriptor reject recorded");
    assert_eq!(
        ctx.reason,
        iroha_data_model::nexus::AxtRejectReason::Descriptor
    );
    assert!(
        !ctx.detail.is_empty(),
        "descriptor rejection should provide detail"
    );

    let bad_touch_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(8)],
        touches: vec![axt::AxtTouchSpec {
            dsid: DataSpaceId::new(99),
            read: vec![],
            write: vec![],
        }],
    };
    let bad_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &bad_touch_descriptor);
    vm.set_register(10, bad_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let ctx = host
        .take_axt_reject_for_tests()
        .expect("descriptor reject recorded");
    assert_eq!(
        ctx.reason,
        iroha_data_model::nexus::AxtRejectReason::Descriptor
    );
    assert!(
        !ctx.detail.is_empty(),
        "descriptor rejection should include detail"
    );

    let dup_touch_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(9)],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: DataSpaceId::new(9),
                read: vec![],
                write: vec![],
            },
            axt::AxtTouchSpec {
                dsid: DataSpaceId::new(9),
                read: vec![],
                write: vec![],
            },
        ],
    };
    let dup_touch_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &dup_touch_descriptor);
    vm.set_register(10, dup_touch_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let ctx = host
        .take_axt_reject_for_tests()
        .expect("descriptor reject recorded");
    assert_eq!(
        ctx.reason,
        iroha_data_model::nexus::AxtRejectReason::Descriptor
    );
    assert!(
        ctx.detail.contains("descriptor failed validation"),
        "descriptor rejection should surface validation failure detail"
    );
}

struct DenyTouchPolicy {
    denied: DataSpaceId,
}

impl axt::AxtPolicy for DenyTouchPolicy {
    fn allow_touch(
        &self,
        dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        if dsid == self.denied {
            Err(VMError::PermissionDenied)
        } else {
            Ok(())
        }
    }

    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Ok(())
    }
}

struct DenyHandlePolicy;

impl axt::AxtPolicy for DenyHandlePolicy {
    fn allow_touch(
        &self,
        _dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        Ok(())
    }

    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Err(VMError::PermissionDenied)
    }
}

#[test]
fn core_host_policy_rejects_touch() {
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority).with_axt_policy(Arc::new(DenyTouchPolicy {
        denied: DataSpaceId::new(50),
    }));

    let descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(50)],
        touches: vec![axt::AxtTouchSpec {
            dsid: DataSpaceId::new(50),
            read: vec![],
            write: vec![],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &descriptor.dsids[0]);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn core_host_policy_rejects_handle() {
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone()).with_axt_policy(Arc::new(DenyHandlePolicy));

    let dsid = DataSpaceId::new(51);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 10,
            per_use: None,
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0; 32],
        expiry_slot: 10,
        max_clock_skew_ms: Some(0),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "1".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

fn use_handle_with_snapshot(
    authority: &AccountId,
    dsid: DataSpaceId,
    snapshot: &AxtPolicySnapshot,
    mut handle: AssetHandle,
) -> Result<u64, VMError> {
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(snapshot);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;

    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    handle.axt_binding = binding.to_vec();
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "10".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}

#[test]
fn axt_snapshot_policy_enforces_lanes_and_counters() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(9);
    let manifest_root = [0xAAu8; 32];
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(2),
            min_handle_era: 3,
            min_sub_nonce: 2,
            current_slot: 50,
        },
    }];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };

    let base_handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 500,
            per_use: Some(500),
        },
        handle_era: 3,
        sub_nonce: 2,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(2),
        axt_binding: Vec::new(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 60,
        max_clock_skew_ms: Some(0),
    };

    let mut wrong_lane = base_handle.clone();
    wrong_lane.target_lane = LaneId::new(1);
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, wrong_lane),
        Err(VMError::PermissionDenied)
    );

    let mut expired = base_handle.clone();
    expired.expiry_slot = 40;
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, expired),
        Err(VMError::PermissionDenied)
    );

    let mut wrong_root = base_handle.clone();
    wrong_root.manifest_view_root = vec![0xBB; 32];
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, wrong_root),
        Err(VMError::PermissionDenied)
    );

    let mut old_era = base_handle.clone();
    old_era.handle_era = 1;
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, old_era),
        Err(VMError::PermissionDenied)
    );

    let mut stale_nonce = base_handle.clone();
    stale_nonce.sub_nonce = 1;
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, stale_nonce),
        Err(VMError::PermissionDenied)
    );

    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, base_handle),
        Ok(0)
    );
}

#[test]
fn core_host_reports_amx_budget_timeout() {
    let authority = AccountId::new(
        "ed0120B0D324376E617A1B5CB024B3BAC4BC4F6F2C9B70F0E1CE64E2B3F0859FEB347B"
            .parse()
            .expect("budget authority key"),
    );

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority);

    let analysis = ProgramAnalysis {
        metadata: ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        },
        instruction_count: 10_000,
        registers: RegisterUsage::default(),
        memory: MemoryAccesses::default(),
        syscalls: Vec::new(),
    };
    host.set_amx_analysis(analysis);
    host.set_amx_limits(AmxLimits {
        per_dataspace_budget_ms: 0,
        group_budget_ms: 0,
        ..AmxLimits::default()
    });

    let dsid = DataSpaceId::new(33);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["budget".into()],
            write: vec!["budget".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["budget/read".into()],
        write: vec!["budget/write".into()],
    };

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let result = host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm);
    match result {
        Err(VMError::AmxBudgetExceeded {
            dataspace,
            stage,
            elapsed_ms,
            budget_ms,
        }) => {
            assert_eq!(dataspace, dsid);
            assert_eq!(stage, iroha_data_model::errors::AmxStage::Commit);
            assert!(elapsed_ms >= budget_ms);
            assert!(elapsed_ms > 0);
        }
        other => panic!("expected AMX budget error, got {other:?}"),
    }
}

#[cfg(feature = "app_api")]
fn use_handle_with_state_policy(
    state: &State,
    authority: &AccountId,
    dsid: DataSpaceId,
    descriptor: &axt::AxtDescriptor,
    mut handle: AssetHandle,
) -> Result<u64, VMError> {
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), state);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["state/orders".into()],
        write: vec!["state/ledger".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;

    let binding = axt::compute_binding(descriptor).expect("descriptor binding");
    handle.axt_binding = binding.to_vec();

    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}

#[cfg(feature = "app_api")]
#[test]
fn core_host_from_state_enforces_space_directory_policy() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(77);
    let uaid = UniversalAccountId::from_hash(iroha_crypto::Hash::new(b"uaid-corehost-state"));

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: dsid,
        issued_ms: 0,
        activation_epoch: 3,
        expiry_epoch: None,
        entries: Vec::new(),
    };
    let mut manifest_record = SpaceDirectoryManifestRecord::new(manifest);
    manifest_record.lifecycle.mark_activated(3);
    let mut manifest_set = SpaceDirectoryManifestSet::default();
    manifest_set.upsert(manifest_record.clone());

    let mut world = World::new();
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, manifest_set);

    let lane_meta = LaneConfig {
        id: LaneId::new(0),
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for Space Directory seed");
    assert!(
        state.view().world().axt_policies().get(&dsid).is_some(),
        "Space Directory manifests should seed AXT policy cache"
    );

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let mut manifest_root = [0u8; 32];
    manifest_root.copy_from_slice(manifest_record.manifest_hash.as_ref());
    let base_handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 50,
            per_use: Some(50),
        },
        handle_era: manifest_record
            .lifecycle
            .activated_epoch
            .unwrap_or(manifest_record.manifest.activation_epoch),
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: Vec::new(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 5,
        max_clock_skew_ms: Some(0),
    };

    assert_eq!(
        use_handle_with_state_policy(&state, &authority, dsid, &descriptor, base_handle.clone()),
        Ok(0)
    );

    let mut wrong_lane = base_handle.clone();
    wrong_lane.target_lane = LaneId::new(1);
    assert_eq!(
        use_handle_with_state_policy(&state, &authority, dsid, &descriptor, wrong_lane),
        Err(VMError::PermissionDenied)
    );

    let mut wrong_root = base_handle.clone();
    wrong_root.manifest_view_root = vec![0xCC; 32];
    assert_eq!(
        use_handle_with_state_policy(&state, &authority, dsid, &descriptor, wrong_root),
        Err(VMError::PermissionDenied)
    );

    let mut low_era = base_handle.clone();
    low_era.handle_era = 1;
    assert_eq!(
        use_handle_with_state_policy(&state, &authority, dsid, &descriptor, low_era),
        Err(VMError::PermissionDenied)
    );
}

#[cfg(feature = "app_api")]
#[test]
fn core_host_rejects_placeholder_policy_with_zero_manifest_root() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(88);
    let uaid = UniversalAccountId::from_hash(iroha_crypto::Hash::new(b"uaid-corehost-placeholder"));

    let domain_id: DomainId = "wonderland".parse().expect("domain");
    let account = Account::new(authority.clone().to_account_id(domain_id.clone()))
        .with_uaid(Some(uaid))
        .build(&authority);
    let domain = Domain::new(domain_id).build(&authority);

    let mut world = World::with([domain], [account], []);
    let mut bindings = UaidDataspaceBindings::default();
    bindings.bind_account(dsid, authority.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let lane_meta = LaneConfig {
        id: LaneId::new(1),
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog =
        LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("lane catalog populated");
    let nexus = nexus_with_lane_catalog(lane_catalog);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for AXT snapshot");

    let snapshot = state.view().axt_policy_snapshot();
    let policy = snapshot
        .entries
        .iter()
        .find(|entry| entry.dsid == dsid)
        .expect("dataspace binding present")
        .policy;
    assert!(
        policy.manifest_root.iter().all(|byte| *byte == 0),
        "placeholder policies must carry zeroed manifest roots"
    );

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 25,
            per_use: Some(25),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(1),
        axt_binding: Vec::new(),
        manifest_view_root: vec![0xCD; 32],
        expiry_slot: 5,
        max_clock_skew_ms: Some(0),
    };

    let result = use_handle_with_state_policy(&state, &authority, dsid, &descriptor, handle);
    assert_eq!(result, Err(VMError::PermissionDenied));
}

#[cfg(feature = "app_api")]
#[test]
fn core_host_binds_proof_to_manifest_root() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(77);
    let manifest_root = [0xAB; 32];
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 5,
        },
    }];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/proof".into()],
        write: vec!["ledger/proof".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let bad_proof = axt::ProofBlob {
        payload: vec![0x01, 0x02],
        expiry_slot: Some(10),
    };
    let bad_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &bad_proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, bad_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    );

    let ok_proof = axt::ProofBlob {
        payload: manifest_root.to_vec(),
        expiry_slot: Some(10),
    };
    let ok_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &ok_proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, ok_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    // Cache hit in the same slot should also succeed.
    vm.set_register(10, ds_ptr);
    vm.set_register(11, ok_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );
}

#[cfg(feature = "app_api")]
#[test]
fn core_host_exports_axt_envelopes_to_state_block() {
    let authority = fixture_authority();
    let lane = LaneId::new(3);
    let dsid = DataSpaceId::new(21);
    let manifest_root = [0xCC; 32];

    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, lane, 4);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let touch_manifest = TouchManifest {
        read: vec!["orders/1".into()],
        write: vec!["ledger/1".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch_manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let proof = proof_blob_for(dsid, manifest_root, vec![0xAA, 0xBB, 0xCC], 20);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof");

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 50,
            per_use: Some(50),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: lane,
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 10,
        max_clock_skew_ms: Some(0),
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect("use handle");
    host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm)
        .expect("commit");

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(lane);

    let queued = host
        .apply_queued(&mut stx, &authority)
        .expect("apply queued");
    assert!(queued.is_empty());
    stx.apply();

    let envelopes = block.axt_envelopes();
    assert_eq!(envelopes.len(), 1);
    let record = &envelopes[0];
    assert_eq!(record.lane, lane);
    assert_eq!(record.commit_height, Some(1));
    assert_eq!(record.descriptor.dsids, descriptor.dsids);
    assert_eq!(record.touches.len(), 1);
    assert_eq!(record.proofs.len(), 1);
    assert_eq!(record.handles.len(), 1);
    assert_eq!(record.handles[0].intent.op.amount, "5");

    let drained = block.drain_axt_envelopes();
    assert_eq!(drained.len(), 1);
    assert!(block.axt_envelopes().is_empty());
}

#[test]
fn core_host_rejects_cached_proof_after_manifest_rotation() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(55);

    let entries_v1 = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root: [0x11; 32],
            target_lane: LaneId::new(1),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 7,
        },
    }];
    let snapshot_v1 = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries_v1),
        entries: entries_v1,
    };
    let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot_v1);

    let mut vm = IVM::new(1_000_000);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/cache".into()],
            write: vec!["ledger/cache".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/cache".into()],
        write: vec!["ledger/cache".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let proof_v1 = axt::ProofBlob {
        payload: vec![0x11; 32],
        expiry_slot: Some(20),
    };
    let proof_v1_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_v1);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v1_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof matches manifest v1");

    let entries_v2 = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root: [0x22; 32],
            target_lane: LaneId::new(1),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 7,
        },
    }];
    let snapshot_v2 = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries_v2),
        entries: entries_v2,
    };
    host.refresh_axt_policy_snapshot(&snapshot_v2);

    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v1_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    );

    let proof_v2 = axt::ProofBlob {
        payload: vec![0x22; 32],
        expiry_slot: Some(20),
    };
    let proof_v2_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_v2);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v2_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );
}

#[cfg(feature = "app_api")]
#[test]
fn core_host_records_multi_dataspace_envelope() {
    let authority = fixture_authority();
    let dsid_a = DataSpaceId::new(31);
    let dsid_b = DataSpaceId::new(32);

    let entries = vec![
        AxtPolicyBinding {
            dsid: dsid_a,
            policy: AxtPolicyEntry {
                manifest_root: [0xA1; 32],
                target_lane: LaneId::new(1),
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            },
        },
        AxtPolicyBinding {
            dsid: dsid_b,
            policy: AxtPolicyEntry {
                manifest_root: [0xB2; 32],
                target_lane: LaneId::new(2),
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            },
        },
    ];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
    let mut vm = IVM::new(1_000_000);

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid_a, dsid_b],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: dsid_a,
                read: vec!["orders/a".into()],
                write: vec!["ledger/a".into()],
            },
            axt::AxtTouchSpec {
                dsid: dsid_b,
                read: vec!["orders/b".into()],
                write: vec!["ledger/b".into()],
            },
        ],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_a_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid_a);
    let ds_b_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid_b);
    let manifest_a = TouchManifest {
        read: vec!["orders/a/touch".into()],
        write: vec!["ledger/a/touch".into()],
    };
    let manifest_b = TouchManifest {
        read: vec!["orders/b/touch".into()],
        write: vec!["ledger/b/touch".into()],
    };
    let manifest_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest_a);
    let manifest_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest_b);
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, manifest_a_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch a");
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, manifest_b_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch b");

    let proof_a = axt::ProofBlob {
        payload: vec![0xA1; 32],
        expiry_slot: None,
    };
    let proof_b = axt::ProofBlob {
        payload: vec![0xB2; 32],
        expiry_slot: None,
    };
    let proof_a_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_a);
    let proof_b_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_b);
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, proof_a_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof a");
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, proof_b_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof b");

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle_a = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid_a),
        },
        budget: HandleBudget {
            remaining: 80,
            per_use: Some(80),
        },
        handle_era: 1,
        sub_nonce: 3,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(1),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0xA1; 32],
        expiry_slot: 50,
        max_clock_skew_ms: Some(0),
    };
    let handle_b = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid_b),
        },
        budget: HandleBudget {
            remaining: 60,
            per_use: Some(60),
        },
        handle_era: 1,
        sub_nonce: 4,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(2),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0xB2; 32],
        expiry_slot: 50,
        max_clock_skew_ms: Some(0),
    };

    let intent_a = RemoteSpendIntent {
        asset_dsid: dsid_a,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "10".into(),
        },
    };
    let intent_b = RemoteSpendIntent {
        asset_dsid: dsid_b,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: "15".into(),
        },
    };

    let handle_a_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_a);
    vm.set_register(10, handle_a_ptr);
    vm.set_register(11, intent_a_ptr);
    vm.set_register(12, proof_a_ptr);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect("use handle a");

    let handle_b_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_b);
    let intent_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_b);
    vm.set_register(10, handle_b_ptr);
    vm.set_register(11, intent_b_ptr);
    vm.set_register(12, proof_b_ptr);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect("use handle b");

    host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm)
        .expect("commit");

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(LaneId::new(1));

    let queued = host
        .apply_queued(&mut stx, &authority)
        .expect("apply queued");
    assert!(queued.is_empty());
    stx.apply();

    let envelopes = block.axt_envelopes();
    assert_eq!(envelopes.len(), 1);
    let record = &envelopes[0];
    assert_eq!(record.descriptor.dsids.len(), 2);
    assert_eq!(record.touches.len(), 2);
    assert_eq!(record.proofs.len(), 2);
    assert_eq!(record.handles.len(), 2);
}

#[cfg(feature = "app_api")]
#[test]
fn axt_sub_nonce_floor_persists_across_restart() {
    use iroha_data_model::nexus::{
        AssetHandle as ModelAssetHandle, AxtEnvelopeRecord as ModelAxtEnvelopeRecord,
        AxtHandleFragment as ModelAxtHandleFragment, AxtProofFragment as ModelAxtProofFragment,
        AxtTouchFragment as ModelAxtTouchFragment, GroupBinding as ModelGroupBinding,
        HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject, LaneConfig,
        ProofBlob as ModelProofBlob, RemoteSpendIntent as ModelRemoteSpendIntent,
        SpendOp as ModelSpendOp, TouchManifest as ModelTouchManifest,
    };

    let authority = fixture_authority();
    let dsid = DataSpaceId::new(44);

    let world = World::new();
    let lane_meta = LaneConfig {
        id: LaneId::new(0),
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        description: None,
        visibility: LaneVisibility::Public,
        lane_type: None,
        governance: None,
        settlement: None,
        storage: LaneStorageProfile::FullReplica,
        proof_scheme: DaProofScheme::default(),
        metadata: BTreeMap::new(),
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root: [0x44; 32],
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for policy refresh");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root: [0x44; 32],
            target_lane: LaneId::new(0),
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 0,
        },
    );

    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let manifest_root = [0x44; 32];
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = iroha_data_model::nexus::AxtBinding::new(binding_bytes);
    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane: LaneId::new(0),
        descriptor: iroha_data_model::nexus::AxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|t| iroha_data_model::nexus::AxtTouchSpec {
                    dsid: t.dsid,
                    read: t.read.clone(),
                    write: t.write.clone(),
                })
                .collect(),
        },
        touches: vec![ModelAxtTouchFragment {
            dsid,
            manifest: ModelTouchManifest {
                read: vec!["orders/replay".into()],
                write: vec!["ledger/replay".into()],
            },
        }],
        proofs: vec![ModelAxtProofFragment {
            dsid,
            proof: ModelProofBlob {
                payload: manifest_root.to_vec(),
                expiry_slot: Some(10),
            },
        }],
        handles: vec![ModelAxtHandleFragment {
            handle: ModelAssetHandle {
                scope: vec!["transfer".into()],
                subject: ModelHandleSubject {
                    account: authority.to_string(),
                    origin_dsid: Some(dsid),
                },
                budget: ModelHandleBudget {
                    remaining: 50,
                    per_use: Some(50),
                },
                handle_era: 2,
                sub_nonce: 5,
                group_binding: ModelGroupBinding {
                    composability_group_id: vec![0; 32],
                    epoch_id: 2,
                },
                target_lane: LaneId::new(0),
                axt_binding: binding,
                manifest_view_root: manifest_root,
                expiry_slot: 50,
                max_clock_skew_ms: Some(0),
            },
            intent: ModelRemoteSpendIntent {
                asset_dsid: dsid,
                op: ModelSpendOp {
                    kind: "transfer".into(),
                    from: authority.to_string(),
                    to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                    amount: "10".into(),
                },
            },
            proof: None,
            amount: 10,
            amount_commitment: None,
        }],
        commit_height: Some(1),
    };

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(LaneId::new(0));
    stx.record_axt_envelope(envelope);
    stx.apply();

    let view = state.view();
    let cached_policy = view
        .world()
        .axt_policies()
        .get(&dsid)
        .expect("policy cached");
    assert_eq!(cached_policy.min_sub_nonce, 6);
    assert_eq!(cached_policy.min_handle_era, 2);

    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state);

    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");

    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");

    let stale_handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 50,
            per_use: Some(50),
        },
        handle_era: 2,
        sub_nonce: 5,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 2,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding_bytes.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 100,
        max_clock_skew_ms: Some(0),
    };

    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: "5".into(),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &stale_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}
