//! Mixed original native rows and retained RFC coefficients under one replay envelope.

use super::*;

// Source output plus current split receiver, both bounded by the original eight
// columns. All field values retain their original native or retained owners.
pub(in super::super::super) struct MainDeepReplayBatchV1<'a> {
    pub(in super::super::super) native: Vec<ZeroizingMainTraceColumnV1>,
    pub(in super::super::super) retained: [Option<&'a [F]>; BATCH],
    pub(in super::super::super) native_indices: [usize; BATCH],
    pub(in super::super::super) length: usize,
}
impl<'a> MainDeepReplayBatchV1<'a> {
    pub(in super::super::super) fn gather_v1(
        columns: core::ops::Range<usize>,
        mut run: impl FnMut(usize, usize) -> Result<(usize, bool), ZkX509StarkErrorV1>,
        mut source: impl FnMut(
            core::ops::Range<usize>,
        ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
        mut retained: impl FnMut(usize) -> Result<&'a [F], ZkX509StarkErrorV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let width = columns
            .end
            .checked_sub(columns.start)
            .filter(|&n| n > 0 && n <= BATCH)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let mut batch = Self {
            native: Vec::new(),
            retained: [None; BATCH],
            native_indices: [0; BATCH],
            length: width,
        };
        batch
            .native
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if batch.native.capacity() != width {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let mut first = columns.start;
        while first < columns.end {
            let (end, cached) = run(first, columns.end)?;
            if end <= first || end > columns.end {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            if cached {
                for column in first..end {
                    batch.retained[column - columns.start] = Some(retained(column)?);
                }
            } else {
                let native = source(first..end)?;
                if native.len() != end - first || native.capacity() != end - first {
                    return Err(ZkX509StarkErrorV1::ProfileMismatch);
                }
                for (column, values) in (first..end).zip(native) {
                    batch.native_indices[column - columns.start] = batch.native.len();
                    batch.native.push(values);
                }
            }
            first = end;
        }
        Ok(batch)
    }
    pub(in super::super::super) fn input_v1<'b>(
        &'b self,
        offset: usize,
        mask: &'b [F],
    ) -> Result<MainMixedInputV1<'b>, ZkX509StarkErrorV1> {
        if offset >= self.length {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        match self.retained[offset] {
            Some(coefficients) => Ok(MainMixedInputV1::Retained(coefficients)),
            None => Ok(MainMixedInputV1::Native(
                self.native
                    .get(self.native_indices[offset])
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
                mask,
            )),
        }
    }
}

#[derive(Clone, Copy)]
pub(in super::super::super) enum MainMixedInputV1<'a> {
    Native(&'a [F], &'a [F]),
    Retained(&'a [F]),
}
pub(in super::super::super) type MainMixedColumnV1<'a> = (MainMixedInputV1<'a>, [E; 2], [E; 2]);

pub(in super::super::super) struct MainMixedDeepPointsV1 {
    native: MainNativeDeepPointsV1,
    retained: Option<MainDeepPointPowersV1>,
    allowance: usize,
}

impl MainMixedDeepPointsV1 {
    const METADATA: usize = 32 * 1024;

