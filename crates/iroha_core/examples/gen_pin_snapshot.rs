//! Utility to print the expected `SoraFS` pin registry snapshot fixture.

use std::{convert::TryInto, error::Error, fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use iroha_data_model::{
    isi::sorafs::{
        ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder, IssueReplicationOrder,
        RegisterPinManifest, RegisterProviderOwner,
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
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;
use norito::{json, json::Value, to_bytes};
#[cfg(test)]
use sorafs_manifest::pin_registry::verify_alias_proof_bundle;
use sorafs_manifest::{
    AliasBindingV1, CouncilSignature, REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1,
    ReplicationOrderSlaV1, ReplicationOrderV1, chunker_registry,
    pin_registry::{AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest},
};

const FIXTURE_PATH: &str = "crates/iroha_core/tests/fixtures/sorafs_pin_registry/snapshot.json";

fn main() -> Result<(), Box<dyn Error>> {
    let state = make_state();
    let mut block = state.block(default_block_header());
    let mut tx = block.transaction();
    bootstrap_sorafs(&mut tx);

    let digest = default_digest();
    let chunk_digest = default_chunk_digest();
    let council_keys = council_keypair();

    register_and_approve(&mut tx, digest, chunk_digest, &council_keys)?;

    let alias_binding = alias_binding_for(digest, "sora", "docs", 12, 36, &council_keys)?;
    BindManifestAlias {
        digest,
        binding: alias_binding.clone(),
        bound_epoch: 12,
        expiry_epoch: 36,
    }
    .execute(&alice(), &mut tx)?;

    let providers = [
        ProviderId::new([0x51; 32]),
        ProviderId::new([0x52; 32]),
        ProviderId::new([0x53; 32]),
    ];
    let order_id = ReplicationOrderId::new([0x44; 32]);
    let order_struct = replication_order(order_id, digest, &providers, 3);
    let order_payload = norito::to_bytes(&order_struct)?;
    IssueReplicationOrder {
        order_id,
        order_payload,
        issued_epoch: 20,
        deadline_epoch: 28,
    }
    .execute(&alice(), &mut tx)?;

    CompleteReplicationOrder {
        order_id,
        completion_epoch: 25,
    }
    .execute(&alice(), &mut tx)?;

    tx.apply();
    block.commit()?;

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
    let pretty = json::to_string_pretty(&snapshot)?;
    fs::write(PathBuf::from(FIXTURE_PATH), format!("{pretty}\n"))?;
    println!("{pretty}");
    Ok(())
}

fn bootstrap_sorafs(tx: &mut iroha_core::state::StateTransaction<'_, '_>) {
    let alice = alice();
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
) -> Result<(), iroha_crypto::Error> {
    RegisterPinManifest {
        digest,
        chunker: default_chunker(),
        chunk_digest_sha3_256: chunk_digest,
        content_length: default_content_length(),
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
    let envelope = build_envelope(&stored, council_keys)?;

    ApprovePinManifest {
        digest,
        approved_epoch: 5,
        council_envelope: Some(envelope),
        council_envelope_digest: None,
    }
    .execute(&alice(), tx)
    .expect("approve manifest");
    Ok(())
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

fn make_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let live = LiveQueryStore::start_test();
    let gov = iroha_config::parameters::actual::Governance::default();
    let world = public_pin_fee_world(&gov);
    State::new_for_testing(world, kura, live)
}

fn public_pin_fee_world(gov: &iroha_config::parameters::actual::Governance) -> World {
    let fee_asset_id = gov.sorafs_pin_fee_asset_id.clone();
    let authority = alice();
    let domains = fee_asset_id
        .try_domain()
        .cloned()
        .map(|domain_id| Domain::new(domain_id).build(&authority))
        .into_iter();
    let mut accounts = vec![Account::new(authority.clone()).build(&authority)];
    let treasury = gov.sorafs_pin_fee_treasury_account.clone();
    if treasury != authority {
        accounts.push(Account::new(treasury).build(&authority));
    }
    let definition = AssetDefinition::numeric(fee_asset_id.clone())
        .with_name(
            fee_asset_id
                .try_name()
                .map_or_else(|| "xor".to_owned(), ToString::to_string),
        )
        .build(&authority);
    let asset = Asset::new(
        AssetId::new(fee_asset_id, authority),
        Numeric::new(10_000_000_000_000_u128, 0),
    );

    World::with_assets(domains, accounts, [definition], [asset], [])
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

fn default_block_header() -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, None, 0, 0)
}

fn default_digest() -> ManifestDigest {
    ManifestDigest::new([0xAA; 32])
}

fn default_chunk_digest() -> [u8; 32] {
    [0xCD; 32]
}

fn default_content_length() -> u64 {
    1_073_741_824
}

fn default_policy() -> PinPolicy {
    PinPolicy {
        min_replicas: 3,
        storage_class: StorageClass::Hot,
        retention_epoch: 42,
    }
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
) -> Result<ManifestAliasBinding, iroha_crypto::Error> {
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
    let signature = Signature::try_new(council_keys.private_key(), digest.as_ref())?;
    let public_bytes = checked_ed25519_public_key_bytes(council_keys, "alias council public key");
    let signer: [u8; 32] = public_bytes
        .try_into()
        .expect("ed25519 public key must contain 32 bytes");

    bundle.council_signatures.push(CouncilSignature {
        signer,
        signature: signature.payload().to_vec(),
    });

    let proof = to_bytes(&bundle).expect("encode alias proof bundle");
    Ok(ManifestAliasBinding {
        name: name.to_owned(),
        namespace: namespace.to_owned(),
        proof,
    })
}

fn council_keypair() -> KeyPair {
    let secret_bytes = [0x11; 32];
    let private =
        PrivateKey::from_bytes(Algorithm::Ed25519, &secret_bytes).expect("private key bytes");
    KeyPair::from_private_key(private).expect("derive keypair")
}

fn checked_ed25519_public_key_bytes<'a>(keypair: &'a KeyPair, context: &str) -> &'a [u8] {
    let (algorithm, public_bytes) = keypair
        .public_key()
        .try_to_bytes()
        .unwrap_or_else(|err| panic!("{context} must be well-formed: {err}"));
    assert_eq!(algorithm, Algorithm::Ed25519, "{context} must be Ed25519");
    public_bytes
}

fn build_envelope(
    record: &PinManifestRecord,
    keypair: &KeyPair,
) -> Result<Vec<u8>, iroha_crypto::Error> {
    let mut sig_entry = json::Map::new();
    let signature = Signature::try_new(keypair.private_key(), record.digest.as_bytes())?;
    let public_bytes_hex = hex::encode(checked_ed25519_public_key_bytes(
        keypair,
        "pin fixture signer public key",
    ));
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
    Ok(serialized)
}

fn alice() -> AccountId {
    AccountId::new(
        "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
            .parse()
            .expect("public key"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest_record() -> PinManifestRecord {
        PinManifestRecord::new(
            default_digest(),
            default_chunker(),
            default_chunk_digest(),
            default_policy(),
            alice(),
            5,
            None,
            None,
            Metadata::default(),
        )
        .with_content_length(default_content_length())
    }

    #[test]
    fn alias_binding_uses_checked_signature_and_verifies() {
        let council_keys = council_keypair();
        let binding = alias_binding_for(default_digest(), "sora", "docs", 12, 36, &council_keys)
            .expect("alias binding should sign");
        let bundle: AliasProofBundleV1 =
            norito::decode_from_bytes(&binding.proof).expect("decode alias proof bundle");

        verify_alias_proof_bundle(&bundle).expect("checked alias proof signature should verify");
    }

    #[test]
    fn council_envelope_uses_checked_signature_and_verifies() {
        let keypair = council_keypair();
        let record = test_manifest_record();
        let envelope = build_envelope(&record, &keypair).expect("council envelope should sign");
        let value: Value = json::from_slice(&envelope).expect("parse council envelope json");
        let map = match value {
            Value::Object(map) => map,
            other => panic!("council envelope should be a JSON object, got {other:?}"),
        };
        let signatures = match map.get("signatures").expect("signatures field") {
            Value::Array(signatures) => signatures,
            other => panic!("signatures field should be an array, got {other:?}"),
        };
        let signature_entry = match signatures.first().expect("one signature entry") {
            Value::Object(entry) => entry,
            other => panic!("signature entry should be an object, got {other:?}"),
        };
        let signature_hex = match signature_entry.get("signature").expect("signature field") {
            Value::String(signature_hex) => signature_hex,
            other => panic!("signature field should be a string, got {other:?}"),
        };
        let signature_bytes = hex::decode(signature_hex).expect("signature hex");
        let signature = Signature::from_bytes(&signature_bytes);

        signature
            .verify(keypair.public_key(), record.digest.as_bytes())
            .expect("checked council envelope signature should verify");
    }
}
