//! World state accumulator algebra, completeness and incremental/cold-capture equivalence.

use super::*;
use crate::{
    smartcontracts::isi::triggers::set::AUTHORITY_FIELDS,
    state::{World, authority_registry::STATE_FIELDS, block_field::BlockField},
};
use iroha_model_base::state_path::StatePath;
use mv::storage::Storage;
use std::{cell::Cell, time::Instant};

fn path(key: &str) -> StatePath {
    key.parse().unwrap()
}

fn sample(index: u8) -> [u8; LANE_BYTES] {
    element(
        &path_hash("world.test", TABLE).unwrap(),
        Some(&Hash::new([index])),
        &Hash::new([index, index]),
    )
}

#[test]
fn ironwood_partial_storage_composition_is_refused_by_cold_capture() {
    use iroha_data_model::runtime::definition::DefinitionDescriptorV1;
    let catalog = crate::runtime::release_catalog().unwrap();
    let successor = catalog.ironwood.as_ref().unwrap();
    for (field, replacement) in &successor.world_storage {
        let mut partial = catalog.baseline.clone();
        partial.bindings.clear();
        partial.active.retain(|id| {
            !matches!(
                catalog.definitions[id],
                DefinitionDescriptorV1::Implementation(_) | DefinitionDescriptorV1::Interface(_)
            )
        });
        partial
            .installed_storage
            .retain(|id| *id != catalog.world_storage[field]);
        partial.installed_storage.push(*replacement);
        partial.installed_storage.sort_unstable();
        partial.check_closure(&catalog.definitions).unwrap();
        let mut world = World::default();
        world.runtime_selection = mv::cell::Cell::new(partial);
        let error = WorldStateAccumulator::capture(&world.block()).unwrap_err();
        assert!(
            error.to_string().contains("complete successor"),
            "{field}: {error}"
        );
    }
}

#[test]
fn ironwood_cutover_preserves_nonempty_predecessor_bytes_and_both_root_rules() {
    use iroha_data_model::privacy::PrivacyProtocolPolicyKeyV1;
    use iroha_data_model::privacy::{PrivacyProtocolPolicyV1, PrivacyRuntimeProtocolIdV1};
    let mut world = World::default();
    let protocol = PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1;
    let key = PrivacyProtocolPolicyKeyV1::new(protocol);
    let policy = PrivacyProtocolPolicyV1::canonical_default_v1(protocol);
    world.privacy_protocol_policies.insert(key, policy.clone());
    let catalog = crate::runtime::release_catalog().unwrap();
    let successor = catalog.ironwood.as_ref().unwrap();
    let mut block = world.block();
    let before = WorldStateAccumulator::capture(&block).unwrap();
    let old_root = before.root(&catalog.baseline).unwrap();
    for id in &successor.definitions {
        block
            .runtime_definitions
            .insert(*id, catalog.definitions[id].clone());
    }
    *block.runtime_selection.get_mut() = successor.selection.clone();
    assert_eq!(
        norito::encode_canonical(block.privacy_protocol_policies.get(&key).unwrap()).unwrap(),
        norito::encode_canonical(&policy).unwrap()
    );
    let after = before.apply_block(&block).unwrap();
    assert_eq!(after, WorldStateAccumulator::capture(&block).unwrap());
    assert_eq!(
        old_root,
        WorldStateAccumulator::capture_predecessor(&block)
            .unwrap()
            .root(&catalog.baseline)
            .unwrap()
    );
    assert_ne!(old_root, after.root(&successor.selection).unwrap());
    assert!(
        crate::runtime::ironwood::validate_cutover(
            &successor.selection,
            &catalog.baseline,
            &catalog.definitions
        )
        .is_err()
    );
    let mut duplicate = successor.selection.clone();
    duplicate
        .installed_storage
        .push(catalog.world_storage["world.privacy_protocol_policies"]);
    duplicate.installed_storage.sort_unstable();
    assert!(installed_slots(field_index().as_ref().unwrap(), &duplicate).is_err());
}

/// Independent restatement of the documented element formula and lane arithmetic.
fn documented_sum(entries: &[(Hash, Option<Hash>, Hash)]) -> [u16; LANES] {
    let mut lanes = [0_u16; LANES];
    for (path, key, value) in entries {
        let mut input = Vec::new();
        input.extend_from_slice(path.as_ref());
        input.push(u8::from(key.is_some()));
        input.extend_from_slice(key.as_ref().map_or(&[0; 32][..], |key| key.as_ref()));
        input.extend_from_slice(value.as_ref());
        let mut bytes = vec![0_u8; 2 * LANES];
        blake3::Hasher::new_derive_key("iroha 2026-09-30 world-state lthash16 element v1")
            .update(&input)
            .finalize_xof()
            .fill(&mut bytes);
        for (lane, pair) in lanes.iter_mut().zip(bytes.as_chunks::<2>().0) {
            *lane = lane.wrapping_add(u16::from(pair[0]) | (u16::from(pair[1]) << 8));
        }
    }
    lanes
}

