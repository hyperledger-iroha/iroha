//! Exact original-pool custody for the entire validated parameter projection.
//!
//! The canonical decoder assembles this owner directly (`decoded`). Its interface
//! deliberately provides no mutable projection or uncharged constructor; prepared
//! successors bind the exact retained predecessor and its original pool.

use std::fmt;

use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedShared};
use iroha_data_model::parameter::CustomParametersAdmissionError;
use iroha_primitives::bigint::BigIntAdmissionCloneError;

use super::StateParameters;

mod decoded;
pub(super) use decoded::{DecodedParameterParts, ReservedParameterGraph};

/// Original local refusal; no monetary validity rule is weakened by admission.
#[derive(Debug, thiserror::Error)]
pub enum ParameterRetentionError {
    /// The exact original finite pool refused the complete requested backing.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// Original physical buffer or reservation refusal.
    #[error(transparent)]
    Buffer(#[from] iroha_allocation::PrepaidBufferError),
    /// Original physical shared-control refusal.
    #[error(transparent)]
    Shared(#[from] iroha_allocation::PrepaidSharedError),
    /// Original custom-map or self-custodial JSON admission refusal.
    #[error(transparent)]
    Custom(#[from] CustomParametersAdmissionError),
    /// Exact native mantissa layout/allocator refusal.
    #[error(transparent)]
    Numeric(#[from] BigIntAdmissionCloneError),
    /// The original codec resource error remains inspectable through this source.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// A different original pool was offered to an existing retained version.
    #[error("parameter owner belongs to a different original allocation pool")]
    ForeignPool,
    /// A sibling update replaced the exact predecessor used for preparation.
    #[error("prepared parameters refer to a different predecessor version")]
    StalePredecessor,
    /// An initialized mandatory policy cannot disappear from a successor.
    #[error("prepared parameters remove an initialized mandatory policy")]
    MissingInitializedPolicy,
    /// The exhaustive private layout plan and copy did not agree.
    #[error("parameter graph allocation plan did not match its exact copy")]
    PlanMismatch,
}

/// One immutable complete canonical-and-projection graph, shared by MV versions.
///
/// Cloning only retains this exact shared allocation. Each custom JSON backing
/// keeps its own original charge even if a raw Json clone escapes the borrow;
/// the outer ledger never charges those independently self-owned allocations.
pub struct RetainedStateParameters(ChargedShared<ParameterGraph>);

// The exact value always dies before every backing charge.
struct ParameterGraph {
    value: StateParameters,
    custody: decoded::DecodedCustody,
}
impl core::ops::Deref for RetainedStateParameters {
    type Target = StateParameters;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

// Snapshot and authority writers borrow the one canonical source. There is no
// ordinary Deserialize implementation: cold restore must provide its real pool.
impl norito::SerializePayload for RetainedStateParameters {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.get().serialize(encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.get().encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.get().encoded_len_exact()
    }
}
impl norito::json::JsonSerialize for RetainedStateParameters {
    fn json_serialize(&self, out: &mut String) {
        self.get().json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        self.get().json_serialize_to(out)
    }
}

impl Clone for RetainedStateParameters {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl fmt::Debug for RetainedStateParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl PartialEq for RetainedStateParameters {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for RetainedStateParameters {}

impl RetainedStateParameters {
    /// Exact production graph shell layout for late-boundary allocator controls.
    #[cfg(test)]
    pub(crate) fn graph_allocation_layout_for_testing() -> std::alloc::Layout {
        ChargedShared::<ParameterGraph>::allocation_layout()
    }

    /// Admit an explicitly authored structural fixture without a runtime transition.
    /// The graph goes through the production snapshot reader into this pool. This is
    /// limited to component tests for missing/corrupt original policy; it grants no
    /// initialization capability and does not relax prepared successors.
    ///
    /// # Errors
    /// Returns the exact original decoder, finite-pool or physical refusal.
    #[cfg(test)]
    pub(crate) fn try_copy_for_testing(
        source: &StateParameters,
        budget: &AllocationBudget,
    ) -> Result<Self, super::ParameterIngressError> {
        let source = norito::json::to_json(source)?;
        let mut parser = norito::json::Parser::new(&source);
        parser.preflight_document()?;
        let parameters = Self::try_decode_json(&mut parser, budget)?;
        parser.finish_document()?;
        Ok(parameters)
    }

    /// Borrow this exact validated graph; no mutable access or decode occurs.
    pub(crate) fn get(&self) -> &StateParameters {
        &self.0.value
    }

    /// Check controls, plain graph custody, and custom/JSON custody together.
    pub(crate) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.belongs_to(budget)
            && self.0.custody.belongs_to(budget)
            && self.get().parameters.custom().belongs_to(budget)
    }

    /// Exact identity of the original retained version, independent of value equality.
    pub(crate) fn same_version(&self, other: &Self) -> bool {
        ChargedShared::ptr_eq(&self.0, &other.0)
    }

    /// Bind a directly admitted complete successor to this exact retained version.
    /// The execution owner validates scope/liabilities and prepares events first.
    /// This transition allocates nothing and preserves both owners on refusal.
    pub(crate) fn prepare_admitted(
        &self,
        next: Self,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, (Self, ParameterRetentionError)> {
        let error = if !self.belongs_to(budget) || !next.belongs_to(budget) {
            Some(ParameterRetentionError::ForeignPool)
        } else if (self.crypto_policy().is_some() && next.crypto_policy().is_none())
            || (self.staking_policy().is_some() && next.staking_policy().is_none())
            || (self.root_fees().is_some() && next.root_fees().is_none())
        {
            Some(ParameterRetentionError::MissingInitializedPolicy)
        } else {
            None
        };
        if let Some(error) = error {
            return Err((next, error));
        }
        Ok(PreparedParameters {
            predecessor: self.clone(),
            successor: next,
        })
    }

    /// Prepare a complete validated successor before a physical MV writer is opened.
    ///
    /// This binds the exact retained predecessor, not only the enclosing block
    /// publication token: two already applied siblings must not share a stale plan.
    /// The execution owner must already have checked scope, authorization, retained
    /// liabilities and funded its decoder/event backing before calling this method.
    #[cfg(test)]
    pub(crate) fn prepare_replace(
        &self,
        next: &StateParameters,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, super::ParameterIngressError> {
        if !self.belongs_to(budget) {
            return Err(ParameterRetentionError::ForeignPool.into());
        }
        if (self.get().crypto_policy().is_some() && next.crypto_policy().is_none())
            || (self.get().staking_policy().is_some() && next.staking_policy().is_none())
            || (self.get().root_fees().is_some() && next.root_fees().is_none())
        {
            return Err(ParameterRetentionError::MissingInitializedPolicy.into());
        }
        self.prepare_admitted(Self::try_copy_for_testing(next, budget)?, budget)
            .map_err(|(next, error)| {
                drop(next);
                error.into()
            })
    }
}

/// A fully admitted successor bound to the exact predecessor and original pool.
#[derive(Debug)]
pub struct PreparedParameters {
    predecessor: RetainedStateParameters,
    successor: RetainedStateParameters,
}

impl PreparedParameters {
    /// Borrow the fully admitted successor for scope and liability checks.
    /// This never exposes mutable backing or commits a State change.
    pub(crate) fn successor(&self) -> &StateParameters {
        self.successor.get()
    }
    /// Refuse foreign or stale custody before opening/touching the destination Cell.
    pub(crate) fn check_source(
        &self,
        current: &RetainedStateParameters,
        budget: &AllocationBudget,
    ) -> Result<(), ParameterRetentionError> {
        if !current.belongs_to(budget)
            || !self.predecessor.belongs_to(budget)
            || !self.successor.belongs_to(budget)
        {
            return Err(ParameterRetentionError::ForeignPool);
        }
        if !self.predecessor.same_version(current) {
            return Err(ParameterRetentionError::StalePredecessor);
        }
        Ok(())
    }

    /// Extract a fully funded successor after checking the retained current source.
    /// The execution owner must complete its authorization, scope and output
    /// preparation before installing this value; this method creates no authority.
    pub(crate) fn into_successor(
        self,
        current: &RetainedStateParameters,
        budget: &AllocationBudget,
    ) -> Result<RetainedStateParameters, (Self, ParameterRetentionError)> {
        if let Err(error) = self.check_source(current, budget) {
            return Err((self, error));
        }
        Ok(self.successor)
    }

    /// Install with no allocation after the source check and all execution preparation.
    /// Refusal returns the original prepared owner intact and leaves current untouched.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn install(
        self,
        current: &mut RetainedStateParameters,
        budget: &AllocationBudget,
    ) -> Result<(), (Self, ParameterRetentionError)> {
        if let Err(error) = self.check_source(current, budget) {
            return Err((self, error));
        }
        *current = self.successor;
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests;
