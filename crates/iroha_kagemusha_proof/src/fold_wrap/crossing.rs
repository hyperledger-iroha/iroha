//! The values that cross between the wraps, in the order of P's instance.
//!
//! P's single public column carries every value one wrap computes and the
//! other consumes. A value of `Fp` (P's native field, a canonical scalar in
//! Omega) is one row. A value of `Fq` is two rows, its canonical S6 limbs
//! `(lo, hi)` (`lo < 2^128`, `hi < 2^127`), because an `Fq` integer need not be
//! below `p`. The side that computes a value range-checks its limbs. The
//! other side's copy is forced, because Omega verifies P under these exact
//! instance values.
//!
//! The generic structs carry one item per row (`X`) or row pair (`[X; 2]`):
//! `Word<Fp>` cells in P, `ScalarCells<Eq>` in Omega, and `Fp` natively.
//! [`PPublic::rows`] and [`PLayout::parse`] are inverse, and fix the order.

use iroha_fold::relation::FoldShape;
use iroha_plonk::frontend::Error;

/// Two rows: a value's canonical limbs `(lo, hi)`.
pub type Pair<X> = [X; 2];

/// An accumulator's field data: its public columns (concatenated), `theta`,
/// `r`, `beta` and `e`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccumulatorData<T> {
    /// The folded public columns, column after column.
    pub public: Vec<T>,
    /// `theta` powers.
    pub theta: Vec<T>,
    /// One `LogUp` challenge per group.
    pub r: Vec<T>,
    /// `pow` challenges.
    pub beta: Vec<T>,
    /// The accumulated error.
    pub e: T,
}

/// The Q-side decider's crossing values. Its commitments are Pallas points
/// (base `Fp`), its scalars `Fq`. The commitment side, with the
/// accumulator, its digest and the opening's group part, is in P. The field
/// side, with the field transcript and the opening scalars, is in Omega.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QCrossing<X> {
    /// `kgwfacd1(U_Q)` (P).
    pub acc_digest: X,
    /// `D_H` of the decider's helpers (P).
    pub helper_digest: X,
    /// `U_Q`'s field data (P).
    pub data: AccumulatorData<Pair<X>>,
    /// `d_F` (Omega).
    pub field_digest: Pair<X>,
    /// `v* = sum_c alpha^c v_c` (Omega).
    pub value: Pair<X>,
    /// `b' = folded_eq(sigma, u)` (Omega).
    pub folded: Pair<X>,
    /// The opening's batching challenge (P).
    pub alpha: Pair<X>,
    /// The opening's round challenges (P).
    pub rounds: Vec<Pair<X>>,
}

/// The A-side decider's crossing values. Its commitments are Vesta points
/// (base `Fq`), its scalars `Fp`. The commitment side (the last A fold, the
/// digest and the opening's group part) is in Omega; the field side is in P.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ACrossing<X> {
    /// `kgwfacd1(U_A')` of the accumulator after the last A fold (Omega).
    pub acc_digest: Pair<X>,
    /// `D_H` of the decider's helpers (Omega).
    pub helper_digest: Pair<X>,
    /// `U_A'`'s field data (Omega's fold).
    pub data: AccumulatorData<X>,
    /// `d_F` (P).
    pub field_digest: X,
    /// `v*` (P).
    pub value: X,
    /// `b'` (P).
    pub folded: X,
    /// The opening's batching challenge (Omega).
    pub alpha: X,
    /// The opening's round challenges (Omega).
    pub rounds: Vec<X>,
}

/// The Pallas claim accumulator's crossing values (kept by A; scalars `Fq`).
/// The group side, with the digest and the committed-mask opening, is in P.
/// The restructuring sumcheck is in Omega.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PallasClaimCrossing<X> {
    /// `kgwfcld1` (P).
    pub digest: X,
    /// The folded challenges (P).
    pub u: Vec<Pair<X>>,
    /// The `pow` challenges (P).
    pub beta: Vec<Pair<X>>,
    /// The accumulated error (P).
    pub e: Pair<X>,
    /// The sumcheck point `rho` (Omega).
    pub rho: Vec<Pair<X>>,
    /// The claimed `s~(rho)` (Omega).
    pub value: Pair<X>,
}

/// The Vesta claim accumulator's crossing values (kept by Q; scalars `Fp`).
/// The restructuring sumcheck is in P, the group side in Omega.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VestaClaimCrossing<X> {
    /// `kgwfcld1` (Omega).
    pub digest: Pair<X>,
    /// The folded challenges (P's field side; Omega's digest absorbs them).
    pub u: Vec<X>,
    /// The `pow` challenges.
    pub beta: Vec<X>,
    /// The accumulated error.
    pub e: X,
    /// The sumcheck point `rho` (P).
    pub rho: Vec<X>,
    /// The claimed `s~(rho)` (P).
    pub value: X,
}

