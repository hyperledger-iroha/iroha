//! Shared helpers for committed transaction predicates.

#![allow(clippy::missing_errors_doc)]

use iroha_crypto::HashOf;
use iroha_primitives::json::Json;
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize, Map, Value};
use thiserror::Error;

use crate::{
    name::Name,
    query::{CommittedTransaction, CommittedTxFilters},
};

/// Predicate tree over committed transactions.
///
/// With the `json` feature, the canonical representation is an app expression
/// object with exactly `op` and `args` fields. Field paths are strings
/// (`block_hash`, `authority`, `timestamp_ms`, `entrypoint_hash`, `result_ok`, or
/// `metadata.<name>`); presence operators carry an explicit boolean argument.
/// Decoding rejects empty logical nodes, empty or duplicate membership sets,
/// unknown fields, alternate literal spellings, and trees exceeding the shared
/// depth, node, or membership budgets. Invalid programmatic trees evaluate to
/// `false`, serialize as `{"op":"const","args":[false]}`, and are rejected by
/// the binary serializer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommittedTxPredicate {
    /// Logical conjunction of sub-predicates.
    And(Vec<CommittedTxPredicate>),
    /// Logical disjunction of sub-predicates.
    Or(Vec<CommittedTxPredicate>),
    /// Logical negation of a sub-predicate.
    Not(Box<CommittedTxPredicate>),
    // Block hash atoms (always present)
    /// Matches transactions whose carrier block hash equals the provided value.
    BlockEq(HashOf<crate::block::BlockHeader>),
    /// Matches transactions whose carrier block hash differs from the provided value.
    BlockNe(HashOf<crate::block::BlockHeader>),
    /// Matches transactions whose carrier block hash is contained in the provided set.
    BlockIn(Vec<HashOf<crate::block::BlockHeader>>),
    /// Matches transactions whose carrier block hash is not contained in the provided set.
    BlockNin(Vec<HashOf<crate::block::BlockHeader>>),
    /// Matches existence (or absence) of a carrier block hash.
    BlockExists(bool),
    // Authority atoms (present only for External entrypoints)
    /// Matches when the transaction authority equals the provided account ID.
    AuthorityEq(crate::account::AccountId),
    /// Matches when the transaction authority differs from the provided account ID.
    AuthorityNe(crate::account::AccountId),
    /// Matches when the transaction authority is contained in the provided set.
    AuthorityIn(Vec<crate::account::AccountId>),
    /// Matches when the transaction authority is not contained in the provided set.
    AuthorityNin(Vec<crate::account::AccountId>),
    /// Matches existence (or absence) of an authority.
    AuthorityExists(bool), // true: exists, false: is null
    // Timestamp atoms (milliseconds)
    /// Matches transactions whose timestamp equals the provided value.
    TsEq(u64),
    /// Matches transactions whose timestamp is strictly less than the provided value.
    TsLt(u64),
    /// Matches transactions whose timestamp is less than or equal to the provided value.
    TsLte(u64),
    /// Matches transactions whose timestamp is strictly greater than the provided value.
    TsGt(u64),
    /// Matches transactions whose timestamp is greater than or equal to the provided value.
    TsGte(u64),
    /// Matches transactions whose timestamp belongs to the provided set.
    TsIn(Vec<u64>),
    /// Matches transactions whose timestamp does not belong to the provided set.
    TsNin(Vec<u64>),
    /// Matches existence (or absence) of a timestamp.
    TsExists(bool),
    // Entrypoint hash atoms (always present)
    /// Matches transactions whose entrypoint hash equals the provided value.
    EntryEq(HashOf<crate::transaction::signed::TransactionEntrypoint>),
    /// Matches transactions whose entrypoint hash differs from the provided value.
    EntryNe(HashOf<crate::transaction::signed::TransactionEntrypoint>),
    /// Matches transactions whose entrypoint hash is contained in the provided set.
    EntryIn(Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>>),
    /// Matches transactions whose entrypoint hash is not contained in the provided set.
    EntryNin(Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>>),
    /// Matches existence (or absence) of an entrypoint hash. This mirror variant is maintained
    /// for symmetry with result predicates and always evaluates to the provided boolean.
    EntryExists(bool),
    // Result .is_ok()
    /// Matches transactions whose result success flag equals the provided value.
    ResultEq(bool),
    /// Matches transactions whose result success flag differs from the provided value.
    ResultNe(bool),
    /// Matches transactions whose result success flag is in the provided set.
    ResultIn(Vec<bool>),
    /// Matches transactions whose result success flag is not in the provided set.
    ResultNin(Vec<bool>),
    /// Matches existence (or absence) of a result (always `true` for committed transactions).
    ResultExists(bool),
    // Metadata atoms (External entrypoints only)
    /// Matches metadata key that equals the provided JSON value.
    MetadataEq {
        /// Metadata key being compared.
        key: Name,
        /// JSON value expected at the metadata key.
        value: Json,
    },
    /// Matches metadata key that differs from the provided JSON value.
    MetadataNe {
        /// Metadata key being compared.
        key: Name,
        /// JSON value that must differ from the stored metadata value.
        value: Json,
    },
    /// Matches metadata key whose value is contained in the provided set.
    MetadataIn {
        /// Metadata key being compared.
        key: Name,
        /// JSON values that satisfy the predicate.
        values: Vec<Json>,
    },
    /// Matches metadata key whose value is not contained in the provided set.
    MetadataNin {
        /// Metadata key being compared.
        key: Name,
        /// JSON values that must not match the stored metadata value.
        values: Vec<Json>,
    },
    /// Matches metadata key existence.
    MetadataExists {
        /// Metadata key being queried.
        key: Name,
        /// Whether the key must exist (`true`) or be absent (`false`).
        exists: bool,
    },
    /// Matches metadata key being `null` (or not).
    MetadataIsNull {
        /// Metadata key being queried.
        key: Name,
        /// Whether the value must be `null`.
        is_null: bool,
    },
    // Constant leaf (used for robust parsing)
    /// Constant predicate returning the provided boolean value.
    Const(bool),
}

