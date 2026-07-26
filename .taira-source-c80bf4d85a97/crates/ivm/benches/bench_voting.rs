//! Benchmark: zero-knowledge voting with Pedersen commitments and Sigma-OR proofs (no trusted setup).
//!
//! This replaces the previous placeholder with a real ZK construction:
//! - Each voter commits to a bit `v ∈ {0,1}` as `C = v·G + r·H` on BLS12-381.
//! - Provides a non-interactive Sigma-OR proof that the committed value is either 0 or 1
//!   without revealing which (Fiat–Shamir via SHA-256). No trusted setup needed.
//! - Aggregator verifies all proofs, homomorphically sums commitments, and opens the
//!   tally using the sum of blindings (simulated here for the benchmark). The final
//!   winner is derived from the tally; individual votes remain hidden.
use std::sync::LazyLock;

use blstrs::{G1Affine, G1Projective, Scalar};
use criterion::Criterion;
use group::{Curve, Group};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

// Fixed generators in affine form to speed up fixed-base scalar muls.
static G_GENERATOR_AFFINE: LazyLock<G1Affine> =
    LazyLock::new(|| G1Projective::generator().to_affine());
static H_GENERATOR_AFFINE: LazyLock<G1Affine> = LazyLock::new(|| {
    // Derive H deterministically from G (benchmark; not for production).
    (G1Projective::generator() * Scalar::from(7u64)).to_affine()
});

#[inline]
fn scalar_from_u64_with_label(x: u64, label: &[u8]) -> Scalar {
    // Deterministic pseudo-random scalar from (label || x)
    let mut h = Sha256::new();
    h.update(label);
    h.update(x.to_le_bytes());
    let h1 = h.finalize();
    // Reduce to 64-bit and lift into the scalar field deterministically.
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&h1[..8]);
    Scalar::from(u64::from_le_bytes(arr))
}

#[inline]
fn pedersen_commit_bit(v: u64, r: Scalar) -> G1Projective {
    debug_assert!(v == 0 || v == 1);
    // Use affine fixed-base muls for speed.
    (*G_GENERATOR_AFFINE) * Scalar::from(v) + (*H_GENERATOR_AFFINE) * r
}

#[inline]
fn to_bytes(p: &G1Projective) -> [u8; 48] {
    p.to_affine().to_compressed()
}

fn fs_challenge(c: &G1Projective, t0: &G1Projective, t1: &G1Projective) -> Scalar {
    let mut h = Sha256::new();
    h.update(to_bytes(c));
    h.update(to_bytes(t0));
    h.update(to_bytes(t1));
    let h1 = h.finalize();
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&h1[..8]);
    Scalar::from(u64::from_le_bytes(arr))
}

// Sigma-OR proof that C commits to either 0 or 1 (bit proof).
struct BitProof {
    t0: G1Projective,
    t1: G1Projective,
    e0: Scalar,
    s0: Scalar,
    s1: Scalar,
}

// Minimal clone-like helper to avoid deriving Clone for heavy points in the hot path.
impl BitProof {
    fn clone_for_batch(&self) -> BitProof {
        BitProof {
            t0: self.t0,
            t1: self.t1,
            e0: self.e0,
            s0: self.s0,
            s1: self.s1,
        }
    }
}

fn prove_bit(c: &G1Projective, v: u64, r: Scalar, idx: u64) -> BitProof {
    assert!(v == 0 || v == 1);

    // Deterministic scalars for benchmarking (avoid external RNG):
    // - alpha_real from ("alpha", idx)
    // - e_fake, s_fake from ("e_fake"/"s_fake", idx)
    let alpha = scalar_from_u64_with_label(idx, b"alpha");
    let e_fake = scalar_from_u64_with_label(idx, b"e_fake");
    let s_fake = scalar_from_u64_with_label(idx, b"s_fake");

    // Real branch commitment: T_real = alpha * H
    // Fake branch commitment: T_fake = s_fake * H - e_fake * (C - j*G)
    let g = *G_GENERATOR_AFFINE;
    let h = *H_GENERATOR_AFFINE;

    let (t0, t1, e0, s0, s1) = if v == 0 {
        let t0 = g1_mul_affine(h, alpha); // real branch is 0
        let c1 = *c - g1_mul_affine(g, Scalar::from(1u64)); // C - 1*G
        let t1 = g1_mul_affine(h, s_fake) - c1 * e_fake;
        let e = fs_challenge(c, &t0, &t1);
        let e0 = e - e_fake; // challenge split
        let s0 = alpha + e0 * r; // response for real branch
        (t0, t1, e0, s0, s_fake)
    } else {
        let c0 = *c; // C - 0*G
        let t0 = g1_mul_affine(h, s_fake) - c0 * e_fake;
        let t1 = g1_mul_affine(h, alpha); // real branch is 1
        let e = fs_challenge(c, &t0, &t1);
        let e1 = e - e_fake;
        let s1 = alpha + e1 * r;
        (t0, t1, e_fake, s_fake, s1)
    };

    BitProof { t0, t1, e0, s0, s1 }
}

