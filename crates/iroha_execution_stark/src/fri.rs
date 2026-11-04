//! FRI low-degree test with arity-`2^a` folding over extension-valued codewords.
//!
//! Layer `l` holds `N_l = N_0 / A^l` evaluations on `shift^(A^l) * <v^(A^l)>`, in natural order.
//! Leaf `j` of a layer holds the `A` positions `j + t*(N_l/A)`, which are the points `x_j * w_A^t`
//! that fold to one point of the next layer. Folding by `A` with challenge `beta` equals `a`
//! binary folds with challenges `beta, beta^2, beta^4, ...` and yields `sum_c beta^c f_c` where
//! `f(X) = sum_c X^c f_c(X^A)`.

use rayon::prelude::*;

use crate::field::{Fp, Fp4};
use crate::merkle::{Digest, MerkleTree, hash_leaf, verify_batch};
use crate::ntt::Domain;
use crate::params::Params;
use crate::transcript::Transcript;

/// The FRI part of a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriProof {
    /// Root of each committed layer.
    pub roots: Vec<Digest>,
    /// Coefficients of the final low-degree polynomial.
    pub final_coefficients: Vec<Fp4>,
    /// Openings of each layer.
    pub layers: Vec<FriLayerProof>,
}

/// Opened leaves of one layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriLayerProof {
    /// Leaves in ascending leaf-index order, each `A` extension elements.
    pub leaves: Vec<Vec<Fp4>>,
    /// Batch-opening siblings.
    pub siblings: Vec<Digest>,
}

struct Layer {
    values: Vec<Fp4>,
    tree: MerkleTree,
}

/// The prover's committed layers.
pub struct FriCommitment {
    layers: Vec<Layer>,
    roots: Vec<Digest>,
    final_coefficients: Vec<Fp4>,
    arity: usize,
}

fn leaf_bytes(values: &[Fp4], leaf: usize, leaves: usize, arity: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(arity * 32);
    for t in 0..arity {
        bytes.extend_from_slice(&values[leaf + t * leaves].to_le_bytes());
    }
    bytes
}

fn inverse_two() -> Fp {
    Fp::TWO.inverse().expect("2 != 0")
}

/// Folds one leaf whose first point is `x` (with `x_inverse = 1/x`).
fn fold_leaf(values: &[Fp4], x: Fp, x_inverse: Fp, beta: Fp4) -> Fp4 {
    let half_inverse = inverse_two();
    let mut current: Vec<Fp4> = values.to_vec();
    let mut len = current.len();
    let (mut xc, mut xc_inv, mut r) = (x, x_inverse, beta);
    while len > 1 {
        let half = len / 2;
        let w_inv = Fp::two_adic_generator(len.trailing_zeros())
            .inverse()
            .expect("root is nonzero");
        let mut y_inv = xc_inv;
        for t in 0..half {
            let (a, b) = (current[t], current[t + half]);
            current[t] = ((a + b) + (r * (a - b)).mul_base(y_inv)).mul_base(half_inverse);
            y_inv *= w_inv;
        }
        len = half;
        xc *= xc;
        xc_inv *= xc_inv;
        r = r * r;
    }
    let _ = xc;
    current[0]
}

fn coefficient_components(values: &[Fp4], shift: Fp) -> [Vec<Fp>; 4] {
    let log_n = values.len().trailing_zeros();
    let domain = Domain::new(log_n.max(1));
    std::array::from_fn(|component| {
        let mut column: Vec<Fp> = values.iter().map(|v| v.0[component]).collect();
        if values.len() == 1 {
            return column;
        }
        domain.inverse_on_coset(&mut column, shift);
        column
    })
}