#[test]
fn accumulator_add_and_remove_are_inverse_and_order_independent() {
    let elements: Vec<_> = (0..16).map(sample).collect();
    let mut forward = WorldStateAccumulator::empty();
    for element in &elements {
        forward.add(element);
    }
    let mut backward = WorldStateAccumulator::empty();
    for element in elements.iter().rev() {
        backward.add(element);
    }
    assert_eq!(forward, backward, "the multiset sum ignores entry order");
    assert_eq!(forward.baseline_root(), backward.baseline_root());
    assert_eq!(forward.entries(), 16);
    let mut interleaved = forward.clone();
    interleaved.remove(&elements[3]);
    interleaved.add(&sample(99));
    interleaved.remove(&sample(99));
    interleaved.add(&elements[3]);
    assert_eq!(interleaved, forward, "remove is the exact inverse of add");
    for element in &elements {
        backward.remove(element);
    }
    assert_eq!(backward, WorldStateAccumulator::empty());
    assert_eq!(
        backward.baseline_root(),
        WorldStateAccumulator::empty().baseline_root()
    );
    // Removing before adding wraps and still cancels: the value is a function of the multiset.
    let mut wrapped = WorldStateAccumulator::empty();
    wrapped.remove(&elements[0]);
    assert_eq!(wrapped.entries(), u64::MAX);
    wrapped.add(&elements[0]);
    assert_eq!(wrapped, WorldStateAccumulator::empty());
    // A duplicate entry is not idempotent: multiplicity is part of the multiset.
    let mut twice = WorldStateAccumulator::empty();
    twice.add(&elements[0]);
    twice.add(&elements[0]);
    let mut once = WorldStateAccumulator::empty();
    once.add(&elements[0]);
    assert_ne!(twice.baseline_root(), once.baseline_root());
}

#[test]
fn accumulator_matches_the_documented_formula_and_known_vectors() {
    let path = path_hash("world.test", TABLE).unwrap();
    let entries = [
        (path, Some(Hash::new(b"key a")), Hash::new(b"value a")),
        (path, Some(Hash::new(b"key b")), Hash::new(b"value b")),
        (
            path_hash("world.cell", CELL).unwrap(),
            None,
            Hash::new(b"cell"),
        ),
    ];
    let mut accumulator = WorldStateAccumulator::empty();
    for (path, key, value) in &entries {
        accumulator.add(&element(path, key.as_ref(), value));
    }
    assert_eq!(*accumulator.lanes, documented_sum(&entries));
    // The expansion is BLAKE3 in derive-key mode: pin its first lanes so a change of context,
    // input layout, lane order or width is caught.
    let single = WorldStateAccumulator {
        lanes: Box::new(documented_sum(&entries[..1])),
        entries: 1,
    };
    let mut expected = WorldStateAccumulator::empty();
    expected.add(&element(
        &entries[0].0,
        entries[0].1.as_ref(),
        &entries[0].2,
    ));
    assert_eq!(single, expected);
    let first_lanes: Vec<String> = single.lanes[..4]
        .iter()
        .map(|lane| format!("{lane:04x}"))
        .collect();
    assert_eq!(
        first_lanes.join(" "),
        KNOWN_FIRST_LANES,
        "element expansion changed"
    );
    // The root binds the selection schema, the entry count and every lane, little endian.
    let schema = selection_schema(&baseline_selection()).unwrap();
    let mut preimage = b"iroha:world-state:root:v1\0".to_vec();
    preimage.extend_from_slice(schema.as_ref());
    preimage.extend_from_slice(&3_u64.to_le_bytes());
    for lane in documented_sum(&entries) {
        preimage.extend_from_slice(&lane.to_le_bytes());
    }
    assert_eq!(accumulator.baseline_root(), Ok(Hash::new(preimage)));
    assert_ne!(
        WorldStateAccumulator::empty().baseline_root(),
        accumulator.baseline_root(),
        "the empty World has its own root"
    );
}

/// First four lanes of the element of `(world.test, "key a", "value a")`.
const KNOWN_FIRST_LANES: &str = "26e9 19ce b836 2d68";

#[test]
fn snapshot_json_and_norito_forms_roundtrip_exactly() {
    let mut accumulator = WorldStateAccumulator::empty();
    accumulator.add(&sample(1));
    accumulator.add(&sample(2));
    let json = norito::json::to_json(&accumulator).unwrap();
    assert_eq!(json.len(), WorldStateAccumulator::JSON_HEX + 2);
    let restored: WorldStateAccumulator = norito::json::from_str(&json).unwrap();
    assert_eq!(restored, accumulator);
    assert!(norito::json::from_str::<WorldStateAccumulator>("\"00\"").is_err());
    let payload = norito::codec::Encode::encode(&accumulator);
    assert_eq!(payload.len(), WorldStateAccumulator::PAYLOAD_BYTES);
    assert_eq!(payload[..8], 2_u64.to_le_bytes());
    assert_eq!(payload[8..10], accumulator.lanes[0].to_le_bytes());
    assert_eq!(hex::encode(&payload), json.trim_matches('"'));
    let mut other = accumulator.clone();
    other.add(&sample(3));
    assert_ne!(
        hash_value(&accumulator).unwrap(),
        hash_value(&other).unwrap()
    );
}