fn verify_bit(c: &G1Projective, proof: &BitProof) -> bool {
    let g = *G_GENERATOR_AFFINE;
    let h = *H_GENERATOR_AFFINE;
    let e = fs_challenge(c, &proof.t0, &proof.t1);
    let e1 = e - proof.e0;

    // Check: s0·H = T0 + e0·(C - 0·G) and s1·H = T1 + e1·(C - 1·G)
    let left0 = g1_mul_affine(h, proof.s0);
    let right0 = proof.t0 + (*c) * proof.e0; // (C - 0*G) = C
    if left0 != right0 {
        return false;
    }
    let left1 = g1_mul_affine(h, proof.s1);
    let right1 = proof.t1 + (*c - g1_mul_affine(g, Scalar::from(1u64))) * e1; // (C - 1*G)
    if left1 != right1 {
        return false;
    }
    true
}

#[inline]
fn g1_mul_affine(base: G1Affine, scalar: Scalar) -> G1Projective {
    // Helper so we can write the same expression style while ensuring
    // we hit the faster fixed-base multiplication path on blstrs.
    base * scalar
}

#[inline]
fn g1_msm_proj(bases: &[G1Projective], scalars: &[Scalar]) -> G1Projective {
    // Fast multi-scalar multiplication provided by blstrs.
    G1Projective::multi_exp(bases, scalars)
}

fn verify_bit_batch(items: &[(G1Projective, BitProof, u64)]) -> bool {
    // Deterministic batch randomizers derived from index: r_i = H("batch" || i)
    // Aggregated equations:
    //  1) H * sum(r_i*s0_i) == sum(r_i*T0_i) + sum((r_i*e0_i)*C_i)
    //  2) H * sum(r_i*s1_i) == sum(r_i*T1_i) + sum((r_i*e1_i)*C_i) - G * sum(r_i*e1_i)

    let mut s0_acc = Scalar::from(0u64);
    let mut s1_acc = Scalar::from(0u64);
    let mut ge1_acc = Scalar::from(0u64);

    let mut c_pts = Vec::with_capacity(items.len());
    let mut t0_pts = Vec::with_capacity(items.len());
    let mut t1_pts = Vec::with_capacity(items.len());

    let mut r_scalars = Vec::with_capacity(items.len());
    let mut alpha0 = Vec::with_capacity(items.len());
    let mut alpha1 = Vec::with_capacity(items.len());

    for (c, proof, idx) in items.iter() {
        // Recompute Fiat–Shamir challenge
        let e = fs_challenge(c, &proof.t0, &proof.t1);
        let e1 = e - proof.e0;

        let r_i = scalar_from_u64_with_label(*idx, b"batch");
        s0_acc += r_i * proof.s0;
        s1_acc += r_i * proof.s1;
        ge1_acc += r_i * e1;

        c_pts.push(*c);
        t0_pts.push(proof.t0);
        t1_pts.push(proof.t1);

        r_scalars.push(r_i);
        alpha0.push(r_i * proof.e0);
        alpha1.push(r_i * e1);
    }

    let g = *G_GENERATOR_AFFINE;
    let h = *H_GENERATOR_AFFINE;

    // Left sides: fixed-base multiplies
    let left0 = g1_mul_affine(h, s0_acc);
    let left1 = g1_mul_affine(h, s1_acc);

    // Right sides: MSMs over t0/t1 and commitments with respective scalars
    let right0 = g1_msm_proj(&t0_pts, &r_scalars) + g1_msm_proj(&c_pts, &alpha0);
    let right1 =
        g1_msm_proj(&t1_pts, &r_scalars) + g1_msm_proj(&c_pts, &alpha1) - g1_mul_affine(g, ge1_acc);

    left0.to_affine() == right0.to_affine() && left1.to_affine() == right1.to_affine()
}

