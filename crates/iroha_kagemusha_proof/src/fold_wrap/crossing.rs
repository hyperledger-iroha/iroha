//! The values that cross between the wraps, in the order of P's instance.
//!
//! P's single public column carries every value one wrap computes and the
//! other consumes. Public columns of the decided accumulators do not cross:
//! the field transcript binds them through each digest, and the side that
//! hashes them computes the instance copy term `I` the field side needs. A value of `Fp` (P's native field, a canonical scalar in
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

/// An accumulator's field terms: `theta`, `r`, `beta`, `e`, and its instance
/// copy term `I` (the public columns stay with the digest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccumulatorTerms<T> {
    /// `theta` powers.
    pub theta: Vec<T>,
    /// One `LogUp` challenge per group.
    pub r: Vec<T>,
    /// `pow` challenges.
    pub beta: Vec<T>,
    /// The accumulated error.
    pub e: T,
    /// The instance copy term `I`.
    pub copy_terms: T,
}

/// One decided Q accumulator's crossing values: its digest and field terms,
/// computed in P (`I` from the public columns P hashes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QMember<X> {
    /// `kgwfacd1(U_Q[k])`.
    pub acc_digest: X,
    /// `U_Q[k]`'s field terms.
    pub terms: AccumulatorTerms<Pair<X>>,
}

/// The Q-side decider's crossing values: one batch over every Q kind. Its
/// commitments are Pallas points (base `Fp`), its scalars `Fq`. The
/// commitment side, with the accumulators, their digests and the opening's
/// group part, is in P. The field side, with the field transcript and the
/// opening scalars, is in Omega.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QCrossing<X> {
    /// The batch's members, in kind order.
    pub members: Vec<QMember<X>>,
    /// `D_H` of the batch's helpers (P).
    pub helper_digest: X,
    /// The copy challenges `gamma`, `eta` (Omega).
    pub copy_challenges: [Pair<X>; 2],
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

/// P's whole public column: what Omega uses (the A side's crossing lives in
/// R's public column, which P verifies).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PPublic<X> {
    /// `D_A`, which Omega publishes.
    pub d_a: X,
    /// The Q-side decider.
    pub q: QCrossing<X>,
    /// The A-side decider's claim (from R) for Omega's `acc_V`.
    pub a_claim: AClaimCrossing<X>,
    /// The Pallas claim accumulator.
    pub pallas: PallasClaimCrossing<X>,
    /// The Vesta claim accumulator.
    pub vesta: VestaClaimCrossing<X>,
}

/// The field-term counts of one relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataShape {
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
            theta: shape.theta_len,
            r: shape.groups.len(),
            beta: shape.index_bits,
        }
    }

    /// Values: the counted ones, `e` and `I`.
    fn values(&self) -> usize {
        self.theta + self.r + self.beta + 2
    }
}

/// The counts that fix P's instance layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PLayout {
    /// Each Q kind's field terms, in kind order.
    pub q_data: Vec<DataShape>,
    /// Opening rounds of either decider (`mu`).
    pub rounds: usize,
    /// Claim rounds (`iroha_fold::claims::ROUNDS`).
    pub claim_rounds: usize,
}

impl PLayout {
    /// The layout for the Q kinds `q`.
    #[must_use]
    pub fn new(q: &[FoldShape], rounds: usize) -> Self {
        Self {
            q_data: q.iter().map(DataShape::of).collect(),
            rounds,
            claim_rounds: iroha_fold::claims::ROUNDS,
        }
    }

