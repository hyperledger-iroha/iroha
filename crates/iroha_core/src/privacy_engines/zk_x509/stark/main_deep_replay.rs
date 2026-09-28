//! Linear DEEP batching with each original opening still checked independently.

use super::super::super::private_table::PrivateTableV1;
use super::*;

type ExtensionColumn = PrivateTableV1<E>;

fn erase_extensions_v1(values: &mut [E]) {
    #[cfg(test)]
    if observations::enabled_v1() {
        let mut observed = (values.len(), 0, 0);
        for value in values {
            observed.1 += usize::from(*value != E::ZERO);
            value.zeroize_v1();
            observed.2 += usize::from(*value != E::ZERO);
        }
        observations::record_v1(observed);
        return;
    }
    for value in values {
        value.zeroize_v1();
    }
}

fn zero_column_v1(length: usize) -> Result<ExtensionColumn, ZkX509StarkErrorV1> {
    let mut column = PrivateTableV1::new(Vec::new(), erase_extensions_v1);
    column
        .try_reserve_exact(length)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    column.resize(length, E::ZERO);
    Ok(column)
}

/// Powers of exactly the two points already bound by the original transcript.
pub(super) struct MainDeepPointPowersV1 {
    points: [E; 2],
    powers: [ExtensionColumn; 2],
}

impl MainDeepPointPowersV1 {
    pub(super) fn new_v1(points: [E; 2], coefficients: usize) -> Result<Self, ZkX509StarkErrorV1> {
        if coefficients == 0
            || coefficients > (1_usize << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1) + MASK_DEGREE + 1
            || points
                .iter()
                .any(|point| !point.is_canonical() || *point == E::ZERO)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut powers = [zero_column_v1(coefficients)?, zero_column_v1(coefficients)?];
        for (column, point) in powers.iter_mut().zip(points) {
            let mut power = E::ONE;
            for value in column.iter_mut() {
                *value = power;
                power = power.mul(point);
            }
        }
        Ok(Self { points, powers })
    }

    pub(super) fn evaluate_v1(&self, coefficients: &[F]) -> Result<[E; 2], ZkX509StarkErrorV1> {
        if coefficients.is_empty()
            || coefficients.len() > self.powers[0].len()
            || coefficients
                .iter()
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut result = [E::ZERO; 2];
        for (degree, &coefficient) in coefficients.iter().enumerate() {
            for (value, powers) in result.iter_mut().zip(&self.powers) {
                *value = value.add(powers[degree].mul_base(coefficient));
            }
        }
        Ok(result)
    }
}

/// Weighted polynomials at one native group's common current/next points.
/// Unequal coefficient lengths are padded with zero, never with witness data.
pub(super) struct MainGroupedDeepQuotientV1 {
    points: [E; 2],
    coefficients: [ExtensionColumn; 2],
    values: [E; 2],
    columns: usize,
}

impl MainGroupedDeepQuotientV1 {
    pub(super) fn new_v1(powers: &MainDeepPointPowersV1) -> Result<Self, ZkX509StarkErrorV1> {
        let length = powers.powers[0].len();
        Ok(Self {
            points: powers.points,
            coefficients: [zero_column_v1(length)?, zero_column_v1(length)?],
            values: [E::ZERO; 2],
            columns: 0,
        })
    }

    pub(super) fn add_v1(
        &mut self,
        powers: &MainDeepPointPowersV1,
        coefficients: &[F],
        values: [E; 2],
        scales: [E; 2],
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.points != powers.points
            || self.coefficients[0].len() != powers.powers[0].len()
            || values
                .iter()
                .chain(&scales)
                .any(|value| !value.is_canonical())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        // Retain the original per-column check, even if two malicious claim
        // changes would cancel in the weighted aggregate.
        if powers.evaluate_v1(coefficients)? != values {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        for (degree, &coefficient) in coefficients.iter().enumerate() {
            for (column, scale) in self.coefficients.iter_mut().zip(scales) {
                column[degree] = column[degree].add(scale.mul_base(coefficient));
            }
        }
        for ((target, value), scale) in self.values.iter_mut().zip(values).zip(scales) {
            *target = target.add(value.mul(scale));
        }
        self.columns += 1;
        Ok(())
    }

    pub(super) fn accumulate_v1(
        self,
        expected_columns: usize,
        accumulator: &mut [E],
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.columns == 0
            || self.columns != expected_columns
            || self.coefficients[0].len() - 1 > accumulator.len()
            || accumulator.iter().any(|value| !value.is_canonical())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        // Division by the same (X-point) is linear. Only these two complete
        // weighted polynomials require extension-field synthetic division.
        for ((coefficients, point), value) in
            self.coefficients.iter().zip(self.points).zip(self.values)
        {
            accumulate_extension_deep_quotient_v1(coefficients, point, value, E::ONE, accumulator)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod observations {
    use std::cell::RefCell;
    thread_local! {
        static CELLS: RefCell<Option<Vec<(usize, usize, usize)>>> = const { RefCell::new(None) };
    }
    pub(super) fn enabled_v1() -> bool {
        CELLS.with_borrow(Option::is_some)
    }
    pub(super) fn record_v1(value: (usize, usize, usize)) {
        CELLS.with_borrow_mut(|cells| {
            if let Some(cells) = cells {
                cells.push(value);
            }
        });
    }
    pub(super) fn observe_v1<T>(operation: impl FnOnce() -> T) -> (T, Vec<(usize, usize, usize)>) {
        struct Scope;
        impl Drop for Scope {
            fn drop(&mut self) {
                CELLS.set(None);
            }
        }
        CELLS.set(Some(Vec::new()));
        let _scope = Scope;
        let result = operation();
        (result, CELLS.take().unwrap())
    }
}

#[cfg(test)]
#[path = "main_deep_replay_tests.rs"]
mod tests;
