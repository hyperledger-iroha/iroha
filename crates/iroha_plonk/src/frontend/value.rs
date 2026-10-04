//! Witness values: [`Value`], a value that may be unknown (key generation),
//! and [`Assigned`], a cell value kept as a fraction so the backend can
//! batch-invert denominators.
//!
//! Both are ports of the halo2 types with the same semantics: arithmetic on an
//! unknown value is unknown, and a fraction with a zero denominator evaluates
//! to zero.

use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use iroha_pasta::PastaField;

use super::assignment::Error;

/// A value that is known during proving and unknown during key generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Value<V> {
    inner: Option<V>,
}

impl<V> Value<V> {
    /// An unknown value.
    #[must_use]
    pub const fn unknown() -> Self {
        Self { inner: None }
    }

    /// A known value.
    #[must_use]
    pub const fn known(value: V) -> Self {
        Self { inner: Some(value) }
    }

    /// Whether the value is known.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        self.inner.is_some()
    }

    /// The value, or [`Error::Synthesis`] when it is unknown.
    pub(crate) fn assign(self) -> Result<V, Error> {
        self.inner.ok_or(Error::Synthesis)
    }

    /// Borrows the value.
    #[must_use]
    pub const fn as_ref(&self) -> Value<&V> {
        Value {
            inner: self.inner.as_ref(),
        }
    }

    /// Mutably borrows the value.
    #[must_use]
    pub fn as_mut(&mut self) -> Value<&mut V> {
        Value {
            inner: self.inner.as_mut(),
        }
    }

    /// Returns [`Error::Synthesis`] when the value is known and `predicate`
    /// holds for it; an unknown value passes.
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] as described.
    pub fn error_if_known_and(&self, predicate: impl FnOnce(&V) -> bool) -> Result<(), Error> {
        match &self.inner {
            Some(value) if predicate(value) => Err(Error::Synthesis),
            _ => Ok(()),
        }
    }

    /// Maps a known value.
    #[must_use]
    pub fn map<W>(self, f: impl FnOnce(V) -> W) -> Value<W> {
        Value {
            inner: self.inner.map(f),
        }
    }

    /// Chains a computation that may itself produce an unknown value.
    #[must_use]
    pub fn and_then<W>(self, f: impl FnOnce(V) -> Value<W>) -> Value<W> {
        self.inner.map_or_else(Value::unknown, f)
    }

    /// Pairs two values; the pair is known when both are.
    #[must_use]
    pub fn zip<W>(self, other: Value<W>) -> Value<(V, W)> {
        Value {
            inner: self.inner.zip(other.inner),
        }
    }
}

impl<V, W> Value<(V, W)> {
    /// Splits a pair.
    #[must_use]
    pub fn unzip(self) -> (Value<V>, Value<W>) {
        match self.inner {
            Some((left, right)) => (Value::known(left), Value::known(right)),
            None => (Value::unknown(), Value::unknown()),
        }
    }
}

impl<V: Copy> Value<&V> {
    /// Copies a borrowed value.
    #[must_use]
    pub fn copied(self) -> Value<V> {
        Value {
            inner: self.inner.copied(),
        }
    }
}

impl<V: Clone> Value<&V> {
    /// Clones a borrowed value.
    #[must_use]
    pub fn cloned(self) -> Value<V> {
        Value {
            inner: self.inner.cloned(),
        }
    }
}

impl<V: Copy, const LEN: usize> Value<[V; LEN]> {
    /// Splits a known array into known elements (all unknown otherwise).
    #[must_use]
    pub fn transpose_array(self) -> [Value<V>; LEN] {
        self.inner.map_or_else(
            || [Value::unknown(); LEN],
            |values| values.map(Value::known),
        )
    }
}