impl FriCommitment {
    /// Commits to `values` (evaluations on `shift * <w_N>`, natural order) of a polynomial of
    /// degree `< 2^log_degree`.
    ///
    /// # Panics
    /// Panics if the values are not of low degree (a prover bug).
    pub fn commit(
        mut values: Vec<Fp4>,
        mut shift: Fp,
        mut log_degree: u32,
        params: &Params,
        transcript: &mut Transcript,
    ) -> Self {
        let arity = 1usize << params.fri_log_arity;
        let mut layers = Vec::new();
        let mut roots = Vec::new();
        while log_degree > params.fri_log_final_degree {
            let n = values.len();
            let leaves = n / arity;
            let leaf_digests: Vec<Digest> = (0..leaves)
                .into_par_iter()
                .map(|leaf| hash_leaf(&leaf_bytes(&values, leaf, leaves, arity)))
                .collect();
            let tree = MerkleTree::new(leaf_digests);
            let root = tree.root();
            transcript.absorb_digest(b"fri-root", &root);
            let beta = transcript.challenge_fp4(b"fri-beta");
            let v = Fp::two_adic_generator(n.trailing_zeros());
            let v_inv = v.inverse().expect("nonzero");
            let shift_inv = shift.inverse().expect("nonzero");
            let folded: Vec<Fp4> = (0..leaves)
                .into_par_iter()
                .map(|leaf| {
                    let x = shift * v.pow(leaf as u64);
                    let x_inv = shift_inv * v_inv.pow(leaf as u64);
                    let leaf_values: Vec<Fp4> =
                        (0..arity).map(|t| values[leaf + t * leaves]).collect();
                    fold_leaf(&leaf_values, x, x_inv, beta)
                })
                .collect();
            layers.push(Layer { values, tree });
            roots.push(root);
            values = folded;
            shift = shift.pow(arity as u64);
            log_degree = log_degree.saturating_sub(params.fri_log_arity);
        }
        let degree = 1usize << log_degree;
        let components = coefficient_components(&values, shift);
        let final_coefficients: Vec<Fp4> = (0..values.len())
            .map(|i| {
                Fp4([
                    components[0][i],
                    components[1][i],
                    components[2][i],
                    components[3][i],
                ])
            })
            .collect();
        assert!(
            final_coefficients[degree.min(final_coefficients.len())..]
                .iter()
                .all(|c| c.is_zero()),
            "FRI input is not of the claimed degree"
        );
        let final_coefficients =
            final_coefficients[..degree.min(final_coefficients.len())].to_vec();
        transcript.absorb_fp4s(b"fri-final", &final_coefficients);
        Self {
            layers,
            roots,
            final_coefficients,
            arity,
        }
    }

    /// Opens every layer at the layer-0 query positions.
    pub fn open(&self, queries: &[usize]) -> FriProof {
        let mut positions: Vec<usize> = queries.to_vec();
        let mut layers = Vec::with_capacity(self.layers.len());
        for layer in &self.layers {
            let leaves = layer.values.len() / self.arity;
            let mut leaf_indices: Vec<usize> = positions.iter().map(|p| p % leaves).collect();
            leaf_indices.sort_unstable();
            leaf_indices.dedup();
            let opened = leaf_indices
                .iter()
                .map(|&leaf| {
                    (0..self.arity)
                        .map(|t| layer.values[leaf + t * leaves])
                        .collect()
                })
                .collect();
            layers.push(FriLayerProof {
                leaves: opened,
                siblings: layer.tree.open_batch(&leaf_indices),
            });
            positions = positions.iter().map(|p| p % leaves).collect();
        }
        FriProof {
            roots: self.roots.clone(),
            final_coefficients: self.final_coefficients.clone(),
            layers,
        }
    }
}

/// Replays the FRI commit phase on the verifier side: absorbs roots, returns folding challenges.
///
/// Returns `None` if the proof has the wrong number of layers or final coefficients.
pub fn verifier_challenges(
    proof: &FriProof,
    log_degree: u32,
    params: &Params,
    transcript: &mut Transcript,
) -> Option<Vec<Fp4>> {
    let mut layers = 0usize;
    let mut d = log_degree;
    while d > params.fri_log_final_degree {
        layers += 1;
        d = d.saturating_sub(params.fri_log_arity);
    }
    if proof.roots.len() != layers || proof.layers.len() != layers {
        return None;
    }
    let degree = 1usize << d;
    let final_domain = 1usize
        << (log_degree + params.log_blowup - params.fri_log_arity * u32::try_from(layers).ok()?);
    if proof.final_coefficients.len() != degree.min(final_domain) {
        return None;
    }
    let mut betas = Vec::with_capacity(layers);
    for root in &proof.roots {
        transcript.absorb_digest(b"fri-root", root);
        betas.push(transcript.challenge_fp4(b"fri-beta"));
    }
    transcript.absorb_fp4s(b"fri-final", &proof.final_coefficients);
    Some(betas)
}

