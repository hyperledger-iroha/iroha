//! Genuine native signatures over explicitly synthetic World fixture data.
use super::*;
use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    Registrable,
    account::AccountId,
    asset::{AssetBalancePolicy, AssetDefinitionId},
    sumeragi::{SumeragiFootprint, SumeragiStatus},
    sumeragi_finality::{
        SumeragiFinalityAttestationBody, WorldStateElementKindV1, WorldStateSnapshotEntryV1,
        test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    },
};
use iroha_model_base::peer::PeerId;
use norito::codec::Encode as _;
use std::sync::OnceLock;

fn fixture() -> &'static KagemushaAuthorityStateV1 {
    static FIXTURE: OnceLock<KagemushaAuthorityStateV1> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let asset: AssetDefinitionId = "839FV3NJC8NfgWQvghXU2hEFQm9a".parse().unwrap();
        let owner = AccountId::new(
            KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let definition = AssetDefinition::numeric(
            asset.clone(),
            "Synthetic fixture",
            AssetBalancePolicy::Global,
            None,
        )
        .build(&owner);
        let incarnation =
            AxtAssetIncarnationV1::try_from_bytes(*Hash::new(b"fixture registration").as_ref())
                .unwrap();
        let registry = KagemushaGovernedVerifierRegistryV1::default();
        let snapshot = WorldStateSnapshotV1 {
            schema_hash: Hash::new(b"synthetic complete World registry"),
            entries: vec![
                WorldStateSnapshotEntryV1 {
                    field_id: "world.asset_definitions".into(),
                    kind: WorldStateElementKindV1::Table,
                    key_hash: Some(world_state_value_hash_v1(&asset).unwrap()),
                    value_hash: world_state_value_hash_v1(&definition).unwrap(),
                },
                WorldStateSnapshotEntryV1 {
                    field_id: "world.axt_asset_incarnations".into(),
                    kind: WorldStateElementKindV1::Table,
                    key_hash: Some(world_state_value_hash_v1(&asset).unwrap()),
                    value_hash: world_state_value_hash_v1(&incarnation).unwrap(),
                },
                WorldStateSnapshotEntryV1 {
                    field_id: "world.kagemusha_verifier_registry".into(),
                    kind: WorldStateElementKindV1::Cell,
                    key_hash: None,
                    value_hash: world_state_value_hash_v1(&registry).unwrap(),
                },
            ],
        };
        let mut native = NativeFinalityFixture::start("shared authority state fixture");
        let block = native.block_with_submitted_work(native.next_header());
        let proof = native.certify_with_world_root(block, snapshot.root().unwrap());
        let node = KeyPair::from_seed(vec![1; 32], Algorithm::BlsNormal);
        let node_id = PeerId::new(node.public_key().clone());
        let config = Hash::new(b"fixture native configuration");
        let body = SumeragiFinalityAttestationBody {
            observed_at_unix_ms: 1_000_000,
            challenge: [7; 32],
            network_id: native.network_id(),
            node_fingerprint: Hash::new(node_id.encode()),
            node_id,
            build_fingerprint: Hash::new(b"fixture native binary"),
            config_fingerprint: config,
            genesis_block_hash: native.genesis().hash(),
            genesis_finality_proof: native.genesis_proof().clone(),
            status: SumeragiStatus {
                protocol_version: 1,
                config_fingerprint: config,
                beacon_horizon: None,
                instance: native.verifier().instance().0,
                height: 3,
                view: 0,
                stage: 0,
                leader: None,
                proxy_tail: None,
                high_qc_view: None,
                level: 0,
                start_level: 0,
                t_retx_ms: 100,
                committed_height: 2,
                applied_height: 2,
                awaiting: false,
                signer: Some(node.public_key().clone()),
                unanchored: false,
                abstaining: false,
                halted: None,
                footprint: SumeragiFootprint::default(),
            },
            finality_proof: proof,
        };
        let attestation = SumeragiFinalityAttestation {
            signature: SignatureOf::try_from_hash(node.private_key(), body.signing_hash()).unwrap(),
            body,
        };
        attestation.verify().unwrap();
        let verified_tip = native
            .verifier()
            .verify_retained_decision(native.latest())
            .unwrap();
        snapshot.authenticate(&verified_tip).unwrap();
        KagemushaAuthorityStateV1 {
            attestation,
            world_snapshot: snapshot,
            asset_definition: definition,
            asset_incarnation: incarnation,
            verifier_registry: registry,
        }
    })
}

fn borrowed(value: &KagemushaAuthorityStateV1) -> KagemushaAuthorityStateRefV1<'_> {
    KagemushaAuthorityStateRefV1::new(
        &value.attestation,
        &value.world_snapshot,
        &value.asset_definition,
        &value.asset_incarnation,
        &value.verifier_registry,
    )
}

#[test]
fn borrowed_binary_and_bounded_json_preserve_the_sole_owned_layout() {
    let value = fixture();
    let original = norito::encode_canonical(value).unwrap();
    assert_eq!(
        norito::encode_canonical(&borrowed(value)).unwrap(),
        original
    );
    assert_eq!(
        decode_unverified_kagemusha_authority_state_v1(&original).unwrap(),
        *value
    );
    let owned_json = norito::json::to_json_bounded(value, 2 * 1024 * 1024).unwrap();
    // Explicit local diagnostic capture of public synthetic fixture data only.
    // Normal CI/test runs have no output-file dependency or side effect.
    if let Some(path) = std::env::var_os("KAGEMUSHA_AUTHORITY_STATE_DIAGNOSTIC_JSON") {
        use std::io::Write as _;
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute());
        let mut original = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        original.write_all(owned_json.as_bytes()).unwrap();
        original.sync_all().unwrap();
    }
    assert_eq!(
        norito::json::to_json_bounded(&borrowed(value), owned_json.len()).unwrap(),
        owned_json
    );
    assert!(norito::json::to_json_bounded(&borrowed(value), owned_json.len() - 1).is_err());
}

#[test]
fn originals_reject_trailing_bytes_empty_input_and_extra_root_selection() {
    let value = fixture();
    let mut original = norito::encode_canonical(value).unwrap();
    original.push(0);
    assert!(decode_unverified_kagemusha_authority_state_v1(&original).is_err());
    assert!(decode_unverified_kagemusha_authority_state_v1(&[]).is_err());
    let mut json = norito::json::to_json(value).unwrap();
    json.insert_str(1, "\"trusted_root\":\"candidate\",");
    assert!(norito::json::from_json::<KagemushaAuthorityStateV1>(&json).is_err());
    assert!(
        value.verifier_registry.active_release_id.is_none(),
        "decoding cannot activate a release"
    );
}
