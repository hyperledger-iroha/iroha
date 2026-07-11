//! Schema-bound construction records for native Kotodama JSON expressions.
//!
//! A construction schema is emitted by the compiler in preorder. Object and
//! array nodes describe source structure, while value nodes reuse the exact
//! aggregate-state schema for the corresponding flattened VM words. This lets
//! the host recursively convert bounded Lists and active-only Options without
//! guessing runtime types or serializing an intermediate projection.

use std::collections::BTreeSet;

use norito::{Decode, Encode};

use crate::state_value::{
    MAX_STATE_VALUE_SCHEMA_BYTES, MAX_STATE_VALUE_WORDS, StateValueKindV1, StateValueNodeV1,
    StateValueSchemaV1,
};

/// Maximum number of construction nodes accepted by the V1 JSON builder.
pub const MAX_JSON_CONSTRUCTION_NODES_V1: usize = 256;
/// Maximum number of statically declared fields or elements in one JSON node.
pub const MAX_JSON_LITERAL_ITEMS_V1: usize = 64;
/// Maximum canonical encoded construction-schema size.
pub const MAX_JSON_CONSTRUCTION_SCHEMA_BYTES_V1: usize = MAX_STATE_VALUE_SCHEMA_BYTES;

/// One preorder node in a compiler-emitted native JSON construction schema.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum JsonConstructionNodeV1 {
    /// JSON object. One child immediately follows for every key in source order.
    Object {
        /// Source keys. The host inserts them into a canonical ordered map.
        keys: Vec<String>,
    },
    /// JSON array. `arity` children immediately follow.
    Array {
        /// Statically known number of array elements.
        arity: u16,
    },
    /// One dynamic source value represented by flattened VM words.
    Value {
        /// Exact source-value schema used to interpret those words.
        schema: StateValueSchemaV1,
    },
}

/// Compiler-owned schema for one native `json { ... }` or `json [ ... ]` expression.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct JsonConstructionSchemaV1 {
    /// Preorder construction tree.
    pub nodes: Vec<JsonConstructionNodeV1>,
}

impl JsonConstructionSchemaV1 {
    /// Validate tree shape, duplicate keys, collection bounds, and source-value types.
    #[must_use]
    pub fn validate(&self) -> bool {
        self.analyze().is_some()
    }

    /// Return the exact number of flattened VM words consumed by this schema.
    #[must_use]
    pub fn word_count(&self) -> Option<usize> {
        self.analyze().map(|analysis| analysis.words)
    }

    fn analyze(&self) -> Option<JsonConstructionAnalysisV1> {
        fn analyze_node(
            nodes: &[JsonConstructionNodeV1],
            index: &mut usize,
            depth: usize,
        ) -> Option<JsonConstructionAnalysisV1> {
            if depth > MAX_JSON_CONSTRUCTION_NODES_V1 {
                return None;
            }
            let node = nodes.get(*index)?;
            *index = index.checked_add(1)?;
            let mut analysis = JsonConstructionAnalysisV1 {
                nodes: 1,
                words: 0,
                depth,
            };
            let mut merge = |child: JsonConstructionAnalysisV1| -> Option<()> {
                analysis.nodes = analysis.nodes.checked_add(child.nodes)?;
                analysis.words = analysis.words.checked_add(child.words)?;
                analysis.depth = analysis.depth.max(child.depth);
                Some(())
            };
            match node {
                JsonConstructionNodeV1::Object { keys } => {
                    if keys.len() > MAX_JSON_LITERAL_ITEMS_V1
                        || keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
                    {
                        return None;
                    }
                    for _ in keys {
                        merge(analyze_node(nodes, index, depth.checked_add(1)?)?)?;
                    }
                }
                JsonConstructionNodeV1::Array { arity } => {
                    if usize::from(*arity) > MAX_JSON_LITERAL_ITEMS_V1 {
                        return None;
                    }
                    for _ in 0..*arity {
                        merge(analyze_node(nodes, index, depth.checked_add(1)?)?)?;
                    }
                }
                JsonConstructionNodeV1::Value { schema } => {
                    if !json_value_schema_is_supported(schema) {
                        return None;
                    }
                    analysis.words = schema.word_count()?;
                }
            }
            (analysis.nodes <= MAX_JSON_CONSTRUCTION_NODES_V1
                && analysis.words <= MAX_STATE_VALUE_WORDS)
                .then_some(analysis)
        }

        if self.nodes.is_empty() || self.nodes.len() > MAX_JSON_CONSTRUCTION_NODES_V1 {
            return None;
        }
        let mut index = 0;
        let analysis = analyze_node(&self.nodes, &mut index, 1)?;
        (index == self.nodes.len()).then_some(analysis)
    }
}