#[test]
fn registry_index_covers_every_world_field_exactly_once() {
    let index = field_index().as_ref().unwrap();
    let mut flat = Vec::new();
    flatten(WORLD_FIELDS, &mut flat);
    let mut canonical = 0;
    for field in flat {
        let name = field.id.strip_prefix("world.").unwrap_or(field.id);
        match field.role {
            Role::Canonical(Canonical::Owner(_)) => {
                assert!(!index.by_name.contains_key(name), "{}", field.id);
            }
            Role::Canonical(Canonical::Table { .. } | Canonical::Cell(_)) => {
                canonical += 1;
                assert!(
                    matches!(index.by_name[name], Classified::Canonical { .. }),
                    "{}",
                    field.id
                );
            }
            Role::Derived { .. } | Role::Local(_) | Role::History { .. } => {
                assert!(
                    matches!(index.by_name[name], Classified::Excluded),
                    "{}",
                    field.id
                );
            }
        }
    }
    assert_eq!(index.canonical, canonical);
    assert_eq!(
        canonical,
        WORLD_FIELDS
            .iter()
            .filter(|field| matches!(field.role, Role::Canonical(_)) && field.id != "world.triggers")
            .count()
            + AUTHORITY_FIELDS
                .iter()
                .filter(|field| matches!(field.role, Role::Canonical(_)))
                .count()
    );
    // The accumulator itself is derived World state and never commits to itself.
    assert!(matches!(
        index.by_name["state_accumulator"],
        Classified::Excluded
    ));
    assert!(
        STATE_FIELDS.iter().any(|field| field.id == "state.world"),
        "the World owner is registered in the State inventory"
    );
}

#[test]
fn every_pass_visits_exactly_the_canonical_fields() {
    let index = field_index().as_ref().unwrap();
    let world = World::default();
    let block = world.block();
    // A complete pass over the actual overlay inventory succeeds.
    WorldStateAccumulator::capture(&block).unwrap();
    WorldStateAccumulator::empty().apply_block(&block).unwrap();
    // A pass that skips one canonical field, repeats one or meets an unclassified or
    // mistyped name fails instead of committing a partial World.
    let mut builder = Builder {
        index,
        accumulator: WorldStateAccumulator::empty(),
        direction: Direction::Capture,
        visited: vec![false; index.canonical],
        installation: vec![Installation::Installed; index.canonical],
        snapshot: None,
        snapshot_field: None,
    };
    assert!(
        builder
            .field("smart_contract_state", TABLE)
            .unwrap()
            .is_some()
    );
    assert!(builder.field("smart_contract_state", TABLE).is_err());
    assert!(builder.field("parameters", TABLE).is_err());
    assert!(builder.field("not_registered", TABLE).is_err());
    assert_eq!(builder.field("domains_by_owner", TABLE), Ok(None));
    assert!(builder.finish().is_err());
}

#[test]
fn capture_binds_untouched_and_snapshot_skipped_values_without_undo_history() {
    let first = World::default();
    let second = World::default();
    let plain = WorldStateAccumulator::capture(&first.block()).unwrap();
    {
        let mut setup = second.block();
        setup
            .smart_contract_state
            .insert(path("untouched/value"), vec![1]);
        *setup.soradns_last_publish_ms.get_mut() = Some(42);
        setup.commit();
    }
    let block = second.block();
    let initial = WorldStateAccumulator::capture(&block).unwrap();
    assert_ne!(plain.baseline_root(), initial.baseline_root());
    assert_eq!(
        initial.entries(),
        plain.entries() + 1,
        "one new table entry"
    );
    drop(block);
    // A different undo journal with identical current values cannot change the root.
    let mut noop = second.block();
    noop.smart_contract_state
        .insert(path("untouched/value"), vec![9]);
    noop.smart_contract_state
        .insert(path("untouched/value"), vec![1]);
    *noop.soradns_last_publish_ms.get_mut() = Some(42);
    assert_eq!(
        initial.baseline_root(),
        WorldStateAccumulator::capture(&noop)
            .unwrap()
            .baseline_root()
    );
    assert_eq!(
        initial.baseline_root(),
        initial.apply_block(&noop).unwrap().baseline_root()
    );
}

#[test]
fn incremental_versions_match_cold_capture_across_commit_and_replacement() {
    let world = World::default();
    let parent = WorldStateAccumulator::capture(&world.block()).unwrap();
    let parent_root = parent.baseline_root();
    let mut block = world.block();
    block.smart_contract_state.insert(path("app/kept"), vec![1]);
    block
        .smart_contract_state
        .insert(path("app/removed"), vec![]);
    *block.soradns_last_publish_ms.get_mut() = Some(7);
    {
        let mut aborted = block.smart_contract_state.transaction();
        aborted.insert(path("app/aborted"), vec![9]);
    }
    let first = parent.apply_block(&block).unwrap();
    assert_eq!(
        first.baseline_root(),
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root()
    );
    assert_eq!(
        parent.baseline_root(),
        WorldStateAccumulator::capture_predecessor(&block)
            .unwrap()
            .baseline_root()
    );
    block.commit();
    assert_eq!(
        first.baseline_root(),
        WorldStateAccumulator::capture(&world.block())
            .unwrap()
            .baseline_root()
    );
    // Opening and dropping the intervening read overlay must preserve the undo
    // needed by replacement; the replacement's before cut is the original parent.
    let mut replacement = world.block_and_revert();
    assert_eq!(
        parent_root,
        WorldStateAccumulator::capture(&replacement)
            .unwrap()
            .baseline_root()
    );
    replacement
        .smart_contract_state
        .insert(path("app/replacement"), vec![3]);
    *replacement.soradns_last_publish_ms.get_mut() = Some(8);
    let replaced = parent.apply_block(&replacement).unwrap();
    assert_eq!(
        replaced.baseline_root(),
        WorldStateAccumulator::capture(&replacement)
            .unwrap()
            .baseline_root()
    );
    assert_eq!(
        parent_root,
        WorldStateAccumulator::capture_predecessor(&replacement)
            .unwrap()
            .baseline_root()
    );
    replacement.commit();
    let mut next = world.block();
    next.smart_contract_state.remove(path("app/replacement"));
    *next.soradns_last_publish_ms.get_mut() = None;
    assert_eq!(
        parent_root,
        replaced.apply_block(&next).unwrap().baseline_root()
    );
    assert_eq!(parent_root, parent.baseline_root());
}

