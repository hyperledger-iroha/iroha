//! Private producer-to-history equations with public padded geometry only.
//!
//! Every ordered packet equals the original complete execution producer's
//! committed columns. The history uses the same typed tuples and post-base
//! permutation challenges, while its event values and enabled count remain
//! witness fields. There is no standalone public event digest or new verifier.
// TODO: Supply these borrowed producer columns from the exhaustive constrained
// machine dispatcher, and bind original initial state, frame lifecycle and final
// invocation semantics. This internal bank is not a production execution proof.

use super::*;

/// Shape-only fixed schedule shared by every witness with the same public cap.
#[derive(Clone, Copy)]
pub(super) struct Schedule {
    trace_log2: u8,
}

impl Schedule {
    pub(super) fn new(trace_log2: u8) -> Option<Self> {
        (MIN_LOG..=MAX_LOG)
            .contains(&trace_log2)
            .then_some(Self { trace_log2 })
    }

    pub(super) fn size(self) -> usize {
        1 << self.trace_log2
    }

    pub(super) fn fixed(self, index: usize) -> Option<[F; FIXED_WIDTH]> {
        if index >= self.size() {
            return None;
        }
        let mut fixed = [F::ZERO; FIXED_WIDTH];
        fixed[PHASE_OFFSET + index % PHASES] = F::ONE;
        fixed[TRANSITION] = F(u64::from(index + 1 < self.size()));
        fixed[FIRST] = F(u64::from(index == 0));
        fixed[LAST] = F(u64::from(index + 1 == self.size()));
        fixed[SLOT] = F((index / PHASES) as u64);
        // The former public event tuple and TOTAL cells remain zero. Neither
        // their replacement nor the actual activity pattern is public metadata.
        Some(fixed)
    }
}

/// Append the private bank to the same committed execution relation.
///
/// `producer` is an original trace-column port, not a host callback or separately
/// supplied event commitment. Its complete semantic producer must also be
/// constrained in that relation; consistency alone cannot authorize its writes.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    row: &[F; ROW_WIDTH],
    next: &[F; ROW_WIDTH],
    aux: &[F],
    next_aux: &[F],
    fixed: &[F; FIXED_WIDTH],
    producer: &[F; packet::WIDTH],
    challenges: &permutation::Challenges,
) {
    let start = out.len();
    for (actual, original) in row[ORDERED..SORTED].iter().zip(producer) {
        out.push(actual.sub(*original));
    }
    sorted::append_private_residues(out, row, next, fixed);
    permutation::append_residues(
        out,
        aux,
        next_aux,
        &row[ORDERED..SORTED],
        &row[SORTED..PREVIOUS],
        [
            fixed[PHASE_OFFSET + PHASES - 1],
            fixed[TRANSITION],
            fixed[FIRST],
            fixed[LAST],
        ],
        challenges,
    );
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

#[cfg(test)]
mod tests;
