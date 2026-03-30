//! Integration tests covering the `SoraFS` pin registry flows.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{convert::TryInto, env, fs, num::NonZeroU64, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder, IssueReplicationOrder,
            RegisterPinManifest, RegisterProviderOwner, RetirePinManifest,
        },
    },
    prelude::*,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
            ManifestDigest, PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId,
            ReplicationOrderRecord, ReplicationOrderStatus, StorageClass,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
    CanIssueSorafsReplicationOrder, CanRegisterSorafsPin, CanRegisterSorafsProviderOwner,
    CanRetireSorafsPin,
};
use mv::storage::StorageReadOnly;
use norito::{decode_from_bytes, json, json::Value, to_bytes};
use sorafs_manifest::{
    AliasBindingV1, CouncilSignature, REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1,
    ReplicationOrderSlaV1, ReplicationOrderV1, chunker_registry,
    pin_registry::{AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest},
};

const FIXTURE_PATH: &str = "tests/fixtures/sorafs_pin_registry/snapshot.json";

#[test]
fn pin_registry_snapshot_matches_fixture() {
    let state = make_state();
    let mut block = state.block(default_block_header());
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let digest = default_digest();
    let chunk_digest = default_chunk_digest();
    let council_keys = council_keypair();

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let alias_binding = alias_binding_for(digest, "sora", "docs", 12, 36, &council_keys);
    BindManifestAlias {
        digest,
        binding: alias_binding.clone(),
        bound_epoch: 12,
        expiry_epoch: 36,
    }
    .execute(&alice(), &mut tx)
    .expect("bind alias");

    let providers = [
        ProviderId::new([0x51; 32]),
        ProviderId::new([0x52; 32]),
        ProviderId::new([0x53; 32]),
    ];
    let order_id = ReplicationOrderId::new([0x44; 32]);
    let order_struct = replication_order(order_id, digest, &providers, 3);
    let order_payload = norito::to_bytes(&order_struct).expect("encode replication order");
    IssueReplicationOrder {
        order_id,
        order_payload,
        issued_epoch: 20,
        deadline_epoch: 28,
    }
    .execute(&alice(), &mut tx)
    .expect("issue replication order");

    CompleteReplicationOrder {
        order_id,
        completion_epoch: 25,
    }
    .execute(&alice(), &mut tx)
    .expect("complete replication order");

    tx.apply();
    block.commit().expect("commit block");

    let view = state.view();
    let world = view.world();

    let manifest = world.pin_manifests().get(&digest).expect("manifest stored");
    let alias_id = ManifestAliasId::from(&alias_binding);
    let alias_record = world
        .manifest_aliases()
        .get(&alias_id)
        .expect("alias stored");
    let order_record = world
        .replication_orders()
        .get(&order_id)
        .expect("order stored");

    let snapshot = snapshot_json(manifest, alias_record, order_record);
    let fixture = load_fixture();

    if env::var_os("UPDATE_FIXTURES").is_some() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);
        let mut payload = norito::json::to_vec_pretty(&snapshot).expect("serialize fixture");
        payload.push(b'\n');
        fs::write(&fixture_path, payload).expect("write fixture");
        panic!(
            "fixture updated: {}. Re-run tests without UPDATE_FIXTURES to verify.",
            fixture_path.display()
        );
    }

    assert_eq!(
        snapshot, fixture,
        "generated snapshot does not match fixture JSON"
    );
}

#[test]
fn alias_proof_fixture_decodes() {
    let fixture = load_fixture();
    let alias_entry = fixture["aliases"]
        .as_array()
        .and_then(|array| array.first())
        .and_then(Value::as_object)
        .expect("alias fixture entry");

    let proof_b64 = alias_entry
        .get("proof_b64")
        .and_then(Value::as_str)
        .expect("alias proof base64");
    let proof_bytes = BASE64_STD
        .decode(proof_b64)
        .expect("alias proof base64 decoding");

    let bundle: AliasProofBundleV1 =
        decode_from_bytes(&proof_bytes).expect("decode alias proof bundle");

    assert_eq!(bundle.binding.alias, "sora/docs");
    assert_eq!(bundle.binding.bound_at, 12);
    assert_eq!(bundle.binding.expiry_epoch, 36);
    assert_eq!(
        bundle.binding.manifest_cid.as_slice(),
        default_digest().as_bytes()
    );
    assert!(
        !bundle.council_signatures.is_empty(),
        "alias proof bundle must contain council signatures"
    );

    let reencoded = to_bytes(&bundle).expect("re-encode alias proof bundle");
    assert_eq!(
        BASE64_STD.encode(reencoded),
        proof_b64,
        "alias proof bundle must roundtrip to the fixture payload"
    );

    let first_sig = &bundle.council_signatures[0];
    let signer =
        PublicKey::from_bytes(Algorithm::Ed25519, &first_sig.signer).expect("parse signer key");
    let signature = Signature::from_bytes(&first_sig.signature);
    let digest = alias_proof_signature_digest(&bundle);
    signature
        .verify(&signer, digest.as_ref())
        .expect("council signature verifies");
}

