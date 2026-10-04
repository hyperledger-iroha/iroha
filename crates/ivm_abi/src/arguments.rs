//! Canonical schema-bound JSON conversion for public Kotodama call arguments.
//!
//! Producers convert before signing. The VM owns metered decoding and materialization.
use crate::{
    VMError,
    codec::encode_canonical_norito as canonical_norito_frame,
    entrypoint::{
        EntrypointArgumentRecordV1, EntrypointArgumentSchemaV1, EntrypointValueAtomV1,
        EntrypointValueKindV1, EntrypointValueTypeNodeV1, EntrypointValueTypeV1,
        MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES, entrypoint_argument_schema_hash_v1,
        value_child_starts as argument_child_starts, value_subtree_end as argument_subtree_end,
    },
    pointer_abi::{PointerType, encode_tlv},
};
use iroha_data_model::{
    account::AccountId,
    prelude::{AssetDefinitionId, AssetId, NftId},
};
use iroha_model_base::{domain::DomainId, name::Name, topology::DataSpaceId};
use iroha_primitives::{
    bigint::BigInt,
    json::Json,
    numeric::{Numeric, Quantity},
    numeric_abi::DecimalValueV1,
};
use norito::json as njson;
use std::str::FromStr;
fn decode_int(value: &njson::Value) -> Result<BigInt, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let parsed: BigInt = raw.parse().map_err(|_| VMError::DecodeError)?;
    if parsed.to_string() != raw {
        return Err(VMError::DecodeError);
    }
    Ok(parsed)
}
fn decode_u64(value: &njson::Value) -> Result<u64, VMError> {
    match value {
        njson::Value::Number(number) => number.as_u64().ok_or(VMError::DecodeError),
        _ => Err(VMError::DecodeError),
    }
}
fn decode_canonical_string<T>(
    value: &njson::Value,
    parse: impl FnOnce(&str) -> Result<T, VMError>,
    canonical: impl FnOnce(&T) -> String,
) -> Result<T, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let parsed = parse(raw)?;
    if canonical(&parsed) != raw {
        return Err(VMError::DecodeError);
    }
    Ok(parsed)
}
fn decode_numeric(value: &njson::Value) -> Result<Numeric, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let parsed: Numeric = raw.parse().map_err(|_| VMError::DecodeError)?;
    if parsed.to_string() != raw {
        return Err(VMError::DecodeError);
    }
    Ok(parsed)
}
fn decode_blob(value: &njson::Value) -> Result<Vec<u8>, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let hex_body = raw.strip_prefix("0x").ok_or(VMError::DecodeError)?;
    if hex_body.len() % 2 != 0 {
        return Err(VMError::DecodeError);
    }
    if hex_body
        .bytes()
        .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(VMError::DecodeError);
    }
    hex::decode(hex_body).map_err(|_| VMError::DecodeError)
}
fn encode_leaf_atom(
    kind: &EntrypointValueKindV1,
    value: &njson::Value,
) -> Result<EntrypointValueAtomV1, VMError> {
    let encoded_pointer = |pointer_type, payload: Vec<u8>| {
        encode_tlv(pointer_type, &payload).map(EntrypointValueAtomV1::Pointer)
    };
    Ok(match kind {
        EntrypointValueKindV1::Int => {
            EntrypointValueAtomV1::Pointer(crate::numeric_tlv::encode_int(&decode_int(value)?)?)
        }
        EntrypointValueKindV1::Decimal => {
            let decimal = DecimalValueV1::try_from_numeric(decode_numeric(value)?)
                .map_err(|_| VMError::DecodeError)?;
            EntrypointValueAtomV1::Pointer(crate::numeric_tlv::encode_decimal(
                decimal.as_numeric(),
            )?)
        }
        EntrypointValueKindV1::Quantity => {
            let quantity = Quantity::try_from_numeric(decode_numeric(value)?)
                .map_err(|_| VMError::DecodeError)?;
            EntrypointValueAtomV1::Pointer(crate::numeric_tlv::encode_quantity(&quantity)?)
        }
        EntrypointValueKindV1::Bool => {
            EntrypointValueAtomV1::Bool(value.as_bool().ok_or(VMError::DecodeError)?)
        }
        EntrypointValueKindV1::String => encoded_pointer(
            PointerType::Blob,
            value
                .as_str()
                .ok_or(VMError::DecodeError)?
                .as_bytes()
                .to_vec(),
        )?,
        EntrypointValueKindV1::Json => encoded_pointer(
            PointerType::Json,
            canonical_norito_frame(
                &Json::from_norito_value_ref(value).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::NoritoInvalid)?,
        )?,
        EntrypointValueKindV1::Name => {
            let name = decode_canonical_string(
                value,
                |raw| Name::from_str(raw).map_err(|_| VMError::DecodeError),
                ToString::to_string,
            )?;
            encoded_pointer(
                PointerType::Name,
                canonical_norito_frame(&name).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::AccountId => {
            let raw = value.as_str().ok_or(VMError::DecodeError)?;
            let parsed = AccountId::parse_encoded(raw).map_err(|_| VMError::DecodeError)?;
            if parsed.to_string() != raw {
                return Err(VMError::DecodeError);
            }
            let account_id = parsed;
            encoded_pointer(
                PointerType::AccountId,
                canonical_norito_frame(&account_id).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::AssetDefinitionId => {
            let asset_definition_id = decode_canonical_string(
                value,
                |raw| {
                    AssetDefinitionId::parse_address_literal(raw).map_err(|_| VMError::DecodeError)
                },
                AssetDefinitionId::canonical_address,
            )?;
            encoded_pointer(
                PointerType::AssetDefinitionId,
                canonical_norito_frame(&asset_definition_id).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::AssetId => {
            let asset_id = decode_canonical_string(
                value,
                |raw| AssetId::parse_literal(raw).map_err(|_| VMError::DecodeError),
                AssetId::canonical_literal,
            )?;
            encoded_pointer(
                PointerType::AssetId,
                canonical_norito_frame(&asset_id).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::DomainId => {
            let domain_id = decode_canonical_string(
                value,
                |raw| DomainId::parse_fully_qualified(raw).map_err(|_| VMError::DecodeError),
                ToString::to_string,
            )?;
            encoded_pointer(
                PointerType::DomainId,
                canonical_norito_frame(&domain_id).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::NftId => {
            let nft_id = decode_canonical_string(
                value,
                |raw| NftId::from_str(raw).map_err(|_| VMError::DecodeError),
                ToString::to_string,
            )?;
            encoded_pointer(
                PointerType::NftId,
                canonical_norito_frame(&nft_id).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::DataSpaceId => {
            let dataspace_id = DataSpaceId::new(decode_u64(value)?);
            encoded_pointer(
                PointerType::DataSpaceId,
                canonical_norito_frame(&dataspace_id).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::Blob => encoded_pointer(PointerType::Blob, decode_blob(value)?)?,
    })
}
fn decode_argument_node(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
    value: &njson::Value,
    out: &mut Vec<EntrypointValueAtomV1>,
) -> Result<(), VMError> {
    enum Task<'a> {
        Visit {
            node_start: usize,
            value: &'a njson::Value,
        },
        FinishProduct {
            children: usize,
        },
        FinishSum {
            tag: bool,
        },
        FinishList {
            item_count_usize: usize,
        },
    }
    let start = *node_index;
    let end = argument_subtree_end(nodes, start)?;
    let mut tasks = vec![Task::Visit {
        node_start: start,
        value,
    }];
    let mut results = Vec::<Vec<EntrypointValueAtomV1>>::new();
    while let Some(task) = tasks.pop() {
        match task {
            Task::Visit { node_start, value } => {
                let node = nodes.get(node_start).ok_or(VMError::DecodeError)?;
                match node {
                    EntrypointValueTypeNodeV1::StateCursor(key) => {
                        let envelope = encode_tlv(PointerType::NoritoBytes, &decode_blob(value)?)?;
                        crate::state_cursor::validate_cursor_envelope(*key, &envelope)?;
                        results.push(vec![EntrypointValueAtomV1::Pointer(envelope)]);
                    }
                    EntrypointValueTypeNodeV1::Unit => {
                        if !matches!(value, njson::Value::Null) {
                            return Err(VMError::DecodeError);
                        }
                        results.push(vec![EntrypointValueAtomV1::Unit]);
                    }
                    EntrypointValueTypeNodeV1::Error(error) => {
                        let name = value.as_str().ok_or(VMError::DecodeError)?;
                        let variant = error
                            .variants
                            .iter()
                            .find(|variant| variant.name == name)
                            .ok_or(VMError::DecodeError)?;
                        results.push(vec![EntrypointValueAtomV1::ErrorCode(variant.code)]);
                    }
                    EntrypointValueTypeNodeV1::Struct(node) => {
                        let object = value.as_object().ok_or(VMError::DecodeError)?;
                        if object.len() != node.fields.len() {
                            return Err(VMError::DecodeError);
                        }
                        let starts = argument_child_starts(nodes, node_start, node.fields.len())?;
                        tasks.push(Task::FinishProduct {
                            children: starts.len(),
                        });
                        for (child, field) in starts.iter().zip(&node.fields).rev() {
                            tasks.push(Task::Visit {
                                node_start: *child,
                                value: object.get(field).ok_or(VMError::DecodeError)?,
                            });
                        }
                    }
                    EntrypointValueTypeNodeV1::Tuple(arity) => {
                        let values = value.as_array().ok_or(VMError::DecodeError)?;
                        if values.len() != usize::from(*arity) {
                            return Err(VMError::DecodeError);
                        }
                        let starts = argument_child_starts(nodes, node_start, values.len())?;
                        tasks.push(Task::FinishProduct {
                            children: starts.len(),
                        });
                        for (child, value) in starts.iter().zip(values).rev() {
                            tasks.push(Task::Visit {
                                node_start: *child,
                                value,
                            });
                        }
                    }
                    EntrypointValueTypeNodeV1::Option => {
                        let object = value.as_object().ok_or(VMError::DecodeError)?;
                        if object.len() != 1 {
                            return Err(VMError::DecodeError);
                        }
                        if let Some(value) = object.get("some") {
                            tasks.push(Task::FinishSum { tag: true });
                            tasks.push(Task::Visit {
                                node_start: node_start
                                    .checked_add(1)
                                    .ok_or(VMError::DecodeError)?,
                                value,
                            });
                        } else if object.get("none") == Some(&njson::Value::Bool(true)) {
                            results.push(vec![EntrypointValueAtomV1::Tag(false)]);
                        } else {
                            return Err(VMError::DecodeError);
                        }
                    }
                    EntrypointValueTypeNodeV1::Result => {
                        let object = value.as_object().ok_or(VMError::DecodeError)?;
                        if object.len() != 1 {
                            return Err(VMError::DecodeError);
                        }
                        let ok_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                        let err_start = argument_subtree_end(nodes, ok_start)?;
                        if let Some(value) = object.get("ok") {
                            tasks.push(Task::FinishSum { tag: true });
                            tasks.push(Task::Visit {
                                node_start: ok_start,
                                value,
                            });
                        } else if let Some(value) = object.get("err") {
                            tasks.push(Task::FinishSum { tag: false });
                            tasks.push(Task::Visit {
                                node_start: err_start,
                                value,
                            });
                        } else {
                            return Err(VMError::DecodeError);
                        }
                    }
                    EntrypointValueTypeNodeV1::List(list) => {
                        let values = value.as_array().ok_or(VMError::DecodeError)?;
                        if values.len() > usize::from(list.capacity) {
                            return Err(VMError::DecodeError);
                        }
                        let element_start =
                            node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                        let _ = argument_subtree_end(nodes, element_start)?;
                        tasks.push(Task::FinishList {
                            item_count_usize: values.len(),
                        });
                        for value in values.iter().rev() {
                            tasks.push(Task::Visit {
                                node_start: element_start,
                                value,
                            });
                        }
                    }
                    EntrypointValueTypeNodeV1::Leaf(kind) => {
                        results.push(vec![encode_leaf_atom(kind, value)?]);
                    }
                }
            }
            Task::FinishProduct { children } => {
                let split = results
                    .len()
                    .checked_sub(children)
                    .ok_or(VMError::DecodeError)?;
                let child_results = results.split_off(split);
                let capacity = child_results
                    .iter()
                    .try_fold(0_usize, |total, child| total.checked_add(child.len()))
                    .ok_or(VMError::DecodeError)?;
                let mut product = Vec::with_capacity(capacity);
                for child in child_results {
                    product.extend(child);
                }
                results.push(product);
            }
            Task::FinishSum { tag } => {
                let child = results.pop().ok_or(VMError::DecodeError)?;
                let mut sum = Vec::with_capacity(child.len().saturating_add(1));
                sum.push(EntrypointValueAtomV1::Tag(tag));
                sum.extend(child);
                results.push(sum);
            }
            Task::FinishList { item_count_usize } => {
                let split = results
                    .len()
                    .checked_sub(item_count_usize)
                    .ok_or(VMError::DecodeError)?;
                let item_results = results.split_off(split);
                let item_count =
                    u8::try_from(item_results.len()).map_err(|_| VMError::DecodeError)?;
                let capacity = item_results
                    .iter()
                    .try_fold(1_usize, |total, item| total.checked_add(item.len()))
                    .ok_or(VMError::DecodeError)?;
                let mut list = Vec::with_capacity(capacity);
                list.push(EntrypointValueAtomV1::List(item_count));
                for item in item_results {
                    list.extend(item);
                }
                results.push(list);
            }
        }
    }
    if results.len() != 1 {
        return Err(VMError::DecodeError);
    }
    out.extend(results.pop().expect("length checked"));
    *node_index = end;
    Ok(())
}
fn decode_argument_value(
    ty: &EntrypointValueTypeV1,
    value: &njson::Value,
    out: &mut Vec<EntrypointValueAtomV1>,
) -> Result<(), VMError> {
    if !ty.validate() {
        return Err(VMError::DecodeError);
    }
    let mut node_index = 0;
    decode_argument_node(&ty.nodes, &mut node_index, value, out)?;
    if node_index != ty.nodes.len() {
        return Err(VMError::DecodeError);
    }
    Ok(())
}
/// Convert one Torii/CLI boundary JSON value into the canonical schema-bound
/// Norito record consumed by a Kotodama V1 entrypoint.
///
/// # Errors
/// Rejects invalid schemas, inexact fields, noncanonical leaf values and malformed active aggregates.
pub fn argument_record_from_json(
    schema: &EntrypointArgumentSchemaV1,
    payload: &Json,
) -> Result<EntrypointArgumentRecordV1, VMError> {
    if !schema.validate() {
        return Err(VMError::DecodeError);
    }
    let value: njson::Value = payload
        .try_into_any_norito()
        .map_err(|_| VMError::DecodeError)?;
    let object = value.as_object().ok_or(VMError::DecodeError)?;
    if object.len() != schema.fields.len() {
        return Err(VMError::DecodeError);
    }
    let expected_words = schema.word_count().ok_or(VMError::DecodeError)?;
    let mut atoms = Vec::with_capacity(expected_words);
    for field in &schema.fields {
        let field_value = object.get(&field.name).ok_or(VMError::DecodeError)?;
        decode_argument_value(&field.ty, field_value, &mut atoms)?;
    }
    if !schema.validate_atoms(&atoms) {
        return Err(VMError::DecodeError);
    }
    let schema_bytes = canonical_norito_frame(schema).map_err(|_| VMError::NoritoInvalid)?;
    Ok(EntrypointArgumentRecordV1 {
        schema_hash: entrypoint_argument_schema_hash_v1(&schema_bytes),
        atoms,
    })
}
/// Encode a canonical public argument record for transport into the IVM host.
///
/// # Errors
/// Rejects invalid boundary values or a complete record exceeding the inclusive V1 byte limit.
pub fn encode_argument_record_from_json(
    schema: &EntrypointArgumentSchemaV1,
    payload: &Json,
) -> Result<Vec<u8>, VMError> {
    let record = argument_record_from_json(schema, payload)?;
    let bytes = canonical_norito_frame(&record).map_err(|_| VMError::NoritoInvalid)?;
    if bytes.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entrypoint::EntrypointArgumentFieldV1;
    #[test]
    fn numeric_argument_atoms_require_canonical_decimal_strings() {
        assert_eq!(decode_int(&norito::json!("-7")), Ok(BigInt::from_i128(-7)));
        assert_eq!(
            decode_numeric(&norito::json!("1.25")),
            Ok(Numeric::new(125, 2))
        );
        for value in [
            norito::json!(7),
            norito::json!(7_u64),
            norito::json!("+7"),
            norito::json!("07"),
            norito::json!("-0"),
        ] {
            assert_eq!(decode_int(&value), Err(VMError::DecodeError));
        }
        for value in [
            norito::json!(1.25),
            norito::json!("+1.25"),
            norito::json!("01.25"),
            norito::json!("1.250"),
            norito::json!("1e0"),
        ] {
            assert_eq!(decode_numeric(&value), Err(VMError::DecodeError));
        }
    }
    #[test]
    fn recursive_tags_reject_ambiguous_or_noncanonical_shapes() {
        let option_type = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        };
        let invalid = [
            norito::json!({ "some": "1", "none": true }),
            norito::json!({ "none": false }),
            norito::json!({ "None": true }),
            njson::Value::String("1".to_owned()),
            njson::Value::Null,
        ];
        for value in invalid {
            let schema = EntrypointArgumentSchemaV1 {
                fields: vec![EntrypointArgumentFieldV1 {
                    name: "value".into(),
                    ty: option_type.clone(),
                }],
            };
            let payload = Json::from(norito::json!({ "value": value }));
            assert_eq!(
                argument_record_from_json(&schema, &payload),
                Err(VMError::DecodeError)
            );
        }
    }

    #[test]
    fn shared_boundary_fixture_preserves_exact_canonical_record_bytes() {
        let fixture = njson::parse_value(include_str!(
            "../../../fixtures/kotodama/entrypoint_argument_record_v1.json"
        ))
        .expect("parse shared boundary fixture");
        assert_eq!(
            fixture.get("generator").and_then(njson::Value::as_str),
            Some("ivm_abi::arguments::encode_argument_record_from_json")
        );
        let schema_hex = fixture
            .pointer("/entrypoint_argument_schema_v1/norito_hex")
            .and_then(njson::Value::as_str)
            .expect("exact schema bytes");
        let schema: EntrypointArgumentSchemaV1 =
            crate::codec::decode_canonical_norito(&hex::decode(schema_hex).expect("schema hex"))
                .expect("canonical schema");
        let payload = Json::from(
            fixture
                .pointer("/torii_boundary/payload")
                .expect("shared payload")
                .clone(),
        );
        let expected = hex::decode(
            fixture
                .pointer("/entrypoint_argument_record_v1/norito_hex")
                .and_then(njson::Value::as_str)
                .expect("record hex"),
        )
        .expect("record bytes");
        assert_eq!(
            encode_argument_record_from_json(&schema, &payload),
            Ok(expected)
        );
        let mut extra: njson::Value = payload.try_into_any_norito().expect("boundary object");
        extra
            .as_object_mut()
            .expect("object")
            .insert("unknown".to_owned(), njson::Value::Bool(true));
        assert_eq!(
            argument_record_from_json(&schema, &Json::from(extra)),
            Err(VMError::DecodeError)
        );
        let mut noncanonical: njson::Value =
            payload.try_into_any_norito().expect("boundary object");
        noncanonical
            .as_object_mut()
            .expect("object")
            .insert("count".to_owned(), njson::Value::String("-0".to_owned()));
        assert_eq!(
            argument_record_from_json(&schema, &Json::from(noncanonical)),
            Err(VMError::DecodeError)
        );
    }
    #[test]
    fn complete_public_record_limit_rejects_an_oversized_canonical_string() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![EntrypointValueTypeNodeV1::Leaf(
                        EntrypointValueKindV1::String,
                    )],
                },
            }],
        };
        // The JSON body fits its inclusive limit; pointer and record framing
        // make the complete canonical argument record exceed its own limit.
        let oversized_string =
            "x".repeat(iroha_primitives::json::MAX_JSON_BYTES - r#"{"value":""}"#.len());
        let payload = Json::try_new(norito::json!({"value": oversized_string}))
            .expect("canonical JSON body fits its byte limit");
        assert_eq!(payload.get().len(), iroha_primitives::json::MAX_JSON_BYTES);
        let record = argument_record_from_json(&schema, &payload)
            .expect("bounded canonical string has valid argument atoms");
        assert!(
            canonical_norito_frame(&record)
                .expect("encode complete canonical argument record")
                .len()
                > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES
        );
        assert_eq!(
            encode_argument_record_from_json(&schema, &payload),
            Err(VMError::NoritoInvalid)
        );
    }
}