/// P's whole public column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PPublic<X> {
    /// `A_n`'s public columns (`[passthrough, h_out, bridge...]` first),
    /// native in P and folded by Omega.
    pub a_public: Vec<X>,
    /// `D_A`, which Omega publishes.
    pub d_a: X,
    /// The Q-side decider.
    pub q: QCrossing<X>,
    /// The A-side decider.
    pub a: ACrossing<X>,
    /// The Pallas claim accumulator.
    pub pallas: PallasClaimCrossing<X>,
    /// The Vesta claim accumulator.
    pub vesta: VestaClaimCrossing<X>,
}

/// The field-data counts of one relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataShape {
    /// Public values over every instance column.
    pub public: usize,
    /// `theta` powers.
    pub theta: usize,
    /// `LogUp` groups.
    pub r: usize,
    /// `pow` challenges.
    pub beta: usize,
}

impl DataShape {
    /// The counts of `shape`.
    #[must_use]
    pub fn of(shape: &FoldShape) -> Self {
        Self {
            public: shape.instance_lengths.iter().sum(),
            theta: shape.theta_len,
            r: shape.groups.len(),
            beta: shape.index_bits,
        }
    }

    fn values(&self) -> usize {
        self.public + self.theta + self.r + self.beta + 1
    }
}

/// The counts that fix P's instance layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PLayout {
    /// `A_n`'s public values.
    pub a_public: usize,
    /// The Q relation's field data.
    pub q_data: DataShape,
    /// The A relation's field data.
    pub a_data: DataShape,
    /// Opening rounds of either decider (`mu`).
    pub rounds: usize,
    /// Claim rounds (`iroha_fold::claims::ROUNDS`).
    pub claim_rounds: usize,
}

impl PLayout {
    /// The layout for the last A relation `a` and the Q relation `q`.
    #[must_use]
    pub fn new(a: &FoldShape, q: &FoldShape, rounds: usize) -> Self {
        Self {
            a_public: a.instance_lengths.iter().sum(),
            q_data: DataShape::of(q),
            a_data: DataShape::of(a),
            rounds,
            claim_rounds: iroha_fold::claims::ROUNDS,
        }
    }

    /// Rows of P's public column.
    #[must_use]
    pub fn rows(&self) -> usize {
        let q = 2 + 2 * self.q_data.values() + 2 * 4 + 2 * self.rounds;
        let a = 2 * 2 + self.a_data.values() + 4 + self.rounds;
        let pallas = 1 + 2 * (3 * self.claim_rounds + 2);
        let vesta = 2 + 3 * self.claim_rounds + 2;
        self.a_public + 1 + q + a + pallas + vesta
    }

    /// Splits P's public rows into their values; the inverse of
    /// [`PPublic::rows`].
    ///
    /// # Errors
    /// [`Error::Synthesis`] for another number of rows.
    pub fn parse<X: Clone>(&self, rows: &[X]) -> Result<PPublic<X>, Error> {
        if rows.len() != self.rows() {
            return Err(Error::Synthesis);
        }
        let mut cursor = Cursor { rows, at: 0 };
        let a_public = cursor.many(self.a_public);
        let d_a = cursor.one();
        let q = QCrossing {
            acc_digest: cursor.one(),
            helper_digest: cursor.one(),
            data: cursor.data_pairs(&self.q_data),
            field_digest: cursor.pair(),
            value: cursor.pair(),
            folded: cursor.pair(),
            alpha: cursor.pair(),
            rounds: cursor.pairs(self.rounds),
        };
        let a = ACrossing {
            acc_digest: cursor.pair(),
            helper_digest: cursor.pair(),
            data: cursor.data(&self.a_data),
            field_digest: cursor.one(),
            value: cursor.one(),
            folded: cursor.one(),
            alpha: cursor.one(),
            rounds: cursor.many(self.rounds),
        };
        let pallas = PallasClaimCrossing {
            digest: cursor.one(),
            u: cursor.pairs(self.claim_rounds),
            beta: cursor.pairs(self.claim_rounds),
            e: cursor.pair(),
            rho: cursor.pairs(self.claim_rounds),
            value: cursor.pair(),
        };
        let vesta = VestaClaimCrossing {
            digest: cursor.pair(),
            u: cursor.many(self.claim_rounds),
            beta: cursor.many(self.claim_rounds),
            e: cursor.one(),
            rho: cursor.many(self.claim_rounds),
            value: cursor.one(),
        };
        if cursor.at != rows.len() {
            return Err(Error::Synthesis);
        }
        Ok(PPublic {
            a_public,
            d_a,
            q,
            a,
            pallas,
            vesta,
        })
    }
}

