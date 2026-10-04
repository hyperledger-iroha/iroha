//! Workspace runner for the vendored halo2-axiom golden proof bytes, and the
//! native `iroha_plonk` parity suites over the same cases.
//!
//! `vendor/halo2-axiom/tests/golden_proof_bytes.rs` has no runner of its own:
//! Cargo refuses `cargo test -p halo2-axiom` for the patched non-member. This
//! target compiles that file against the patched halo2-axiom and
//! halo2curves-axiom that `iroha_core_zk` links, with the same features.
//! Nothing under `vendor/` is edited: `build.rs` copies the file into
//! `OUT_DIR` with its `//!` lines turned into `//` (the only change; `include!`
//! rejects inner doc comments), and the `golden_proof_bytes` module includes
//! the copy. Its test `includable_copy_is_exact` checks that the copy differs
//! from the vendored file only in those lines.
//!
//! Vendored goldens (unchanged): every case proves the sigma-shaped (k = 6 and
//! 9) and wide (k = 8 and 10) circuits over both Pasta cycles with prover
//! seeds 42 and 43, inside Rayon pools of 1, 2, 4 and 7 threads. The bytes
//! must not depend on the pool, each proof must verify, and its SHA-256 must
//! equal the vendored constant. The k = 11 sigma cases are ignored release
//! cases.
//!
//! Native parity (milestones M1b and M1c), over the same circuits, seeds and
//! constants:
//!
//! - `export_parity` (every build): each case is exported into the native IR
//!   (`iroha_plonk_oracle::export`). The replayed constraint system equals
//!   the vendored one, the native selector compression equals the vendored
//!   `VerifyingKey::cs`, the exported advice, fixed (selector columns checked
//!   on their own) and permutation tables equal the vendored `MockProver`'s,
//!   native keygen and parameters do not depend on the pool size, and the
//!   native verifying-key and parameter bytes equal the vendored ones.
//! - `proof_parity` (oracle builds, `--cfg iroha_plonk_oracle`): the native
//!   prover with the injected vendored `transcript_repr` re-proves every
//!   golden case in pools of 1, 2, 4 and 7 threads; the bytes must not depend
//!   on the pool and their SHA-256 must equal the vendored constant. Both
//!   verifiers accept every golden.
//! - `verdict_parity` (oracle builds): structure-aware tamper corpora of at
//!   least 200 cases per family and curve; the native verifier must return
//!   the vendored verdict on every case, except the registered stricter
//!   rejections (`deviation_registry::DEVIATIONS`), each with its typed
//!   native reason.
//! - `deviation_registry` (every build): every `DEV-xx` row of spec section
//!   14 names a native test that exists and mentions it, and every oracle
//!   deviation names a `both`-mode row.
//! - `kagemusha_vendored` (every build) and `kagemusha_parity` (oracle
//!   builds): the KAGEMUSHA proving path of `iroha_core_zk` (Poseidon
//!   transcript, folded-generator suffix) over the same circuits; the native
//!   prover must reproduce its bytes at 1, 2, 4 and 7 threads, and the tamper
//!   corpora run on this path too.
//!
//! Every setup is built once per process (`cases::setup`) and shared by the
//! suites.
//! - `timing` (oracle builds, ignored): native against vendored prove time on
//!   the k = 11 sigma golden at 1 and 4 threads.
//!
//! Oracle baseline (recorded with the vectors in
//! `fixtures/native_prover/kats_v1.json`, section `oracle_baseline`): repository
//! HEAD `1de7210a74d62ae5232c67b910dbf4b6b1bcf757`, last `vendor/halo2-axiom`
//! commit `8f41274044c93ad7e363fdc8be30a671efb436d4`. The golden table itself is
//! pinned in the same fixture (section `golden_proofs`), so a vendored constant
//! that changes fails `tests/native_prover_kats.rs` as well.
//!
//! Run (oracle builds use their own target directory, because `RUSTFLAGS`
//! changes rebuild everything; the oracle run is manual today, TODO: a CI
//! job):
//! - `cargo test -p iroha_plonk_oracle --test vendored_goldens`
//! - `RUSTFLAGS="--cfg iroha_plonk_oracle" cargo test -p iroha_plonk_oracle --test vendored_goldens`
//! - `RUSTFLAGS="--cfg iroha_plonk_oracle" cargo test --release -p iroha_plonk_oracle --test vendored_goldens -- --include-ignored`
//! - timing: `RUSTFLAGS="--cfg iroha_plonk_oracle" cargo test --release -p iroha_plonk_oracle --test vendored_goldens timing -- --ignored --nocapture --test-threads=1`

/// The vendored golden file (included unchanged but for its `//!` lines) and
/// the small interface the native suites use: the vendored circuits are
/// private to this module, so they are handed out through `GoldenVisitor`.
// The vendored file is reviewed upstream code that this crate must not edit;
// its pedantic lints belong to its own crate, not here.
#[allow(clippy::pedantic)]
mod golden_proof_bytes {
    include!(concat!(env!("OUT_DIR"), "/golden_proof_bytes.rs"));

