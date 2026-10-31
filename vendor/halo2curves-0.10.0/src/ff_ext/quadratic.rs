use ff::Field;
use subtle::{Choice, CtOption};

use super::{
    cubic::{CubicExtField, CubicSparseMul},
    ExtField,
};

pub trait QuadSparseMul {
    type Base: ExtField;

    fn mul_by_014(
        lhs: &mut QuadExtField<CubicExtField<Self::Base>>,
        c0: &Self::Base,
        c1: &Self::Base,
        c4: &Self::Base,
    ) where
        CubicExtField<Self::Base>: CubicSparseMul<Base = Self::Base> + ExtField,
    {
        let aa = CubicExtField::mul_by_01(&lhs.c0, c0, c1);
        let bb = CubicExtField::mul_by_1(&lhs.c1, c4);
        let t0 = &(lhs.c1 + lhs.c0);
        let t1 = *c1 + c4;
        lhs.c1 = CubicExtField::mul_by_01(t0, c0, &t1) - (aa + bb);
        lhs.c0 = bb.mul_by_nonresidue() + aa;
    }

    fn mul_by_034(
        lhs: &mut QuadExtField<CubicExtField<Self::Base>>,
        c0: &Self::Base,
        c3: &Self::Base,
        c4: &Self::Base,
    ) where
        CubicExtField<Self::Base>: CubicSparseMul<Base = Self::Base> + ExtField,
    {
        let t0 = CubicExtField {
            c0: lhs.c0.c0 * c0,
            c1: lhs.c0.c1 * c0,
            c2: lhs.c0.c2 * c0,
        };
        let t1 = CubicExtField::mul_by_01(&lhs.c1, c3, c4);
        let t2 = lhs.c0 + lhs.c1;
        let t3 = *c0 + c3;
        lhs.c1 = CubicExtField::mul_by_01(&t2, &t3, c4) - t0 - t1;
        lhs.c0 = t0 + t1.mul_by_nonresidue();
    }
}

// Algorithm 9 of https://eprint.iacr.org/2012/685.pdf
pub fn sqrt_algo9<F: ExtField, S: AsRef<[u64]>>(
    e: &QuadExtField<F>,
    q_minus_3_over_4: S,
    q_minus_1_over_2: S,
) -> subtle::CtOption<QuadExtField<F>>
where
    QuadExtField<F>: QuadExtFieldArith<Base = F> + ExtField,
{
    if e.is_zero().into() {
        subtle::CtOption::new(QuadExtField::ZERO, subtle::Choice::from(1))
    } else {
        let mut a1 = e.pow(q_minus_3_over_4);

        let alpha = a1.square();
        let alpha = alpha * e;

        let mut a0 = alpha;
        a0.frobenius_map(1);
        let a0 = a0 * alpha;

        let neg1 = QuadExtField::<F> {
            c0: F::ZERO - F::ONE,
            c1: F::ZERO,
        };

        if a0 == neg1 {
            subtle::CtOption::new(a0, subtle::Choice::from(0))
        } else {
            a1.mul_assign(e);

            if alpha == neg1 {
                a1.mul_assign(&QuadExtField::<F> {
                    c0: F::ZERO,
                    c1: F::ONE,
                });
            } else {
                let alpha = alpha + QuadExtField::<F>::ONE;
                let alpha = alpha.pow(q_minus_1_over_2);
                a1.mul_assign(&alpha);
            }
            subtle::CtOption::new(a1, subtle::Choice::from(1))
        }
    }
}

// Algorithm 10 of https://eprint.iacr.org/2012/685.pdf
pub fn sqrt_algo10<F: ExtField, S: AsRef<[u64]>>(
    el: &QuadExtField<F>,
    precompute_e: &QuadExtField<F>,
    precompute_f: &QuadExtField<F>,
    q_minus_1_over_4: S,
) -> subtle::CtOption<QuadExtField<F>>
where
    QuadExtField<F>: QuadExtFieldArith<Base = F> + ExtField,
{
    let b = el.pow_vartime(q_minus_1_over_4);

    let b_2 = b.square();
    let mut b_2_q = b_2;
    b_2_q.frobenius_map(1);

    let a0 = b_2_q * b_2;
    let neg1 = QuadExtField::<F> {
        c0: F::ZERO - F::ONE,
        c1: F::ZERO,
    };

    if a0 == neg1 {
        CtOption::new(a0, Choice::from(0))
    } else {
        let mut x = b;
        x.frobenius_map(1);
        if x * b == QuadExtField::ONE {
            let x0 = (b_2 * el).c0.sqrt().unwrap();
            x.c0.mul_assign(x0);
            x.c1.mul_assign(x0);
            CtOption::new(x, Choice::from(1))
        } else {
            let x0 = (b_2 * precompute_f * el).sqrt().unwrap();
            x *= x0 * precompute_e;
            CtOption::new(x, Choice::from(1))
        }
    }
}

pub enum SQRT<F: Field> {
    Algorithm9 {
        q_minus_3_over_4: &'static [u64],
        q_minus_1_over_2: &'static [u64],
    },
    Algorithm10 {
        precompute_e: QuadExtField<F>,
        precompute_f: QuadExtField<F>,
        q_minus_1_over_4: &'static [u64],
    },
    Unimplemented,
}

