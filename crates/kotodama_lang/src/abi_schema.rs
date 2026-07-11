//! Pure schema construction shared by semantic checking and IR lowering.
//!
//! Keeping ABI-shape validation independent of instruction emission ensures
//! `check` and `build` reject the same oversized or unsupported typed values.

use crate::semantic::{self, ExprKind, Type, TypedExpr};

pub(crate) fn state_value_kind_for_type(
    ty: &Type,
) -> Option<ivm_abi::state_value::StateValueKindV1> {
    use ivm_abi::state_value::StateValueKindV1 as Kind;

    Some(match semantic::resolve_struct_type(ty) {
        Type::Int => Kind::Int,
        Type::FixedU128 => Kind::U128,
        Type::Amount => Kind::Amount,
        Type::Bool => Kind::Bool,
        Type::String => Kind::String,
        Type::Json => Kind::Json,
        Type::Bytes => Kind::Bytes,
        Type::AccountId => Kind::AccountId,
        Type::AssetDefinitionId => Kind::AssetDefinitionId,
        Type::AssetId => Kind::AssetId,
        Type::DomainId => Kind::DomainId,
        Type::NftId => Kind::NftId,
        Type::Name => Kind::Name,
        Type::DataSpaceId => Kind::DataSpaceId,
        Type::AxtDescriptor => Kind::AxtDescriptor,
        Type::AssetHandle => Kind::AssetHandle,
        Type::ProofBlob => Kind::ProofBlob,
        Type::SoracloudRequest => Kind::SoracloudRequest,
        Type::SoracloudResponse => Kind::SoracloudResponse,
        Type::Unit
        | Type::Secret(_)
        | Type::StateMap(_, _)
        | Type::Option(_)
        | Type::Result(_, _)
        | Type::List(_, _)
        | Type::Tuple(_)
        | Type::Struct { .. }
        | Type::NamedStruct(_) => return None,
    })
}

fn append_state_value_schema_nodes(
    ty: &Type,
    nodes: &mut Vec<ivm_abi::state_value::StateValueNodeV1>,
) -> bool {
    use ivm_abi::state_value::StateValueNodeV1 as Node;

    match semantic::resolve_struct_type(ty) {
        Type::Struct { name, fields } => {
            nodes.push(Node::Struct {
                name,
                fields: fields.iter().map(|(name, _)| name.clone()).collect(),
            });
            fields
                .iter()
                .all(|(_, field_ty)| append_state_value_schema_nodes(field_ty, nodes))
        }
        Type::Tuple(items) => {
            let Ok(arity) = u16::try_from(items.len()) else {
                return false;
            };
            nodes.push(Node::Tuple { arity });
            items
                .iter()
                .all(|item| append_state_value_schema_nodes(item, nodes))
        }
        Type::Option(inner) => {
            nodes.push(Node::Option);
            append_state_value_schema_nodes(&inner, nodes)
        }
        Type::Result(ok, err) => {
            nodes.push(Node::Result);
            append_state_value_schema_nodes(&ok, nodes)
                && append_state_value_schema_nodes(&err, nodes)
        }
        Type::List(element, capacity) => {
            let mut element_nodes = Vec::new();
            if !append_state_value_schema_nodes(&element, &mut element_nodes) {
                return false;
            }
            let element = ivm_abi::state_value::StateValueSchemaV1 {
                nodes: element_nodes,
            };
            if !element.validate() {
                return false;
            }
            nodes.push(Node::List {
                element: Box::new(element),
                capacity,
            });
            true
        }
        leaf => {
            let Some(kind) = state_value_kind_for_type(&leaf) else {
                return false;
            };
            nodes.push(Node::Leaf(kind));
            true
        }
    }
}

pub(crate) fn state_value_schema(ty: &Type) -> Option<ivm_abi::state_value::StateValueSchemaV1> {
    let mut nodes = Vec::new();
    if !append_state_value_schema_nodes(ty, &mut nodes) {
        return None;
    }
    let schema = ivm_abi::state_value::StateValueSchemaV1 { nodes };
    schema.validate().then_some(schema)
}

fn append_json_construction_schema_nodes(
    expression: &TypedExpr,
    nodes: &mut Vec<ivm_abi::json::JsonConstructionNodeV1>,
) -> bool {
    use ivm_abi::json::JsonConstructionNodeV1 as Node;

    match expression.kind() {
        ExprKind::JsonObject(entries) => {
            nodes.push(Node::Object {
                keys: entries.iter().map(|(key, _)| key.clone()).collect(),
            });
            entries
                .iter()
                .all(|(_, value)| append_json_construction_schema_nodes(value, nodes))
        }
        ExprKind::JsonArray(elements) => {
            let Ok(arity) = u16::try_from(elements.len()) else {
                return false;
            };
            nodes.push(Node::Array { arity });
            elements
                .iter()
                .all(|element| append_json_construction_schema_nodes(element, nodes))
        }
        _ => {
            let Some(schema) = state_value_schema(&expression.ty) else {
                return false;
            };
            if !ivm_abi::json::json_value_schema_is_supported(&schema) {
                return false;
            }
            nodes.push(Node::Value { schema });
            true
        }
    }
}

/// A fully validated V1 native-JSON construction schema.
pub(crate) struct JsonConstructionSchema {
    pub(crate) encoded: Vec<u8>,
    pub(crate) word_count: usize,
}

/// Stable reason a typed native-JSON expression cannot cross the V1 ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JsonConstructionSchemaError {
    UnsupportedValueType,
    InvalidShape,
    Encoding,
    EncodedSize,
}

impl core::fmt::Display for JsonConstructionSchemaError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedValueType => {
                "native JSON expression contains an unsupported value type"
            }
            Self::InvalidShape => "native JSON construction schema exceeds V1 shape limits",
            Self::Encoding => "native JSON construction schema cannot be canonically encoded",
            Self::EncodedSize => "native JSON construction schema exceeds the V1 byte limit",
        })
    }
}

pub(crate) fn json_construction_schema(
    expression: &TypedExpr,
) -> Result<JsonConstructionSchema, JsonConstructionSchemaError> {
    let mut nodes = Vec::new();
    if !append_json_construction_schema_nodes(expression, &mut nodes) {
        return Err(JsonConstructionSchemaError::UnsupportedValueType);
    }
    let schema = ivm_abi::json::JsonConstructionSchemaV1 { nodes };
    let word_count = schema
        .word_count()
        .ok_or(JsonConstructionSchemaError::InvalidShape)?;
    let encoded = norito::to_bytes(&schema).map_err(|_| JsonConstructionSchemaError::Encoding)?;
    if encoded.len() > ivm_abi::json::MAX_JSON_CONSTRUCTION_SCHEMA_BYTES_V1 {
        return Err(JsonConstructionSchemaError::EncodedSize);
    }
    Ok(JsonConstructionSchema {
        encoded,
        word_count,
    })
}
