//! Shared helpers for committed transaction predicates.

#![allow(clippy::missing_errors_doc)]

use iroha_crypto::HashOf;
use iroha_primitives::json::Json;
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
#[cfg(feature = "json")]
use norito::json::Value;

use crate::{
    name::Name,
    query::{CommittedTransaction, CommittedTxFilters},
};

/// Predicate tree over committed transactions.
#[derive(Clone)]
pub enum CommittedTxPredicate {
    /// Logical conjunction of sub-predicates.
    And(Vec<CommittedTxPredicate>),
    /// Logical disjunction of sub-predicates.
    Or(Vec<CommittedTxPredicate>),
    /// Logical negation of a sub-predicate.
    Not(Box<CommittedTxPredicate>),
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

impl CommittedTxPredicate {
    fn authority_of(tx: &CommittedTransaction) -> Option<crate::account::AccountId> {
        match &tx.entrypoint {
            crate::transaction::signed::TransactionEntrypoint::External(signed) => {
                Some(signed.authority().clone())
            }
            _ => None,
        }
    }

    fn timestamp_ms_of(tx: &CommittedTransaction) -> Option<u64> {
        match &tx.entrypoint {
            crate::transaction::signed::TransactionEntrypoint::External(s) => {
                u64::try_from(s.creation_time().as_millis()).ok()
            }
            crate::transaction::signed::TransactionEntrypoint::Time(_) => None,
        }
    }

