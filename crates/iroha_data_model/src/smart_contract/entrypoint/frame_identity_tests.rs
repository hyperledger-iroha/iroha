//! Original compiler frames for public entrypoint schemas and their exact limits.

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../fixtures/kotodama/compiler_frame_identity_v1.json");

fn fixture_groups(owner: &str) -> Vec<norito::json::Value> {
    let fixture: norito::json::Value = norito::json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture["schema"].as_str(),
        Some("iroha.compiler.frame-observations.v1")
    );
    assert_eq!(
        fixture["layout_flags"].as_u64(),
        Some(u64::from(norito::core::default_encode_flags()))
    );
    fixture["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|group| group["owner"].as_str() == Some(owner))
        .cloned()
        .collect()
}

fn check_frame<T>(row: &norito::json::Value, value: T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + std::fmt::Debug + Eq,
{
    assert_eq!(T::nominal_name(), row["nominal"].as_str().unwrap());
    assert_eq!(T::frame_name(), row["nominal"].as_str().unwrap());
    let frame_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(
        hex::encode(frame_hash),
        row["serialize_schema_hash"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(frame_hash),
        row["deserialize_schema_hash"].as_str().unwrap()
    );
    let captured = hex::decode(row["frame_hex"].as_str().unwrap()).unwrap();
    let header = norito::core::Header::read(captured.as_slice()).unwrap();
    assert_eq!(header.schema, frame_hash);
    // An unrelated ambient layout must not change the canonical ABI boundary.
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(norito::encode_canonical(&value).unwrap(), captured);
    assert_eq!(norito::decode_canonical::<T>(&captured).unwrap(), value);
    for length in [0, captured.len() / 2, captured.len() - 1] {
        assert!(norito::decode_canonical::<T>(&captured[..length]).is_err());
    }
    let mut trailing = captured.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let mut corrupt = captured;
    corrupt[0] ^= 1;
    assert!(norito::decode_canonical::<T>(&corrupt).is_err());
}

fn check_group<T>(group: &norito::json::Value, value: T)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + Clone
        + std::fmt::Debug
        + Eq,
{
    let frames = group["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 5);
    for (row, expected) in
        frames
            .iter()
            .zip(["root", "option_none", "option_some", "vec_empty", "vec_two"])
    {
        assert_eq!(row["shape"].as_str(), Some(expected));
    }
    check_frame(&frames[0], value.clone());
    check_frame(&frames[1], None::<T>);
    check_frame(&frames[2], Some(value.clone()));
    check_frame(&frames[3], Vec::<T>::new());
    check_frame(&frames[4], vec![value.clone(), value]);
}

fn boolean() -> EntrypointValueTypeV1 {
    EntrypointValueTypeV1 {
        nodes: vec![EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool)],
    }
}

fn maximum_arguments() -> EntrypointArgumentSchemaV1 {
    EntrypointArgumentSchemaV1 {
        fields: (0..MAX_ENTRYPOINT_ARGUMENTS)
            .map(|index| EntrypointArgumentFieldV1 {
                name: format!("field_{index}"),
                ty: boolean(),
            })
            .collect(),
    }
}

#[test]
fn original_compiler_schema_frames_are_exact() {
    let groups = fixture_groups("entrypoint_argument_schema");
    assert_eq!(groups.len(), 3);
    for group in groups {
        let value = match group["case"].as_str().unwrap() {
            "scalar" => EntrypointArgumentSchemaV1 {
                fields: vec![EntrypointArgumentFieldV1 {
                    name: "value".into(),
                    ty: boolean(),
                }],
            },
            "optional" => EntrypointArgumentSchemaV1 {
                fields: vec![EntrypointArgumentFieldV1 {
                    name: "value".into(),
                    ty: EntrypointValueTypeV1 {
                        nodes: vec![
                            EntrypointValueTypeNodeV1::Option,
                            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
                        ],
                    },
                }],
            },
            "maximum" => maximum_arguments(),
            case => panic!("unexpected captured schema case: {case}"),
        };
        assert!(value.validate());
        check_group(&group, value);
    }
}

#[test]
fn maximum_schema_ownership_and_bounds_survive_framing() {
    let maximum = maximum_arguments();
    assert_eq!(maximum.fields.len(), 13);
    assert_eq!(maximum.word_count(), Some(13));
    let schema = EntrypointArgumentSchemaV1::schema();
    assert!(schema.contains_key::<EntrypointArgumentSchemaV1>());
    assert!(schema.contains_key::<EntrypointArgumentFieldV1>());
    let encoded = norito::encode_canonical(&maximum).unwrap();
    let decoded: EntrypointArgumentSchemaV1 = norito::decode_canonical(&encoded).unwrap();
    assert_eq!(decoded, maximum);
    assert!(decoded.validate());
    let mut material = ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1.to_vec();
    material.extend_from_slice(&encoded);
    assert_eq!(
        entrypoint_argument_schema_hash_v1(&encoded),
        *Hash::new(material).as_ref()
    );
    let mut too_many = decoded.clone();
    too_many.fields.push(EntrypointArgumentFieldV1 {
        name: "overflow".into(),
        ty: boolean(),
    });
    assert!(!too_many.validate());
    assert_eq!(too_many.word_count(), None);
    let mut duplicate = decoded.clone();
    duplicate.fields[1].name = duplicate.fields[0].name.clone();
    assert!(!duplicate.validate());
    let at_depth = EntrypointArgumentSchemaV1 {
        fields: vec![EntrypointArgumentFieldV1 {
            name: "value".into(),
            ty: EntrypointValueTypeV1 {
                nodes: std::iter::repeat_n(
                    EntrypointValueTypeNodeV1::Option,
                    MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1,
                )
                .chain([EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool)])
                .collect(),
            },
        }],
    };
    assert!(at_depth.validate());
    let deep_bytes = norito::encode_canonical(&at_depth).unwrap();
    let deep_decoded: EntrypointArgumentSchemaV1 = norito::decode_canonical(&deep_bytes).unwrap();
    assert_eq!(deep_decoded, at_depth);
    let mut too_deep = at_depth.clone();
    too_deep.fields[0]
        .ty
        .nodes
        .insert(0, EntrypointValueTypeNodeV1::Option);
    assert!(!too_deep.validate());
    let invalid = norito::encode_canonical(&too_deep).unwrap();
    assert!(norito::decode_canonical::<EntrypointArgumentSchemaV1>(&invalid).is_err());
    drop((
        maximum,
        decoded,
        too_many,
        duplicate,
        at_depth,
        deep_decoded,
        too_deep,
        schema,
    ));
}
