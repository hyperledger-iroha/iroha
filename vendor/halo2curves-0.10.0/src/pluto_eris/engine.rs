#![allow(clippy::suspicious_arithmetic_impl)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::{
    borrow::Borrow,
    iter::Sum,
    ops::{Add, Mul, MulAssign, Neg, Sub},
};

use ff::Field;
use pairing::{Engine, MillerLoopResult, MultiMillerLoop, PairingCurveAffine};
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

use crate::{
    ff::PrimeField,
    ff_ext::{quadratic::QuadSparseMul, ExtField},
    group::{cofactor::CofactorCurveAffine, Group},
    pluto_eris::{curve::*, fp::*, fp12::*, fp2::*, fp6::FROBENIUS_COEFF_FP6_C1, fq::Fq},
};

/// Adaptation of Algorithm 1, https://eprint.iacr.org/2013/722.pdf
/// the parameter for the curve Pluto: u = -0x4000000000001000008780000000
const NEG_PLUTO_U: u128 = 0x4000000000001000008780000000;

const NEG_SIX_U_PLUS_2_NAF: [i8; 114] = [
    0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 1, 0, -1, 0, -1, 0, 1, 0, -1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    -1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 1,
];

crate::impl_gt!(Gt, Fp12, Fq);
crate::impl_miller_loop_components!(Pluto, G1, G1Affine, G2, G2Affine, Fp12, Gt, Fq);

pub fn multi_miller_loop(terms: &[(&G1Affine, &G2Affine)]) -> Fp12 {
    let terms = terms
        .iter()
        .filter_map(|&(p, q)| {
            if bool::from(p.is_identity()) || bool::from(q.is_identity()) {
                None
            } else {
                Some((*p, *q))
            }
        })
        .collect::<Vec<_>>();

    let mut f = Fp12::one();
    let mut r = terms.iter().map(|(_, q)| q.to_curve()).collect::<Vec<_>>();

    for (i, x) in NEG_SIX_U_PLUS_2_NAF.iter().rev().skip(1).enumerate() {
        (i != 0).then(|| f.square_assign());

        for ((p, _), r) in terms.iter().zip(r.iter_mut()) {
            double(&mut f, r, p);
        }

        match x {
            &val @ (1 | -1) => {
                for ((p, q), r) in terms.iter().zip(r.iter_mut()) {
                    if val == 1 {
                        add(&mut f, r, q, p);
                    } else {
                        add(&mut f, r, &q.neg(), p);
                    }
                }
            }
            _ => continue,
        }
    }

    /// Value of (57/(u + 3))^((p - 1)/2) where u^2 + 5 = 0 in Fp2.
    const XI_TO_P_MINUS_1_OVER_2: Fp2 = Fp2 {
        c0: Fp::from_raw([
            0x54cf5ad1c0926216,
            0x186c1f3ce4a46d4e,
            0x9c23800ce9c9452f,
            0x50e0d09ff6d6c08b,
            0x7cf421e4d46f6666,
            0x678664ba4b6d8343,
            0x21cc26d5de0f80f4,
        ]),

        c1: Fp::from_raw([
            0xc0505f4c260e91f4,
            0xe7bbd15f10723657,
            0xb4b3e0c35358097e,
            0x87c56f42a558750d,
            0x4b7211d23f34f0ae,
            0xf6839d29e2f0d250,
            0x16ebe8b2e12a1106,
        ]),
    };

    for ((p, q), r) in terms.iter().zip(r.iter_mut()) {
        let mut q1: G2Affine = *q;
        q1.x.conjugate();
        q1.x.mul_assign(&FROBENIUS_COEFF_FP6_C1[1]);
        q1.y.conjugate();
        q1.y.mul_assign(&XI_TO_P_MINUS_1_OVER_2);
        add(&mut f, r, &q1.neg(), p);
    }

    for ((p, q), r) in terms.iter().zip(r.iter_mut()) {
        let mut minusq2: G2Affine = *q;
        minusq2.x.mul_assign(&FROBENIUS_COEFF_FP6_C1[2]);
        add(&mut f, r, &minusq2.neg(), p);
    }

    f
}

impl MillerLoopResult for Fp12 {
    type Gt = Gt;

    fn final_exponentiation(&self) -> Gt {
        fn exp_by_x(f: &mut Fp12) {
            let x = NEG_PLUTO_U;
            let mut res = Fp12::one();
            for i in (0..111).rev() {
                res.cyclotomic_square();
                if ((x >> i) & 1) == 1 {
                    res.mul_assign(f);
                }
            }
            res.conjugate();
            *f = res;
        }

        let r = *self;
        let mut f1 = *self;
        f1.conjugate();

        Gt(r.invert()
            .map(|mut f2| {
                let mut r = f1;
                r.mul_assign(&f2);
                f2 = r;
                r.frobenius_map(2);
                r.mul_assign(&f2);

                let mut fp = r;
                fp.frobenius_map(1);

                let mut fp2 = r;
                fp2.frobenius_map(2);
                let mut fp3 = fp2;
                fp3.frobenius_map(1);

                let mut fu = r;
                exp_by_x(&mut fu);

                let mut fu2 = fu;
                exp_by_x(&mut fu2);

                let mut fu3 = fu2;
                exp_by_x(&mut fu3);

                let mut y3 = fu;
                y3.frobenius_map(1);

                let mut fu2p = fu2;
                fu2p.frobenius_map(1);

                let mut fu3p = fu3;
                fu3p.frobenius_map(1);

                let mut y2 = fu2;
                y2.frobenius_map(2);

                let mut y0 = fp;
                y0.mul_assign(&fp2);
                y0.mul_assign(&fp3);

                let mut y1 = r;
                y1.conjugate();

                let mut y5 = fu2;
                y5.conjugate();

                y3.conjugate();

                let mut y4 = fu;
                y4.mul_assign(&fu2p);
                y4.conjugate();

                let mut y6 = fu3;
                y6.mul_assign(&fu3p);
                y6.conjugate();

                y6.cyclotomic_square();
                y6.mul_assign(&y4);
                y6.mul_assign(&y5);

                let mut t1 = y3;
                t1.mul_assign(&y5);
                t1.mul_assign(&y6);

                y6.mul_assign(&y2);

                t1.cyclotomic_square();
                t1.mul_assign(&y6);
                t1.cyclotomic_square();

                let mut t0 = t1;
                t0.mul_assign(&y1);

                t1.mul_assign(&y0);

                t0.cyclotomic_square();
                t0.mul_assign(&t1);

                t0
            })
            .unwrap())
    }
}

// Final steps of the line function on prepared coefficients
fn ell(f: &mut Fp12, coeffs: &(Fp2, Fp2, Fp2), p: &G1Affine) {
    let mut c0 = coeffs.0;
    let mut c1 = coeffs.1;
    c0.c0.mul_assign(&p.y);
    c0.c1.mul_assign(&p.y);
    c1.c0.mul_assign(&p.x);
    c1.c1.mul_assign(&p.x);
    Fp12::mul_by_034(f, &c0, &c1, &coeffs.2);
}

#[cfg(test)]
mod test {
    use ff::Field;
    use group::{prime::PrimeCurveAffine, Curve, Group};
    use pairing::{Engine, MillerLoopResult, PairingCurveAffine};
    use rand_core::OsRng;

    use super::{
        super::{Fq, Pluto, G1, G2},
        multi_miller_loop, Fp12, G1Affine, G2Affine, Gt,
    };
    crate::test_pairing!(Pluto, G1, G1Affine, G2, G2Affine, Fp12, Gt, Fq);
}