    fn metadata_value<'tx>(tx: &'tx CommittedTransaction, key: &Name) -> Option<&'tx Json> {
        match &tx.entrypoint {
            crate::transaction::signed::TransactionEntrypoint::External(signed) => {
                signed.metadata().get(key)
            }
            _ => None,
        }
    }

    fn metadata_json_value(json: &Json) -> Option<norito::json::Value> {
        json.try_into_any_norito::<norito::json::Value>().ok()
    }

    /// Evaluate the predicate against the provided committed transaction.
    #[must_use]
    pub fn applies(&self, tx: &CommittedTransaction) -> bool {
        use CommittedTxPredicate as P;
        match self {
            P::Const(v) => *v,
            P::And(list) => list.iter().all(|p| p.applies(tx)),
            P::Or(list) => list.iter().any(|p| p.applies(tx)),
            P::Not(inner) => !inner.applies(tx),

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
    use iroha_crypto::HashOf;
    use iroha_primitives::json::Json;
    use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
    use norito::{NoritoDeserialize, NoritoSerialize, core::Error};

    use super::CommittedTxPredicate;
    use crate::name::Name;

    #[derive(Clone, NoritoSerialize, NoritoDeserialize)]
    pub(super) enum Node {
        And { child_count: u32 },
        Or { child_count: u32 },
        Not,
        AuthorityEq(crate::account::AccountId),
        AuthorityNe(crate::account::AccountId),
        AuthorityIn(Vec<crate::account::AccountId>),
        AuthorityNin(Vec<crate::account::AccountId>),
        AuthorityExists(bool),
        TsEq(u64),
        TsLt(u64),
        TsLte(u64),
        TsGt(u64),
        TsGte(u64),
        TsIn(Vec<u64>),
        TsNin(Vec<u64>),
        TsExists(bool),
        EntryEq(HashOf<crate::transaction::signed::TransactionEntrypoint>),
        EntryNe(HashOf<crate::transaction::signed::TransactionEntrypoint>),
        EntryIn(Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>>),
        EntryNin(Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>>),
        EntryExists(bool),
        ResultEq(bool),
        ResultNe(bool),
        ResultIn(Vec<bool>),
        ResultNin(Vec<bool>),
        ResultExists(bool),
        MetadataEq(Name, Json),
        MetadataNe(Name, Json),
        MetadataIn(Name, Vec<Json>),
        MetadataNin(Name, Vec<Json>),
        MetadataExists(Name, bool),
        MetadataIsNull(Name, bool),
        Const(bool),
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
        let mut nodes = Vec::new();
        flatten_inner(root, &mut nodes)?;
        Ok(nodes)
    }

    pub(super) fn inflate(nodes: &[Node]) -> Result<CommittedTxPredicate, Error> {
        let mut cursor = 0usize;
        let tree = inflate_inner(nodes, &mut cursor)?;
        if cursor != nodes.len() {
            return Err(Error::Message(
                "CommittedTxPredicate: trailing nodes".into(),
            ));
        }
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
            P::AuthorityEq(id) => out.push(Node::AuthorityEq(id.clone())),
            P::AuthorityNe(id) => out.push(Node::AuthorityNe(id.clone())),
            P::AuthorityIn(ids) => out.push(Node::AuthorityIn(ids.clone())),
            P::AuthorityNin(ids) => out.push(Node::AuthorityNin(ids.clone())),
            P::AuthorityExists(flag) => out.push(Node::AuthorityExists(*flag)),
            P::TsEq(v) => out.push(Node::TsEq(*v)),
            P::TsLt(v) => out.push(Node::TsLt(*v)),
            P::TsLte(v) => out.push(Node::TsLte(*v)),
            P::TsGt(v) => out.push(Node::TsGt(*v)),
            P::TsGte(v) => out.push(Node::TsGte(*v)),
            P::TsIn(values) => out.push(Node::TsIn(values.clone())),
            P::TsNin(values) => out.push(Node::TsNin(values.clone())),
            P::TsExists(flag) => out.push(Node::TsExists(*flag)),
            P::EntryEq(hash) => out.push(Node::EntryEq(*hash)),
            P::EntryNe(hash) => out.push(Node::EntryNe(*hash)),
            P::EntryIn(values) => out.push(Node::EntryIn(values.clone())),
            P::EntryNin(values) => out.push(Node::EntryNin(values.clone())),
            P::EntryExists(flag) => out.push(Node::EntryExists(*flag)),
            P::ResultEq(flag) => out.push(Node::ResultEq(*flag)),
            P::ResultNe(flag) => out.push(Node::ResultNe(*flag)),
            P::ResultIn(values) => out.push(Node::ResultIn(values.clone())),
            P::ResultNin(values) => out.push(Node::ResultNin(values.clone())),
            P::ResultExists(flag) => out.push(Node::ResultExists(*flag)),
            P::MetadataEq { key, value } => out.push(Node::MetadataEq(key.clone(), value.clone())),
            P::MetadataNe { key, value } => out.push(Node::MetadataNe(key.clone(), value.clone())),
            P::MetadataIn { key, values } => {
                out.push(Node::MetadataIn(key.clone(), values.clone()))
            }
            P::MetadataNin { key, values } => {
                out.push(Node::MetadataNin(key.clone(), values.clone()))
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

    fn inflate_inner(nodes: &[Node], cursor: &mut usize) -> Result<CommittedTxPredicate, Error> {
        use CommittedTxPredicate as P;

        let node = nodes.get(*cursor).ok_or(Error::LengthMismatch)?;
        *cursor += 1;
        match node {
            Node::And { child_count } => {
                let mut children = Vec::with_capacity(*child_count as usize);
                for _ in 0..*child_count {
                    children.push(inflate_inner(nodes, cursor)?);
                }
                Ok(P::And(children))
            }
            Node::Or { child_count } => {
                let mut children = Vec::with_capacity(*child_count as usize);
                for _ in 0..*child_count {
                    children.push(inflate_inner(nodes, cursor)?);
                }
                Ok(P::Or(children))
            }
            Node::Not => {
                let child = inflate_inner(nodes, cursor)?;
                Ok(P::Not(Box::new(child)))
            }
            Node::AuthorityEq(id) => Ok(P::AuthorityEq(id.clone())),
            Node::AuthorityNe(id) => Ok(P::AuthorityNe(id.clone())),
            Node::AuthorityIn(ids) => Ok(P::AuthorityIn(ids.clone())),
            Node::AuthorityNin(ids) => Ok(P::AuthorityNin(ids.clone())),
            Node::AuthorityExists(flag) => Ok(P::AuthorityExists(*flag)),
            Node::TsEq(v) => Ok(P::TsEq(*v)),
            Node::TsLt(v) => Ok(P::TsLt(*v)),
            Node::TsLte(v) => Ok(P::TsLte(*v)),
            Node::TsGt(v) => Ok(P::TsGt(*v)),
            Node::TsGte(v) => Ok(P::TsGte(*v)),
            Node::TsIn(values) => Ok(P::TsIn(values.clone())),
            Node::TsNin(values) => Ok(P::TsNin(values.clone())),
            Node::TsExists(flag) => Ok(P::TsExists(*flag)),
            Node::EntryEq(hash) => Ok(P::EntryEq(*hash)),
            Node::EntryNe(hash) => Ok(P::EntryNe(*hash)),
            Node::EntryIn(values) => Ok(P::EntryIn(values.clone())),
            Node::EntryNin(values) => Ok(P::EntryNin(values.clone())),
            Node::EntryExists(flag) => Ok(P::EntryExists(*flag)),
            Node::ResultEq(flag) => Ok(P::ResultEq(*flag)),
            Node::ResultNe(flag) => Ok(P::ResultNe(*flag)),
            Node::ResultIn(values) => Ok(P::ResultIn(values.clone())),
            Node::ResultNin(values) => Ok(P::ResultNin(values.clone())),
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
                values: values.clone(),
            }),
            Node::MetadataNin(key, values) => Ok(P::MetadataNin {
                key: key.clone(),
                values: values.clone(),
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
        let nodes = <Vec<wire::Node> as norito::core::NoritoDeserialize>::deserialize(nodes_arch);
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

/// Parse a JSON predicate payload into deterministic filters.
#[cfg(feature = "json")]
pub fn committed_tx_filters_from_json(raw: &str) -> Option<CommittedTxFilters> {
    let value = norito::json::from_json::<Value>(raw).ok()?;
    let mut filters = CommittedTxFilters::default();
    apply_committed_tx_filter_expr(&mut filters, &value)?;
    Some(filters)
}

/// Fallback when JSON support is disabled.
#[cfg(not(feature = "json"))]
#[allow(unused_variables)]
pub fn committed_tx_filters_from_json(raw: &str) -> Option<CommittedTxFilters> {
    None
}

#[cfg(feature = "json")]
#[allow(clippy::too_many_lines)]
fn apply_committed_tx_filter_expr(filters: &mut CommittedTxFilters, expr: &Value) -> Option<()> {
    let obj = expr.as_object()?;
    let op = obj.get("op")?.as_str()?;
    let args = obj.get("args")?;
    match op {
        "and" => {
            let arr = args.as_array()?;
            for item in arr {
                apply_committed_tx_filter_expr(filters, item)?;
            }
            Some(())
        }
        "eq" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Eq, field, value)
        }
        "ne" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Ne, field, value)
        }
        "in" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::In, field, value)
        }
        "nin" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Nin, field, value)
        }
        "exists" => {
            let (field, flag) = parse_field_and_optional_bool(args, true)?;
            set_exists_flag(filters, field, flag)
        }
        "is_null" => {
            let (field, flag) = parse_field_and_optional_bool(args, false)?;
            set_exists_flag(filters, field, flag)
        }
        "gt" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Gt, field, value)
        }
        "gte" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Gte, field, value)
        }
        "lt" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Lt, field, value)
        }
        "lte" => {
            let (field, value) = parse_field_and_value(args)?;
            apply_relational_filter(filters, RelationalOp::Lte, field, value)
        }
        _ => None,
    }
}

