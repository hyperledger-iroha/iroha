//! One clearing physical source matrix consumed before commitment allocations.

use rand::TryCryptoRng;
use zeroize::Zeroize;

use super::deep_masked_replay::{MaskedTraceReplay, ReplayLimits};
use crate::{
    Error, Result,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT, SmtRow},
        compact_trace_columns::smt_row_cells,
    },
};

/// Fixed column-major storage; no duplicate matrix or independently growing columns.
/// It has no Clone, Debug or serialization surface and is consumed by replay setup.
pub(super) struct OwnedTraceSource {
    values: Box<[u64]>,
}

impl OwnedTraceSource {
    fn zeroed(cells: usize) -> Result<Self> {
        cells
            .checked_mul(size_of::<u64>())
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or_else(|| invalid("physical source allocation overflows"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(cells)
            .map_err(|_| invalid("physical source allocation failed"))?;
        values.resize(cells, 0);
        // Any conversion reallocates only public zeroes; install this Drop owner
        // before transposing even the first private row into the allocation.
        Ok(Self {
            values: values.into_boxed_slice(),
        })
    }

    pub(super) fn from_rows(rows: &[SmtRow]) -> Result<Self> {
        if rows.len() != PHYSICAL_ROW_COUNT {
            return Err(invalid(
                "physical source requires the fixed complete row count",
            ));
        }
        let mut source = Self::zeroed(COLUMN_COUNT * PHYSICAL_ROW_COUNT)?;
        for (row, value) in rows.iter().enumerate() {
            for (column, value) in smt_row_cells(value).into_iter().enumerate() {
                source.values[column * PHYSICAL_ROW_COUNT + row] = value;
            }
        }
        Ok(source)
    }

    fn columns(&self) -> Result<[&[u64]; COLUMN_COUNT]> {
        if self.values.len() != COLUMN_COUNT * PHYSICAL_ROW_COUNT {
            return Err(invalid(
                "physical source requires the fixed complete matrix",
            ));
        }
        Ok(core::array::from_fn(|column| {
            &self.values[column * PHYSICAL_ROW_COUNT..(column + 1) * PHYSICAL_ROW_COUNT]
        }))
    }

    /// Validate and initialize the existing replay in its original entropy order.
    /// This source is cleared before returning, including validation/RNG failures.
    pub(super) fn into_replay(
        self,
        limits: ReplayLimits,
        rng: &mut impl TryCryptoRng,
    ) -> Result<MaskedTraceReplay> {
        let replay = MaskedTraceReplay::new(limits, &self.columns()?, rng)?;
        drop(self);
        Ok(replay)
    }
}

impl Drop for OwnedTraceSource {
    fn drop(&mut self) {
        #[cfg(test)]
        let before = ERASURES.with(|observed| {
            observed
                .borrow()
                .as_ref()
                .map(|_| self.values.iter().filter(|value| **value != 0).count())
        });
        self.values.zeroize();
        #[cfg(test)]
        if let Some(before) = before {
            ERASURES.with(|observed| {
                if let Some(records) = observed.borrow_mut().as_mut() {
                    records.push((
                        self.values.len(),
                        before,
                        self.values.iter().filter(|value| **value != 0).count(),
                    ));
                }
            });
        }
    }
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
thread_local! {
    static ERASURES: std::cell::RefCell<Option<Vec<(usize, usize, usize)>>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::compact_public_columns::{PUBLIC_COLUMNS, base_values};
    use crate::gadgets::compact_smt_air::PhysicalRowIndex;
    use rand::{SeedableRng, TryRngCore, rngs::StdRng};

    fn limits() -> ReplayLimits {
        ReplayLimits {
            max_payload_bytes: 2 * 1024 * 1024 * 1024,
            max_work_units: 1_usize << 42,
            max_full_passes: 3,
        }
    }

    struct CountedRng {
        rng: StdRng,
        words: usize,
        fail_after: Option<usize>,
    }
    impl CountedRng {
        fn new() -> Self {
            Self {
                rng: StdRng::from_seed([83; 32]),
                words: 0,
                fail_after: None,
            }
        }
    }
    impl TryRngCore for CountedRng {
        type Error = &'static str;
        fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
            panic!("the existing replay samples only u64 coordinates")
        }
        fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
            if self.fail_after == Some(self.words) {
                return Err("synthetic entropy failure");
            }
            self.words += 1;
            Ok(self.rng.try_next_u64().unwrap())
        }
        fn try_fill_bytes(&mut self, _: &mut [u8]) -> std::result::Result<(), Self::Error> {
            panic!("the existing replay samples only u64 coordinates")
        }
    }
    impl TryCryptoRng for CountedRng {}

    fn fixture() -> OwnedTraceSource {
        let mut source = OwnedTraceSource::zeroed(COLUMN_COUNT * PHYSICAL_ROW_COUNT).unwrap();
        for row in 0..PHYSICAL_ROW_COUNT {
            for (&column, value) in PUBLIC_COLUMNS
                .iter()
                .zip(base_values(PhysicalRowIndex::new(row).unwrap()))
            {
                source.values[column * PHYSICAL_ROW_COUNT + row] = value;
            }
            source.values[row] = (17 * row + 91) as u64;
        }
        source
    }

    #[test]
    fn contiguous_source_transposes_every_column_without_row_reordering() {
        let mut rows = zeroize::Zeroizing::new(vec![SmtRow::zero(); PHYSICAL_ROW_COUNT]);
        for (index, row) in rows.iter_mut().enumerate() {
            row.old_child = core::array::from_fn(|limb| (index * 17 + limb) as u64);
            row.new_child = core::array::from_fn(|limb| (index * 29 + limb + 1) as u64);
            row.sibling = core::array::from_fn(|limb| (index * 31 + limb + 2) as u64);
            row.starting_root = core::array::from_fn(|limb| (index * 37 + limb + 3) as u64);
        }
        let source = OwnedTraceSource::from_rows(&rows).unwrap();
        let columns = source.columns().unwrap();
        for (index, row) in rows.iter().enumerate() {
            for (column, value) in smt_row_cells(row).into_iter().enumerate() {
                assert_eq!(columns[column][index], value);
            }
        }
    }

    #[test]
    fn consumed_source_preserves_every_masked_coefficient_and_entropy_position() {
        let source = fixture();
        let mut reference_rng = CountedRng::new();
        let reference =
            MaskedTraceReplay::new(limits(), &source.columns().unwrap(), &mut reference_rng)
                .unwrap();
        let mut owned_rng = CountedRng::new();
        ERASURES.with(|records| *records.borrow_mut() = Some(Vec::new()));
        let actual = source.into_replay(limits(), &mut owned_rng).unwrap();
        let records = ERASURES.with(|records| records.borrow_mut().take().unwrap());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, COLUMN_COUNT * PHYSICAL_ROW_COUNT);
        assert!(records[0].1 > PHYSICAL_ROW_COUNT);
        assert_eq!(
            records[0].2, 0,
            "physical source must be erased before caller can commit"
        );
        assert_eq!(reference_rng.words, owned_rng.words);
        assert!(owned_rng.words > 500_000);
        for _ in 0..8 {
            assert_eq!(reference_rng.try_next_u64(), owned_rng.try_next_u64());
        }
        assert_eq!(reference.plan(), actual.plan());
        for column in 0..reference.width() {
            for degree in 0..reference.coefficient_extent() {
                assert_eq!(
                    reference.coefficient(column, degree),
                    actual.coefficient(column, degree)
                );
            }
        }
        assert_eq!(reference.quotient_mask(), actual.quotient_mask());
        assert_eq!(reference.composition_mask(), actual.composition_mask());
    }

    #[test]
    fn consumed_source_clears_real_cells_on_shape_error_entropy_error_and_unwind() {
        for mode in 0..3 {
            ERASURES.with(|records| *records.borrow_mut() = Some(Vec::new()));
            let result = std::panic::catch_unwind(|| {
                let mut source = if mode == 1 {
                    fixture()
                } else {
                    OwnedTraceSource::zeroed(17).unwrap()
                };
                source.values[0] = 123;
                if mode == 2 {
                    panic!("synthetic source owner unwind");
                }
                let mut rng = CountedRng::new();
                if mode == 1 {
                    rng.fail_after = Some(7);
                }
                assert!(source.into_replay(limits(), &mut rng).is_err());
                assert_eq!(rng.words, if mode == 1 { 7 } else { 0 });
            });
            assert_eq!(result.is_err(), mode == 2);
            let records = ERASURES.with(|records| records.borrow_mut().take().unwrap());
            assert_eq!(records.len(), 1);
            assert!(records[0].1 > 0);
            assert_eq!(records[0].2, 0);
        }
        assert!(OwnedTraceSource::zeroed(usize::MAX).is_err());
        assert!(OwnedTraceSource::from_rows(&[]).is_err());
    }
}
