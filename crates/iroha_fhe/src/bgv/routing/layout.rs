//! Immutable public coefficient support shared by routing plans and owners.

use std::{mem::size_of, sync::Arc};

use super::SlotLinearError;

/// An increasing arithmetic progression of public logical columns in one row.
///
/// Input columns are bank-major scalar slots; the final column is the bias.
/// The support is public. It must never be derived from private nonzero entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotLinearSegment {
    start: usize,
    count: usize,
    stride: usize,
    offset: usize,
}

impl SlotLinearSegment {
    /// Describe a public progression; layout construction validates all fields.
    #[must_use]
    pub const fn new(start: usize, count: usize, stride: usize) -> Self {
        Self {
            start,
            count,
            stride,
            offset: 0,
        }
    }
    /// First logical column in this progression.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }
    /// Number of retained coefficients in this progression.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }
    /// Positive distance between successive columns.
    #[must_use]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LayoutData {
    slots: usize,
    columns: usize,
    coefficient_count: usize,
    metadata_bytes: usize,
    row_offsets: Box<[usize]>,
    segments: Box<[SlotLinearSegment]>,
    // Bank-pair-major bits, indexed by odd unit h=2*j+1 modulo2S. These are
    // public support unions, never a scan of private coefficient values.
    active_diagonals: Box<[u64]>,
}

/// Immutable public row support for one packed linear map geometry.
///
/// Clones share one allocation graph. Rows are output-bank-major scalar slots.
/// Within a row, segments have strictly separated numeric spans; coefficients
/// are packed in increasing row/column order. Unrepresented positions are
/// structural zeros, including biases when their columns are absent. Runtime
/// lookup addresses and branching depend exclusively on this public layout.
#[derive(Clone, Debug)]
pub struct SlotLinearLayout<const INPUTS: usize, const OUTPUTS: usize> {
    data: Arc<LayoutData>,
}

impl<const I: usize, const O: usize> PartialEq for SlotLinearLayout<I, O> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) || self.data == other.data
    }
}
impl<const I: usize, const O: usize> Eq for SlotLinearLayout<I, O> {}

impl<const I: usize, const O: usize> SlotLinearLayout<I, O> {
    fn geometry(slots: usize) -> Result<(usize, usize), SlotLinearError> {
        if I == 0 || O == 0 || slots == 0 {
            return Err(SlotLinearError::EmptyDimensions);
        }
        if slots < 2 || !slots.is_power_of_two() {
            return Err(SlotLinearError::InvalidLayout);
        }
        slots
            .checked_mul(2)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let rows = O
            .checked_mul(slots)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let columns = I
            .checked_mul(slots)
            .and_then(|x| x.checked_add(1))
            .ok_or(SlotLinearError::GeometryOverflow)?;
        rows.checked_mul(columns)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        rows.checked_add(1)
            .ok_or(SlotLinearError::GeometryOverflow)?;
        Ok((rows, columns))
    }

    fn activity_words(slots: usize) -> Result<usize, SlotLinearError> {
        I.checked_mul(O)
            .and_then(|x| x.checked_mul(slots.div_ceil(64)))
            .ok_or(SlotLinearError::GeometryOverflow)
    }

    /// Exact resident metadata payload for the given public descriptor counts.
    ///
    /// Counts row-offset/segment elements, public diagonal bitsets, their
    /// boxed-slice fields, the immutable data object and two Arc reference counts. Allocator headers,
    /// alignment padding outside Rust objects, caller-owned construction inputs
    /// and the small layout handle embedded in each map/plan are excluded.
    ///
    /// # Errors
    /// Refuses empty/overflowing geometry or byte arithmetic overflow.
    pub fn metadata_bytes_for(slots: usize, segments: usize) -> Result<usize, SlotLinearError> {
        let (rows, _) = Self::geometry(slots)?;
        let activity_bytes = Self::activity_words(slots)?
            .checked_mul(size_of::<u64>())
            .ok_or(SlotLinearError::GeometryOverflow)?;
        (rows + 1)
            .checked_mul(size_of::<usize>())
            .and_then(|x| {
                segments
                    .checked_mul(size_of::<SlotLinearSegment>())?
                    .checked_add(x)
            })
            .and_then(|x| x.checked_add(size_of::<LayoutData>() + 2 * size_of::<usize>()))
            .and_then(|x| x.checked_add(activity_bytes))
            .ok_or(SlotLinearError::GeometryOverflow)
    }