    /// Rows of P's public column.
    #[must_use]
    pub fn rows(&self) -> usize {
        let q_members: usize = self.q_data.iter().map(|d| 1 + 2 * d.values()).sum();
        let q = q_members + 1 + 2 * 2 + 2 * 4 + 2 * self.rounds;
        let a_claim = 2 * 2 + self.rounds;
        let pallas = 1 + 2 * (3 * self.claim_rounds + 2);
        let vesta = 2 + 3 * self.claim_rounds + 2;
        1 + q + a_claim + pallas + vesta
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
        let d_a = cursor.one();
        let q = QCrossing {
            members: self
                .q_data
                .iter()
                .map(|shape| QMember {
                    acc_digest: cursor.one(),
                    terms: cursor.terms_pairs(shape),
                })
                .collect(),
            helper_digest: cursor.one(),
            copy_challenges: [cursor.pair(), cursor.pair()],
            field_digest: cursor.pair(),
            value: cursor.pair(),
            folded: cursor.pair(),
            alpha: cursor.pair(),
            rounds: cursor.pairs(self.rounds),
        };
        let a_claim = AClaimCrossing {
            g: [cursor.pair(), cursor.pair()],
            u: cursor.many(self.rounds),
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
            d_a,
            q,
            a_claim,
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
    fn terms(&mut self, shape: &DataShape) -> AccumulatorTerms<X> {
        AccumulatorTerms {
            theta: self.many(shape.theta),
            r: self.many(shape.r),
            beta: self.many(shape.beta),
            e: self.one(),
            copy_terms: self.one(),
        }
    }
    fn terms_pairs(&mut self, shape: &DataShape) -> AccumulatorTerms<Pair<X>> {
        AccumulatorTerms {
            theta: self.pairs(shape.theta),
            r: self.pairs(shape.r),
            beta: self.pairs(shape.beta),
            e: self.pair(),
            copy_terms: self.pair(),
        }
    }
}

fn push_terms<X: Clone>(out: &mut Vec<X>, terms: &AccumulatorTerms<X>) {
    for list in [&terms.theta, &terms.r, &terms.beta] {
        out.extend(list.iter().cloned());
    }
    out.extend([terms.e.clone(), terms.copy_terms.clone()]);
}

fn push_pairs<X: Clone>(out: &mut Vec<X>, pairs: &[Pair<X>]) {
    for pair in pairs {
        out.extend(pair.iter().cloned());
    }
}

fn push_terms_pairs<X: Clone>(out: &mut Vec<X>, terms: &AccumulatorTerms<Pair<X>>) {
    for list in [&terms.theta, &terms.r, &terms.beta] {
        push_pairs(out, list);
    }
    push_pairs(out, &[terms.e.clone(), terms.copy_terms.clone()]);
}

impl<X: Clone> PPublic<X> {
    /// The rows in instance order.
    #[must_use]
    pub fn rows(&self) -> Vec<X> {
        let mut out = vec![self.d_a.clone()];
        let q = &self.q;
        for member in &q.members {
            out.push(member.acc_digest.clone());
            push_terms_pairs(&mut out, &member.terms);
        }
        out.push(q.helper_digest.clone());
        push_pairs(&mut out, &q.copy_challenges);
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
        push_pairs(&mut out, &self.a_claim.g);
        out.extend(self.a_claim.u.iter().cloned());
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

/// One decided A accumulator's values in R's public column, computed in R
/// (the last kind after the fold of `A_n`; `I` from the public columns R
/// hashes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RMember<X> {
    /// `kgwfacd1(U_A[k])` (an `Fq` value, native in R).
    pub acc_digest: X,
    /// `U_A[k]`'s field terms (`Fp` values).
    pub terms: AccumulatorTerms<X>,
}

/// R's whole public column: everything R exchanges with P, which verifies
/// R. R is an `Fq` circuit, so every value is one row: `Fq` values natively,
/// `Fp` values as the same integer. The A-side decider's crossing lives here
/// (its field side is in P), together with `A_n`'s public columns (P checks
/// `h_out`), the A decider's claim (P forwards it to Omega's `acc_V`) and the
/// Vesta claim digest (Omega computes it; P forwards it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RPublic<X> {
    /// `A_n`'s public columns (`[passthrough, h_out, ...]` first).
    pub a_public: Vec<X>,
    /// The decided A accumulators, in kind order (R).
    pub members: Vec<RMember<X>>,
    /// `D_H` of the batch's helpers (R).
    pub helper_digest: X,
    /// The copy challenges `gamma`, `eta` (P).
    pub copy_challenges: [X; 2],
    /// `d_F` (P).
    pub field_digest: X,
    /// `v*` (P).
    pub value: X,
    /// `b'` (P).
    pub folded: X,
    /// The opening's batching challenge (R).
    pub alpha: X,
    /// The opening's round challenges (R).
    pub rounds: Vec<X>,
    /// The opening's undecided claim `G'_0` (R).
    pub claim: [X; 2],
    /// `kgwfcld1` of the Vesta claim accumulator (Omega, through P).
    pub claim_digest: X,
}

/// The counts that fix R's public column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RLayout {
    /// `A_n`'s public values.
    pub a_public: usize,
    /// Each A kind's field terms, in kind order.
    pub a_data: Vec<DataShape>,
    /// Opening rounds (`mu`).
    pub rounds: usize,
}

impl RLayout {
    /// Rows of R's public column.
    #[must_use]
    pub fn rows(&self) -> usize {
        let members: usize = self.a_data.iter().map(|d| 1 + d.values()).sum();
        self.a_public + members + 1 + 2 + 3 + 1 + self.rounds + 2 + 1
    }