#[test]
fn a_foreign_predecessor_yields_a_root_that_differs_from_the_actual_world() {
    let source = World::default();
    let other = World::default();
    {
        let mut setup = other.block();
        setup
            .smart_contract_state
            .insert(path("app/stale"), vec![99]);
        setup.commit();
    }
    let foreign = WorldStateAccumulator::capture(&source.block()).unwrap();
    let mut block = other.block();
    block
        .smart_contract_state
        .insert(path("app/first"), vec![1]);
    block
        .smart_contract_state
        .insert(path("app/stale"), vec![2]);
    // The multiset hash cannot see the missing preimage locally; the certified root does.
    let applied = foreign.apply_block(&block).unwrap();
    let actual = WorldStateAccumulator::capture(&block).unwrap();
    assert_ne!(applied.baseline_root(), actual.baseline_root());
    let actual_parent = WorldStateAccumulator::capture_predecessor(&block).unwrap();
    assert_eq!(
        actual_parent.apply_block(&block).unwrap().baseline_root(),
        actual.baseline_root()
    );
}

#[test]
fn incremental_encoding_is_limited_to_touched_values() {
    let index = field_index().as_ref().unwrap();
    let storage: Storage<u64, Vec<u8>> = (0..512)
        .map(|key| {
            (
                key,
                vec![u8::try_from((key) & 0xff).expect("masked fixture byte fits u8")],
            )
        })
        .collect();
    let mut block = BlockField::new(storage.block());
    let builder = |accumulator, direction| Builder {
        index,
        accumulator,
        direction,
        visited: vec![false; index.canonical],
        installation: vec![Installation::Installed; index.canonical],
        snapshot: None,
        snapshot_field: None,
    };
    let mut initial = builder(WorldStateAccumulator::empty(), Direction::Capture);
    initial
        .append_storage_with("smart_contract_state", &block, hash_value)
        .unwrap();
    let initial = initial.accumulator;
    assert_eq!(initial.entries(), 512);
    block.insert(17, vec![9]);
    let calls = Cell::new(0);
    let mut next = builder(initial.clone(), Direction::Forward);
    next.append_storage_with("smart_contract_state", &block, |value| {
        calls.set(calls.get() + 1);
        hash_value(value)
    })
    .unwrap();
    assert_eq!(
        calls.get(),
        2,
        "encode just one before/after pair, not 512 entries"
    );
    let mut cold = builder(WorldStateAccumulator::empty(), Direction::Capture);
    cold.append_storage_with("smart_contract_state", &block, hash_value)
        .unwrap();
    assert_eq!(next.accumulator, cold.accumulator);
    let mut failed = builder(initial.clone(), Direction::Forward);
    assert!(
        failed
            .append_storage_with("smart_contract_state", &block, |_| Err(
                "injected encoding error".into()
            ))
            .is_err()
    );
    assert_ne!(initial, next.accumulator);
}

#[test]
fn field_identity_and_kind_are_bound_into_each_entry() {
    let index = field_index().as_ref().unwrap();
    let storage: Storage<u64, Vec<u8>> = [(1, vec![1])].into_iter().collect();
    let block = BlockField::new(storage.block());
    let mut roots = Vec::new();
    for name in ["smart_contract_state", "contract_code"] {
        let mut builder = Builder {
            index,
            accumulator: WorldStateAccumulator::empty(),
            direction: Direction::Capture,
            visited: vec![false; index.canonical],
            installation: vec![Installation::Installed; index.canonical],
            snapshot: None,
            snapshot_field: None,
        };
        builder
            .append_storage_with(name, &block, hash_value)
            .unwrap();
        roots.push(builder.accumulator.baseline_root());
    }
    assert_ne!(
        roots[0], roots[1],
        "the same entry in another field differs"
    );
    let cell = mv::cell::Cell::new(Vec::<u8>::new());
    let block = BlockField::new(cell.block());
    let mut builder = Builder {
        index,
        accumulator: WorldStateAccumulator::empty(),
        direction: Direction::Capture,
        visited: vec![false; index.canonical],
        installation: vec![Installation::Installed; index.canonical],
        snapshot: None,
        snapshot_field: None,
    };
    assert!(
        builder
            .append_cell_with("smart_contract_state", &block, hash_value)
            .is_err(),
        "a table cannot be projected as a cell"
    );
}

#[test]
fn trigger_stores_share_the_complete_world_visitor() {
    use crate::smartcontracts::isi::triggers::specialized::{
        SpecializedAction, SpecializedTrigger,
    };
    use iroha_data_model::{events::execute_trigger::ExecuteTriggerEventFilter, prelude::*};
    let world = World::default();
    let mut block = world.block();
    let parent = WorldStateAccumulator::capture(&block).unwrap();
    {
        let mut tx = block.triggers.transaction();
        let action = SpecializedAction::new(
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "accumulator".to_owned(),
                ))]
                .into(),
            ),
            Repeats::Exactly(3),
            iroha_test_samples::ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(
                "accumulator".parse().unwrap(),
                action
            ))
            .unwrap()
        );
        tx.apply();
    }
    let after = parent.apply_block(&block).unwrap();
    assert_eq!(
        after.baseline_root(),
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root()
    );
    assert_ne!(after.baseline_root(), parent.baseline_root());
    assert_eq!(
        parent.baseline_root(),
        WorldStateAccumulator::capture_predecessor(&block)
            .unwrap()
            .baseline_root()
    );
    assert_eq!(
        after.entries(),
        parent.entries() + 1,
        "the action is authoritative; its id and active-id indexes are derived"
    );
}

