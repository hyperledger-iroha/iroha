//! Exact AND/OR/XOR equations using the common Boolean source decomposition.

use super::{F, word::Sources};

fn selected_bit(left: F, right: F, is_and: F, inclusive_or: F, exclusive_or: F) -> F {
    let both = left.mul(right);
    is_and
        .mul(both)
        .add(inclusive_or.mul(left.add(right).sub(both)))
        .add(exclusive_or.mul(left.add(right).sub(both.mul(F(2)))))
}

pub(super) fn append_residues(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    result_digits: &[F],
    sources: Sources<'_>,
    selectors: [F; 3],
) {
    let [is_and, inclusive_or, exclusive_or] = selectors;
    let selected = is_and.add(inclusive_or).add(exclusive_or);
    for (digit, result) in result_digits.iter().enumerate() {
        let low = selected_bit(
            sources.bits(0)[2 * digit],
            sources.bits(1)[2 * digit],
            is_and,
            inclusive_or,
            exclusive_or,
        );
        let high = selected_bit(
            sources.bits(0)[2 * digit + 1],
            sources.bits(1)[2 * digit + 1],
            is_and,
            inclusive_or,
            exclusive_or,
        );
        out.push(selected.mul(*result).sub(low.add(high.mul(F(2)))));
    }
}
