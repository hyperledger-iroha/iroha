//! Custom gates: named polynomial constraints over column queries.
//!
//! A gate holds one or more polynomials, each of which must vanish on every
//! row of the domain. Names are diagnostics only; they are not part of the
//! circuit descriptor.

use core::iter::{Map, Repeat, Zip};

use super::expression::{Any, Column, Expression, Rotation, Selector};

/// A cell queried by a gate, relative to the gate's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualCell {
    /// The queried column.
    pub column: Column<Any>,
    /// The rotation relative to the gate's row.
    pub rotation: Rotation,
}

impl<C: Into<Column<Any>>> From<(C, Rotation)> for VirtualCell {
    fn from((column, rotation): (C, Rotation)) -> Self {
        Self {
            column: column.into(),
            rotation,
        }
    }
}

/// One named polynomial constraint of a gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint<F> {
    name: String,
    poly: Expression<F>,
}

impl<F> Constraint<F> {
    /// The constraint name (may be empty).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The constraint polynomial.
    #[must_use]
    pub const fn polynomial(&self) -> &Expression<F> {
        &self.poly
    }

    /// Splits the constraint into its name and polynomial.
    #[must_use]
    pub fn into_parts(self) -> (String, Expression<F>) {
        (self.name, self.poly)
    }
}

impl<F> From<Expression<F>> for Constraint<F> {
    fn from(poly: Expression<F>) -> Self {
        Self {
            name: String::new(),
            poly,
        }
    }
}

impl<F, S: AsRef<str>> From<(S, Expression<F>)> for Constraint<F> {
    fn from((name, poly): (S, Expression<F>)) -> Self {
        Self {
            name: name.as_ref().to_owned(),
            poly,
        }
    }
}

impl<F> From<Expression<F>> for Vec<Constraint<F>> {
    fn from(poly: Expression<F>) -> Self {
        vec![Constraint::from(poly)]
    }
}

/// Constraints sharing one selector: each constraint `c` becomes
/// `selector * c` (a `Product` with the selector on the left, as in halo2).
#[derive(Clone, Debug)]
pub struct Constraints<F, C: Into<Constraint<F>>, I: IntoIterator<Item = C>> {
    selector: Expression<F>,
    constraints: I,
}

impl<F, C: Into<Constraint<F>>, I: IntoIterator<Item = C>> Constraints<F, C, I> {
    /// Gates every constraint in `constraints` with `selector`.
    pub const fn with_selector(selector: Expression<F>, constraints: I) -> Self {
        Self {
            selector,
            constraints,
        }
    }
}

/// Multiplies one constraint by the shared selector.
fn apply_selector<F, C: Into<Constraint<F>>>(
    (selector, constraint): (Expression<F>, C),
) -> Constraint<F> {
    let constraint: Constraint<F> = constraint.into();
    Constraint {
        name: constraint.name,
        poly: selector * constraint.poly,
    }
}

/// The function type that gates one constraint with the shared selector.
type ApplySelector<F, C> = fn((Expression<F>, C)) -> Constraint<F>;

/// Iterator over selector-gated constraints.
type ConstraintsIter<F, C, I> = Map<Zip<Repeat<Expression<F>>, I>, ApplySelector<F, C>>;

impl<F: Clone, C: Into<Constraint<F>>, I: IntoIterator<Item = C>> IntoIterator
    for Constraints<F, C, I>
{
    type Item = Constraint<F>;
    type IntoIter = ConstraintsIter<F, C, I::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        core::iter::repeat(self.selector)
            .zip(self.constraints)
            .map(apply_selector)
    }
}

/// A gate: named polynomials that must vanish on every row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate<F> {
    name: String,
    constraint_names: Vec<String>,
    polys: Vec<Expression<F>>,
    queried_selectors: Vec<Selector>,
    queried_cells: Vec<VirtualCell>,
}

impl<F> Gate<F> {
    /// Assembles a gate. The constraint system checks that it is non-empty
    /// and that it follows the simple-selector rules.
    pub(crate) fn new(
        name: String,
        constraint_names: Vec<String>,
        polys: Vec<Expression<F>>,
        queried_selectors: Vec<Selector>,
        queried_cells: Vec<VirtualCell>,
    ) -> Self {
        Self {
            name,
            constraint_names,
            polys,
            queried_selectors,
            queried_cells,
        }
    }

    /// The gate name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The name of polynomial `index` (empty when unnamed or out of range).
    #[must_use]
    pub fn constraint_name(&self, index: usize) -> &str {
        self.constraint_names.get(index).map_or("", String::as_str)
    }

    /// The gate polynomials.
    #[must_use]
    pub fn polynomials(&self) -> &[Expression<F>] {
        &self.polys
    }

    /// Selectors queried by the gate, in first-query order.
    #[must_use]
    pub fn queried_selectors(&self) -> &[Selector] {
        &self.queried_selectors
    }

    /// Cells queried by the gate, in first-query order.
    #[must_use]
    pub fn queried_cells(&self) -> &[VirtualCell] {
        &self.queried_cells
    }

    /// Replaces the polynomials (selector substitution), keeping the names.
    pub(crate) fn set_polynomials(&mut self, polys: Vec<Expression<F>>) {
        self.polys = polys;
    }
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use iroha_pasta::Fp;

    use super::*;
    use crate::cs::expression::{Advice, Fixed};

    #[test]
    fn constraint_conversions_keep_names() {
        let poly = Column::new(0, Advice).cur::<Fp>();
        let unnamed = Constraint::from(poly.clone());
        assert_eq!(unnamed.name(), "");
        let named = Constraint::from(("bool", poly.clone()));
        assert_eq!(named.name(), "bool");
        assert_eq!(named.polynomial(), &poly);
        let (name, inner) = named.into_parts();
        assert_eq!((name.as_str(), inner), ("bool", poly.clone()));
        let list: Vec<Constraint<Fp>> = poly.into();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn with_selector_multiplies_on_the_left() {
        let s = Selector::new(0, true).expr::<Fp>();
        let a = Column::new(0, Advice).cur::<Fp>();
        let b = Column::new(1, Advice).cur::<Fp>();
        let gated: Vec<Constraint<Fp>> =
            Constraints::with_selector(s.clone(), [("a", a.clone()), ("b", b.clone())])
                .into_iter()
                .collect();
        assert_eq!(gated.len(), 2);
        assert_eq!(gated[0].name(), "a");
        assert_eq!(gated[0].polynomial(), &(s.clone() * a));
        assert_eq!(gated[1].polynomial(), &(s * b));
    }

    #[test]
    fn gate_accessors() {
        let poly = Column::new(0, Fixed).cur::<Fp>();
        let cell = VirtualCell::from((Column::new(0, Fixed), Rotation::cur()));
        let mut gate = Gate::new(
            "g".to_owned(),
            vec!["c0".to_owned()],
            vec![poly.clone()],
            vec![Selector::new(0, true)],
            vec![cell],
        );
        assert_eq!(gate.name(), "g");
        assert_eq!(gate.constraint_name(0), "c0");
        assert_eq!(gate.constraint_name(9), "");
        assert_eq!(gate.polynomials(), &[poly]);
        assert_eq!(gate.queried_selectors(), &[Selector::new(0, true)]);
        assert_eq!(gate.queried_cells(), &[cell]);
        gate.set_polynomials(vec![Expression::Constant(Fp::ONE)]);
        assert_eq!(gate.polynomials(), &[Expression::Constant(Fp::ONE)]);
    }
}