    fn forecast_v1(native_log: u8, retained: bool) -> Result<usize, ZkX509StarkErrorV1> {
        if native_log == 0 || native_log > ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let n = 1usize << native_log;
        let length = checked_sum(&[n, MASK_LENGTH])?;
        let metadata = checked_sum(&[
            core::mem::size_of::<Self>(),
            SECURITY_LANES * core::mem::size_of::<MainMixedDeepQuotientV1>(),
            core::mem::size_of::<Vec<MainMixedDeepQuotientV1>>(),
            core::mem::size_of::<MainDeepReplayBatchV1<'_>>(),
            2 * BATCH * core::mem::size_of::<ZeroizingMainTraceColumnV1>(),
            2 * core::mem::size_of::<Vec<ZeroizingMainTraceColumnV1>>(),
            2 * core::mem::size_of::<std::vec::IntoIter<ZeroizingMainTraceColumnV1>>(),
            core::mem::size_of::<[MainMixedColumnV1<'_>; BATCH]>(),
            core::mem::size_of::<[NativeColumnV1<'_>; BATCH]>(),
            core::mem::size_of::<MainDeepStackValuesV1<{ 2 * BATCH }>>(),
            core::mem::size_of::<MainDeepStackValuesV1<1>>(),
            BATCH * core::mem::size_of::<[E; 16]>(),
            core::mem::size_of::<ExtensionColumn>(), // native setup prefix
            2048, // public counters and bounded callback/reference metadata
        ])?;
        if metadata > Self::METADATA {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let setup = checked_product(&[2 * n + 2 * MASK_LENGTH, core::mem::size_of::<E>()])?;
        let retained_cells = if retained {
            (2 + 2 * SECURITY_LANES) * length
        } else {
            0
        };
        let steady = checked_sum(&[
            checked_product(&[
                n + 2 * MASK_LENGTH + 2 * SECURITY_LANES * length + retained_cells,
                core::mem::size_of::<E>(),
            ])?,
            checked_product(&[BATCH, n, core::mem::size_of::<F>()])?,
        ])?;
        checked_sum(&[setup.max(steady), Self::METADATA])
    }

    pub(in super::super::super) fn new_v1(
        native_log: u8,
        point: E,
        retained: bool,
        allowance: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        // This is the single original replay_batch reservation. Native-only
        // checks still enforce their narrower internal owner cap separately.
        if Self::forecast_v1(native_log, retained)? > allowance {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let native = MainNativeDeepPointsV1::new_v1(native_log, point)?;
        let coefficients = native.native_rows + MASK_LENGTH;
        let retained = if retained {
            Some(MainDeepPointPowersV1::new_v1(native.points, coefficients)?)
        } else {
            None
        };
        let result = Self {
            native,
            retained,
            allowance,
        };
        result.check_workspace_v1(&[], 0, &[], 0)?;
        Ok(result)
    }

    pub(in super::super::super) fn check_workspace_v1(
        &self,
        weighted: &[MainMixedDeepQuotientV1],
        weighted_capacity: usize,
        native: &[ZeroizingMainTraceColumnV1],
        native_capacity: usize,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if weighted.len() > SECURITY_LANES
            || weighted_capacity != weighted.len()
            || native.len() > BATCH
            || native_capacity > BATCH
            || native_capacity < native.len()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut cells = checked_sum(&[
            self.native.weights.capacity(),
            self.native.mask_powers[0].capacity(),
            self.native.mask_powers[1].capacity(),
        ])?;
        if let Some(powers) = &self.retained {
            cells = checked_sum(&[
                cells,
                powers.powers[0].capacity(),
                powers.powers[1].capacity(),
            ])?;
        }
        let mut native_weighted = 0;
        for lane in weighted {
            native_weighted =
                checked_sum(&[native_weighted, lane.native.extension_capacity_v1()?])?;
            cells = checked_sum(&[cells, lane.native.extension_capacity_v1()?])?;
            if let Some(retained) = &lane.retained {
                cells = checked_sum(&[
                    cells,
                    retained.coefficients[0].capacity(),
                    retained.coefficients[1].capacity(),
                ])?;
            }
        }
        let native_cells = native
            .iter()
            .try_fold(0, |sum, column| checked_sum(&[sum, column.0.capacity()]))?;
        let native_bytes = checked_product(&[native_cells, core::mem::size_of::<F>()])?;
        self.native
            .check_live_v1(native_weighted, native_bytes, 0)?;
        if checked_sum(&[
            checked_product(&[cells, core::mem::size_of::<E>()])?,
            native_bytes,
            Self::METADATA,
        ])? > self.allowance
        {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        Ok(())
    }

    fn validate_input_v1(&self, input: MainMixedInputV1<'_>) -> Result<(), ZkX509StarkErrorV1> {
        match input {
            MainMixedInputV1::Native(native, mask) => self.native.validate_column_v1(native, mask),
            MainMixedInputV1::Retained(coefficients) => {
                let powers = self
                    .retained
                    .as_ref()
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
                if coefficients.len() != powers.powers[0].len()
                    || coefficients
                        .iter()
                        .any(|value| F::canonical(value.0).is_none())
                {
                    return Err(ZkX509StarkErrorV1::ProfileMismatch);
                }
                Ok(())
            }
        }
    }
    pub(in super::super::super) fn evaluate_v1(
        &self,
        input: MainMixedInputV1<'_>,
    ) -> Result<[E; 2], ZkX509StarkErrorV1> {
        self.validate_input_v1(input)?;
        match input {
            MainMixedInputV1::Native(native, mask) => self.native.evaluate_v1(native, mask),
            MainMixedInputV1::Retained(coefficients) => self
                .retained
                .as_ref()
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
                .evaluate_v1(coefficients),
        }
    }
}

pub(in super::super::super) struct MainMixedDeepQuotientV1 {
    native: MainNativeDeepQuotientV1,
    retained: Option<MainGroupedDeepQuotientV1>,
}
impl MainMixedDeepQuotientV1 {
    /// Reserve the public lane count before constructing any private owner.
    /// Fallible iterator collection may overallocate, which violates the exact
    /// owner-header reservation checked by `check_workspace_v1`.
    pub(in super::super::super) fn new_lanes_v1(
        points: &MainMixedDeepPointsV1,
    ) -> Result<Vec<Self>, ZkX509StarkErrorV1> {
        let mut lanes = Vec::new();
        lanes
            .try_reserve_exact(SECURITY_LANES)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if lanes.capacity() != SECURITY_LANES {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for _ in 0..SECURITY_LANES {
            lanes.push(Self::new_v1(points)?);
        }
        Ok(lanes)
    }

    pub(in super::super::super) fn new_v1(
        points: &MainMixedDeepPointsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        Ok(Self {
            native: MainNativeDeepQuotientV1::new_v1(&points.native)?,
            retained: points
                .retained
                .as_ref()
                .map(MainGroupedDeepQuotientV1::new_v1)
                .transpose()?,
        })
    }
    pub(in super::super::super) fn add_batch_v1(
        &mut self,
        points: &MainMixedDeepPointsV1,
        columns: &[MainMixedColumnV1<'_>],
    ) -> Result<(), ZkX509StarkErrorV1> {
        // Preserve original precedence: public owner/claim canonicality, then
        // all input shape/canonicality, then individual false-opening checks.
        if columns.is_empty()
            || columns.len() > BATCH
            || self.native.native_log != points.native.native_log
            || self.native.points != points.native.points
            || self.native.native_rows != points.native.native_rows
            || self.native.root != points.native.root
            || self
                .native
                .coefficients
                .iter()
                .any(|column| column.len() != points.native.native_rows + MASK_LENGTH)
            || self.retained.is_some() != points.retained.is_some()
            || columns.iter().any(|(_, values, scales)| {
                values
                    .iter()
                    .chain(scales)
                    .any(|value| !value.is_canonical())
            })
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if let (Some(owner), Some(powers)) = (&self.retained, &points.retained) {
            if owner.points != powers.points
                || owner
                    .coefficients
                    .iter()
                    .any(|column| column.len() != powers.powers[0].len())
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
        }
        for (input, _, _) in columns {
            points.validate_input_v1(*input)?;
        }
        let empty: &[F] = &[];
        let mut native_batch: [NativeColumnV1<'_>; BATCH] =
            core::array::from_fn(|_| (empty, empty, [E::ZERO; 2], [E::ZERO; 2]));
        let mut native_count = 0;
        let mut retained_count = 0;
        for &(input, values, scales) in columns {
            match input {
                MainMixedInputV1::Native(native, mask) => {
                    native_batch[native_count] = (native, mask, values, scales);
                    native_count += 1;
                }
                MainMixedInputV1::Retained(_) => retained_count += 1,
            }
        }
        let total_native = self
            .native
            .columns
            .checked_add(native_count)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let total_retained = self
            .retained
            .as_ref()
            .map_or(0, |owner| owner.columns)
            .checked_add(retained_count)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        // No weighted owner changes until every original opening has passed.
        columns.par_iter().try_for_each(|(input, values, _)| {
            if points.evaluate_v1(*input)? != *values {
                return Err(ZkX509StarkErrorV1::ConstraintOpening);
            }
            Ok(())
        })?;
        if native_count != 0 {
            self.native
                .accumulate_validated_batch_v1(&native_batch[..native_count], total_native);
        }
        if retained_count != 0 {
            let retained = self
                .retained
                .as_mut()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
            for (side, target) in retained.coefficients.iter_mut().enumerate() {
                target
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(degree, target)| {
                        for &(input, _, scales) in columns {
                            if let MainMixedInputV1::Retained(coefficients) = input {
                                *target = target.add(scales[side].mul_base(coefficients[degree]));
                            }
                        }
                    });
            }
            for &(input, values, scales) in columns {
                if matches!(input, MainMixedInputV1::Retained(_)) {
                    for side in 0..2 {
                        retained.values[side] =
                            retained.values[side].add(values[side].mul(scales[side]));
                    }
                }
            }
            retained.columns = total_retained;
        }
        Ok(())
    }
    pub(in super::super::super) fn accumulate_v1(
        self,
        expected_native: usize,
        expected_retained: usize,
        accumulator: &mut [E],
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.native.columns != expected_native
            || self.retained.as_ref().map_or(0, |owner| owner.columns) != expected_retained
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if expected_native != 0 {
            self.native.accumulate_v1(expected_native, accumulator)?;
        }
        if let Some(retained) = self.retained {
            if expected_retained != 0 {
                retained.accumulate_v1(expected_retained, accumulator)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "main_mixed_deep_tests.rs"]
mod tests;
