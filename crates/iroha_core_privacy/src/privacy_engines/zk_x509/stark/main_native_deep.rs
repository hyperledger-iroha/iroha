//! Native-row DEEP openings and two weighted interpolations per security lane and native group.

use super::*;
use crate::privacy_engines::transparent_stark::goldilocks_fp4_ifft_v1;

const MASK_LENGTH: usize = MASK_DEGREE + 1;
const BATCH: usize = aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1;
// Covers owners, bounded borrowed descriptors, source Vec headers and scalar
// temporaries inside the field payload freed from the previous replay plan.
const METADATA_BYTES: usize = 16 * 1024;

fn checked_sum(parts: &[usize]) -> Result<usize, ZkX509StarkErrorV1> {
    parts.iter().try_fold(0_usize, |total, &part| {
        total
            .checked_add(part)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
    })
}
fn checked_product(parts: &[usize]) -> Result<usize, ZkX509StarkErrorV1> {
    parts.iter().try_fold(1_usize, |total, &part| {
        total
            .checked_mul(part)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
    })
}

/// One stack-only original column; values and scales come from the transcript.
pub(in super::super) type NativeColumnV1<'a> = (&'a [F], &'a [F], [E; 2], [E; 2]);

/// Public Lagrange weights and mask powers for exactly one native group.
pub(in super::super) struct MainNativeDeepPointsV1 {
    native_log: u8,
    native_rows: usize,
    root: F,
    points: [E; 2],
    vanishing: E,
    weights: ExtensionColumn,
    mask_powers: [ExtensionColumn; 2],
    allowance: usize,
}

