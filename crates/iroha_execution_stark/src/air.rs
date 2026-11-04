//! Compiled constraint evaluation shared by the prover (base field, over the quotient domain) and
//! the verifier (extension field, at the out-of-domain point).
//!
//! A row buffer is two consecutive rows of `[main | public]`: the current row then the next row, so
//! a window of a row-major tile is a buffer without copying. Auxiliary (LogUp) values are extension
//! elements: `helpers[0..h] | table helper | accumulator`.

use std::ops::{Add, Mul, Neg, Sub};

use crate::circuit::{Circuit, Lin, Var};
use crate::field::{Fp, Fp4};

/// Field operations needed to evaluate constraints.
pub trait Element:
    Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
    + Send
    + Sync
{
    /// Zero.
    const ZERO: Self;
    /// Embeds a base element.
    #[must_use]
    fn from_fp(value: Fp) -> Self;
    /// Multiplies by a base element.
    #[must_use]
    fn mul_fp(self, value: Fp) -> Self;
    /// Embeds into the extension.
    fn to_fp4(self) -> Fp4;
    /// `alpha * self` in the extension.
    fn scale_ext(self, alpha: Fp4) -> Fp4;
    /// `h * prod_i(beta - a_i) - sum_i prod_{j != i}(beta - a_j)` for up to four values.
    fn helper_identity(helper: Fp4, values: &[Self], beta: &BetaPowers) -> Fp4;
}

/// `beta^0..=beta^4` for the LogUp helper identities.
#[derive(Clone, Copy, Debug)]
pub struct BetaPowers(pub [Fp4; 5]);

impl BetaPowers {
    /// Powers of `beta`.
    #[must_use]
    pub fn new(beta: Fp4) -> Self {
        let mut powers = [Fp4::ONE; 5];
        for i in 1..5 {
            powers[i] = powers[i - 1] * beta;
        }
        Self(powers)
    }
}

/// Generic helper identity (extension-valued lookups).
fn helper_identity_generic(helper: Fp4, values: &[Fp4], beta: Fp4) -> Fp4 {
    let mut buffer = [Fp4::ZERO; 4];
    for (slot, a) in buffer.iter_mut().zip(values) {
        *slot = beta - *a;
    }
    match &buffer[..values.len()] {
        [d0] => helper * *d0 - Fp4::ONE,
        [d0, d1] => helper * *d0 * *d1 - (*d0 + *d1),
        [d0, d1, d2] => helper * *d0 * *d1 * *d2 - (*d1 * *d2 + *d0 * *d2 + *d0 * *d1),
        [d0, d1, d2, d3] => {
            let (p01, p23) = (*d0 * *d1, *d2 * *d3);
            helper * p01 * p23 - (p23 * (*d0 + *d1) + p01 * (*d2 + *d3))
        }
        _ => unreachable!("helper sizes are 1..=4"),
    }
}

impl Element for Fp {
    const ZERO: Self = Fp::ZERO;
    #[inline]
    fn from_fp(value: Fp) -> Self {
        value
    }
    #[inline]
    fn mul_fp(self, value: Fp) -> Self {
        self * value
    }
    #[inline]
    fn to_fp4(self) -> Fp4 {
        Fp4::from_base(self)
    }
    #[inline]
    fn scale_ext(self, alpha: Fp4) -> Fp4 {
        alpha.mul_base(self)
    }
    /// Base-field lookups: with elementary symmetric sums `e_k` of the values,
    /// `prod(beta - a_i) = sum_k (-1)^k e_k beta^(m-k)` and the numerator is its derivative in `beta`.
    #[inline]
    fn helper_identity(helper: Fp4, values: &[Self], beta: &BetaPowers) -> Fp4 {
        let b = &beta.0;
        let m = values.len();
        // e[k] for k = 0..=m.
        let mut e = [Fp::ZERO; 5];
        e[0] = Fp::ONE;
        for (i, a) in values.iter().enumerate() {
            for k in (1..=i + 1).rev() {
                e[k] += e[k - 1] * *a;
            }
        }
        let mut product = Fp4::ZERO;
        let mut derivative = Fp4::ZERO;
        for k in 0..=m {
            let coefficient = if k % 2 == 0 { e[k] } else { -e[k] };
            let degree = m - k;
            product += b[degree].mul_base(coefficient);
            if degree > 0 {
                derivative += b[degree - 1].mul_base(coefficient * Fp::new(degree as u64));
            }
        }
        helper * product - derivative
    }
}

