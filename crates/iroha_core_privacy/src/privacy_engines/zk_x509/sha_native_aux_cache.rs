//! Fixed native SHA auxiliary rows retained under MAIN's original source allowance.
//!
//! This owner stores the existing post-X5B1 recurrence without changing a row,
//! mask, transcript, polynomial, opening or terminal claim. It is built only by
//! the bound source. Native fields and terminal products are erased on release.

use super::*;

type Row = [F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1];
type Error = ZkX509ShaCallBusStarkErrorV1;

/// One original segment matrix and its clearing, private terminal owner.
pub(super) struct ShaNativeAuxCacheV1 {
    rows: PrivateTableV1<Row>,
    terminals: ZkX509ShaSegmentAirTerminalsV1,
}

impl ShaNativeAuxCacheV1 {
    /// Inline headers are charged by the enclosing source; include all possible
    /// terminal capacity, even in a segment with fewer or no CA calls.
    pub(super) fn heap_forecast_v1(rows: usize) -> Result<usize, Error> {
        if rows == 0 {
            return Err(Error::Resource);
        }
        rows.checked_mul(core::mem::size_of::<Row>())
            .and_then(|bytes| {
                ZK_X509_SHA_CA_CALL_COUNT_V1
                    .checked_mul(core::mem::size_of::<ZkX509ShaCallBoundaryTerminalV1>())
                    .and_then(|terminals| bytes.checked_add(terminals))
            })
            .ok_or(Error::Resource)
    }

    /// Reserve the complete public extent before invoking the original recurrence.
    pub(super) fn build_v1(
        row_count: usize,
        segment: usize,
        replay: impl FnOnce(&mut dyn FnMut(usize, Row)) -> Result<ZkX509ShaSegmentAirTerminalsV1, Error>,
    ) -> Result<Self, Error> {
        let forecast = Self::heap_forecast_v1(row_count)?;
        if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
            return Err(Error::Topology);
        }
        let mut rows = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1);
        rows.try_reserve_exact(row_count)
            .map_err(|_| Error::Resource)?;
        // Refuse allocator over-capacity before any private field is written.
        if rows.capacity() != row_count {
            return Err(Error::Resource);
        }
        rows.resize(row_count, [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]);
        let mut written = 0;
        let mut valid = true;
        let terminals = replay(&mut |index, row| {
            if !valid
                || index != written
                || index >= row_count
                || row.iter().any(|value| F::canonical(value.0).is_none())
            {
                valid = false;
                return;
            }
            rows[index] = row;
            written += 1;
        })?;
        if !valid || written != row_count {
            return Err(Error::Topology);
        }
        validate_terminals_v1(segment, &terminals)?;
        let cache = Self { rows, terminals };
        if cache.heap_bytes_v1() > forecast {
            return Err(Error::Resource);
        }
        Ok(cache)
    }

    /// Count actual capacities; overflow cannot reduce the resource charge.
    pub(super) fn heap_bytes_v1(&self) -> usize {
        self.rows
            .capacity()
            .saturating_mul(core::mem::size_of::<Row>())
            .saturating_add(
                self.terminals
                    .ca_call_boundaries
                    .capacity()
                    .saturating_mul(core::mem::size_of::<ZkX509ShaCallBoundaryTerminalV1>()),
            )
    }

    /// Test-only immutable comparison with the independent original recurrence.
    #[cfg(test)]
    pub(super) fn rows_v1(&self) -> &[Row] {
        &self.rows
    }

    /// Project adjacent columns through the existing atomic destination guards.
    pub(super) fn fill_columns_v1(
        &self,
        first: usize,
        fills: &mut [ZkX509ShaColumnFillGuardV1<'_>],
    ) -> Result<(), Error> {
        if fills.is_empty()
            || fills.len()
                > crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || first
                .checked_add(fills.len())
                .is_none_or(|end| end > ZK_X509_SHA_BATCH_AUX_WIDTH_V1)
            || fills
                .iter()
                .any(|fill| fill.target.len() != self.rows.len())
        {
            return Err(Error::Topology);
        }
        for (index, row) in self.rows.iter().enumerate() {
            for (offset, fill) in fills.iter_mut().enumerate() {
                fill.write_v1(index, row[first + offset]);
            }
        }
        Ok(())
    }

    /// A fallible, clearing copy of the small terminal owner. Row storage is
    /// never cloned, and no private product is copied before capacity is fixed.
    pub(super) fn copy_terminals_v1(&self) -> Result<ZkX509ShaSegmentAirTerminalsV1, Error> {
        let original = &self.terminals.ca_call_boundaries;
        let mut boundaries = PrivateTableV1::new(Vec::new(), clear_ca_boundary_products_v1);
        boundaries
            .try_reserve_exact(original.len())
            .map_err(|_| Error::Resource)?;
        if boundaries.capacity() != original.len() {
            return Err(Error::Resource);
        }
        boundaries.extend_from_slice(original);
        Ok(ZkX509ShaSegmentAirTerminalsV1 {
            segment: self.terminals.segment.clone(),
            ca_call_boundaries: boundaries.into_vec(),
        })
    }
}

fn validate_terminals_v1(
    segment: usize,
    terminals: &ZkX509ShaSegmentAirTerminalsV1,
) -> Result<(), Error> {
    if usize::from(terminals.segment.segment) != segment
        || terminals
            .segment
            .rfc_stream_products
            .iter()
            .flatten()
            .any(|value| F::canonical(value.0).is_none())
        || terminals.ca_call_boundaries.capacity() > ZK_X509_SHA_CA_CALL_COUNT_V1
    {
        return Err(Error::Terminal);
    }
    let first = ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[..segment]
        .iter()
        .sum::<usize>();
    let end = first + ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[segment];
    let mut actual = terminals.ca_call_boundaries.iter();
    for call in &ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1[first..end] {
        if usize::from(*call) < ZK_X509_SHA_CA_LEAF_CALL_V1 {
            continue;
        }
        let terminal = actual.next().ok_or(Error::Terminal)?;
        terminal.validate_identity_v1(usize::from(*call) - ZK_X509_SHA_CA_LEAF_CALL_V1)?;
    }
    if actual.next().is_some() {
        return Err(Error::Terminal);
    }
    Ok(())
}

#[cfg(test)]
#[path = "sha_native_aux_cache_tests.rs"]
mod tests;