#[cfg(feature = "json")]
fn parse_field_and_value(args: &Value) -> Option<(FilterField, &Value)> {
    let arr = args.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let field = parse_field(&arr[0])?;
    Some((field, &arr[1]))
}

#[cfg(feature = "json")]
fn parse_field_and_optional_bool(args: &Value, default: bool) -> Option<(FilterField, bool)> {
    if let Some(arr) = args.as_array() {
        if arr.is_empty() {
            return None;
        }
        let field = parse_field(&arr[0])?;
        let flag = if arr.len() > 1 {
            arr[1].as_bool()?
        } else {
            default
        };
        return Some((field, flag));
    }
    let field = parse_field(args)?;
    Some((field, default))
}

#[cfg(feature = "json")]
fn parse_field(value: &Value) -> Option<FilterField> {
    match value {
        Value::Object(map) => map
            .get("FieldPath")
            .and_then(Value::as_str)
            .and_then(FilterField::from_str),
        Value::String(s) => FilterField::from_str(s),
        _ => None,
    }
}

#[cfg(feature = "json")]
fn set_exists_flag(filters: &mut CommittedTxFilters, field: FilterField, flag: bool) -> Option<()> {
    match field {
        FilterField::Authority => {
            filters.authority_exists = Some(flag);
            Some(())
        }
        FilterField::EntrypointHash => {
            filters.entry_exists = Some(flag);
            Some(())
        }
        FilterField::ResultOk => {
            filters.result_exists = Some(flag);
            Some(())
        }
        _ => None,
    }
}

