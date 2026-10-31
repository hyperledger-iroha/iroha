//! Exact initial paired and selected scalar row-state custody.

use super::*;
use crate::privacy_engines::privacy_outer_hash::{
    PrivacyOuterLastFieldPairV1, PrivacyOuterLastFieldStreamErrorV1,
    paired_stream_workspace_bytes_v1,
};

const ROWS_PER_TASK: usize = 1024;

pub(super) enum RowStreamsV1 {
    InitialPairs(Vec<PrivacyOuterLastFieldPairV1>),
    Scalar(Vec<PrivacyOuterLastFieldStreamV1>),
}

/// Actual backing plus exact inline and conservative construction workspace.
pub(crate) fn initial_payload_v1(rows: usize) -> Result<usize, AggregateStarkErrorV1> {
    if rows == 0 || !rows.is_power_of_two() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let (count, width) = if rows == 1 {
        (1, size_of::<PrivacyOuterLastFieldStreamV1>())
    } else {
        (rows / 2, size_of::<PrivacyOuterLastFieldPairV1>())
    };
    count
        .checked_mul(width)
        .and_then(|n| n.checked_add(size_of::<RowStreamsV1>()))
        .and_then(|n| n.checked_add(paired_stream_workspace_bytes_v1()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

impl RowStreamsV1 {
    pub(super) fn with_capacity_v1(rows: usize) -> Result<Self, AggregateStarkErrorV1> {
        if rows == 0 || !rows.is_power_of_two() {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if rows == 1 {
            let mut values = Vec::new();
            values
                .try_reserve_exact(1)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
            if values.capacity() != 1 {
                return Err(AggregateStarkErrorV1::AllocationFailure);
            }
            Ok(Self::Scalar(values))
        } else {
            let mut values = Vec::new();
            values
                .try_reserve_exact(rows / 2)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
            if values.capacity() != rows / 2 {
                return Err(AggregateStarkErrorV1::AllocationFailure);
            }
            Ok(Self::InitialPairs(values))
        }
    }

    pub(super) fn initialize_v1(
        &mut self,
        prefix: &PrivacyOuterDomainPrefixV1,
        rows: usize,
        counter: u64,
        fields: &[&[u8]],
        value_bytes: usize,
    ) -> Result<(), AggregateStarkErrorV1> {
        if rows == 0
            || !rows.is_power_of_two()
            || self.len() != 0
            || self.capacity() != rows
            || matches!(self, Self::InitialPairs(_)) && rows < 2
            || matches!(self, Self::Scalar(_)) && rows != 1
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        match self {
            Self::InitialPairs(values) => {
                for row in (0..rows).step_by(2) {
                    values.push(
                        prefix
                            .last_field_pair_at_with_counter_v1(
                                [row as u64, (row + 1) as u64],
                                counter,
                                fields,
                                value_bytes,
                            )
                            .map_err(|(_, error)| {
                                map_transparent_error_v1(map_digest_stream_error_v1(error))
                            })?,
                    );
                }
            }
            Self::Scalar(values) => {
                for row in 0..rows {
                    values.push(
                        prefix
                            .last_field_stream_at_with_counter(
                                row as u64,
                                counter,
                                fields,
                                value_bytes,
                            )
                            .map_err(map_digest_stream_error_v1)
                            .map_err(map_transparent_error_v1)?,
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::InitialPairs(values) => 2 * values.len(),
            Self::Scalar(values) => values.len(),
        }
    }
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::InitialPairs(values) => 2 * values.capacity(),
            Self::Scalar(values) => values.capacity(),
        }
    }
    pub(super) fn scalar_mut_v1(
        &mut self,
    ) -> Result<&mut Vec<PrivacyOuterLastFieldStreamV1>, AggregateStarkErrorV1> {
        match self {
            Self::Scalar(values) => Ok(values),
            Self::InitialPairs(_) => Err(AggregateStarkErrorV1::InternalInvariant),
        }
    }

    pub(super) fn absorb_v1<const PACKED_BYTES: usize, C: AsRef<[F]> + Sync>(
        &mut self,
        columns: &[C],
    ) -> Option<(usize, PrivacyOuterLastFieldStreamErrorV1)> {
        let earliest =
            |left: Option<(usize, PrivacyOuterLastFieldStreamErrorV1)>,
             right: Option<(usize, PrivacyOuterLastFieldStreamErrorV1)>| match (
                left, right,
            ) {
                (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
                (left, right) => left.or(right),
            };
        match self {
            Self::Scalar(values) => values
                .par_chunks_mut(ROWS_PER_TASK)
                .enumerate()
                .map(|(chunk, streams)| {
                    let mut packed = zeroize::Zeroizing::new([0_u8; PACKED_BYTES]);
                    for (offset, stream) in streams.iter_mut().enumerate() {
                        let row = chunk * ROWS_PER_TASK + offset;
                        for (column, values) in columns.iter().enumerate() {
                            packed[8 * column..8 * (column + 1)]
                                .copy_from_slice(&values.as_ref()[row].0.to_be_bytes());
                        }
                        if let Err(error) = stream.update(&packed[..columns.len() * 8]) {
                            return Some((row, error));
                        }
                    }
                    None
                })
                .reduce(|| None, earliest),
            Self::InitialPairs(values) => values
                .par_chunks_mut(ROWS_PER_TASK / 2)
                .enumerate()
                .map(|(chunk, pairs)| {
                    let mut packed = zeroize::Zeroizing::new([[0_u8; PACKED_BYTES]; 2]);
                    for (offset, pair) in pairs.iter_mut().enumerate() {
                        let first = chunk * ROWS_PER_TASK + 2 * offset;
                        for lane in 0..2 {
                            for (column, values) in columns.iter().enumerate() {
                                packed[lane][8 * column..8 * (column + 1)].copy_from_slice(
                                    &values.as_ref()[first + lane].0.to_be_bytes(),
                                );
                            }
                        }
                        if let Err((lane, error)) = pair.update_v1([
                            &packed[0][..columns.len() * 8],
                            &packed[1][..columns.len() * 8],
                        ]) {
                            return Some((first + lane, error));
                        }
                    }
                    None
                })
                .reduce(|| None, earliest),
        }
    }

    #[cfg(test)]
    pub(super) fn reserve(&mut self, additional: usize) {
        match self {
            Self::InitialPairs(values) => values.reserve(additional),
            Self::Scalar(values) => values.reserve(additional),
        }
    }
    #[cfg(test)]
    pub(super) fn corrupt_remaining_v1(&mut self, row: usize, bytes: usize) {
        match self {
            Self::InitialPairs(values) => values[row / 2].corrupt_remaining_v1(row % 2, bytes),
            Self::Scalar(values) => {
                values[row].update(&vec![0; bytes]).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_row_storage_has_exact_capacity_and_complete_payload_charge() {
        for rows in [0, 3, 6] {
            assert!(matches!(
                RowStreamsV1::with_capacity_v1(rows),
                Err(AggregateStarkErrorV1::InvalidLayout)
            ));
            assert_eq!(
                initial_payload_v1(rows),
                Err(AggregateStarkErrorV1::InvalidLayout)
            );
        }
        for rows in [1, 2, 4, 1024] {
            let mut owner = RowStreamsV1::with_capacity_v1(rows).unwrap();
            assert_eq!(owner.capacity(), rows);
            assert_eq!(owner.len(), 0);
            assert_eq!(owner.scalar_mut_v1().is_ok(), rows == 1);
            let backing = if rows == 1 {
                size_of::<PrivacyOuterLastFieldStreamV1>()
            } else {
                rows / 2 * size_of::<PrivacyOuterLastFieldPairV1>()
            };
            assert_eq!(
                initial_payload_v1(rows).unwrap(),
                backing + size_of::<RowStreamsV1>() + paired_stream_workspace_bytes_v1()
            );
        }
        let rows = 1 << 22;
        assert!(
            initial_payload_v1(rows).unwrap() < rows * size_of::<PrivacyOuterLastFieldStreamV1>()
        );
        assert_eq!(
            streaming_commitment::initial_payload_bound_v1().unwrap(),
            streaming_commitment::payload_bound_v1().unwrap()
                + crate::privacy_engines::privacy_outer_hash::MAX_PRIVACY_OUTER_BATCH_FRAMES_V1
                    * paired_stream_workspace_bytes_v1()
        );
    }

    #[test]
    fn paired_row_initialization_refuses_reuse_or_wrong_capacity_without_growth() {
        let context = TransparentStarkDigestContextV1::new(
            iroha_data_model::privacy::PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            b"paired-row-initialization",
        );
        let catalog = TransparentStarkDigestContextV1::catalog_v1();
        let domain = context
            .domain_v1(&catalog, b"leaf", b"vector-row-leaf", 0, 0, 0)
            .unwrap();
        let prefix = PrivacyOuterDomainPrefixV1::new(domain).unwrap();
        for rows in [1, 2, 8] {
            let mut owner = RowStreamsV1::with_capacity_v1(rows).unwrap();
            let capacity = owner.capacity();
            for wrong in [0, 3, rows * 2] {
                assert_eq!(
                    owner.initialize_v1(&prefix, wrong, 0, &[], 8),
                    Err(AggregateStarkErrorV1::InvalidLayout)
                );
                assert_eq!((owner.len(), owner.capacity()), (0, capacity));
            }
            owner.initialize_v1(&prefix, rows, 0, &[], 8).unwrap();
            assert_eq!(
                owner.initialize_v1(&prefix, rows, 0, &[], 8),
                Err(AggregateStarkErrorV1::InvalidLayout)
            );
            assert_eq!((owner.len(), owner.capacity()), (rows, capacity));
            assert!(owner.absorb_v1::<64, _>(&[vec![F::ONE; rows]]).is_none());
            let mut enlarged = RowStreamsV1::with_capacity_v1(rows).unwrap();
            match &mut enlarged {
                RowStreamsV1::Scalar(values) => values.reserve(rows + 1),
                RowStreamsV1::InitialPairs(values) => values.reserve(rows + 1),
            }
            let actual = enlarged.capacity();
            assert!(actual > rows);
            assert_eq!(
                enlarged.initialize_v1(&prefix, rows, 0, &[], 8),
                Err(AggregateStarkErrorV1::InvalidLayout)
            );
            assert_eq!((enlarged.len(), enlarged.capacity()), (0, actual));
        }
    }

    #[test]
    fn paired_initial_rows_preserve_roots_frontiers_partials_and_all_tile_boundaries() {
        let context = TransparentStarkDigestContextV1::new(
            iroha_data_model::privacy::PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            b"paired-row-reference",
        );
        for rows in [1, 2, 4, 4096, 8192] {
            let mut indices = vec![0, rows - 1];
            indices.dedup();
            for width in [1, 7, 8, 9, 16, 17] {
                let columns = (0..width)
                    .map(|column| {
                        (0..rows)
                            .map(|row| F((column * 17 + row * 31 + 1) as u64))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let tree = row_tree_v1(context, b"paired-leaf", b"paired-node", 0, &columns, rows)
                    .unwrap();
                for batch in [8, 16] {
                    let mut owner = StreamingRowCommitmentV1::new(
                        context,
                        b"paired-leaf",
                        b"paired-node",
                        0,
                        rows,
                        width,
                        &indices,
                    )
                    .unwrap();
                    for columns in columns.chunks(batch) {
                        if batch == 8 {
                            owner.absorb_columns_v1(columns).unwrap();
                        } else {
                            owner.absorb_initial_columns_v1(columns).unwrap();
                        }
                    }
                    let actual = owner.finish().unwrap();
                    assert_eq!(actual.commitment.root, tree.root());
                    assert_eq!(
                        actual.commitment.frontier,
                        canonical_multiproof_frontier_v1(&tree, rows, &indices).unwrap()
                    );
                    for &row in &indices {
                        assert_eq!(
                            actual.opened_rows[&row],
                            columns.iter().map(|column| column[row]).collect::<Vec<_>>()
                        );
                    }
                }
                if rows >= 2 {
                    let mut owner = StreamingRowCommitmentV1::new(
                        context,
                        b"paired-leaf",
                        b"paired-node",
                        0,
                        rows,
                        width,
                        &[],
                    )
                    .unwrap();
                    for columns in columns.chunks(16) {
                        owner.absorb_initial_columns_v1(columns).unwrap();
                    }
                    let (actual, _cut) = owner.finish_retaining_cut_v1().unwrap();
                    assert_eq!(actual.commitment.root, tree.root());
                }
            }
        }
    }
}