impl Element for Fp4 {
    const ZERO: Self = Fp4::ZERO;
    #[inline]
    fn from_fp(value: Fp) -> Self {
        Fp4::from_base(value)
    }
    #[inline]
    fn mul_fp(self, value: Fp) -> Self {
        self.mul_base(value)
    }
    #[inline]
    fn to_fp4(self) -> Fp4 {
        self
    }
    #[inline]
    fn scale_ext(self, alpha: Fp4) -> Fp4 {
        self * alpha
    }
    fn helper_identity(helper: Fp4, values: &[Self], beta: &BetaPowers) -> Fp4 {
        helper_identity_generic(helper, values, beta.0[1])
    }
}

#[derive(Clone, Debug)]
struct CLin {
    constant: Fp,
    /// `(slot, coefficient, kind)` with kind 1 = +1, 2 = -1, 0 = general.
    terms: Vec<(u32, Fp, u8)>,
}

impl CLin {
    fn compile(lin: &Lin, layout: &Layout) -> Self {
        Self {
            constant: Fp::from_i64(lin.constant),
            terms: lin
                .terms
                .iter()
                .map(|(var, c)| {
                    let kind = match *c {
                        1 => 1,
                        -1 => 2,
                        _ => 0,
                    };
                    (layout.slot(*var), Fp::from_i64(*c), kind)
                })
                .collect(),
        }
    }

    #[inline]
    fn eval<E: Element>(&self, row: &[E]) -> E {
        let mut acc = E::from_fp(self.constant);
        for (slot, c, kind) in &self.terms {
            let v = row[*slot as usize];
            acc = match kind {
                1 => acc + v,
                2 => acc - v,
                _ => acc + v.mul_fp(*c),
            };
        }
        acc
    }
}

#[derive(Clone, Debug)]
struct CConstraint {
    lin: CLin,
    products: Vec<(Fp, CLin, CLin)>,
    gate: Option<CLin>,
    transition: bool,
}

impl CConstraint {
    #[inline]
    fn eval<E: Element>(&self, row: &[E]) -> E {
        let mut acc = self.lin.eval(row);
        for (c, left, right) in &self.products {
            let product = left.eval(row) * right.eval(row);
            acc = if *c == Fp::ONE {
                acc + product
            } else {
                acc + product.mul_fp(*c)
            };
        }
        match &self.gate {
            Some(gate) => acc * gate.eval(row),
            None => acc,
        }
    }
}

/// Positions of each variable in a row buffer.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// Main columns.
    pub main: usize,
    /// Public columns.
    pub public: usize,
}

impl Layout {
    /// Slot of a variable.
    pub fn slot(&self, var: Var) -> u32 {
        let slot = match var {
            Var::Main(i) => i as usize,
            Var::Public(i) => self.main + i as usize,
            Var::Next(i) => self.row() + i as usize,
        };
        u32::try_from(slot).expect("small")
    }

    /// Width of one row (`main + public`).
    pub const fn row(&self) -> usize {
        self.main + self.public
    }

    /// Row buffer length (two rows).
    pub const fn width(&self) -> usize {
        2 * self.row()
    }
}

/// Number of quotient chunks for identities of a given maximum degree.
pub fn quotient_chunks(max_degree: u32) -> usize {
    (max_degree as usize - 1).max(1)
}

/// Log of the evaluation-domain blowup needed for those chunks.
pub fn quotient_log_blowup(max_degree: u32) -> u32 {
    let chunks = quotient_chunks(max_degree);
    chunks.next_power_of_two().trailing_zeros()
}

/// The two quotient groups: circuit/boundary/accumulator identities (low degree) and LogUp helper
/// identities (degree `lookups_per_helper + 1`). Each group has its own quotient chunks and
/// evaluation domain, so wide helpers do not double the cost of evaluating the circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotientShape {
    /// Maximum degree of the low group.
    pub low_degree: u32,
    /// Maximum degree of the helper group (0 without lookups).
    pub high_degree: u32,
}