impl<V, I: IntoIterator<Item = V>> Value<I> {
    /// Splits a known collection into `length` known elements (all unknown
    /// otherwise).
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] when a known collection does not have `length`
    /// elements.
    pub fn transpose_vec(self, length: usize) -> Result<Vec<Value<V>>, Error> {
        self.inner.map_or_else(
            || Ok((0..length).map(|_| Value::unknown()).collect()),
            |values| {
                let values: Vec<Value<V>> = values.into_iter().map(Value::known).collect();
                if values.len() == length {
                    Ok(values)
                } else {
                    Err(Error::Synthesis)
                }
            },
        )
    }
}

impl<A, V: FromIterator<A>> FromIterator<Value<A>> for Value<V> {
    fn from_iter<I: IntoIterator<Item = Value<A>>>(iter: I) -> Self {
        Self {
            inner: iter.into_iter().map(|value| value.inner).collect(),
        }
    }
}

impl<V: Neg> Neg for Value<V> {
    type Output = Value<V::Output>;

    fn neg(self) -> Self::Output {
        self.map(Neg::neg)
    }
}

impl<V: Add<W>, W> Add<Value<W>> for Value<V> {
    type Output = Value<V::Output>;

    fn add(self, rhs: Value<W>) -> Self::Output {
        self.zip(rhs).map(|(a, b)| a + b)
    }
}

impl<V: Sub<W>, W> Sub<Value<W>> for Value<V> {
    type Output = Value<V::Output>;

    fn sub(self, rhs: Value<W>) -> Self::Output {
        self.zip(rhs).map(|(a, b)| a - b)
    }
}

impl<V: Mul<W>, W> Mul<Value<W>> for Value<V> {
    type Output = Value<V::Output>;

    fn mul(self, rhs: Value<W>) -> Self::Output {
        self.zip(rhs).map(|(a, b)| a * b)
    }
}

impl<F: PastaField> Value<F> {
    /// Converts a field value into an assigned value.
    #[must_use]
    pub fn into_field(self) -> Value<Assigned<F>> {
        self.map(Assigned::Trivial)
    }
}

impl<F: PastaField> Value<Assigned<F>> {
    /// Evaluates the fraction (one unbatched inversion).
    #[must_use]
    pub fn evaluate(self) -> Value<F> {
        self.map(Assigned::evaluate)
    }
}

impl<F: PastaField> From<Value<F>> for Value<Assigned<F>> {
    fn from(value: Value<F>) -> Self {
        value.into_field()
    }
}

/// A cell value kept as a fraction; `x / 0` evaluates to zero.
#[derive(Clone, Copy, Debug)]
pub enum Assigned<F> {
    /// Zero.
    Zero,
    /// A value that needs no inversion.
    Trivial(F),
    /// `numerator / denominator`.
    Rational(F, F),
}

impl<F: PastaField> From<F> for Assigned<F> {
    fn from(value: F) -> Self {
        Self::Trivial(value)
    }
}

impl<F: PastaField> From<&F> for Assigned<F> {
    fn from(value: &F) -> Self {
        Self::Trivial(*value)
    }
}

impl<F: PastaField> From<&Self> for Assigned<F> {
    fn from(value: &Self) -> Self {
        *value
    }
}

impl<F: PastaField> From<(F, F)> for Assigned<F> {
    fn from((numerator, denominator): (F, F)) -> Self {
        Self::Rational(numerator, denominator)
    }
}

impl<F: PastaField> PartialEq for Assigned<F> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Zero, Self::Zero) => true,
            (Self::Zero, x) | (x, Self::Zero) => x.is_zero_vartime(),
            (Self::Rational(_, denominator), x) | (x, Self::Rational(_, denominator))
                if denominator.is_zero_vartime() =>
            {
                x.is_zero_vartime()
            }
            (Self::Trivial(left), Self::Trivial(right)) => left == right,
            (Self::Trivial(x), Self::Rational(numerator, denominator))
            | (Self::Rational(numerator, denominator), Self::Trivial(x)) => {
                *x * denominator == *numerator
            }
            (
                Self::Rational(left_numerator, left_denominator),
                Self::Rational(right_numerator, right_denominator),
            ) => *left_numerator * right_denominator == *left_denominator * right_numerator,
        }
    }
}

