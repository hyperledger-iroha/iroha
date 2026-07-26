//! Utilities for testing
use crate::{
    gates::{
        circuit::{builder::RangeCircuitBuilder, BaseCircuitParams, CircuitBuilderStage},
        flex_gate::threads::SinglePhaseCoreManager,
        GateChip, RangeChip,
    },
    halo2_proofs::{
        dev::MockProver,
        halo2curves::bn256::{Bn256, Fr, G1Affine},
        plonk::{
            create_proof, keygen_pk, keygen_vk, verify_proof, Circuit, ProvingKey, VerifyingKey,
        },
        poly::commitment::ParamsProver,
        poly::kzg::{
            commitment::KZGCommitmentScheme, commitment::ParamsKZG, multiopen::ProverSHPLONK,
            multiopen::VerifierSHPLONK, strategy::SingleStrategy,
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    },
    Context,
};
use ark_std::{end_timer, perf_trace::TimerInfo, start_timer};
use rand::{rngs::StdRng, SeedableRng};

use super::fs::gen_srs;

/// Helper function to generate a proof with real prover using SHPLONK KZG multi-open polynomical commitment scheme
/// and Blake2b as the hash function for Fiat-Shamir.
pub fn gen_proof_with_instances(
    params: &ParamsKZG<Bn256>,
    pk: &ProvingKey<G1Affine>,
    circuit: impl Circuit<Fr>,
    instances: &[&[Fr]],
) -> Vec<u8> {
    let rng = StdRng::seed_from_u64(0);
    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    create_proof::<
        KZGCommitmentScheme<Bn256>,
        ProverSHPLONK<'_, Bn256>,
        Challenge255<_>,
        _,
        Blake2bWrite<Vec<u8>, G1Affine, _>,
        _,
    >(params, pk, &[circuit], &[instances], rng, &mut transcript)
    .expect("prover should not fail");
    transcript.finalize()
}

/// For testing use only: Helper function to generate a proof **without public instances** with real prover using SHPLONK KZG multi-open polynomical commitment scheme
/// and Blake2b as the hash function for Fiat-Shamir.
pub fn gen_proof(
    params: &ParamsKZG<Bn256>,
    pk: &ProvingKey<G1Affine>,
    circuit: impl Circuit<Fr>,
) -> Vec<u8> {
    gen_proof_with_instances(params, pk, circuit, &[])
}

/// Helper function to verify a proof (generated using [`gen_proof_with_instances`]) using SHPLONK KZG multi-open polynomical commitment scheme
/// and Blake2b as the hash function for Fiat-Shamir.
pub fn check_proof_with_instances(
    params: &ParamsKZG<Bn256>,
    vk: &VerifyingKey<G1Affine>,
    proof: &[u8],
    instances: &[&[Fr]],
    expect_satisfied: bool,
) {
    let verifier_params = params.verifier_params();
    let strategy = SingleStrategy::new(params);
    let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(proof);
    let res = verify_proof::<
        KZGCommitmentScheme<Bn256>,
        VerifierSHPLONK<'_, Bn256>,
        Challenge255<G1Affine>,
        Blake2bRead<&[u8], G1Affine, Challenge255<G1Affine>>,
        SingleStrategy<'_, Bn256>,
    >(verifier_params, vk, strategy, &[instances], &mut transcript);
    // Just FYI, because strategy is `SingleStrategy`, the output `res` is `Result<(), Error>`, so there is no need to call `res.finalize()`.

    if expect_satisfied {
        res.unwrap();
    } else {
        assert!(res.is_err());
    }
}

/// For testing only: Helper function to verify a proof (generated using [`gen_proof`]) without public instances using SHPLONK KZG multi-open polynomical commitment scheme
/// and Blake2b as the hash function for Fiat-Shamir.
pub fn check_proof(
    params: &ParamsKZG<Bn256>,
    vk: &VerifyingKey<G1Affine>,
    proof: &[u8],
    expect_satisfied: bool,
) {
    check_proof_with_instances(params, vk, proof, &[], expect_satisfied);
}

/// Helper to facilitate easier writing of tests using `RangeChip` and `RangeCircuitBuilder`.
/// By default, the [`MockProver`] is used.
///
/// Currently this tester uses all private inputs.
pub struct BaseTester {
    k: u32,
    lookup_bits: Option<usize>,
    expect_satisfied: bool,
    unusable_rows: usize,
}

impl Default for BaseTester {
    fn default() -> Self {
        Self { k: 10, lookup_bits: Some(9), expect_satisfied: true, unusable_rows: 9 }
    }
}

/// Creates a [`BaseTester`]
pub fn base_test() -> BaseTester {
    BaseTester::default()
}

impl BaseTester {
    /// Changes the number of rows in the circuit to 2<sup>k</sup>.
    /// By default it will also set lookup bits as large as possible, to `k - 1`.
    pub fn k(mut self, k: u32) -> Self {
        self.k = k;
        self.lookup_bits = Some(k as usize - 1);
        self
    }

    /// Sets the size of the lookup table used for range checks to [0, 2<sup>lookup_bits</sup>)
    pub fn lookup_bits(mut self, lookup_bits: usize) -> Self {
        assert!(lookup_bits < self.k as usize, "lookup_bits must be less than k");
        self.lookup_bits = Some(lookup_bits);
        self
    }

    /// Specify whether you expect this test to pass or fail. Default: pass
    pub fn expect_satisfied(mut self, expect_satisfied: bool) -> Self {
        self.expect_satisfied = expect_satisfied;
        self
    }

    /// Set the number of blinding (poisoned) rows
    pub fn unusable_rows(mut self, unusable_rows: usize) -> Self {
        self.unusable_rows = unusable_rows;
        self
    }

    /// Run a mock test by providing a closure that uses a `ctx` and `RangeChip`.
    /// - `expect_satisfied`: flag for whether you expect the test to pass or fail. Failure means a constraint system failure -- the tester does not catch system panics.
    pub fn run<R>(&self, f: impl FnOnce(&mut Context<Fr>, &RangeChip<Fr>) -> R) -> R {
        self.run_builder(|builder, range| f(builder.main(), range))
    }

    /// Run a mock test by providing a closure that uses a `ctx` and `GateChip`.
    /// - `expect_satisfied`: flag for whether you expect the test to pass or fail. Failure means a constraint system failure -- the tester does not catch system panics.
    pub fn run_gate<R>(&self, f: impl FnOnce(&mut Context<Fr>, &GateChip<Fr>) -> R) -> R {
        self.run(|ctx, range| f(ctx, &range.gate))
    }

    /// Run a mock test by providing a closure that uses a `builder` and `RangeChip`.
    pub fn run_builder<R>(
        &self,
        f: impl FnOnce(&mut SinglePhaseCoreManager<Fr>, &RangeChip<Fr>) -> R,
    ) -> R {
        let mut builder = RangeCircuitBuilder::default().use_k(self.k as usize);
        if let Some(lb) = self.lookup_bits {
            builder.set_lookup_bits(lb)
        }
        let range = RangeChip::new(self.lookup_bits.unwrap_or(0), builder.lookup_manager().clone());
        // run the function, mutating `builder`
        let res = f(builder.pool(0), &range);

        // helper check: if your function didn't use lookups, turn lookup table "off"
        let t_cells_lookup =
            builder.lookup_manager().iter().map(|lm| lm.total_rows()).sum::<usize>();
        let lookup_bits = if t_cells_lookup == 0 { None } else { self.lookup_bits };
        builder.config_params.lookup_bits = lookup_bits;

        // configure the circuit shape, 9 blinding rows seems enough
        builder.calculate_params(Some(self.unusable_rows));
        if self.expect_satisfied {
            MockProver::run(self.k, &builder, vec![]).unwrap().assert_satisfied();
        } else {
            assert!(MockProver::run(self.k, &builder, vec![]).unwrap().verify().is_err());
        }
        res
    }

    /// Runs keygen, real prover, and verifier by providing a closure that uses a `builder` and `RangeChip`.
    ///
    /// Must provide `init_input` for use during key generation, which is preferably not equal to `logic_input`.
    /// These are the inputs to the closure, not necessary public inputs to the circuit.
    ///
    /// Currently for testing, no public instances.
    pub fn bench_builder<I: Clone>(
        &self,
        init_input: I,
        logic_input: I,
        f: impl Fn(&mut SinglePhaseCoreManager<Fr>, &RangeChip<Fr>, I),
    ) -> BenchStats {
        let mut builder =
            RangeCircuitBuilder::from_stage(CircuitBuilderStage::Keygen).use_k(self.k as usize);
        if let Some(lb) = self.lookup_bits {
            builder.set_lookup_bits(lb)
        }
        let range = RangeChip::new(self.lookup_bits.unwrap_or(0), builder.lookup_manager().clone());
        // run the function, mutating `builder`
        f(builder.pool(0), &range, init_input);

        // helper check: if your function didn't use lookups, turn lookup table "off"
        let t_cells_lookup =
            builder.lookup_manager().iter().map(|lm| lm.total_rows()).sum::<usize>();
        let lookup_bits = if t_cells_lookup == 0 { None } else { self.lookup_bits };
        builder.config_params.lookup_bits = lookup_bits;

        // configure the circuit shape, 9 blinding rows seems enough
        let config_params = builder.calculate_params(Some(self.unusable_rows));

        let params = gen_srs(self.k);
        let vk_time = start_timer!(|| "Generating vkey");
        let vk = keygen_vk(&params, &builder).unwrap();
        end_timer!(vk_time);
        let pk_time = start_timer!(|| "Generating pkey");
        let pk = keygen_pk(&params, vk, &builder).unwrap();
        end_timer!(pk_time);

        let break_points = builder.break_points();
        drop(builder);
        // create real proof
        let proof_time = start_timer!(|| "Proving time");
        let mut builder = RangeCircuitBuilder::prover(config_params.clone(), break_points);
        let range = RangeChip::new(self.lookup_bits.unwrap_or(0), builder.lookup_manager().clone());
        f(builder.pool(0), &range, logic_input);
        let proof = gen_proof(&params, &pk, builder);
        end_timer!(proof_time);

        let proof_size = proof.len();

        let verify_time = start_timer!(|| "Verify time");
        check_proof(&params, pk.get_vk(), &proof, self.expect_satisfied);
        end_timer!(verify_time);

        BenchStats { config_params, vk_time, pk_time, proof_time, proof_size, verify_time }
    }
}

/// Bench stats
pub struct BenchStats {
    /// Config params
    pub config_params: BaseCircuitParams,
    /// Vkey gen time
    pub vk_time: TimerInfo,
    /// Pkey gen time
    pub pk_time: TimerInfo,
    /// Proving time
    pub proof_time: TimerInfo,
    /// Proof size in bytes
    pub proof_size: usize,
    /// Verify time
    pub verify_time: TimerInfo,
}