#[test]
fn replication_order_fixture_decodes() {
    let fixture = load_fixture();
    let order_entry = fixture["replication_orders"]
        .as_array()
        .and_then(|array| array.first())
        .and_then(Value::as_object)
        .expect("replication order fixture entry");

    let order_b64 = order_entry
        .get("canonical_order_b64")
        .and_then(Value::as_str)
        .expect("replication order base64");
    let order_bytes = BASE64_STD
        .decode(order_b64)
        .expect("replication order base64 decoding");

    let order: ReplicationOrderV1 =
        decode_from_bytes(&order_bytes).expect("decode replication order");

    assert_eq!(order.order_id, [0x44; 32]);
    assert_eq!(order.manifest_digest, *default_digest().as_bytes());
    assert_eq!(order.target_replicas, 3);
    assert_eq!(order.assignments.len(), 3);
    assert_eq!(
        order.chunking_profile,
        format!(
            "{}.{}@{}",
            default_chunker().namespace,
            default_chunker().name,
            default_chunker().semver
        )
    );
    assert_eq!(order.issued_at, 1_700_000_000);
    assert_eq!(order.deadline_at, 1_700_086_400);

    let reencoded = to_bytes(&order).expect("re-encode replication order");
    assert_eq!(
        BASE64_STD.encode(reencoded),
        order_b64,
        "replication order canonical payload must match fixture bytes"
    );
}