#[test]
fn unwitnessed_role_and_parameter_changes_change_the_root() {
    use iroha_data_model::{
        parameter::{BlockParameter, Parameter},
        prelude::*,
    };
    let world = World::default();
    let mut block = world.block();
    let parent = WorldStateAccumulator::capture(&block).unwrap();
    let role = RoleId::new("auditor".parse().unwrap());
    block.account_roles.insert(
        crate::role::RoleIdWithOwner::new(iroha_test_samples::ALICE_ID.clone(), role),
        (),
    );
    let granted = parent.apply_block(&block).unwrap();
    assert_ne!(
        granted.baseline_root(),
        parent.baseline_root(),
        "a role grant is committed"
    );
    assert_eq!(
        granted.baseline_root(),
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root()
    );
    block
        .set_parameter_for_testing(Parameter::Block(BlockParameter::MaxTransactions(
            std::num::NonZeroU64::new(7).unwrap(),
        )))
        .expect("valid fixture parameters");
    let parameterized = parent.apply_block(&block).unwrap();
    assert_ne!(
        parameterized.baseline_root(),
        granted.baseline_root(),
        "a parameter is committed"
    );
    assert_eq!(
        parameterized.baseline_root(),
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root()
    );
}

#[test]
fn derived_index_changes_do_not_create_independent_world_authority() {
    use iroha_model_base::domain::DomainId;

    let world = World::default();
    let mut block = world.block();
    let parent = WorldStateAccumulator::capture(&block).unwrap();
    block.domains_by_owner.insert(
        (
            iroha_test_samples::ALICE_ID.clone(),
            DomainId::try_new("derived", "only").unwrap(),
        ),
        (),
    );
    let current = WorldStateAccumulator::capture(&block).unwrap();
    assert_eq!(current.baseline_root(), parent.baseline_root());
    assert_eq!(
        parent.apply_block(&block).unwrap().baseline_root(),
        parent.baseline_root()
    );
    // The stored accumulator is derived too: writing it never changes the root it commits.
    *block.state_accumulator.get_mut() = current.clone();
    assert_eq!(
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root(),
        parent.baseline_root()
    );

    block
        .smart_contract_state
        .insert(path("authority/changed"), vec![1]);
    let changed = WorldStateAccumulator::capture(&block).unwrap();
    assert_ne!(changed.baseline_root(), parent.baseline_root());
    assert_eq!(
        parent.apply_block(&block).unwrap().baseline_root(),
        changed.baseline_root()
    );
}

#[test]
fn musubi_availability_uses_its_semantic_anchor_while_publication_binds_full_row() {
    use iroha_data_model::musubi::{
        ArchiveId, MusubiArchiveAvailabilityV1, MusubiStorageAvailabilityV1,
    };

    let world = World::default();
    let archive_id = ArchiveId::new([0x41; 32]);
    let original = MusubiArchiveAvailabilityV1 {
        archive_id,
        availability: MusubiStorageAvailabilityV1::Unavailable,
        healthy_replicas: 0,
        active_locations: 0,
        finalized_height: 7,
        finalized_block_hash: [0x42; 32],
        index_revision: 9,
    };
    original.validate().unwrap();
    let mut first = world.block();
    first
        .musubi_archive_availability
        .insert(archive_id, original);
    first.commit();

    let mut second = world.block();
    let accumulator = WorldStateAccumulator::capture(&second).unwrap();
    let before_publication = second.publication_state_delta().unwrap();
    let mut derived_change = original;
    derived_change.availability = MusubiStorageAvailabilityV1::BelowQuorum;
    derived_change.healthy_replicas = 1;
    derived_change.active_locations = 1;
    derived_change.validate().unwrap();
    second
        .musubi_archive_availability
        .insert(archive_id, derived_change);
    let after = WorldStateAccumulator::capture(&second).unwrap();
    assert_eq!(after.baseline_root(), accumulator.baseline_root());
    assert_eq!(
        accumulator.apply_block(&second).unwrap().baseline_root(),
        after.baseline_root()
    );
    assert_ne!(
        second.publication_state_delta().unwrap(),
        before_publication
    );

    let mut new_anchor = derived_change;
    new_anchor.index_revision += 1;
    second
        .musubi_archive_availability
        .insert(archive_id, new_anchor);
    assert_ne!(
        WorldStateAccumulator::capture(&second)
            .unwrap()
            .baseline_root(),
        accumulator.baseline_root()
    );
}