#[cfg(feature = "json")]
#[derive(Clone, Copy)]
enum RelationalOp {
    Eq,
    Ne,
    In,
    Nin,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[cfg(feature = "json")]
#[derive(Clone, Copy)]
enum FilterField {
    Authority,
    Timestamp,
    EntrypointHash,
    ResultOk,
}

#[cfg(feature = "json")]
impl FilterField {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "authority" => Some(Self::Authority),
            "timestamp_ms" => Some(Self::Timestamp),
            "entrypoint_hash" => Some(Self::EntrypointHash),
            "result_ok" => Some(Self::ResultOk),
            _ => None,
        }
    }
}

#[cfg(feature = "json")]
fn apply_relational_filter(
    filters: &mut CommittedTxFilters,
    op: RelationalOp,
    field: FilterField,
    value: &Value,
) -> Option<()> {
    match field {
        FilterField::Authority => apply_account_filter(filters, op, value),
        FilterField::EntrypointHash => apply_entrypoint_filter(filters, op, value),
        FilterField::ResultOk => apply_result_filter(filters, op, value),
        FilterField::Timestamp => apply_timestamp_filter(filters, op, value),
    }
}

#[cfg(feature = "json")]
fn apply_account_filter(
    filters: &mut CommittedTxFilters,
    op: RelationalOp,
    value: &Value,
) -> Option<()> {
    fn parse_account_literal(raw: &str) -> Option<crate::account::AccountId> {
        crate::account::AccountId::parse_encoded(raw)
            .ok()
            .map(crate::account::ParsedAccountId::into_account_id)
    }

    use RelationalOp::*;
    match op {
        Eq => {
            filters.authority_eq = Some(parse_account_literal(value.as_str()?)?);
            Some(())
        }
        Ne => {
            filters.authority_ne = Some(parse_account_literal(value.as_str()?)?);
            Some(())
        }
        In => {
            let accounts =
                collect_parsed_slice(value.as_array()?, |v| parse_account_literal(v.as_str()?))?;
            filters.authority_in.extend(accounts);
            Some(())
        }
        Nin => {
            let accounts =
                collect_parsed_slice(value.as_array()?, |v| parse_account_literal(v.as_str()?))?;
            filters.authority_nin.extend(accounts);
            Some(())
        }
        _ => None,
    }
}

#[cfg(feature = "json")]
fn apply_entrypoint_filter(
    filters: &mut CommittedTxFilters,
    op: RelationalOp,
    value: &Value,
) -> Option<()> {
    use RelationalOp::*;
    match op {
        Eq => {
            filters.entry_eq = Some(value.as_str()?.parse().ok()?);
            Some(())
        }
        Ne => {
            filters.entry_ne = Some(value.as_str()?.parse().ok()?);
            Some(())
        }
        In => {
            let hashes: Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>> =
                collect_parsed_slice(value.as_array()?, |v| v.as_str()?.parse().ok())?;
            filters.entry_in.extend(hashes);
            Some(())
        }
        Nin => {
            let hashes: Vec<HashOf<crate::transaction::signed::TransactionEntrypoint>> =
                collect_parsed_slice(value.as_array()?, |v| v.as_str()?.parse().ok())?;
            filters.entry_nin.extend(hashes);
            Some(())
        }
        _ => None,
    }
}

#[cfg(feature = "json")]
fn apply_result_filter(
    filters: &mut CommittedTxFilters,
    op: RelationalOp,
    value: &Value,
) -> Option<()> {
    use RelationalOp::*;
    match op {
        Eq => {
            filters.result_ok = Some(value.as_bool()?);
            Some(())
        }
        Ne => {
            filters.result_ok_ne = Some(value.as_bool()?);
            Some(())
        }
        In => {
            let flags = collect_parsed_slice(value.as_array()?, Value::as_bool)?;
            filters.result_ok_in.extend(flags);
            Some(())
        }
        Nin => {
            let flags = collect_parsed_slice(value.as_array()?, Value::as_bool)?;
            filters.result_ok_nin.extend(flags);
            Some(())
        }
        _ => None,
    }
}

#[cfg(feature = "json")]
fn apply_timestamp_filter(
    filters: &mut CommittedTxFilters,
    op: RelationalOp,
    value: &Value,
) -> Option<()> {
    use RelationalOp::*;
    let ts = value.as_u64()?;
    match op {
        Eq => {
            filters.ts_ge = Some(filters.ts_ge.map_or(ts, |old| old.max(ts)));
            filters.ts_le = Some(filters.ts_le.map_or(ts, |old| old.min(ts)));
            Some(())
        }
        Gt => {
            let next = ts.saturating_add(1);
            filters.ts_ge = Some(filters.ts_ge.map_or(next, |old| old.max(next)));
            Some(())
        }
        Gte => {
            filters.ts_ge = Some(filters.ts_ge.map_or(ts, |old| old.max(ts)));
            Some(())
        }
        Lt => {
            let prev = ts.saturating_sub(1);
            filters.ts_le = Some(filters.ts_le.map_or(prev, |old| old.min(prev)));
            Some(())
        }
        Lte => {
            filters.ts_le = Some(filters.ts_le.map_or(ts, |old| old.min(ts)));
            Some(())
        }
        _ => None,
    }
}