/// Checks the openings: `initial[i]` is the claimed layer-0 value at `queries[i]`.
pub fn verify_queries(
    proof: &FriProof,
    betas: &[Fp4],
    queries: &[usize],
    initial: &[Fp4],
    log_domain: u32,
    shift: Fp,
    params: &Params,
) -> bool {
    let arity = 1usize << params.fri_log_arity;
    let mut positions: Vec<usize> = queries.to_vec();
    let mut expected: Vec<Fp4> = initial.to_vec();
    let mut n = 1usize << log_domain;
    let mut shift = shift;
    for (layer, (opening, beta)) in proof.layers.iter().zip(betas).enumerate() {
        let leaves = n / arity;
        let mut leaf_indices: Vec<usize> = positions.iter().map(|p| p % leaves).collect();
        leaf_indices.sort_unstable();
        leaf_indices.dedup();
        if opening.leaves.len() != leaf_indices.len()
            || opening.leaves.iter().any(|l| l.len() != arity)
        {
            return false;
        }
        let digests: Vec<(usize, Digest)> = leaf_indices
            .iter()
            .zip(&opening.leaves)
            .map(|(index, values)| {
                let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                (*index, hash_leaf(&bytes))
            })
            .collect();
        let depth = leaves.trailing_zeros();
        if !verify_batch(&proof.roots[layer], depth, &digests, &opening.siblings) {
            return false;
        }
        let v = Fp::two_adic_generator(n.trailing_zeros());
        let mut next_expected = Vec::with_capacity(positions.len());
        for (position, value) in positions.iter().zip(&expected) {
            let leaf = position % leaves;
            let slot = position / leaves;
            let index = leaf_indices.binary_search(&leaf).expect("leaf was opened");
            let leaf_values = &opening.leaves[index];
            if leaf_values[slot] != *value {
                return false;
            }
            let x = shift * v.pow(leaf as u64);
            let Some(x_inv) = x.inverse() else {
                return false;
            };
            next_expected.push(fold_leaf(leaf_values, x, x_inv, *beta));
        }
        positions = positions.iter().map(|p| p % leaves).collect();
        expected = next_expected;
        n = leaves;
        shift = shift.pow(arity as u64);
    }
    let v = Fp::two_adic_generator(n.trailing_zeros().max(1));
    for (position, value) in positions.iter().zip(&expected) {
        let x = if n == 1 {
            shift
        } else {
            shift * v.pow(*position as u64)
        };
        let mut acc = Fp4::ZERO;
        for c in proof.final_coefficients.iter().rev() {
            acc = acc.mul_base(x) + *c;
        }
        if acc != *value {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn low_degree_codeword(log_degree: u32, log_blowup: u32, shift: Fp) -> Vec<Fp4> {
        let n = 1usize << (log_degree + log_blowup);
        let domain = Domain::new(log_degree + log_blowup);
        let components: [Vec<Fp>; 4] = std::array::from_fn(|c| {
            let mut coefficients = vec![Fp::ZERO; n];
            for (i, slot) in coefficients.iter_mut().take(1 << log_degree).enumerate() {
                *slot = Fp::new((i as u64 + 1) * 7919 + c as u64);
            }
            domain.forward_on_coset(&mut coefficients, shift);
            coefficients
        });
        (0..n)
            .map(|i| {
                Fp4([
                    components[0][i],
                    components[1][i],
                    components[2][i],
                    components[3][i],
                ])
            })
            .collect()
    }

    #[test]
    fn fri_accepts_low_degree_and_rejects_tampering() {
        let params = Params {
            log_blowup: 3,
            queries: 20,
            fri_log_arity: 3,
            fri_log_final_degree: 2,
            grinding_bits: 0,
        };
        let shift = Fp::new(7);
        let log_degree = 10;
        let values = low_degree_codeword(log_degree, 3, shift);
        let mut prover = Transcript::new(b"fri-test");
        let commitment =
            FriCommitment::commit(values.clone(), shift, log_degree, &params, &mut prover);
        let queries = prover.challenge_unique_indices(b"q", params.queries, log_degree + 3);
        let proof = commitment.open(&queries);
        let initial: Vec<Fp4> = queries.iter().map(|q| values[*q]).collect();

        let mut verifier = Transcript::new(b"fri-test");
        let betas = verifier_challenges(&proof, log_degree, &params, &mut verifier).unwrap();
        let verifier_queries =
            verifier.challenge_unique_indices(b"q", params.queries, log_degree + 3);
        assert_eq!(queries, verifier_queries);
        assert!(verify_queries(
            &proof,
            &betas,
            &queries,
            &initial,
            log_degree + 3,
            shift,
            &params
        ));

        let mut bad_initial = initial.clone();
        bad_initial[3] += Fp4::ONE;
        assert!(!verify_queries(
            &proof,
            &betas,
            &queries,
            &bad_initial,
            log_degree + 3,
            shift,
            &params
        ));
        let mut bad = proof.clone();
        bad.final_coefficients[0] += Fp4::ONE;
        assert!(!verify_queries(
            &bad,
            &betas,
            &queries,
            &initial,
            log_degree + 3,
            shift,
            &params
        ));
        let mut bad = proof.clone();
        bad.layers[1].leaves[0][0] += Fp4::ONE;
        assert!(!verify_queries(
            &bad,
            &betas,
            &queries,
            &initial,
            log_degree + 3,
            shift,
            &params
        ));
    }

    #[test]
    #[should_panic(expected = "not of the claimed degree")]
    fn fri_prover_refuses_high_degree_input() {
        let params = Params {
            log_blowup: 3,
            queries: 8,
            fri_log_arity: 3,
            fri_log_final_degree: 2,
            grinding_bits: 0,
        };
        let shift = Fp::new(7);
        let values = low_degree_codeword(9, 3, shift);
        let mut transcript = Transcript::new(b"fri-test");
        // Claim degree 2^6 for a degree-2^9 polynomial.
        let truncated: Vec<Fp4> = values.iter().step_by(8).copied().collect();
        let _ = FriCommitment::commit(truncated, shift.pow(8), 6, &params, &mut transcript);
    }
}
