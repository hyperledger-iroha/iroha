//! Native deciders: the non-succinct test oracle of the folding relation.
//!
//! [`decide`] accepts an accumulator exactly when its witness opens every
//! commitment, satisfies every copy and `LogUp` sum, and has accumulated error `e`.
//! [`decide_fresh`] accepts a fresh instance exactly when its `LogUp` challenges
//! are its own, its witness opens every commitment and satisfies every
//! constraint, copy and `LogUp` sum. Neither is a proof: compression replaces them
//! with succinct deciders.
//!
//! A `LogUp` sum counts each reciprocal column `h_s` on its slot's cells and the
//! table column `g` on the usable rows: exactly the cells the relation
//! constrains ([`crate::logup`]).

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField};

use crate::{
    Error,
    commit::{CommitKey, Witness},
    fold::{Accumulator, Instance, View, constraints_at, pow_vector},
    relation::{Cell, FoldedRelation},
};

fn check_shape<F: PastaField>(relation: &FoldedRelation<F>, witness: &Witness<F>) -> Result<(), Error> {
    let columns = relation.columns();
    if witness.columns.len() != columns
        || witness.blinds.len() != columns
        || witness.columns.iter().any(|c| c.len() != relation.n)
    {
        return Err(Error::Shape("witness"));
    }
    Ok(())
}

fn check_openings<C: PastaCurve>(
    key: &CommitKey<C>,
    instance: &Instance<C>,
    witness: &Witness<C::ScalarExt>,
) -> Result<(), Error> {
    if instance.commitments.len() != witness.columns.len() {
        return Err(Error::Shape("commitments"));
    }
    for ((commitment, column), blind) in instance.commitments.iter().zip(&witness.columns).zip(&witness.blinds) {
        if key.commit(column, blind)? != *commitment {
            return Err(Error::Rejected("commitment opening"));
        }
    }
    Ok(())
}

fn check_copies<F: PastaField>(
    relation: &FoldedRelation<F>,
    witness: &Witness<F>,
    public: &[Vec<F>],
) -> Result<(), Error> {
    let value = |cell: Cell| match cell {
        Cell::Advice(c, r) => witness.columns[c as usize][r as usize],
        Cell::Fixed(c, r) => relation.fixed[c as usize][r as usize],
        Cell::Instance(c, r) => public
            .get(c as usize)
            .and_then(|column| column.get(r as usize))
            .copied()
            .unwrap_or(F::ZERO),
    };
    if relation.copies.iter().any(|(a, b)| value(*a) != value(*b)) {
        return Err(Error::Rejected("copy constraint"));
    }
    Ok(())
}

/// Each group's `LogUp` sum over exactly the cells its constraints cover: every
/// `h_s` on its slot's cells, `g` on the usable rows. No constraint touches the
/// other `h` and `g` values, so they count nowhere; counting them would let one
/// such cell balance the reciprocal of an input outside its table.
fn check_logup_sums<F: PastaField>(relation: &FoldedRelation<F>, witness: &Witness<F>) -> Result<(), Error> {
    for group in &relation.groups {
        let mut sum = F::ZERO;
        for (slot, cells) in group.slots.iter().enumerate() {
            let h = &witness.columns[group.h_column(slot)];
            sum += cells.iter().fold(F::ZERO, |acc, cell| acc + h[cell.row as usize]);
        }
        let table = witness.columns[group.g_column()][..group.usable].iter().fold(F::ZERO, |acc, v| acc + v);
        if sum != table {
            return Err(Error::Rejected("LogUp sum"));
        }
    }
    Ok(())
}

/// The error `sum_i pow_i(beta) f_i(w)` of a witness.
pub(crate) fn error_sum<F: PastaField>(relation: &FoldedRelation<F>, view: View<'_, F>, beta: &[F]) -> F {
    let pb = relation.constraint_bits;
    let pow_lo = pow_vector(&beta[..pb]);
    let pow_hi = pow_vector(&beta[pb..]);
    let mut total = F::ZERO;
    constraints_at(relation, view, |row, index, value| {
        total += pow_hi[row] * pow_lo[index] * value;
    });
    total
}

/// Decides an accumulator natively.
///
/// # Errors
/// [`Error::Rejected`] naming the first failing check, [`Error::Shape`] for a
/// malformed witness.
pub fn decide<C: PastaCurve>(
    relation: &FoldedRelation<C::ScalarExt>,
    key: &CommitKey<C>,
    acc: &Accumulator<C>,
    witness: &Witness<C::ScalarExt>,
) -> Result<(), Error> {
    check_shape(relation, witness)?;
    if acc.beta.len() != relation.index_bits() {
        return Err(Error::Shape("accumulator beta"));
    }
    check_openings(key, &acc.instance, witness)?;
    check_copies(relation, witness, &acc.instance.public)?;
    check_logup_sums(relation, witness)?;
    let view = View::of(witness, &acc.instance.public, &acc.instance.theta, &acc.instance.r);
    if error_sum(relation, view, &acc.beta) != acc.e {
        return Err(Error::Rejected("accumulated error"));
    }
    Ok(())
}

/// Decides a fresh instance natively.
///
/// # Errors
/// As [`decide`]; also rejects foreign `LogUp` challenges or any nonzero constraint.
pub fn decide_fresh<C: PastaCurve>(
    relation: &FoldedRelation<C::ScalarExt>,
    key: &CommitKey<C>,
    domain: &[u8; 32],
    instance: &Instance<C>,
    witness: &Witness<C::ScalarExt>,
) -> Result<(), Error> {
    check_shape(relation, witness)?;
    if !instance.has_fresh_challenges(relation, domain) {
        return Err(Error::Rejected("fresh instance challenges"));
    }
    check_openings(key, instance, witness)?;
    check_copies(relation, witness, &instance.public)?;
    check_logup_sums(relation, witness)?;
    let view = View::of(witness, &instance.public, &instance.theta, &instance.r);
    let mut failed = false;
    constraints_at(relation, view, |_, _, value| failed |= !bool::from(value.is_zero()));
    if failed {
        return Err(Error::Rejected("constraint"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