struct Cursor<'a, X> {
    rows: &'a [X],
    at: usize,
}

impl<X: Clone> Cursor<'_, X> {
    fn one(&mut self) -> X {
        self.at += 1;
        self.rows[self.at - 1].clone()
    }
    fn pair(&mut self) -> Pair<X> {
        [self.one(), self.one()]
    }
    fn many(&mut self, count: usize) -> Vec<X> {
        (0..count).map(|_| self.one()).collect()
    }
    fn pairs(&mut self, count: usize) -> Vec<Pair<X>> {
        (0..count).map(|_| self.pair()).collect()
    }
    fn data(&mut self, shape: &DataShape) -> AccumulatorData<X> {
        AccumulatorData {
            public: self.many(shape.public),
            theta: self.many(shape.theta),
            r: self.many(shape.r),
            beta: self.many(shape.beta),
            e: self.one(),
        }
    }
    fn data_pairs(&mut self, shape: &DataShape) -> AccumulatorData<Pair<X>> {
        AccumulatorData {
            public: self.pairs(shape.public),
            theta: self.pairs(shape.theta),
            r: self.pairs(shape.r),
            beta: self.pairs(shape.beta),
            e: self.pair(),
        }
    }
}

fn push_data<X: Clone>(out: &mut Vec<X>, data: &AccumulatorData<X>) {
    for list in [&data.public, &data.theta, &data.r, &data.beta] {
        out.extend(list.iter().cloned());
    }
    out.push(data.e.clone());
}

fn push_pairs<X: Clone>(out: &mut Vec<X>, pairs: &[Pair<X>]) {
    for pair in pairs {
        out.extend(pair.iter().cloned());
    }
}

fn push_data_pairs<X: Clone>(out: &mut Vec<X>, data: &AccumulatorData<Pair<X>>) {
    for list in [&data.public, &data.theta, &data.r, &data.beta] {
        push_pairs(out, list);
    }
    out.extend(data.e.iter().cloned());
}

impl<X: Clone> PPublic<X> {
    /// The rows in instance order.
    #[must_use]
    pub fn rows(&self) -> Vec<X> {
        let mut out = self.a_public.clone();
        out.push(self.d_a.clone());
        let q = &self.q;
        out.extend([q.acc_digest.clone(), q.helper_digest.clone()]);
        push_data_pairs(&mut out, &q.data);
        push_pairs(
            &mut out,
            &[
                q.field_digest.clone(),
                q.value.clone(),
                q.folded.clone(),
                q.alpha.clone(),
            ],
        );
        push_pairs(&mut out, &q.rounds);
        let a = &self.a;
        push_pairs(&mut out, &[a.acc_digest.clone(), a.helper_digest.clone()]);
        push_data(&mut out, &a.data);
        out.extend([
            a.field_digest.clone(),
            a.value.clone(),
            a.folded.clone(),
            a.alpha.clone(),
        ]);
        out.extend(a.rounds.iter().cloned());
        let p = &self.pallas;
        out.push(p.digest.clone());
        push_pairs(&mut out, &p.u);
        push_pairs(&mut out, &p.beta);
        out.extend(p.e.iter().cloned());
        push_pairs(&mut out, &p.rho);
        out.extend(p.value.iter().cloned());
        let v = &self.vesta;
        out.extend(v.digest.iter().cloned());
        out.extend(v.u.iter().cloned());
        out.extend(v.beta.iter().cloned());
        out.push(v.e.clone());
        out.extend(v.rho.iter().cloned());
        out.push(v.value.clone());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_parse_are_inverse() {
        let layout = PLayout {
            a_public: 3,
            q_data: DataShape {
                public: 5,
                theta: 2,
                r: 3,
                beta: 18,
            },
            a_data: DataShape {
                public: 3,
                theta: 1,
                r: 1,
                beta: 17,
            },
            rounds: 16,
            claim_rounds: 16,
        };
        let rows: Vec<usize> = (0..layout.rows()).collect();
        let parsed = layout.parse(&rows).expect("exact rows");
        assert_eq!(parsed.rows(), rows);
        assert_eq!(parsed.d_a, 3);
        assert_eq!(parsed.vesta.value, layout.rows() - 1);
        assert!(layout.parse(&rows[1..]).is_err());
    }
}
