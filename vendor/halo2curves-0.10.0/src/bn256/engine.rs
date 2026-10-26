#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::{
    borrow::Borrow,
    iter::Sum,
    ops::{Add, Mul, MulAssign, Neg, Sub},
};
use pairing::{Engine, MillerLoopResult, MultiMillerLoop, PairingCurveAffine};
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

use crate::{
    bn256::{curve::*, fq::*, fq12::*, fq2::*, fq6::FROBENIUS_COEFF_FQ6_C1, fr::*},
    ff::PrimeField,
    ff_ext::{quadratic::QuadSparseMul, ExtField},
    group::{cofactor::CofactorCurveAffine, Group},
};

crate::impl_gt!(Gt, Fq12, Fr);
crate::impl_miller_loop_components!(Bn256, G1, G1Affine, G2, G2Affine, Fq12, Gt, Fr);

impl MillerLoopResult for Fq12 {
    type Gt = Gt;

    fn final_exponentiation(&self) -> Self::Gt {
        fn exp_by_x(f: &mut Fq12) {
            let x = super::BN_X;
            let mut res = Fq12::one();
            for i in (0..64).rev() {
                res.cyclotomic_square();
                if ((x >> i) & 1) == 1 {
                    res.mul_assign(f);
                }
            }
            *f = res;
        }

        let r = *self;
        let mut f1 = *self;
        f1.conjugate();

        use ff::Field;
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

pub fn multi_miller_loop(terms: &[(&G1Affine, &G2Affine)]) -> Fq12 {
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

    let mut f = Fq12::one();
    let mut r = terms.iter().map(|(_, q)| q.to_curve()).collect::<Vec<_>>();

    for (i, x) in super::SIX_U_PLUS_2_NAF.iter().rev().skip(1).enumerate() {
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

    const XI_TO_Q_MINUS_1_OVER_2: Fq2 = Fq2 {
        c0: Fq([
            0xe4bbdd0c2936b629,
            0xbb30f162e133bacb,
            0x31a9d1b6f9645366,
            0x253570bea500f8dd,
        ]),
        c1: Fq([
            0xa1d77ce45ffe77c7,
            0x07affd117826d1db,
            0x6d16bd27bb7edc6b,
            0x2c87200285defecc,
        ]),
    };

    for ((p, q), r) in terms.iter().zip(r.iter_mut()) {
        let mut q1: G2Affine = *q;
        q1.x.conjugate();
        q1.x.mul_assign(&FROBENIUS_COEFF_FQ6_C1[1]);
        q1.y.conjugate();
        q1.y.mul_assign(&XI_TO_Q_MINUS_1_OVER_2);
        add(&mut f, r, &q1, p);
    }

    for ((p, q), r) in terms.iter().zip(r.iter_mut()) {
        let mut minusq2: G2Affine = *q;
        minusq2.x.mul_assign(&FROBENIUS_COEFF_FQ6_C1[2]);
        add(&mut f, r, &minusq2, p);
    }

    f
}

// Final steps of the line function on prepared coefficients
fn ell(f: &mut Fq12, coeffs: &(Fq2, Fq2, Fq2), p: &G1Affine) {
    let mut c0 = coeffs.0;
    let mut c1 = coeffs.1;
    c0.c0.mul_assign(&p.y);
    c0.c1.mul_assign(&p.y);
    c1.c0.mul_assign(&p.x);
    c1.c1.mul_assign(&p.x);
    Fq12::mul_by_034(f, &c0, &c1, &coeffs.2);
}

#[cfg(test)]
mod test {
    use ff::Field;
    use group::{prime::PrimeCurveAffine, Curve, Group};
    use pairing::{Engine, MillerLoopResult, PairingCurveAffine};
    use rand_core::OsRng;

    use super::{
        super::{Bn256, Fr, G1, G2},
        multi_miller_loop, Fq12, G1Affine, G2Affine, Gt,
    };
    crate::test_pairing!(Bn256, G1, G1Affine, G2, G2Affine, Fq12, Gt, Fr);
}