    /// Validate and own a complete public row descriptor under a metadata ceiling.
    ///
    /// `row_offsets` has `OUTPUTS*slots+1` entries indexing `segments`; empty
    /// rows are allowed. Segment count/stride must be positive, checked last
    /// columns must be in range, and each last column precedes the next start.
    /// The row partition must start at zero and consume every segment exactly.
    /// No coefficient allocation or private data is involved. Box shrinking
    /// and the Arc allocation use the infallible allocator; this is not a claim
    /// of complete allocation-failure recovery.
    ///
    /// # Errors
    /// Refuses invalid public ordering, overlap, stride, geometry or budget.
    pub fn new(
        slots: usize,
        row_offsets: Vec<usize>,
        mut segments: Vec<SlotLinearSegment>,
        maximum_metadata_bytes: usize,
    ) -> Result<Self, SlotLinearError> {
        let (rows, columns) = Self::geometry(slots)?;
        let metadata_bytes = Self::metadata_bytes_for(slots, segments.len())?;
        if metadata_bytes > maximum_metadata_bytes {
            return Err(SlotLinearError::LayoutBudget {
                required: metadata_bytes,
                budget: maximum_metadata_bytes,
            });
        }
        if row_offsets.len() != rows + 1
            || row_offsets.first() != Some(&0)
            || row_offsets.last() != Some(&segments.len())
        {
            return Err(SlotLinearError::InvalidLayout);
        }
        let mut coefficient_count = 0_usize;
        for pair in row_offsets.windows(2) {
            if pair[0] > pair[1] || pair[1] > segments.len() {
                return Err(SlotLinearError::InvalidLayout);
            }
            let mut previous = None;
            for segment in &mut segments[pair[0]..pair[1]] {
                if segment.count == 0 || segment.stride == 0 {
                    return Err(SlotLinearError::InvalidLayout);
                }
                let last = (segment.count - 1)
                    .checked_mul(segment.stride)
                    .and_then(|x| segment.start.checked_add(x))
                    .ok_or(SlotLinearError::GeometryOverflow)?;
                if last >= columns || previous.is_some_and(|x| x >= segment.start) {
                    return Err(SlotLinearError::InvalidLayout);
                }
                segment.offset = coefficient_count;
                coefficient_count = coefficient_count
                    .checked_add(segment.count)
                    .ok_or(SlotLinearError::GeometryOverflow)?;
                previous = Some(last);
            }
        }
        coefficient_count
            .checked_mul(size_of::<u32>())
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let mut active_diagonals = Vec::new();
        active_diagonals
            .try_reserve_exact(Self::activity_words(slots)?)
            .map_err(|_| SlotLinearError::Allocation)?;
        active_diagonals.resize(Self::activity_words(slots)?, 0_u64);
        let order = u64::try_from(2 * slots).map_err(|_| SlotLinearError::GeometryOverflow)?;
        let words = slots.div_ceil(64);
        for row in 0..rows {
            // Euler inverse in the power-of-two odd-unit group. SlotEncoding's
            // actual lifted Galois action reduces to this same action modulo2S.
            let inverse = crate::modular::mod_pow_u64(
                (2 * (row % slots) + 1) as u64,
                slots as u64 - 1,
                order,
            );
            for segment in &segments[row_offsets[row]..row_offsets[row + 1]] {
                for offset in 0..segment.count {
                    let column = segment.start + offset * segment.stride;
                    if column == columns - 1 {
                        continue;
                    } // Bias is not a matrix diagonal.
                    let unit = (((2 * (column % slots) + 1) as u128 * u128::from(inverse))
                        % u128::from(order)) as usize;
                    let bit = (unit - 1) / 2;
                    let pair = row / slots * I + column / slots;
                    active_diagonals[pair * words + bit / 64] |= 1_u64 << (bit % 64);
                }
            }
        }
        Ok(Self {
            data: Arc::new(LayoutData {
                slots,
                columns,
                coefficient_count,
                metadata_bytes,
                row_offsets: row_offsets.into_boxed_slice(),
                segments: segments.into_boxed_slice(),
                active_diagonals: active_diagonals.into_boxed_slice(),
            }),
        })
    }

