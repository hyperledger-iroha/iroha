//! Ordered output for the shared SHA polynomial equations.

use crate::privacy_engines::transparent_stark::PolynomialAirFieldV1;

/// Append-only equations; evaluators never inspect previously emitted values.
pub(super) trait ShaResidueSinkV1<A: PolynomialAirFieldV1> {
    /// Number of equations already emitted by the shared relation.
    fn len(&self) -> usize;
    /// Preserve the collecting API's allocation point without requiring storage.
    fn reserve(&mut self, additional: usize);
    /// Append the next equation in its canonical alpha order.
    fn push(&mut self, value: A);
    /// Append an ordered batch without introducing a second polynomial path.
    fn extend_residues(&mut self, values: impl IntoIterator<Item = A>) {
        for value in values {
            self.push(value);
        }
    }
}

impl<A: PolynomialAirFieldV1> ShaResidueSinkV1<A> for Vec<A> {
    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn reserve(&mut self, additional: usize) {
        Vec::reserve_exact(self, additional);
    }

    fn push(&mut self, value: A) {
        Vec::push(self, value);
    }
}
