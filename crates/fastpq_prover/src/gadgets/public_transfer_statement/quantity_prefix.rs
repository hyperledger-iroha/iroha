//! Incremental arithmetic and chronology checks for one original occurrence.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{FastpqQuantityUnits, TransferDeltaTranscript};
use iroha_primitives::numeric::MAX_DECIMAL_SCALE;

use super::{
    DeltaView, balance_key, check_delta_digest_policy, check_limit, checked_add, invariant,
    normalized_delta_values_for,
};
use crate::Result;

/// Explicit inclusive bounds for one incrementally checked transcript occurrence.
/// These limits do not replace public statement byte, row or path-allocation limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityPrefixLimits {
    /// Maximum original debit/credit pairs in this occurrence.
    pub max_deltas: usize,
    /// Maximum distinct complete canonical asset/account balance keys.
    pub max_unique_keys: usize,
}

/// Incremental full-domain arithmetic and balance chronology for one occurrence.
///
/// Each append checks all five original quantities, exact sender subtraction and
/// receiver addition including reconstruction into the ledger quantity domain,
/// debit-then-credit repeated-key chronology, and the complete prefix's singleton
/// or multiple-delta digest policy. Keys use the existing canonical full identity
/// encoding. At most two keys are consulted and updated per delta; prior maps and
/// original deltas are never copied or replayed.
///
/// Scale 28 is used only for internal exact comparisons. This type never generates
/// selected-asset-scale rows or leaf commitments. Passing these checks establishes
/// the same arithmetic/chronology Boolean facts as full quantity preparation; error
/// variants and precedence can differ. Profile, row coverage, public byte bounds,
/// allocation, roots, authority and source authentication remain separate checks.
/// Construct a new validator for each separately measured transcript occurrence.
#[derive(Debug)]
pub struct QuantityPrefixValidator {
    batch_hash: Hash,
    limits: QuantityPrefixLimits,
    count: usize,
    last_values: BTreeMap<Vec<u8>, FastpqQuantityUnits>,
}

impl QuantityPrefixValidator {
    /// Start an empty occurrence with a caller-owned batch identity and explicit caps.
    /// The identity is retained, not authenticated; zero caps permit no appends.
    #[must_use]
    pub fn new(batch_hash: Hash, limits: QuantityPrefixLimits) -> Self {
        Self {
            batch_hash,
            limits,
            count: 0,
            last_values: BTreeMap::new(),
        }
    }

    /// Check the next original delta without mutating committed prefix state.
    ///
    /// `digest` is the digest policy of the complete prospective prefix: the first
    /// delta requires its original Poseidon digest; all later prefixes require
    /// `None`. The pending update exclusively borrows this validator and immutably
    /// borrows the original delta, so validation cannot become stale before commit.
    /// Dropping the pending update leaves counts and chronology unchanged.
    ///
    /// # Errors
    /// Rejects count/key limits or overflow, digest policy, full-domain arithmetic,
    /// canonical key encoding errors, and repeated-key chronology disagreements.
    pub fn prepare<'a>(
        &'a mut self,
        delta: &'a TransferDeltaTranscript,
        digest: Option<Hash>,
    ) -> Result<PendingQuantityPrefixUpdate<'a>> {
        let count = checked_add(self.count, 1)?;
        check_limit("max_public_transfer_deltas", count, self.limits.max_deltas)?;
        let view = DeltaView::from(delta);
        check_delta_digest_policy(&self.batch_hash, digest, count, view)?;
        let values = normalized_delta_values_for::<FastpqQuantityUnits>(view, MAX_DECIMAL_SCALE)?;
        let from_key = balance_key(view.asset, view.from)?;
        let to_key = balance_key(view.asset, view.to)?;
        let from_previous = self.last_values.get(&from_key);
        if from_previous.is_some_and(|previous| *previous != values[1]) {
            return Err(invariant("public repeated-key balances do not chain"));
        }
        let same_key = from_key == to_key;
        let to_previous = if same_key {
            // Debit executes first even for zero amounts and self transfers.
            Some(&values[2])
        } else {
            self.last_values.get(&to_key)
        };
        if to_previous.is_some_and(|previous| *previous != values[3]) {
            return Err(invariant("public repeated-key balances do not chain"));
        }
        let added_keys =
            usize::from(from_previous.is_none()) + usize::from(!same_key && to_previous.is_none());
        let unique_keys = checked_add(self.last_values.len(), added_keys)?;
        check_limit(
            "max_public_transfer_unique_keys",
            unique_keys,
            self.limits.max_unique_keys,
        )?;
        let from_after = if same_key { values[4] } else { values[2] };
        let to_update = (!same_key).then_some((to_key, values[4]));
        Ok(PendingQuantityPrefixUpdate {
            validator: self,
            delta,
            count,
            unique_keys,
            from_update: (from_key, from_after),
            to_update,
        })
    }

    /// Number of original deltas whose pending updates have been committed.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Number of complete balance keys retained by committed deltas.
    #[must_use]
    pub fn unique_keys(&self) -> usize {
        self.last_values.len()
    }

    /// Caller-supplied batch identity used by singleton digest validation.
    #[must_use]
    pub const fn batch_hash(&self) -> Hash {
        self.batch_hash
    }

    /// Explicit occurrence limits supplied at construction.
    #[must_use]
    pub const fn limits(&self) -> QuantityPrefixLimits {
        self.limits
    }
}

/// Checked, uncommitted next delta tied to its original data and exclusive owner.
/// Drop this value to abandon the append. It cannot be cloned or independently built.
#[must_use = "commit the checked delta or drop it to preserve the previous prefix"]
#[derive(Debug)]
pub struct PendingQuantityPrefixUpdate<'a> {
    validator: &'a mut QuantityPrefixValidator,
    delta: &'a TransferDeltaTranscript,
    count: usize,
    unique_keys: usize,
    from_update: (Vec<u8>, FastpqQuantityUnits),
    to_update: Option<(Vec<u8>, FastpqQuantityUnits)>,
}

impl<'a> PendingQuantityPrefixUpdate<'a> {
    /// Exact original delta borrowed during validation, including untouched witnesses.
    #[must_use]
    pub const fn delta(&self) -> &'a TransferDeltaTranscript {
        self.delta
    }

    /// Prospective original-delta count after committing this update.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Prospective distinct complete-key count after committing this update.
    #[must_use]
    pub const fn unique_keys(&self) -> usize {
        self.unique_keys
    }

    /// Consume the checked update and install its at-most-two final balance values.
    /// No validation or fallible arithmetic remains; no prior-prefix scan is needed.
    pub fn commit(self) {
        let Self {
            validator,
            count,
            from_update: (from_key, from_after),
            to_update,
            ..
        } = self;
        validator.last_values.insert(from_key, from_after);
        if let Some((key, after)) = to_update {
            validator.last_values.insert(key, after);
        }
        validator.count = count;
    }
}

#[cfg(test)]
mod tests;