pub trait QuadExtFieldArith {
    type Base: ExtField;
    const SQRT: SQRT<Self::Base> = SQRT::Unimplemented;

    fn mul_assign(lhs: &mut QuadExtField<Self::Base>, rhs: &QuadExtField<Self::Base>) {
        let v0 = lhs.c0 * rhs.c0;
        let v1 = lhs.c1 * rhs.c1;
        lhs.c1 = (lhs.c0 + lhs.c1) * (rhs.c0 + rhs.c1) - (v0 + v1);
        lhs.c0 = v0 + v1.mul_by_nonresidue();
    }

    fn square_assign(el: &mut QuadExtField<Self::Base>) {
        let ab = el.c0 * el.c1;
        let c0c1 = el.c0 + el.c1;
        let c0 = (el.c1.mul_by_nonresidue() + el.c0) * c0c1 - ab;
        el.c1 = ab.double();
        el.c0 = c0 - ab.mul_by_nonresidue();
    }
}

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct QuadExtField<F: ff::Field> {
    pub(crate) c0: F,
    pub(crate) c1: F,
}

impl<F: ff::Field> QuadExtField<F> {
    #[inline]
    pub const fn new(c0: F, c1: F) -> Self {
        Self { c0, c1 }
    }

    #[inline]
    pub const fn zero() -> Self {
        Self {
            c0: F::ZERO,
            c1: F::ZERO,
        }
    }

    #[inline]
    pub const fn one() -> Self {
        Self {
            c0: F::ONE,
            c1: F::ZERO,
        }
    }

    #[inline]
    pub fn c0(&self) -> &F {
        &self.c0
    }

    #[inline]
    pub fn c1(&self) -> &F {
        &self.c1
    }

    #[inline]
    pub fn double(&self) -> Self {
        Self {
            c0: self.c0.double(),
            c1: self.c1.double(),
        }
    }

    #[inline]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            c0: self.c0 + other.c0,
            c1: self.c1 + other.c1,
        }
    }

    #[inline]
    pub fn sub(&self, other: &Self) -> Self {
        Self {
            c0: self.c0 - other.c0,
            c1: self.c1 - other.c1,
        }
    }

    #[inline]
    pub fn neg(&self) -> Self {
        Self {
            c0: -self.c0,
            c1: -self.c1,
        }
    }

    #[inline]
    pub fn conjugate(&mut self) {
        self.c1 = -self.c1;
    }
}

impl<F: ExtField> QuadExtField<F>
where
    Self: QuadExtFieldArith<Base = F>,
{
    pub fn mul(&self, rhs: &Self) -> Self {
        let mut lhs = *self;
        Self::mul_assign(&mut lhs, rhs);
        lhs
    }

    pub fn mul_assign(&mut self, rhs: &Self) {
        <Self as QuadExtFieldArith>::mul_assign(self, rhs);
    }

    pub fn square(el: &Self) -> Self {
        let mut el = *el;
        Self::square_assign(&mut el);
        el
    }

    pub fn square_assign(&mut self) {
        <Self as QuadExtFieldArith>::square_assign(self);
    }

    pub fn norm(&self) -> F {
        self.c0.square() - self.c1.square().mul_by_nonresidue()
    }
}

impl<F: ExtField> Field for QuadExtField<F>
where
    QuadExtField<F>: QuadExtFieldArith<Base = F> + ExtField,
{
    const ZERO: Self = Self::zero();
    const ONE: Self = Self::one();

    fn random(mut rng: impl rand_core::RngCore) -> Self {
        Self::new(F::random(&mut rng), F::random(&mut rng))
    }

    fn is_zero(&self) -> subtle::Choice {
        self.c0.is_zero() & self.c1.is_zero()
    }

    fn square(&self) -> Self {
        QuadExtField::square(self)
    }

    fn double(&self) -> Self {
        self.double()
    }

    fn sqrt(&self) -> subtle::CtOption<Self> {
        match Self::SQRT {
            SQRT::Algorithm9 {
                q_minus_3_over_4,
                q_minus_1_over_2,
            } => sqrt_algo9(self, q_minus_3_over_4, q_minus_1_over_2),
            SQRT::Algorithm10 {
                precompute_e,
                precompute_f,
                q_minus_1_over_4,
            } => sqrt_algo10(self, &precompute_e, &precompute_f, q_minus_1_over_4),
            SQRT::Unimplemented => unimplemented!(),
        }
    }

    fn sqrt_ratio(_: &Self, _: &Self) -> (subtle::Choice, Self) {
        unimplemented!()
    }

    fn invert(&self) -> subtle::CtOption<Self> {
        self.norm().invert().map(|t| Self {
            c0: self.c0 * t,
            c1: self.c1 * -t,
        })
    }
}

impl<F: ff::Field> subtle::ConditionallySelectable for QuadExtField<F> {
    fn conditional_select(a: &Self, b: &Self, choice: subtle::Choice) -> Self {
        QuadExtField {
            c0: F::conditional_select(&a.c0, &b.c0, choice),
            c1: F::conditional_select(&a.c1, &b.c1, choice),
        }
    }
}

impl<F: ff::Field> subtle::ConstantTimeEq for QuadExtField<F> {
    fn ct_eq(&self, other: &Self) -> subtle::Choice {
        self.c0.ct_eq(&other.c0) & self.c1.ct_eq(&other.c1)
    }
}
