//! Canonical recursive schema evidence, independent of the State registry and catalog.
//!
//! Stable identities name exact metadata shapes; conflicting identities fail closed. Every
//! metadata token is separately framed by Norito, so names never act as evidence delimiters.

use iroha_schema::IntoSchema as _;
use std::collections::BTreeMap;

pub(super) fn digest(schema: iroha_schema::MetaMap) -> String {
    use iroha_schema::{FloatMode, IntMode, Metadata};
    use sha2::{Digest, Sha256};
    let ids: BTreeMap<core::any::TypeId, &str> = schema
        .iter()
        .map(|(rust_type_id, entry)| (*rust_type_id, entry.type_id.as_str()))
        .collect();
    let id = |rust_type_id: &core::any::TypeId| {
        ids.get(rust_type_id)
            .expect("a schema references only types of its own map")
            .to_string()
    };
    // Every token is a separate canonical Norito string. Tags, arities and option markers
    // make the grammar explicit; user-declared names never act as separators or syntax.
    let mut unique = BTreeMap::new();
    for (_, entry) in schema.iter() {
        let mut row = vec![entry.type_id.clone(), entry.type_name.clone()];
        match &entry.metadata {
            Metadata::Struct(fields) => {
                row.extend(["struct".to_owned(), fields.declarations.len().to_string()]);
                for field in &fields.declarations {
                    row.extend([field.name.clone(), id(&field.ty)]);
                }
            }
            Metadata::Tuple(fields) => {
                row.extend(["tuple".to_owned(), fields.types.len().to_string()]);
                row.extend(fields.types.iter().map(id));
            }
            Metadata::Enum(variants) => {
                row.extend(["enum".to_owned(), variants.variants.len().to_string()]);
                for variant in &variants.variants {
                    row.extend([variant.tag.clone(), variant.discriminant.to_string()]);
                    match &variant.ty {
                        Some(ty) => row.extend(["some".to_owned(), id(ty)]),
                        None => row.push("none".to_owned()),
                    }
                }
            }
            Metadata::Int(IntMode::FixedWidth) => row.push("fixed-int".to_owned()),
            Metadata::Int(IntMode::Compact) => row.push("compact-int".to_owned()),
            Metadata::Float(FloatMode::Binary32) => row.push("binary32".to_owned()),
            Metadata::Float(FloatMode::Binary64) => row.push("binary64".to_owned()),
            Metadata::String => row.push("string".to_owned()),
            Metadata::Bool => row.push("bool".to_owned()),
            Metadata::FixedPoint(fixed) => row.extend([
                "fixed".to_owned(),
                id(&fixed.base),
                fixed.decimal_places.to_string(),
            ]),
            Metadata::Array(array) => {
                row.extend(["array".to_owned(), id(&array.ty), array.len.to_string()])
            }
            Metadata::Vec(vector) => row.extend(["vec".to_owned(), id(&vector.ty)]),
            Metadata::Map(map) => row.extend(["map".to_owned(), id(&map.key), id(&map.value)]),
            Metadata::Option(ty) => row.extend(["option".to_owned(), id(ty)]),
            Metadata::Result(result) => {
                row.extend(["result".to_owned(), id(&result.ok), id(&result.err)])
            }
            Metadata::Bitmap(bitmap) => {
                row.extend([
                    "bitmap".to_owned(),
                    id(&bitmap.repr),
                    bitmap.masks.len().to_string(),
                ]);
                for mask in &bitmap.masks {
                    row.extend([mask.name.clone(), mask.mask.to_string()]);
                }
            }
        }
        // A graph reference names exactly one shape. Exact aliases such as String and
        // Box<str> collapse; conflicting stable identities cannot conceal a reference swap.
        if let Some(previous) = unique.get(entry.type_id.as_str()) {
            assert_eq!(
                previous, &row,
                "conflicting structural schemas share stable identity {}",
                entry.type_id
            );
        } else {
            unique.insert(entry.type_id.as_str(), row);
        }
    }
    let rows: Vec<Vec<String>> = unique.into_values().collect();
    let bytes = norito::encode_canonical(&rows).expect("canonical structural evidence");
    let mut hash = Sha256::new();
    hash.update(b"iroha:runtime:structural-storage-evidence:v1\0");
    hash.update(bytes);
    hex::encode(hash.finalize())
}