    /// Splits R's public rows; the inverse of [`RPublic::rows`].
    ///
    /// # Errors
    /// [`Error::Synthesis`] for another number of rows.
    pub fn parse<X: Clone>(&self, rows: &[X]) -> Result<RPublic<X>, Error> {
        if rows.len() != self.rows() {
            return Err(Error::Synthesis);
        }
        let mut cursor = Cursor { rows, at: 0 };
        let public = RPublic {
            a_public: cursor.many(self.a_public),
            members: self
                .a_data
                .iter()
                .map(|shape| RMember {
                    acc_digest: cursor.one(),
                    terms: cursor.terms(shape),
                })
                .collect(),
            helper_digest: cursor.one(),
            copy_challenges: [cursor.one(), cursor.one()],
            field_digest: cursor.one(),
            value: cursor.one(),
            folded: cursor.one(),
            alpha: cursor.one(),
            rounds: cursor.many(self.rounds),
            claim: [cursor.one(), cursor.one()],
            claim_digest: cursor.one(),
        };
        if cursor.at != rows.len() {
            return Err(Error::Synthesis);
        }
        Ok(public)
    }
}

impl<X: Clone> RPublic<X> {
    /// The rows in instance order.
    #[must_use]
    pub fn rows(&self) -> Vec<X> {
        let mut out = self.a_public.clone();
        for member in &self.members {
            out.push(member.acc_digest.clone());
            push_terms(&mut out, &member.terms);
        }
        out.push(self.helper_digest.clone());
        out.extend(self.copy_challenges.iter().cloned());
        out.extend([
            self.field_digest.clone(),
            self.value.clone(),
            self.folded.clone(),
            self.alpha.clone(),
        ]);
        out.extend(self.rounds.iter().cloned());
        out.extend(self.claim.iter().cloned());
        out.push(self.claim_digest.clone());
        out
    }
}

/// The A-side decider's claim as P forwards it to Omega's `acc_V`: `G'_0`'s
/// coordinates (`Fq`, canonical limbs) and the round challenges (`Fp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AClaimCrossing<X> {
    /// `G'_0`'s coordinates.
    pub g: [Pair<X>; 2],
    /// The round challenges.
    pub u: Vec<X>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_parse_are_inverse() {
        let shape = |theta, r, beta| DataShape { theta, r, beta };
        let layout = PLayout {
            q_data: vec![shape(2, 3, 18), shape(0, 0, 18)],
            rounds: 16,
            claim_rounds: 16,
        };
        let rows: Vec<usize> = (0..layout.rows()).collect();
        let parsed = layout.parse(&rows).expect("exact rows");
        assert_eq!(parsed.rows(), rows);
        assert_eq!(parsed.d_a, 0);
        assert_eq!(parsed.q.members.len(), 2);
        assert_eq!(parsed.vesta.value, layout.rows() - 1);
        assert!(layout.parse(&rows[1..]).is_err());
    }

    #[test]
    fn r_rows_and_parse_are_inverse() {
        let shape = |theta, r, beta| DataShape { theta, r, beta };
        let layout = RLayout {
            a_public: 2,
            a_data: vec![shape(1, 1, 17), shape(0, 2, 17)],
            rounds: 16,
        };
        let rows: Vec<usize> = (0..layout.rows()).collect();
        let parsed = layout.parse(&rows).expect("exact rows");
        assert_eq!(parsed.rows(), rows);
        assert_eq!(parsed.claim_digest, layout.rows() - 1);
        assert!(layout.parse(&rows[1..]).is_err());
    }
}