impl MainNativeDeepPointsV1 {
    pub(in super::super) fn new_v1(native_log: u8, point: E) -> Result<Self, ZkX509StarkErrorV1> {
        if native_log == 0
            || native_log > ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
            || !point.is_canonical()
            || point == E::ZERO
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let native_rows = 1_usize << native_log;
        let length = checked_sum(&[native_rows, MASK_LENGTH])?;
        // Exact former DEEP field allowance already admitted by replay_batch.
        let allowance = checked_sum(&[
            checked_product(&[2 + 2 * SECURITY_LANES, length, core::mem::size_of::<E>()])?,
            checked_product(&[BATCH, length, core::mem::size_of::<F>()])?,
        ])?;
        let metadata = checked_sum(&[
            core::mem::size_of::<Self>(),
            core::mem::size_of::<ExtensionColumn>(), // transient inversion prefix
            checked_product(&[
                SECURITY_LANES,
                core::mem::size_of::<MainNativeDeepQuotientV1>(),
            ])?,
            core::mem::size_of::<Vec<MainNativeDeepQuotientV1>>(),
            core::mem::size_of::<[NativeColumnV1<'_>; BATCH]>(),
            core::mem::size_of::<MainDeepStackValuesV1<{ 2 * BATCH }>>(),
            core::mem::size_of::<MainDeepStackValuesV1<1>>(),
            core::mem::size_of::<Vec<ZeroizingMainTraceColumnV1>>(),
            checked_product(&[BATCH, core::mem::size_of::<ZeroizingMainTraceColumnV1>()])?,
            checked_product(&[BATCH, core::mem::size_of::<[E; 16]>()])?, // bounded parallel field temporaries
            512, // remaining public scalar arithmetic and loop temporaries
        ])?;
        if metadata > METADATA_BYTES {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        // Admit both lifetimes before reserving any arrays. The actual capacity
        // checks below retain this same cap even if an allocator rounds upward.
        let setup =
            checked_product(&[2 * native_rows + 2 * MASK_LENGTH, core::mem::size_of::<E>()])?;
        let steady = checked_sum(&[
            checked_product(&[
                native_rows + 2 * MASK_LENGTH + 2 * SECURITY_LANES * length,
                core::mem::size_of::<E>(),
            ])?,
            checked_product(&[BATCH, native_rows, core::mem::size_of::<F>()])?,
        ])?;
        if checked_sum(&[setup.max(steady), METADATA_BYTES])? > allowance {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let root = goldilocks_primitive_root_v1(native_log).map_err(map_transparent_error_v1)?;
        let points = [point, point.mul_base(root)];
        let vanishing = point.pow(native_rows as u128).sub(E::ONE);
        let mut this = Self {
            native_log,
            native_rows,
            root,
            points,
            vanishing,
            weights: zero_column_v1(native_rows)?,
            mask_powers: [zero_column_v1(MASK_LENGTH)?, zero_column_v1(MASK_LENGTH)?],
            allowance,
        };
        this.check_live_v1(0, 0, 0)?;
        let mut root_power = F::ONE;
        let mut native_index = None;
        for (index, weight) in this.weights.iter_mut().enumerate() {
            *weight = point.sub(E::from_base(root_power));
            if *weight == E::ZERO {
                native_index = Some(index);
            }
            root_power = root_power.mul(root);
        }
        if let Some(index) = native_index {
            // Preserve valid nonzero subgroup points without a zero inverse.
            this.weights.fill(E::ZERO);
            this.weights[index] = E::ONE;
        } else {
            let mut prefix = zero_column_v1(native_rows)?;
            this.check_live_v1(prefix.capacity(), 0, 0)?;
            let mut product = E::ONE;
            for (prefix, &denominator) in prefix.iter_mut().zip(this.weights.iter()) {
                *prefix = product;
                product = product.mul(denominator);
            }
            let mut inverse = product.inv().ok_or(ZkX509StarkErrorV1::ConstraintOpening)?;
            for index in (0..native_rows).rev() {
                let denominator = this.weights[index];
                this.weights[index] = inverse.mul(prefix[index]);
                inverse = inverse.mul(denominator);
            }
            let scale = vanishing.mul_base(
                F::reduce(native_rows as u128)
                    .inv()
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            );
            root_power = F::ONE;
            for weight in this.weights.iter_mut() {
                *weight = weight.mul(scale).mul_base(root_power);
                root_power = root_power.mul(root);
            }
            // Prefix is public but uses the same clearing ownership. Its entire
            // lifetime ends before any weighted or native source batch exists.
        }
        for (powers, point) in this.mask_powers.iter_mut().zip(points) {
            let mut power = E::ONE;
            for value in powers.iter_mut() {
                *value = power;
                power = power.mul(point);
            }
        }
        this.check_live_v1(
            2 * SECURITY_LANES * length,
            BATCH * native_rows * core::mem::size_of::<F>(),
            0,
        )?;
        Ok(this)
    }

    /// Charge live allocation capacities; source-owned scratch keeps its separate
    /// existing replay reservation. This guard does not increase the old cap.
    pub(in super::super) fn check_live_v1(
        &self,
        other_extension_capacity: usize,
        native_bytes: usize,
        extra_metadata_bytes: usize,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let capacity = checked_sum(&[
            self.weights.capacity(),
            self.mask_powers[0].capacity(),
            self.mask_powers[1].capacity(),
            other_extension_capacity,
        ])?;
        let bytes = checked_sum(&[
            checked_product(&[capacity, core::mem::size_of::<E>()])?,
            native_bytes,
            METADATA_BYTES,
            extra_metadata_bytes,
        ])?;
        if bytes > self.allowance {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        Ok(())
    }

    pub(in super::super) fn check_workspace_v1(
        &self,
        weighted: &[MainNativeDeepQuotientV1],
        weighted_capacity: usize,
        native: &[ZeroizingMainTraceColumnV1],
        native_capacity: usize,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if weighted.len() > SECURITY_LANES
            || weighted_capacity < weighted.len()
            || native.len() > BATCH
            || native_capacity < native.len()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let extensions = weighted.iter().try_fold(0_usize, |sum, owner| {
            checked_sum(&[sum, owner.extension_capacity_v1()?])
        })?;
        let native_cells = native.iter().try_fold(0_usize, |sum, owner| {
            checked_sum(&[sum, owner.0.capacity()])
        })?;
        self.check_live_v1(
            extensions,
            checked_product(&[native_cells, core::mem::size_of::<F>()])?,
            checked_sum(&[
                checked_product(&[
                    weighted_capacity,
                    core::mem::size_of::<MainNativeDeepQuotientV1>(),
                ])?,
                checked_product(&[
                    native_capacity,
                    core::mem::size_of::<ZeroizingMainTraceColumnV1>(),
                ])?,
            ])?,
        )
    }

    fn validate_column_v1(&self, native: &[F], mask: &[F]) -> Result<(), ZkX509StarkErrorV1> {
        if native.len() != self.native_rows
            || mask.len() != MASK_LENGTH
            || native
                .iter()
                .chain(mask)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }

    pub(in super::super) fn evaluate_v1(
        &self,
        native: &[F],
        mask: &[F],
    ) -> Result<[E; 2], ZkX509StarkErrorV1> {
        self.validate_column_v1(native, mask)?;
        let mut values = [E::ZERO; 2];
        for (index, &value) in native.iter().enumerate() {
            values[0] = values[0].add(self.weights[index].mul_base(value));
            // L_i(omega*z) = L_(i-1 mod N)(z), including subgroup points.
            values[1] = values[1].add(
                self.weights[(index + self.native_rows - 1) & (self.native_rows - 1)]
                    .mul_base(value),
            );
        }
        for (value, powers) in values.iter_mut().zip(&self.mask_powers) {
            let mask_value = mask
                .iter()
                .zip(powers.iter())
                .fold(E::ZERO, |sum, (&coefficient, power)| {
                    sum.add(power.mul_base(coefficient))
                });
            *value = value.add(mask_value.mul(self.vanishing));
        }
        Ok(values)
    }
}

/// Two weighted native columns followed by their original weighted mask tails.
pub(in super::super) struct MainNativeDeepQuotientV1 {
    native_log: u8,
    native_rows: usize,
    root: F,
    points: [E; 2],
    coefficients: [ExtensionColumn; 2],
    values: [E; 2],
    columns: usize,
}

impl MainNativeDeepQuotientV1 {
    pub(in super::super) fn new_v1(
        points: &MainNativeDeepPointsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let length = points.native_rows + MASK_LENGTH;
        let result = Self {
            native_log: points.native_log,
            native_rows: points.native_rows,
            root: points.root,
            points: points.points,
            coefficients: [zero_column_v1(length)?, zero_column_v1(length)?],
            values: [E::ZERO; 2],
            columns: 0,
        };
        points.check_live_v1(result.extension_capacity_v1()?, 0, 0)?;
        Ok(result)
    }

    pub(in super::super) fn extension_capacity_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        checked_sum(&[
            self.coefficients[0].capacity(),
            self.coefficients[1].capacity(),
        ])
    }

    pub(in super::super) fn add_batch_v1(
        &mut self,
        points: &MainNativeDeepPointsV1,
        columns: &[NativeColumnV1<'_>],
    ) -> Result<(), ZkX509StarkErrorV1> {
        let count = self
            .columns
            .checked_add(columns.len())
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if columns.is_empty()
            || columns.len() > BATCH
            || self.native_log != points.native_log
            || self.points != points.points
            || self.native_rows != points.native_rows
            || self.root != points.root
            || self
                .coefficients
                .iter()
                .any(|column| column.len() != self.native_rows + MASK_LENGTH)
            || columns.iter().any(|(_, _, values, scales)| {
                values
                    .iter()
                    .chain(scales)
                    .any(|value| !value.is_canonical())
            })
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        // Shape and canonicality have a deterministic serial precedence over
        // false openings. A parallel malformed/false pair cannot race two errors.
        for (native, mask, _, _) in columns {
            points.validate_column_v1(native, mask)?;
        }
        // Validate every original claim before either weighted array changes.
        // Cancelling malicious claims never gain authority through linearity.
        columns
            .par_iter()
            .try_for_each(|(native, mask, values, _)| {
                if points.evaluate_v1(native, mask)? != *values {
                    return Err(ZkX509StarkErrorV1::ConstraintOpening);
                }
                Ok(())
            })?;
        let n = self.native_rows;
        for (side, target) in self.coefficients.iter_mut().enumerate() {
            target
                .par_iter_mut()
                .enumerate()
                .for_each(|(index, target)| {
                    // Each output index has one owner; exact column order is fixed.
                    for (native, mask, _, scales) in columns {
                        let source = if index < n {
                            native[index]
                        } else {
                            mask[index - n]
                        };
                        *target = target.add(scales[side].mul_base(source));
                    }
                });
        }
        for (_, _, values, scales) in columns {
            for side in 0..2 {
                self.values[side] = self.values[side].add(values[side].mul(scales[side]));
            }
        }
        self.columns = count;
        Ok(())
    }

    pub(in super::super) fn accumulate_v1(
        mut self,
        expected_columns: usize,
        accumulator: &mut [E],
    ) -> Result<(), ZkX509StarkErrorV1> {
        let length = self.native_rows + MASK_LENGTH;
        if self.columns == 0
            || self.columns != expected_columns
            || self.coefficients.iter().any(|column| {
                column.len() != length || column.iter().any(|value| !value.is_canonical())
            })
            || self
                .points
                .iter()
                .chain(&self.values)
                .any(|value| !value.is_canonical())
            || length - 1 > accumulator.len()
            || accumulator.iter().any(|value| !value.is_canonical())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        {
            let mut original_mask = MainDeepStackValuesV1::<1>::zero_v1();
            for coefficients in &mut self.coefficients {
                goldilocks_fp4_ifft_v1(&mut coefficients[..self.native_rows], self.root)
                    .map_err(map_transparent_error_v1)?;
                // The tail already contains X^N*r. Subtract r in increasing order:
                // for M>N a later subtraction may overwrite an earlier-read tail,
                // but no earlier subtraction can overwrite a future source cell.
                for degree in 0..MASK_LENGTH {
                    original_mask.as_mut_slice_v1()[0] = coefficients[self.native_rows + degree];
                    coefficients[degree] = coefficients[degree].sub(original_mask.as_slice_v1()[0]);
                    original_mask.clear_v1();
                }
            }
        }
        for ((coefficients, point), value) in
            self.coefficients.iter().zip(self.points).zip(self.values)
        {
            accumulate_extension_deep_quotient_v1(coefficients, point, value, E::ONE, accumulator)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "main_native_deep_tests.rs"]
mod tests;
