//! Portable complete-snapshot checks over genuine certificates and synthetic World data.
use super::*;
use crate::sumeragi_finality::test_fixtures::NativeFinalityFixture;
use crate::{
    asset::AssetDefinitionId, kagemusha::KagemushaGovernedVerifierRegistryV1,
    nexus::AxtAssetIncarnationV1,
};

fn snapshot() -> (
    WorldStateSnapshotV1,
    AssetDefinitionId,
    AxtAssetIncarnationV1,
    KagemushaGovernedVerifierRegistryV1,
) {
    let asset: AssetDefinitionId = "839FV3NJC8NfgWQvghXU2hEFQm9a".parse().unwrap();
    let incarnation =
        AxtAssetIncarnationV1::try_from_bytes(*Hash::new(b"synthetic asset registration").as_ref())
            .unwrap();
    let registry = KagemushaGovernedVerifierRegistryV1::default();
    let snapshot = WorldStateSnapshotV1 {
        schema_hash: Hash::new(b"synthetic complete registry schema"),
        entries: vec![
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
    (snapshot, asset, incarnation, registry)
}

fn certify(snapshot: &WorldStateSnapshotV1) -> VerifiedSumeragiBlock {
    let mut native = NativeFinalityFixture::new();
    let block = native.block_with_submitted_work(native.next_header());
    let proof = native.certify_with_world_root(block, snapshot.root().unwrap());
    native.verifier().verify_retained_decision(&proof).unwrap()
}

#[test]
fn canonical_trigger_owner_identities_roundtrip_and_bind_exact_hash_preimages() {
    let (mut snapshot, _, _, _) = snapshot();
    let trigger: crate::trigger::TriggerId = "snapshot_trigger".parse().unwrap();
    let value = vec![1_u8, 2, 3];
    let fields = [
        "triggers.by_call",
        "triggers.contracts",
        "triggers.data",
        "triggers.pipeline",
        "triggers.time",
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let kind = WorldStateElementKindV1::Table;
        let path = world_state_path_hash_v1(field, kind).unwrap();
        assert_eq!(
            path,
            Hash::new_from_chunks(&[
                PATH,
                &[kind.tag()],
                &(field.len() as u64).to_le_bytes(),
                field.as_bytes(),
            ]),
            "the trigger registry identity is hashed exactly as declared"
        );
        snapshot.entries.insert(
            index,
            WorldStateSnapshotEntryV1 {
                field_id: field.into(),
                kind,
                key_hash: Some(world_state_value_hash_v1(&trigger).unwrap()),
                value_hash: world_state_value_hash_v1(&value).unwrap(),
            },
        );
    }
    let wire = norito::encode_canonical(&snapshot).unwrap();
    let decoded = WorldStateSnapshotV1::decode_bounded_canonical(&wire).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(
        norito::json::from_json::<WorldStateSnapshotV1>(&norito::json::to_json(&snapshot).unwrap())
            .unwrap(),
        snapshot
    );
    let verified = snapshot.authenticate(&certify(&snapshot)).unwrap();
    for field in fields {
        verified
            .verify_table_value(field, &trigger, &value)
            .unwrap();
    }
    let mut changed = snapshot;
    changed.entries.remove(0);
    assert!(changed.authenticate(&certify(&decoded)).is_err());
}

#[test]
fn world_path_hash_rejects_foreign_namespaces_and_empty_identity_components() {
    for field in [
        "state.world",
        "runtime.lanes",
        "data",
        "world.",
        "triggers.",
        "world..accounts",
        "triggers..data",
        "world.accounts.",
        "triggers.data.",
        "triggers.data/row",
    ] {
        assert!(
            world_state_path_hash_v1(field, WorldStateElementKindV1::Table).is_err(),
            "invalid registry identity {field}"
        );
    }
    let excessive = format!("triggers.{}", "a".repeat(192));
    assert!(world_state_path_hash_v1(&excessive, WorldStateElementKindV1::Table).is_err());
}

#[test]
fn complete_snapshot_binds_real_typed_asset_and_registry_preimages_to_certified_root() {
    let (snapshot, asset, incarnation, registry) = snapshot();
    let tip = certify(&snapshot);
    let verified = snapshot.authenticate(&tip).unwrap();
    assert_eq!(verified.height(), tip.height());
    assert_eq!(verified.context_id(), tip.context_id());
    assert_eq!(verified.world_root(), tip.execution().world_state_root);
    verified
        .verify_table_value("world.axt_asset_incarnations", &asset, &incarnation)
        .unwrap();
    verified
        .verify_cell_value("world.kagemusha_verifier_registry", &registry)
        .unwrap();
    let other =
        AxtAssetIncarnationV1::try_from_bytes(*Hash::new(b"reregistered asset").as_ref()).unwrap();
    assert!(
        verified
            .verify_table_value("world.axt_asset_incarnations", &asset, &other)
            .is_err()
    );
    assert!(
        verified
            .verify_cell_value("world.axt_asset_incarnations", &incarnation)
            .is_err()
    );
}

#[test]
fn omitted_added_or_changed_complete_elements_cannot_match_a_certified_root() {
    let (snapshot, _, _, _) = snapshot();
    let tip = certify(&snapshot);
    for mutation in 0..4 {
        let mut altered = snapshot.clone();
        match mutation {
            0 => {
                altered.entries.pop();
            }
            1 => altered.entries[0].value_hash = Hash::new(b"changed incarnation"),
            2 => altered.schema_hash = Hash::new(b"other registry schema"),
            _ => altered.entries[0].key_hash = Some(Hash::new(b"other canonical key")),
        }
        assert!(altered.authenticate(&tip).is_err());
    }
}

#[test]
fn duplicate_cell_table_key_and_incompatible_field_kind_are_refused() {
    let (snapshot, _, _, _) = snapshot();
    for index in 0..2 {
        let mut duplicate = snapshot.clone();
        duplicate
            .entries
            .insert(index, duplicate.entries[index].clone());
        assert!(duplicate.root().is_err());
    }
    let mut incompatible = snapshot.clone();
    incompatible.entries[0].kind = WorldStateElementKindV1::Cell;
    assert!(incompatible.root().is_err());
    let mut reordered = snapshot;
    reordered.entries.reverse();
    assert!(reordered.root().is_err());
}

#[test]
fn canonical_original_rejects_trailing_bytes_and_unknown_residual_lanes() {
    let (snapshot, _, _, _) = snapshot();
    let original = norito::encode_canonical(&snapshot).unwrap();
    assert_eq!(
        WorldStateSnapshotV1::decode_bounded_canonical(&original).unwrap(),
        snapshot
    );
    let mut trailing = original;
    trailing.push(0);
    assert!(WorldStateSnapshotV1::decode_bounded_canonical(&trailing).is_err());
    let mut json = norito::json::to_json(&snapshot).unwrap();
    json.insert_str(1, "\"residual_lanes\":[],");
    assert!(norito::json::from_json::<WorldStateSnapshotV1>(&json).is_err());
}

#[test]
fn ordinary_write_root_and_uncertified_genesis_do_not_authenticate_current_world() {
    let (snapshot, _, _, _) = snapshot();
    let native = NativeFinalityFixture::new();
    let verifier = native.verifier();
    let genesis = verifier
        .verify_retained_decision(native.genesis_proof())
        .unwrap();
    assert!(snapshot.authenticate(&genesis).is_err());
    let ordinary = verifier.verify_retained_decision(native.latest()).unwrap();
    assert!(snapshot.authenticate(&ordinary).is_err());
}

#[test]
fn typed_asset_absence_requires_complete_certified_snapshot_and_exact_native_key() {
    let (mut snapshot, asset, _, _) = snapshot();
    snapshot.entries.insert(
        0,
        WorldStateSnapshotEntryV1 {
            field_id: "world.asset_definitions".into(),
            kind: WorldStateElementKindV1::Table,
            key_hash: Some(world_state_value_hash_v1(&asset).unwrap()),
            value_hash: Hash::new(b"synthetic definition semantic value"),
        },
    );
    let tip = certify(&snapshot);
    let verified = snapshot.authenticate(&tip).unwrap();
    assert!(verified.verify_asset_definition_absent(&asset).is_err());
    let other = crate::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::parse_fully_qualified("synthetic.is2").unwrap(),
        "OTHER".parse().unwrap(),
    );
    assert_ne!(asset, other);
    verified.verify_asset_definition_absent(&other).unwrap();
    let mut omitted = snapshot.clone();
    omitted.entries.remove(0);
    // Omitting the selected row must fail certification before absence is available.
    assert!(omitted.authenticate(&tip).is_err());
    // A separately certified complete native cut may have no definitions.
    let empty_tip = certify(&omitted);
    omitted
        .authenticate(&empty_tip)
        .unwrap()
        .verify_asset_definition_absent(&asset)
        .unwrap();
}

#[test]
fn asset_absence_rejects_incompatible_field_kind_even_in_certified_synthetic_data() {
    let (mut snapshot, asset, _, _) = snapshot();
    snapshot.entries.insert(
        0,
        WorldStateSnapshotEntryV1 {
            field_id: "world.asset_definitions".into(),
            kind: WorldStateElementKindV1::Cell,
            key_hash: None,
            value_hash: Hash::new(b"synthetic incompatible field"),
        },
    );
    let tip = certify(&snapshot);
    let verified = snapshot.authenticate(&tip).unwrap();
    assert!(verified.verify_asset_definition_absent(&asset).is_err());
}
