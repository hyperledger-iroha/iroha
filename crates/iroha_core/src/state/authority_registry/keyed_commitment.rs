//! Construction-independent contract of the single State-owned keyed commitment.
//!
//! `specs/sumeragi.md` §16 is the normative text; rule numbers `K1`..`K12` below
//! refer to its §16.4. This module fixes only what every candidate construction
//! must agree on:
//!
//! - the **committed object**: a finite map from `(table identity, canonical key
//!   bytes)` to canonical value bytes, over the canonical tables and cells that the
//!   exhaustive authority registry declares ([`StateSchema::from_registry`]);
//! - the **statements** a witness can establish against one 32-byte root:
//!   inclusion, absence and a complete half-open key range of one table;
//! - the **update**: one block's net change set, applied atomically.
//!
//! The governed radix profile selects the production tree in
//! `iroha_data_model::sumeragi_finality::keyed_state`. Native publication reads
//! the original State journals, binds the pre-tail execution version in the
//! result and retains the published version for the next block. The LtHash
//! predecessor continues to interpret the slots at cuts that select that profile.
//!
//! Witnesses are opaque canonical byte strings, so native and in-circuit
//! measurements count the same bytes and a carried witness is validated from
//! transaction bytes alone. Verification is a pure function of the schema, the
//! root, the statement and the witness bytes: it reads no State, cache, clock or
//! node configuration, and its only outcomes are acceptance and deterministic
//! rejection.
//!
//! The G.2 candidates are development-only prototypes in
//! `fastpq_prover::state_commitment_prototypes`; `candidates_tests.rs` puts each
//! behind this interface and runs `conformance::check_contract` on it (unit tests
//! built with the `state-commitment-prototypes` feature). Their measurements are
//! in `specs/state_commitment_candidates.json`. The selected radix implementation
//! runs the same suite in `selected_tests`. The shared wire type is [`KeyedStateRoot`].

pub(crate) mod selected_schema;

use super::{Canonical, CanonicalLayout, Field, Role, Schema};

pub use iroha_data_model::sumeragi_finality::keyed_schema::{
    CELL_KEY, SchemaError, StatementError, TableDescriptor, TableShape,
};
use iroha_data_model::sumeragi_finality::keyed_schema::StateTableDirectoryV1;

/// Byte length of every keyed State root.
pub const ROOT_BYTES: usize = 32;

pub use iroha_data_model::sumeragi_finality::KeyedStateRoot;

/// Construction-independent schema with conformance checks and native registry resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateSchema(StateTableDirectoryV1);

