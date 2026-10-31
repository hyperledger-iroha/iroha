//! Bounded DER projection preserving the original native source checks.
//!
//! The caller owns the already admitted column buffers. Select each row once
//! per public batch without retaining another private matrix or changing
//! canonical inactive rows, carried terminals or field-validation boundaries.

use super::*;
use crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1;
use zeroize::Zeroizing;

type Error = ZkX509DerStarkErrorV1;

/// Public column positions select the same carried suffixes as the dense
/// padding constructor. Neither the plan nor its allocation depends on fields.
struct AuxProjectionV1 {
    first: usize,
    width: usize,
    padding_sources: [Option<usize>; MASKED_TRACE_LDE_COLUMN_BATCH_V1],
}

// The selected output and temporary base validation, including this public
// plan, fit below the original full auxiliary-row scratch. No additional
// source allowance or parallel private owner is required.
const _: () = assert!(
    core::mem::size_of::<AuxProjectionV1>()
        + core::mem::size_of::<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]>()
        + 2 * core::mem::size_of::<[F; MASKED_TRACE_LDE_COLUMN_BATCH_V1]>()
        <= core::mem::size_of::<[F; ZK_X509_DER_STARK_AUX_WIDTH_V1]>()
);

impl AuxProjectionV1 {
    fn new_v1(first: usize, width: usize) -> Result<Self, Error> {
        if width == 0
            || width > MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || first
                .checked_add(width)
                .is_none_or(|end| end > ZK_X509_DER_STARK_AUX_WIDTH_V1)
        {
            return Err(Error::Resource);
        }
        let mut padding_sources = [None; MASKED_TRACE_LDE_COLUMN_BATCH_V1];
        for (offset, source) in padding_sources.iter_mut().take(width).enumerate() {
            let column = first + offset;
            for (before, after) in [
                (AUX_STACK_PUSH_BEFORE, AUX_STACK_PUSH_AFTER),
                (AUX_STACK_POP_BEFORE, AUX_STACK_POP_AFTER),
                (AUX_DOCUMENT_BEFORE, AUX_DOCUMENT_AFTER),
                (AUX_NODE_BEFORE, AUX_NODE_AFTER),
                (AUX_PAIR_PRODUCER_BEFORE, AUX_PAIR_PRODUCER_AFTER),
                (AUX_PAIR_CONSUMER_BEFORE, AUX_PAIR_CONSUMER_AFTER),
                (AUX_BYTE_TABLE_SUM_BEFORE, AUX_BYTE_TABLE_SUM_AFTER),
                (AUX_BYTE_QUERY_SUM_BEFORE, AUX_BYTE_QUERY_SUM_AFTER),
                (
                    AUX_BYTE_TABLE_ZERO_COUNT_BEFORE,
                    AUX_BYTE_TABLE_ZERO_COUNT_AFTER,
                ),
                (
                    AUX_BYTE_QUERY_ZERO_COUNT_BEFORE,
                    AUX_BYTE_QUERY_ZERO_COUNT_AFTER,
                ),
                (AUX_INPUT_BYTE_BEFORE, AUX_INPUT_BYTE_AFTER),
            ] {
                for start in [before, after] {
                    if (start..start + ZK_X509_DER_STARK_BUS_LANES_V1).contains(&column) {
                        *source = Some(after + column - start);
                    }
                }
            }
        }
        Ok(Self {
            first,
            width,
            padding_sources,
        })
    }