fn bench_zk_voting(c: &mut Criterion) {
    // Default voter count can be overridden via VOTERS env var.
    // Large N may take a long time with real proofs; defaults to 100_000.
    let voters: u64 = std::env::var("VOTERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);

    // Optional: only verify every K-th proof (K=1 verifies all).
    let verify_every: u64 = std::env::var("VERIFY_EVERY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let threads = num_cpus::get_physical();
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();

    c.bench_function("zk_voting_sigma_or", |b| {
        b.iter(|| {
            let verify_batch = std::env::var("VERIFY_BATCH")
                .ok()
                .map(|s| matches!(s.as_str(), "1" | "true" | "yes"))
                .unwrap_or(false);

            // Map over voters to build commitments and proofs.
            if verify_batch && verify_every == 1 {
                // Batch path: collect all items, run one aggregated check, then sum.
                let items: Vec<_> = (0..voters)
                    .into_par_iter()
                    .map(|i| {
                        let v = if i % 2 == 0 { 1u64 } else { 0u64 };
                        let r = scalar_from_u64_with_label(i, b"blind");
                        let c = pedersen_commit_bit(v, r);
                        let proof = prove_bit(&c, v, r, i);
                        (c, r, v, proof, i)
                    })
                    .collect();

                // Verify batch
                let proofs: Vec<_> = items
                    .iter()
                    .map(|(c, _r, _v, p, i)| (*c, p.clone_for_batch(), *i))
                    .collect();
                assert!(verify_bit_batch(&proofs), "batch bit proof failed");

                // Reduce sums sequentially (cost negligible vs curve ops)
                let mut sum_c = G1Projective::identity();
                let mut sum_r = Scalar::from(0u64);
                let mut yes_count = 0u64;
                for (c, r, v, _p, _i) in items.into_iter() {
                    sum_c += c;
                    sum_r += r;
                    yes_count += v;
                }

                // Open the aggregated commitment: C_sum ?= yes_count·G + R_sum·H
                let g = *G_GENERATOR_AFFINE;
                let h = *H_GENERATOR_AFFINE;
                let opened = g1_mul_affine(g, Scalar::from(yes_count)) + g1_mul_affine(h, sum_r);
                assert_eq!(
                    opened.to_affine(),
                    sum_c.to_affine(),
                    "aggregate opening failed"
                );

                let a = yes_count;
                let b = voters - yes_count;
                let winner = if a >= b { a } else { b };
                std::hint::black_box(winner);
                return;
            }

            // Default path: streaming reduce with optional subsampled per-proof verification.
            let (sum_c, sum_r, yes_count) = (0..voters)
                .into_par_iter()
                .map(|i| {
                    // Deterministic vote and blinding for reproducibility.
                    let v = if i % 2 == 0 { 1u64 } else { 0u64 };
                    let r = scalar_from_u64_with_label(i, b"blind");
                    let c = pedersen_commit_bit(v, r);
                    let proof = prove_bit(&c, v, r, i);
                    if verify_every == 1 || i % verify_every == 0 {
                        assert!(verify_bit(&c, &proof), "invalid bit proof");
                    }
                    (c, r, v)
                })
                .reduce(
                    || (G1Projective::identity(), Scalar::from(0u64), 0u64),
                    |acc, item| (acc.0 + item.0, acc.1 + item.1, acc.2 + item.2),
                );

            // Open the aggregated commitment: C_sum ?= yes_count·G + R_sum·H
            let g = *G_GENERATOR_AFFINE;
            let h = *H_GENERATOR_AFFINE;
            let opened = g1_mul_affine(g, Scalar::from(yes_count)) + g1_mul_affine(h, sum_r);
            assert_eq!(
                opened.to_affine(),
                sum_c.to_affine(),
                "aggregate opening failed"
            );

            // Winner: A (yes) vs B (no)
            let a = yes_count;
            let b = voters - yes_count;
            let winner = if a >= b { a } else { b };
            std::hint::black_box(winner);
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_zk_voting(&mut c);
    c.final_summary();
}
