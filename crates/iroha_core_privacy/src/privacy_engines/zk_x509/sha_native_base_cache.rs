//! Lossless compact storage of the original, immutable native SHA base rows.
//!
//! The validated word circuit uses eleven u32 columns and Boolean cells elsewhere.
//! Packing checks every original cell before retaining it. This is a source owner,
//! never a polynomial or an alternate witness, and moves unchanged through X5B1.

use super::*;

type Row = [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1];
type Error = ZkX509ShaCallBusStarkErrorV1;

/// Explicitly initialized reserved word avoids implicit alignment padding. Every
/// initialized byte belongs to one of the primitive arrays cleared on release.
#[repr(C)]
#[derive(Default)]
struct PackedRowV1 {
    words: [u32; 12],
    bits: [u64; 2],
}

const _: () = assert!(core::mem::size_of::<PackedRowV1>() == 64);

const fn word_index_v1(column: usize) -> Option<usize> {
    match column {
        0..=5 => Some(column),
        36..=39 => Some(column - 30),
        75 => Some(10),
        _ => None,
    }
}

impl PackedRowV1 {
    fn pack_v1(&mut self, row: &Row) -> Result<(), Error> {
        // Reject the entire original row before copying any field. In particular
        // a noncanonical Goldilocks representative cannot be truncated into u32.
        for (column, value) in row.iter().enumerate() {
            if word_index_v1(column).is_some() {
                u32::try_from(value.0).map_err(|_| Error::Topology)?;
            } else if value.0 > 1 {
                return Err(Error::Topology);
            }
        }
        for (column, value) in row.iter().enumerate() {
            if let Some(index) = word_index_v1(column) {
                self.words[index] = value.0 as u32;
            } else {
                self.bits[column / 64] |= value.0 << (column % 64);
            }
        }
        Ok(())
    }

    fn field_v1(&self, column: usize) -> F {
        if let Some(index) = word_index_v1(column) {
            F(u64::from(self.words[index]))
        } else {
            F((self.bits[column / 64] >> (column % 64)) & 1)
        }
    }
}

fn clear_rows_v1(rows: &mut [PackedRowV1]) {
    for row in rows {
        super::super::private_table::zeroize_words_v1(&mut row.words);
        super::super::private_table::zeroize_words_v1(&mut row.bits);
    }
}

/// A single exact-capacity native segment, with no copied challenge or terminal.
pub(super) struct ShaNativeBaseCacheV1 {
    rows: PrivateTableV1<PackedRowV1>,
}

impl ShaNativeBaseCacheV1 {
    /// The enclosing source charges the inline owner; this is only its heap.
    pub(super) fn heap_forecast_v1(rows: usize) -> Result<usize, Error> {
        if rows == 0 {
            return Err(Error::Resource);
        }
        rows.checked_mul(core::mem::size_of::<PackedRowV1>())
            .ok_or(Error::Resource)
    }

    /// Run the original validated replay once after reserving the entire public
    /// extent. Failed, incomplete, reordered and unwinding replays clear all rows.
    pub(super) fn build_v1(
        row_count: usize,
        replay: impl FnOnce(&mut dyn FnMut(usize, &Row) -> Result<(), Error>) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let forecast = Self::heap_forecast_v1(row_count)?;
        let mut rows = PrivateTableV1::new(Vec::new(), clear_rows_v1);
        rows.try_reserve_exact(row_count)
            .map_err(|_| Error::Resource)?;
        if rows.capacity() != row_count {
            return Err(Error::Resource);
        }
        rows.resize_with(row_count, PackedRowV1::default);
        let mut written = 0;
        let mut valid = true;
        replay(&mut |index, row| {
            if !valid || index != written || index >= row_count {
                valid = false;
                return Err(Error::Topology);
            }
            if let Err(error) = rows[index].pack_v1(row) {
                valid = false;
                return Err(error);
            }
            written += 1;
            Ok(())
        })?;
        if !valid || written != row_count {
            return Err(Error::Topology);
        }
        let cache = Self { rows };
        if cache.heap_bytes_v1() != forecast {
            return Err(Error::Resource);
        }
        Ok(cache)
    }

    /// Actual capacity, including every reserved word and bitset.
    pub(super) fn heap_bytes_v1(&self) -> usize {
        self.rows
            .capacity()
            .saturating_mul(core::mem::size_of::<PackedRowV1>())
    }

    /// Independent source tests decode the complete row into a clearing owner.
    #[cfg(test)]
    pub(super) fn row_v1(&self, index: usize) -> zeroize::Zeroizing<Row> {
        zeroize::Zeroizing::new(core::array::from_fn(|column| {
            self.rows[index].field_v1(column)
        }))
    }

    /// Test that X5B1 transfers the one allocation without a retained duplicate.
    #[cfg(test)]
    pub(super) fn allocation_identity_v1(&self) -> usize {
        self.rows.as_ptr() as usize
    }

    /// Decode only the requested columns into the existing atomic fill owners.
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
                .is_none_or(|end| end > ZK_X509_SHA_BATCH_BASE_WIDTH_V1)
            || fills
                .iter()
                .any(|fill| fill.target.len() != self.rows.len())
        {
            return Err(Error::Topology);
        }
        for (index, row) in self.rows.iter().enumerate() {
            for (offset, fill) in fills.iter_mut().enumerate() {
                fill.write_v1(index, row.field_v1(first + offset));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "sha_native_base_cache_tests.rs"]
mod tests;
