//! Paired canonical table construction using the caller's original allocation pool.
//!
//! The ordered tree retains funded final key/digest backing, Merkle levels and
//! its private owner. Staging rows and canonical keys retain their own original
//! charges through sorting and final copies. Lookup nodes retain the same pool;
//! table-selection bits are inline; schema-name and serializer scratch funding remain open.
//! No finalized authority is claimed.

use super::*;
#[path = "paired/staging.rs"]
mod staging;
use staging::{PairedDigestRow, StagedRows};

#[path = "paired/typed.rs"]
mod typed;
pub(in crate::state) use typed::{
    TypedPairedRowAllowance, TypedPairedRowError, TypedPairedTableBuilder,
};

impl CanonicalTableLeafSet {
    /// Build both table indexes from one canonical streaming value per row.
    ///
    /// Both digests consume identical bytes in a common encoder pass. Only
    /// bounded keys and digests are retained, even for large code or histories.
    ///
    /// # Errors
    /// Refuses unresolved schemas, nominal-type mismatches, duplicate keys,
    /// fixed table/row/byte bounds, and local allocation failure.
    pub(crate) fn paired_table_from_rows<'a, K, V>(
        table: &str,
        limits: LeafLimits,
        budget: &mv::allocation::AllocationBudget,
        rows: impl IntoIterator<Item = (&'a K, &'a V)>,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError>
    where
        K: Encode + NoritoSchema + 'a,
        V: Encode + NoritoSchema + 'a,
    {
        let mut builder = TypedPairedTableBuilder::new(table, limits, budget)?;
        for (key, value) in rows {
            builder = builder
                .push(
                    key,
                    value,
                    TypedPairedRowAllowance {
                        streamed_bytes: limits.max_streamed_value_bytes,
                        ordered_bytes: limits.max_ordered_table_bytes,
                    },
                )
                .map_err(TypedPairedRowError::into_leaf)?;
        }
        builder.finish()
    }

    /// Build one scoped pair from an owner-supplied semantic value projection.
    ///
    /// The caller must be the reviewed owner of the actual value and supply its
    /// declared borrowed V1 encoder. The exact semantic identity, canonical
    /// key type, row count, and bounded encoded bytes are checked here. This
    /// still cannot establish a complete State root or finality.
    ///
    /// # Errors
    /// Rejects an unresolved or mismatched schema, changed encodings, duplicate
    /// keys, row/byte limits, or local allocation failure before returning nodes.
    pub(crate) fn paired_semantic_table_from_rows<'a, K, V, S>(
        table: &str,
        semantic_identity: &'static str,
        limits: LeafLimits,
        budget: &mv::allocation::AllocationBudget,
        rows: impl IntoIterator<Item = (&'a K, &'a V)>,
        project: impl Fn(&'a V) -> S,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError>
    where
        K: Encode + NoritoSchema + 'a,
        V: 'a,
        S: Encode,
    {
        let selection = Self::new(&[table], limits, budget)?;
        let Some((table, (key_schema, value_schema))) = selection.selection.table(table) else {
            unreachable!("selected table remains registered")
        };
        if !matches!(value_schema, Schema::Semantic { identity, .. } if identity == semantic_identity)
        {
            return Err(LeafError::TypeMismatch(table));
        }
        let max_rows = limits.max_rows.min(MAX_NORITO_TREE_ENTRIES as u64);
        let mut encoded = StagedRows::new(max_rows as usize, budget)?;
        let mut retained_bytes = 0_usize;
        let mut streamed_bytes = 0_u64;
        for (key, value) in rows {
            if encoded.len() as u64 >= max_rows {
                return Err(LeafError::RowLimit);
            }
            let key =
                staging::encode_key(table, key_schema, key, limits.max_payload_bytes, budget)?;
            charge_digest_row(&mut retained_bytes, key.as_slice().len(), limits)?;
            let bound = value_stream_bound(limits, streamed_bytes)?;
            let (ordered_value_digest, lookup_value_digest, length) =
                semantic_bare_payload_digests(
                    table,
                    value_schema,
                    semantic_identity,
                    &project(value),
                    bound,
                )
                .map_err(|error| value_stream_error(error, bound, limits))?;
            charge_streamed_value(&mut streamed_bytes, length, limits)?;
            encoded.push(PairedDigestRow {
                key,
                ordered_value_digest,
                lookup_value_digest,
            })?;
        }
        Self::paired_table_from_digest_rows(
            table,
            limits,
            budget,
            selection,
            encoded,
            retained_bytes,
        )
    }

    fn paired_table_from_digest_rows(
        table: &str,
        limits: LeafLimits,
        budget: &mv::allocation::AllocationBudget,
        mut lookup: Self,
        mut encoded: StagedRows<'_>,
        retained_bytes: usize,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError> {
        let Some((table, (key_schema, _))) = lookup.selection.table(table) else {
            unreachable!("selected table remains registered")
        };
        let maximum = limits
            .max_ordered_table_bytes
            .min(MAX_NORITO_TREE_PAYLOAD_BYTES);
        if !encoded.is_empty() && ordered_retained_bytes(encoded.len(), retained_bytes)? > maximum {
            return Err(LeafError::OrderedRange(NoritoKeyRangeError::Capacity));
        }
        encoded
            .as_mut_slice()
            .sort_unstable_by(|left, right| left.key.as_slice().cmp(right.key.as_slice()));
        let ordered = NoritoKeyDigestRangeTreeV1::from_sorted_digests(
            lookup.selection.schema,
            table.as_bytes(),
            encoded
                .as_slice()
                .iter()
                .map(|row| (row.key.as_slice(), row.ordered_value_digest)),
            limits.max_ordered_table_bytes,
            budget,
        )
        .map_err(LeafError::OrderedRange)?;
        let table_len = u64::try_from(table.len()).expect("static table identity fits u64");
        for row in encoded.as_slice() {
            let key_hash = bare_payload_hash(table, key_schema, row.key.as_slice(), KEY_PAYLOAD)?;
            let path = Hash::new_from_chunks(&[
                PATH,
                &table_len.to_le_bytes(),
                table.as_bytes(),
                key_hash.as_ref(),
            ]);
            lookup
                .leaves
                .replace(path, None, Some(row.lookup_value_digest))
                .map_err(|error| lookup_error(error, table))?;
        }
        let lookup_root = lookup.root();
        let ordered_root = ordered.root();
        let row_count = u64::try_from(ordered.len()).expect("bounded table row count fits u64");
        let root = paired_root(table, row_count, lookup_root, ordered_root);
        Ok(CanonicalTablePairedSnapshot {
            lookup,
            ordered,
            root,
        })
    }
}

#[cfg(test)]
#[path = "paired/funding_tests.rs"]
mod funding_tests;

/// Exact retained canonical row framing plus complete ordered Merkle levels.
fn ordered_retained_bytes(rows: usize, retained: usize) -> Result<usize, LeafError> {
    let nodes = if rows == 0 {
        0
    } else {
        rows.checked_next_power_of_two()
            .and_then(|count| count.checked_mul(2))
            .and_then(|count| count.checked_sub(1))
            .ok_or(LeafError::OrderedRange(NoritoKeyRangeError::Capacity))?
    };
    nodes
        .checked_mul(Hash::LENGTH)
        .and_then(|bytes| retained.checked_add(bytes))
        .ok_or(LeafError::OrderedRange(NoritoKeyRangeError::Capacity))
}
