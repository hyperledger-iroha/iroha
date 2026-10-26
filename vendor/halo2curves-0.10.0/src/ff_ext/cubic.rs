use super::ExtField;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CubicExtField<F: ff::Field> {
    pub(crate) c0: F,
    pub(crate) c1: F,
    pub(crate) c2: F,
}

pub trait CubicSparseMul {
    type Base: ExtField;

    #[must_use]
    fn mul_by_1(lhs: &CubicExtField<Self::Base>, c1: &Self::Base) -> CubicExtField<Self::Base> {
        let b_b = lhs.c1 * c1;

        let t1 = (lhs.c1 + lhs.c2) * c1 - b_b;
        let t1 = t1.mul_by_nonresidue();
        let t2 = (lhs.c0 + lhs.c1) * c1 - b_b;

        CubicExtField {
            c0: t1,
            c1: t2,
            c2: b_b,
        }
    }

    #[must_use]
    fn mul_by_01(
        lhs: &CubicExtField<Self::Base>,
        c0: &Self::Base,
        c1: &Self::Base,
    ) -> CubicExtField<Self::Base> {
        let a_a = lhs.c0 * c0;
        let b_b = lhs.c1 * c1;

        let t1 = *c1 * (lhs.c1 + lhs.c2) - b_b;
        let t1 = a_a + t1.mul_by_nonresidue();
        let t3 = *c0 * (lhs.c0 + lhs.c2) - a_a + b_b;
        let t2 = (*c0 + c1) * (lhs.c0 + lhs.c1) - a_a - b_b;

        CubicExtField {
            c0: t1,
            c1: t2,
            c2: t3,
        }
    }
}

pub trait CubicExtFieldArith {
    type Base: ExtField;

    fn mul_assign(lhs: &mut CubicExtField<Self::Base>, rhs: &CubicExtField<Self::Base>) {
        let a_a = lhs.c0 * rhs.c0;
        let b_b = lhs.c1 * rhs.c1;
        let c_c = lhs.c2 * rhs.c2;

        let t1 = (rhs.c1 + rhs.c2) * (lhs.c1 + lhs.c2) - (c_c + b_b);

        let t1 = a_a + t1.mul_by_nonresidue();

        let t3 = (rhs.c0 + rhs.c2) * (lhs.c0 + lhs.c2) - (a_a - b_b + c_c);

        let t2 = (rhs.c0 + rhs.c1) * (lhs.c0 + lhs.c1) - (a_a + b_b);
        let t2 = t2 + c_c.mul_by_nonresidue();

        lhs.c0 = t1;
        lhs.c1 = t2;
        lhs.c2 = t3;
    }

    fn square_assign(el: &mut CubicExtField<Self::Base>) {
        use ff::Field;

        let s0 = el.c0.square();
        let s1 = (el.c0 * el.c1).double();
        let s2 = (el.c0 - el.c1 + el.c2).square();
        let s3 = (el.c1 * el.c2).double();
        let s4 = el.c2.square();

        el.c0 = s3.mul_by_nonresidue() + s0;
        el.c1 = s4.mul_by_nonresidue() + s1;
        el.c2 = s1 + s2 + s3 - s0 - s4;
    }
}

impl<F: ff::Field> CubicExtField<F> {
    #[inline]
    pub const fn new(c0: F, c1: F, c2: F) -> Self {
        Self { c0, c1, c2 }
    }

    #[inline]
    pub const fn zero() -> Self {
        Self {
            c0: F::ZERO,
            c1: F::ZERO,
            c2: F::ZERO,
        }
    }

    #[inline]
    pub const fn one() -> Self {
        Self {
            c0: F::ONE,
            c1: F::ZERO,
            c2: F::ZERO,
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
    pub fn c2(&self) -> &F {
        &self.c2
    }

    #[inline]
    pub fn double(&self) -> Self {
        Self {
            c0: self.c0.double(),
            c1: self.c1.double(),
            c2: self.c2.double(),
        }
    }

    #[inline]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            c0: self.c0 + other.c0,
            c1: self.c1 + other.c1,
            c2: self.c2 + other.c2,
        }
    }

    #[inline]
    pub fn sub(&self, other: &Self) -> Self {
        Self {
            c0: self.c0 - other.c0,
            c1: self.c1 - other.c1,
            c2: self.c2 - other.c2,
        }
    }

    #[inline]
    pub fn neg(&self) -> Self {
        Self {
            c0: -self.c0,
            c1: -self.c1,
            c2: -self.c2,
        }
    }
}

impl<F: ff::Field> CubicExtField<F>
where
    Self: CubicExtFieldArith<Base = F>,
{
    pub fn mul(&self, rhs: &Self) -> Self {
        let mut lhs = *self;
        Self::mul_assign(&mut lhs, rhs);
        lhs
    }

    pub fn mul_assign(&mut self, rhs: &Self) {
        <Self as CubicExtFieldArith>::mul_assign(self, rhs);
    }

    pub fn square(el: &Self) -> Self {
        let mut el = *el;
        Self::square_assign(&mut el);
        el
    }

    pub fn square_assign(&mut self) {
        <Self as CubicExtFieldArith>::square_assign(self);
    }
}

impl<F: ExtField> ff::Field for CubicExtField<F>
where
    CubicExtField<F>: CubicExtFieldArith<Base = F> + ExtField, /* kind of cyclic being
                                                                * `ExtField: Field` but it seems
                                                                * alright */
{
    const ZERO: Self = Self::zero();
    const ONE: Self = Self::one();

    fn random(mut rng: impl rand_core::RngCore) -> Self {
        Self::new(
            F::random(&mut rng),
            F::random(&mut rng),
            F::random(&mut rng),
        )
    }

    fn is_zero(&self) -> subtle::Choice {
        self.c0.is_zero() & self.c1.is_zero()
    }

    fn square(&self) -> Self {
        CubicExtField::square(self)
    }

    fn double(&self) -> Self {
        self.double()
    }

    fn sqrt(&self) -> subtle::CtOption<Self> {
        unimplemented!()
    }

    fn sqrt_ratio(_: &Self, _: &Self) -> (subtle::Choice, Self) {
        unimplemented!()
    }

    fn invert(&self) -> subtle::CtOption<Self> {
        let c0 = self.c2.mul_by_nonresidue() * self.c1.neg() + self.c0.square();
        let c1 = self.c2.square().mul_by_nonresidue() - (self.c0 * self.c1);
        let c2 = self.c1.square() - (self.c0 * self.c2);

        let t = (self.c2 * c1) + (self.c1 * c2);
        let t = t.mul_by_nonresidue() + (self.c0 * c0);

        t.invert().map(|t| Self {
            c0: t * c0,
            c1: t * c1,
            c2: t * c2,
        })
    }
}

impl<F: ff::Field> subtle::ConditionallySelectable for CubicExtField<F> {
    fn conditional_select(a: &Self, b: &Self, choice: subtle::Choice) -> Self {
        CubicExtField {
            c0: F::conditional_select(&a.c0, &b.c0, choice),
            c1: F::conditional_select(&a.c1, &b.c1, choice),
            c2: F::conditional_select(&a.c2, &b.c2, choice),
        }
    }
}

impl<F: ff::Field> subtle::ConstantTimeEq for CubicExtField<F> {
    fn ct_eq(&self, other: &Self) -> subtle::Choice {
        self.c0.ct_eq(&other.c0) & self.c1.ct_eq(&other.c1) & self.c2.ct_eq(&other.c2)
    }
}
