//! Private RFC source storage selected solely by the public row family.
//!
//! Proving still consumes the full 285-column rows. Construction rejects a
//! nonzero omitted field instead of silently discarding a semantic operand.
//! Original and replacement capacities overlap within the existing source
//! scratch allowance; both allocations have clearing owners throughout.

use super::*;

/// Retained prefix width for each verifier-fixed family. Decimal rows carry
/// GENERALIZED near the end of the calendar columns and retain the full row.
const fn prefix_width_v1(family: usize) -> usize {
    if family == ZkX509Rfc5280StarkFamilyV1::SourceNode as usize {
        name_policy::NODE_PREFIX_END // Time and exact Name-role classifications.
    } else if family == ZkX509Rfc5280StarkFamilyV1::Grammar as usize {
        BASE_ORDINAL_EQUAL_CONTINUE + 1
    } else if family == ZkX509Rfc5280StarkFamilyV1::Calendar as usize
        || family == ZkX509Rfc5280StarkFamilyV1::Decimal as usize
    {
        ZK_X509_RFC5280_STARK_BASE_WIDTH_V1
    } else {
        BASE_CERT2_ACTIVE + 1
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct FamilyRowsV1 {
    width: usize,
    cells: Vec<F>,
}

impl core::fmt::Debug for FamilyRowsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("FamilyRowsV1 { <private rows redacted> }")
    }
}

impl Drop for FamilyRowsV1 {
    fn drop(&mut self) {
        self.clear();
    }
}

impl FamilyRowsV1 {
    fn empty_v1(family: usize) -> Self {
        Self {
            width: prefix_width_v1(family),
            cells: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.cells.len() / self.width
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub(super) fn allocated_heap_bytes_v1(&self) -> usize {
        super::super::allocation_payload::vector_v1(&self.cells)
    }

    pub(super) fn clear(&mut self) {
        zeroize_fields_v1(&mut self.cells);
        self.cells.clear();
    }

    pub(super) fn get(&self, row: usize) -> Option<ZkX509Rfc5280StarkBaseRowV1> {
        let start = row.checked_mul(self.width)?;
        let end = start.checked_add(self.width)?;
        let stored = self.cells.get(start..end)?;
        let mut result = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
        result[..self.width].copy_from_slice(stored);
        Some(result)
    }

    #[cfg(test)]
    pub(super) fn iter(&self) -> core::slice::ChunksExact<'_, F> {
        self.cells.chunks_exact(self.width)
    }

    #[cfg(test)]
    pub(super) fn iter_mut(&mut self) -> core::slice::ChunksExactMut<'_, F> {
        self.cells.chunks_exact_mut(self.width)
    }

    #[cfg(test)]
    pub(super) fn initialized_cells_v1(&self) -> usize {
        self.cells.len()
    }
}