impl QuotientShape {
    /// Chunks of the low group.
    pub fn low_chunks(&self) -> usize {
        quotient_chunks(self.low_degree)
    }

    /// Chunks of the helper group.
    pub fn high_chunks(&self) -> usize {
        if self.high_degree == 0 {
            0
        } else {
            quotient_chunks(self.high_degree)
        }
    }

    /// All chunks.
    pub fn chunks(&self) -> usize {
        self.low_chunks() + self.high_chunks()
    }
}

/// The quotient groups of a circuit (independent of any statement).
pub fn quotient_shape(circuit: &Circuit) -> QuotientShape {
    let low_degree = circuit
        .constraints
        .iter()
        .map(crate::circuit::Constraint::degree)
        .max()
        .unwrap_or(1)
        .max(2);
    let high_degree = if circuit.lookups.is_empty() {
        0
    } else {
        u32::try_from(circuit.lookups_per_helper).expect("small") + 1
    };
    QuotientShape {
        low_degree,
        high_degree,
    }
}

/// A circuit compiled for evaluation.
#[derive(Clone, Debug)]
pub struct CompiledAir {
    layout: Layout,
    constraints: Vec<CConstraint>,
    lookups: Vec<CLin>,
    helpers: Vec<(usize, usize)>,
    shape: QuotientShape,
    /// Identity count: circuit and boundary identities, the accumulator, then helpers.
    pub total_constraints: usize,
}

impl CompiledAir {
    /// Compiles the circuit's identities plus boundary identities for one statement: the first
    /// row equals `initial`, the last step's row equals `final_state` and the checkpoint row (if
    /// the statement has one; otherwise its selector is zero everywhere) equals `checkpoint`.
    pub fn new(
        circuit: &Circuit,
        initial: &[i64],
        final_state: &[i64],
        checkpoint: &[i64],
    ) -> Self {
        let layout = Layout {
            main: circuit.columns.len(),
            public: circuit.public.len(),
        };
        let mut constraints: Vec<CConstraint> = circuit
            .constraints
            .iter()
            .map(|c| CConstraint {
                lin: CLin::compile(&c.body.lin, &layout),
                products: c
                    .body
                    .products
                    .iter()
                    .map(|p| {
                        (
                            Fp::from_i64(p.coeff),
                            CLin::compile(&p.left, &layout),
                            CLin::compile(&p.right, &layout),
                        )
                    })
                    .collect(),
                gate: c.gate.as_ref().map(|g| CLin::compile(g, &layout)),
                transition: c.transition,
            })
            .collect();
        // Boundary identities: first * (reg - initial), last * (reg - final) and
        // checkpoint * (reg - checkpoint value).
        for (register, ((init, fin), mid)) in circuit
            .registers
            .iter()
            .zip(initial.iter().zip(final_state).zip(checkpoint))
        {
            for (selector, value) in [
                (crate::circuit::PUBLIC_FIRST, *init),
                (crate::circuit::PUBLIC_LAST, *fin),
                (crate::circuit::PUBLIC_CHECKPOINT, *mid),
            ] {
                constraints.push(CConstraint {
                    lin: CLin::compile(&Lin::default(), &layout),
                    products: vec![(
                        Fp::ONE,
                        CLin::compile(&Lin::var(Var::Public(selector)), &layout),
                        CLin::compile(
                            &Lin {
                                constant: -value,
                                terms: vec![(Var::Main(register.column), 1)],
                            },
                            &layout,
                        ),
                    )],
                    gate: None,
                    transition: false,
                });
            }
        }
        let lookups: Vec<CLin> = circuit
            .lookups
            .iter()
            .map(|l| CLin::compile(l, &layout))
            .collect();
        let per = circuit.lookups_per_helper;
        let helpers: Vec<(usize, usize)> = (0..circuit.helper_count())
            .map(|h| (h * per, ((h + 1) * per).min(lookups.len())))
            .collect();
        let shape = quotient_shape(circuit);
        let total_constraints = constraints.len()
            + if lookups.is_empty() {
                0
            } else {
                1 + helpers.len()
            };
        Self {
            layout,
            constraints,
            lookups,
            helpers,
            shape,
            total_constraints,
        }
    }