    /// Construct full public row support, including every bias, in the same owner.
    ///
    /// # Errors
    /// Refuses invalid geometry, exceeded metadata budget or reservation failure.
    pub fn dense(slots: usize, maximum_metadata_bytes: usize) -> Result<Self, SlotLinearError> {
        let (rows, columns) = Self::geometry(slots)?;
        rows.checked_mul(columns)
            .and_then(|x| x.checked_mul(size_of::<u32>()))
            .ok_or(SlotLinearError::GeometryOverflow)?;
        let required = Self::metadata_bytes_for(slots, rows)?;
        if required > maximum_metadata_bytes {
            return Err(SlotLinearError::LayoutBudget {
                required,
                budget: maximum_metadata_bytes,
            });
        }
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(rows + 1)
            .map_err(|_| SlotLinearError::Allocation)?;
        offsets.extend(0..=rows);
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(rows)
            .map_err(|_| SlotLinearError::Allocation)?;
        segments.resize(rows, SlotLinearSegment::new(0, columns, 1));
        Self::new(slots, offsets, segments, maximum_metadata_bytes)
    }

    /// Number of scalar slots in each bank.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.data.slots
    }
    /// Number of logical output rows.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.data.row_offsets.len() - 1
    }
    /// Number of logical input columns including the final bias column.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.data.columns
    }
    /// Exact retained private scalar count, including supported biases.
    #[must_use]
    pub fn coefficient_count(&self) -> usize {
        self.data.coefficient_count
    }
    /// Exact resident shared metadata payload, with exclusions documented above.
    #[must_use]
    pub fn metadata_bytes(&self) -> usize {
        self.data.metadata_bytes
    }
    /// Public segments for a logical row, or `None` when the row is out of range.
    #[must_use]
    pub fn row_segments(&self, row: usize) -> Option<&[SlotLinearSegment]> {
        let start = *self.data.row_offsets.get(row)?;
        let end = *self.data.row_offsets.get(row.checked_add(1)?)?;
        Some(&self.data.segments[start..end])
    }
    /// Whether the public support contains a coefficient on this group diagonal.
    ///
    /// `element` is the actual odd Galois element; its scalar-slot action is
    /// reduced modulo2S. Invalid banks/even elements return false. No private
    /// coefficient is read. Bits use the canonical odd-unit order1,3,...,2S-1.
    #[must_use]
    pub fn diagonal_active(&self, output_bank: usize, input_bank: usize, element: u32) -> bool {
        if output_bank >= O || input_bank >= I || element.is_multiple_of(2) {
            return false;
        }
        let bit = (element as usize % (2 * self.slots()) - 1) / 2;
        let words = self.slots().div_ceil(64);
        let word = self.data.active_diagonals[(output_bank * I + input_bank) * words + bit / 64];
        word & (1_u64 << (bit % 64)) != 0
    }

    /// Public packed index, or `None` for a structural zero/out-of-range position.
    #[must_use]
    pub fn coefficient_index(&self, row: usize, column: usize) -> Option<usize> {
        for segment in self.row_segments(row)? {
            if column < segment.start {
                return None;
            }
            let distance = column - segment.start;
            if distance.is_multiple_of(segment.stride) && distance / segment.stride < segment.count
            {
                return Some(segment.offset + distance / segment.stride);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_segments_preserve_canonical_coordinates_and_shared_equality() {
        let segments = vec![
            SlotLinearSegment::new(0, 3, 2),
            SlotLinearSegment::new(8, 1, 1),
            SlotLinearSegment::new(1, 2, 3),
        ];
        let bytes = SlotLinearLayout::<2, 1>::metadata_bytes_for(4, 3).unwrap();
        let layout =
            SlotLinearLayout::<2, 1>::new(4, vec![0, 2, 2, 3, 3], segments.clone(), bytes).unwrap();
        assert_eq!(
            (
                layout.slots(),
                layout.rows(),
                layout.columns(),
                layout.coefficient_count(),
                layout.metadata_bytes()
            ),
            (4, 4, 9, 6, bytes)
        );
        let shared = layout.clone();
        assert_eq!(layout, shared);
        assert!(Arc::ptr_eq(&layout.data, &shared.data));
        assert_eq!(
            layout,
            SlotLinearLayout::new(4, vec![0, 2, 2, 3, 3], segments, bytes).unwrap()
        );
        assert_eq!(
            layout
                .row_segments(0)
                .unwrap()
                .iter()
                .map(|s| (s.start(), s.count(), s.stride()))
                .collect::<Vec<_>>(),
            vec![(0, 3, 2), (8, 1, 1)]
        );
        for row in 0..=4 {
            for column in 0..=9 {
                let expected = match (row, column) {
                    (0, 0) => Some(0),
                    (0, 2) => Some(1),
                    (0, 4) => Some(2),
                    (0, 8) => Some(3),
                    (2, 1) => Some(4),
                    (2, 4) => Some(5),
                    _ => None,
                };
                assert_eq!(layout.coefficient_index(row, column), expected);
            }
        }
        assert!(layout.row_segments(usize::MAX).is_none());
        let dense = SlotLinearLayout::<2, 1>::dense(4, 4096).unwrap();
        assert_eq!(dense.coefficient_count(), 36);
        for row in 0..4 {
            for column in 0..9 {
                assert_eq!(dense.coefficient_index(row, column), Some(row * 9 + column));
            }
        }
        assert_ne!(layout, dense);
        let empty = SlotLinearLayout::<2, 1>::new(4, vec![0; 5], vec![], 4096).unwrap();
        assert_eq!(empty.coefficient_count(), 0);
        assert_eq!(empty.coefficient_index(0, 0), None);
    }

    #[test]
    fn public_diagonal_bits_equal_direct_slot_permutation_for_every_supported_geometry() {
        for slots in [2_usize, 4, 8, 16, 128] {
            let mut offsets = vec![0];
            let mut segments = Vec::new();
            for row in 0..3 * slots {
                for column in 0..2 * slots + 1 {
                    if (row * 7 + column * 11) % 13 < 3 {
                        segments.push(SlotLinearSegment::new(column, 1, 1));
                    }
                }
                offsets.push(segments.len());
            }
            let bytes =
                SlotLinearLayout::<2, 3>::metadata_bytes_for(slots, segments.len()).unwrap();
            let layout = SlotLinearLayout::<2, 3>::new(slots, offsets, segments, bytes).unwrap();
            for output in 0..3 {
                for input in 0..2 {
                    for odd in (1..2 * slots).step_by(2) {
                        let expected = (0..slots).any(|row| {
                            let column = ((odd * (2 * row + 1) % (2 * slots)) - 1) / 2;
                            layout
                                .coefficient_index(output * slots + row, input * slots + column)
                                .is_some()
                        });
                        assert_eq!(layout.diagonal_active(output, input, odd as u32), expected);
                        assert_eq!(
                            layout.diagonal_active(output, input, (odd + 2 * slots) as u32),
                            expected
                        );
                    }
                }
            }
            assert!(!layout.diagonal_active(3, 0, 1));
            assert!(!layout.diagonal_active(0, 2, 1));
            assert!(!layout.diagonal_active(0, 0, 2));
        }
        for slots in [1, 3, 6] {
            assert_eq!(
                SlotLinearLayout::<1, 1>::dense(slots, 4096).unwrap_err(),
                SlotLinearError::InvalidLayout
            );
        }
    }

    #[test]
    fn public_geometry_order_stride_overflow_and_exact_metadata_refusals() {
        type L = SlotLinearLayout<2, 1>;
        let valid = SlotLinearSegment::new(0, 1, 1);
        for offsets in [
            vec![],
            vec![1, 1, 1, 1, 1],
            vec![0, 0, 0, 0, 0],
            vec![0, 2, 1, 1, 1],
            vec![0, 1, 0, 1, 1],
        ] {
            assert_eq!(
                L::new(4, offsets, vec![valid], 4096).unwrap_err(),
                SlotLinearError::InvalidLayout
            );
        }
        for invalid in [
            SlotLinearSegment::new(0, 0, 1),
            SlotLinearSegment::new(0, 1, 0),
            SlotLinearSegment::new(9, 1, 1),
        ] {
            assert_eq!(
                L::new(4, vec![0, 1, 1, 1, 1], vec![invalid], 4096).unwrap_err(),
                SlotLinearError::InvalidLayout
            );
        }
        for invalid in [
            vec![
                SlotLinearSegment::new(0, 3, 2),
                SlotLinearSegment::new(3, 1, 1),
            ],
            vec![
                SlotLinearSegment::new(3, 1, 1),
                SlotLinearSegment::new(1, 1, 1),
            ],
        ] {
            assert_eq!(
                L::new(4, vec![0, 2, 2, 2, 2], invalid, 4096).unwrap_err(),
                SlotLinearError::InvalidLayout
            );
        }
        assert_eq!(
            L::new(
                4,
                vec![0, 1, 1, 1, 1],
                vec![SlotLinearSegment::new(1, usize::MAX, 2)],
                4096
            )
            .unwrap_err(),
            SlotLinearError::GeometryOverflow
        );
        assert_eq!(
            SlotLinearLayout::<0, 1>::dense(4, 4096).unwrap_err(),
            SlotLinearError::EmptyDimensions
        );
        assert_eq!(
            SlotLinearLayout::<1, 0>::dense(4, 4096).unwrap_err(),
            SlotLinearError::EmptyDimensions
        );
        assert_eq!(
            L::dense(0, 4096).unwrap_err(),
            SlotLinearError::EmptyDimensions
        );
        assert_eq!(
            SlotLinearLayout::<1, { usize::MAX }>::dense(4, usize::MAX).unwrap_err(),
            SlotLinearError::GeometryOverflow
        );
        assert_eq!(
            L::metadata_bytes_for(4, usize::MAX).unwrap_err(),
            SlotLinearError::GeometryOverflow
        );
        assert_eq!(
            SlotLinearLayout::<{ usize::MAX / 4 }, 2>::metadata_bytes_for(2, 0).unwrap_err(),
            SlotLinearError::GeometryOverflow
        );
        let required = L::metadata_bytes_for(4, 1).unwrap();
        assert_eq!(
            L::new(4, vec![0, 1, 1, 1, 1], vec![valid], required - 1).unwrap_err(),
            SlotLinearError::LayoutBudget {
                required,
                budget: required - 1
            }
        );
        let required = L::metadata_bytes_for(4, 4).unwrap();
        assert_eq!(
            L::dense(4, required - 1).unwrap_err(),
            SlotLinearError::LayoutBudget {
                required,
                budget: required - 1
            }
        );
    }
}