fn compact_family_v1(
    family: usize,
    rows: &[ZkX509Rfc5280StarkBaseRowV1],
    other_capacity_bytes: usize,
    scratch_limit: usize,
) -> Result<FamilyRowsV1, ZkX509Rfc5280StarkErrorV1> {
    if family >= FAMILY_COUNT_V1 || rows.len() > ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    let width = prefix_width_v1(family);
    let cells = rows
        .len()
        .checked_mul(width)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    let required_bytes = cells
        .checked_mul(core::mem::size_of::<F>())
        .and_then(|bytes| bytes.checked_add(other_capacity_bytes))
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    if required_bytes > scratch_limit {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    let mut compact = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    compact
        .try_reserve_exact(cells)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    let actual_bytes = compact
        .capacity()
        .checked_mul(core::mem::size_of::<F>())
        .and_then(|bytes| bytes.checked_add(other_capacity_bytes))
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    if actual_bytes > scratch_limit {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    for row in rows {
        if row[width..].iter().any(|value| *value != F::ZERO) {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        compact.extend_from_slice(&row[..width]);
    }
    Ok(FamilyRowsV1 {
        width,
        cells: compact.into_vec(),
    })
}

pub(super) fn compact_families_v1(
    rows: [PrivateTableV1<ZkX509Rfc5280StarkBaseRowV1>; FAMILY_COUNT_V1],
    retained_material_bytes: usize,
) -> Result<[FamilyRowsV1; FAMILY_COUNT_V1], ZkX509Rfc5280StarkErrorV1> {
    compact_families_with_limit_v1(
        rows,
        retained_material_bytes,
        super::super::allocation_payload::MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1,
    )
}

fn compact_families_with_limit_v1(
    rows: [PrivateTableV1<ZkX509Rfc5280StarkBaseRowV1>; FAMILY_COUNT_V1],
    retained_material_bytes: usize,
    scratch_limit: usize,
) -> Result<[FamilyRowsV1; FAMILY_COUNT_V1], ZkX509Rfc5280StarkErrorV1> {
    use super::super::allocation_payload::vector_v1;

    let mut original_bytes = rows.iter().try_fold(0usize, |sum, family| {
        sum.checked_add(vector_v1(family))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)
    })?;
    let mut retained_bytes = retained_material_bytes;
    let mut compact = core::array::from_fn(FamilyRowsV1::empty_v1);
    for (family, original) in rows.into_iter().enumerate() {
        let old_capacity = vector_v1(&original);
        compact[family] = compact_family_v1(
            family,
            &original,
            original_bytes
                .checked_add(retained_bytes)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
            scratch_limit,
        )?;
        retained_bytes = retained_bytes
            .checked_add(compact[family].allocated_heap_bytes_v1())
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        // Release the old guarded allocation before subtracting its charge.
        drop(original);
        original_bytes = original_bytes
            .checked_sub(old_capacity)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    }
    Ok(compact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::private_table::inspection;

    fn rows_v1(family: usize, count: usize) -> PrivateTableV1<ZkX509Rfc5280StarkBaseRowV1> {
        let mut rows = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1);
        for row in 0..count {
            let mut values = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
            for (column, value) in values[..prefix_width_v1(family)].iter_mut().enumerate() {
                *value = F((row * 313 + column + 1) as u64);
            }
            rows.push(values);
        }
        rows
    }

    #[test]
    fn every_public_family_reconstructs_every_cell_and_checks_bounds() {
        for family in 0..FAMILY_COUNT_V1 {
            let original = rows_v1(family, 3);
            let compact = compact_family_v1(family, &original, 0, usize::MAX).unwrap();
            assert_eq!(compact.len(), original.len());
            assert!(!compact.is_empty());
            for (row, expected) in original.iter().enumerate() {
                assert_eq!(compact.get(row).as_ref(), Some(expected));
            }
            assert_eq!(compact.get(original.len()), None);
            assert_eq!(compact.get(usize::MAX), None);
            assert_eq!(compact.iter().len(), original.len());
            assert_eq!(compact.initialized_cells_v1(), 3 * prefix_width_v1(family));
            assert!(format!("{compact:?}").contains("redacted"));
            assert!(compact.allocated_heap_bytes_v1() >= compact.cells.len() * 8);
        }
        assert!(compact_family_v1(FAMILY_COUNT_V1, &[], 0, usize::MAX).is_err());
        assert_eq!(
            prefix_width_v1(ZkX509Rfc5280StarkFamilyV1::SourceByte as usize),
            66
        );
        assert_eq!(
            prefix_width_v1(ZkX509Rfc5280StarkFamilyV1::SourceNode as usize),
            134
        );
        assert_eq!(
            prefix_width_v1(ZkX509Rfc5280StarkFamilyV1::Grammar as usize),
            102
        );
    }

    #[test]
    fn omitted_nonzero_cells_reject_and_erase_partial_compaction() {
        for family in 0..FAMILY_COUNT_V1 {
            for column in prefix_width_v1(family)..ZK_X509_RFC5280_STARK_BASE_WIDTH_V1 {
                let (result, observations) = inspection::observe_v1(|| {
                    let mut original = rows_v1(family, 3);
                    original[2][column] = F(91);
                    compact_family_v1(family, &original, 0, usize::MAX)
                });
                assert_eq!(result, Err(ZkX509Rfc5280StarkErrorV1::Source));
                assert!(
                    observations
                        .iter()
                        .any(|row| row.cells == 2 * prefix_width_v1(family))
                );
                assert!(observations.iter().any(|row| row.nonzero_before > 0));
                assert!(observations.iter().all(|row| row.nonzero_after == 0));
            }
        }
    }

    #[test]
    fn overlap_admission_counts_capacity_and_never_wraps() {
        let original = rows_v1(0, 2);
        let payload = 2 * prefix_width_v1(0) * core::mem::size_of::<F>();
        assert!(compact_family_v1(0, &original, 17, payload + 17).is_ok());
        assert_eq!(
            compact_family_v1(0, &original, 18, payload + 17),
            Err(ZkX509Rfc5280StarkErrorV1::Resource)
        );
        assert_eq!(
            compact_family_v1(0, &original, usize::MAX, usize::MAX),
            Err(ZkX509Rfc5280StarkErrorV1::Resource)
        );
        let mut families = core::array::from_fn(|family| rows_v1(family, 0));
        // Spare initialized storage is not needed for the capacity charge.
        families[0].try_reserve_exact(8192).unwrap();
        let full_bytes = super::super::super::allocation_payload::vector_v1(&families[0]);
        assert!(
            full_bytes > families[0].len() * core::mem::size_of::<ZkX509Rfc5280StarkBaseRowV1>()
        );
        assert_eq!(
            compact_families_with_limit_v1(families, 0, full_bytes - 1),
            Err(ZkX509Rfc5280StarkErrorV1::Resource)
        );
        let empty = core::array::from_fn(|family| rows_v1(family, 0));
        let result = compact_families_with_limit_v1(empty, 0, 0).unwrap();
        assert!(result.iter().all(FamilyRowsV1::is_empty));
    }

    #[test]
    fn original_partial_output_clone_error_and_unwind_owners_clear() {
        for unwind in [false, true] {
            let (result, observations) = inspection::observe_v1(|| {
                std::panic::catch_unwind(|| {
                    let original = core::array::from_fn(|family| rows_v1(family, 2));
                    let mut compact = compact_families_v1(original, 0).unwrap();
                    let retained = compact.clone();
                    compact[0].iter_mut().next().unwrap()[0] = F(41);
                    assert_ne!(compact, retained);
                    compact[0].clear();
                    assert!(compact[0].is_empty());
                    if unwind {
                        panic!("injected private family-owner unwind");
                    }
                    Err::<(), _>(ZkX509Rfc5280StarkErrorV1::Source)
                })
            });
            assert_eq!(result.is_err(), unwind);
            assert!(
                observations
                    .iter()
                    .any(|row| row.cells == 2 * ZK_X509_RFC5280_STARK_BASE_WIDTH_V1)
            );
            assert!(
                observations
                    .iter()
                    .any(|row| row.cells == 2 * prefix_width_v1(0))
            );
            assert!(observations.iter().any(|row| row.nonzero_before > 0));
            assert!(observations.iter().all(|row| row.nonzero_after == 0));
        }
    }
}