impl<F: PastaField> Eq for Assigned<F> {}

impl<F: PastaField> Neg for Assigned<F> {
    type Output = Self;

    fn neg(self) -> Self {
        match self {
            Self::Zero => Self::Zero,
            Self::Trivial(value) => Self::Trivial(-value),
            Self::Rational(numerator, denominator) => Self::Rational(-numerator, denominator),
        }
    }
}

impl<F: PastaField> Add for Assigned<F> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Zero, _) => rhs,
            (_, Self::Zero) => self,
            (Self::Rational(_, denominator), other) | (other, Self::Rational(_, denominator))
                if denominator.is_zero_vartime() =>
            {
                other
            }
            (Self::Trivial(left), Self::Trivial(right)) => Self::Trivial(left + right),
            (Self::Rational(numerator, denominator), Self::Trivial(other))
            | (Self::Trivial(other), Self::Rational(numerator, denominator)) => {
                Self::Rational(numerator + denominator * other, denominator)
            }
            (
                Self::Rational(left_numerator, left_denominator),
                Self::Rational(right_numerator, right_denominator),
            ) => Self::Rational(
                left_numerator * right_denominator + left_denominator * right_numerator,
                left_denominator * right_denominator,
            ),
        }
    }
}

impl<F: PastaField> Add<F> for Assigned<F> {
    type Output = Self;

    fn add(self, rhs: F) -> Self {
        self + Self::Trivial(rhs)
    }
}

impl<F: PastaField> AddAssign for Assigned<F> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<F: PastaField> Sub for Assigned<F> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl<F: PastaField> Sub<F> for Assigned<F> {
    type Output = Self;

    fn sub(self, rhs: F) -> Self {
        self + (-rhs)
    }
}

impl<F: PastaField> SubAssign for Assigned<F> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl<F: PastaField> Mul for Assigned<F> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Zero, _) | (_, Self::Zero) => Self::Zero,
            (Self::Trivial(left), Self::Trivial(right)) => Self::Trivial(left * right),
            (Self::Rational(numerator, denominator), Self::Trivial(other))
            | (Self::Trivial(other), Self::Rational(numerator, denominator)) => {
                Self::Rational(numerator * other, denominator)
            }
            (
                Self::Rational(left_numerator, left_denominator),
                Self::Rational(right_numerator, right_denominator),
            ) => Self::Rational(
                left_numerator * right_numerator,
                left_denominator * right_denominator,
            ),
        }
    }
}

impl<F: PastaField> Mul<F> for Assigned<F> {
    type Output = Self;

    fn mul(self, rhs: F) -> Self {
        self * Self::Trivial(rhs)
    }
}

impl<F: PastaField> MulAssign for Assigned<F> {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl<F: PastaField> Assigned<F> {
    /// The numerator.
    #[must_use]
    pub fn numerator(&self) -> F {
        match self {
            Self::Zero => F::ZERO,
            Self::Trivial(value) | Self::Rational(value, _) => *value,
        }
    }

    /// The denominator, if the value is a fraction.
    #[must_use]
    pub const fn denominator(&self) -> Option<F> {
        match self {
            Self::Zero | Self::Trivial(_) => None,
            Self::Rational(_, denominator) => Some(*denominator),
        }
    }

    /// Whether the value is zero (`x / 0` is zero).
    #[must_use]
    pub fn is_zero_vartime(&self) -> bool {
        match self {
            Self::Zero => true,
            Self::Trivial(value) => value.is_zero_vartime(),
            Self::Rational(numerator, denominator) => {
                numerator.is_zero_vartime() || denominator.is_zero_vartime()
            }
        }
    }

    /// Doubles the value.
    #[must_use]
    pub fn double(&self) -> Self {
        match self {
            Self::Zero => Self::Zero,
            Self::Trivial(value) => Self::Trivial(value.double()),
            Self::Rational(numerator, denominator) => {
                Self::Rational(numerator.double(), *denominator)
            }
        }
    }