    /// Row-buffer layout.
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    /// Quotient groups.
    pub const fn shape(&self) -> QuotientShape {
        self.shape
    }

    /// Number of extension-valued auxiliary columns (helpers then the accumulator).
    pub fn aux_columns(&self) -> usize {
        if self.lookups.is_empty() {
            0
        } else {
            self.helpers.len() + 1
        }
    }

    /// Evaluates every lookup value of a base-field row.
    pub fn lookup_values(&self, row: &[Fp], out: &mut Vec<Fp>) {
        out.clear();
        out.extend(self.lookups.iter().map(|l| l.eval(row)));
    }

    /// Helper values of one row given `1/(beta - a)` for each lookup.
    pub fn helper_values(&self, inverses: &[Fp4], out: &mut Vec<Fp4>) {
        out.clear();
        for (start, end) in &self.helpers {
            let mut sum = Fp4::ZERO;
            for inv in &inverses[*start..*end] {
                sum += *inv;
            }
            out.push(sum);
        }
    }

    /// Low group at one point: circuit and boundary identities split by zerofier, plus the cyclic
    /// accumulator identity `S(next) - S - sum(helpers) + table/n = 0` on every row.
    ///
    /// Returns `(every_row_sum, transition_sum)`.
    pub fn combine_low<E: Element>(
        &self,
        row: &[E],
        aux: &[Fp4],
        aux_next_accumulator: Fp4,
        table_per_row: Fp4,
        alphas: &[Fp4],
    ) -> (Fp4, Fp4) {
        let mut every = Fp4::ZERO;
        let mut transition = Fp4::ZERO;
        for (constraint, alpha) in self.constraints.iter().zip(alphas) {
            let value = constraint.eval(row).scale_ext(*alpha);
            if constraint.transition {
                transition += value;
            } else {
                every += value;
            }
        }
        if !self.lookups.is_empty() {
            let helper_count = self.helpers.len();
            let mut helper_sum = Fp4::ZERO;
            for helper in &aux[..helper_count] {
                helper_sum += *helper;
            }
            let accumulator = aux[helper_count];
            every += (aux_next_accumulator - accumulator - helper_sum + table_per_row)
                * alphas[self.constraints.len()];
        }
        (every, transition)
    }

    /// Helper group at one point: `h * prod(beta - a_i) - sum_i prod_{s != i}(beta - a_s)`.
    pub fn combine_helpers<E: Element>(
        &self,
        row: &[E],
        aux: &[Fp4],
        beta: &BetaPowers,
        alphas: &[Fp4],
    ) -> Fp4 {
        let mut total = Fp4::ZERO;
        let base = self.constraints.len() + 1;
        let mut values = [E::ZERO; 4];
        for (h, (start, end)) in self.helpers.iter().enumerate() {
            let count = end - start;
            for (slot, lookup) in values.iter_mut().zip(&self.lookups[*start..*end]) {
                *slot = lookup.eval(row);
            }
            total += E::helper_identity(aux[h], &values[..count], beta) * alphas[base + h];
        }
        total
    }

    /// Evaluates one circuit constraint (diagnostics).
    pub fn eval_constraint(&self, index: usize, row: &[Fp]) -> Fp {
        self.constraints[index].eval(row)
    }

    /// Number of circuit and boundary identities (without LogUp).
    pub fn base_constraint_count(&self) -> usize {
        self.constraints.len()
    }

    /// Whether constraint `index` is a transition identity.
    pub fn is_transition(&self, index: usize) -> bool {
        self.constraints[index].transition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_helper_identity_matches_the_generic_form() {
        let beta = Fp4([Fp::new(11), Fp::new(22), Fp::new(33), Fp::new(44)]);
        let powers = BetaPowers::new(beta);
        let helper = Fp4([Fp::new(5), Fp::new(6), Fp::new(7), Fp::new(8)]);
        for count in 1..=4 {
            let values: Vec<Fp> = (0..count).map(|i| Fp::new(1000 + 37 * i as u64)).collect();
            let ext: Vec<Fp4> = values.iter().map(|v| Fp4::from_base(*v)).collect();
            assert_eq!(
                <Fp as Element>::helper_identity(helper, &values, &powers),
                helper_identity_generic(helper, &ext, beta)
            );
        }
    }
}
