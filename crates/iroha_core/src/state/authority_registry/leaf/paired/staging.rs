//! Fixed staged-row owners admitting replacement overlap before growth.

use super::*;
use iroha_allocation::{AllocationBudget, ChargedBuffer};

/// A staged canonical key and the two value commitments from one encoder pass.
pub(super) struct PairedDigestRow {
    pub(super) key: ChargedBuffer<u8>,
    pub(super) ordered_value_digest: Hash,
    pub(super) lookup_value_digest: Hash,
}

pub(super) fn encode_key<K: Encode + NoritoSchema>(
    table: &'static str,
    schema: Schema,
    key: &K,
    maximum: usize,
    budget: &AllocationBudget,
) -> Result<ChargedBuffer<u8>, LeafError> {
    typed_bare_payload(
        table,
        schema,
        key,
        maximum.min(MAX_NORITO_KEY_BYTES),
        |length| frame::FundedFrame::new(length, budget),
    )
    .map(frame::FundedFrame::into_buffer)
}

/// One single-pass staging owner borrowing the caller's unchanged original pool.
pub(super) struct StagedRows<'budget> {
    rows: ChargedBuffer<PairedDigestRow>,
    maximum: usize,
    budget: &'budget AllocationBudget,
}

impl<'budget> StagedRows<'budget> {
    pub(super) fn new(
        maximum: usize,
        budget: &'budget AllocationBudget,
    ) -> Result<Self, LeafError> {
        Ok(Self {
            rows: ChargedBuffer::new(0, budget).map_err(frame::buffer_error)?,
            maximum,
            budget,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.rows.as_slice().len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.rows.as_slice().is_empty()
    }
    pub(super) fn as_slice(&self) -> &[PairedDigestRow] {
        self.rows.as_slice()
    }
    pub(super) fn as_mut_slice(&mut self) -> &mut [PairedDigestRow] {
        self.rows.as_mut_slice()
    }

    /// An allocation refusal leaves every previous row and charge with this owner.
    pub(super) fn push(&mut self, row: PairedDigestRow) -> Result<(), LeafError> {
        if self.len() == self.maximum {
            return Err(LeafError::RowLimit);
        }
        if self.len() == self.rows.capacity() {
            let capacity = self
                .rows
                .capacity()
                .checked_mul(2)
                .unwrap_or(self.maximum)
                .max(1)
                .min(self.maximum);
            // Admit the new exact backing while the old row backing and all
            // initialized keys remain charged. Moves invoke no Clone/encoder.
            let mut grown =
                ChargedBuffer::new(capacity, self.budget).map_err(frame::buffer_error)?;
            for old in self.rows.drain_all() {
                grown.push_reserved(old);
            }
            self.rows = grown;
        }
        self.rows.push_reserved(row);
        Ok(())
    }
}

#[cfg(test)]
#[path = "staging/tests.rs"]
mod tests;
