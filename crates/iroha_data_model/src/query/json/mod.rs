//! JSON helper structures for query envelopes and predicate DSL.
//!
//! This module defines the Norito-JSON structures that clients use when
//! building query requests via the command-line interface or other tools.
//! The definitions intentionally cover a subset of the full Norito DSL and
//! focus on deterministic encoding so that the resulting `CompoundPredicate`
//! payload can be signed and compared reliably across platforms.

mod envelope;
mod predicate;

pub use envelope::{
    IterableQueryJson, IterableQueryKind, IterableQueryParamsJson, QueryEnvelopeJson,
    QueryJsonError, SingularQueryJson,
};
pub use predicate::{EqualsCondition, InCondition, PredicateJson};
