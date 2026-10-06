//! The existing FF CRT predicate serialized onto shared Glue/range ports.
//!
//! No new gate, advice query, fixed column or lookup is configured here. Each
//! of the four carry equations and the native residue from `fused_constraints`
//! is lowered to standard Glue rows. Result limbs retain 87/87/82 bits,
//! quotient limbs retain 87 bits and offset carries retain 105 bits. Operand
//! admission is exactly the existing 94-bit-envelope/product-bound check;
//! these are the same integer constraints, not a widened carry argument.
//!
//! The complete recursive interpreter can select this profile explicitly.
//! TODO: qualify the composed source/outer layouts; sharing these constraints
//! does not by itself establish their row or transport budget.

use iroha_pasta::PastaField;
use iroha_plonk::frontend::{Error, Region, Value};

use super::{
    CARRIES, CARRY_OFFSET_BITS, FfChip, FfValue, ForeignModulus, Form, FusedWitness, LIMB_BITS,
    LIMBS, Mode, Nat, PROPER_BOUNDS, TOP_LIMB_BITS, compare_witness, div_witness, from_limbs,
    limb_fields, mul_witness,
};
use crate::{
    GlueChip, Word,
    arith::{Coefficients, Slot},
    range::RunningSumChip,
};

/// A stateless FF lowering sharing the caller's Glue and range cursors.
/// Opaque [`FfValue`] inputs carry their existing constrained limb bounds.
#[derive(Clone, Copy, Debug, Default)]
pub struct SerializedFf;

impl SerializedFf {
    /// Assigns an integer below `2^256` with proper 87/87/82-bit limbs.
    ///
    /// # Errors
    /// Layout errors. An out-of-range limb is unsatisfiable.
    pub fn witness<F: PastaField>(
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        modulus: ForeignModulus,
        value: Value<[u64; 4]>,
    ) -> Result<FfValue<F>, Error> {
        let values = value.map(|words| limb_fields::<F>(&Nat::from_words(words)));
        let limbs = ranged(range, region, values, [LIMB_BITS, LIMB_BITS, TOP_LIMB_BITS])?;
        Ok(FfValue::from_parts(
            limbs,
            PROPER_BOUNDS,
            modulus,
            Form::Proper,
        ))
    }

    /// Proves that the retained proper integer is below its exact modulus.
    /// No reduction or reinterpretation of its limbs occurs.
    ///
    /// # Errors
    /// Non-proper input or layout errors. An integer at least the modulus
    /// makes the comparison unsatisfiable.
    pub fn assert_canonical<F: PastaField>(
        glue: &mut GlueChip<F>,
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        value: &FfValue<F>,
    ) -> Result<FfValue<F>, Error> {
        if value.form == Form::Canonical {
            return Ok(value.clone());
        }
        if value.form != Form::Proper {
            return Err(Error::Synthesis);
        }
        let difference = value
            .limb_values()
            .map(|x| compare_witness(value.modulus, &x));
        let difference = ranged(range, region, difference, [LIMB_BITS; LIMBS])?;
        let bound = value.modulus.limbs_minus_one();
        let radix = F::from_u128(1 << LIMB_BITS);
        let borrow = glue.linear(
            region,
            &[(-F::ONE, &value.limbs[2]), (-F::ONE, &difference[2])],
            F::from_u128(bound[2]),
        )?;
        glue.assert_bool(region, &borrow)?;
        let low = glue.linear(
            region,
            &[
                (F::ONE, &value.limbs[0]),
                (F::ONE, &difference[0]),
                (-radix * radix, &borrow),
            ],
            -F::from_u128(bound[0]),
        )?;
        let high = glue.linear(
            region,
            &[(F::ONE, &value.limbs[1]), (F::ONE, &difference[1])],
            -F::from_u128(bound[1]),
        )?;
        constrain_linear(glue, region, [(F::ONE, &low), (radix, &high)], F::ZERO)?;
        Ok(FfValue {
            form: Form::Canonical,
            ..value.clone()
        })
    }