    /// Preserve the dense constructor's source accesses and validation order,
    /// including its base-padding validation, before projecting private cells.
    fn row_v1(
        &self,
        trace: &ZkX509DerStarkTraceV1,
        index: usize,
    ) -> Result<[F; MASKED_TRACE_LDE_COLUMN_BATCH_V1], Error> {
        if index >= ZK_X509_DER_STARK_TRACE_SIZE_V1 {
            return Err(Error::Resource);
        }
        let compact_index = if index < trace.base.private_shape.parser_rows {
            Some(index)
        } else if (ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1
            ..ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + trace.base.private_shape.comparator_rows)
            .contains(&index)
        {
            Some(
                trace
                    .base
                    .private_shape
                    .parser_rows
                    .checked_add(index - ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1)
                    .ok_or(Error::Resource)?,
            )
        } else {
            None
        };
        if let Some(compact_index) = compact_index {
            let row = trace.aux_rows.get(compact_index).ok_or(Error::Row)?;
            let mut selected = Zeroizing::new([F::ZERO; MASKED_TRACE_LDE_COLUMN_BATCH_V1]);
            selected[..self.width].copy_from_slice(&row[self.first..self.first + self.width]);
            return Ok(*selected);
        }
        let carry_index = if index < ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 {
            trace
                .base
                .private_shape
                .parser_rows
                .checked_sub(1)
                .ok_or(Error::Shape)?
        } else {
            trace.aux_rows.len().checked_sub(1).ok_or(Error::Shape)?
        };
        let carry = trace.aux_rows.get(carry_index).ok_or(Error::Row)?;
        let _base = Zeroizing::new(zk_x509_der_stark_aggregate_base_row_v1(&trace.base, index)?);
        let mut selected = Zeroizing::new([F::ZERO; MASKED_TRACE_LDE_COLUMN_BATCH_V1]);
        for (offset, target) in selected.iter_mut().take(self.width).enumerate() {
            *target = match self.padding_sources[offset] {
                Some(source) => carry[source],
                None => DER_INACTIVE_LOW_DEGREE_AUXILIARIES_V1[self.first + offset],
            };
        }
        Ok(*selected)
    }
}

struct ColumnBatchGuardV1<'a, 'b> {
    targets: &'a mut [&'b mut [F]],
    complete: bool,
}

impl Drop for ColumnBatchGuardV1<'_, '_> {
    fn drop(&mut self) {
        if !self.complete {
            for target in self.targets.iter_mut() {
                super::super::private_table::zeroize_fields_v1(target);
            }
        }
    }
}

fn project_rows_v1<const WIDTH: usize>(
    row_count: usize,
    first: usize,
    targets: &mut [&mut [F]],
    mut row_at: impl FnMut(usize) -> Result<[F; WIDTH], Error>,
) -> Result<(), Error> {
    // Public extent refusals precede source access and leave targets untouched.
    if row_count == 0
        || targets.is_empty()
        || targets.len() > MASKED_TRACE_LDE_COLUMN_BATCH_V1
        || first
            .checked_add(targets.len())
            .is_none_or(|end| end > WIDTH)
        || targets.iter().any(|target| target.len() != row_count)
    {
        return Err(Error::Resource);
    }
    let mut guard = ColumnBatchGuardV1 {
        targets,
        complete: false,
    };
    for index in 0..row_count {
        let row = Zeroizing::new(row_at(index)?);
        for (offset, target) in guard.targets.iter_mut().enumerate() {
            // Preserve exact fields; MAIN's existing native-column admission
            // performs canonicality checks before mask sampling as before.
            target[index] = row[first + offset];
        }
    }
    guard.complete = true;
    Ok(())
}

/// Fill up to eight adjacent base columns without allocating private backing.
pub fn fill_zk_x509_der_stark_native_base_columns_v1(
    base: &ZkX509DerStarkBaseV1,
    first: usize,
    targets: &mut [&mut [F]],
) -> Result<(), Error> {
    project_rows_v1(ZK_X509_DER_STARK_TRACE_SIZE_V1, first, targets, |index| {
        zk_x509_der_stark_aggregate_base_row_v1(base, index)
    })
}

/// Fill up to eight adjacent auxiliary columns, retaining original padding checks.
pub fn fill_zk_x509_der_stark_native_aux_columns_v1(
    trace: &ZkX509DerStarkTraceV1,
    first: usize,
    targets: &mut [&mut [F]],
) -> Result<(), Error> {
    let projection = AuxProjectionV1::new_v1(first, targets.len())?;
    project_rows_v1(ZK_X509_DER_STARK_TRACE_SIZE_V1, 0, targets, |index| {
        projection.row_v1(trace, index)
    })
}

#[cfg(test)]
#[path = "der_native_columns_tests.rs"]
mod tests;
