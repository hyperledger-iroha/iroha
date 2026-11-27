//! A companion-matrix basis for the two linear words of RP57's partial rounds.

use super::{PARTIAL_ROUNDS, WIDTH, pow5};
use crate::field::PastaField;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PartialRounds<F> {
    basis: [[F; 2]; 2],
    inverse_basis: [[F; 2]; 2],
    nonlinear_column: [F; WIDTH],
    trace: F,
    neg_determinant: F,
    constants: [F; PARTIAL_ROUNDS],
    final_offset: [F; WIDTH],
}

#[inline]
fn dot<F: PastaField>(a: [F; 2], b: [F; 2]) -> F {
    a[0] * b[0] + a[1] * b[1]
}

impl<F: PastaField> PartialRounds<F> {
    pub(super) fn new(mds: &[[F; WIDTH]; WIDTH], constants: &[[F; WIDTH]]) -> Option<Self> {
        // With the MDS partitioned as [m a; v B], write Y for the two passive
        // state words and change coordinates to p = aY, q = aBY.
        let [[m, a0, a1], [v0, b00, b01], [v1, b10, b11]] = *mds;
        let a = [a0, a1];
        let ab = [dot(a, [b00, b10]), dot(a, [b01, b11])];
        let determinant = a0 * ab[1] - a1 * ab[0];
        let inv = determinant.invert_vartime()?; // Only the public MDS enters here.
        let inverse_basis = [[ab[1] * inv, -a1 * inv], [-ab[0] * inv, a0 * inv]];

        // Translate the two passive round constants through the linear layer.
        // At each S-box its input is the active word plus its accumulated offset.
        let mut offset = [F::ZERO; WIDTH];
        let constants = std::array::from_fn(|round| {
            let rc = constants[round];
            let active = rc[0] + offset[0];
            let passive = [offset[1] + rc[1], offset[2] + rc[2]];
            offset = [
                dot(a, passive),
                dot([b00, b01], passive),
                dot([b10, b11], passive),
            ];
            active
        });
        Some(Self {
            basis: [a, ab],
            inverse_basis,
            nonlinear_column: [m, dot(a, [v0, v1]), dot(ab, [v0, v1])],
            trace: b00 + b11,
            neg_determinant: b01 * b10 - b00 * b11,
            constants,
            final_offset: offset,
        })
    }

    pub(super) fn apply(&self, state: &mut [F; WIDTH]) {
        let mut active = state[0];
        let passive = [state[1], state[2]];
        let (mut p, mut q) = (dot(self.basis[0], passive), dot(self.basis[1], passive));
        let [m, av, abv] = self.nonlinear_column;
        for &constant in &self.constants {
            let sbox = pow5(active + constant);
            // Cayley–Hamilton gives aB²Y = tr(B) q - det(B) p.
            (active, p, q) = (
                m * sbox + p,
                av * sbox + q,
                abv * sbox + self.trace * q + self.neg_determinant * p,
            );
        }
        let [ox, oy, oz] = self.final_offset;
        *state = [
            active + ox,
            dot(self.inverse_basis[0], [p, q]) + oy,
            dot(self.inverse_basis[1], [p, q]) + oz,
        ];
    }
}