/// Per-block cost is proportional to the change set, not to the World: a block that touches
/// ten entries of a large World updates the accumulator far faster than a cold capture, and
/// only the touched values are encoded.
#[test]
fn per_block_cost_is_proportional_to_the_change_set() {
    const ENTRIES: usize = 20_000;
    const TOUCHED: usize = 10;
    let world = World::default();
    {
        let mut setup = world.block();
        for index in 0..ENTRIES {
            setup
                .smart_contract_state
                .insert(path(&format!("large/{index}")), vec![7; 64]);
        }
        setup.commit();
    }
    let started = Instant::now();
    let parent = WorldStateAccumulator::capture(&world.block()).unwrap();
    let cold = started.elapsed();
    let mut block = world.block();
    for index in 0..TOUCHED {
        block
            .smart_contract_state
            .insert(path(&format!("large/{index}")), vec![8; 64]);
    }
    let started = Instant::now();
    let incremental = parent.apply_block(&block).unwrap();
    let per_block = started.elapsed();
    assert_eq!(
        incremental.baseline_root(),
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root()
    );
    println!(
        "world state accumulator: cold capture of {ENTRIES} entries {cold:?}, \
         block touching {TOUCHED} entries {per_block:?}"
    );
    assert!(
        per_block * 20 < cold,
        "incremental update ({per_block:?}) must not scale with the World ({cold:?})"
    );
}

#[test]
fn flattened_memberships_and_counts_do_not_change_world_authority() {
    use iroha_data_model::{
        account::AccountAliasDomain,
        asset::{AssetDefinitionId, AssetId},
        escrow::{AssetEscrowStatus, EscrowId},
        nft::NftId,
        proof::{ProofId, ProofStatus},
        rwa::RwaId,
    };
    use iroha_model_base::{domain::DomainId, topology::DataSpaceId};

    let world = World::default();
    let mut block = world.block();
    let parent = WorldStateAccumulator::capture(&block).unwrap();
    let owner = iroha_test_samples::ALICE_ID.clone();
    let domain = DomainId::try_new("derived", "only").unwrap();
    let definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
    let asset = AssetId::new(definition.clone(), owner.clone());
    let nft = NftId::new(domain.clone(), "badge".parse().unwrap());
    let rwa = RwaId::generated(domain.clone(), Hash::new("derived-rwa"));
    let escrow = EscrowId::new(Hash::new("derived-escrow"));
    let proof = ProofId {
        backend: "derived-proof".to_owned(),
        proof_hash: [7; 32],
    };
    let scope = (
        DataSpaceId::new(0),
        AccountAliasDomain::new("derived".parse().unwrap()),
    );

    block
        .account_scope_accounts
        .insert((scope.clone(), owner.clone()), ());
    block.account_scope_account_counts.insert(scope, 1);
    block
        .domain_asset_definitions
        .insert((domain.clone(), definition.clone()), ());
    block
        .asset_definitions_by_owner
        .insert((owner.clone(), definition.clone()), ());
    block
        .asset_definition_holders
        .insert((definition.clone(), owner.clone()), ());
    block
        .asset_definition_assets
        .insert((definition.clone(), asset.clone()), ());
    block
        .asset_definition_asset_counts
        .insert(definition.clone(), 1);
    block
        .asset_definition_nonzero_holders
        .insert((definition, owner.clone()), ());
    block.assets_by_domain.insert((domain.clone(), asset), ());
    block.nfts_by_owner.insert((owner.clone(), nft.clone()), ());
    block.nfts_by_domain.insert((domain, nft), ());
    block.rwas_by_owner.insert((owner.clone(), rwa.clone()), ());
    block.rwas_by_status.insert((None, rwa.clone()), ());
    block.rwas_by_frozen.insert((false, rwa), ());
    block
        .asset_escrows_by_seller
        .insert((owner.clone(), escrow), ());
    block.asset_escrows_by_buyer.insert((owner, escrow), ());
    block
        .asset_escrows_by_status
        .insert((AssetEscrowStatus::Open, escrow), ());
    block
        .proofs_by_status
        .insert((ProofStatus::Verified, proof.clone()), ());
    block.proofs_by_tag.insert((*b"TEST", proof), ());

    assert_eq!(
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root(),
        parent.baseline_root()
    );
    assert_eq!(
        WorldStateAccumulator::capture_predecessor(&block)
            .unwrap()
            .baseline_root(),
        parent.baseline_root()
    );
    assert_eq!(
        parent.apply_block(&block).unwrap().baseline_root(),
        parent.baseline_root()
    );
}

/// Pin the schema of the baseline selection and the empty World's root under it. A changed
/// baseline storage definition requires an explicit review of both golden digests and of
/// `specs/runtime_baseline_catalog.json`.
#[test]
fn current_world_schema_and_empty_predecessor_match_the_canonical_inventory() {
    let schema = selection_schema(&baseline_selection()).unwrap();
    assert_eq!(
        crate::state::State::native_world_schema_hash_v1().unwrap(),
        schema
    );
    assert_eq!(
        hex::encode_upper(schema.as_ref()),
        "BEFAAB6ECA40248A3533BE72147CFF9CA9893B721998610A20BBE77860C2367D"
    );
    assert_eq!(
        hex::encode_upper(
            WorldStateAccumulator::empty()
                .baseline_root()
                .unwrap()
                .as_ref()
        ),
        "487F15449ABB0C28510EDE9FD6767A3A72725CAC00240CDAEA0FAE9BE2F7FA2F"
    );
}

