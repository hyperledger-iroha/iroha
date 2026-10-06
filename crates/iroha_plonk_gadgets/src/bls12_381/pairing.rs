//! Constrained BLS12-381 Miller-loop building blocks.
//!
//! These implement the M-twist homogeneous line formulas used by the native
//! BLS12-381 pairing. They are bounded arithmetic steps, not a pairing verifier:
//! a consumer must bind the complete fixed `|x|` schedule, negative-x
//! conjugation, final exponentiation, and all point subgroup/source checks.
//! Continuation witnesses must be copy-bound or commitment-bound to the exact
//! preceding step. Merely assigning a valid intermediate point or line is not
//! evidence that the complete Miller loop was run.

use super::{
    curve::{G1Value, G2Value},
    extension::{Fp2, Fp2Value, Fp6Value, Fp12Value},
    field::Bls381Chip,
    native,
};
use iroha_pasta::PastaField;
use iroha_plonk::frontend::{Error, Region, Value};

/// Magnitude of the negative BLS parameter; process bits MSB first, skipping
/// the leading one, then conjugate the Miller accumulator.
pub const MILLER_X: u64 = 0xd201_0000_0001_0000;

/// Native homogeneous witness, representing affine `(x/z,y/z)`.
#[derive(Clone, Copy, Debug)]
pub struct MillerG2Witness {
    /// Canonical homogeneous x coordinate.
    pub x: Fp2,
    /// Canonical homogeneous y coordinate.
    pub y: Fp2,
    /// Canonical nonzero homogeneous z coordinate.
    pub z: Fp2,
}
/// Nonzero homogeneous on-curve point for one Miller continuation.
#[derive(Clone, Debug)]
pub struct MillerG2Value<F: PastaField> {
    x: Fp2Value<F>,
    y: Fp2Value<F>,
    z: Fp2Value<F>,
}
impl<F: PastaField> MillerG2Value<F> {
    /// Homogeneous x coordinate.
    pub const fn x(&self) -> &Fp2Value<F> {
        &self.x
    }
    /// Homogeneous y coordinate.
    pub const fn y(&self) -> &Fp2Value<F> {
        &self.y
    }
    /// Nonzero homogeneous z coordinate.
    pub const fn z(&self) -> &Fp2Value<F> {
        &self.z
    }
    /// Native witness; grants no source or subgroup authority.
    pub fn value(&self) -> Value<MillerG2Witness> {
        self.x
            .value()
            .zip(self.y.value())
            .zip(self.z.value())
            .map(|((x, y), z)| MillerG2Witness { x, y, z })
    }
}
/// Three canonical Fp2 coefficients of an M-twist line.
#[derive(Clone, Debug)]
pub struct MillerLine<F: PastaField> {
    coefficients: [Fp2Value<F>; 3],
}
impl<F: PastaField> MillerLine<F> {
    /// Coefficients before multiplying the last two by G1 x and y.
    pub const fn coefficients(&self) -> &[Fp2Value<F>; 3] {
        &self.coefficients
    }
    /// Native line witness, without an origin claim.
    pub fn value(&self) -> Value<[Fp2; 3]> {
        self.coefficients[0]
            .value()
            .zip(self.coefficients[1].value())
            .zip(self.coefficients[2].value())
            .map(|((a, b), c)| [a, b, c])
    }
}
impl<F: PastaField> Bls381Chip<'_, F> {
    fn require_miller_z(
        &mut self,
        region: &mut Region<'_, F>,
        z: &Fp2Value<F>,
    ) -> Result<(), Error> {
        let zero = self.is_zero_fp2(region, z)?;
        self.glue().enforce_constant(region, zero.word(), F::ZERO)
    }
    /// Assign a canonical nonzero homogeneous continuation and its curve equation.
    /// Its exact source must additionally be bound by the consuming relation.
    /// # Errors
    /// Returns layout errors; zero z and off-curve points are unsatisfiable.
    pub fn assign_miller_g2(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<MillerG2Witness>,
    ) -> Result<MillerG2Value<F>, Error> {
        let x = self.assign_fp2(region, value.map(|p| p.x))?;
        let y = self.assign_fp2(region, value.map(|p| p.y))?;
        let z = self.assign_fp2(region, value.map(|p| p.z))?;
        self.require_miller_z(region, &z)?;
        let yy = self.square_fp2(region, &y)?;
        let lhs = self.mul_fp2(region, &yy, &z)?;
        let xx = self.square_fp2(region, &x)?;
        let xxx = self.mul_fp2(region, &xx, &x)?;
        let zz = self.square_fp2(region, &z)?;
        let zzz = self.mul_fp2(region, &zz, &z)?;
        let b = self.constant_fp2(region, [[4, 0, 0, 0, 0, 0]; 2])?;
        let bz = self.mul_fp2(region, &b, &zzz)?;
        let rhs = self.add_fp2(region, &xxx, &bz)?;
        self.assert_equal_fp2(region, &lhs, &rhs)?;
        Ok(MillerG2Value { x, y, z })
    }
    /// Start a Miller loop at a nonidentity affine point with fixed `z=1`.
    /// # Errors
    /// Returns layout errors; the identity is unsatisfiable.
    pub fn start_miller_g2(
        &mut self,
        region: &mut Region<'_, F>,
        point: &G2Value<F>,
    ) -> Result<MillerG2Value<F>, Error> {
        self.assert_nonidentity_g2(region, point)?;
        let z = self.constant_fp2(region, [native::ONE, native::ZERO])?;
        Ok(MillerG2Value {
            x: point.x().clone(),
            y: point.y().clone(),
            z,
        })
    }
    /// Assign canonical line coefficients for a commitment-bound continuation.
    /// This assignment by itself does not prove the line's origin.
    /// # Errors
    /// Returns layout errors.
    pub fn assign_miller_line(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<[Fp2; 3]>,
    ) -> Result<MillerLine<F>, Error> {
        Ok(MillerLine {
            coefficients: [
                self.assign_fp2(region, value.map(|x| x[0]))?,
                self.assign_fp2(region, value.map(|x| x[1]))?,
                self.assign_fp2(region, value.map(|x| x[2]))?,
            ],
        })
    }
    /// One homogeneous doubling and its exact M-twist line coefficients.
    /// # Errors
    /// Returns layout errors; a degenerate output at infinity is unsatisfiable.
    /// Valid nonidentity prime-subgroup points cannot reach this exception in
    /// the fixed BLS Miller schedule.
    pub fn miller_double(
        &mut self,
        region: &mut Region<'_, F>,
        p: &MillerG2Value<F>,
    ) -> Result<(MillerG2Value<F>, MillerLine<F>), Error> {
        let half = self.constant(
            region,
            [
                0xdcff7fffffffd556,
                0x0f55ffff58a9ffff,
                0xb39869507b587b12,
                0xb23ba5c279c2895f,
                0x258dd3db21a5d66b,
                0x0d0088f51cbff34d,
            ],
        )?;
        let a = self.mul_fp2(region, &p.x, &p.y)?;
        let a = self.mul_fp2_by_fp(region, &a, &half)?;
        let b = self.square_fp2(region, &p.y)?;
        let c = self.square_fp2(region, &p.z)?;
        let twice_c = self.add_fp2(region, &c, &c)?;
        let thrice_c = self.add_fp2(region, &twice_c, &c)?;
        let coefficient_b = self.constant_fp2(region, [[4, 0, 0, 0, 0, 0]; 2])?;
        let e = self.mul_fp2(region, &coefficient_b, &thrice_c)?;
        let twice_e = self.add_fp2(region, &e, &e)?;
        let f = self.add_fp2(region, &twice_e, &e)?;
        let g = self.add_fp2(region, &b, &f)?;
        let g = self.mul_fp2_by_fp(region, &g, &half)?;
        let yz = self.add_fp2(region, &p.y, &p.z)?;
        let h = self.square_fp2(region, &yz)?;
        let bc = self.add_fp2(region, &b, &c)?;
        let h = self.sub_fp2(region, &h, &bc)?;
        let i = self.sub_fp2(region, &e, &b)?;
        let j = self.square_fp2(region, &p.x)?;
        let e_squared = self.square_fp2(region, &e)?;
        let b_minus_f = self.sub_fp2(region, &b, &f)?;
        let x = self.mul_fp2(region, &a, &b_minus_f)?;
        let g_squared = self.square_fp2(region, &g)?;
        let twice_e_squared = self.add_fp2(region, &e_squared, &e_squared)?;
        let thrice_e_squared = self.add_fp2(region, &twice_e_squared, &e_squared)?;
        let y = self.sub_fp2(region, &g_squared, &thrice_e_squared)?;
        let z = self.mul_fp2(region, &b, &h)?;
        self.require_miller_z(region, &z)?;
        let twice_j = self.add_fp2(region, &j, &j)?;
        let thrice_j = self.add_fp2(region, &twice_j, &j)?;
        let negative_h = self.neg_fp2(region, &h)?;
        Ok((
            MillerG2Value { x, y, z },
            MillerLine {
                coefficients: [i, thrice_j, negative_h],
            },
        ))
    }
    /// One homogeneous addition of the fixed affine base and its M-twist line.
    /// # Errors
    /// Returns layout errors; infinity inputs or a degenerate output are
    /// unsatisfiable. The fixed prime-subgroup Miller schedule excludes them.
    pub fn miller_add(
        &mut self,
        region: &mut Region<'_, F>,
        p: &MillerG2Value<F>,
        q: &G2Value<F>,
    ) -> Result<(MillerG2Value<F>, MillerLine<F>), Error> {
        self.assert_nonidentity_g2(region, q)?;
        let qyz = self.mul_fp2(region, q.y(), &p.z)?;
        let theta = self.sub_fp2(region, &p.y, &qyz)?;
        let qxz = self.mul_fp2(region, q.x(), &p.z)?;
        let lambda = self.sub_fp2(region, &p.x, &qxz)?;
        let c = self.square_fp2(region, &theta)?;
        let d = self.square_fp2(region, &lambda)?;
        let e = self.mul_fp2(region, &lambda, &d)?;
        let f = self.mul_fp2(region, &p.z, &c)?;
        let g = self.mul_fp2(region, &p.x, &d)?;
        let ef = self.add_fp2(region, &e, &f)?;
        let twice_g = self.add_fp2(region, &g, &g)?;
        let h = self.sub_fp2(region, &ef, &twice_g)?;
        let x = self.mul_fp2(region, &lambda, &h)?;
        let gh = self.sub_fp2(region, &g, &h)?;
        let first_y = self.mul_fp2(region, &theta, &gh)?;
        let second_y = self.mul_fp2(region, &e, &p.y)?;
        let y = self.sub_fp2(region, &first_y, &second_y)?;
        let z = self.mul_fp2(region, &p.z, &e)?;
        self.require_miller_z(region, &z)?;
        let j0 = self.mul_fp2(region, &theta, q.x())?;
        let j1 = self.mul_fp2(region, &lambda, q.y())?;
        let j = self.sub_fp2(region, &j0, &j1)?;
        let negative_theta = self.neg_fp2(region, &theta)?;
        Ok((
            MillerG2Value { x, y, z },
            MillerLine {
                coefficients: [j, negative_theta, lambda],
            },
        ))
    }
    /// Multiply an accumulator by the M-twist line evaluated at a nonidentity G1 point.
    /// # Errors
    /// Returns layout errors; infinity is unsatisfiable.
    pub fn miller_evaluate(
        &mut self,
        region: &mut Region<'_, F>,
        acc: &Fp12Value<F>,
        line: &MillerLine<F>,
        p: &G1Value<F>,
    ) -> Result<Fp12Value<F>, Error> {
        self.assert_nonidentity_g1(region, p)?;
        let c1 = self.mul_fp2_by_fp(region, &line.coefficients[1], p.x())?;
        let c2 = self.mul_fp2_by_fp(region, &line.coefficients[2], p.y())?;
        let zero = self.constant_fp2(region, [native::ZERO; 2])?;
        let factor = Fp12Value::from_coefficients([
            Fp6Value::from_coefficients([line.coefficients[0].clone(), c1, zero.clone()]),
            Fp6Value::from_coefficients([zero.clone(), c2, zero]),
        ]);
        self.mul_fp12(region, acc, &factor)
    }
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;

/// Fixed native final-exponent arithmetic schedule for recursive continuations.
#[path = "pairing/final_exponent.rs"]
pub mod final_exponent;

/// Fixed two-pair Miller schedule for recursively linked arithmetic steps.
#[path = "pairing/miller_program.rs"]
pub mod miller_program;
