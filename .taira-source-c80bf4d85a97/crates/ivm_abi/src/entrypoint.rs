//! Canonical ABI records used at public Kotodama entrypoint boundaries.
//!
//! The data model owns these types because the same exact recursive schema is
//! embedded in CNTR metadata, copied into signed manifests, and consumed by
//! clients. Re-exporting it here keeps VM and compiler call sites concise while
//! preventing a second ABI model from drifting.

pub use iroha_data_model::smart_contract::entrypoint::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_unknown_schema_field(value: norito::json::Value, expected: &str) {
        let error = norito::json::from_value::<EntrypointValueTypeV1>(value)
            .expect_err("unknown V1 entrypoint schema field must reject");
        match error {
            norito::json::Error::UnknownField { field } => assert_eq!(field, expected),
            other => panic!("expected unknown field `{expected}`, got {other:?}"),
        }
    }

    fn leaf() -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int)],
        }
    }

    fn nested_list_schema(list_count: usize) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(list_count.saturating_add(1));
        nodes.extend((0..list_count).map(|_| {
            EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: MIN_ENTRYPOINT_LIST_CAPACITY_V1,
            })
        }));
        nodes.push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int));
        EntrypointValueTypeV1 { nodes }
    }

    fn nested_list_schema_json_value(list_count: usize) -> norito::json::Value {
        norito::json::to_value(&nested_list_schema(list_count))
            .expect("convert flat preorder schema to JSON value")
    }

    #[test]
    fn flat_schema_binary_decode_enforces_the_v1_boundary() {
        let at_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1);
        let bytes = norito::to_bytes(&at_limit).expect("encode schema at V1 limit");
        assert_eq!(
            norito::decode_from_bytes::<EntrypointValueTypeV1>(&bytes)
                .expect("decode schema at V1 limit"),
            at_limit
        );

        let over_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        let bytes = norito::to_bytes(&over_limit).expect("encode over-limit schema fixture");
        assert!(norito::decode_from_bytes::<EntrypointValueTypeV1>(&bytes).is_err());
    }

    #[test]
    fn flat_list_schema_requires_exactly_one_inline_element_subtree() {
        let missing = EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: MIN_ENTRYPOINT_LIST_CAPACITY_V1,
            })],
        };
        let bytes = norito::to_bytes(&missing).expect("encode missing-element schema fixture");
        assert!(norito::decode_from_bytes::<EntrypointValueTypeV1>(&bytes).is_err());

        let mut trailing = nested_list_schema(1);
        trailing
            .nodes
            .push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool));
        let bytes = norito::to_bytes(&trailing).expect("encode trailing-root schema fixture");
        assert!(norito::decode_from_bytes::<EntrypointValueTypeV1>(&bytes).is_err());
    }

    #[test]
    fn flat_schema_json_from_value_enforces_the_v1_depth_boundary() {
        let at_limit =
            nested_list_schema_json_value(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH.saturating_sub(1));
        let decoded = norito::json::from_value::<EntrypointValueTypeV1>(at_limit)
            .expect("decode flat schema at V1 limit");
        assert_eq!(
            decoded,
            nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1)
        );

        let over_limit = nested_list_schema_json_value(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        assert!(norito::json::from_value::<EntrypointValueTypeV1>(over_limit).is_err());

        let leaf = norito::json::to_value(&leaf()).expect("convert recovery schema");
        norito::json::from_value::<EntrypointValueTypeV1>(leaf)
            .expect("failed decode must not poison a later schema decode");
    }

    #[test]
    fn legacy_recursive_list_json_is_rejected() {
        let element = norito::json::to_value(&leaf()).expect("convert legacy element fixture");
        let flat_leaf_node = element
            .as_object()
            .and_then(|object| object.get("nodes"))
            .and_then(norito::json::Value::as_array)
            .and_then(|nodes| nodes.first())
            .cloned()
            .expect("extract flat leaf node fixture");
        let list = norito::json::object([
            ("capacity", norito::json::Value::from(1_u8)),
            ("element", element),
        ])
        .expect("construct retired recursive list metadata");
        let node =
            norito::json::object([("kind", norito::json::Value::from("List")), ("value", list)])
                .expect("construct retired recursive list node");
        let schema = norito::json::object([(
            "nodes",
            norito::json::Value::Array(vec![node, flat_leaf_node]),
        )])
        .expect("construct retired recursive schema");

        assert_unknown_schema_field(schema, "element");
    }

    #[test]
    fn v1_schema_json_rejects_unknown_fields_at_every_object_layer() {
        fn with_unknown_field(
            mut schema: norito::json::Value,
            pointer: &str,
            field: &str,
        ) -> norito::json::Value {
            schema
                .pointer_mut(pointer)
                .and_then(norito::json::Value::as_object_mut)
                .expect("test pointer selects a schema object")
                .insert(field.to_owned(), norito::json::Value::Bool(true));
            schema
        }

        let leaf_schema = norito::json::to_value(&leaf()).expect("convert leaf schema");
        assert_unknown_schema_field(
            with_unknown_field(leaf_schema.clone(), "", "legacy_root"),
            "legacy_root",
        );
        assert_unknown_schema_field(
            with_unknown_field(leaf_schema, "/nodes/0", "legacy_envelope"),
            "legacy_envelope",
        );

        let list_schema =
            norito::json::to_value(&nested_list_schema(1)).expect("convert list schema");
        assert_unknown_schema_field(
            with_unknown_field(list_schema, "/nodes/0/value", "element"),
            "element",
        );

        let struct_schema = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "Receipt".to_owned(),
                    fields: vec!["value".to_owned()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        };
        let struct_schema = norito::json::to_value(&struct_schema).expect("convert struct schema");
        assert_unknown_schema_field(
            with_unknown_field(struct_schema, "/nodes/0/value", "positional_fields"),
            "positional_fields",
        );
    }
}