#[derive(Clone, Copy)]
struct JsonConstructionAnalysisV1 {
    nodes: usize,
    words: usize,
    depth: usize,
}

/// Return whether an aggregate-state schema belongs to the recursively
/// JSON-convertible V1 subset.
#[must_use]
pub fn json_value_schema_is_supported(schema: &StateValueSchemaV1) -> bool {
    fn supported_nodes(nodes: &[StateValueNodeV1], index: &mut usize) -> bool {
        let Some(node) = nodes.get(*index) else {
            return false;
        };
        *index = index.saturating_add(1);
        match node {
            StateValueNodeV1::Option => supported_nodes(nodes, index),
            StateValueNodeV1::List { element, .. } => json_value_schema_is_supported(element),
            StateValueNodeV1::Leaf(kind) => matches!(
                kind,
                StateValueKindV1::Int
                    | StateValueKindV1::Decimal
                    | StateValueKindV1::Quantity
                    | StateValueKindV1::Bool
                    | StateValueKindV1::String
                    | StateValueKindV1::Json
                    | StateValueKindV1::Bytes
                    | StateValueKindV1::AccountId
                    | StateValueKindV1::AssetDefinitionId
                    | StateValueKindV1::AssetId
                    | StateValueKindV1::DomainId
                    | StateValueKindV1::NftId
                    | StateValueKindV1::Name
                    | StateValueKindV1::DataSpaceId
            ),
            StateValueNodeV1::Struct { .. }
            | StateValueNodeV1::Tuple { .. }
            | StateValueNodeV1::Result => false,
        }
    }

    if !schema.validate() {
        return false;
    }
    let mut index = 0;
    supported_nodes(&schema.nodes, &mut index) && index == schema.nodes.len()
}

#[cfg(test)]
mod tests {
    use norito::{decode_from_bytes, to_bytes};

    use super::*;

    fn leaf(kind: StateValueKindV1) -> StateValueSchemaV1 {
        StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(kind)],
        }
    }

    #[test]
    fn construction_schema_counts_words_and_rejects_duplicate_keys() {
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Object {
                    keys: vec!["amount".into(), "labels".into()],
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Quantity),
                },
                JsonConstructionNodeV1::Array { arity: 2 },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::String),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::String),
                },
            ],
        };
        assert!(schema.validate());
        assert_eq!(schema.word_count(), Some(3));

        let duplicate = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Object {
                    keys: vec!["same".into(), "same".into()],
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Int),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Int),
                },
            ],
        };
        assert!(!duplicate.validate());
        assert_eq!(duplicate.word_count(), None);
    }

    #[test]
    fn supported_values_allow_nested_options_and_lists_but_reject_products_and_results() {
        let option_list = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::List {
                    element: Box::new(leaf(StateValueKindV1::AccountId)),
                    capacity: 4,
                },
            ],
        };
        assert!(json_value_schema_is_supported(&option_list));

        for rejected in [
            StateValueSchemaV1 {
                nodes: vec![
                    StateValueNodeV1::Tuple { arity: 2 },
                    StateValueNodeV1::Leaf(StateValueKindV1::Int),
                    StateValueNodeV1::Leaf(StateValueKindV1::Int),
                ],
            },
            StateValueSchemaV1 {
                nodes: vec![
                    StateValueNodeV1::Result,
                    StateValueNodeV1::Leaf(StateValueKindV1::Int),
                    StateValueNodeV1::Leaf(StateValueKindV1::Int),
                ],
            },
            leaf(StateValueKindV1::AssetHandle),
        ] {
            assert!(!json_value_schema_is_supported(&rejected));
        }
    }

    #[test]
    fn construction_schema_roundtrips_canonically() {
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Array { arity: 2 },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Quantity),
                },
                JsonConstructionNodeV1::Value {
                    schema: StateValueSchemaV1 {
                        nodes: vec![
                            StateValueNodeV1::Option,
                            StateValueNodeV1::Leaf(StateValueKindV1::Name),
                        ],
                    },
                },
            ],
        };
        let encoded = to_bytes(&schema).expect("encode JSON construction schema");
        let decoded: JsonConstructionSchemaV1 =
            decode_from_bytes(&encoded).expect("decode JSON construction schema");
        assert_eq!(decoded, schema);
        assert_eq!(to_bytes(&decoded).expect("re-encode schema"), encoded);
    }
}
