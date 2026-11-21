//! Independent release evidence for the recursively described layouts of installed baseline storage.
//!
//! These hashes are local drift guards, never definition IDs, commitment inputs or upgrade authority.
//! Native schemas retain every reachable field, enum discriminant and container. Borrowed semantic
//! projections retain their owned DTO schema; separate encoder/DTO conformance tests cover their
//! manual forwarding. These hashes do not prove arbitrary manual codec semantics. Creating the
//! first fixture captures this checkout; it does not retroactively qualify an earlier release.

use std::{collections::BTreeMap, fmt::Write as _, io::Write as _};

use iroha_data_model::runtime::definition::{DefinitionDescriptorV1, StorageShapeV1};
use iroha_schema::IntoSchema as _;

use super::{Canonical, Field, Role, Schema, WORLD_FIELDS};

const PINNED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../specs/runtime_baseline_storage_layouts.tsv"
);

mod structure;

fn semantic_structure(identity: &str) -> iroha_schema::MetaMap {
    use super::world::{
        musubi_availability_policy::MusubiAvailabilityAuthorityV1,
        musubi_universal_policy::{MusubiDirectoryAuthorityV1, MusubiResolverAuthorityV1},
    };
    match identity {
        "iroha:state:executor-semantic:v1" => {
            crate::executor::executor_norito::state_authority_structure()
        }
        "iroha:state:musubi-availability-authority:v1" => MusubiAvailabilityAuthorityV1::schema(),
        "iroha:state:musubi-resolver-authority:v1" => MusubiResolverAuthorityV1::schema(),
        "iroha:state:musubi-directory-authority:v1" => MusubiDirectoryAuthorityV1::schema(),
        _ => crate::smartcontracts::isi::triggers::set::world_authority_structure(identity)
            .unwrap_or_else(|| {
                panic!("semantic projection {identity} has no structural evidence owner")
            }),
    }
}

fn schema_evidence(field: &'static str, schema: Schema) -> (String, String) {
    let codec =
        super::keyed_commitment::codec_identity(field, schema).expect("resolved storage codec");
    let structure = match schema {
        Schema::Norito { structure, .. } => {
            structure.unwrap_or_else(|| panic!("{field} has no typed structural schema"))()
        }
        Schema::Semantic { identity, .. } => semantic_structure(identity),
        Schema::Required { .. } => panic!("{field} has no canonical storage codec"),
    };
    (codec, structure::digest(structure))
}

fn flatten(fields: &'static [Field], out: &mut BTreeMap<&'static str, &'static Field>) {
    for field in fields {
        assert!(
            out.insert(field.id, field).is_none(),
            "duplicate State authority identity"
        );
        if let Role::Canonical(Canonical::Owner(children)) = field.role {
            flatten(children, out);
        }
    }
}

fn baseline_storage_layouts() -> String {
    let catalog = crate::runtime::release_catalog().expect("release catalog");
    let mut fields = BTreeMap::new();
    flatten(WORLD_FIELDS, &mut fields);
    let mut rows = BTreeMap::new();
    for id in &catalog.baseline.installed_storage {
        let DefinitionDescriptorV1::Storage(storage) = &catalog.definitions[id] else {
            panic!("baseline installed identity is not storage");
        };
        let field = fields
            .get(storage.namespace.as_str())
            .expect("baseline storage authority owner");
        let (key, value) = match field.role {
            Role::Canonical(Canonical::Table { key, value }) => {
                assert!(matches!(storage.shape, StorageShapeV1::Table { .. }));
                (Some(key), value)
            }
            Role::Canonical(Canonical::Cell(value)) => {
                assert!(matches!(storage.shape, StorageShapeV1::Cell));
                (None, value)
            }
            _ => panic!("baseline storage is not a canonical table or cell"),
        };
        for (position, schema) in key
            .map(|key| ("key", key))
            .into_iter()
            .chain([("value", value)])
        {
            let (codec, digest) = schema_evidence(field.id, schema);
            assert!(rows.insert((field.id, position), (codec, digest)).is_none());
        }
    }
    assert!(!rows.is_empty());
    let mut output = String::from(
        "# Baseline installed storage; local structural evidence only. Never a definition or commitment input.\n# namespace\tposition\tdeclared_codec\tsha256_recursive_structure\n",
    );
    for ((namespace, position), (codec, digest)) in rows {
        writeln!(output, "{namespace}\t{position}\t{codec}\t{digest}").unwrap();
    }
    output
}

#[test]
fn baseline_storage_layouts_stay_frozen() {
    let expected =
        std::fs::read_to_string(PINNED).expect("baseline storage structural evidence fixture");
    let actual = baseline_storage_layouts();
    let expected_rows: BTreeMap<_, _> = expected
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut columns = line.splitn(3, '\t');
            (
                (columns.next().unwrap(), columns.next().unwrap()),
                columns.next().unwrap(),
            )
        })
        .collect();
    let changed: Vec<_> = actual
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut columns = line.splitn(3, '\t');
            let key = (columns.next().unwrap(), columns.next().unwrap());
            (expected_rows.get(&key).copied() != columns.next()).then_some(key)
        })
        .collect();
    assert!(
        actual == expected,
        "baseline storage layout drift at {changed:?}; preserve its existing codec or publish a governed storage successor and migration. This fixture is independent of frozen descriptor IDs and must not be regenerated to conceal drift"
    );
}

#[test]
fn every_canonical_world_schema_has_closed_structural_evidence() {
    // Additions need complete owner graphs too, even while they are dormant and
    // therefore absent from the immutable baseline fixture.
    let mut fields = BTreeMap::new();
    flatten(WORLD_FIELDS, &mut fields);
    for field in fields.into_values() {
        match field.role {
            Role::Canonical(Canonical::Table { key, value }) => {
                schema_evidence(field.id, key);
                schema_evidence(field.id, value);
            }
            Role::Canonical(Canonical::Cell(value)) => {
                schema_evidence(field.id, value);
            }
            _ => {}
        }
    }
}

#[test]
#[ignore = "creates the first local storage-layout evidence fixture only; refuses to replace existing evidence"]
fn capture_runtime_baseline_storage_layouts() {
    let evidence = baseline_storage_layouts();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(PINNED)
        .expect("create first evidence fixture; existing evidence must not be overwritten");
    file.write_all(evidence.as_bytes())
        .expect("write structural evidence");
}

#[test]
#[should_panic(expected = "has no structural evidence owner")]
fn an_unowned_semantic_projection_cannot_pass_on_its_nominal_identity() {
    semantic_structure("iroha:state:unowned-test-projection:v1");
}

#[test]
#[should_panic(expected = "has no typed structural schema")]
fn a_native_codec_without_structural_evidence_cannot_pass_on_its_nominal_identity() {
    schema_evidence("world.unowned_test", super::schema::<bool>());
}