const MAX_COMMITTED_TX_PREDICATE_DEPTH: usize = 64;
const MAX_COMMITTED_TX_PREDICATE_NODES: usize = 1_024;
const MAX_COMMITTED_TX_MEMBERSHIP_VALUES: usize = 1_024;
const MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
enum CommittedTxPredicateValidationError {
    #[error("operator `{0}` requires at least one child")]
    EmptyLogical(&'static str),
    #[error("membership values for field `{0}` must not be empty")]
    EmptyMembership(&'static str),
    #[error("membership values for field `{0}` contain a duplicate literal")]
    DuplicateMembership(&'static str),
    #[error("metadata predicate contains invalid JSON")]
    InvalidMetadataJson,
    #[error("membership values for field `{field}` exceed the limit of {max}")]
    TooManyMembershipValues { field: &'static str, max: usize },
    #[error("committed transaction predicate membership literals exceed the limit of {0}")]
    TooManyTotalMembershipValues(usize),
    #[error("committed transaction predicate depth exceeds the limit of {0}")]
    TooDeep(usize),
    #[error("committed transaction predicate nodes exceed the limit of {0}")]
    TooManyNodes(usize),
}

#[derive(Default)]
struct PredicateValidationBudget {
    nodes: usize,
    membership_values: usize,
}

impl PredicateValidationBudget {
    fn enter_node(&mut self, depth: usize) -> Result<(), CommittedTxPredicateValidationError> {
        if depth > MAX_COMMITTED_TX_PREDICATE_DEPTH {
            return Err(CommittedTxPredicateValidationError::TooDeep(
                MAX_COMMITTED_TX_PREDICATE_DEPTH,
            ));
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_COMMITTED_TX_PREDICATE_NODES {
            return Err(CommittedTxPredicateValidationError::TooManyNodes(
                MAX_COMMITTED_TX_PREDICATE_NODES,
            ));
        }
        Ok(())
    }

    fn validate_membership<T: Ord>(
        &mut self,
        field: &'static str,
        values: &[T],
    ) -> Result<(), CommittedTxPredicateValidationError> {
        if values.is_empty() {
            return Err(CommittedTxPredicateValidationError::EmptyMembership(field));
        }
        if values.len() > MAX_COMMITTED_TX_MEMBERSHIP_VALUES {
            return Err(
                CommittedTxPredicateValidationError::TooManyMembershipValues {
                    field,
                    max: MAX_COMMITTED_TX_MEMBERSHIP_VALUES,
                },
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        for value in values {
            if !seen.insert(value) {
                return Err(CommittedTxPredicateValidationError::DuplicateMembership(
                    field,
                ));
            }
        }
        self.membership_values = self.membership_values.saturating_add(values.len());
        if self.membership_values > MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES {
            return Err(
                CommittedTxPredicateValidationError::TooManyTotalMembershipValues(
                    MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES,
                ),
            );
        }
        Ok(())
    }
}

fn validate_committed_tx_predicate_inner(
    predicate: &CommittedTxPredicate,
    depth: usize,
    budget: &mut PredicateValidationBudget,
) -> Result<(), CommittedTxPredicateValidationError> {
    use CommittedTxPredicate as P;

    budget.enter_node(depth)?;
    match predicate {
        P::And(children) => {
            if children.is_empty() {
                return Err(CommittedTxPredicateValidationError::EmptyLogical("and"));
            }
            for child in children {
                validate_committed_tx_predicate_inner(child, depth + 1, budget)?;
            }
        }
        P::Or(children) => {
            if children.is_empty() {
                return Err(CommittedTxPredicateValidationError::EmptyLogical("or"));
            }
            for child in children {
                validate_committed_tx_predicate_inner(child, depth + 1, budget)?;
            }
        }
        P::Not(inner) => validate_committed_tx_predicate_inner(inner, depth + 1, budget)?,
        P::BlockIn(values) | P::BlockNin(values) => {
            budget.validate_membership("block_hash", values)?;
        }
        P::AuthorityIn(values) | P::AuthorityNin(values) => {
            budget.validate_membership("authority", values)?;
        }
        P::TsIn(values) | P::TsNin(values) => {
            budget.validate_membership("timestamp_ms", values)?;
        }
        P::EntryIn(values) | P::EntryNin(values) => {
            budget.validate_membership("entrypoint_hash", values)?;
        }
        P::ResultIn(values) | P::ResultNin(values) => {
            budget.validate_membership("result_ok", values)?;
        }
        P::MetadataIn { values, .. } | P::MetadataNin { values, .. } => {
            budget.validate_membership("metadata", values)?;
            for value in values {
                let parsed = value
                    .try_into_any_norito::<norito::json::Value>()
                    .map_err(|_| CommittedTxPredicateValidationError::InvalidMetadataJson)?;
                let canonical = Json::from_norito_value_ref(&parsed)
                    .map_err(|_| CommittedTxPredicateValidationError::InvalidMetadataJson)?;
                if &canonical != value {
                    return Err(CommittedTxPredicateValidationError::InvalidMetadataJson);
                }
            }
        }
        P::MetadataEq { value, .. } | P::MetadataNe { value, .. } => {
            let parsed = value
                .try_into_any_norito::<norito::json::Value>()
                .map_err(|_| CommittedTxPredicateValidationError::InvalidMetadataJson)?;
            let canonical = Json::from_norito_value_ref(&parsed)
                .map_err(|_| CommittedTxPredicateValidationError::InvalidMetadataJson)?;
            if &canonical != value {
                return Err(CommittedTxPredicateValidationError::InvalidMetadataJson);
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_committed_tx_predicate(
    predicate: &CommittedTxPredicate,
) -> Result<(), CommittedTxPredicateValidationError> {
    validate_committed_tx_predicate_inner(predicate, 1, &mut PredicateValidationBudget::default())
}

/// Validation error for the committed-transaction app-expression JSON codec.
#[cfg(feature = "json")]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(super) enum CommittedTxPredicateJsonError {
    /// A predicate node was not a JSON object.
    #[error("committed transaction predicate nodes must be objects")]
    ExpectedObject,
    /// A required object field was absent.
    #[error("committed transaction predicate node is missing `{0}`")]
    MissingField(&'static str),
    /// Predicate nodes have a closed schema.
    #[error("unknown committed transaction predicate field `{0}`")]
    UnknownField(String),
    /// The operator was not encoded as a string.
    #[error("committed transaction predicate `op` must be a string")]
    ExpectedOperatorString,
    /// The operator is not part of the committed-transaction grammar.
    #[error("unsupported committed transaction predicate operator `{0}`")]
    UnsupportedOperator(String),
    /// An operator received the wrong number or shape of arguments.
    #[error("operator `{op}` expects {expected}; received {actual}")]
    InvalidArity {
        /// Operator whose arguments were malformed.
        op: String,
        /// Human-readable canonical argument shape.
        expected: &'static str,
        /// Number of arguments supplied, or zero when `args` was not an array.
        actual: usize,
    },
    /// Field paths are canonical strings, not tagged or shorthand objects.
    #[error("committed transaction predicate field path must be a string")]
    ExpectedFieldString,
    /// The field path is not supported or is not canonical.
    #[error("unsupported or non-canonical committed transaction field `{0}`")]
    InvalidField(String),
    /// The operator is not valid for the selected field.
    #[error("operator `{op}` is not supported for committed transaction field `{field}`")]
    OperatorFieldMismatch {
        /// Rejected operator.
        op: String,
        /// Rejected field path.
        field: String,
    },
    /// A field literal had the wrong JSON type or an invalid textual form.
    #[error("operator `{op}` requires {expected} for field `{field}`")]
    InvalidValue {
        /// Operator consuming the literal.
        op: String,
        /// Field whose literal was rejected.
        field: String,
        /// Expected literal type or format.
        expected: &'static str,
    },
    /// Membership with no candidates can accidentally widen negated predicates.
    #[error("membership values for field `{0}` must not be empty")]
    EmptyMembership(String),
    /// Membership sets use unique canonical literals.
    #[error("membership values for field `{0}` contain a duplicate literal")]
    DuplicateMembership(String),
    /// A single membership set exceeded its deterministic bound.
    #[error("membership values for field `{field}` exceed the limit of {max}")]
    TooManyMembershipValues {
        /// Field whose set was oversized.
        field: String,
        /// Maximum accepted set size.
        max: usize,
    },
    /// The aggregate number of membership literals exceeded its bound.
    #[error("committed transaction predicate membership literals exceed the limit of {0}")]
    TooManyTotalMembershipValues(usize),
    /// The predicate tree exceeded its recursion-depth bound.
    #[error("committed transaction predicate depth exceeds the limit of {0}")]
    TooDeep(usize),
    /// The predicate tree exceeded its node-count bound.
    #[error("committed transaction predicate nodes exceed the limit of {0}")]
    TooManyNodes(usize),
    /// A textual identifier parsed but did not use its canonical representation.
    #[error("non-canonical {kind} literal for field `{field}`")]
    NonCanonicalLiteral {
        /// Literal kind, such as an account ID or hash.
        kind: &'static str,
        /// Field containing the literal.
        field: String,
    },
    /// Metadata JSON could not be normalized into the canonical wrapper.
    #[error("metadata predicate value is not valid canonical JSON")]
    InvalidMetadataJson,
    /// The raw JSON document was malformed.
    #[error("invalid committed transaction predicate JSON: {0}")]
    MalformedJson(String),
    /// A Norito JSON wire payload used an alternate textual representation.
    #[error("committed transaction predicate JSON wire payload must use canonical encoding")]
    NonCanonicalWireEncoding,
    /// A programmatically constructed tree violated the codec invariants.
    #[error("invalid committed transaction predicate tree: {0}")]
    InvalidTree(String),
}

#[cfg(feature = "json")]
#[derive(Default)]
struct PredicateJsonBudget {
    nodes: usize,
    membership_values: usize,
}

#[cfg(feature = "json")]
impl PredicateJsonBudget {
    fn enter_node(&mut self, depth: usize) -> Result<(), CommittedTxPredicateJsonError> {
        if depth > MAX_COMMITTED_TX_PREDICATE_DEPTH {
            return Err(CommittedTxPredicateJsonError::TooDeep(
                MAX_COMMITTED_TX_PREDICATE_DEPTH,
            ));
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_COMMITTED_TX_PREDICATE_NODES {
            return Err(CommittedTxPredicateJsonError::TooManyNodes(
                MAX_COMMITTED_TX_PREDICATE_NODES,
            ));
        }
        Ok(())
    }

    fn add_membership_values(
        &mut self,
        field: &str,
        count: usize,
    ) -> Result<(), CommittedTxPredicateJsonError> {
        if count == 0 {
            return Err(CommittedTxPredicateJsonError::EmptyMembership(
                field.to_owned(),
            ));
        }
        if count > MAX_COMMITTED_TX_MEMBERSHIP_VALUES {
            return Err(CommittedTxPredicateJsonError::TooManyMembershipValues {
                field: field.to_owned(),
                max: MAX_COMMITTED_TX_MEMBERSHIP_VALUES,
            });
        }
        self.membership_values = self.membership_values.saturating_add(count);
        if self.membership_values > MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES {
            return Err(CommittedTxPredicateJsonError::TooManyTotalMembershipValues(
                MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES,
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "json")]
enum CommittedTxField {
    BlockHash,
    Authority,
    Timestamp,
    EntrypointHash,
    ResultOk,
    Metadata(Name),
}

#[cfg(feature = "json")]
fn parse_committed_tx_field(
    value: &Value,
) -> Result<CommittedTxField, CommittedTxPredicateJsonError> {
    let raw = value
        .as_str()
        .ok_or(CommittedTxPredicateJsonError::ExpectedFieldString)?;
    match raw {
        "block_hash" => Ok(CommittedTxField::BlockHash),
        "authority" => Ok(CommittedTxField::Authority),
        "timestamp_ms" => Ok(CommittedTxField::Timestamp),
        "entrypoint_hash" => Ok(CommittedTxField::EntrypointHash),
        "result_ok" => Ok(CommittedTxField::ResultOk),
        _ => {
            let Some(key) = raw.strip_prefix("metadata.") else {
                return Err(CommittedTxPredicateJsonError::InvalidField(raw.to_owned()));
            };
            if key.is_empty() {
                return Err(CommittedTxPredicateJsonError::InvalidField(raw.to_owned()));
            }
            let name = key
                .parse::<Name>()
                .map_err(|_| CommittedTxPredicateJsonError::InvalidField(raw.to_owned()))?;
            if name.to_string() != key {
                return Err(CommittedTxPredicateJsonError::InvalidField(raw.to_owned()));
            }
            Ok(CommittedTxField::Metadata(name))
        }
    }
}

#[cfg(feature = "json")]
fn predicate_expr(op: &str, args: Vec<Value>) -> Value {
    let mut object = Map::new();
    object.insert("args".to_owned(), Value::Array(args));
    object.insert("op".to_owned(), Value::String(op.to_owned()));
    Value::Object(object)
}

#[cfg(feature = "json")]
fn binary_predicate_expr(op: &str, field: impl Into<String>, value: Value) -> Value {
    predicate_expr(op, vec![Value::String(field.into()), value])
}

#[cfg(feature = "json")]
fn exact_args<'a>(
    op: &str,
    args: &'a Value,
    expected: usize,
    shape: &'static str,
) -> Result<&'a [Value], CommittedTxPredicateJsonError> {
    let Some(args) = args.as_array() else {
        return Err(CommittedTxPredicateJsonError::InvalidArity {
            op: op.to_owned(),
            expected: shape,
            actual: 0,
        });
    };
    if args.len() != expected {
        return Err(CommittedTxPredicateJsonError::InvalidArity {
            op: op.to_owned(),
            expected: shape,
            actual: args.len(),
        });
    }
    Ok(args)
}

#[cfg(feature = "json")]
fn invalid_value(op: &str, field: &str, expected: &'static str) -> CommittedTxPredicateJsonError {
    CommittedTxPredicateJsonError::InvalidValue {
        op: op.to_owned(),
        field: field.to_owned(),
        expected,
    }
}

#[cfg(feature = "json")]
fn parse_account_literal(
    op: &str,
    field: &str,
    value: &Value,
) -> Result<crate::account::AccountId, CommittedTxPredicateJsonError> {
    let raw = value
        .as_str()
        .ok_or_else(|| invalid_value(op, field, "a canonical account ID string"))?;
    let account = crate::account::AccountId::parse_encoded(raw)
        .map_err(|_| invalid_value(op, field, "a canonical account ID string"))?
        .into_account_id();
    if account.to_string() != raw {
        return Err(CommittedTxPredicateJsonError::NonCanonicalLiteral {
            kind: "account ID",
            field: field.to_owned(),
        });
    }
    Ok(account)
}

#[cfg(feature = "json")]
fn parse_entrypoint_hash_literal(
    op: &str,
    field: &str,
    value: &Value,
) -> Result<HashOf<crate::transaction::signed::TransactionEntrypoint>, CommittedTxPredicateJsonError>
{
    let raw = value
        .as_str()
        .ok_or_else(|| invalid_value(op, field, "a canonical lowercase entrypoint hash"))?;
    let canonical = raw.to_ascii_lowercase();
    let hash = canonical
        .parse::<HashOf<crate::transaction::signed::TransactionEntrypoint>>()
        .map_err(|_| invalid_value(op, field, "a canonical lowercase entrypoint hash"))?;
    if hash.to_string() != raw {
        return Err(CommittedTxPredicateJsonError::NonCanonicalLiteral {
            kind: "entrypoint hash",
            field: field.to_owned(),
        });
    }
    Ok(hash)
}

#[cfg(feature = "json")]
fn parse_block_hash_literal(
    op: &str,
    field: &str,
    value: &Value,
) -> Result<HashOf<crate::block::BlockHeader>, CommittedTxPredicateJsonError> {
    let raw = value
        .as_str()
        .ok_or_else(|| invalid_value(op, field, "a canonical lowercase block hash"))?;
    let hash = raw
        .parse::<HashOf<crate::block::BlockHeader>>()
        .map_err(|_| invalid_value(op, field, "a canonical lowercase block hash"))?;
    if hash.to_string() != raw {
        return Err(CommittedTxPredicateJsonError::NonCanonicalLiteral {
            kind: "block hash",
            field: field.to_owned(),
        });
    }
    Ok(hash)
}

#[cfg(feature = "json")]
fn parse_metadata_literal(value: &Value) -> Result<Json, CommittedTxPredicateJsonError> {
    Json::from_norito_value_ref(value)
        .map_err(|_| CommittedTxPredicateJsonError::InvalidMetadataJson)
}

#[cfg(feature = "json")]
fn parse_equality_atom(
    op: &str,
    field_path: &str,
    field: CommittedTxField,
    value: &Value,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    use CommittedTxPredicate as P;

    Ok(match field {
        CommittedTxField::BlockHash => {
            let value = parse_block_hash_literal(op, field_path, value)?;
            if op == "eq" {
                P::BlockEq(value)
            } else {
                P::BlockNe(value)
            }
        }
        CommittedTxField::Authority => {
            let value = parse_account_literal(op, field_path, value)?;
            if op == "eq" {
                P::AuthorityEq(value)
            } else {
                P::AuthorityNe(value)
            }
        }
        CommittedTxField::Timestamp => {
            let value = value
                .as_u64()
                .ok_or_else(|| invalid_value(op, field_path, "an unsigned 64-bit integer"))?;
            if op == "eq" {
                P::TsEq(value)
            } else {
                return Err(CommittedTxPredicateJsonError::OperatorFieldMismatch {
                    op: op.to_owned(),
                    field: field_path.to_owned(),
                });
            }
        }
        CommittedTxField::EntrypointHash => {
            let value = parse_entrypoint_hash_literal(op, field_path, value)?;
            if op == "eq" {
                P::EntryEq(value)
            } else {
                P::EntryNe(value)
            }
        }
        CommittedTxField::ResultOk => {
            let value = value
                .as_bool()
                .ok_or_else(|| invalid_value(op, field_path, "a boolean"))?;
            if op == "eq" {
                P::ResultEq(value)
            } else {
                P::ResultNe(value)
            }
        }
        CommittedTxField::Metadata(key) => {
            let value = parse_metadata_literal(value)?;
            if op == "eq" {
                P::MetadataEq { key, value }
            } else {
                P::MetadataNe { key, value }
            }
        }
    })
}

#[cfg(feature = "json")]
fn parse_ordering_atom(
    op: &str,
    field_path: &str,
    field: &CommittedTxField,
    value: &Value,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    use CommittedTxPredicate as P;

    if !matches!(field, CommittedTxField::Timestamp) {
        return Err(CommittedTxPredicateJsonError::OperatorFieldMismatch {
            op: op.to_owned(),
            field: field_path.to_owned(),
        });
    }
    let value = value
        .as_u64()
        .ok_or_else(|| invalid_value(op, field_path, "an unsigned 64-bit integer"))?;
    Ok(match op {
        "lt" => P::TsLt(value),
        "lte" => P::TsLte(value),
        "gt" => P::TsGt(value),
        "gte" => P::TsGte(value),
        _ => unreachable!("caller restricts ordering operators"),
    })
}

#[cfg(feature = "json")]
fn parse_membership_atom(
    op: &str,
    field_path: &str,
    field: CommittedTxField,
    value: &Value,
    budget: &mut PredicateJsonBudget,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    use CommittedTxPredicate as P;

    let values = value
        .as_array()
        .ok_or_else(|| invalid_value(op, field_path, "a non-empty array"))?;
    budget.add_membership_values(field_path, values.len())?;
    Ok(match field {
        CommittedTxField::BlockHash => {
            let parsed = values
                .iter()
                .map(|value| parse_block_hash_literal(op, field_path, value))
                .collect::<Result<Vec<_>, _>>()?;
            if op == "in" {
                P::BlockIn(parsed)
            } else {
                P::BlockNin(parsed)
            }
        }
        CommittedTxField::Authority => {
            let parsed = values
                .iter()
                .map(|value| parse_account_literal(op, field_path, value))
                .collect::<Result<Vec<_>, _>>()?;
            if op == "in" {
                P::AuthorityIn(parsed)
            } else {
                P::AuthorityNin(parsed)
            }
        }
        CommittedTxField::Timestamp => {
            let parsed = values
                .iter()
                .map(|value| {
                    value.as_u64().ok_or_else(|| {
                        invalid_value(op, field_path, "an array of unsigned 64-bit integers")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if op == "in" {
                P::TsIn(parsed)
            } else {
                P::TsNin(parsed)
            }
        }
        CommittedTxField::EntrypointHash => {
            let parsed = values
                .iter()
                .map(|value| parse_entrypoint_hash_literal(op, field_path, value))
                .collect::<Result<Vec<_>, _>>()?;
            if op == "in" {
                P::EntryIn(parsed)
            } else {
                P::EntryNin(parsed)
            }
        }
        CommittedTxField::ResultOk => {
            let parsed = values
                .iter()
                .map(|value| {
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_value(op, field_path, "an array of booleans"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if op == "in" {
                P::ResultIn(parsed)
            } else {
                P::ResultNin(parsed)
            }
        }
        CommittedTxField::Metadata(key) => {
            let parsed = values
                .iter()
                .map(parse_metadata_literal)
                .collect::<Result<Vec<_>, _>>()?;
            if op == "in" {
                P::MetadataIn {
                    key,
                    values: parsed,
                }
            } else {
                P::MetadataNin {
                    key,
                    values: parsed,
                }
            }
        }
    })
}

#[cfg(feature = "json")]
fn parse_presence_atom(
    op: &str,
    field_path: &str,
    field: CommittedTxField,
    value: &Value,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    use CommittedTxPredicate as P;

    let flag = value
        .as_bool()
        .ok_or_else(|| invalid_value(op, field_path, "a boolean"))?;
    match (op, field) {
        ("exists", CommittedTxField::BlockHash) => Ok(P::BlockExists(flag)),
        ("exists", CommittedTxField::Authority) => Ok(P::AuthorityExists(flag)),
        ("exists", CommittedTxField::Timestamp) => Ok(P::TsExists(flag)),
        ("exists", CommittedTxField::EntrypointHash) => Ok(P::EntryExists(flag)),
        ("exists", CommittedTxField::ResultOk) => Ok(P::ResultExists(flag)),
        ("exists", CommittedTxField::Metadata(key)) => Ok(P::MetadataExists { key, exists: flag }),
        ("is_null", CommittedTxField::Metadata(key)) => {
            Ok(P::MetadataIsNull { key, is_null: flag })
        }
        _ => Err(CommittedTxPredicateJsonError::OperatorFieldMismatch {
            op: op.to_owned(),
            field: field_path.to_owned(),
        }),
    }
}

#[cfg(feature = "json")]
#[allow(clippy::too_many_lines)]
fn parse_committed_tx_predicate_inner(
    value: &Value,
    depth: usize,
    budget: &mut PredicateJsonBudget,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    use CommittedTxPredicate as P;

    budget.enter_node(depth)?;
    let object = value
        .as_object()
        .ok_or(CommittedTxPredicateJsonError::ExpectedObject)?;
    for key in object.keys() {
        if key != "op" && key != "args" {
            return Err(CommittedTxPredicateJsonError::UnknownField(key.clone()));
        }
    }
    let op = object
        .get("op")
        .ok_or(CommittedTxPredicateJsonError::MissingField("op"))?
        .as_str()
        .ok_or(CommittedTxPredicateJsonError::ExpectedOperatorString)?;
    let args = object
        .get("args")
        .ok_or(CommittedTxPredicateJsonError::MissingField("args"))?;

    match op {
        "and" | "or" => {
            let Some(children) = args.as_array() else {
                return Err(CommittedTxPredicateJsonError::InvalidArity {
                    op: op.to_owned(),
                    expected: "a non-empty array of predicate nodes",
                    actual: 0,
                });
            };
            if children.is_empty() {
                return Err(CommittedTxPredicateJsonError::InvalidArity {
                    op: op.to_owned(),
                    expected: "a non-empty array of predicate nodes",
                    actual: 0,
                });
            }
            if children.len() > MAX_COMMITTED_TX_PREDICATE_NODES.saturating_sub(budget.nodes) {
                return Err(CommittedTxPredicateJsonError::TooManyNodes(
                    MAX_COMMITTED_TX_PREDICATE_NODES,
                ));
            }
            let mut parsed_children = Vec::with_capacity(children.len());
            for child in children {
                parsed_children.push(parse_committed_tx_predicate_inner(
                    child,
                    depth + 1,
                    budget,
                )?);
            }
            if op == "and" {
                Ok(P::And(parsed_children))
            } else {
                Ok(P::Or(parsed_children))
            }
        }
        "not" => {
            let args = exact_args(op, args, 1, "exactly one predicate argument")?;
            Ok(P::Not(Box::new(parse_committed_tx_predicate_inner(
                &args[0],
                depth + 1,
                budget,
            )?)))
        }
        "const" => {
            let args = exact_args(op, args, 1, "exactly one boolean argument")?;
            let value = args[0]
                .as_bool()
                .ok_or_else(|| invalid_value(op, "const", "a boolean"))?;
            Ok(P::Const(value))
        }
        "eq" | "ne" | "lt" | "lte" | "gt" | "gte" | "in" | "nin" | "exists" | "is_null" => {
            let args = exact_args(op, args, 2, "exactly [field, value]")?;
            let field_path = args[0]
                .as_str()
                .ok_or(CommittedTxPredicateJsonError::ExpectedFieldString)?;
            let field = parse_committed_tx_field(&args[0])?;
            match op {
                "eq" | "ne" => parse_equality_atom(op, field_path, field, &args[1]),
                "lt" | "lte" | "gt" | "gte" => {
                    parse_ordering_atom(op, field_path, &field, &args[1])
                }
                "in" | "nin" => parse_membership_atom(op, field_path, field, &args[1], budget),
                "exists" | "is_null" => parse_presence_atom(op, field_path, field, &args[1]),
                _ => unreachable!("operator was exhaustively matched"),
            }
        }
        other => Err(CommittedTxPredicateJsonError::UnsupportedOperator(
            other.to_owned(),
        )),
    }
}

/// Parse a validated committed-transaction app-expression JSON value.
#[cfg(feature = "json")]
pub(super) fn committed_tx_predicate_from_value(
    value: &Value,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    let predicate =
        parse_committed_tx_predicate_inner(value, 1, &mut PredicateJsonBudget::default())?;
    validate_committed_tx_predicate(&predicate).map_err(|error| match error {
        CommittedTxPredicateValidationError::EmptyLogical(op) => {
            CommittedTxPredicateJsonError::InvalidArity {
                op: op.to_owned(),
                expected: "a non-empty array of predicate nodes",
                actual: 0,
            }
        }
        CommittedTxPredicateValidationError::EmptyMembership(field) => {
            CommittedTxPredicateJsonError::EmptyMembership(field.to_owned())
        }
        CommittedTxPredicateValidationError::DuplicateMembership(field) => {
            CommittedTxPredicateJsonError::DuplicateMembership(field.to_owned())
        }
        CommittedTxPredicateValidationError::InvalidMetadataJson => {
            CommittedTxPredicateJsonError::InvalidMetadataJson
        }
        CommittedTxPredicateValidationError::TooManyMembershipValues { field, max } => {
            CommittedTxPredicateJsonError::TooManyMembershipValues {
                field: field.to_owned(),
                max,
            }
        }
        CommittedTxPredicateValidationError::TooManyTotalMembershipValues(max) => {
            CommittedTxPredicateJsonError::TooManyTotalMembershipValues(max)
        }
        CommittedTxPredicateValidationError::TooDeep(max) => {
            CommittedTxPredicateJsonError::TooDeep(max)
        }
        CommittedTxPredicateValidationError::TooManyNodes(max) => {
            CommittedTxPredicateJsonError::TooManyNodes(max)
        }
    })?;
    Ok(predicate)
}

#[cfg(feature = "json")]
fn metadata_field_path(key: &Name) -> String {
    format!("metadata.{key}")
}

#[cfg(feature = "json")]
fn metadata_json_value(value: &Json) -> Value {
    value
        .try_into_any_norito::<Value>()
        .expect("Json values maintain their documented validity invariant")
}

#[cfg(feature = "json")]
#[allow(clippy::too_many_lines)]
fn committed_tx_predicate_to_value_unchecked(predicate: &CommittedTxPredicate) -> Value {
    use CommittedTxPredicate as P;

    match predicate {
        P::And(children) => predicate_expr(
            "and",
            children
                .iter()
                .map(committed_tx_predicate_to_value_unchecked)
                .collect(),
        ),
        P::Or(children) => predicate_expr(
            "or",
            children
                .iter()
                .map(committed_tx_predicate_to_value_unchecked)
                .collect(),
        ),
        P::Not(inner) => predicate_expr(
            "not",
            vec![committed_tx_predicate_to_value_unchecked(inner)],
        ),
        P::BlockEq(value) => {
            binary_predicate_expr("eq", "block_hash", Value::String(value.to_string()))
        }
        P::BlockNe(value) => {
            binary_predicate_expr("ne", "block_hash", Value::String(value.to_string()))
        }
        P::BlockIn(values) => binary_predicate_expr(
            "in",
            "block_hash",
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        ),
        P::BlockNin(values) => binary_predicate_expr(
            "nin",
            "block_hash",
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        ),
        P::BlockExists(value) => binary_predicate_expr("exists", "block_hash", Value::Bool(*value)),
        P::AuthorityEq(value) => {
            binary_predicate_expr("eq", "authority", Value::String(value.to_string()))
        }
        P::AuthorityNe(value) => {
            binary_predicate_expr("ne", "authority", Value::String(value.to_string()))
        }
        P::AuthorityIn(values) => binary_predicate_expr(
            "in",
            "authority",
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        ),
        P::AuthorityNin(values) => binary_predicate_expr(
            "nin",
            "authority",
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        ),
        P::AuthorityExists(value) => {
            binary_predicate_expr("exists", "authority", Value::Bool(*value))
        }
        P::TsEq(value) => binary_predicate_expr("eq", "timestamp_ms", Value::from(*value)),
        P::TsLt(value) => binary_predicate_expr("lt", "timestamp_ms", Value::from(*value)),
        P::TsLte(value) => binary_predicate_expr("lte", "timestamp_ms", Value::from(*value)),
        P::TsGt(value) => binary_predicate_expr("gt", "timestamp_ms", Value::from(*value)),
        P::TsGte(value) => binary_predicate_expr("gte", "timestamp_ms", Value::from(*value)),
        P::TsIn(values) => binary_predicate_expr(
            "in",
            "timestamp_ms",
            Value::Array(values.iter().copied().map(Value::from).collect()),
        ),
        P::TsNin(values) => binary_predicate_expr(
            "nin",
            "timestamp_ms",
            Value::Array(values.iter().copied().map(Value::from).collect()),
        ),
        P::TsExists(value) => binary_predicate_expr("exists", "timestamp_ms", Value::Bool(*value)),
        P::EntryEq(value) => {
            binary_predicate_expr("eq", "entrypoint_hash", Value::String(value.to_string()))
        }
        P::EntryNe(value) => {
            binary_predicate_expr("ne", "entrypoint_hash", Value::String(value.to_string()))
        }
        P::EntryIn(values) => binary_predicate_expr(
            "in",
            "entrypoint_hash",
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        ),
        P::EntryNin(values) => binary_predicate_expr(
            "nin",
            "entrypoint_hash",
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        ),
        P::EntryExists(value) => {
            binary_predicate_expr("exists", "entrypoint_hash", Value::Bool(*value))
        }
        P::ResultEq(value) => binary_predicate_expr("eq", "result_ok", Value::Bool(*value)),
        P::ResultNe(value) => binary_predicate_expr("ne", "result_ok", Value::Bool(*value)),
        P::ResultIn(values) => binary_predicate_expr(
            "in",
            "result_ok",
            Value::Array(values.iter().copied().map(Value::Bool).collect()),
        ),
        P::ResultNin(values) => binary_predicate_expr(
            "nin",
            "result_ok",
            Value::Array(values.iter().copied().map(Value::Bool).collect()),
        ),
        P::ResultExists(value) => binary_predicate_expr("exists", "result_ok", Value::Bool(*value)),
        P::MetadataEq { key, value } => {
            binary_predicate_expr("eq", metadata_field_path(key), metadata_json_value(value))
        }
        P::MetadataNe { key, value } => {
            binary_predicate_expr("ne", metadata_field_path(key), metadata_json_value(value))
        }
        P::MetadataIn { key, values } => binary_predicate_expr(
            "in",
            metadata_field_path(key),
            Value::Array(values.iter().map(metadata_json_value).collect()),
        ),
        P::MetadataNin { key, values } => binary_predicate_expr(
            "nin",
            metadata_field_path(key),
            Value::Array(values.iter().map(metadata_json_value).collect()),
        ),
        P::MetadataExists { key, exists } => {
            binary_predicate_expr("exists", metadata_field_path(key), Value::Bool(*exists))
        }
        P::MetadataIsNull { key, is_null } => {
            binary_predicate_expr("is_null", metadata_field_path(key), Value::Bool(*is_null))
        }
        P::Const(value) => predicate_expr("const", vec![Value::Bool(*value)]),
    }
}

/// Convert a validated committed-transaction predicate to its canonical app-expression value.
#[cfg(feature = "json")]
pub(super) fn committed_tx_predicate_to_value(
    predicate: &CommittedTxPredicate,
) -> Result<Value, CommittedTxPredicateJsonError> {
    validate_committed_tx_predicate(predicate)
        .map_err(|error| CommittedTxPredicateJsonError::InvalidTree(error.to_string()))?;
    Ok(committed_tx_predicate_to_value_unchecked(predicate))
}

/// Parse raw JSON and require the exact canonical app-expression encoding.
#[cfg(feature = "json")]
pub(super) fn committed_tx_predicate_from_canonical_json(
    raw: &str,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    let value = json::from_json::<Value>(raw)
        .map_err(|error| CommittedTxPredicateJsonError::MalformedJson(error.to_string()))?;
    let predicate = committed_tx_predicate_from_value(&value)?;
    let canonical = json::to_json(&committed_tx_predicate_to_value(&predicate)?)
        .map_err(|error| CommittedTxPredicateJsonError::MalformedJson(error.to_string()))?;
    if raw != canonical {
        return Err(CommittedTxPredicateJsonError::NonCanonicalWireEncoding);
    }
    Ok(predicate)
}

/// Convert the generic builder schema into the typed committed-transaction tree.
#[cfg(feature = "json")]
pub(super) fn committed_tx_predicate_from_predicate_json(
    predicate: &crate::query::json::PredicateJson,
) -> Result<CommittedTxPredicate, CommittedTxPredicateJsonError> {
    let child_count = predicate
        .equals
        .len()
        .checked_add(predicate.r#in.len())
        .and_then(|count| count.checked_add(predicate.exists.len()))
        .ok_or(CommittedTxPredicateJsonError::TooManyNodes(
            MAX_COMMITTED_TX_PREDICATE_NODES,
        ))?;
    let required_nodes = child_count.saturating_add(usize::from(child_count > 1));
    if required_nodes > MAX_COMMITTED_TX_PREDICATE_NODES {
        return Err(CommittedTxPredicateJsonError::TooManyNodes(
            MAX_COMMITTED_TX_PREDICATE_NODES,
        ));
    }
    let mut membership_values = 0usize;
    for condition in &predicate.r#in {
        if condition.values.is_empty() {
            return Err(CommittedTxPredicateJsonError::EmptyMembership(
                condition.field.clone(),
            ));
        }
        if condition.values.len() > MAX_COMMITTED_TX_MEMBERSHIP_VALUES {
            return Err(CommittedTxPredicateJsonError::TooManyMembershipValues {
                field: condition.field.clone(),
                max: MAX_COMMITTED_TX_MEMBERSHIP_VALUES,
            });
        }
        membership_values = membership_values.saturating_add(condition.values.len());
        if membership_values > MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES {
            return Err(CommittedTxPredicateJsonError::TooManyTotalMembershipValues(
                MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES,
            ));
        }
    }

    let mut children = Vec::with_capacity(child_count);
    children.extend(
        predicate.equals.iter().map(|condition| {
            binary_predicate_expr("eq", &condition.field, condition.value.clone())
        }),
    );
    children.extend(predicate.r#in.iter().map(|condition| {
        binary_predicate_expr(
            "in",
            &condition.field,
            Value::Array(condition.values.clone()),
        )
    }));
    children.extend(
        predicate
            .exists
            .iter()
            .map(|field| binary_predicate_expr("exists", field, Value::Bool(true))),
    );

    match children.len() {
        0 => Ok(CommittedTxPredicate::Const(true)),
        1 => committed_tx_predicate_from_value(&children[0]),
        _ => committed_tx_predicate_from_value(&predicate_expr("and", children)),
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for CommittedTxPredicate {
    fn json_serialize(&self, out: &mut String) {
        match committed_tx_predicate_to_value(self) {
            Ok(value) => value.json_serialize(out),
            Err(_) => {
                committed_tx_predicate_to_value_unchecked(&Self::Const(false)).json_serialize(out)
            }
        }
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for CommittedTxPredicate {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        committed_tx_predicate_from_value(&value)
            .map_err(|error| json::Error::Message(error.to_string()))
    }
}

/// Convert the legacy flat filter representation into the lossless predicate tree.
pub(super) fn committed_tx_predicate_from_filters(
    filters: impl std::borrow::Borrow<CommittedTxFilters>,
) -> CommittedTxPredicate {
    use CommittedTxPredicate as P;

    let filters = filters.borrow();
    let mut parts = Vec::new();
    if let Some(value) = filters.block_eq.as_ref() {
        parts.push(P::BlockEq(*value));
    }
    if let Some(value) = filters.block_ne.as_ref() {
        parts.push(P::BlockNe(*value));
    }
    if !filters.block_in.is_empty() {
        parts.push(P::BlockIn(filters.block_in.clone()));
    }
    if !filters.block_nin.is_empty() {
        parts.push(P::BlockNin(filters.block_nin.clone()));
    }
    if let Some(value) = filters.block_exists {
        parts.push(P::BlockExists(value));
    }
    if let Some(value) = filters.authority_eq.as_ref() {
        parts.push(P::AuthorityEq(value.clone()));
    }
    if let Some(value) = filters.authority_ne.as_ref() {
        parts.push(P::AuthorityNe(value.clone()));
    }
    if !filters.authority_in.is_empty() {
        parts.push(P::AuthorityIn(filters.authority_in.clone()));
    }
    if !filters.authority_nin.is_empty() {
        parts.push(P::AuthorityNin(filters.authority_nin.clone()));
    }
    if let Some(value) = filters.authority_exists {
        parts.push(P::AuthorityExists(value));
    }
    if let Some(value) = filters.ts_ge {
        parts.push(P::TsGte(value));
    }
    if let Some(value) = filters.ts_le {
        parts.push(P::TsLte(value));
    }
    if let Some(value) = filters.entry_eq.as_ref() {
        parts.push(P::EntryEq(*value));
    }
    if let Some(value) = filters.entry_ne.as_ref() {
        parts.push(P::EntryNe(*value));
    }
    if !filters.entry_in.is_empty() {
        parts.push(P::EntryIn(filters.entry_in.clone()));
    }
    if !filters.entry_nin.is_empty() {
        parts.push(P::EntryNin(filters.entry_nin.clone()));
    }
    if let Some(value) = filters.entry_exists {
        parts.push(P::EntryExists(value));
    }
    if let Some(value) = filters.result_ok {
        parts.push(P::ResultEq(value));
    }
    if let Some(value) = filters.result_ok_ne {
        parts.push(P::ResultNe(value));
    }
    if !filters.result_ok_in.is_empty() {
        parts.push(P::ResultIn(filters.result_ok_in.clone()));
    }
    if !filters.result_ok_nin.is_empty() {
        parts.push(P::ResultNin(filters.result_ok_nin.clone()));
    }
    if let Some(value) = filters.result_exists {
        parts.push(P::ResultExists(value));
    }

    match parts.len() {
        0 => P::Const(true),
        1 => parts.into_iter().next().expect("non-empty parts"),
        _ => P::And(parts),
    }
}

/// Recover the legacy flat filter view when a typed tree can be represented
/// without dropping or weakening any condition.
///
/// This is used only as an index-planning hint. The typed predicate remains the
/// authoritative filter evaluated against every candidate transaction.
pub(super) fn committed_tx_filters_from_predicate(
    predicate: &CommittedTxPredicate,
) -> Option<CommittedTxFilters> {
    use CommittedTxPredicate as P;

    fn set_same_or_empty<T: Clone + PartialEq>(slot: &mut Option<T>, value: &T) -> Option<()> {
        match slot {
            Some(current) if current != value => None,
            Some(_) => Some(()),
            None => {
                *slot = Some(value.clone());
                Some(())
            }
        }
    }

    fn set_vec_same_or_empty<T: Clone + PartialEq>(slot: &mut Vec<T>, value: &[T]) -> Option<()> {
        if slot.is_empty() {
            slot.extend_from_slice(value);
            Some(())
        } else if slot == value {
            Some(())
        } else {
            None
        }
    }

    fn add(predicate: &P, filters: &mut CommittedTxFilters) -> Option<()> {
        match predicate {
            P::And(children) => {
                for child in children {
                    add(child, filters)?;
                }
                Some(())
            }
            P::BlockEq(value) => set_same_or_empty(&mut filters.block_eq, value),
            P::BlockNe(value) => set_same_or_empty(&mut filters.block_ne, value),
            P::BlockIn(values) => set_vec_same_or_empty(&mut filters.block_in, values),
            P::BlockNin(values) => set_vec_same_or_empty(&mut filters.block_nin, values),
            P::BlockExists(value) => set_same_or_empty(&mut filters.block_exists, value),
            P::AuthorityEq(value) => set_same_or_empty(&mut filters.authority_eq, value),
            P::AuthorityNe(value) => set_same_or_empty(&mut filters.authority_ne, value),
            P::AuthorityIn(values) => set_vec_same_or_empty(&mut filters.authority_in, values),
            P::AuthorityNin(values) => set_vec_same_or_empty(&mut filters.authority_nin, values),
            P::AuthorityExists(value) => set_same_or_empty(&mut filters.authority_exists, value),
            P::TsEq(value) => {
                filters.ts_ge = Some(filters.ts_ge.map_or(*value, |current| current.max(*value)));
                filters.ts_le = Some(filters.ts_le.map_or(*value, |current| current.min(*value)));
                Some(())
            }
            P::TsLt(value) => {
                let upper = value.checked_sub(1)?;
                filters.ts_le = Some(filters.ts_le.map_or(upper, |current| current.min(upper)));
                Some(())
            }
            P::TsLte(value) => {
                filters.ts_le = Some(filters.ts_le.map_or(*value, |current| current.min(*value)));
                Some(())
            }
            P::TsGt(value) => {
                let lower = value.checked_add(1)?;
                filters.ts_ge = Some(filters.ts_ge.map_or(lower, |current| current.max(lower)));
                Some(())
            }
            P::TsGte(value) => {
                filters.ts_ge = Some(filters.ts_ge.map_or(*value, |current| current.max(*value)));
                Some(())
            }
            P::EntryEq(value) => set_same_or_empty(&mut filters.entry_eq, value),
            P::EntryNe(value) => set_same_or_empty(&mut filters.entry_ne, value),
            P::EntryIn(values) => set_vec_same_or_empty(&mut filters.entry_in, values),
            P::EntryNin(values) => set_vec_same_or_empty(&mut filters.entry_nin, values),
            P::EntryExists(value) => set_same_or_empty(&mut filters.entry_exists, value),
            P::ResultEq(value) => set_same_or_empty(&mut filters.result_ok, value),
            P::ResultNe(value) => set_same_or_empty(&mut filters.result_ok_ne, value),
            P::ResultIn(values) => set_vec_same_or_empty(&mut filters.result_ok_in, values),
            P::ResultNin(values) => set_vec_same_or_empty(&mut filters.result_ok_nin, values),
            P::ResultExists(value) => set_same_or_empty(&mut filters.result_exists, value),
            P::Const(true) => Some(()),
            P::Or(_)
            | P::Not(_)
            | P::TsIn(_)
            | P::TsNin(_)
            | P::TsExists(_)
            | P::MetadataEq { .. }
            | P::MetadataNe { .. }
            | P::MetadataIn { .. }
            | P::MetadataNin { .. }
            | P::MetadataExists { .. }
            | P::MetadataIsNull { .. }
            | P::Const(false) => None,
        }
    }

    validate_committed_tx_predicate(predicate).ok()?;
    let mut filters = CommittedTxFilters::default();
    add(predicate, &mut filters)?;
    Some(filters)
}

impl CommittedTxPredicate {
    fn authority_of(tx: &CommittedTransaction) -> Option<crate::account::AccountId> {
        tx.entrypoint.authority_opt().cloned()
    }

    fn timestamp_ms_of(tx: &CommittedTransaction) -> Option<u64> {
        tx.entrypoint.creation_time_ms()
    }

    fn metadata_value<'tx>(tx: &'tx CommittedTransaction, key: &Name) -> Option<&'tx Json> {
        tx.entrypoint
            .metadata()
            .and_then(|metadata| metadata.get(key))
    }

    fn metadata_json_value(json: &Json) -> Option<norito::json::Value> {
        json.try_into_any_norito::<norito::json::Value>().ok()
    }

    /// Evaluate the predicate against the provided committed transaction.
    #[must_use]
    pub fn applies(&self, tx: &CommittedTransaction) -> bool {
        validate_committed_tx_predicate(self).is_ok() && self.applies_unchecked(tx)
    }

    fn applies_unchecked(&self, tx: &CommittedTransaction) -> bool {
        use CommittedTxPredicate as P;
        match self {
            P::Const(v) => *v,
            P::And(list) => list.iter().all(|p| p.applies_unchecked(tx)),
            P::Or(list) => list.iter().any(|p| p.applies_unchecked(tx)),
            P::Not(inner) => !inner.applies_unchecked(tx),

            // Carrier block hash (always present)
            P::BlockEq(hash) => &tx.block_hash == hash,
            P::BlockNe(hash) => &tx.block_hash != hash,
            P::BlockIn(list) => list.iter().any(|hash| hash == &tx.block_hash),
            P::BlockNin(list) => !list.iter().any(|hash| hash == &tx.block_hash),
            P::BlockExists(required) => *required,

            // Authority
            P::AuthorityExists(req) => (Self::authority_of(tx).is_some()) == *req,
            P::AuthorityEq(a) => Self::authority_of(tx).as_ref() == Some(a),
            P::AuthorityNe(a) => Self::authority_of(tx).as_ref() != Some(a),
            P::AuthorityIn(list) => Self::authority_of(tx)
                .as_ref()
                .is_some_and(|a| list.iter().any(|x| x == a)),
            P::AuthorityNin(list) => Self::authority_of(tx)
                .as_ref()
                .is_none_or(|a| !list.iter().any(|x| x == a)),

            // Timestamp (None for triggers)
            P::TsEq(n) => Self::timestamp_ms_of(tx) == Some(*n),
            P::TsLt(n) => Self::timestamp_ms_of(tx).is_some_and(|m| m < *n),
            P::TsLte(n) => Self::timestamp_ms_of(tx).is_some_and(|m| m <= *n),
            P::TsGt(n) => Self::timestamp_ms_of(tx).is_some_and(|m| m > *n),
            P::TsGte(n) => Self::timestamp_ms_of(tx).is_some_and(|m| m >= *n),
            P::TsIn(list) => Self::timestamp_ms_of(tx).is_some_and(|m| list.contains(&m)),
            P::TsNin(list) => Self::timestamp_ms_of(tx).is_none_or(|m| !list.contains(&m)),
            P::TsExists(req) => (Self::timestamp_ms_of(tx).is_some()) == *req,

            // Entrypoint hash (always present)
            P::EntryEq(h) => &tx.entrypoint_hash == h,
            P::EntryNe(h) => &tx.entrypoint_hash != h,
            P::EntryIn(list) => list.iter().any(|x| x == &tx.entrypoint_hash),
            P::EntryNin(list) => !list.iter().any(|x| x == &tx.entrypoint_hash),
            P::EntryExists(req) | P::ResultExists(req) => *req,
            // Result .is_ok()
            P::ResultEq(b) => tx.result.as_ref().is_ok() == *b,
            P::ResultNe(b) => tx.result.as_ref().is_ok() != *b,
            P::ResultIn(list) => list.contains(&tx.result.as_ref().is_ok()),
            P::ResultNin(list) => !list.contains(&tx.result.as_ref().is_ok()),
            // Metadata map comparisons (External entrypoints only)
            P::MetadataExists { key, exists } => Self::metadata_value(tx, key).is_some() == *exists,
            P::MetadataEq { key, value } => {
                Self::metadata_value(tx, key).is_some_and(|json| json.get() == value.get())
            }
            P::MetadataNe { key, value } => {
                Self::metadata_value(tx, key).is_none_or(|json| json.get() != value.get())
            }
            P::MetadataIn { key, values } => {
                let Some(actual) = Self::metadata_value(tx, key) else {
                    return false;
                };
                values
                    .iter()
                    .any(|candidate| candidate.get() == actual.get())
            }
            P::MetadataNin { key, values } => Self::metadata_value(tx, key).is_none_or(|actual| {
                !values
                    .iter()
                    .any(|candidate| candidate.get() == actual.get())
            }),
            P::MetadataIsNull { key, is_null } => {
                let Some(value) = Self::metadata_value(tx, key).and_then(Self::metadata_json_value)
                else {
                    return false;
                };
                if *is_null {
                    value.is_null()
                } else {
                    !value.is_null()
                }
            }
        }
    }
}

mod wire {
    use std::cell::Cell;

    use iroha_crypto::HashOf;
    use iroha_primitives::json::Json;
    use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
    use norito::{NoritoDeserialize, NoritoSerialize, core::Error};

    use super::{
        CommittedTxPredicate, MAX_COMMITTED_TX_MEMBERSHIP_VALUES, MAX_COMMITTED_TX_PREDICATE_DEPTH,
        MAX_COMMITTED_TX_PREDICATE_NODES, MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES,
        validate_committed_tx_predicate,
    };
    use crate::name::Name;

    thread_local! {
        /// Remaining aggregate membership literals while decoding one predicate.
        ///
        /// The outer node stream is always decoded serially, so every nested
        /// membership decoder observes this same scoped budget before it asks
        /// the allocator to materialize its vector.
        static MEMBERSHIP_DECODE_REMAINING: Cell<Option<usize>> = const { Cell::new(None) };
    }

    struct MembershipDecodeBudgetGuard {
        previous: Option<usize>,
    }

    impl MembershipDecodeBudgetGuard {
        fn enter() -> Self {
            let previous = MEMBERSHIP_DECODE_REMAINING.with(|remaining| {
                remaining.replace(Some(MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES))
            });
            Self { previous }
        }
    }

    impl Drop for MembershipDecodeBudgetGuard {
        fn drop(&mut self) {
            MEMBERSHIP_DECODE_REMAINING.with(|remaining| remaining.set(self.previous));
        }
    }

    fn claim_membership_decode_budget(count: usize) -> Result<(), Error> {
        MEMBERSHIP_DECODE_REMAINING.with(|remaining| {
            let Some(available) = remaining.get() else {
                return Err(Error::Message(
                    "CommittedTxPredicate membership decoded outside its predicate budget".into(),
                ));
            };
            let next = available.checked_sub(count).ok_or_else(|| {
                Error::Message(format!(
                    "CommittedTxPredicate membership literals exceed the limit of {MAX_COMMITTED_TX_TOTAL_MEMBERSHIP_VALUES}"
                ))
            })?;
            remaining.set(Some(next));
            Ok(())
        })
    }

    fn archived_sequence_bytes<T>(
        archived: &norito::core::Archived<Vec<T>>,
    ) -> Result<&'static [u8], Error> {
        norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())
    }

    fn ensure_sequence_len_at_most<T>(
        archived: &norito::core::Archived<Vec<T>>,
        max: usize,
        label: &str,
    ) -> Result<(), Error> {
        let bytes = archived_sequence_bytes(archived)?;
        let (count, _) = norito::core::read_seq_len_slice(bytes)?;
        if count > max {
            return Err(Error::Message(format!(
                "CommittedTxPredicate {label} exceed the limit of {max}"
            )));
        }
        Ok(())
    }

    #[derive(Clone)]
    pub(super) struct MembershipValues<T>(Vec<T>);

    impl<T> From<Vec<T>> for MembershipValues<T> {
        fn from(values: Vec<T>) -> Self {
            Self(values)
        }
    }

    impl<T: NoritoSerialize> NoritoSerialize for MembershipValues<T> {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
            self.0.serialize(writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            self.0.encoded_len_exact()
        }
    }

    impl<'de, T> NoritoDeserialize<'de> for MembershipValues<T>
    where
        T: NoritoSerialize + for<'a> NoritoDeserialize<'a>,
    {
        fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
            Self::try_deserialize(archived)
                .expect("CommittedTxPredicate membership values must deserialize")
        }

        fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, Error> {
            let archived = archived.cast::<Vec<T>>();
            ensure_sequence_len_at_most(
                archived,
                MAX_COMMITTED_TX_MEMBERSHIP_VALUES,
                "membership values",
            )?;
            let bytes = archived_sequence_bytes(archived)?;
            let (count, _) = norito::core::read_seq_len_slice(bytes)?;
            claim_membership_decode_budget(count)?;
            let (values, _) = norito::core::decode_vec_from_slice_serial::<T>(bytes)?;
            Ok(Self(values))
        }
    }

    #[derive(Clone, NoritoSerialize, NoritoDeserialize)]
    pub(super) enum Node {
        And { child_count: u32 },
        Or { child_count: u32 },
        Not,
        AuthorityEq(crate::account::AccountId),
        AuthorityNe(crate::account::AccountId),
        AuthorityIn(MembershipValues<crate::account::AccountId>),
        AuthorityNin(MembershipValues<crate::account::AccountId>),
        AuthorityExists(bool),
        TsEq(u64),
        TsLt(u64),
        TsLte(u64),
        TsGt(u64),
        TsGte(u64),
        TsIn(MembershipValues<u64>),
        TsNin(MembershipValues<u64>),
        TsExists(bool),
        EntryEq(HashOf<crate::transaction::signed::TransactionEntrypoint>),
        EntryNe(HashOf<crate::transaction::signed::TransactionEntrypoint>),
        EntryIn(MembershipValues<HashOf<crate::transaction::signed::TransactionEntrypoint>>),
        EntryNin(MembershipValues<HashOf<crate::transaction::signed::TransactionEntrypoint>>),
        EntryExists(bool),
        ResultEq(bool),
        ResultNe(bool),
        ResultIn(MembershipValues<bool>),
        ResultNin(MembershipValues<bool>),
        ResultExists(bool),
        MetadataEq(Name, Json),
        MetadataNe(Name, Json),
        MetadataIn(Name, MembershipValues<Json>),
        MetadataNin(Name, MembershipValues<Json>),
        MetadataExists(Name, bool),
        MetadataIsNull(Name, bool),
        Const(bool),
        BlockEq(HashOf<crate::block::BlockHeader>),
        BlockNe(HashOf<crate::block::BlockHeader>),
        BlockIn(MembershipValues<HashOf<crate::block::BlockHeader>>),
        BlockNin(MembershipValues<HashOf<crate::block::BlockHeader>>),
        BlockExists(bool),
    }

    impl TypeId for Node {
        fn id() -> iroha_schema::Ident {
            std::any::type_name::<Self>().to_owned()
        }
    }

    impl IntoSchema for Node {
        fn type_name() -> iroha_schema::Ident {
            "CommittedTxPredicateNode".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta { types: vec![] }));
        }
    }

    pub(super) fn flatten(root: &CommittedTxPredicate) -> Result<Vec<Node>, Error> {
        validate_committed_tx_predicate(root)
            .map_err(|error| Error::Message(format!("invalid CommittedTxPredicate: {error}")))?;
        let mut nodes = Vec::new();
        flatten_inner(root, &mut nodes)?;
        Ok(nodes)
    }

    pub(super) fn inflate(nodes: &[Node]) -> Result<CommittedTxPredicate, Error> {
        if nodes.is_empty() {
            return Err(Error::LengthMismatch);
        }
        if nodes.len() > MAX_COMMITTED_TX_PREDICATE_NODES {
            return Err(Error::Message(format!(
                "CommittedTxPredicate nodes exceed the limit of {MAX_COMMITTED_TX_PREDICATE_NODES}"
            )));
        }
        let mut cursor = 0usize;
        let tree = inflate_inner(nodes, &mut cursor, 1)?;
        if cursor != nodes.len() {
            return Err(Error::Message(
                "CommittedTxPredicate: trailing nodes".into(),
            ));
        }
        validate_committed_tx_predicate(&tree)
            .map_err(|error| Error::Message(format!("invalid CommittedTxPredicate: {error}")))?;
        Ok(tree)
    }

    fn flatten_inner(node: &CommittedTxPredicate, out: &mut Vec<Node>) -> Result<(), Error> {
        use CommittedTxPredicate as P;
        match node {
            P::And(children) => {
                let count = u32::try_from(children.len()).map_err(|_| {
                    Error::Message("CommittedTxPredicate And arity overflow".into())
                })?;
                out.push(Node::And { child_count: count });
                for child in children {
                    flatten_inner(child, out)?;
                }
            }
            P::Or(children) => {
                let count = u32::try_from(children.len())
                    .map_err(|_| Error::Message("CommittedTxPredicate Or arity overflow".into()))?;
                out.push(Node::Or { child_count: count });
                for child in children {
                    flatten_inner(child, out)?;
                }
            }
            P::Not(inner) => {
                out.push(Node::Not);
                flatten_inner(inner, out)?;
            }
            P::BlockEq(hash) => out.push(Node::BlockEq(*hash)),
            P::BlockNe(hash) => out.push(Node::BlockNe(*hash)),
            P::BlockIn(values) => out.push(Node::BlockIn(values.clone().into())),
            P::BlockNin(values) => out.push(Node::BlockNin(values.clone().into())),
            P::BlockExists(flag) => out.push(Node::BlockExists(*flag)),
            P::AuthorityEq(id) => out.push(Node::AuthorityEq(id.clone())),
            P::AuthorityNe(id) => out.push(Node::AuthorityNe(id.clone())),
            P::AuthorityIn(ids) => out.push(Node::AuthorityIn(ids.clone().into())),
            P::AuthorityNin(ids) => out.push(Node::AuthorityNin(ids.clone().into())),
            P::AuthorityExists(flag) => out.push(Node::AuthorityExists(*flag)),
            P::TsEq(v) => out.push(Node::TsEq(*v)),
            P::TsLt(v) => out.push(Node::TsLt(*v)),
            P::TsLte(v) => out.push(Node::TsLte(*v)),
            P::TsGt(v) => out.push(Node::TsGt(*v)),
            P::TsGte(v) => out.push(Node::TsGte(*v)),
            P::TsIn(values) => out.push(Node::TsIn(values.clone().into())),
            P::TsNin(values) => out.push(Node::TsNin(values.clone().into())),
            P::TsExists(flag) => out.push(Node::TsExists(*flag)),
            P::EntryEq(hash) => out.push(Node::EntryEq(*hash)),
            P::EntryNe(hash) => out.push(Node::EntryNe(*hash)),
            P::EntryIn(values) => out.push(Node::EntryIn(values.clone().into())),
            P::EntryNin(values) => out.push(Node::EntryNin(values.clone().into())),
            P::EntryExists(flag) => out.push(Node::EntryExists(*flag)),
            P::ResultEq(flag) => out.push(Node::ResultEq(*flag)),
            P::ResultNe(flag) => out.push(Node::ResultNe(*flag)),
            P::ResultIn(values) => out.push(Node::ResultIn(values.clone().into())),
            P::ResultNin(values) => out.push(Node::ResultNin(values.clone().into())),
            P::ResultExists(flag) => out.push(Node::ResultExists(*flag)),
            P::MetadataEq { key, value } => out.push(Node::MetadataEq(key.clone(), value.clone())),
            P::MetadataNe { key, value } => out.push(Node::MetadataNe(key.clone(), value.clone())),
            P::MetadataIn { key, values } => {
                out.push(Node::MetadataIn(key.clone(), values.clone().into()))
            }
            P::MetadataNin { key, values } => {
                out.push(Node::MetadataNin(key.clone(), values.clone().into()))
            }
            P::MetadataExists { key, exists } => {
                out.push(Node::MetadataExists(key.clone(), *exists))
            }
            P::MetadataIsNull { key, is_null } => {
                out.push(Node::MetadataIsNull(key.clone(), *is_null))
            }
            P::Const(flag) => out.push(Node::Const(*flag)),
        }
        Ok(())
    }

    fn inflate_inner(
        nodes: &[Node],
        cursor: &mut usize,
        depth: usize,
    ) -> Result<CommittedTxPredicate, Error> {
        use CommittedTxPredicate as P;

        if depth > MAX_COMMITTED_TX_PREDICATE_DEPTH {
            return Err(Error::Message(format!(
                "CommittedTxPredicate depth exceeds the limit of {MAX_COMMITTED_TX_PREDICATE_DEPTH}"
            )));
        }
        let node = nodes.get(*cursor).ok_or(Error::LengthMismatch)?;
        *cursor += 1;
        match node {
            Node::And { child_count } => {
                let child_count =
                    usize::try_from(*child_count).map_err(|_| Error::LengthMismatch)?;
                if child_count == 0 || child_count > nodes.len().saturating_sub(*cursor) {
                    return Err(Error::LengthMismatch);
                }
                let mut children = Vec::with_capacity(child_count);
                for _ in 0..child_count {
                    children.push(inflate_inner(nodes, cursor, depth + 1)?);
                }
                Ok(P::And(children))
            }
            Node::Or { child_count } => {
                let child_count =
                    usize::try_from(*child_count).map_err(|_| Error::LengthMismatch)?;
                if child_count == 0 || child_count > nodes.len().saturating_sub(*cursor) {
                    return Err(Error::LengthMismatch);
                }
                let mut children = Vec::with_capacity(child_count);
                for _ in 0..child_count {
                    children.push(inflate_inner(nodes, cursor, depth + 1)?);
                }
                Ok(P::Or(children))
            }
            Node::Not => {
                let child = inflate_inner(nodes, cursor, depth + 1)?;
                Ok(P::Not(Box::new(child)))
            }
            Node::BlockEq(hash) => Ok(P::BlockEq(*hash)),
            Node::BlockNe(hash) => Ok(P::BlockNe(*hash)),
            Node::BlockIn(values) => Ok(P::BlockIn(values.0.clone())),
            Node::BlockNin(values) => Ok(P::BlockNin(values.0.clone())),
            Node::BlockExists(flag) => Ok(P::BlockExists(*flag)),
            Node::AuthorityEq(id) => Ok(P::AuthorityEq(id.clone())),
            Node::AuthorityNe(id) => Ok(P::AuthorityNe(id.clone())),
            Node::AuthorityIn(ids) => Ok(P::AuthorityIn(ids.0.clone())),
            Node::AuthorityNin(ids) => Ok(P::AuthorityNin(ids.0.clone())),
            Node::AuthorityExists(flag) => Ok(P::AuthorityExists(*flag)),
            Node::TsEq(v) => Ok(P::TsEq(*v)),
            Node::TsLt(v) => Ok(P::TsLt(*v)),
            Node::TsLte(v) => Ok(P::TsLte(*v)),
            Node::TsGt(v) => Ok(P::TsGt(*v)),
            Node::TsGte(v) => Ok(P::TsGte(*v)),
            Node::TsIn(values) => Ok(P::TsIn(values.0.clone())),
            Node::TsNin(values) => Ok(P::TsNin(values.0.clone())),
            Node::TsExists(flag) => Ok(P::TsExists(*flag)),
            Node::EntryEq(hash) => Ok(P::EntryEq(*hash)),
            Node::EntryNe(hash) => Ok(P::EntryNe(*hash)),
            Node::EntryIn(values) => Ok(P::EntryIn(values.0.clone())),
            Node::EntryNin(values) => Ok(P::EntryNin(values.0.clone())),
            Node::EntryExists(flag) => Ok(P::EntryExists(*flag)),
            Node::ResultEq(flag) => Ok(P::ResultEq(*flag)),
            Node::ResultNe(flag) => Ok(P::ResultNe(*flag)),
            Node::ResultIn(values) => Ok(P::ResultIn(values.0.clone())),
            Node::ResultNin(values) => Ok(P::ResultNin(values.0.clone())),
            Node::ResultExists(flag) => Ok(P::ResultExists(*flag)),
            Node::MetadataEq(key, value) => Ok(P::MetadataEq {
                key: key.clone(),
                value: value.clone(),
            }),
            Node::MetadataNe(key, value) => Ok(P::MetadataNe {
                key: key.clone(),
                value: value.clone(),
            }),
            Node::MetadataIn(key, values) => Ok(P::MetadataIn {
                key: key.clone(),
                values: values.0.clone(),
            }),
            Node::MetadataNin(key, values) => Ok(P::MetadataNin {
                key: key.clone(),
                values: values.0.clone(),
            }),
            Node::MetadataExists(key, flag) => Ok(P::MetadataExists {
                key: key.clone(),
                exists: *flag,
            }),
            Node::MetadataIsNull(key, flag) => Ok(P::MetadataIsNull {
                key: key.clone(),
                is_null: *flag,
            }),
            Node::Const(flag) => Ok(P::Const(*flag)),
        }
    }

    pub(super) fn decode_nodes(
        archived: &norito::core::Archived<Vec<Node>>,
    ) -> Result<Vec<Node>, Error> {
        ensure_sequence_len_at_most(archived, MAX_COMMITTED_TX_PREDICATE_NODES, "nodes")?;
        let bytes = archived_sequence_bytes(archived)?;
        let _budget = MembershipDecodeBudgetGuard::enter();
        let (nodes, _) = norito::core::decode_vec_from_slice_serial::<Node>(bytes)?;
        Ok(nodes)
    }
}

impl norito::core::NoritoSerialize for CommittedTxPredicate {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let nodes = wire::flatten(self)?;
        norito::core::NoritoSerialize::serialize(&nodes, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        None
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for CommittedTxPredicate {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("CommittedTxPredicate deserialization must succeed")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let nodes_arch = archived.cast::<Vec<wire::Node>>();
        let nodes = wire::decode_nodes(nodes_arch)?;
        wire::inflate(&nodes)
    }
}

impl TypeId for CommittedTxPredicate {
    fn id() -> iroha_schema::Ident {
        std::any::type_name::<Self>().to_owned()
    }
}

impl IntoSchema for CommittedTxPredicate {
    fn type_name() -> iroha_schema::Ident {
        "CommittedTxPredicate".to_owned()
    }

    fn update_schema_map(map: &mut MetaMap) {
        map.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta { types: vec![] }));
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use std::str::FromStr;

    use hex;
    use iroha_crypto::{Algorithm, Hash, HashOf, MerkleProof};

    use super::*;
    use crate::{
        AssetDefinitionId,
        domain::DomainId,
        kaigi::{
            KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
            KaigiRoomPolicy,
        },
        metadata::Metadata,
        name::Name,
        transaction::{
            PrivateCreateKaigi, PrivateKaigiAction, PrivateKaigiArtifacts, PrivateKaigiFeeSpend,
            PrivateKaigiTemplate, PrivateKaigiTransaction, TransactionEntrypoint,
            TransactionResult,
        },
    };

    fn sample_account(seed: u8) -> crate::account::AccountId {
        let _domain: crate::domain::DomainId =
            DomainId::try_new("wonderland", "universal").unwrap();
        let (public_key, _) =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed derives Ed25519 keypair")
                .into_parts();
        crate::account::AccountId::new(public_key)
    }

    fn sample_hash_literal(seed: u8) -> String {
        let mut bytes = [seed; Hash::LENGTH];
        bytes[Hash::LENGTH - 1] |= 1;
        hex::encode(bytes)
    }

    fn zero_hash<T>() -> HashOf<T> {
        let zero = [0u8; 32];
        HashOf::from_untyped_unchecked(Hash::prehashed(zero))
    }

    fn frame_predicate_nodes(nodes: &[wire::Node]) -> Vec<u8> {
        let (payload, flags) = norito::codec::encode_with_header_flags(&nodes.to_vec());
        norito::core::frame_bare_with_header_flags::<CommittedTxPredicate>(&payload, flags)
            .expect("frame raw predicate node payload")
    }

    fn sample_private_committed_tx() -> CommittedTransaction {
        let mut metadata = Metadata::default();
        metadata.insert(Name::from_str("topic").expect("metadata key"), "private");
        let entrypoint = TransactionEntrypoint::PrivateKaigi(PrivateKaigiTransaction {
            chain: "test-chain".parse().expect("chain"),
            creation_time_ms: 77,
            nonce: None,
            metadata,
            action: PrivateKaigiAction::Create(PrivateCreateKaigi {
                call: PrivateKaigiTemplate {
                    id: KaigiId::new(
                        DomainId::try_new("kaigi", "universal").expect("domain"),
                        Name::from_str("room").expect("call"),
                    ),
                    title: None,
                    description: None,
                    max_participants: Some(2),
                    gas_rate_per_minute: 1,
                    metadata: Metadata::default(),
                    scheduled_start_ms: None,
                    privacy_mode: KaigiPrivacyMode::ZkRosterV1,
                    room_policy: KaigiRoomPolicy::Authenticated,
                    relay_manifest: None,
                },
            }),
            artifacts: PrivateKaigiArtifacts {
                commitment: KaigiParticipantCommitment {
                    commitment: Hash::new(b"commitment"),
                    alias_tag: None,
                },
                nullifier: KaigiParticipantNullifier {
                    digest: Hash::new(b"nullifier"),
                    issued_at_ms: 77,
                },
                roster_root: Hash::new(b"root"),
                proof: vec![1, 2, 3],
            },
            fee_spend: PrivateKaigiFeeSpend {
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").expect("domain"),
                    Name::from_str("xor").expect("name"),
                ),
                anchor_root: Hash::new(b"anchor"),
                nullifiers: vec![[0xAA; 32]],
                output_commitments: vec![[0xBB; 32]],
                encrypted_change_payloads: vec![vec![0xCC]],
                proof: vec![0xDD],
            },
        });
        let result = TransactionResult::new(Ok(crate::trigger::DataTriggerSequence::default()));
        CommittedTransaction {
            block_hash: zero_hash(),
            entrypoint_hash: entrypoint.hash(),
            entrypoint_proof: MerkleProof::from_audit_path(0, vec![]),
            entrypoint: entrypoint.clone(),
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, vec![]),
            result,
            merge_inclusion: None,
        }
    }

    #[test]
    fn predicate_matches_private_kaigi_null_authority_and_metadata() {
        let tx = sample_private_committed_tx();

        assert!(CommittedTxPredicate::AuthorityExists(false).applies(&tx));
        assert!(CommittedTxPredicate::TsEq(77).applies(&tx));
        assert!(
            CommittedTxPredicate::MetadataEq {
                key: Name::from_str("topic").expect("metadata key"),
                value: iroha_primitives::json::Json::new("private"),
            }
            .applies(&tx)
        );
        assert!(!CommittedTxPredicate::AuthorityExists(true).applies(&tx));
    }

    #[test]
    fn predicate_applies_extended_atoms_and_boolean_forms() {
        let tx = sample_private_committed_tx();
        let topic = Name::from_str("topic").expect("metadata key");
        let private = iroha_primitives::json::Json::new("private");
        let public = iroha_primitives::json::Json::new("public");
        let other_block =
            HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::new(b"other block"));
        let other_entry = zero_hash::<crate::transaction::signed::TransactionEntrypoint>();

        assert!(CommittedTxPredicate::BlockNe(other_block).applies(&tx));
        assert!(CommittedTxPredicate::BlockNin(vec![other_block]).applies(&tx));
        assert!(!CommittedTxPredicate::BlockNin(vec![tx.block_hash]).applies(&tx));

        assert!(CommittedTxPredicate::TsGt(50).applies(&tx));
        assert!(!CommittedTxPredicate::TsGt(77).applies(&tx));
        assert!(CommittedTxPredicate::TsNin(vec![1, 2, 3]).applies(&tx));
        assert!(!CommittedTxPredicate::TsNin(vec![77]).applies(&tx));

        assert!(CommittedTxPredicate::EntryNe(other_entry).applies(&tx));
        assert!(CommittedTxPredicate::EntryNin(vec![other_entry]).applies(&tx));
        assert!(!CommittedTxPredicate::EntryNin(vec![tx.entrypoint_hash]).applies(&tx));

        assert!(
            CommittedTxPredicate::MetadataIn {
                key: topic.clone(),
                values: vec![public.clone(), private.clone()],
            }
            .applies(&tx)
        );
        assert!(
            !CommittedTxPredicate::MetadataIn {
                key: topic.clone(),
                values: vec![public.clone()],
            }
            .applies(&tx)
        );
        assert!(
            CommittedTxPredicate::MetadataNin {
                key: topic.clone(),
                values: vec![public.clone()],
            }
            .applies(&tx)
        );
        assert!(
            !CommittedTxPredicate::MetadataNin {
                key: topic.clone(),
                values: vec![private.clone()],
            }
            .applies(&tx)
        );
        assert!(
            CommittedTxPredicate::MetadataIsNull {
                key: topic.clone(),
                is_null: false,
            }
            .applies(&tx)
        );

        assert!(
            CommittedTxPredicate::Not(Box::new(CommittedTxPredicate::ResultEq(false))).applies(&tx)
        );
        assert!(CommittedTxPredicate::Const(true).applies(&tx));
        assert!(!CommittedTxPredicate::Const(false).applies(&tx));
    }

    #[test]
    fn predicate_norito_roundtrip_preserves_complex_boolean_tree() {
        let topic = Name::from_str("topic").expect("metadata key");
        let predicate = CommittedTxPredicate::Or(vec![
            CommittedTxPredicate::Not(Box::new(CommittedTxPredicate::ResultEq(false))),
            CommittedTxPredicate::BlockNe(zero_hash::<crate::block::BlockHeader>()),
            CommittedTxPredicate::TsGt(50),
            CommittedTxPredicate::EntryNe(zero_hash::<
                crate::transaction::signed::TransactionEntrypoint,
            >()),
            CommittedTxPredicate::MetadataIn {
                key: topic,
                values: vec![iroha_primitives::json::Json::new("private")],
            },
            CommittedTxPredicate::Const(false),
        ]);

        let bytes = norito::to_bytes(&predicate).expect("encode predicate");
        let decoded: CommittedTxPredicate =
            norito::decode_from_bytes(&bytes).expect("decode predicate");

        match decoded {
            CommittedTxPredicate::Or(children) => {
                assert_eq!(children.len(), 6);
                assert!(matches!(
                    children.first(),
                    Some(CommittedTxPredicate::Not(inner))
                        if matches!(inner.as_ref(), CommittedTxPredicate::ResultEq(false))
                ));
                assert!(matches!(
                    children.get(1),
                    Some(CommittedTxPredicate::BlockNe(_))
                ));
                assert!(matches!(
                    children.get(2),
                    Some(CommittedTxPredicate::TsGt(50))
                ));
                assert!(matches!(
                    children.get(3),
                    Some(CommittedTxPredicate::EntryNe(_))
                ));
                assert!(matches!(
                    children.get(4),
                    Some(CommittedTxPredicate::MetadataIn { .. })
                ));
                assert!(matches!(
                    children.get(5),
                    Some(CommittedTxPredicate::Const(false))
                ));
            }
            other => panic!(
                "expected Or predicate after roundtrip, got {:?}",
                core::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn predicate_json_roundtrip_covers_every_atom_and_boolean_node() {
        let alice = sample_account(0x31);
        let bob = sample_account(0x32);
        let block_a = sample_hash_literal(0x39)
            .parse::<HashOf<crate::block::BlockHeader>>()
            .expect("block hash A");
        let block_b = sample_hash_literal(0x3A)
            .parse::<HashOf<crate::block::BlockHeader>>()
            .expect("block hash B");
        let entry_a = sample_hash_literal(0x41)
            .parse::<HashOf<crate::transaction::signed::TransactionEntrypoint>>()
            .expect("entry hash A");
        let entry_b = sample_hash_literal(0x42)
            .parse::<HashOf<crate::transaction::signed::TransactionEntrypoint>>()
            .expect("entry hash B");
        let key = Name::from_str("topic").expect("metadata key");
        let scalar = Json::new("private");
        let structured = Json::new(norito::json!({"nested": [1, true, null]}));

        let predicates = vec![
            CommittedTxPredicate::And(vec![
                CommittedTxPredicate::ResultEq(true),
                CommittedTxPredicate::TsGte(10),
            ]),
            CommittedTxPredicate::Or(vec![
                CommittedTxPredicate::ResultEq(false),
                CommittedTxPredicate::TsLt(10),
            ]),
            CommittedTxPredicate::Not(Box::new(CommittedTxPredicate::Const(false))),
            CommittedTxPredicate::BlockEq(block_a),
            CommittedTxPredicate::BlockNe(block_b),
            CommittedTxPredicate::BlockIn(vec![block_a, block_b]),
            CommittedTxPredicate::BlockNin(vec![block_b]),
            CommittedTxPredicate::BlockExists(true),
            CommittedTxPredicate::AuthorityEq(alice.clone()),
            CommittedTxPredicate::AuthorityNe(bob.clone()),
            CommittedTxPredicate::AuthorityIn(vec![alice.clone(), bob.clone()]),
            CommittedTxPredicate::AuthorityNin(vec![bob]),
            CommittedTxPredicate::AuthorityExists(true),
            CommittedTxPredicate::TsEq(1),
            CommittedTxPredicate::TsLt(2),
            CommittedTxPredicate::TsLte(3),
            CommittedTxPredicate::TsGt(4),
            CommittedTxPredicate::TsGte(5),
            CommittedTxPredicate::TsIn(vec![6, 7]),
            CommittedTxPredicate::TsNin(vec![8, 9]),
            CommittedTxPredicate::TsExists(false),
            CommittedTxPredicate::EntryEq(entry_a),
            CommittedTxPredicate::EntryNe(entry_b),
            CommittedTxPredicate::EntryIn(vec![entry_a, entry_b]),
            CommittedTxPredicate::EntryNin(vec![entry_b]),
            CommittedTxPredicate::EntryExists(false),
            CommittedTxPredicate::ResultEq(true),
            CommittedTxPredicate::ResultNe(false),
            CommittedTxPredicate::ResultIn(vec![true, false]),
            CommittedTxPredicate::ResultNin(vec![false]),
            CommittedTxPredicate::ResultExists(true),
            CommittedTxPredicate::MetadataEq {
                key: key.clone(),
                value: structured.clone(),
            },
            CommittedTxPredicate::MetadataNe {
                key: key.clone(),
                value: scalar.clone(),
            },
            CommittedTxPredicate::MetadataIn {
                key: key.clone(),
                values: vec![scalar.clone(), structured.clone()],
            },
            CommittedTxPredicate::MetadataNin {
                key: key.clone(),
                values: vec![structured],
            },
            CommittedTxPredicate::MetadataExists {
                key: key.clone(),
                exists: false,
            },
            CommittedTxPredicate::MetadataIsNull { key, is_null: true },
            CommittedTxPredicate::Const(true),
            CommittedTxPredicate::Const(false),
        ];

        for predicate in predicates {
            let raw = norito::json::to_json(&predicate).expect("encode predicate JSON");
            let decoded: CommittedTxPredicate =
                norito::json::from_json(&raw).expect("decode predicate JSON");
            assert_eq!(decoded, predicate, "roundtrip failed for {raw}");
            assert_eq!(
                committed_tx_predicate_from_canonical_json(&raw).expect("canonical predicate JSON"),
                predicate
            );
        }
    }

    #[test]
    fn predicate_json_rejects_closed_schema_arity_and_type_confusion() {
        let invalid = [
            norito::json!(null),
            norito::json!([]),
            norito::json!({}),
            norito::json!({"op": "const"}),
            norito::json!({"args": [true]}),
            norito::json!({"op": 1, "args": [true]}),
            norito::json!({"op": "unknown", "args": []}),
            norito::json!({"op": "const", "args": []}),
            norito::json!({"op": "const", "args": [true, false]}),
            norito::json!({"op": "const", "args": [1]}),
            norito::json!({"op": "and", "args": []}),
            norito::json!({"op": "or", "args": []}),
            norito::json!({"op": "not", "args": []}),
            norito::json!({"op": "not", "args": [{"op": "const", "args": [true]}, {"op": "const", "args": [false]}]}),
            norito::json!({"op": "eq", "args": ["result_ok"]}),
            norito::json!({"op": "eq", "args": ["result_ok", true, false]}),
            norito::json!({"op": "eq", "args": [{"FieldPath": "result_ok"}, true]}),
            norito::json!({"op": "eq", "args": ["unknown", true]}),
            norito::json!({"op": "eq", "args": ["result_ok", "true"]}),
            norito::json!({"op": "ne", "args": ["timestamp_ms", 1]}),
            norito::json!({"op": "gt", "args": ["authority", 1]}),
            norito::json!({"op": "is_null", "args": ["authority", true]}),
            norito::json!({"op": "exists", "args": ["metadata.", true]}),
            norito::json!({"op": "eq", "args": ["result_ok", true], "extra": false}),
            norito::json!({"equals": [{"field": "result_ok", "value": true}]}),
        ];

        for value in invalid {
            assert!(
                committed_tx_predicate_from_value(&value).is_err(),
                "malformed predicate was accepted: {value:?}"
            );
        }
    }

    #[test]
    fn predicate_json_rejects_empty_duplicate_and_oversized_membership() {
        for op in ["in", "nin"] {
            let empty = binary_predicate_expr(op, "timestamp_ms", Value::Array(Vec::new()));
            assert!(matches!(
                committed_tx_predicate_from_value(&empty),
                Err(CommittedTxPredicateJsonError::EmptyMembership(field))
                    if field == "timestamp_ms"
            ));

            let duplicate = binary_predicate_expr(
                op,
                "timestamp_ms",
                Value::Array(vec![1_u64.into(), 1_u64.into()]),
            );
            assert!(matches!(
                committed_tx_predicate_from_value(&duplicate),
                Err(CommittedTxPredicateJsonError::DuplicateMembership(field))
                    if field == "timestamp_ms"
            ));
        }

        let oversized = binary_predicate_expr(
            "in",
            "timestamp_ms",
            Value::Array(
                (0..=MAX_COMMITTED_TX_MEMBERSHIP_VALUES)
                    .map(|value| Value::from(value as u64))
                    .collect(),
            ),
        );
        assert!(matches!(
            committed_tx_predicate_from_value(&oversized),
            Err(CommittedTxPredicateJsonError::TooManyMembershipValues { .. })
        ));

        let aggregate = predicate_expr(
            "and",
            (0..5)
                .map(|set| {
                    binary_predicate_expr(
                        "in",
                        "timestamp_ms",
                        Value::Array(
                            (0..MAX_COMMITTED_TX_MEMBERSHIP_VALUES)
                                .map(|value| Value::from((set * 10_000 + value) as u64))
                                .collect(),
                        ),
                    )
                })
                .collect(),
        );
        assert!(matches!(
            committed_tx_predicate_from_value(&aggregate),
            Err(CommittedTxPredicateJsonError::TooManyTotalMembershipValues(
                _
            ))
        ));
    }

    #[test]
    fn predicate_json_rejects_excessive_depth_nodes_and_noncanonical_literals() {
        let mut too_deep = predicate_expr("const", vec![Value::Bool(true)]);
        for _ in 0..MAX_COMMITTED_TX_PREDICATE_DEPTH {
            too_deep = predicate_expr("not", vec![too_deep]);
        }
        assert!(matches!(
            committed_tx_predicate_from_value(&too_deep),
            Err(CommittedTxPredicateJsonError::TooDeep(_))
        ));

        let too_many_nodes = predicate_expr(
            "and",
            (0..MAX_COMMITTED_TX_PREDICATE_NODES)
                .map(|_| predicate_expr("const", vec![Value::Bool(true)]))
                .collect(),
        );
        assert!(matches!(
            committed_tx_predicate_from_value(&too_many_nodes),
            Err(CommittedTxPredicateJsonError::TooManyNodes(_))
        ));

        let uppercase_hash = sample_hash_literal(0xab).to_ascii_uppercase();
        let noncanonical_hash =
            binary_predicate_expr("eq", "entrypoint_hash", Value::String(uppercase_hash));
        assert!(matches!(
            committed_tx_predicate_from_value(&noncanonical_hash),
            Err(CommittedTxPredicateJsonError::NonCanonicalLiteral {
                kind: "entrypoint hash",
                ..
            })
        ));

        let padded_account = format!(" {} ", sample_account(0x52));
        let noncanonical_account =
            binary_predicate_expr("eq", "authority", Value::String(padded_account));
        assert!(matches!(
            committed_tx_predicate_from_value(&noncanonical_account),
            Err(CommittedTxPredicateJsonError::NonCanonicalLiteral {
                kind: "account ID",
                ..
            })
        ));
    }

    #[test]
    fn predicate_json_wire_requires_unique_keys_and_exact_canonical_text() {
        let duplicate_key = r#"{"op":"const","op":"const","args":[true]}"#;
        assert!(norito::json::from_json::<CommittedTxPredicate>(duplicate_key).is_err());

        let predicate = CommittedTxPredicate::Not(Box::new(CommittedTxPredicate::ResultEq(false)));
        let canonical = norito::json::to_json(&predicate).expect("canonical JSON");
        assert_eq!(
            committed_tx_predicate_from_canonical_json(&canonical).expect("canonical wire JSON"),
            predicate
        );
        let replay = format!(" {canonical}");
        assert!(matches!(
            committed_tx_predicate_from_canonical_json(&replay),
            Err(CommittedTxPredicateJsonError::NonCanonicalWireEncoding)
        ));
    }

    #[test]
    fn invalid_internal_predicates_serialize_and_fail_closed() {
        let mut too_deep = CommittedTxPredicate::Const(true);
        for _ in 0..MAX_COMMITTED_TX_PREDICATE_DEPTH {
            too_deep = CommittedTxPredicate::Not(Box::new(too_deep));
        }
        let invalid = vec![
            CommittedTxPredicate::And(Vec::new()),
            CommittedTxPredicate::Or(Vec::new()),
            CommittedTxPredicate::TsIn(Vec::new()),
            CommittedTxPredicate::TsNin(vec![1, 1]),
            CommittedTxPredicate::TsIn((0..=MAX_COMMITTED_TX_MEMBERSHIP_VALUES as u64).collect()),
            CommittedTxPredicate::MetadataEq {
                key: "topic".parse().expect("metadata key"),
                value: Json::from_raw_json("{ \"value\": true }".into())
                    .expect("valid noncanonical JSON fixture"),
            },
            too_deep,
        ];
        let fail_closed =
            norito::json::to_json(&CommittedTxPredicate::Const(false)).expect("const false JSON");
        let tx = sample_private_committed_tx();

        for predicate in invalid {
            assert!(validate_committed_tx_predicate(&predicate).is_err());
            assert!(!predicate.applies(&tx));
            assert_eq!(
                norito::json::to_json(&predicate).expect("fail-closed JSON"),
                fail_closed
            );
            assert!(norito::to_bytes(&predicate).is_err());
        }
    }

    #[test]
    fn flat_filter_index_view_roundtrips_losslessly_and_rejects_richer_trees() {
        let account_a = sample_account(0x61);
        let account_b = sample_account(0x62);
        let entry_a = sample_hash_literal(0x71)
            .parse::<HashOf<crate::transaction::signed::TransactionEntrypoint>>()
            .expect("entry hash A");
        let entry_b = sample_hash_literal(0x72)
            .parse::<HashOf<crate::transaction::signed::TransactionEntrypoint>>()
            .expect("entry hash B");
        let block_a = sample_hash_literal(0x73)
            .parse::<HashOf<crate::block::BlockHeader>>()
            .expect("block hash A");
        let block_b = sample_hash_literal(0x74)
            .parse::<HashOf<crate::block::BlockHeader>>()
            .expect("block hash B");
        let filters = CommittedTxFilters {
            block_eq: Some(block_a),
            block_ne: Some(block_b),
            block_in: vec![block_a],
            block_nin: vec![block_b],
            block_exists: Some(true),
            authority_eq: Some(account_a.clone()),
            authority_ne: Some(account_b.clone()),
            authority_in: vec![account_a],
            authority_nin: vec![account_b],
            authority_exists: Some(true),
            ts_ge: Some(10),
            ts_le: Some(20),
            entry_eq: Some(entry_a),
            entry_in: vec![entry_a],
            entry_ne: Some(entry_b),
            entry_nin: vec![entry_b],
            entry_exists: Some(true),
            result_ok: Some(true),
            result_ok_ne: Some(false),
            result_ok_in: vec![true],
            result_ok_nin: vec![false],
            result_exists: Some(true),
        };
        let tree = committed_tx_predicate_from_filters(&filters);
        assert_eq!(committed_tx_filters_from_predicate(&tree), Some(filters));

        assert!(
            committed_tx_filters_from_predicate(&CommittedTxPredicate::Or(vec![
                CommittedTxPredicate::ResultEq(true),
                CommittedTxPredicate::TsGte(10),
            ]))
            .is_none()
        );
        assert!(
            committed_tx_filters_from_predicate(&CommittedTxPredicate::MetadataEq {
                key: "topic".parse().expect("metadata key"),
                value: Json::new("private"),
            })
            .is_none()
        );
        assert!(
            committed_tx_filters_from_predicate(&CommittedTxPredicate::ResultIn(vec![true, true]))
                .is_none()
        );
    }

    #[test]
    fn predicate_wire_inflate_rejects_missing_and_trailing_nodes() {
        assert!(wire::inflate(&[wire::Node::And { child_count: 1 }]).is_err());
        assert!(wire::inflate(&[wire::Node::Not]).is_err());
        assert!(wire::inflate(&[wire::Node::Const(true), wire::Node::Const(false)]).is_err());
    }

    #[test]
    fn predicate_wire_inflate_enforces_all_resource_and_set_invariants() {
        let hostile_arity = vec![wire::Node::And {
            child_count: u32::MAX,
        }];
        assert!(wire::inflate(&hostile_arity).is_err());
        let hostile_bytes = frame_predicate_nodes(&hostile_arity);
        assert!(norito::decode_from_bytes::<CommittedTxPredicate>(&hostile_bytes).is_err());
        assert!(wire::inflate(&[wire::Node::And { child_count: 0 }]).is_err());
        assert!(wire::inflate(&[wire::Node::TsIn(Vec::new().into())]).is_err());
        let duplicate = vec![wire::Node::TsNin(vec![7, 7].into())];
        assert!(wire::inflate(&duplicate).is_err());
        let duplicate_bytes = frame_predicate_nodes(&duplicate);
        assert!(norito::decode_from_bytes::<CommittedTxPredicate>(&duplicate_bytes).is_err());
        assert!(
            wire::inflate(&[wire::Node::TsIn(
                (0..=MAX_COMMITTED_TX_MEMBERSHIP_VALUES as u64)
                    .collect::<Vec<_>>()
                    .into(),
            )])
            .is_err()
        );
        let mut aggregate_membership = vec![wire::Node::And { child_count: 5 }];
        aggregate_membership.extend((0..5).map(|set| {
            wire::Node::TsIn(
                (0..MAX_COMMITTED_TX_MEMBERSHIP_VALUES)
                    .map(|value| (set * 10_000 + value) as u64)
                    .collect::<Vec<_>>()
                    .into(),
            )
        }));
        assert!(wire::inflate(&aggregate_membership).is_err());

        let too_many_wire = vec![wire::Node::Const(true); MAX_COMMITTED_TX_PREDICATE_NODES + 1];
        let too_many_wire_bytes = frame_predicate_nodes(&too_many_wire);
        assert!(norito::decode_from_bytes::<CommittedTxPredicate>(&too_many_wire_bytes).is_err());

        let oversized_membership_wire = vec![wire::Node::TsIn(
            (0..=MAX_COMMITTED_TX_MEMBERSHIP_VALUES as u64)
                .collect::<Vec<_>>()
                .into(),
        )];
        let oversized_membership_bytes = frame_predicate_nodes(&oversized_membership_wire);
        assert!(
            norito::decode_from_bytes::<CommittedTxPredicate>(&oversized_membership_bytes).is_err()
        );

        let aggregate_membership_bytes = frame_predicate_nodes(&aggregate_membership);
        assert!(
            norito::decode_from_bytes::<CommittedTxPredicate>(&aggregate_membership_bytes).is_err()
        );

        let mut deep = vec![wire::Node::Not; MAX_COMMITTED_TX_PREDICATE_DEPTH];
        deep.push(wire::Node::Const(true));
        assert!(wire::inflate(&deep).is_err());
        assert!(
            norito::decode_from_bytes::<CommittedTxPredicate>(&frame_predicate_nodes(&deep))
                .is_err()
        );

        let too_many = vec![wire::Node::Const(true); MAX_COMMITTED_TX_PREDICATE_NODES + 1];
        assert!(wire::inflate(&too_many).is_err());
    }
}