#[test]
fn structural_evidence_detects_a_nested_change_under_the_same_nominal_names() {
    use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId};
    struct Root;
    struct Leaf;
    impl TypeId for Root {
        fn id() -> String {
            "root".to_owned()
        }
    }
    impl TypeId for Leaf {
        fn id() -> String {
            "leaf".to_owned()
        }
    }
    impl IntoSchema for Root {
        fn type_name() -> String {
            "Root".to_owned()
        }
        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Vec(iroha_schema::VecMeta {
                ty: core::any::TypeId::of::<Leaf>(),
            }));
            Leaf::update_schema_map(map);
        }
    }
    impl IntoSchema for Leaf {
        fn type_name() -> String {
            "Leaf".to_owned()
        }
        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Bool);
        }
    }
    let original = Root::schema();
    let mut changed = Root::schema();
    changed.insert::<Leaf>(Metadata::String);
    assert_ne!(digest(original), digest(changed));
    assert_eq!(digest(Root::schema()), digest(Root::schema()));
}

fn conflicting_leaf_schema(swapped: bool) -> iroha_schema::MetaMap {
    use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
    struct Root;
    struct BoolLeaf;
    struct StringLeaf;
    impl TypeId for Root {
        fn id() -> String {
            "root".to_owned()
        }
    }
    impl TypeId for BoolLeaf {
        fn id() -> String {
            "leaf".to_owned()
        }
    }
    impl TypeId for StringLeaf {
        fn id() -> String {
            "leaf".to_owned()
        }
    }
    impl IntoSchema for Root {
        fn type_name() -> String {
            "Root".to_owned()
        }
        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta {
                types: vec![
                    core::any::TypeId::of::<BoolLeaf>(),
                    core::any::TypeId::of::<StringLeaf>(),
                ],
            }));
            BoolLeaf::update_schema_map(map);
            StringLeaf::update_schema_map(map);
        }
    }
    impl IntoSchema for BoolLeaf {
        fn type_name() -> String {
            "Leaf".to_owned()
        }
        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Bool);
        }
    }
    impl IntoSchema for StringLeaf {
        fn type_name() -> String {
            "Leaf".to_owned()
        }
        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::String);
        }
    }
    let mut schema = Root::schema();
    if swapped {
        schema.insert::<Root>(Metadata::Tuple(UnnamedFieldsMeta {
            types: vec![
                core::any::TypeId::of::<StringLeaf>(),
                core::any::TypeId::of::<BoolLeaf>(),
            ],
        }));
    }
    schema
}

#[test]
#[should_panic(expected = "conflicting structural schemas share stable identity leaf")]
fn conflicting_type_identities_are_rejected_before_a_reference_swap() {
    digest(conflicting_leaf_schema(false));
}

#[test]
#[should_panic(expected = "conflicting structural schemas share stable identity leaf")]
fn swapping_conflicting_same_name_references_cannot_hide_layout_drift() {
    digest(conflicting_leaf_schema(true));
}

#[test]
fn exact_same_shape_aliases_remain_unambiguous() {
    let original = String::schema();
    let mut with_alias = String::schema();
    <Box<str>>::update_schema_map(&mut with_alias);
    assert_eq!(digest(original), digest(with_alias));
}

#[test]
fn schema_identity_delimiters_cannot_alias_different_reference_lists() {
    use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
    struct Root;
    struct Left;
    struct Right;
    struct OtherLeft;
    struct OtherRight;
    macro_rules! leaf {
        ($ty:ty, $id:literal, $shape:expr) => {
            impl TypeId for $ty {
                fn id() -> String {
                    $id.to_owned()
                }
            }
            impl IntoSchema for $ty {
                fn type_name() -> String {
                    "Leaf".to_owned()
                }
                fn update_schema_map(map: &mut MetaMap) {
                    map.insert::<Self>($shape);
                }
            }
        };
    }
    leaf!(Left, "left,inner\tname", Metadata::Bool);
    leaf!(Right, "right\nleaf", Metadata::Bool);
    leaf!(OtherLeft, "left", Metadata::String);
    leaf!(OtherRight, "inner\tname,right\nleaf", Metadata::Bool);
    impl TypeId for Root {
        fn id() -> String {
            "root".to_owned()
        }
    }
    impl IntoSchema for Root {
        fn type_name() -> String {
            "Root".to_owned()
        }
        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta {
                types: vec![
                    core::any::TypeId::of::<Left>(),
                    core::any::TypeId::of::<Right>(),
                ],
            }));
            Left::update_schema_map(map);
            Right::update_schema_map(map);
            OtherLeft::update_schema_map(map);
            OtherRight::update_schema_map(map);
        }
    }
    // Both reference lists collapse to the same old comma-joined representation, while
    // one tuple encodes (bool, bool) and the other encodes (String, bool).
    assert_eq!(
        format!("{},{}", Left::id(), Right::id()),
        format!("{},{}", OtherLeft::id(), OtherRight::id())
    );
    let original = Root::schema();
    let mut changed = Root::schema();
    changed.insert::<Root>(Metadata::Tuple(UnnamedFieldsMeta {
        types: vec![
            core::any::TypeId::of::<OtherLeft>(),
            core::any::TypeId::of::<OtherRight>(),
        ],
    }));
    assert_ne!(digest(original), digest(changed));
}