impl std::ops::Deref for StateSchema {
    type Target = StateTableDirectoryV1;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl StateSchema {
    pub(crate) fn new(tables: Vec<TableDescriptor>) -> Result<Self, SchemaError> {
        StateTableDirectoryV1::new(tables).map(Self)
    }

    /// Check a range statement (K6): the table is declared, the interval is nonempty and
    /// the claimed entries are strictly ascending inside it.
    ///
    /// This is the construction-independent part of range verification. It establishes
    /// nothing about the committed State.
    ///
    /// # Errors
    /// See [`StatementError`].
    pub(crate) fn check_range(
        &self,
        table: &str,
        range: KeyRange<'_>,
        entries: &[Entry<'_>],
    ) -> Result<usize, StatementError> {
        let (index, descriptor) = self.table(table).ok_or(StatementError::UnknownTable)?;
        range.validate()?;
        let mut previous: Option<&[u8]> = None;
        for entry in entries {
            if !range.contains(entry.key) {
                return Err(StatementError::EntryOutsideRange);
            }
            if descriptor.shape() == TableShape::Cell && entry.key != CELL_KEY {
                return Err(StatementError::CellKey);
            }
            if previous.is_some_and(|previous| previous >= entry.key) {
                return Err(StatementError::EntryOrder);
            }
            previous = Some(entry.key);
        }
        Ok(index)
    }

    /// The schema of the actual State: every canonical table and cell of the exhaustive
    /// authority registry, nested owners included.
    ///
    /// Derived indexes, authenticated history and node-local fields are not committed
    /// entries; the registry classifies them and the inventory
    /// (`specs/state_table_inventory.json`) records why.
    ///
    /// # Errors
    /// A canonical field still declares a `Required` projection, or two fields share an
    /// identity.
    pub(crate) fn from_registry(fields: &'static [Field]) -> Result<Self, SchemaError> {
        let mut tables = Vec::new();
        collect_registry(fields, &mut tables)?;
        Self::new(tables)
    }
}

fn collect_registry(
    fields: &'static [Field],
    tables: &mut Vec<TableDescriptor>,
) -> Result<(), SchemaError> {
    for field in fields {
        match field.role {
            Role::Canonical(Canonical::Table { key, value }) => tables.push(TableDescriptor::table(
                field.id, codec_identity(field.id, key)?, codec_identity(field.id, value)?
            )),
            Role::Canonical(Canonical::Cell(value)) => tables.push(TableDescriptor::cell(
                field.id, codec_identity(field.id, value)?
            )),
            Role::Canonical(Canonical::Owner(children)) => collect_registry(children, tables)?,
            Role::Derived { .. } | Role::History { .. } | Role::Local(_) => {}
        }
    }
    Ok(())
}

/// The codec identity of a registry schema: codec kind, declared identity and layout.
///
/// `norito:<nominal name>@<major>.<minor>.<flags>` for a declared Norito type and
/// `semantic:<identity>@<major>.<minor>.<flags>` for a semantic projection.
pub fn codec_identity(field: &'static str, schema: Schema) -> Result<String, SchemaError> {
    let (kind, identity, layout): (&str, std::borrow::Cow<'static, str>, CanonicalLayout) =
        match schema {
            Schema::Norito {
                nominal_name,
                layout,
            } => ("norito", nominal_name(), layout),
            Schema::Semantic {
                identity, layout, ..
            } => ("semantic", std::borrow::Cow::Borrowed(identity), layout),
            Schema::Required { .. } => return Err(SchemaError::Unresolved(field)),
        };
    Ok(format!(
        "{kind}:{identity}@{}.{}.{}",
        layout.major, layout.minor, layout.flags
    ))
}

/// One committed entry: canonical key and value bytes inside one table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    /// Canonical key bytes ([`CELL_KEY`] for a cell).
    pub(crate) key: &'a [u8],
    /// Canonical value bytes; an empty value is present, never absent.
    pub(crate) value: &'a [u8],
}

/// One net change of a block: the value after the block, or `None` when the key is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change<'a> {
    /// Registry identity of the table or cell.
    pub(crate) table: &'a str,
    /// Canonical key bytes ([`CELL_KEY`] for a cell).
    pub(crate) key: &'a [u8],
    /// Canonical value bytes after the block, or `None` for an absent key.
    pub(crate) value: Option<&'a [u8]>,
}

/// A half-open interval of canonical key bytes inside one table (K6).
///
/// Keys are compared as byte strings. `lower` is inclusive; `upper` is exclusive and
/// `None` is unbounded, so `KeyRange::FULL` is the complete table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRange<'a> {
    /// Inclusive lower bound; the empty string is the start of the table.
    pub(crate) lower: &'a [u8],
    /// Exclusive upper bound; `None` is the end of the table.
    pub(crate) upper: Option<&'a [u8]>,
}

impl<'a> KeyRange<'a> {
    /// Every key of a table.
    pub(crate) const FULL: KeyRange<'static> = KeyRange {
        lower: &[],
        upper: None,
    };

    /// The interval `[lower, upper)`.
    pub(crate) const fn new(lower: &'a [u8], upper: Option<&'a [u8]>) -> Self {
        Self { lower, upper }
    }

    /// An interval must admit at least one byte string: `lower < upper`.
    ///
    /// # Errors
    /// [`StatementError::EmptyRange`] when the upper bound does not exceed the lower.
    pub(crate) fn validate(&self) -> Result<(), StatementError> {
        match self.upper {
            Some(upper) if upper <= self.lower => Err(StatementError::EmptyRange),
            _ => Ok(()),
        }
    }

    /// Whether `key` lies inside the interval.
    pub(crate) fn contains(&self, key: &[u8]) -> bool {
        key >= self.lower && self.upper.is_none_or(|upper| key < upper)
    }
}

/// The exclusive upper bound of the keys that start with `prefix`, or `None` when no
/// byte string bounds them (an empty or all-`0xff` prefix).
///
/// A tuple or record key encodes every component as `[len][payload]` (Norito's field
/// framing), so "every key whose leading components are `c`" is the interval
/// `[p, prefix_upper_bound(p))` for the exact leading bytes `p` that the key encoding
/// emits for those components, framing included. The bare encoding of a component
/// alone is not that prefix.
pub fn prefix_upper_bound(prefix: &[u8]) -> Option<Vec<u8>> {
    let last = prefix.iter().rposition(|byte| *byte != u8::MAX)?;
    let mut upper = prefix[..=last].to_vec();
    upper[last] += 1;
    Some(upper)
}

/// Opaque canonical witness bytes of one statement (K9).
///
/// The encoding belongs to the construction. Verification consumes it completely:
/// truncated encodings and trailing bytes are rejected, and every byte counts toward
/// the carried witness caps of a transaction. Proofs need not be non-malleable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness(Vec<u8>);

