//! Sumcheck round messages.
//!
//! A round polynomial `s` of degree at most `d` is sent as `s(0), s(2), ...,
//! s(d)`; the verifier sets `s(1) = claim - s(0)`, so `s(0) + s(1)` equals the
//! running claim by construction, and the next claim is `s(rho)` for the round
//! challenge `rho`.

use ff::PrimeField;

/// The evaluation points of a round polynomial of degree `degree` that the
/// prover sends: `0, 2, 3, ..., degree` (the prover walks them implicitly).
#[cfg(test)]
pub(crate) fn points<F: PrimeField>(degree: usize) -> Vec<F> {
    core::iter::once(0u64)
        .chain(2..=u64::try_from(degree).expect("degree fits u64"))
        .map(F::from)
        .collect()
}

/// Evaluates at `x` the polynomial of degree `< values.len()` with
/// `values[t] = p(t)` (Lagrange over the nodes `0..values.len()`).
pub(crate) fn interpolate<F: PrimeField>(values: &[F], x: F) -> F {
    let nodes: Vec<F> = (0..values.len()).map(|t| F::from(t as u64)).collect();
    let mut total = F::ZERO;
    for (i, value) in values.iter().enumerate() {
        let mut numerator = F::ONE;
        let mut denominator = F::ONE;
        for (j, node) in nodes.iter().enumerate() {
            if i != j {
                numerator *= x - node;
                denominator *= nodes[i] - node;
            }
        }
        total += *value * numerator * denominator.invert().expect("distinct nodes");
    }
    total
}

/// One verifier round: from the running `claim` and the sent values
/// `s(0), s(2), ..., s(d)`, the next claim `s(rho)`.
pub(crate) fn next_claim<F: PrimeField>(claim: F, sent: &[F], rho: F) -> F {
    let mut values = Vec::with_capacity(sent.len() + 1);
    values.push(sent[0]);
    values.push(claim - sent[0]);
    values.extend_from_slice(&sent[1..]);
    interpolate(&values, rho)
}