#[cfg(feature = "json")]
fn collect_parsed_slice<T, F>(items: &[Value], mut parse: F) -> Option<Vec<T>>
where
    F: FnMut(&Value) -> Option<T>,
{
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(parse(item)?);
    }
    Some(out)
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use hex;
    use iroha_crypto::{Algorithm, Hash};

    use super::*;

    type EntryHash = HashOf<crate::transaction::signed::TransactionEntrypoint>;

    fn sample_account(seed: u8) -> crate::account::AccountId {
        let domain: crate::domain::DomainId = "wonderland".parse().unwrap();
        let (public_key, _) =
            iroha_crypto::KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519).into_parts();
        crate::account::AccountId::new(domain, public_key)
    }

    fn sample_hash_literal(seed: u8) -> String {
        let mut bytes = [seed; Hash::LENGTH];
        bytes[Hash::LENGTH - 1] |= 1;
        hex::encode_upper(bytes)
    }

    fn parse_filters(json: &str) -> CommittedTxFilters {
        committed_tx_filters_from_json(json).expect("filter parsing")
    }

    #[test]
    fn parses_basic_comparisons() {
        let alice_account = sample_account(0x11);
        let bob_account = sample_account(0x22);
        let alice_id = alice_account.to_string();
        let bob_id = bob_account.to_string();
        let entry_eq_literal = sample_hash_literal(0x33);
        let entry_in_literal = sample_hash_literal(0x44);
        let entry_eq_hash: EntryHash = entry_eq_literal.parse().unwrap();
        let entry_in_hash: EntryHash = entry_in_literal.parse().unwrap();
        let json = format!(
            r#"{{"op":"and","args":[
                {{"op":"eq","args":[{{"FieldPath":"authority"}},"{alice_id}"]}},
                {{"op":"ne","args":[{{"FieldPath":"authority"}},"{bob_id}"]}},
                {{"op":"eq","args":[{{"FieldPath":"entrypoint_hash"}},"{entry_eq_literal}"]}},
                {{"op":"in","args":[{{"FieldPath":"entrypoint_hash"}},["{entry_in_literal}"]]}},
                {{"op":"nin","args":[{{"FieldPath":"result_ok"}},[false]]}},
                {{"op":"exists","args":[{{"FieldPath":"result_ok"}}]}},
                {{"op":"is_null","args":[{{"FieldPath":"authority"}},true]}}
            ]}}"#
        );

        let filters = parse_filters(&json);
        assert_eq!(filters.authority_eq.as_ref(), Some(&alice_account));
        assert_eq!(filters.authority_ne.as_ref(), Some(&bob_account));
        assert!(filters.authority_in.is_empty());
        assert!(filters.authority_nin.is_empty());
        assert_eq!(filters.authority_exists, Some(true));
        assert_eq!(filters.entry_eq.as_ref(), Some(&entry_eq_hash));
        assert_eq!(filters.entry_in, vec![entry_in_hash]);
        assert!(filters.entry_nin.is_empty());
        assert_eq!(filters.entry_exists, None);
        assert_eq!(filters.result_ok_nin, vec![false]);
        assert_eq!(filters.result_exists, Some(true));
    }

    #[test]
    fn parses_timestamp_bounds() {
        let json = r#"{"op":"and","args":[
            {"op":"eq","args":[{"FieldPath":"timestamp_ms"},1000]},
            {"op":"gt","args":[{"FieldPath":"timestamp_ms"},2000]},
            {"op":"lte","args":[{"FieldPath":"timestamp_ms"},5000]}
        ]}"#;

        let filters = parse_filters(json);
        assert_eq!(filters.ts_ge, Some(2001));
        assert_eq!(filters.ts_le, Some(1000));
    }

    #[test]
    fn rejects_unknown_field() {
        let json = r#"{
            "op":"eq",
            "args":[{"FieldPath":"unknown"},1]
        }"#;
        assert!(committed_tx_filters_from_json(json).is_none());
    }
}