impl Witness {
    /// Wrap witness bytes received from a transaction or produced by a prover.
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The canonical witness bytes.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Why a construction could not complete an operation on this node.
///
/// A local failure is never a verdict about a transaction: the caller retries or
/// defers. Verification has no such outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LocalFailure {
    /// A local memory or work budget refused the operation.
    #[error("local resources were refused")]
    Resources,
    /// Local durable storage of the construction's nodes failed.
    #[error("local storage failed")]
    Storage,
}

/// A change set that was refused; the commitment is unchanged (K10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// A change addresses an undeclared table or a cell under a nonempty key.
    #[error("invalid keyed State change: {0}")]
    Statement(#[from] StatementError),
    /// One `(table, key)` occurs twice in a net change set.
    #[error("a net change set names one key twice")]
    DuplicateChange,
    /// A local failure of the construction. Never a verdict.
    #[error("keyed State construction failed locally: {0}")]
    Local(LocalFailure),
}

/// A witness that the prover cannot produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProveError {
    /// The statement is ill-formed.
    #[error("invalid keyed State statement: {0}")]
    Statement(#[from] StatementError),
    /// Inclusion was requested for an absent key.
    #[error("the key is absent")]
    KeyAbsent,
    /// Absence was requested for a present key.
    #[error("the key is present")]
    KeyPresent,
    /// A local failure of the construction. Never a verdict.
    #[error("keyed State construction failed locally: {0}")]
    Local(LocalFailure),
}

/// Deterministic rejection of a carried witness.
///
/// Verification has no local-failure outcome: it is a pure function of its inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    /// The statement is ill-formed.
    #[error("invalid keyed State statement: {0}")]
    Statement(#[from] StatementError),
    /// The witness bytes are not a canonical witness of this construction.
    #[error("the witness is malformed")]
    Malformed,
    /// The witness is well-formed but does not establish the statement at this root.
    #[error("the witness does not establish the statement at this root")]
    NotProven,
}

/// One candidate construction of the keyed State commitment.
///
/// An implementation owns the committed entries (or their digests) of one State
/// version and answers the three statement kinds against its root. The contract
/// (`specs/sumeragi.md` §16.4, K1..K12) is checked by `conformance::check_contract`.
pub trait KeyedStateCommitment: Sized {
    /// Stable identity of the construction and its parameters, recorded with every
    /// measurement (for example `reference-oracle/v1`).
    const CONSTRUCTION: &'static str;

    /// The commitment of the State with no entries under `schema`.
    fn empty(schema: &StateSchema) -> Self;

    /// Apply one block's net change set. Each `(table, key)` occurs at most once; a
    /// `None` value for an absent key and a value equal to the committed one are
    /// no-ops. Either every change is applied or none is (K10).
    ///
    /// # Errors
    /// See [`UpdateError`]; the commitment is unchanged.
    fn apply(&mut self, changes: &[Change<'_>]) -> Result<(), UpdateError>;

    /// The root: a function of the schema and the committed entries alone (K1, K2).
    fn root(&self) -> KeyedStateRoot;

    /// Number of committed entries.
    fn entries(&self) -> u64;

    /// Witness that `(table, key)` is present with its committed value (K4).
    ///
    /// # Errors
    /// The statement is ill-formed or the key is absent.
    fn prove_inclusion(&self, table: &str, key: &[u8]) -> Result<Witness, ProveError>;

    /// Witness that `(table, key)` is absent (K5).
    ///
    /// # Errors
    /// The statement is ill-formed or the key is present.
    fn prove_absence(&self, table: &str, key: &[u8]) -> Result<Witness, ProveError>;

    /// Witness of every committed entry of `table` inside `range`, in key order (K6).
    ///
    /// # Errors
    /// The statement is ill-formed.
    fn prove_range(&self, table: &str, range: KeyRange<'_>) -> Result<Witness, ProveError>;

    /// Accept iff `root` commits `(table, key) ↦ value`.
    ///
    /// # Errors
    /// See [`Rejection`].
    fn verify_inclusion(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        key: &[u8],
        value: &[u8],
        witness: &Witness,
    ) -> Result<(), Rejection>;

    /// Accept iff `root` commits no entry at `(table, key)`.
    ///
    /// # Errors
    /// See [`Rejection`].
    fn verify_absence(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        key: &[u8],
        witness: &Witness,
    ) -> Result<(), Rejection>;

    /// Accept iff `entries` is exactly the ascending list of every entry of `table`
    /// that `root` commits inside `range`.
    ///
    /// # Errors
    /// See [`Rejection`].
    fn verify_range(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        range: KeyRange<'_>,
        entries: &[Entry<'_>],
        witness: &Witness,
    ) -> Result<(), Rejection>;
}

// The G.2 candidate prototypes behind this contract; development only.
// TODO(G.3): remove with the prototypes.
#[cfg(all(test, feature = "state-commitment-prototypes"))]
mod candidates_tests;
#[cfg(test)]
pub mod conformance;
#[cfg(test)]
pub mod reference;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod selected_tests;