#[test]
fn duplicate_alias_binding_is_rejected() {
    let state = make_state();
    let council_keys = council_keypair();

    {
        let mut block = state.block(block_header(1));
        let mut tx = block.transaction();
        bootstrap_sorafs(&mut tx);
        let digest = default_digest();
        let chunk_digest = default_chunk_digest();

        register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

        let alias_binding = alias_binding_for(digest, "sora", "docs", 12, 36, &council_keys);
        BindManifestAlias {
            digest,
            binding: alias_binding,
            bound_epoch: 12,
            expiry_epoch: 36,
        }
        .execute(&alice(), &mut tx)
        .expect("initial alias bind succeeds");

        tx.apply();
        block.commit().expect("commit block");
    }

    let digest_b = ManifestDigest::new([0xAB; 32]);
    let chunk_digest_b = [0xBC; 32];

    let mut block = state.block(block_header(2));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    register_and_approve(&mut tx, digest_b, chunk_digest_b, &council_keys);
    let alias_binding = alias_binding_for(digest_b, "sora", "docs", 16, 36, &council_keys);

    let err = BindManifestAlias {
        digest: digest_b,
        binding: alias_binding,
        bound_epoch: 16,
        expiry_epoch: 36,
    }
    .execute(&alice(), &mut tx)
    .expect_err("duplicate alias binding must fail");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => {
            assert!(
                message.contains("alias `sora/docs` is already"),
                "expected duplicate alias error message, got {message}"
            );
        }
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn replication_order_with_mismatched_profile_is_rejected() {
    let state = make_state();
    let council_keys = council_keypair();
    let digest = default_digest();
    let chunk_digest = default_chunk_digest();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let providers = [
        ProviderId::new([0x61; 32]),
        ProviderId::new([0x62; 32]),
        ProviderId::new([0x63; 32]),
    ];
    let order_id = ReplicationOrderId::new([0x45; 32]);
    let mut order_struct = replication_order(order_id, digest, &providers, 3);
    order_struct.chunking_profile = "sorafs.chunker@9.9.9".into();
    let order_payload = norito::to_bytes(&order_struct).expect("encode replication order");

    let err = IssueReplicationOrder {
        order_id,
        order_payload,
        issued_epoch: 20,
        deadline_epoch: 28,
    }
    .execute(&alice(), &mut tx)
    .expect_err("replication order with mismatched profile must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => {
            assert!(
                message.contains("unknown chunker handle"),
                "expected unknown chunker handle validation failure, got {message}"
            );
        }
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn alias_binding_with_expiry_before_bound_is_rejected() {
    let state = make_state();
    let council_keys = council_keypair();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    let digest = default_digest();
    let chunk_digest = default_chunk_digest();

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let alias_binding = alias_binding_for(digest, "sora", "docs", 12, 10, &council_keys);
    let err = BindManifestAlias {
        digest,
        binding: alias_binding,
        bound_epoch: 12,
        expiry_epoch: 10,
    }
    .execute(&alice(), &mut tx)
    .expect_err("expiry before bound must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => {
            assert!(
                message.contains("expiry epoch"),
                "expected expiry-before-bound rejection, got {message}"
            );
        }
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn alias_binding_exceeding_retention_is_rejected() {
    let state = make_state();
    let council_keys = council_keypair();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    let digest = default_digest();
    let chunk_digest = default_chunk_digest();

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let alias_binding = alias_binding_for(digest, "sora", "docs", 12, 100, &council_keys);
    let err = BindManifestAlias {
        digest,
        binding: alias_binding,
        bound_epoch: 12,
        expiry_epoch: 100,
    }
    .execute(&alice(), &mut tx)
    .expect_err("expiry after retention must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => {
            assert!(
                message.contains("retention"),
                "expected retention guard, got {message}"
            );
        }
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn replication_order_below_min_replicas_is_rejected() {
    let state = make_state();
    let council_keys = council_keypair();
    let digest = default_digest();
    let chunk_digest = default_chunk_digest();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let providers = [
        ProviderId::new([0x71; 32]),
        ProviderId::new([0x72; 32]),
        ProviderId::new([0x73; 32]),
    ];
    let order_id = ReplicationOrderId::new([0x46; 32]);
    let order_struct = replication_order(order_id, digest, &providers, 1);
    let order_payload = norito::to_bytes(&order_struct).expect("encode replication order");

    let err = IssueReplicationOrder {
        order_id,
        order_payload,
        issued_epoch: 20,
        deadline_epoch: 28,
    }
    .execute(&alice(), &mut tx)
    .expect_err("replication order below minimum replicas must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => {
            assert!(
                message.contains("target replicas"),
                "expected target replicas rejection, got {message}"
            );
        }
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn register_manifest_rejects_unknown_chunker_profile() {
    let state = make_state();
    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let mut chunker = default_chunker();
    chunker.namespace = "unknown".into();
    chunker.name = "mystery".into();

    let err = RegisterPinManifest {
        digest: ManifestDigest::new([0xCC; 32]),
        chunker,
        chunk_digest_sha3_256: [0xEF; 32],
        policy: default_policy(),
        submitted_epoch: 5,
        alias: None,
        successor_of: None,
    }
    .execute(&alice(), &mut tx)
    .expect_err("manifest with unknown chunker must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("chunker descriptor mismatch"),
            "expected chunker descriptor validation failure, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn register_manifest_rejects_unknown_successor() {
    let state = make_state();
    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let digest = ManifestDigest::new([0xE1; 32]);
    let unknown_parent = ManifestDigest::new([0xF1; 32]);

    let err = RegisterPinManifest {
        digest,
        chunker: default_chunker(),
        chunk_digest_sha3_256: default_chunk_digest(),
        policy: default_policy(),
        submitted_epoch: 5,
        alias: None,
        successor_of: Some(unknown_parent),
    }
    .execute(&alice(), &mut tx)
    .expect_err("successor must reference existing manifest");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("is not registered"),
            "expected unknown successor message, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn register_manifest_rejects_self_successor() {
    let state = make_state();
    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let digest = ManifestDigest::new([0xE2; 32]);

    let err = RegisterPinManifest {
        digest,
        chunker: default_chunker(),
        chunk_digest_sha3_256: default_chunk_digest(),
        policy: default_policy(),
        submitted_epoch: 5,
        alias: None,
        successor_of: Some(digest),
    }
    .execute(&alice(), &mut tx)
    .expect_err("manifest cannot succeed itself");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("cannot declare itself as successor"),
            "expected self-successor guard message, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn register_manifest_accepts_active_successor() {
    let state = make_state();
    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let parent = ManifestDigest::new([0xE3; 32]);
    RegisterPinManifest {
        digest: parent,
        chunker: default_chunker(),
        chunk_digest_sha3_256: default_chunk_digest(),
        policy: default_policy(),
        submitted_epoch: 5,
        alias: None,
        successor_of: None,
    }
    .execute(&alice(), &mut tx)
    .expect("register predecessor");
    tx.apply();
    block.commit().expect("commit predecessor");

    let mut block = state.block(block_header(2));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    RegisterPinManifest {
        digest: ManifestDigest::new([0xE4; 32]),
        chunker: default_chunker(),
        chunk_digest_sha3_256: default_chunk_digest(),
        policy: default_policy(),
        submitted_epoch: 6,
        alias: None,
        successor_of: Some(parent),
    }
    .execute(&alice(), &mut tx)
    .expect("active predecessor should accept successor");

    let stored = tx
        .world()
        .pin_manifests()
        .get(&ManifestDigest::new([0xE4; 32]))
        .expect("successor stored");
    assert_eq!(stored.successor_of, Some(parent));
}

#[test]
fn register_manifest_rejects_retired_successor() {
    let state = make_state();
    let council_keys = council_keypair();
    let parent = ManifestDigest::new([0xE5; 32]);

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    register_and_approve(&mut tx, parent, default_chunk_digest(), &council_keys);
    tx.apply();
    block.commit().expect("commit approved predecessor");

    let mut block = state.block(block_header(2));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    RetirePinManifest {
        digest: parent,
        retired_epoch: 20,
        reason: None,
    }
    .execute(&alice(), &mut tx)
    .expect("retire predecessor");
    tx.apply();
    block.commit().expect("commit retirement");

    let mut block = state.block(block_header(3));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    let err = RegisterPinManifest {
        digest: ManifestDigest::new([0xE6; 32]),
        chunker: default_chunker(),
        chunk_digest_sha3_256: default_chunk_digest(),
        policy: default_policy(),
        submitted_epoch: 30,
        alias: None,
        successor_of: Some(parent),
    }
    .execute(&alice(), &mut tx)
    .expect_err("successor must reject retired predecessor");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("was retired"),
            "expected retired successor guard message, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn register_manifest_with_successor_persists_pointer() {
    let state = make_state();
    let council_keys = council_keypair();
    let parent = ManifestDigest::new([0xE7; 32]);

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);
    register_and_approve(&mut tx, parent, default_chunk_digest(), &council_keys);
    tx.apply();
    block.commit().expect("commit approved predecessor");

    let mut block = state.block(block_header(2));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let child = ManifestDigest::new([0xE8; 32]);
    RegisterPinManifest {
        digest: child,
        chunker: default_chunker(),
        chunk_digest_sha3_256: default_chunk_digest(),
        policy: default_policy(),
        submitted_epoch: 40,
        alias: None,
        successor_of: Some(parent),
    }
    .execute(&alice(), &mut tx)
    .expect("register successor manifest");

    let stored = tx
        .world()
        .pin_manifests()
        .get(&child)
        .cloned()
        .expect("successor stored");
    assert_eq!(stored.successor_of.as_ref(), Some(&parent));
}

#[test]
fn bind_alias_rejects_expiry_before_bound_epoch() {
    let state = make_state();
    let council_keys = council_keypair();
    let digest = ManifestDigest::new([0xDD; 32]);
    let chunk_digest = default_chunk_digest();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let alias_binding = alias_binding_for(digest, "sora", "docs", 20, 18, &council_keys);
    let err = BindManifestAlias {
        digest,
        binding: alias_binding,
        bound_epoch: 20,
        expiry_epoch: 18,
    }
    .execute(&alice(), &mut tx)
    .expect_err("alias expiry earlier than bound epoch must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("expiry epoch must be greater than or equal to bound epoch"),
            "expected expiry guard message, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn bind_alias_rejects_bound_epoch_before_approval() {
    let state = make_state();
    let council_keys = council_keypair();
    let digest = ManifestDigest::new([0xDE; 32]);
    let chunk_digest = default_chunk_digest();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let alias_binding = alias_binding_for(digest, "sora", "docs", 4, 20, &council_keys);
    let err = BindManifestAlias {
        digest,
        binding: alias_binding,
        bound_epoch: 4,
        expiry_epoch: 20,
    }
    .execute(&alice(), &mut tx)
    .expect_err("alias bound epoch before approval must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("bound_epoch 4 precedes manifest approval epoch 5"),
            "expected approval-bound guard message, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

#[test]
fn bind_alias_rejects_expiry_after_retention_epoch() {
    let state = make_state();
    let council_keys = council_keypair();
    let digest = ManifestDigest::new([0xDF; 32]);
    let chunk_digest = default_chunk_digest();

    let mut block = state.block(block_header(1));
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys);

    let alias_binding = alias_binding_for(
        digest,
        "sora",
        "docs",
        12,
        default_policy().retention_epoch + 5,
        &council_keys,
    );
    let err = BindManifestAlias {
        digest,
        binding: alias_binding,
        bound_epoch: 12,
        expiry_epoch: default_policy().retention_epoch + 5,
    }
    .execute(&alice(), &mut tx)
    .expect_err("alias expiry beyond retention must be rejected");

    match err {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) => assert!(
            message.contains("expiry epoch"),
            "expected retention guard message, got {message}"
        ),
        other => panic!("expected invalid parameter error, received {other:?}"),
    }
}

fn make_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let live = LiveQueryStore::start_test();
    let default_domain: DomainId = iroha_data_model::account::address::default_domain_name()
        .parse()
        .expect("default account domain label");
    let alice = alice();
    let bob = iroha_test_samples::BOB_ID.clone();
    let domain = Domain::new(default_domain.clone()).build(&alice);
    let alice_account = Account::new(alice.clone()).build(&alice);
    let bob_account = Account::new(bob.clone()).build(&bob);
    let world = World::with(
        [domain],
        [alice_account, bob_account],
        std::iter::empty::<AssetDefinition>(),
    );
    State::new_for_testing(world, kura, live)
}

fn default_block_header() -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, None, 0, 0)
}

fn block_header(height: u64) -> iroha_data_model::block::BlockHeader {
    let nz_height = NonZeroU64::new(height).expect("height must be non-zero");
    iroha_data_model::block::BlockHeader::new(nz_height, None, None, None, 0, 0)
}

fn default_digest() -> ManifestDigest {
    ManifestDigest::new([0xAA; 32])
}

fn default_chunk_digest() -> [u8; 32] {
    [0xCD; 32]
}

fn default_chunker() -> ChunkerProfileHandle {
    let descriptor = chunker_registry::default_descriptor();
    ChunkerProfileHandle {
        profile_id: descriptor.id.0,
        namespace: descriptor.namespace.to_owned(),
        name: descriptor.name.to_owned(),
        semver: descriptor.semver.to_owned(),
        multihash_code: descriptor.multihash_code,
    }
}

fn default_policy() -> PinPolicy {
    PinPolicy {
        min_replicas: 3,
        storage_class: StorageClass::Hot,
        retention_epoch: 42,
    }
}

fn bootstrap_sorafs(tx: &mut iroha_core::state::StateTransaction<'_, '_>) {
    let alice = alice();
    let default_domain: DomainId = iroha_data_model::account::address::default_domain_name()
        .parse()
        .expect("default account domain label");
    if tx.world().domains().get(&default_domain).is_none() {
        Register::domain(Domain::new(default_domain.clone()))
            .execute(&alice, tx)
            .expect("register default domain");
    }
    if tx.world().account(&alice).is_err() {
        Register::account(NewAccount::new(alice.clone()))
            .execute(&alice, tx)
            .expect("register sorafs authority");
    }

    {
        let world = &mut tx.world;
        for perm in [
            Permission::from(CanRegisterSorafsPin),
            Permission::from(CanApproveSorafsPin),
            Permission::from(CanRetireSorafsPin),
            Permission::from(CanBindSorafsAlias),
            Permission::from(CanIssueSorafsReplicationOrder),
            Permission::from(CanCompleteSorafsReplicationOrder),
            Permission::from(CanRegisterSorafsProviderOwner),
        ] {
            world.add_account_permission(&alice, perm);
        }
    }

    for provider_id in [
        ProviderId::new([0x51; 32]),
        ProviderId::new([0x52; 32]),
        ProviderId::new([0x53; 32]),
        ProviderId::new([0x61; 32]),
        ProviderId::new([0x62; 32]),
        ProviderId::new([0x63; 32]),
        ProviderId::new([0x71; 32]),
        ProviderId::new([0x72; 32]),
        ProviderId::new([0x73; 32]),
    ] {
        RegisterProviderOwner {
            provider_id,
            owner: alice.clone(),
        }
        .execute(&alice, tx)
        .expect("register provider owner");
    }
}

fn register_and_approve(
    tx: &mut iroha_core::state::StateTransaction<'_, '_>,
    digest: ManifestDigest,
    chunk_digest: [u8; 32],
    council_keys: &KeyPair,
) {
    RegisterPinManifest {
        digest,
        chunker: default_chunker(),
        chunk_digest_sha3_256: chunk_digest,
        policy: default_policy(),
        submitted_epoch: 5,
        alias: None,
        successor_of: None,
    }
    .execute(&alice(), tx)
    .expect("register manifest");

    let stored = tx
        .world()
        .pin_manifests()
        .get(&digest)
        .expect("manifest stored")
        .clone();
    let envelope = build_envelope(&stored, council_keys);

    ApprovePinManifest {
        digest,
        approved_epoch: 5,
        council_envelope: Some(envelope),
        council_envelope_digest: None,
    }
    .execute(&alice(), tx)
    .expect("approve manifest");
}

fn replication_order(
    order_id: ReplicationOrderId,
    manifest: ManifestDigest,
    providers: &[ProviderId],
    target_replicas: u16,
) -> ReplicationOrderV1 {
    let assignments = providers
        .iter()
        .map(|provider| ReplicationAssignmentV1 {
            provider_id: *provider.as_bytes(),
            slice_gib: 512,
            lane: None,
        })
        .collect();
    ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: *order_id.as_bytes(),
        manifest_cid: b"bafyreplicaexamplecidroot".to_vec(),
        manifest_digest: *manifest.as_bytes(),
        chunking_profile: format!(
            "{}.{}@{}",
            default_chunker().namespace,
            default_chunker().name,
            default_chunker().semver
        ),
        target_replicas,
        assignments,
        issued_at: 1_700_000_000,
        deadline_at: 1_700_086_400,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 86_400,
            min_availability_percent_milli: 99_500,
            min_por_success_percent_milli: 98_000,
        },
        metadata: Vec::new(),
    }
}

fn alias_binding_for(
    digest: ManifestDigest,
    namespace: &str,
    name: &str,
    bound_at: u64,
    expiry_epoch: u64,
    council_keys: &KeyPair,
) -> ManifestAliasBinding {
    let binding_payload = AliasBindingV1 {
        alias: format!("{namespace}/{name}"),
        manifest_cid: digest.as_bytes().to_vec(),
        bound_at,
        expiry_epoch,
    };

    let merkle_path: Vec<[u8; 32]> = Vec::new();
    let registry_root =
        alias_merkle_root(&binding_payload, &merkle_path).expect("compute alias proof merkle root");

    let generated_at_unix = 1_700_000_000;
    let expires_at_unix = generated_at_unix + 86_400;

    let mut bundle = AliasProofBundleV1 {
        binding: binding_payload,
        registry_root,
        registry_height: bound_at,
        generated_at_unix,
        expires_at_unix,
        merkle_path,
        council_signatures: Vec::new(),
    };

    let digest = alias_proof_signature_digest(&bundle);
    let signature = Signature::new(council_keys.private_key(), digest.as_ref());
    let (_, public_bytes) = council_keys.public_key().to_bytes();
    let signer: [u8; 32] = public_bytes
        .try_into()
        .expect("ed25519 public key must contain 32 bytes");

    bundle.council_signatures.push(CouncilSignature {
        signer,
        signature: signature.payload().to_vec(),
    });

    let proof = to_bytes(&bundle).expect("encode alias proof bundle");
    ManifestAliasBinding {
        name: name.to_owned(),
        namespace: namespace.to_owned(),
        proof,
    }
}

fn council_keypair() -> KeyPair {
    let secret_bytes = [0x11; 32];
    let private =
        PrivateKey::from_bytes(Algorithm::Ed25519, &secret_bytes).expect("private key bytes");
    KeyPair::from_private_key(private).expect("derive keypair")
}

fn build_envelope(record: &PinManifestRecord, keypair: &KeyPair) -> Vec<u8> {
    let mut sig_entry = json::Map::new();
    let signature = Signature::new(keypair.private_key(), record.digest.as_bytes());
    let public_bytes_hex = hex::encode(keypair.public_key().to_bytes().1);
    sig_entry.insert("algorithm".into(), Value::from("ed25519"));
    sig_entry.insert("signer".into(), Value::from(public_bytes_hex));
    sig_entry.insert(
        "signature".into(),
        Value::from(hex::encode(signature.payload())),
    );
    sig_entry.insert(
        "signer_multihash".into(),
        Value::from(keypair.public_key().to_string()),
    );

    let mut envelope = json::Map::new();
    envelope.insert(
        "chunk_digest_sha3_256".into(),
        Value::from(hex::encode(record.chunk_digest_sha3_256)),
    );
    envelope.insert(
        "manifest_blake3".into(),
        Value::from(hex::encode(record.digest.as_bytes())),
    );
    envelope.insert("profile".into(), Value::from(record.chunker.to_handle()));
    envelope.insert(
        "signatures".into(),
        Value::Array(vec![Value::Object(sig_entry)]),
    );

    let mut serialized = json::to_vec_pretty(&Value::Object(envelope)).expect("serialize envelope");
    serialized.push(b'\n');
    serialized
}

fn alice() -> AccountId {
    AccountId::new(
        "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
            .parse()
            .expect("public key"),
    )
}

fn snapshot_json(
    manifest: &PinManifestRecord,
    alias: &ManifestAliasRecord,
    order: &ReplicationOrderRecord,
) -> Value {
    let manifest_obj = manifest_snapshot(manifest);
    let alias_obj = alias_snapshot(alias);
    let order_obj = order_snapshot(order);

    let mut root = json::Map::new();
    root.insert(
        "manifests".into(),
        Value::Array(vec![Value::Object(manifest_obj)]),
    );
    root.insert(
        "aliases".into(),
        Value::Array(vec![Value::Object(alias_obj)]),
    );
    root.insert(
        "replication_orders".into(),
        Value::Array(vec![Value::Object(order_obj)]),
    );
    Value::Object(root)
}

fn manifest_snapshot(manifest: &PinManifestRecord) -> json::Map {
    let mut manifest_obj = json::Map::new();
    manifest_obj.insert(
        "digest_hex".into(),
        Value::String(hex::encode(manifest.digest.as_bytes())),
    );
    let (status_label, status_epoch) = match manifest.status {
        PinStatus::Pending => ("pending", None),
        PinStatus::Approved(epoch) => ("approved", Some(epoch)),
        PinStatus::Retired(epoch) => ("retired", Some(epoch)),
    };
    manifest_obj.insert("status".into(), Value::String(status_label.into()));
    manifest_obj.insert(
        "status_epoch".into(),
        status_epoch.map_or(Value::Null, Value::from),
    );
    manifest_obj.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::String(hex::encode(manifest.chunk_digest_sha3_256)),
    );
    manifest_obj.insert(
        "chunker_handle".into(),
        Value::String(manifest.chunker.to_handle()),
    );
    let mut policy_obj = json::Map::new();
    policy_obj.insert(
        "min_replicas".into(),
        Value::from(manifest.policy.min_replicas),
    );
    policy_obj.insert(
        "storage_class".into(),
        Value::String(
            match manifest.policy.storage_class {
                StorageClass::Hot => "hot",
                StorageClass::Warm => "warm",
                StorageClass::Cold => "cold",
            }
            .into(),
        ),
    );
    policy_obj.insert(
        "retention_epoch".into(),
        Value::from(manifest.policy.retention_epoch),
    );
    manifest_obj.insert("policy".into(), Value::Object(policy_obj));
    manifest_obj.insert(
        "submitted_by".into(),
        Value::String(manifest.submitted_by.to_string()),
    );
    manifest_obj.insert(
        "submitted_epoch".into(),
        Value::from(manifest.submitted_epoch),
    );
    manifest_obj.insert(
        "alias_label".into(),
        manifest.alias.as_ref().map_or(Value::Null, |binding| {
            Value::String(format!("{}/{}", binding.namespace, binding.name))
        }),
    );
    manifest_obj.insert(
        "council_envelope_digest_hex".into(),
        manifest
            .council_envelope_digest
            .map_or(Value::Null, |digest| Value::String(hex::encode(digest))),
    );
    manifest_obj
}