    /// Multiplies operands admitted by the unchanged fused FF bounds.
    ///
    /// # Errors
    /// Mixed moduli, an inadmissible tracked product bound, or layout errors.
    /// Callers must explicitly reduce operands before an inadmissible product.
    pub fn mul<F: PastaField>(
        glue: &mut GlueChip<F>,
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        left: &FfValue<F>,
        right: &FfValue<F>,
    ) -> Result<FfValue<F>, Error> {
        if left.modulus != right.modulus
            || !FfChip::<F>::mul_admissible(left.modulus, &left.bounds, &right.bounds)
        {
            return Err(Error::Synthesis);
        }
        let witness = left
            .limb_values()
            .zip(right.limb_values())
            .map(|(a, b)| mul_witness(left.modulus, &a, &b));
        fused(glue, range, region, Mode::Mul, left, right, witness)
    }

    /// Divides using the same fixed padding and admission bound as fused FF.
    /// The numerator and divisor are retained as exact constrained limbs.
    ///
    /// # Errors
    /// Mixed moduli, inadmissible bounds, unsupported padding or layout errors.
    /// A zero divisor with nonzero numerator makes the circuit unsatisfiable.
    pub fn div<F: PastaField>(
        glue: &mut GlueChip<F>,
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        numerator: &FfValue<F>,
        divisor: &FfValue<F>,
    ) -> Result<FfValue<F>, Error> {
        if numerator.modulus != divisor.modulus
            || !FfChip::<F>::div_admissible(numerator.modulus, &numerator.bounds, &divisor.bounds)
        {
            return Err(Error::Synthesis);
        }
        let witness = numerator
            .limb_values()
            .zip(divisor.limb_values())
            .map(|(a, b)| div_witness(numerator.modulus, &a, &b));
        fused(glue, range, region, Mode::Div, numerator, divisor, witness)
    }
}

pub(super) fn ranged<F: PastaField, const N: usize>(
    range: &mut RunningSumChip<F>,
    region: &mut Region<'_, F>,
    values: Value<[F; N]>,
    widths: [usize; N],
) -> Result<[Word<F>; N], Error> {
    widths
        .into_iter()
        .enumerate()
        .map(|(i, width)| range.witness_range_checked(region, values.map(|x| x[i]), width))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Synthesis)
}

pub(super) fn constrain_linear<F: PastaField, const N: usize>(
    glue: &mut GlueChip<F>,
    region: &mut Region<'_, F>,
    terms: [(F, &Word<F>); N],
    constant: F,
) -> Result<(), Error> {
    if N > 4 {
        return Err(Error::Synthesis);
    }
    let mut slots = [Slot::Empty; 4];
    let mut coefficients = [F::ZERO; 4];
    for (i, (coefficient, word)) in terms.into_iter().enumerate() {
        slots[i] = Slot::Copy(word);
        coefficients[i] = coefficient;
    }
    let [a, b, c, d] = coefficients;
    glue.row(
        region,
        Coefficients {
            a,
            b,
            c,
            d,
            k: constant,
            m: F::ZERO,
        },
        slots,
        None,
    )?;
    Ok(())
}

pub(super) fn recompose<F: PastaField>(
    glue: &mut GlueChip<F>,
    region: &mut Region<'_, F>,
    limbs: &[Word<F>; LIMBS],
) -> Result<Word<F>, Error> {
    let radix = F::from_u128(1 << LIMB_BITS);
    glue.linear(
        region,
        &[
            (F::ONE, &limbs[0]),
            (radix, &limbs[1]),
            (radix * radix, &limbs[2]),
        ],
        F::ZERO,
    )
}

fn fused<F: PastaField>(
    glue: &mut GlueChip<F>,
    range: &mut RunningSumChip<F>,
    region: &mut Region<'_, F>,
    mode: Mode,
    left: &FfValue<F>,
    right: &FfValue<F>,
    witness: Value<FusedWitness<F>>,
) -> Result<FfValue<F>, Error> {
    let limbs = constrain_fused(
        glue,
        range,
        region,
        mode,
        left.modulus,
        (&left.limbs, &right.limbs),
        witness,
    )?;
    Ok(FfValue::from_parts(
        limbs,
        PROPER_BOUNDS,
        left.modulus,
        Form::Proper,
    ))
}