#[test]
fn canonical_merge_cell_deltas_preserve_nonempty_history_and_exact_undo() {
    let world = World::default();
    let empty = WorldStateAccumulator::capture(&world.block()).unwrap();
    let hints = vec![Hash::new(b"retained canonical hint")];
    let root = Some(Hash::new(b"retained canonical merge value"));
    let mut block = world.block();
    {
        let mut aborted = block.merge_hint_roots.transaction();
        *aborted = hints.clone();
    }
    assert_eq!(
        empty.baseline_root(),
        empty.apply_block(&block).unwrap().baseline_root()
    );
    *block.merge_hint_roots.get_mut() = hints.clone();
    *block.merge_global_state_root.get_mut() = root;
    let populated = empty.apply_block(&block).unwrap();
    assert_ne!(empty.baseline_root(), populated.baseline_root());
    assert_eq!(
        populated.baseline_root(),
        WorldStateAccumulator::capture(&block)
            .unwrap()
            .baseline_root()
    );
    assert_eq!(
        empty.baseline_root(),
        WorldStateAccumulator::capture_predecessor(&block)
            .unwrap()
            .baseline_root()
    );
    block.commit();
    // A later block with no metadata writer retains both canonical values and their roots.
    {
        let mut next = world.block();
        assert_eq!(next.merge_hint_roots.as_slice(), hints.as_slice());
        assert_eq!(**next.merge_global_state_root, root);
        *next.merge_hint_roots.get_mut() = hints.clone();
        *next.merge_global_state_root.get_mut() = root;
        assert_eq!(
            populated.baseline_root(),
            populated.apply_block(&next).unwrap().baseline_root(),
            "same-value touches preserve the canonical semantic root"
        );
    }
    let replacement = world.block_and_revert();
    assert!(replacement.merge_hint_roots.is_empty());
    assert_eq!(**replacement.merge_global_state_root, None);
    assert_eq!(
        empty.baseline_root(),
        WorldStateAccumulator::capture(&replacement)
            .unwrap()
            .baseline_root()
    );
}

/// Keep the historical signed bytes as a negative schema-binding control.
/// Decoding a carrier cannot make its retired predecessor a current World root.
#[test]
fn retired_schema_genesis_does_not_bind_the_current_empty_world() {
    use iroha_data_model::{
        block::decode_framed_signed_block,
        sumeragi_finality::{CertifiedResultScope, CertifiedResultV1},
    };
    use sha2::{Digest, Sha256};
    let wire = include_bytes!("../../tests/fixtures/taira_world_continuity/genesis.block.nrt");
    assert_eq!(wire.len(), 114_919);
    assert_eq!(
        hex::encode(Sha256::digest(wire)),
        "35565d5b041ee144cc5922451423544037f82d2259788c3a9bc2009fc80e8f5c"
    );
    let genesis = decode_framed_signed_block(wire).expect("exact historical native frame");
    assert!(genesis.header().is_genesis());
    let certificate = genesis
        .commit_certificate()
        .expect("retained result-only certificate");
    assert!(certificate.consensus_header().is_empty());
    assert!(certificate.commit_qc().is_empty());
    assert!(certificate.availability().is_empty());
    let retained = CertifiedResultV1::decode(
        certificate.result_preimage(),
        CertifiedResultScope::TairaGlobal,
    )
    .expect("original typed certified result preimage");
    assert_eq!(retained.commitment().height, 1);
    assert_eq!(
        hex::encode_upper(
            retained
                .commitment()
                .execution
                .parent_world_state_root
                .as_ref()
        ),
        "D3026F30932BBC181F5243B72C9FC2E4FC6C729C718BBCC55BD2554C527341AD",
        "the original signature binds its original predecessor; the fixture is never rewritten"
    );
    assert_ne!(
        retained.commitment().execution.parent_world_state_root,
        WorldStateAccumulator::empty().baseline_root().unwrap(),
        "retired privacy-table layouts cannot establish a current first-release predecessor"
    );
    assert_eq!(
        genesis.encode_wire().unwrap().as_slice(),
        wire.as_slice(),
        "no carrier conversion or re-signing"
    );
}

/// Genuine genesis binds the reviewed schema; its successor binds the finalized World.
#[test]
fn current_certified_genesis_binds_the_current_empty_world() {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
        .expect("current signed genesis executes and authenticates");
    let genesis = chain.committed(1);
    assert_eq!(genesis.commitment().height, 1);
    assert_eq!(
        genesis.commitment().execution.parent_world_state_root,
        WorldStateAccumulator::empty().baseline_root().unwrap()
    );
    // Finalization publishes certificate-dependent state after the execution root.
    // The next certified result binds that complete published World as its parent.
    let finalized = chain.state().verify_world_state_accumulator().unwrap();
    chain.commit(Vec::new());
    assert_eq!(
        chain
            .committed(2)
            .commitment()
            .execution
            .parent_world_state_root,
        finalized
    );
}

/// `S` of a selection binds its commitment profile and installed storage in identity order.
#[test]
fn selection_schema_folds_its_profile_and_installed_storage() {
    let selection = baseline_selection();
    let mut schema = Hash::new_from_chunks(&[SCHEMA_SELECTION, &selection.commitment_profile.0]);
    for storage in &selection.installed_storage {
        schema = Hash::new_from_chunks(&[SCHEMA_FIELD, schema.as_ref(), &storage.0]);
    }
    let reference = Hash::new_from_chunks(&[
        SCHEMA_SELECTION,
        schema.as_ref(),
        &u64::try_from(selection.installed_storage.len())
            .unwrap()
            .to_le_bytes(),
    ]);
    assert_eq!(selection_schema(&selection).unwrap(), reference);
}