fn alias_snapshot(alias: &ManifestAliasRecord) -> json::Map {
    let mut alias_obj = json::Map::new();
    alias_obj.insert(
        "alias_label".into(),
        Value::String(alias.alias_id().as_label()),
    );
    alias_obj.insert(
        "namespace".into(),
        Value::String(alias.binding.namespace.clone()),
    );
    alias_obj.insert("name".into(), Value::String(alias.binding.name.clone()));
    alias_obj.insert(
        "manifest_digest_hex".into(),
        Value::String(hex::encode(alias.manifest.as_bytes())),
    );
    alias_obj.insert("bound_by".into(), Value::String(alias.bound_by.to_string()));
    alias_obj.insert("bound_epoch".into(), Value::from(alias.bound_epoch));
    alias_obj.insert("expiry_epoch".into(), Value::from(alias.expiry_epoch));
    alias_obj.insert(
        "proof_b64".into(),
        Value::String(BASE64_STD.encode(&alias.binding.proof)),
    );
    alias_obj
}

fn order_snapshot(order: &ReplicationOrderRecord) -> json::Map {
    let order_payload: ReplicationOrderV1 =
        norito::decode_from_bytes(&order.canonical_order).expect("decode order payload");
    let mut order_obj = json::Map::new();
    order_obj.insert(
        "order_id_hex".into(),
        Value::String(hex::encode(order.order_id.as_bytes())),
    );
    order_obj.insert(
        "manifest_digest_hex".into(),
        Value::String(hex::encode(order.manifest_digest.as_bytes())),
    );
    order_obj.insert(
        "issued_by".into(),
        Value::String(order.issued_by.to_string()),
    );
    order_obj.insert("issued_epoch".into(), Value::from(order.issued_epoch));
    order_obj.insert("deadline_epoch".into(), Value::from(order.deadline_epoch));
    let (status_label, status_epoch) = match order.status {
        ReplicationOrderStatus::Pending => ("pending", None),
        ReplicationOrderStatus::Completed(epoch) => ("completed", Some(epoch)),
        ReplicationOrderStatus::Expired(epoch) => ("expired", Some(epoch)),
    };
    order_obj.insert("status".into(), Value::String(status_label.into()));
    order_obj.insert(
        "status_epoch".into(),
        status_epoch.map_or(Value::Null, Value::from),
    );
    order_obj.insert(
        "target_replicas".into(),
        Value::from(order_payload.target_replicas),
    );
    let assignments = order_payload
        .assignments
        .iter()
        .map(|assignment| {
            let mut map = json::Map::new();
            map.insert(
                "provider_id_hex".into(),
                Value::String(hex::encode(assignment.provider_id)),
            );
            map.insert("slice_gib".into(), Value::from(assignment.slice_gib));
            Value::Object(map)
        })
        .collect::<Vec<_>>();
    order_obj.insert("assignments".into(), Value::Array(assignments));
    order_obj.insert(
        "canonical_order_b64".into(),
        Value::String(BASE64_STD.encode(&order.canonical_order)),
    );
    order_obj.insert(
        "sla_ingest_deadline_secs".into(),
        Value::from(order_payload.sla.ingest_deadline_secs),
    );
    order_obj.insert(
        "sla_min_availability_percent_milli".into(),
        Value::from(order_payload.sla.min_availability_percent_milli),
    );
    order_obj.insert(
        "sla_min_por_success_percent_milli".into(),
        Value::from(order_payload.sla.min_por_success_percent_milli),
    );
    order_obj
}

fn load_fixture() -> Value {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join(FIXTURE_PATH);
    let json_str = fs::read_to_string(path).expect("fixture readable");
    norito::json::from_str(&json_str).expect("fixture parses as JSON")
}