pub(super) fn constrain_fused<F: PastaField>(
    glue: &mut GlueChip<F>,
    range: &mut RunningSumChip<F>,
    region: &mut Region<'_, F>,
    mode: Mode,
    modulus: ForeignModulus,
    operands: (&[Word<F>; LIMBS], &[Word<F>; LIMBS]),
    witness: Value<FusedWitness<F>>,
) -> Result<[Word<F>; LIMBS], Error> {
    let (left, right) = operands;
    let result = ranged(
        range,
        region,
        witness.map(|w| w.c),
        [LIMB_BITS, LIMB_BITS, TOP_LIMB_BITS],
    )?;
    let quotient = ranged(range, region, witness.map(|w| w.q), [LIMB_BITS; LIMBS])?;
    let carries = ranged(
        range,
        region,
        witness.map(|w| w.u),
        [CARRY_OFFSET_BITS + 1; CARRIES],
    )?;
    let (factor_left, factor_right, subtracted, padding) = match mode {
        Mode::Mul => (left, right, &result, [0; LIMBS]),
        Mode::Div => (
            right,
            &result,
            left,
            modulus.division_padding().ok_or(Error::Synthesis)?.1,
        ),
    };
    let m = modulus.limbs();
    let radix = F::from_u128(1 << LIMB_BITS);
    let offset = F::from_u128(1 << CARRY_OFFSET_BITS);
    for column in 0..CARRIES {
        // Sum the non-product terms in at most two linear rows. Carries are
        // stored offset by 2^104; remove the incoming offset here and restore
        // the outgoing offset in the final product row below.
        let mut terms = Vec::new();
        for (i, quotient) in quotient.iter().enumerate() {
            if let Some(j) = column.checked_sub(i).filter(|j| *j < LIMBS) {
                terms.push((-F::from_u128(m[j]), quotient));
            }
        }
        if column < LIMBS {
            terms.push((-F::ONE, &subtracted[column]));
        }
        if column > 0 {
            terms.push((F::ONE, &carries[column - 1]));
        }
        let constant = F::from_u128(padding.get(column).copied().unwrap_or(0))
            - if column > 0 { offset } else { F::ZERO };
        let (first, rest) = terms.split_at(terms.len().min(3));
        let mut sum = glue.linear(region, first, constant)?;
        for chunk in rest.chunks(2) {
            let mut next = vec![(F::ONE, &sum)];
            next.extend_from_slice(chunk);
            sum = glue.linear(region, &next, F::ZERO)?;
        }
        let products = (0..LIMBS)
            .filter_map(|i| {
                column
                    .checked_sub(i)
                    .filter(|j| *j < LIMBS)
                    .map(|j| (&factor_left[i], &factor_right[j]))
            })
            .collect::<Vec<_>>();
        for (index, (a, b)) in products.iter().enumerate() {
            if index + 1 == products.len() {
                glue.row(
                    region,
                    Coefficients {
                        m: F::ONE,
                        c: F::ONE,
                        d: -radix,
                        k: offset * radix,
                        ..Coefficients::zero()
                    },
                    [
                        Slot::Copy(a),
                        Slot::Copy(b),
                        Slot::Copy(&sum),
                        Slot::Copy(&carries[column]),
                    ],
                    None,
                )?;
            } else {
                sum = glue.mul_add(region, a, b, &sum)?;
            }
        }
    }
    let left_native = recompose(glue, region, factor_left)?;
    let right_native = recompose(glue, region, factor_right)?;
    let subtracted_native = recompose(glue, region, subtracted)?;
    let quotient_native = recompose(glue, region, &quotient)?;
    glue.row(
        region,
        Coefficients {
            m: F::ONE,
            c: -F::ONE,
            d: -modulus.nat().to_field::<F>(),
            k: from_limbs(&padding).to_field::<F>(),
            ..Coefficients::zero()
        },
        [
            Slot::Copy(&left_native),
            Slot::Copy(&right_native),
            Slot::Copy(&subtracted_native),
            Slot::Copy(&quotient_native),
        ],
        None,
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests;