#[test]
fn ironwood_accumulator_uses_process_wide_storage_validation_cache() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut selection = catalog.ironwood.as_ref().unwrap().selection.clone();
    // A distinct, closure-valid identity avoids relying on other tests' cache entries.
    selection.completion_closures =
        vec![iroha_data_model::runtime::definition::CompletionClosureV1 {
            obligation: Hash::new(b"ironwood-accumulator-global-cache-selection").into(),
            definitions: vec![selection.services_profile],
        }];
    selection.check_closure(&catalog.definitions).unwrap();
    let before = crate::runtime::ironwood::STORAGE_VALIDATION_MISSES.with(|v| v.get());
    let index = field_index().as_ref().unwrap();
    for _ in 0..3 {
        installed_slots(index, &selection).unwrap();
    }
    let after = crate::runtime::ironwood::STORAGE_VALIDATION_MISSES.with(|v| v.get());
    assert_eq!(
        after - before,
        1,
        "all accumulator passes use the real global cache"
    );
}

#[test]
fn sorafs_authority_storage_preserves_old_root_and_binds_every_selected_table() {
    use crate::smartcontracts::isi::sorafs_authority::{
        SorafsAuthorityHeadV1, SorafsAuthorityLeaseV1, SorafsAuthorityProposalRecordV1,
    };
    use iroha_data_model::{
        governance::types::{GovernanceAttemptId, GovernanceCertificateId},
        isi::sorafs_authority::{
            SorafsAuthorityActionV1, SorafsAuthorityProposalV1, SorafsAuthorityTermGridV1,
            SorafsOperatingAuthorityV1,
        },
        runtime::lifecycle::RootScopeV1,
    };
    let world = World::default();
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut block = world.block();
    let before = WorldStateAccumulator::capture(&block).unwrap();
    let old_root = before.root(&catalog.baseline).unwrap();
    let proposal = SorafsAuthorityProposalV1 {
        grid: SorafsAuthorityTermGridV1 {
            epoch_length_blocks: 100,
        },
        term: 1,
        network: iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed([0x71; 32])),
        ),
        root: RootScopeV1::Universal,
        account: iroha_test_samples::ALICE_ID.clone(),
        authority: SorafsOperatingAuthorityV1::SetReservePolicy,
        action: SorafsAuthorityActionV1::Grant,
        expected_revision: 0,
        completion_policy: None,
    };
    // Canonical storage rows exercise commitment ownership only. The fabricated
    // receipt identifiers here never authorize execution or satisfy restore.
    let content = proposal.proposal_content_id();
    let record = SorafsAuthorityProposalRecordV1 {
        proposer: proposal.account.clone(),
        proposal: proposal.clone(),
        created_height: 1,
        status: crate::state::GovernanceProposalStatus::Proposed,
        deadline_closure: None,
    };
    let head = SorafsAuthorityHeadV1 {
        plan_id: proposal.plan_id(),
        term: 1,
        expires_at_height: 12_801,
        authority: proposal.authority,
        account: proposal.account.clone(),
        revision: 2,
        granted: false,
        proposal_content_id: content,
        governance_attempt_id: GovernanceAttemptId::derive_v1(content, 0),
        certificate_id: GovernanceCertificateId::new([0x71; 32]),
        enacted_at_height: 1,
    };
    let lease = SorafsAuthorityLeaseV1 {
        plan: proposal.clone(),
        grant_approval: None,
        terminal_approval: None,
        activation: None,
        completion: None,
        start_processed: false,
    };
    let fields = [
        "world.sorafs_authority_proposals",
        "world.sorafs_authority_heads",
        "world.sorafs_authority_leases",
    ];
    let index = field_index().as_ref().unwrap();
    let old_slots = installed_slots(index, &catalog.baseline).unwrap();
    for field in fields {
        assert!(!old_slots[index.ids.binary_search(&field).unwrap()]);
    }
    let published = crate::runtime::change_set::PublishedDefinitions(&block.runtime_definitions);
    let change = catalog
        .activation(
            crate::runtime::catalog::additions::SORAFS_AUTHORITY.module,
            block.runtime_selection.get(),
            &published,
            proposal.network,
            1,
            [0x71; 32],
        )
        .unwrap();
    let applied = change
        .apply(block.runtime_selection.get(), &published)
        .unwrap();
    for (id, descriptor) in applied.published {
        block.runtime_definitions.insert(id, descriptor);
    }
    *block.runtime_selection.get_mut() = applied.selection;
    let selected_slots = installed_slots(index, block.runtime_selection.get()).unwrap();
    for field in fields {
        assert!(selected_slots[index.ids.binary_search(&field).unwrap()]);
    }
    let selected_empty = WorldStateAccumulator::capture(&block).unwrap();
    for (which, field) in fields.iter().enumerate() {
        let key = [0x71; 32];
        match which {
            0 => {
                block.sorafs_authority_proposals.insert(key, record.clone());
            }
            1 => {
                block.sorafs_authority_heads.insert(key, head.clone());
            }
            2 => {
                block.sorafs_authority_leases.insert(key, lease.clone());
            }
            _ => unreachable!(),
        }
        let after = before.apply_block(&block).unwrap();
        assert_eq!(after, WorldStateAccumulator::capture(&block).unwrap());
        assert_ne!(after, selected_empty, "selected {field} is committed");
        assert_eq!(
            old_root,
            WorldStateAccumulator::capture_predecessor(&block)
                .unwrap()
                .root(&catalog.baseline)
                .unwrap()
        );
        match which {
            0 => {
                block.sorafs_authority_proposals.remove(key);
            }
            1 => {
                block.sorafs_authority_heads.remove(key);
            }
            2 => {
                block.sorafs_authority_leases.remove(key);
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(selected_empty, before.apply_block(&block).unwrap());
    assert_eq!(old_root, before.root(&catalog.baseline).unwrap());
}