    /// `(case name, SHA-256 hex)` of every vendored golden proof.
    pub const GOLDEN_CASES: &[(&str, &str)] = GOLDEN_SHA256;
    /// The vendored prover seeds.
    pub const SEEDS: [[u8; 32]; 2] = PROVER_SEEDS;

    /// Receives one golden circuit, its public input and its constraint-system
    /// shape `(degree, permutation columns)`.
    pub trait GoldenVisitor<F: PrimeField> {
        /// What the visitor returns.
        type Output;

        /// Visits the circuit.
        fn visit<C: Circuit<F> + Clone + Send + Sync + 'static>(
            self,
            circuit: C,
            public: Vec<F>,
            shape: (usize, usize),
        ) -> Self::Output;
    }

    /// Builds the golden circuit of `family` (`"sigma"` or `"wide"`) at `k`
    /// and hands it to `visitor`; `None` for an unknown family.
    pub fn visit_golden<F: PrimeField + 'static, V: GoldenVisitor<F>>(
        family: &str,
        k: u32,
        visitor: V,
    ) -> Option<V::Output> {
        match family {
            "sigma" => {
                let circuit = SigmaCircuit::<F>::new(k);
                let public = circuit.public();
                let shape = (SIGMA_SHAPE.degree, SIGMA_SHAPE.permutation_columns);
                Some(visitor.visit(circuit, public, shape))
            }
            "wide" => {
                let circuit = WideCircuit::<F>::new(k);
                let public = circuit.public();
                let shape = (WIDE_SHAPE.degree, WIDE_SHAPE.permutation_columns);
                Some(visitor.visit(circuit, public, shape))
            }
            _ => None,
        }
    }

    /// The vendored golden prover (`prove_once`): one proof with the
    /// `Blake2b` transcript and the `ChaCha20` `seed`.
    pub fn vendored_prove<C, Circ>(
        params: &ParamsIPA<C>,
        pk: &ProvingKey<C>,
        circuit: &Circ,
        public: &[C::Scalar],
        seed: [u8; 32],
    ) -> Vec<u8>
    where
        C: CurveAffine,
        C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
        Circ: Circuit<C::Scalar> + Clone,
    {
        prove_once(params, pk, circuit, public, seed)
    }

    /// The hex SHA-256 the golden table records.
    pub fn digest_hex(bytes: &[u8]) -> String {
        sha256_hex(bytes)
    }

    #[test]
    fn includable_copy_is_exact() {
        let copy = include_str!(concat!(env!("OUT_DIR"), "/golden_proof_bytes.rs"));
        let vendored = include_str!("../../../vendor/halo2-axiom/tests/golden_proof_bytes.rs");
        assert_eq!(copy.lines().count(), vendored.lines().count());
        let mut changed = 0;
        for (copied, original) in copy.lines().zip(vendored.lines()) {
            if copied != original {
                changed += 1;
                let indent = original.len() - original.trim_start().len();
                let (head, rest) = original.split_at(indent);
                let comment = rest.strip_prefix("//!").expect("only //! lines change");
                assert_eq!(copied, format!("{head}//{comment}"));
            }
        }
        assert!(
            changed > 0,
            "the vendored file starts with inner doc comments"
        );
    }

    #[test]
    fn visitor_hands_out_both_families() {
        struct Shape;
        impl<F: PrimeField> GoldenVisitor<F> for Shape {
            type Output = (usize, usize, usize);
            fn visit<C: Circuit<F> + Clone + Send + Sync + 'static>(
                self,
                _: C,
                public: Vec<F>,
                shape: (usize, usize),
            ) -> Self::Output {
                (public.len(), shape.0, shape.1)
            }
        }
        use halo2_axiom::halo2curves::pasta::Fp;
        assert_eq!(visit_golden::<Fp, _>("sigma", 6, Shape), Some((1, 7, 5)));
        assert_eq!(visit_golden::<Fp, _>("wide", 8, Shape), Some((1, 4, 13)));
        assert_eq!(visit_golden::<Fp, _>("other", 8, Shape), None);
        assert_eq!(GOLDEN_CASES.len(), 20);
        assert_eq!(SEEDS, [[42; 32], [43; 32]]);
        assert_eq!(digest_hex(b"").len(), 64);
    }
}

// The native suites live in `tests/vendored_goldens/`. This file is the
// target root, and Cargo resolves a test root's `mod` declarations from
// `tests/` itself, so each submodule needs its `#[path]`.
#[path = "vendored_goldens/cases.rs"]
mod cases;
#[path = "vendored_goldens/deviation_registry.rs"]
mod deviation_registry;
#[path = "vendored_goldens/export_parity.rs"]
mod export_parity;
#[cfg(iroha_plonk_oracle)]
#[path = "vendored_goldens/kagemusha_parity.rs"]
mod kagemusha_parity;
#[path = "vendored_goldens/kagemusha_vendored.rs"]
mod kagemusha_vendored;
#[cfg(iroha_plonk_oracle)]
#[path = "vendored_goldens/proof_parity.rs"]
mod proof_parity;
#[cfg(iroha_plonk_oracle)]
#[path = "vendored_goldens/timing.rs"]
mod timing;
#[cfg(iroha_plonk_oracle)]
#[path = "vendored_goldens/verdict_parity.rs"]
mod verdict_parity;