    /// Squares the value.
    #[must_use]
    pub fn square(&self) -> Self {
        match self {
            Self::Zero => Self::Zero,
            Self::Trivial(value) => Self::Trivial(value.square()),
            Self::Rational(numerator, denominator) => {
                Self::Rational(numerator.square(), denominator.square())
            }
        }
    }

    /// Cubes the value.
    #[must_use]
    pub fn cube(&self) -> Self {
        self.square() * *self
    }

    /// Inverts the value (the inverse of zero is zero).
    #[must_use]
    pub fn invert(&self) -> Self {
        match self {
            Self::Zero => Self::Zero,
            Self::Trivial(value) => Self::Rational(F::ONE, *value),
            Self::Rational(numerator, denominator) => Self::Rational(*denominator, *numerator),
        }
    }

    /// Evaluates the value with one constant-time inversion when needed.
    #[must_use]
    pub fn evaluate(self) -> F {
        match self {
            Self::Zero => F::ZERO,
            Self::Trivial(value) => value,
            Self::Rational(numerator, denominator) => {
                if denominator == F::ONE {
                    numerator
                } else {
                    numerator * denominator.invert().unwrap_or(F::ZERO)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use iroha_pasta::Fp;
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    use super::*;

    #[test]
    fn value_combinators() {
        let a = Value::known(Fp::from(3));
        let b = Value::known(Fp::from(4));
        let unknown = Value::<Fp>::unknown();
        assert_eq!((a + b).assign(), Ok(Fp::from(7)));
        assert_eq!((a - b).assign(), Ok(-Fp::ONE));
        assert_eq!((a * b).assign(), Ok(Fp::from(12)));
        assert_eq!((-a).assign(), Ok(-Fp::from(3)));
        assert!(!(a + unknown).is_known());
        assert_eq!(unknown.assign(), Err(Error::Synthesis));
        assert_eq!(a.as_ref().copied(), a);
        assert_eq!(a.as_ref().cloned(), a);
        let mut c = a;
        if let Some(value) = c.as_mut().inner {
            *value = Fp::from(9);
        }
        assert_eq!(c, Value::known(Fp::from(9)));
        assert_eq!(
            a.and_then(|x| Value::known(x.double())),
            Value::known(Fp::from(6))
        );
        assert!(!unknown.and_then(Value::known).is_known());
        let (left, right) = a.zip(b).unzip();
        assert_eq!((left, right), (a, b));
        let (left, right) = a.zip(unknown).unzip();
        assert!(!left.is_known() && !right.is_known());
        assert!(a.error_if_known_and(|x| *x == Fp::from(3)).is_err());
        assert!(unknown.error_if_known_and(|_| true).is_ok());
        assert!(Value::<Fp>::default() == unknown);
    }

    #[test]
    fn transpose_and_collect() {
        let known = Value::known([Fp::ONE, Fp::ZERO]);
        assert_eq!(
            known.transpose_array(),
            [Value::known(Fp::ONE), Value::known(Fp::ZERO)]
        );
        assert!(!Value::<[Fp; 2]>::unknown().transpose_array()[1].is_known());
        let vec = Value::known(vec![Fp::ONE, Fp::ONE]);
        assert_eq!(vec.clone().transpose_vec(2).expect("length").len(), 2);
        assert_eq!(vec.transpose_vec(3), Err(Error::Synthesis));
        let unknown = Value::<Vec<Fp>>::unknown()
            .transpose_vec(4)
            .expect("unknown");
        assert!(unknown.iter().all(|value| !value.is_known()));
        let collected: Value<Vec<Fp>> = [Value::known(Fp::ONE), Value::known(Fp::ZERO)]
            .into_iter()
            .collect();
        assert_eq!(collected, Value::known(vec![Fp::ONE, Fp::ZERO]));
        let partial: Value<Vec<Fp>> = [Value::known(Fp::ONE), Value::unknown()]
            .into_iter()
            .collect();
        assert!(!partial.is_known());
    }

    #[test]
    fn assigned_semantics_match_halo2() {
        let two = Assigned::Trivial(Fp::from(2));
        let half = Assigned::Rational(Fp::ONE, Fp::from(2));
        let inv_zero = Assigned::Rational(Fp::ONE, Fp::ZERO);
        assert_eq!(two * half, Assigned::Trivial(Fp::ONE));
        assert_eq!((two + inv_zero).evaluate(), Fp::from(2));
        assert_eq!(inv_zero.evaluate(), Fp::ZERO);
        assert_eq!(inv_zero, Assigned::Zero);
        assert!(inv_zero.is_zero_vartime());
        assert_eq!((half + half).evaluate(), Fp::ONE);
        assert_eq!((half - half).evaluate(), Fp::ZERO);
        assert_eq!((two - Fp::ONE).evaluate(), Fp::ONE);
        assert_eq!((two + Fp::ONE).evaluate(), Fp::from(3));
        assert_eq!((half * Fp::from(4)).evaluate(), Fp::from(2));
        assert_eq!(two.invert(), half);
        assert_eq!(half.invert(), two);
        assert_eq!(Assigned::<Fp>::Zero.invert(), Assigned::Zero);
        assert_eq!(half.square().evaluate(), Fp::from(4).invert().unwrap());
        assert_eq!(two.cube().evaluate(), Fp::from(8));
        assert_eq!(half.double().evaluate(), Fp::ONE);
        assert_eq!(Assigned::<Fp>::Zero.double(), Assigned::Zero);
        assert_eq!(Assigned::<Fp>::Zero.square(), Assigned::Zero);
        assert_eq!(half.numerator(), Fp::ONE);
        assert_eq!(half.denominator(), Some(Fp::from(2)));
        assert_eq!(two.denominator(), None);
        assert_eq!(Assigned::<Fp>::Zero.numerator(), Fp::ZERO);
        assert_eq!(-half, Assigned::Rational(-Fp::ONE, Fp::from(2)));
        assert_eq!(-Assigned::<Fp>::Zero, Assigned::Zero);
        assert_eq!(-two, Assigned::Trivial(-Fp::from(2)));
        assert_eq!(Assigned::from((Fp::ONE, Fp::ONE)).evaluate(), Fp::ONE);
        assert_eq!(Assigned::from(&Fp::ONE), Assigned::Trivial(Fp::ONE));
        assert_eq!(Assigned::from(&two), two);
        let mut acc = two;
        acc += half;
        acc -= half;
        acc *= half;
        assert_eq!(acc.evaluate(), Fp::ONE);
        assert_eq!(
            Assigned::Rational(Fp::from(2), Fp::from(4)),
            Assigned::Rational(Fp::ONE, Fp::from(2))
        );
        assert_eq!(Assigned::Zero, Assigned::Trivial(Fp::ZERO));
        assert_ne!(two, half);
    }

    #[test]
    fn assigned_arithmetic_agrees_with_field_arithmetic() {
        let mut rng = ChaCha20Rng::from_seed([7; 32]);
        for _ in 0..200 {
            let values: [Fp; 4] = core::array::from_fn(|_| Fp::random(&mut rng));
            let a = Assigned::Rational(values[0], values[1]);
            let b = Assigned::Rational(values[2], values[3]);
            let ea = values[0] * values[1].invert().unwrap();
            let eb = values[2] * values[3].invert().unwrap();
            assert_eq!((a + b).evaluate(), ea + eb);
            assert_eq!((a - b).evaluate(), ea - eb);
            assert_eq!((a * b).evaluate(), ea * eb);
            assert_eq!(a.invert().evaluate(), ea.invert().unwrap());
        }
    }

    #[test]
    fn value_assigned_conversions() {
        let value: Value<Assigned<Fp>> = Value::known(Fp::from(5)).into();
        assert_eq!(value.evaluate(), Value::known(Fp::from(5)));
        assert_eq!(
            Value::known(Fp::ONE).into_field(),
            Value::known(Assigned::Trivial(Fp::ONE))
        );
    }
}
