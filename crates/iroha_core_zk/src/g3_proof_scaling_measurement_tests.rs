//! Measurement harnesses for KAGEMUSHA G3 proof-relation cost scaling.
//!
//! Every test in this module is ignored and exists only to measure how a
//! genuine Pasta IPA proof scales with Base advice width and domain size `k`.
//! Each case:
//!
//! 1. builds a real `BaseCircuitBuilder` circuit in keygen mode and generates
//!    the proving key with Halo2's memory-bounded consuming keygen path
//!    (compressed selectors);
//! 2. rebuilds the witness with a witness-only Base prover pinned to the keygen
//!    configuration and break points;
//! 3. proves through the fixed KAGEMUSHA Poseidon IPA transcript and appends
//!    the transcript-derived folded SRS generator, reproducing the canonical
//!    byte format of `kagemusha_v1_recursion::generation::augment_halo2_ipa_proof_v1`;
//! 4. checks the proof length against the canonical KAGEMUSHA ordinary-proof
//!    profile formula, verifies it natively (rejecting a wrong public input) and
//!    prints one machine-readable `G3_SCALING` line.
//!
//! `keygen_ms` covers keygen-circuit construction plus proving/verifying-key
//! generation (parameter generation is reported separately as `params_ms`).
//! `prove_ms` covers witness generation, `create_proof` and the folded-generator
//! augmentation. `verify_ms` is one native IPA verification of the raw transcript
//! plus the folded-generator equality check. `rss_mib_*` values are sampled
//! per-phase resident-set peaks (lower bounds); use `/usr/bin/time -l` for the
//! exact process peak.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    io::{self, Cursor, Read, Write},
    process::Command,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use der_parser::num_bigint::BigUint;
use halo2_base::{
    AssignedValue, Context,
    QuantumCell::{Constant, Existing, Witness},
    gates::{
        GateChip, GateInstructions, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    poseidon::hasher::PoseidonHasher,
    utils::{BigPrimeField, CurveAffineExt, ScalarField, modulus},
};
use halo2_ecc::{
    ecc::{EcPoint, EccChip},
    fields::{FieldChip, Selectable, fp::FpChip},
};
use halo2_proofs::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::{
        CurveAffine, CurveExt,
        ff::{Field as _, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
        secp256r1::{Fp as P256Base, Fq as P256Scalar, Secp256r1Affine},
    },
    plonk::{
        Circuit, ConstraintSystem, Error as PlonkError, KeygenWithExtractorError, VerifyingKey,
        create_proof, keygen_pk2_consuming_with_profile, keygen_vk_custom, verify_proof,
    },
    poly::{
        VerificationStrategy,
        commitment::{MSM as _, Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            msm::MSMIPA,
            multiopen::{ProverIPA, VerifierIPA},
            strategy::GuardIPA,
        },
    },
};
use rand_core_06::OsRng;
use snark_verifier::{
    loader::{halo2::Halo2Loader, native::NativeLoader},
    pcs::{
        AccumulationDecider,
        ipa::{Bgh19, IpaAs, IpaDecidingKey, IpaSuccinctVerifyingKey},
    },
    system::halo2::{
        Config, compile,
        transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    },
    util::{
        arithmetic::{Domain, root_of_unity},
        hash::Poseidon as LoaderPoseidon,
        transcript::Transcript as _,
    },
    verifier::{
        SnarkVerifier as _,
        plonk::{PlonkProtocol, PlonkSuccinctVerifier},
    },
};

use crate::kagemusha_p256_curve_gadget::{
    P256_LIMB_BITS, P256_NUM_LIMBS, assert_p256_ecdsa_digest,
};
use crate::kagemusha_v1_poseidon::{
    KAGEMUSHA_REPLAY_EMPTY_DOMAIN_V1, KAGEMUSHA_REPLAY_LEAF_DOMAIN_V1,
    KAGEMUSHA_REPLAY_NODE_DOMAIN_V1, KAGEMUSHA_STATE_DOMAIN_V1, KagemushaPoseidonChipV1,
    hash as kagemusha_native_poseidon_hash,
};
use crate::kagemusha_v1_recursion::{
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KagemushaPoseidonFieldV1,
};
use crate::pasta_cycle_loader::{
    DeferredEquationWitness, DeferredScalarEccChip, LIMB_BITS, LIMBS, PastaCycleEccChip,
    constrain_reciprocal_poseidon_v1,
};
use crate::pasta_native_poseidon::{PastaNativePoseidonConfigV1, PastaNativePoseidonJobsV1};

/// Rows reserved for blinding, matching the P-256 inventory and recursion circuits.
const MINIMUM_ROWS: usize = 9;
/// Lookup-table size used by the full-width P-256 inventory at `k = 16`.
const K16_LOOKUP_BITS: usize = 15;
/// Percentage of a single column left unused, so the synthetic chain fills
/// exactly the requested number of advice columns (break-point copies included).
const UNUSED_PERCENT_OF_ONE_COLUMN: usize = 2;
/// One `K16_LOOKUP_BITS` range-checked multiplicand every this many chain steps.
const LOOKUP_EVERY_STEPS: usize = 2;
const KEYGEN_SEED: u64 = 0x6733_5f6b_6579_6765;
const PROVER_SEED: u64 = 0x6733_5f70_726f_7665;
const PHASES: usize = 4;
const PHASE_PARAMS: usize = 0;
const PHASE_KEYGEN: usize = 1;
const PHASE_PROVE: usize = 2;
const PHASE_VERIFY: usize = 3;

type KagemushaTranscript<C, S> = PoseidonTranscript<
    C,
    NativeLoader,
    S,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

/// Deterministic, fast witness source; values only need to be full-width field elements.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn field<F: FromUniformBytes<64>>(&mut self) -> F {
        let mut bytes = [0_u8; 64];
        for chunk in bytes.chunks_exact_mut(8) {
            chunk.copy_from_slice(&self.next_u64().to_le_bytes());
        }
        F::from_uniform_bytes(&bytes)
    }
}

/// Counts serialized bytes without retaining them.
#[derive(Clone, Copy, Default)]
struct ByteCounter(usize);

impl Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Samples this process's resident set with `ps` and keeps one peak per phase.
struct RssSampler {
    phase: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    peaks_kib: Arc<Mutex<[u64; PHASES]>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl RssSampler {
    fn start() -> Self {
        let phase = Arc::new(AtomicUsize::new(PHASE_PARAMS));
        let stop = Arc::new(AtomicBool::new(false));
        let peaks_kib = Arc::new(Mutex::new([0_u64; PHASES]));
        let worker = {
            let phase = Arc::clone(&phase);
            let stop = Arc::clone(&stop);
            let peaks_kib = Arc::clone(&peaks_kib);
            thread::spawn(move || {
                let pid = std::process::id().to_string();
                while !stop.load(Ordering::Relaxed) {
                    let index = phase.load(Ordering::Relaxed);
                    let sampled = Command::new("/bin/ps")
                        .args(["-o", "rss=", "-p", &pid])
                        .output()
                        .ok()
                        .and_then(|output| String::from_utf8(output.stdout).ok())
                        .and_then(|text| text.trim().parse::<u64>().ok());
                    if let Some(kib) = sampled {
                        let mut peaks = peaks_kib.lock().expect("RSS peaks lock");
                        peaks[index] = peaks[index].max(kib);
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            })
        };
        Self {
            phase,
            stop,
            peaks_kib,
            worker: Some(worker),
        }
    }

    fn enter(&self, phase: usize) {
        self.phase.store(phase, Ordering::Relaxed);
    }

    fn finish(mut self) -> [u64; PHASES] {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("RSS sampler thread");
        }
        *self.peaks_kib.lock().expect("RSS peaks lock")
    }
}

/// Native verification strategy returning the IPA folded SRS generator.
///
/// This mirrors the KAGEMUSHA proof augmentation: the full IPA MSM must pass
/// before the derived generator is accepted.
struct FoldedGenerator<'params, C: CurveAffine> {
    params: &'params ParamsIPA<C>,
}

impl<'params, C: CurveAffine>
    VerificationStrategy<'params, IPACommitmentScheme<C>, VerifierIPA<'params, C>>
    for FoldedGenerator<'params, C>
{
    type Output = C;

    fn new(params: &'params ParamsIPA<C>) -> Self {
        Self { params }
    }

    fn process(
        self,
        verifier: impl FnOnce(MSMIPA<'params, C>) -> Result<GuardIPA<'params, C>, PlonkError>,
    ) -> Result<Self::Output, PlonkError> {
        let guard = verifier(MSMIPA::new(self.params))?;
        let folded_generator = guard.compute_g();
        let (check, _) = guard.use_g(folded_generator);
        if !check.check() {
            return Err(PlonkError::ConstraintSystemFailure);
        }
        Ok(folded_generator)
    }

    fn finalize(self) -> bool {
        true
    }
}

/// Fully verify a raw KAGEMUSHA-transcript proof and return its folded generator.
fn verified_folded_generator<C>(
    params: &ParamsIPA<C>,
    vk: &VerifyingKey<C>,
    raw: &[u8],
    public: &[C::ScalarExt],
) -> Result<C, PlonkError>
where
    C: CurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
{
    let columns: [&[C::ScalarExt]; 1] = [public];
    let instances: [&[&[C::ScalarExt]]; 1] = [&columns];
    let mut cursor = Cursor::new(raw);
    let mut transcript =
        KagemushaTranscript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    let folded_generator = verify_proof::<
        IPACommitmentScheme<C>,
        VerifierIPA<'_, C>,
        ChallengeScalar<C>,
        _,
        FoldedGenerator<'_, C>,
    >(
        params,
        vk,
        FoldedGenerator { params },
        &instances,
        &mut transcript,
    )?;
    drop(transcript);
    if cursor.position() != raw.len() as u64 {
        return Err(PlonkError::Transcript(io::Error::new(
            io::ErrorKind::InvalidData,
            "raw KAGEMUSHA IPA proof has trailing bytes",
        )));
    }
    Ok(folded_generator)
}

/// Canonical KAGEMUSHA ordinary-proof length (`ordinary_ipa_proof_profile_at_k_v1`).
fn canonical_ordinary_proof_bytes<C: CurveAffine>(k: u32, protocol: &PlonkProtocol<C>) -> usize {
    let witness_commitments: usize = protocol.num_witness.iter().sum();
    let mut rotations = BTreeMap::<usize, BTreeSet<i32>>::new();
    for query in &protocol.queries {
        rotations
            .entry(query.poly)
            .or_default()
            .insert(query.rotation.0);
    }
    let rotation_sets = rotations.into_values().collect::<BTreeSet<_>>().len();
    let opening_items = 2 * k as usize + 5;
    32 * (witness_commitments
        + protocol.quotient.num_chunk()
        + protocol.evaluations.len()
        + rotation_sets
        + opening_items)
}

/// CPU time of the whole process (all threads) in milliseconds.
///
/// Unlike wall time, this is insensitive to other processes competing for cores;
/// for a one-thread run it approximates the uncontended single-core wall time.
fn process_cpu_ms() -> f64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::ProcessCPUTime);
    now.tv_sec as f64 * 1000.0 + now.tv_nsec as f64 / 1_000_000.0
}

/// CPU time of the calling thread in milliseconds.
fn thread_cpu_ms() -> f64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::ThreadCPUTime);
    now.tv_sec as f64 * 1000.0 + now.tv_nsec as f64 / 1_000_000.0
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn mib(kib: u64) -> u64 {
    kib / 1024
}

/// Prove and verify one Base circuit in the real KAGEMUSHA IPA proof format.
fn measure<C>(
    case: &str,
    k: u32,
    lookup_bits: Option<usize>,
    build: impl Fn(&mut BaseCircuitBuilder<C::ScalarExt>, u64) -> Vec<C::ScalarExt>,
) where
    C: SerdeCurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    drop(measure_impl::<C>(case, k, lookup_bits, None, build));
}

/// One measured proof retained for recursive and native-verifier measurements.
struct MeasuredProof<C: CurveAffine> {
    params: ParamsIPA<C>,
    vk: VerifyingKey<C>,
    /// Augmented KAGEMUSHA proof: raw transcript plus the 32-byte folded generator.
    proof: Vec<u8>,
    public: Vec<C::ScalarExt>,
}

/// One measured circuit: a Base builder plus any extra synthesized regions.
///
/// [`measure_circuit`] calculates Base parameters, statistics and keygen break points through
/// this view, so plain Base circuits and Base-plus-native-lane circuits share one procedure.
trait G3MeasuredCircuit<F: ScalarField>: Circuit<F> {
    /// The Base builder holding the virtual graph, break points and public instances.
    fn base(&self) -> &BaseCircuitBuilder<F>;
    /// Mutable Base builder, used to calculate parameters before key generation.
    fn base_mut(&mut self) -> &mut BaseCircuitBuilder<F>;
    /// Extra ` key=value` fields appended to the `G3_SCALING` line.
    fn extra_fields(&self) -> String {
        String::new()
    }
}

impl<F: ScalarField> G3MeasuredCircuit<F> for BaseCircuitBuilder<F> {
    fn base(&self) -> &BaseCircuitBuilder<F> {
        self
    }

    fn base_mut(&mut self) -> &mut BaseCircuitBuilder<F> {
        self
    }
}

/// [`measure`] with optional parameter reuse, returning the verified proof.
///
/// Reused parameters are only a wall-time saving; `params_ms` then reports zero.
fn measure_impl<C>(
    case: &str,
    k: u32,
    lookup_bits: Option<usize>,
    reuse_params: Option<ParamsIPA<C>>,
    build: impl Fn(&mut BaseCircuitBuilder<C::ScalarExt>, u64) -> Vec<C::ScalarExt>,
) -> MeasuredProof<C>
where
    C: SerdeCurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    measure_circuit::<C, BaseCircuitBuilder<C::ScalarExt>>(
        case,
        k,
        lookup_bits,
        reuse_params,
        |mut builder, seed| {
            let public = build(&mut builder, seed);
            (builder, public)
        },
    )
}

/// [`measure_impl`] for any [`G3MeasuredCircuit`].
///
/// `make` receives a configured Base builder (keygen mode, then witness-only prover mode
/// pinned to the keygen parameters and break points), builds the relation into it and
/// returns the complete circuit with its public values.
fn measure_circuit<C, Circ>(
    case: &str,
    k: u32,
    lookup_bits: Option<usize>,
    reuse_params: Option<ParamsIPA<C>>,
    make: impl Fn(BaseCircuitBuilder<C::ScalarExt>, u64) -> (Circ, Vec<C::ScalarExt>),
) -> MeasuredProof<C>
where
    C: SerdeCurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
    Circ: G3MeasuredCircuit<C::ScalarExt>,
{
    let field = if <C::ScalarExt as KagemushaPoseidonFieldV1>::IS_EQ_PARITY {
        "eq"
    } else {
        "ep"
    };
    let threads = std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".to_owned());
    let rss = RssSampler::start();

    rss.enter(PHASE_PARAMS);
    let started = Instant::now();
    let params = match reuse_params {
        Some(params) => {
            assert_eq!(params.k(), k, "{case}: reused parameters have the wrong k");
            params
        }
        None => ParamsIPA::<C>::new(k),
    };
    let params_ms = elapsed_ms(started);

    rss.enter(PHASE_KEYGEN);
    let started = Instant::now();
    let keygen_cpu_started = process_cpu_ms();
    let mut keygen_base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(k as usize)
        .use_instance_columns(1);
    if let Some(bits) = lookup_bits {
        keygen_base.set_lookup_bits(bits);
    }
    let (mut keygen_circuit, keygen_public) = make(keygen_base, KEYGEN_SEED);
    let instance_count = keygen_public.len();
    let circuit_params = keygen_circuit
        .base_mut()
        .calculate_params(Some(MINIMUM_ROWS));
    let statistics = keygen_circuit.base().statistics();
    let extra_fields = keygen_circuit.extra_fields();
    let cells = statistics.gate.total_advice_per_phase[0];
    let lookup_cells = statistics.total_lookup_advice_per_phase[0];
    let keygen_build_ms = elapsed_ms(started);
    let (pk, break_points) = match keygen_pk2_consuming_with_profile(
        &params,
        keygen_circuit,
        true,
        |circuit: &Circ, _profile| Ok::<_, Infallible>(circuit.base().break_points()),
    ) {
        Ok(generated) => generated,
        Err(KeygenWithExtractorError::Keygen(error)) => panic!("{case}: keygen failed: {error}"),
        Err(KeygenWithExtractorError::Extractor(never)) => match never {},
    };
    halo2_proofs::release_allocator_slack();
    let keygen_ms = elapsed_ms(started);
    let keygen_cpu_ms = process_cpu_ms() - keygen_cpu_started;

    rss.enter(PHASE_PROVE);
    let started = Instant::now();
    let prove_cpu_started = process_cpu_ms();
    let (prover, public) = make(
        BaseCircuitBuilder::<C::ScalarExt>::prover(circuit_params.clone(), break_points),
        PROVER_SEED,
    );
    assert_eq!(public.len(), instance_count, "{case}: instance shape");
    let witness_ms = elapsed_ms(started);
    let started = Instant::now();
    let raw = {
        let columns: [&[C::ScalarExt]; 1] = [&public];
        let instances: [&[&[C::ScalarExt]]; 1] = [&columns];
        let mut transcript = KagemushaTranscript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(
            Vec::<u8>::new(),
        );
        create_proof::<IPACommitmentScheme<C>, ProverIPA<'_, C>, ChallengeScalar<C>, _, _, _>(
            &params,
            &pk,
            &[prover],
            &instances,
            OsRng,
            &mut transcript,
        )
        .unwrap_or_else(|error| panic!("{case}: create_proof failed: {error}"));
        transcript.finalize()
    };
    halo2_proofs::release_allocator_slack();
    let create_proof_ms = elapsed_ms(started);
    let started = Instant::now();
    let folded_generator = verified_folded_generator(&params, pk.get_vk(), &raw, &public)
        .unwrap_or_else(|error| panic!("{case}: folded-generator derivation failed: {error}"));
    let mut proof = raw.clone();
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    let augment_ms = elapsed_ms(started);
    let prove_ms = witness_ms + create_proof_ms + augment_ms;
    let prove_cpu_ms = process_cpu_ms() - prove_cpu_started;

    rss.enter(PHASE_VERIFY);
    let started = Instant::now();
    let verify_cpu_started = process_cpu_ms();
    let (raw_part, generator_part) = proof.split_at(proof.len() - 32);
    let verified = verified_folded_generator(&params, pk.get_vk(), raw_part, &public)
        .unwrap_or_else(|error| panic!("{case}: verification failed: {error}"));
    let verified_ok = verified.to_bytes().as_ref() == generator_part;
    let verify_ms = elapsed_ms(started);
    let verify_cpu_ms = process_cpu_ms() - verify_cpu_started;
    let mut wrong_public = public.clone();
    wrong_public[0] += C::ScalarExt::ONE;
    let wrong_public_rejected =
        verified_folded_generator(&params, pk.get_vk(), raw_part, &wrong_public).is_err();
    let peaks = rss.finish();

    let vk = pk.get_vk();
    let cs = vk.cs();
    let mut counter = ByteCounter::default();
    pk.write(&mut counter, SerdeFormat::Processed)
        .expect("count proving-key bytes");
    let pk_bytes = counter.0;
    let mut counter = ByteCounter::default();
    vk.write(&mut counter, SerdeFormat::Processed)
        .expect("count verifying-key bytes");
    let vk_bytes = counter.0;
    let protocol = compile(
        &params,
        vk,
        Config::ipa().with_num_instance(vec![public.len()]),
    );
    let expected_proof_bytes = canonical_ordinary_proof_bytes(k, &protocol);
    let advice_cols = circuit_params.num_advice_per_phase[0];
    let lookup_cols = cs.num_advice_columns() - advice_cols;
    println!(
        "G3_SCALING k={k} field={field} advice_cols={advice_cols} lookup_cols={lookup_cols} cells={cells} keygen_ms={keygen_ms:.0} pk_bytes={pk_bytes} vk_bytes={vk_bytes} prove_ms={prove_ms:.0} verify_ms={verify_ms:.1} proof_bytes={} case={case} threads={threads} lookup_cells={lookup_cells} lookup_args={} fixed_cols={} perm_cols={} degree={} quotient_chunks={} instances={} raw_proof_bytes={} expected_proof_bytes={expected_proof_bytes} params_ms={params_ms:.0} keygen_build_ms={keygen_build_ms:.0} witness_ms={witness_ms:.0} create_proof_ms={create_proof_ms:.0} augment_ms={augment_ms:.0} rss_mib_params={} rss_mib_keygen={} rss_mib_prove={} rss_mib_verify={} keygen_cpu_ms={keygen_cpu_ms:.0} prove_cpu_ms={prove_cpu_ms:.0} verify_cpu_ms={verify_cpu_ms:.1} total_advice_cols={}{extra_fields}",
        proof.len(),
        cs.lookups().len(),
        cs.num_fixed_columns(),
        cs.permutation().get_columns().len(),
        cs.degree(),
        protocol.quotient.num_chunk(),
        public.len(),
        raw.len(),
        mib(peaks[PHASE_PARAMS]),
        mib(peaks[PHASE_KEYGEN]),
        mib(peaks[PHASE_PROVE]),
        mib(peaks[PHASE_VERIFY]),
        cs.num_advice_columns(),
    );
    assert!(verified_ok, "{case}: appended folded generator mismatch");
    assert!(wrong_public_rejected, "{case}: wrong public input accepted");
    assert_eq!(
        proof.len(),
        expected_proof_bytes,
        "{case}: proof is not the canonical KAGEMUSHA ordinary length"
    );
    let vk = vk.clone();
    drop(pk);
    halo2_proofs::release_allocator_slack();
    MeasuredProof {
        params,
        vk,
        proof,
        public,
    }
}

/// Chained `acc + x * y = acc'` vertical gates filling `width` Base columns.
///
/// Every gate is a real constraint over private full-width witnesses. With
/// `lookups`, every `LOOKUP_EVERY_STEPS`-th multiplicand is a `K16_LOOKUP_BITS`
/// range-checked limb, giving roughly one lookup column per six gate columns.
fn chained_mul_add<F: BigPrimeField>(
    builder: &mut BaseCircuitBuilder<F>,
    k: u32,
    width: usize,
    lookups: bool,
    seed: u64,
) -> Vec<F> {
    let usable_rows = (1_usize << k) - MINIMUM_ROWS;
    let unused_rows = usable_rows * UNUSED_PERCENT_OF_ONE_COLUMN / 100;
    let steps = (width * usable_rows - unused_rows - 1) / 3;
    let lookup_mask = (1_u64 << K16_LOOKUP_BITS) - 1;
    let range = lookups.then(|| builder.range_chip());
    let mut rng = SplitMix64(seed);
    let ctx = builder.main(0);
    let mut acc = F::from(7_u64);
    let _constant = ctx.load_constant(acc);
    for step in 0..steps {
        let checked = range.as_ref().filter(|_| step % LOOKUP_EVERY_STEPS == 0);
        let x = if checked.is_some() {
            F::from(rng.next_u64() & lookup_mask)
        } else {
            rng.field()
        };
        let y: F = rng.field();
        acc += x * y;
        ctx.assign_region([Witness(x), Witness(y), Witness(acc)], [-1]);
        if let Some(range) = checked {
            let x_cell = ctx.get(-3);
            range.range_check(ctx, x_cell, K16_LOOKUP_BITS);
        }
    }
    let output = ctx.get(-1);
    builder.assigned_instances[0].push(output);
    vec![acc]
}

/// One full-width low-S P-256 ECDSA verification, as in the gadget inventory test.
///
/// `r = s = z = x(2G)` with public key `Q = G` gives `u1 = u2 = 1`, so
/// `R = 2G` and `x(R) = r`: a valid low-S signature over the 32-byte digest.
fn p256_ecdsa_low_s<F: BigPrimeField>(builder: &mut BaseCircuitBuilder<F>, _seed: u64) -> Vec<F> {
    let range = builder.range_chip();
    let base_chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let scalar_chip = FpChip::<F, P256Scalar>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let ctx = builder.main(0);
    let generator = Secp256r1Affine::generator();
    let two = (generator.to_curve() + generator.to_curve()).to_affine();
    let (two_x, _) = two.into_coordinates();
    let scalar = P256Scalar::from_repr(two_x.to_repr())
        .expect("2G x-coordinate is below the P-256 scalar order");
    let r = scalar_chip.load_private(ctx, scalar);
    let s = scalar_chip.load_private(ctx, scalar);
    let z = scalar_chip.load_private(ctx, scalar);
    let mut digest_be = scalar.to_repr();
    digest_be.reverse();
    let digest: [AssignedValue<F>; 32] =
        std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(digest_be[i]))));
    let (gx, gy) = generator.into_coordinates();
    let key = EcPoint::new(
        base_chip.load_private(ctx, gx),
        base_chip.load_private(ctx, gy),
    );
    let mut sec1 = [0_u8; 65];
    sec1[0] = 4;
    let mut x_be = gx.to_repr();
    let mut y_be = gy.to_repr();
    x_be.reverse();
    y_be.reverse();
    sec1[1..33].copy_from_slice(&x_be);
    sec1[33..65].copy_from_slice(&y_be);
    let enrolled_sec1: [AssignedValue<F>; 65] =
        std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(sec1[i]))));
    let digest_quotient = ctx.load_witness(F::ZERO);
    assert_p256_ecdsa_digest::<F, 256, true>(
        &base_chip,
        ctx,
        &key,
        &key,
        &enrolled_sec1,
        &r,
        &s,
        &z,
        &digest,
        digest_quotient,
    );
    let public = digest[0];
    builder.assigned_instances[0].push(public);
    vec![*public.value()]
}

fn measure_synthetic_k16<C>(width: usize)
where
    C: SerdeCurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    measure::<C>(
        &format!("synthetic_w{width}"),
        16,
        Some(K16_LOOKUP_BITS),
        |builder, seed| chained_mul_add(builder, 16, width, true, seed),
    );
}

fn measure_narrow_eq(k: u32, width: usize) {
    measure::<EqAffine>(&format!("narrow_w{width}"), k, None, |builder, seed| {
        chained_mul_add(builder, k, width, false, seed)
    });
}

fn measure_p256<C>()
where
    C: SerdeCurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    measure::<C>(
        "p256_ecdsa_low_s",
        16,
        Some(K16_LOOKUP_BITS),
        p256_ecdsa_low_s,
    );
}

macro_rules! synthetic_k16 {
    ($($name:ident: $curve:ty, $width:expr;)+) => {$(
        /// Measurement harness: synthetic `k = 16` Base circuit filling the
        /// stated advice width with chained private `mul_add` gates and range
        /// lookups; proves, verifies and prints one `G3_SCALING` line.
        #[test]
        #[ignore = "G3 proof-scaling measurement harness; run explicitly in release"]
        fn $name() {
            measure_synthetic_k16::<$curve>($width);
        }
    )+};
}

synthetic_k16! {
    k16_eq_w001: EqAffine, 1;
    k16_ep_w001: EpAffine, 1;
    k16_eq_w008: EqAffine, 8;
    k16_ep_w008: EpAffine, 8;
    k16_eq_w023: EqAffine, 23;
    k16_ep_w023: EpAffine, 23;
    k16_eq_w032: EqAffine, 32;
    k16_ep_w032: EpAffine, 32;
    k16_eq_w064: EqAffine, 64;
    k16_ep_w064: EpAffine, 64;
    k16_eq_w128: EqAffine, 128;
    k16_ep_w128: EpAffine, 128;
}

/// Measurement harness: one full-width low-S P-256 ECDSA verification proved
/// at `k = 16` on the Eq (Fp) side; prints one `G3_SCALING` line.
#[test]
#[ignore = "G3 proof-scaling measurement harness; run explicitly in release"]
fn k16_eq_p256_ecdsa() {
    measure_p256::<EqAffine>();
}

/// Measurement harness: one full-width low-S P-256 ECDSA verification proved
/// at `k = 16` on the Ep (Fq) side; prints one `G3_SCALING` line.
#[test]
#[ignore = "G3 proof-scaling measurement harness; run explicitly in release"]
fn k16_ep_p256_ecdsa() {
    measure_p256::<EpAffine>();
}

macro_rules! narrow_eq {
    ($($name:ident: $k:expr, $width:expr;)+) => {$(
        /// Measurement harness: narrow outer-layer candidate on the Eq side
        /// without lookups; proves, verifies and prints one `G3_SCALING` line.
        #[test]
        #[ignore = "G3 proof-scaling measurement harness; run explicitly in release"]
        fn $name() {
            measure_narrow_eq($k, $width);
        }
    )+};
}

narrow_eq! {
    k18_eq_w1_no_lookup: 18, 1;
    k18_eq_w2_no_lookup: 18, 2;
    k18_eq_w3_no_lookup: 18, 3;
    k19_eq_w1_no_lookup: 19, 1;
    k19_eq_w2_no_lookup: 19, 2;
    k19_eq_w3_no_lookup: 19, 3;
    k20_eq_w1_no_lookup: 20, 1;
    k20_eq_w2_no_lookup: 20, 2;
    k20_eq_w3_no_lookup: 20, 3;
}

// ---------------------------------------------------------------------------
// M6: fast-path building blocks.
//
// (1) a Poseidon-only transition-like circuit (no non-native curve arithmetic),
// (2) the same circuit plus one native-field Schnorr verification,
// (3) one in-circuit succinct verification of a k = 16 KAGEMUSHA IPA proof
//     (scalar half plus reciprocal curve half), and
// (4) native verifier costs (P-256 ECDSA, Pasta Schnorr, Pasta IPA).
// Every case below is ignored; all `G3_SCALING` lines come from `measure_impl`.
// ---------------------------------------------------------------------------

/// Field elements in the stand-in wallet state opening (flattened spec section 3 state).
const M6_STATE_FIELDS: usize = 40;
/// Consumed-credit sparse-Merkle depth (`credit_id` is a 256-bit digest).
const M6_CONSUMED_CREDIT_DEPTH: usize = 256;
/// Ordinal-keyed map depth (pending outgoing, load, redeem and fee-claim maps).
const M6_ORDINAL_MAP_DEPTH: usize = 128;
const M6_PENDING_EMPTY_DOMAIN: u64 = u64::from_le_bytes(*b"m6pndemp");
const M6_PENDING_LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"m6pndlf1");
const M6_PENDING_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"m6pndnd1");
const M6_SCHNORR_DOMAIN: u64 = u64::from_le_bytes(*b"m6schnr1");
/// Window of both in-circuit scalar multiplications (`halo2_ecc` default).
const M6_EC_WINDOW_BITS: usize = 4;
/// Repetitions of each native verifier timing (median reported).
const M6_NATIVE_REPEATS: usize = 7;
/// Iterations of each native signature timing loop.
const M6_SIGNATURE_ITERATIONS: usize = 2_000;

/// One native-field cell used as a `halo2_ecc` field element.
#[derive(Clone, Copy, Debug)]
struct M6NativePoint<F: BigPrimeField>(AssignedValue<F>);

impl<F: BigPrimeField> From<&M6NativePoint<F>> for M6NativePoint<F> {
    fn from(value: &M6NativePoint<F>) -> Self {
        *value
    }
}

/// Native-field `FieldChip`; a test-only copy of `pasta_cycle_loader`'s private adapter.
///
/// Every operation is one ordinary gate operation modulo the circuit field, so
/// arithmetic on the Pasta curve whose base field is the circuit field needs no
/// limb emulation.
#[derive(Clone, Debug)]
struct M6NativeFieldChip<'range, F: BigPrimeField> {
    range: &'range RangeChip<F>,
    native_modulus: BigUint,
}

impl<'range, F: BigPrimeField> M6NativeFieldChip<'range, F> {
    fn new(range: &'range RangeChip<F>) -> Self {
        Self {
            range,
            native_modulus: modulus::<F>(),
        }
    }

    fn signed_constant(value: i64) -> F {
        let magnitude = F::from(value.unsigned_abs());
        if value.is_negative() {
            -magnitude
        } else {
            magnitude
        }
    }
}

impl<F: BigPrimeField> FieldChip<F> for M6NativeFieldChip<'_, F> {
    const PRIME_FIELD_NUM_BITS: u32 = F::NUM_BITS;
    type UnsafeFieldPoint = M6NativePoint<F>;
    type FieldPoint = M6NativePoint<F>;
    type ReducedFieldPoint = M6NativePoint<F>;
    type FieldType = F;
    type RangeChip = RangeChip<F>;

    fn native_modulus(&self) -> &BigUint {
        &self.native_modulus
    }

    fn range(&self) -> &Self::RangeChip {
        self.range
    }

    fn limb_bits(&self) -> usize {
        F::NUM_BITS as usize
    }

    fn get_assigned_value(&self, value: &Self::UnsafeFieldPoint) -> Self::FieldType {
        *value.0.value()
    }

    fn load_private(&self, ctx: &mut Context<F>, value: Self::FieldType) -> Self::FieldPoint {
        M6NativePoint(ctx.load_witness(value))
    }

    fn load_constant(&self, ctx: &mut Context<F>, value: Self::FieldType) -> Self::FieldPoint {
        M6NativePoint(ctx.load_constant(value))
    }

    fn add_no_carry(
        &self,
        ctx: &mut Context<F>,
        lhs: impl Into<Self::UnsafeFieldPoint>,
        rhs: impl Into<Self::UnsafeFieldPoint>,
    ) -> Self::UnsafeFieldPoint {
        let lhs = lhs.into();
        let rhs = rhs.into();
        M6NativePoint(self.gate().add(ctx, Existing(lhs.0), Existing(rhs.0)))
    }

    fn add_constant_no_carry(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::UnsafeFieldPoint>,
        constant: Self::FieldType,
    ) -> Self::UnsafeFieldPoint {
        let value = value.into();
        M6NativePoint(self.gate().add(ctx, Existing(value.0), Constant(constant)))
    }

    fn sub_no_carry(
        &self,
        ctx: &mut Context<F>,
        lhs: impl Into<Self::UnsafeFieldPoint>,
        rhs: impl Into<Self::UnsafeFieldPoint>,
    ) -> Self::UnsafeFieldPoint {
        let lhs = lhs.into();
        let rhs = rhs.into();
        M6NativePoint(<GateChip<F> as GateInstructions<F>>::sub(
            self.gate(),
            ctx,
            Existing(lhs.0),
            Existing(rhs.0),
        ))
    }

    fn negate(&self, ctx: &mut Context<F>, value: Self::FieldPoint) -> Self::FieldPoint {
        M6NativePoint(<GateChip<F> as GateInstructions<F>>::neg(
            self.gate(),
            ctx,
            Existing(value.0),
        ))
    }

    fn scalar_mul_no_carry(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::UnsafeFieldPoint>,
        constant: i64,
    ) -> Self::UnsafeFieldPoint {
        let value = value.into();
        M6NativePoint(self.gate().mul(
            ctx,
            Existing(value.0),
            Constant(Self::signed_constant(constant)),
        ))
    }

    fn scalar_mul_and_add_no_carry(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::UnsafeFieldPoint>,
        addend: impl Into<Self::UnsafeFieldPoint>,
        constant: i64,
    ) -> Self::UnsafeFieldPoint {
        let value = value.into();
        let addend = addend.into();
        M6NativePoint(self.gate().mul_add(
            ctx,
            Existing(value.0),
            Constant(Self::signed_constant(constant)),
            Existing(addend.0),
        ))
    }

    fn mul_no_carry(
        &self,
        ctx: &mut Context<F>,
        lhs: impl Into<Self::UnsafeFieldPoint>,
        rhs: impl Into<Self::UnsafeFieldPoint>,
    ) -> Self::UnsafeFieldPoint {
        let lhs = lhs.into();
        let rhs = rhs.into();
        M6NativePoint(self.gate().mul(ctx, Existing(lhs.0), Existing(rhs.0)))
    }

    fn check_carry_mod_to_zero(&self, ctx: &mut Context<F>, value: Self::UnsafeFieldPoint) {
        self.gate().assert_is_const(ctx, &value.0, &F::ZERO);
    }

    fn carry_mod(&self, _ctx: &mut Context<F>, value: Self::UnsafeFieldPoint) -> Self::FieldPoint {
        value
    }

    fn range_check(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::FieldPoint>,
        max_bits: usize,
    ) {
        assert!(
            max_bits <= F::NUM_BITS as usize,
            "native range check exceeds the field width"
        );
        if max_bits < F::NUM_BITS as usize {
            self.range.range_check(ctx, value.into().0, max_bits);
        }
    }

    fn enforce_less_than(
        &self,
        _ctx: &mut Context<F>,
        value: Self::FieldPoint,
    ) -> Self::ReducedFieldPoint {
        // An assigned native-field cell is already a unique element modulo F.
        value
    }

    fn is_soft_zero(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::FieldPoint>,
    ) -> AssignedValue<F> {
        self.gate().is_zero(ctx, value.into().0)
    }

    fn is_soft_nonzero(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::FieldPoint>,
    ) -> AssignedValue<F> {
        let is_zero = self.gate().is_zero(ctx, value.into().0);
        self.gate().not(ctx, is_zero)
    }

    fn is_zero(
        &self,
        ctx: &mut Context<F>,
        value: impl Into<Self::FieldPoint>,
    ) -> AssignedValue<F> {
        self.gate().is_zero(ctx, value.into().0)
    }

    fn is_equal_unenforced(
        &self,
        ctx: &mut Context<F>,
        lhs: Self::ReducedFieldPoint,
        rhs: Self::ReducedFieldPoint,
    ) -> AssignedValue<F> {
        self.gate().is_equal(ctx, Existing(lhs.0), Existing(rhs.0))
    }

    fn assert_equal(
        &self,
        ctx: &mut Context<F>,
        lhs: impl Into<Self::FieldPoint>,
        rhs: impl Into<Self::FieldPoint>,
    ) {
        ctx.constrain_equal(&lhs.into().0, &rhs.into().0);
    }
}

impl<F: BigPrimeField> Selectable<F, M6NativePoint<F>> for M6NativeFieldChip<'_, F> {
    fn select(
        &self,
        ctx: &mut Context<F>,
        when_true: M6NativePoint<F>,
        when_false: M6NativePoint<F>,
        selector: AssignedValue<F>,
    ) -> M6NativePoint<F> {
        M6NativePoint(<GateChip<F> as GateInstructions<F>>::select(
            self.gate(),
            ctx,
            when_true.0,
            when_false.0,
            selector,
        ))
    }

    fn select_by_indicator(
        &self,
        ctx: &mut Context<F>,
        values: &impl AsRef<[M6NativePoint<F>]>,
        coefficients: &[AssignedValue<F>],
    ) -> M6NativePoint<F> {
        let values = values.as_ref();
        assert_eq!(
            values.len(),
            coefficients.len(),
            "native indicator shape mismatch"
        );
        M6NativePoint(self.gate().inner_product(
            ctx,
            values.iter().map(|value| Existing(value.0)),
            coefficients.iter().copied().map(Existing),
        ))
    }
}

/// The Pasta curve whose base field is this circuit field (native curve arithmetic).
///
/// In the Eq/Fp circuit that is Pallas (`EpAffine`); in the Ep/Fq circuit, Vesta (`EqAffine`).
trait M6NativeCurve: KagemushaPoseidonFieldV1 + FromUniformBytes<64> {
    /// Scalar field of the native curve (the other Pasta field).
    type Scalar: PrimeField + FromUniformBytes<64>;
    /// Native curve: its base field is the circuit field.
    type Curve: CurveAffineExt<Base = Self, ScalarExt = Self::Scalar>;
}

impl M6NativeCurve for Fp {
    type Scalar = Fq;
    type Curve = EpAffine;
}

impl M6NativeCurve for Fq {
    type Scalar = Fp;
    type Curve = EqAffine;
}

/// Tree-node hash of the transition stand-in.
#[derive(Clone, Copy, Debug)]
enum M6Node {
    /// The repository's current node, `KagemushaPoseidonChipV1::hash(domain, [l, r])`:
    /// absorbs domain, arity, left and right, i.e. three Pow5 permutations.
    RepoSponge,
    /// Cost-equivalent stand-in for a one-permutation 2-to-1 compression.
    ///
    /// NOT a sound hash: it folds `left + domain * right` and permutes once. Its
    /// constraint count equals a width-3 compression `[domain, left, right] -> s[1]`
    /// (one permutation plus two additions), which is what this variant measures.
    OnePermutationStandIn,
}

impl M6Node {
    fn label(self) -> &'static str {
        match self {
            Self::RepoSponge => "repo_sponge",
            Self::OnePermutationStandIn => "one_perm_standin",
        }
    }
}

/// Node hasher shared by both sparse-Merkle maps.
struct M6Hasher<'chip, F: KagemushaPoseidonFieldV1> {
    sponge: &'chip KagemushaPoseidonChipV1<F>,
    single:
        Option<PoseidonHasher<F, KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KAGEMUSHA_IPA_POSEIDON_RATE_V1>>,
}

impl<'chip, F: KagemushaPoseidonFieldV1> M6Hasher<'chip, F> {
    fn new(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        sponge: &'chip KagemushaPoseidonChipV1<F>,
        node: M6Node,
    ) -> Self {
        let single = matches!(node, M6Node::OnePermutationStandIn).then(|| {
            let mut hasher = PoseidonHasher::new(F::kagemusha_poseidon_spec_v1().clone());
            hasher.initialize_consts(ctx, range.gate());
            hasher
        });
        Self { sponge, single }
    }

    fn node(
        &self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        domain: u64,
        left: AssignedValue<F>,
        right: AssignedValue<F>,
    ) -> AssignedValue<F> {
        match &self.single {
            None => self.sponge.hash(ctx, range, domain, &[left, right]),
            Some(single) => {
                let folded = range
                    .gate()
                    .mul_add(ctx, right, Constant(F::from(domain)), left);
                single.hash_fix_len_array(ctx, range.gate(), &[folded])
            }
        }
    }
}

fn m6_u128(rng: &mut SplitMix64) -> u128 {
    (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64())
}

/// Little-endian `(low, high)` 128-bit halves of a 32-byte canonical encoding.
fn m6_u128_halves(bytes: &[u8]) -> (u128, u128) {
    let low = u128::from_le_bytes(bytes[..16].try_into().expect("16-byte low half"));
    let high = u128::from_le_bytes(bytes[16..32].try_into().expect("16-byte high half"));
    (low, high)
}

/// Reduce a 32-byte little-endian integer into another prime field.
fn m6_reduce<S: FromUniformBytes<64>>(bytes: &[u8]) -> S {
    let mut wide = [0_u8; 64];
    wide[..32].copy_from_slice(&bytes[..32]);
    S::from_uniform_bytes(&wide)
}

fn m6_two_pow_128<F: BigPrimeField>() -> F {
    F::from_u128(u128::MAX) + F::ONE
}

/// One range-checked `u128` witness.
fn m6_u128_cell<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u128,
) -> AssignedValue<F> {
    let cell = ctx.load_witness(F::from_u128(value));
    range.range_check(ctx, cell, 128);
    cell
}

/// One 256-bit digest as two range-checked 128-bit limbs.
fn m6_digest<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    rng: &mut SplitMix64,
) -> [AssignedValue<F>; 2] {
    [m6_u128(rng), m6_u128(rng)].map(|limb| m6_u128_cell(ctx, range, limb))
}

/// Assert that the 128-bit limb pair `(low, high)` is at most `max = (max_low, max_high)`.
///
/// Callers' bit decompositions bound each limb below `2^128`.
fn m6_assert_limbs_at_most<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    low: AssignedValue<F>,
    high: AssignedValue<F>,
    (max_low, max_high): (u128, u128),
) {
    let gate = range.gate();
    let high_below = range.is_less_than(ctx, high, Constant(F::from_u128(max_high)), 128);
    let high_equal = gate.is_equal(ctx, high, Constant(F::from_u128(max_high)));
    let low_at_most = range.is_less_than(ctx, low, Constant(F::from_u128(max_low) + F::ONE), 129);
    let tail = gate.and(ctx, high_equal, low_at_most);
    let canonical = gate.or(ctx, high_below, tail);
    gate.assert_is_const(ctx, &canonical, &F::ONE);
}

/// Split a field cell into its canonical 128-bit limbs (no `value + p` alias).
fn m6_canonical_split<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
) -> [AssignedValue<F>; 2] {
    let (low_value, high_value) = m6_u128_halves(value.value().to_repr().as_ref());
    let low = ctx.load_witness(F::from_u128(low_value));
    let high = ctx.load_witness(F::from_u128(high_value));
    let recomposed = range
        .gate()
        .mul_add(ctx, high, Constant(m6_two_pow_128::<F>()), low);
    ctx.constrain_equal(&recomposed, &value);
    let maximum = m6_u128_halves((-F::ONE).to_repr().as_ref());
    m6_assert_limbs_at_most(ctx, range, low, high, maximum);
    [low, high]
}

/// In-circuit Schnorr verification over the native Pasta curve.
///
/// Checks `[s]G == R + [e]PK` with `e = Poseidon(domain, R, PK, message)` computed by
/// the repository Poseidon chip. `s` and `e` are canonical two-limb scalars; `PK`
/// and `R` are on-curve checked. The fixed-base and variable-base multiplications
/// use `halo2_ecc` with 4-bit windows over the native field chip.
fn m6_schnorr_verify<F: M6NativeCurve>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    sponge: &KagemushaPoseidonChipV1<F>,
    message: &[AssignedValue<F>],
    rng: &mut SplitMix64,
) {
    let native = M6NativeFieldChip::new(range);
    let ecc = EccChip::new(&native);
    let generator = F::Curve::generator();
    let secret: F::Scalar = rng.field();
    let nonce: F::Scalar = rng.field();
    let key = (generator * secret).to_affine();
    let commitment = (generator * nonce).to_affine();
    let (key_x, key_y) = key.into_coordinates();
    let (commitment_x, commitment_y) = commitment.into_coordinates();
    let mut preimage = vec![commitment_x, commitment_y, key_x, key_y];
    preimage.extend(message.iter().map(|cell| *cell.value()));
    let challenge = kagemusha_native_poseidon_hash::<F>(M6_SCHNORR_DOMAIN, &preimage);
    let response = nonce + m6_reduce::<F::Scalar>(challenge.to_repr().as_ref()) * secret;

    let key_point = ecc.load_private::<F::Curve>(ctx, (key_x, key_y));
    let commitment_point = ecc.load_private::<F::Curve>(ctx, (commitment_x, commitment_y));
    let mut inputs = vec![
        commitment_point.x().0,
        commitment_point.y().0,
        key_point.x().0,
        key_point.y().0,
    ];
    inputs.extend_from_slice(message);
    let challenge_cell = sponge.hash(ctx, range, M6_SCHNORR_DOMAIN, &inputs);
    let challenge_limbs = m6_canonical_split(ctx, range, challenge_cell);
    let (response_low, response_high) = m6_u128_halves(response.to_repr().as_ref());
    let response_limbs = [
        ctx.load_witness(F::from_u128(response_low)),
        ctx.load_witness(F::from_u128(response_high)),
    ];
    let scalar_maximum = m6_u128_halves((-F::Scalar::ONE).to_repr().as_ref());
    m6_assert_limbs_at_most(
        ctx,
        range,
        response_limbs[0],
        response_limbs[1],
        scalar_maximum,
    );
    let lhs = ecc.fixed_base_scalar_mult::<F::Curve>(
        ctx,
        &generator,
        response_limbs.to_vec(),
        128,
        M6_EC_WINDOW_BITS,
    );
    let scaled_key = ecc.scalar_mult::<F::Curve>(
        ctx,
        key_point,
        challenge_limbs.to_vec(),
        128,
        M6_EC_WINDOW_BITS,
    );
    let rhs = ecc.add_unequal(ctx, &commitment_point, &scaled_key, true);
    ecc.assert_equal(ctx, lhs, rhs);
}

/// Insert one leaf into a sparse-Merkle map whose old leaf is known.
///
/// Both paths share the witness siblings and key bits, so this proves exact
/// non-membership (old leaf) and insertion (new leaf) under one opening.
#[allow(clippy::too_many_arguments)]
fn m6_sparse_merkle_update<F: M6NativeCurve>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    hasher: &M6Hasher<'_, F>,
    node_domain: u64,
    key_bits: &[AssignedValue<F>],
    old_leaf: AssignedValue<F>,
    new_leaf: AssignedValue<F>,
    rng: &mut SplitMix64,
) -> (AssignedValue<F>, AssignedValue<F>) {
    let gate = range.gate();
    let (mut old, mut new) = (old_leaf, new_leaf);
    for &direction in key_bits.iter().rev() {
        let sibling = ctx.load_witness(rng.field::<F>());
        let old_left = GateInstructions::select(gate, ctx, sibling, old, direction);
        let old_right = GateInstructions::select(gate, ctx, old, sibling, direction);
        old = hasher.node(ctx, range, node_domain, old_left, old_right);
        let new_left = GateInstructions::select(gate, ctx, sibling, new, direction);
        let new_right = GateInstructions::select(gate, ctx, new, sibling, direction);
        new = hasher.node(ctx, range, node_domain, new_left, new_right);
    }
    (old, new)
}

/// Poseidon-only transition-like relation (one parity).
///
/// Opens a 40-field predecessor state commitment, proves consumed-credit
/// non-membership plus insertion at depth 256, inserts one pending-outgoing
/// descriptor at depth 128 keyed by the send ordinal, performs checked `u128`
/// credit/debit/sequence/quota arithmetic, and opens the successor commitment.
/// Public: predecessor commitment, successor commitment, sent amount. With
/// `schnorr`, it also verifies one native-field Schnorr signature over both
/// commitments. It contains no proof verification and no P-256 arithmetic.
fn m6_transition<F: M6NativeCurve>(
    builder: &mut BaseCircuitBuilder<F>,
    seed: u64,
    node: M6Node,
    schnorr: bool,
) -> Vec<F> {
    let range = builder.range_chip();
    let mut rng = SplitMix64(seed);
    let ctx = builder.main(0);
    let gate = range.gate();
    let sponge = KagemushaPoseidonChipV1::new(ctx, &range);
    let hasher = M6Hasher::new(ctx, &range, &sponge, node);

    // Consumed-credit map: credit_id absent, then bound to the Payment digest.
    let credit_id =
        [m6_u128(&mut rng), m6_u128(&mut rng)].map(|limb| ctx.load_witness(F::from_u128(limb)));
    let mut credit_bits = Vec::with_capacity(M6_CONSUMED_CREDIT_DEPTH);
    for limb in credit_id {
        credit_bits.extend(gate.num_to_bits(ctx, limb, 128));
    }
    let payment_digest = m6_digest(ctx, &range, &mut rng);
    let empty_credit = sponge.hash(ctx, &range, KAGEMUSHA_REPLAY_EMPTY_DOMAIN_V1, &[]);
    let credit_leaf = sponge.hash(
        ctx,
        &range,
        KAGEMUSHA_REPLAY_LEAF_DOMAIN_V1,
        &[
            credit_id[0],
            credit_id[1],
            payment_digest[0],
            payment_digest[1],
        ],
    );
    let (consumed_before, consumed_after) = m6_sparse_merkle_update(
        ctx,
        &range,
        &hasher,
        KAGEMUSHA_REPLAY_NODE_DOMAIN_V1,
        &credit_bits,
        empty_credit,
        credit_leaf,
        &mut rng,
    );

    // Checked u128 arithmetic: credit, gross debit, sequence, quota windows.
    let balance = m6_u128_cell(ctx, &range, (1_u128 << 100) | (m6_u128(&mut rng) >> 30));
    let credit_amount = m6_u128_cell(ctx, &range, u128::from(rng.next_u64() >> 8) + 1);
    let send_amount = m6_u128_cell(ctx, &range, u128::from(rng.next_u64() >> 8) + 1);
    let fee = m6_u128_cell(ctx, &range, u128::from(rng.next_u64() >> 40));
    for amount in [credit_amount, send_amount] {
        let is_zero = gate.is_zero(ctx, amount);
        gate.assert_is_const(ctx, &is_zero, &F::ZERO);
    }
    let credited = gate.add(ctx, balance, credit_amount);
    range.range_check(ctx, credited, 128);
    let debit = gate.add(ctx, send_amount, fee);
    range.range_check(ctx, debit, 128);
    let balance_after = gate.sub(ctx, credited, debit);
    range.range_check(ctx, balance_after, 128);
    let sequence = m6_u128_cell(ctx, &range, u128::from(rng.next_u64()));
    let sequence_after = gate.add(ctx, sequence, Constant(F::ONE));
    range.range_check(ctx, sequence_after, 128);
    let next_send = ctx.load_witness(F::from(rng.next_u64()));
    let next_send_bits = gate.num_to_bits(ctx, next_send, M6_ORDINAL_MAP_DEPTH);
    let next_send_after = gate.add(ctx, next_send, Constant(F::ONE));
    range.range_check(ctx, next_send_after, 128);
    let quota_day = m6_u128_cell(ctx, &range, u128::from(rng.next_u64() >> 20));
    let quota_month = m6_u128_cell(ctx, &range, u128::from(rng.next_u64() >> 16));
    let quota_day_after = gate.add(ctx, quota_day, debit);
    range.range_check(ctx, quota_day_after, 128);
    let quota_month_after = gate.add(ctx, quota_month, debit);
    range.range_check(ctx, quota_month_after, 128);

    // Pending-outgoing map keyed by the send ordinal: insert one descriptor.
    let receiver = m6_digest(ctx, &range, &mut rng);
    let empty_pending = sponge.hash(ctx, &range, M6_PENDING_EMPTY_DOMAIN, &[]);
    let pending_leaf = sponge.hash(
        ctx,
        &range,
        M6_PENDING_LEAF_DOMAIN,
        &[
            credit_id[0],
            credit_id[1],
            send_amount,
            fee,
            receiver[0],
            receiver[1],
        ],
    );
    let (pending_before, pending_after) = m6_sparse_merkle_update(
        ctx,
        &range,
        &hasher,
        M6_PENDING_NODE_DOMAIN,
        &next_send_bits,
        empty_pending,
        pending_leaf,
        &mut rng,
    );

    // Remaining fields: four other map roots, counters, lifecycle, policy, time, digests.
    let other_roots: [AssignedValue<F>; 4] =
        std::array::from_fn(|_| ctx.load_witness(rng.field::<F>()));
    let next_load = m6_u128_cell(ctx, &range, u128::from(rng.next_u64()));
    let next_redeem = m6_u128_cell(ctx, &range, u128::from(rng.next_u64()));
    let lifecycle = ctx.load_witness(F::ZERO);
    gate.assert_bit(ctx, lifecycle);
    let policy_epoch = ctx.load_witness(F::from(rng.next_u64()));
    range.range_check(ctx, policy_epoch, 64);
    let accepted_time = ctx.load_witness(F::from(rng.next_u64()));
    range.range_check(ctx, accepted_time, 64);
    let mut digests = Vec::with_capacity(24);
    for _ in 0..12 {
        digests.extend(m6_digest(ctx, &range, &mut rng));
    }
    let mut predecessor = vec![consumed_before, pending_before];
    predecessor.extend(other_roots);
    predecessor.extend([
        balance,
        sequence,
        next_send,
        next_load,
        next_redeem,
        quota_day,
        quota_month,
        lifecycle,
        policy_epoch,
        accepted_time,
    ]);
    predecessor.extend(digests);
    assert_eq!(predecessor.len(), M6_STATE_FIELDS, "state field layout");
    let mut successor = predecessor.clone();
    successor[0] = consumed_after;
    successor[1] = pending_after;
    successor[6] = balance_after;
    successor[7] = sequence_after;
    successor[8] = next_send_after;
    successor[11] = quota_day_after;
    successor[12] = quota_month_after;
    let predecessor_commitment = sponge.hash(ctx, &range, KAGEMUSHA_STATE_DOMAIN_V1, &predecessor);
    let successor_commitment = sponge.hash(ctx, &range, KAGEMUSHA_STATE_DOMAIN_V1, &successor);
    if schnorr {
        m6_schnorr_verify(
            ctx,
            &range,
            &sponge,
            &[predecessor_commitment, successor_commitment],
            &mut rng,
        );
    }
    let public = [predecessor_commitment, successor_commitment, send_amount];
    builder.assigned_instances[0].extend(public);
    public.iter().map(|cell| *cell.value()).collect()
}

/// One native-field Schnorr verification over two message elements, alone.
fn m6_schnorr_only<F: M6NativeCurve>(builder: &mut BaseCircuitBuilder<F>, seed: u64) -> Vec<F> {
    let range = builder.range_chip();
    let mut rng = SplitMix64(seed);
    let ctx = builder.main(0);
    let sponge = KagemushaPoseidonChipV1::new(ctx, &range);
    let message = [
        ctx.load_witness(rng.field::<F>()),
        ctx.load_witness(rng.field::<F>()),
    ];
    m6_schnorr_verify(ctx, &range, &sponge, &message, &mut rng);
    builder.assigned_instances[0].push(message[0]);
    vec![*message[0].value()]
}

fn m6_transition_case(node: M6Node, schnorr: bool) -> String {
    format!(
        "m6_transition_{}{}",
        node.label(),
        if schnorr { "_schnorr" } else { "" }
    )
}

fn m6_measure_transition<C>(k: u32, node: M6Node, schnorr: bool)
where
    C: SerdeCurveAffine,
    C::ScalarExt: M6NativeCurve + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    measure::<C>(
        &m6_transition_case(node, schnorr),
        k,
        Some(k as usize - 1),
        |builder, seed| m6_transition(builder, seed, node, schnorr),
    );
}

fn m6_measure_schnorr_only<C>(k: u32)
where
    C: SerdeCurveAffine,
    C::ScalarExt: M6NativeCurve + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    measure::<C>("m6_schnorr_only", k, Some(k as usize - 1), m6_schnorr_only);
}

fn m6_inventory_line<F: BigPrimeField>(
    case: &str,
    field: &str,
    k: u32,
    build: impl Fn(&mut BaseCircuitBuilder<F>, u64) -> Vec<F>,
) {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(k as usize)
        .use_instance_columns(1);
    builder.set_lookup_bits(k as usize - 1);
    let started = Instant::now();
    build(&mut builder, KEYGEN_SEED);
    let build_ms = elapsed_ms(started);
    let statistics = builder.statistics();
    let params = builder.calculate_params(Some(MINIMUM_ROWS));
    println!(
        "M6_INVENTORY case={case} field={field} k={k} advice_cells={} lookup_cells={} advice_cols={} lookup_cols={} fixed_cols={} build_ms={build_ms:.0}",
        statistics.gate.total_advice_per_phase[0],
        statistics.total_lookup_advice_per_phase[0],
        params.num_advice_per_phase[0],
        params.num_lookup_advice_per_phase[0],
        params.num_fixed,
    );
}

/// Exact Base cells of one Pow5 permutation, one repository tree node and one state opening.
fn m6_poseidon_unit_costs<F: M6NativeCurve>(field: &str) {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(16)
        .use_instance_columns(1);
    builder.set_lookup_bits(K16_LOOKUP_BITS);
    let range = builder.range_chip();
    let sponge = KagemushaPoseidonChipV1::new(builder.main(0), &range);
    let mut single =
        PoseidonHasher::<F, KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KAGEMUSHA_IPA_POSEIDON_RATE_V1>::new(
            F::kagemusha_poseidon_spec_v1().clone(),
        );
    single.initialize_consts(builder.main(0), range.gate());
    let left = builder.main(0).load_witness(F::from(3_u64));
    let right = builder.main(0).load_witness(F::from(5_u64));
    let cells =
        |builder: &BaseCircuitBuilder<F>| builder.statistics().gate.total_advice_per_phase[0];
    let start = cells(&builder);
    single.hash_fix_len_array(builder.main(0), range.gate(), &[left]);
    let one_permutation = cells(&builder);
    sponge.hash(
        builder.main(0),
        &range,
        KAGEMUSHA_REPLAY_NODE_DOMAIN_V1,
        &[left, right],
    );
    let repo_node = cells(&builder);
    let state = vec![left; M6_STATE_FIELDS];
    sponge.hash(builder.main(0), &range, KAGEMUSHA_STATE_DOMAIN_V1, &state);
    let state_opening = cells(&builder);
    println!(
        "M6_POSEIDON_UNIT field={field} one_permutation_cells={} repo_node_cells={} state40_opening_cells={}",
        one_permutation - start,
        repo_node - one_permutation,
        state_opening - repo_node,
    );
}

/// Measurement harness (M6): synthesis-only cell/column inventory of every M6 circuit.
#[test]
#[ignore = "M6 fast-path measurement harness; run explicitly in release"]
fn m6_inventory() {
    m6_poseidon_unit_costs::<Fp>("eq");
    m6_poseidon_unit_costs::<Fq>("ep");
    for k in 12..=20_u32 {
        for node in [M6Node::RepoSponge, M6Node::OnePermutationStandIn] {
            for schnorr in [false, true] {
                m6_inventory_line::<Fp>(&m6_transition_case(node, schnorr), "eq", k, |b, s| {
                    m6_transition(b, s, node, schnorr)
                });
            }
        }
        m6_inventory_line::<Fp>("m6_schnorr_only", "eq", k, m6_schnorr_only);
    }
    for node in [M6Node::RepoSponge, M6Node::OnePermutationStandIn] {
        m6_inventory_line::<Fq>(&m6_transition_case(node, true), "ep", 16, |b, s| {
            m6_transition(b, s, node, true)
        });
    }
    m6_inventory_line::<Fq>("m6_schnorr_only", "ep", 16, m6_schnorr_only);
}

macro_rules! m6_transition_tests {
    ($($name:ident: $curve:ty, $k:expr, $node:expr, $schnorr:expr;)+) => {$(
        /// Measurement harness (M6): Poseidon-only transition-like relation,
        /// optionally with one native-field Schnorr verification; proves,
        /// verifies and prints one `G3_SCALING` line.
        #[test]
        #[ignore = "M6 fast-path measurement harness; run explicitly in release"]
        fn $name() {
            m6_measure_transition::<$curve>($k, $node, $schnorr);
        }
    )+};
}

m6_transition_tests! {
    m6_t1_repo_eq_k16: EqAffine, 16, M6Node::RepoSponge, false;
    m6_t1_repo_ep_k16: EpAffine, 16, M6Node::RepoSponge, false;
    m6_t1_repo_eq_k17: EqAffine, 17, M6Node::RepoSponge, false;
    m6_t1_repo_ep_k17: EpAffine, 17, M6Node::RepoSponge, false;
    m6_t1_repo_eq_k18: EqAffine, 18, M6Node::RepoSponge, false;
    m6_t1_repo_ep_k18: EpAffine, 18, M6Node::RepoSponge, false;
    m6_t1_repo_eq_k19: EqAffine, 19, M6Node::RepoSponge, false;
    m6_t1_repo_ep_k19: EpAffine, 19, M6Node::RepoSponge, false;
    m6_t1_repo_eq_k20: EqAffine, 20, M6Node::RepoSponge, false;
    m6_t1_repo_ep_k20: EpAffine, 20, M6Node::RepoSponge, false;
    m6_t1_onep_eq_k14: EqAffine, 14, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_ep_k14: EpAffine, 14, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_eq_k15: EqAffine, 15, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_ep_k15: EpAffine, 15, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_eq_k16: EqAffine, 16, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_ep_k16: EpAffine, 16, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_eq_k17: EqAffine, 17, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_ep_k17: EpAffine, 17, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_eq_k18: EqAffine, 18, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_ep_k18: EpAffine, 18, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_eq_k19: EqAffine, 19, M6Node::OnePermutationStandIn, false;
    m6_t1_onep_ep_k19: EpAffine, 19, M6Node::OnePermutationStandIn, false;
    m6_t2_repo_eq_k16: EqAffine, 16, M6Node::RepoSponge, true;
    m6_t2_repo_ep_k16: EpAffine, 16, M6Node::RepoSponge, true;
    m6_t2_repo_eq_k18: EqAffine, 18, M6Node::RepoSponge, true;
    m6_t2_repo_ep_k18: EpAffine, 18, M6Node::RepoSponge, true;
    m6_t2_repo_eq_k20: EqAffine, 20, M6Node::RepoSponge, true;
    m6_t2_repo_ep_k20: EpAffine, 20, M6Node::RepoSponge, true;
    m6_t2_onep_eq_k15: EqAffine, 15, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_ep_k15: EpAffine, 15, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_eq_k16: EqAffine, 16, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_ep_k16: EpAffine, 16, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_eq_k17: EqAffine, 17, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_ep_k17: EpAffine, 17, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_eq_k18: EqAffine, 18, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_ep_k18: EpAffine, 18, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_eq_k19: EqAffine, 19, M6Node::OnePermutationStandIn, true;
    m6_t2_onep_ep_k19: EpAffine, 19, M6Node::OnePermutationStandIn, true;
}

macro_rules! m6_schnorr_tests {
    ($($name:ident: $curve:ty, $k:expr;)+) => {$(
        /// Measurement harness (M6): one native-field Schnorr verification alone;
        /// proves, verifies and prints one `G3_SCALING` line.
        #[test]
        #[ignore = "M6 fast-path measurement harness; run explicitly in release"]
        fn $name() {
            m6_measure_schnorr_only::<$curve>($k);
        }
    )+};
}

m6_schnorr_tests! {
    m6_schnorr_only_eq_k12: EqAffine, 12;
    m6_schnorr_only_ep_k12: EpAffine, 12;
    m6_schnorr_only_eq_k13: EqAffine, 13;
    m6_schnorr_only_ep_k13: EpAffine, 13;
    m6_schnorr_only_eq_k14: EqAffine, 14;
    m6_schnorr_only_ep_k14: EpAffine, 14;
    m6_schnorr_only_eq_k16: EqAffine, 16;
    m6_schnorr_only_ep_k16: EpAffine, 16;
}

/// Succinct IPA verifying key exactly as the KAGEMUSHA composite circuits derive it.
fn m6_succinct_vk<C: CurveAffine>(params: &ParamsIPA<C>) -> IpaSuccinctVerifyingKey<C> {
    let hash_to_curve = <C::CurveExt as CurveExt>::hash_to_curve("Halo2-Parameters");
    IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

/// Proof reader recording how many bytes the in-circuit transcript consumed.
struct M6CountingReader<'proof> {
    bytes: &'proof [u8],
    position: Rc<Cell<usize>>,
}

impl Read for M6CountingReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let start = self.position.get();
        let available = &self.bytes[start..];
        let len = available.len().min(output.len());
        output[..len].copy_from_slice(&available[..len]);
        self.position.set(start + len);
        Ok(len)
    }
}

type M6DeferredLoader<'chip, C> = Rc<Halo2Loader<C, DeferredScalarEccChip<'chip, C>>>;
type M6DeferredScalar<'chip, C> =
    snark_verifier::loader::halo2::Scalar<C, DeferredScalarEccChip<'chip, C>>;
type M6DeferredTranscript<'chip, 'proof, C> = PoseidonTranscript<
    C,
    M6DeferredLoader<'chip, C>,
    M6CountingReader<'proof>,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

/// Field-native Poseidon digest through the scalar-half loader (as `deferred_parent`).
fn m6_loader_poseidon_digest<'chip, C>(
    loader: &M6DeferredLoader<'chip, C>,
    elements: Vec<AssignedValue<C::ScalarExt>>,
) -> AssignedValue<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let elements = elements
        .into_iter()
        .map(|element| loader.scalar_from_assigned(element))
        .collect::<Vec<_>>();
    let mut poseidon = LoaderPoseidon::<
        C::ScalarExt,
        M6DeferredScalar<'chip, C>,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    >::new::<
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    >(loader);
    poseidon.update(&elements);
    poseidon.squeeze().into_assigned()
}

/// Scalar half of one in-circuit succinct verification of a KAGEMUSHA IPA proof.
///
/// Mirrors `deferred_parent::verify_ordinary_proof_v1`: the proof is parsed through the
/// fixed Poseidon transcript, all challenges and scalar arithmetic are constrained in this
/// field, and every curve operation is recorded as a symbolic equation over canonical
/// non-native point sources. The deferred audit (sources and equations) is committed with
/// the production element encoding; the digest and the final transcript squeeze are public.
/// The equations are exported through `stash` for the reciprocal curve half. History
/// folding (BGH19 accumulation of the emitted accumulator) is not included.
fn m6_scalar_half<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    inner: &MeasuredProof<C>,
    svk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C>,
    stash: &RefCell<Option<DeferredEquationWitness<C>>>,
) -> Vec<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let loader: M6DeferredLoader<'_, C> = Halo2Loader::new(
        DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer),
        std::mem::take(builder.pool(0)),
    );
    let (digest, binding) = {
        let loaded = protocol.loaded(&loader);
        let instances = vec![
            inner
                .public
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>(),
        ];
        let position = Rc::new(Cell::new(0));
        let reader = M6CountingReader {
            bytes: &inner.proof,
            position: Rc::clone(&position),
        };
        let mut transcript =
            M6DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&loader, reader);
        let parsed = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::read_proof(
            svk,
            &loaded,
            &instances,
            &mut transcript,
        )
        .expect("in-circuit proof parse");
        let accumulators =
            PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(svk, &loaded, &instances, &parsed)
                .expect("in-circuit succinct verification");
        assert_eq!(accumulators.len(), 1, "one IPA accumulator");
        let binding = transcript.squeeze_challenge().into_assigned();
        drop(transcript);
        assert_eq!(
            position.get(),
            inner.proof.len(),
            "in-circuit transcript must consume the whole proof"
        );
        let equation_count = loader.ecc_chip().equation_count();
        let selectors = {
            let mut ctx = loader.ctx_mut();
            (0..equation_count)
                .map(|_| ctx.main().load_constant(C::ScalarExt::ONE))
                .collect::<Vec<_>>()
        };
        let tags = vec![1_u32; equation_count];
        let elements = {
            let chip = loader.ecc_chip();
            let mut ctx = loader.ctx_mut();
            chip.assigned_equation_poseidon_elements_v1(&mut ctx, &tags, &selectors)
                .expect("deferred audit elements")
        };
        let digest = m6_loader_poseidon_digest(&loader, elements);
        *stash.borrow_mut() = Some(loader.ecc_chip().witness());
        drop(accumulators);
        drop(parsed);
        drop(instances);
        drop(loaded);
        (digest, binding)
    };
    *builder.pool(0) = loader.take_ctx();
    builder.assigned_instances[0].extend([digest, binding]);
    vec![*digest.value(), *binding.value()]
}

/// Reciprocal curve half: assigns every deferred source natively, recomputes the audit
/// commitment with the production reciprocal Poseidon, and enforces all equations through
/// one challenge-batched serialized variable-base MSM (the transport-decider layout,
/// `constrain_deferred_equation_batch_generic_v1`; no dense MSM lanes).
fn m6_ec_half<C>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    witness: &DeferredEquationWitness<C>,
) -> Vec<C::Base>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    let range = builder.range_chip();
    let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let mut chip = PastaCycleEccChip::<C>::new(&base, &scalar);
    let mut ctx = std::mem::take(builder.pool(0));
    let selectors = (0..witness.equations.len())
        .map(|_| ctx.main().load_witness(C::Base::ONE))
        .collect::<Vec<_>>();
    let audit = chip
        .assign_deferred_equations_with_selectors(&mut ctx, witness, &selectors)
        .expect("reciprocal source assignment");
    let tags = vec![1_u32; witness.equations.len()];
    let (elements, _) = chip
        .assigned_equation_poseidon_elements_v1(&mut ctx, &audit, &tags, &selectors)
        .expect("reciprocal audit elements");
    let digest = constrain_reciprocal_poseidon_v1::<C>(&mut ctx, &base, &scalar, elements);
    chip.constrain_deferred_equation_batch_generic_v1(&mut ctx, &audit, &selectors, &digest)
        .expect("reciprocal batched equations");
    let public = *digest.native();
    *builder.pool(0) = ctx;
    builder.assigned_instances[0].push(public);
    vec![*public.value()]
}

fn m6_point_bytes<C: CurveAffine>(point: C) -> Vec<u8> {
    point.to_bytes().as_ref().to_vec()
}

fn m6_median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// Inner k = 16 proof verified by the recursion measurements.
#[derive(Clone, Copy, Debug)]
enum M6Inner {
    /// Synthetic Base circuit of this many advice columns (`chained_mul_add`).
    Synthetic(usize),
    /// The M6 transition stand-in with one-permutation nodes (29 + 1 columns at k = 16).
    TransitionOnePermutation,
}

impl M6Inner {
    fn label(self) -> String {
        match self {
            Self::Synthetic(columns) => format!("synthetic_w{columns}"),
            Self::TransitionOnePermutation => "transition_one_perm_k16".to_owned(),
        }
    }
}

macro_rules! m6_recursion_driver {
    ($driver:ident, $inner:ty, $outer:ty, $label:literal) => {
        /// Prove one k = 16 inner proof, time its native verifier, then prove the
        /// scalar half (same parity) and, when requested, the generic curve half
        /// (other parity).
        fn $driver(inner_kind: M6Inner, curve_half: bool) {
            let width = inner_kind.label();
            let inner = measure_impl::<$inner>(
                &format!("m6_rec_inner_{}_{width}", $label),
                16,
                Some(K16_LOOKUP_BITS),
                None,
                |builder, seed| match inner_kind {
                    M6Inner::Synthetic(columns) => {
                        chained_mul_add(builder, 16, columns, true, seed)
                    }
                    M6Inner::TransitionOnePermutation => {
                        m6_transition(builder, seed, M6Node::OnePermutationStandIn, false)
                    }
                },
            );
            let svk = m6_succinct_vk(&inner.params);
            let protocol = compile(
                &inner.params,
                &inner.vk,
                Config::ipa().with_num_instance(vec![inner.public.len()]),
            );
            let instances = vec![inner.public.clone()];
            let deciding_key = IpaDecidingKey::new(svk.clone(), inner.params.get_g().to_vec());
            let (mut succinct, mut decide, mut full) = (Vec::new(), Vec::new(), Vec::new());
            let (mut succinct_cpu, mut decide_cpu, mut full_cpu) =
                (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..M6_NATIVE_REPEATS {
                let started = Instant::now();
                let cpu_started = process_cpu_ms();
                let mut transcript = KagemushaTranscript::<$inner, _>::new::<
                    KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
                >(Cursor::new(inner.proof.as_slice()));
                let parsed = PlonkSuccinctVerifier::<IpaAs<$inner, Bgh19>>::read_proof(
                    &svk,
                    &protocol,
                    &instances,
                    &mut transcript,
                )
                .expect("native proof parse");
                let mut accumulators = PlonkSuccinctVerifier::<IpaAs<$inner, Bgh19>>::verify(
                    &svk,
                    &protocol,
                    &instances,
                    &parsed,
                )
                .expect("native succinct verification");
                succinct.push(elapsed_ms(started));
                succinct_cpu.push(process_cpu_ms() - cpu_started);
                let started = Instant::now();
                let cpu_started = process_cpu_ms();
                <IpaAs<$inner, Bgh19> as AccumulationDecider<$inner, NativeLoader>>::decide(
                    &deciding_key,
                    accumulators.remove(0),
                )
                .expect("native IPA decide");
                decide.push(elapsed_ms(started));
                decide_cpu.push(process_cpu_ms() - cpu_started);
                let started = Instant::now();
                let cpu_started = process_cpu_ms();
                let (raw, generator) = inner.proof.split_at(inner.proof.len() - 32);
                let folded = verified_folded_generator(&inner.params, &inner.vk, raw, &inner.public)
                    .expect("native full verification");
                assert_eq!(m6_point_bytes(folded), generator, "folded generator");
                full.push(elapsed_ms(started));
                full_cpu.push(process_cpu_ms() - cpu_started);
            }
            let threads =
                std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".to_owned());
            println!(
                "M6_NATIVE_IPA inner={} inner_kind={width} k=16 proof_bytes={} threads={threads} runs={} succinct_ms_median={:.1} decide_ms_median={:.1} full_verify_ms_median={:.1} succinct_cpu_ms_median={:.1} decide_cpu_ms_median={:.1} full_verify_cpu_ms_median={:.1}",
                $label,
                inner.proof.len(),
                M6_NATIVE_REPEATS,
                m6_median(succinct),
                m6_median(decide),
                m6_median(full),
                m6_median(succinct_cpu),
                m6_median(decide_cpu),
                m6_median(full_cpu),
            );
            let stash = RefCell::new(None);
            drop(measure_impl::<$inner>(
                &format!("m6_rec_scalar_half_inner_{}_{width}", $label),
                16,
                Some(K16_LOOKUP_BITS),
                Some(inner.params.clone()),
                |builder, _| m6_scalar_half::<$inner>(builder, &inner, &svk, &protocol, &stash),
            ));
            let witness = stash
                .borrow_mut()
                .take()
                .expect("scalar half exported its deferred equations");
            println!(
                "M6_RECURSION inner={} inner_kind={width} inner_proof_bytes={} sources={} equations={} terms={}",
                $label,
                inner.proof.len(),
                witness.sources.len(),
                witness.equations.len(),
                witness.equations.iter().map(Vec::len).sum::<usize>(),
            );
            if !curve_half {
                return;
            }
            drop(measure_impl::<$outer>(
                &format!("m6_rec_ec_half_inner_{}_{width}", $label),
                16,
                Some(K16_LOOKUP_BITS),
                None,
                |builder, _| m6_ec_half::<$inner>(builder, &witness),
            ));
        }
    };
}

m6_recursion_driver!(m6_measure_recursion_eq_inner, EqAffine, EpAffine, "eq");
m6_recursion_driver!(m6_measure_recursion_ep_inner, EpAffine, EqAffine, "ep");

macro_rules! m6_recursion_tests {
    ($($name:ident: $driver:ident, $inner_kind:expr, $curve_half:expr;)+) => {$(
        /// Measurement harness (M6): one recursive succinct verification of a
        /// k = 16 IPA proof: its scalar half (and, when enabled, the generic
        /// reciprocal curve half), plus native verifier timings.
        #[test]
        #[ignore = "M6 fast-path measurement harness; run explicitly in release"]
        fn $name() {
            $driver($inner_kind, $curve_half);
        }
    )+};
}

m6_recursion_tests! {
    m6_rec_eq_inner_w001: m6_measure_recursion_eq_inner, M6Inner::Synthetic(1), true;
    m6_rec_scalar_eq_inner_w008: m6_measure_recursion_eq_inner, M6Inner::Synthetic(8), false;
    m6_rec_scalar_eq_inner_w023: m6_measure_recursion_eq_inner, M6Inner::Synthetic(23), false;
    m6_rec_scalar_ep_inner_w001: m6_measure_recursion_ep_inner, M6Inner::Synthetic(1), false;
    m6_rec_scalar_eq_inner_transition: m6_measure_recursion_eq_inner, M6Inner::TransitionOnePermutation, false;
    m6_rec_scalar_ep_inner_transition: m6_measure_recursion_ep_inner, M6Inner::TransitionOnePermutation, false;
}

/// Native Pasta Schnorr verification time in microseconds (Poseidon challenge included).
fn m6_native_schnorr_us<F: M6NativeCurve>(iterations: usize) -> f64 {
    let mut rng = SplitMix64(PROVER_SEED);
    let generator = F::Curve::generator();
    let secret: F::Scalar = rng.field();
    let nonce: F::Scalar = rng.field();
    let key = (generator * secret).to_affine();
    let commitment = (generator * nonce).to_affine();
    let message = [rng.field::<F>(), rng.field::<F>()];
    let challenge = |commitment: F::Curve, key: F::Curve| -> F::Scalar {
        let (key_x, key_y) = key.into_coordinates();
        let (commitment_x, commitment_y) = commitment.into_coordinates();
        let digest = kagemusha_native_poseidon_hash::<F>(
            M6_SCHNORR_DOMAIN,
            &[
                commitment_x,
                commitment_y,
                key_x,
                key_y,
                message[0],
                message[1],
            ],
        );
        m6_reduce::<F::Scalar>(digest.to_repr().as_ref())
    };
    let response = nonce + challenge(commitment, key) * secret;
    let started = thread_cpu_ms();
    for _ in 0..iterations {
        let commitment = std::hint::black_box(commitment);
        let key = std::hint::black_box(key);
        let e = challenge(commitment, key);
        let lhs = generator * std::hint::black_box(response);
        let rhs = commitment.to_curve() + key * e;
        assert!(lhs == rhs, "native Schnorr verification");
    }
    (thread_cpu_ms() - started) * 1000.0 / iterations as f64
}

/// Measurement harness (M6): native single-thread verifier costs (thread CPU time) of the
/// signature primitives (P-256 ECDSA with SHA-256 of a 256-byte body, SEC1 key
/// decoding, SHA-256 of a 10,000-byte Payment, Pasta Schnorr in both fields).
#[test]
#[ignore = "M6 fast-path measurement harness; run explicitly in release"]
fn m6_native_signature_costs() {
    use p256::ecdsa::{
        Signature, SigningKey, VerifyingKey,
        signature::{Signer as _, Verifier as _},
    };
    use sha2::{Digest as _, Sha256};

    let signing = SigningKey::from_bytes((&[0x42_u8; 32]).into()).expect("P-256 signing key");
    let verifying = signing.verifying_key().clone();
    let body = [0x5a_u8; 256];
    let signature: Signature = signing.sign(&body);
    let signature = signature.normalize_s().unwrap_or(signature);
    let sec1 = verifying.to_encoded_point(false);
    for _ in 0..100 {
        verifying
            .verify(&body, &signature)
            .expect("valid P-256 signature");
    }
    let iterations = M6_SIGNATURE_ITERATIONS;
    let started = thread_cpu_ms();
    for _ in 0..iterations {
        std::hint::black_box(&verifying)
            .verify(
                std::hint::black_box(&body),
                std::hint::black_box(&signature),
            )
            .expect("valid P-256 signature");
    }
    let p256_verify_us = (thread_cpu_ms() - started) * 1000.0 / iterations as f64;
    let started = thread_cpu_ms();
    for _ in 0..iterations {
        let key = VerifyingKey::from_sec1_bytes(std::hint::black_box(sec1.as_bytes()))
            .expect("SEC1 P-256 key");
        key.verify(&body, &signature)
            .expect("valid P-256 signature");
    }
    let p256_decode_verify_us = (thread_cpu_ms() - started) * 1000.0 / iterations as f64;
    let payment = vec![0xa5_u8; 10_000];
    let started = thread_cpu_ms();
    for _ in 0..iterations {
        std::hint::black_box(Sha256::digest(std::hint::black_box(&payment)));
    }
    let sha256_payment_us = (thread_cpu_ms() - started) * 1000.0 / iterations as f64;
    let schnorr_fp_us = m6_native_schnorr_us::<Fp>(iterations / 4);
    let schnorr_fq_us = m6_native_schnorr_us::<Fq>(iterations / 4);
    let threads = std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".to_owned());
    println!(
        "M6_NATIVE_SIG threads={threads} iterations={iterations} p256_ecdsa_verify_256B_us={p256_verify_us:.1} p256_sec1_decode_and_verify_us={p256_decode_verify_us:.1} sha256_10000B_us={sha256_payment_us:.1} pasta_schnorr_pallas_in_fp_us={schnorr_fp_us:.1} pasta_schnorr_vesta_in_fq_us={schnorr_fq_us:.1}"
    );
}

// ---------------------------------------------------------------------------
// M7: split-lineage step relations sigma_send and sigma_recv.
//
// The split-lineage design proves a small non-recursive step relation on the payment path and
// the recursive lineage in the background. Each step circuit below is single parity (Eq),
// non-recursive and uses only Poseidon and checked u128 arithmetic:
//
// - it opens the predecessor state commitment and commits the successor;
// - it appends one chain accumulator (send_chain for sigma_send, recv_chain for sigma_recv);
// - it checks balance, sequence, send ordinal, fee, policy epoch and accepted-time window;
// - sigma_send also hashes the Request body (the credit_id preimage) as field elements and
//   ties its ordinal, amount, fee and receiver to the effect and the opened state;
// - one Poseidon digest of the G1 statement field encoding is public (sigma_send also makes
//   its Request digest public).
//
// Two Poseidon backends compute the identical hash function, asserted against the native hash
// on every call: halo2-base gates (`KagemushaPoseidonChipV1`) and the native Pasta Poseidon
// lanes (`pasta_native_poseidon.rs`, one or two lanes, k <= 16). Two state layouts: the flat
// 40-field commitment measured in M6, and a two-level commitment of a 10-field core plus one
// digest of the other 30 fields. Identity and policy fields that live only in the two-level
// remainder (scheme, asset, wallet, credential, fee schedule) are witnesses bound by the public
// digests; the split-lineage security review (change 1) binds them natively through the
// predecessor lineage proof's public outputs. The flat layout reuses the opened state cells for
// them at no extra cost. Enabled regulatory controls (review change 4) are not included.
// ---------------------------------------------------------------------------

/// Fields of the flat state commitment (the M6 40-field opening).
const M7_STATE_FIELDS: usize = 40;
/// Fields the step relations read or write; they form the two-level commitment's core.
const M7_CORE_FIELDS: usize = 10;
const M7_BALANCE: usize = 0;
const M7_SEQUENCE: usize = 1;
const M7_NEXT_SEND: usize = 2;
const M7_NEXT_LOAD: usize = 3;
const M7_SEND_CHAIN: usize = 4;
const M7_RECV_CHAIN: usize = 5;
const M7_STATE_NONCE: usize = 6;
const M7_LIFECYCLE: usize = 7;
const M7_POLICY_EPOCH: usize = 8;
const M7_TIME_FLOOR: usize = 9;
/// First remainder field: the state version, then five 128-bit limb pairs.
const M7_STATE_VERSION: usize = 10;
const M7_SCHEME_ID: usize = 11;
const M7_ASSET: usize = 13;
const M7_WALLET_ID: usize = 15;
const M7_CREDENTIAL: usize = 17;
const M7_FEE_SCHEDULE: usize = 19;
/// First of the 19 other carried fields (map roots, remaining policy fields).
const M7_OTHER_CARRIED: usize = 21;
/// G1 statement field encoding: version, scheme (2), relation (2), credential (2), asset (2),
/// lifecycle, sequence, next_load, predecessor (own parity + other parity's 2 limbs),
/// successor (same), effect tag and the 13-field effect union.
const M7_STATEMENT_FIELDS: usize = 32;
const M7_EFFECT_UNION_FIELDS: usize = 13;
/// G1 Request body (credit_id preimage) field encoding.
const M7_REQUEST_FIELDS: usize = 24;
/// send_chain entry: previous chain plus the pending-outgoing leaf fields.
const M7_SEND_CHAIN_FIELDS: usize = 10;
/// recv_chain entry: previous chain, credit_id (2), payer wallet (2) and amount.
const M7_RECV_CHAIN_FIELDS: usize = 6;
const M7_VERSION: u64 = 1;
const M7_LIFECYCLE_ACTIVE: u64 = 1;
const M7_EFFECT_SEND: u64 = 3;
const M7_EFFECT_RECEIVE: u64 = 4;
const M7_RELATION_SEND: u128 = u128::from_le_bytes(*b"m7-sigma-send-v1");
const M7_RELATION_RECEIVE: u128 = u128::from_le_bytes(*b"m7-sigma-recv-v1");
const M7_STATEMENT_DOMAIN: u64 = u64::from_le_bytes(*b"m7stmnt1");
const M7_REQUEST_DOMAIN: u64 = u64::from_le_bytes(*b"m7reqst1");
const M7_SEND_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"m7sendc1");
const M7_RECV_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"m7recvc1");
const M7_CORE_DOMAIN: u64 = u64::from_le_bytes(*b"m7score1");
const M7_REST_DOMAIN: u64 = u64::from_le_bytes(*b"m7srest1");
const M7_U64_BITS: usize = 64;
const M7_U128_BITS: usize = 128;
/// Rows of one native lane permutation block (`pasta_native_poseidon.rs`: 65 rounds + state).
const M7_NATIVE_PERMUTATION_ROWS: usize = 66;
/// Largest `k` of the native lanes' fixed row envelope.
const M7_NATIVE_MAX_K: u32 = 16;
/// Proof-size gate of the split-lineage recommendation (3.5 KB).
const M7_PROOF_BYTES_GATE: usize = 3_500;

/// Step relation on the split-lineage payment path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum M7Step {
    /// sigma_send: debit amount + fee, advance the send ordinal, append send_chain, bind the Request.
    Send,
    /// sigma_recv: credit the amount and append recv_chain (precomputed: payment digest zero).
    Receive,
}

/// State commitment layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum M7Layout {
    /// One Poseidon hash over all 40 state fields, opened for predecessor and successor.
    Flat,
    /// Poseidon over the 10-field core plus one carried digest of the other 30 fields.
    TwoLevel,
}

/// Poseidon permutation backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum M7Backend {
    /// halo2-base Poseidon gates (`KagemushaPoseidonChipV1`).
    Base,
    /// Native Pasta Poseidon lanes; Base keeps the domain prefix, absorption and padding.
    Native,
}

/// Witness mutation for the relation checks (each must be rejected).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum M7Mutation {
    /// Honest witness.
    None,
    /// sigma_send: amount + fee exceeds the balance.
    Overdraft,
    /// sigma_recv: balance + amount reaches 2^128.
    Overflow,
    /// sigma_send: the Request's policy epoch is newer than the payer's.
    StaleEpoch,
    /// sigma_send: the accepted lower time is below the payer's accepted-time floor.
    EarlyTime,
}

impl M7Step {
    fn label(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Receive => "recv",
        }
    }
}

impl M7Layout {
    fn label(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::TwoLevel => "two_level",
        }
    }
}

impl M7Backend {
    fn label(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Native => "native",
        }
    }
}

fn m7_case(step: M7Step, backend: M7Backend, layout: M7Layout) -> String {
    format!(
        "m7_sigma_{}_{}_{}",
        step.label(),
        backend.label(),
        layout.label()
    )
}

/// Pow5 permutations of one domain-and-arity-prefixed sponge hash of `fields` inputs.
fn m7_hash_permutations(fields: usize) -> usize {
    (fields + 2) / 2 + 1
}

/// Exact permutation count of one step relation.
fn m7_permutations(step: M7Step, layout: M7Layout) -> usize {
    let state = match layout {
        M7Layout::Flat => M7_STATE_FIELDS,
        M7Layout::TwoLevel => M7_CORE_FIELDS + 1,
    };
    let (chain, request) = match step {
        M7Step::Send => (
            M7_SEND_CHAIN_FIELDS,
            m7_hash_permutations(M7_REQUEST_FIELDS),
        ),
        M7Step::Receive => (M7_RECV_CHAIN_FIELDS, 0),
    };
    2 * m7_hash_permutations(state)
        + m7_hash_permutations(chain)
        + request
        + m7_hash_permutations(M7_STATEMENT_FIELDS)
}

/// The smallest native lane count (one or two) holding `permutations` at `k`, if any.
fn m7_native_lanes(k: u32, permutations: usize) -> Option<usize> {
    if k > M7_NATIVE_MAX_K {
        return None;
    }
    let blocks = ((1_usize << k) - MINIMUM_ROWS) / M7_NATIVE_PERMUTATION_ROWS;
    (1..=2).find(|lanes| lanes * blocks >= permutations)
}

/// Poseidon hashing through one backend, counting permutations.
struct M7Hasher<'jobs, F: KagemushaPoseidonFieldV1> {
    chip: Option<KagemushaPoseidonChipV1<F>>,
    native: Option<&'jobs mut PastaNativePoseidonJobsV1<F>>,
    permutations: usize,
}

impl<'jobs, F: KagemushaPoseidonFieldV1> M7Hasher<'jobs, F> {
    fn new(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        native: Option<&'jobs mut PastaNativePoseidonJobsV1<F>>,
    ) -> Self {
        let chip = native
            .is_none()
            .then(|| KagemushaPoseidonChipV1::new(ctx, range));
        Self {
            chip,
            native,
            permutations: 0,
        }
    }

    /// `H(domain, arity, inputs)` in either backend; asserted against the native hash.
    fn hash(
        &mut self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        domain: u64,
        inputs: &[AssignedValue<F>],
    ) -> AssignedValue<F> {
        let arity = u64::try_from(inputs.len()).expect("bounded Poseidon arity");
        let digest = match (&self.chip, self.native.as_deref_mut()) {
            (Some(chip), _) => chip.hash(ctx, range, domain, inputs),
            (None, Some(jobs)) => {
                let prefix = [F::from(domain), F::from(arity)];
                let mut cells = Vec::with_capacity(inputs.len() + 2);
                cells.extend(prefix.map(|value| ctx.load_constant(value)));
                cells.extend_from_slice(inputs);
                jobs.queue_raw(ctx, range.gate(), cells, &prefix)
                    .unwrap_or_else(|error| panic!("native Poseidon lanes: {error}"))
            }
            (None, None) => unreachable!("one Poseidon backend is always present"),
        };
        self.permutations += m7_hash_permutations(inputs.len());
        let values = inputs.iter().map(|cell| *cell.value()).collect::<Vec<_>>();
        assert_eq!(
            *digest.value(),
            kagemusha_native_poseidon_hash::<F>(domain, &values),
            "M7 in-circuit Poseidon digest equals the native hash"
        );
        digest
    }
}

/// Predecessor or successor commitment preimage for `layout`.
fn m7_state_inputs<F: ScalarField>(
    core: &[AssignedValue<F>],
    rest: &[AssignedValue<F>],
    rest_digest: Option<AssignedValue<F>>,
) -> Vec<AssignedValue<F>> {
    let mut inputs = core.to_vec();
    inputs.extend_from_slice(rest);
    inputs.extend(rest_digest);
    inputs
}

/// A 128-bit limb pair of the state remainder: the opened cells (flat) or fresh witnesses.
fn m7_identity<F: ScalarField>(
    ctx: &mut Context<F>,
    rest: &[AssignedValue<F>],
    values: &[F],
    index: usize,
) -> [AssignedValue<F>; 2] {
    if rest.is_empty() {
        [
            ctx.load_witness(values[index]),
            ctx.load_witness(values[index + 1]),
        ]
    } else {
        [
            rest[index - M7_CORE_FIELDS],
            rest[index + 1 - M7_CORE_FIELDS],
        ]
    }
}

/// Two 128-bit limbs of a 32-byte digest, bound only through the public digests.
fn m7_limbs<F: ScalarField>(ctx: &mut Context<F>, rng: &mut SplitMix64) -> [AssignedValue<F>; 2] {
    [m6_u128(rng), m6_u128(rng)].map(|limb| ctx.load_witness(F::from_u128(limb)))
}

/// One range-checked `u64` witness.
fn m7_u64_cell<F: ScalarField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u64,
) -> AssignedValue<F> {
    let cell = ctx.load_witness(F::from(value));
    range.range_check(ctx, cell, M7_U64_BITS);
    cell
}

/// Constrain `low <= high` for cells already range checked to `bits` (far below the field).
fn m7_assert_at_most<F: ScalarField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    low: AssignedValue<F>,
    high: AssignedValue<F>,
    bits: usize,
) {
    let difference = range.gate().sub(ctx, high, low);
    range.range_check(ctx, difference, bits);
}

/// One built step relation.
struct M7Built<F> {
    /// Statement digest, plus the Request digest for sigma_send.
    public: Vec<F>,
    /// Pow5 permutations hashed by the relation.
    permutations: usize,
    /// Base advice length after each component, in build order.
    marks: Vec<(&'static str, usize)>,
}

/// Build one split-lineage step relation into `builder` (one parity).
///
/// With `native`, every permutation runs in the native lanes; otherwise in halo2-base gates.
fn m7_step<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    native: Option<&mut PastaNativePoseidonJobsV1<F>>,
    seed: u64,
    step: M7Step,
    layout: M7Layout,
    mutation: M7Mutation,
) -> M7Built<F> {
    let range = builder.range_chip();
    let gate = range.gate();
    let mut rng = SplitMix64(seed);
    let ctx = builder.main(0);
    let mut hasher = M7Hasher::new(ctx, &range, native);
    let mut marks = vec![("setup", ctx.advice_len())];

    // Predecessor state values (this parity's field encoding).
    let amount = u128::from(rng.next_u64() >> 8) + 1;
    let fee = match step {
        M7Step::Send => u128::from(rng.next_u64() >> 40),
        M7Step::Receive => 0,
    };
    let balance = match mutation {
        M7Mutation::Overdraft => amount + fee - 1,
        M7Mutation::Overflow => u128::MAX - amount + 1,
        _ => (1_u128 << 100) | (m6_u128(&mut rng) >> 30),
    };
    let policy_epoch = (rng.next_u64() >> 24) + 256;
    let time_floor = (rng.next_u64() >> 24) + 1;
    let mut values = vec![F::ZERO; M7_STATE_FIELDS];
    values[M7_BALANCE] = F::from_u128(balance);
    values[M7_SEQUENCE] = F::from(rng.next_u64());
    values[M7_NEXT_SEND] = F::from(rng.next_u64());
    values[M7_NEXT_LOAD] = F::from(rng.next_u64());
    values[M7_SEND_CHAIN] = rng.field();
    values[M7_RECV_CHAIN] = rng.field();
    values[M7_STATE_NONCE] = rng.field();
    values[M7_LIFECYCLE] = F::from(M7_LIFECYCLE_ACTIVE);
    values[M7_POLICY_EPOCH] = F::from(policy_epoch);
    values[M7_TIME_FLOOR] = F::from(time_floor);
    values[M7_STATE_VERSION] = F::from(M7_VERSION);
    for value in &mut values[M7_SCHEME_ID..M7_OTHER_CARRIED] {
        *value = F::from_u128(m6_u128(&mut rng));
    }
    for value in &mut values[M7_OTHER_CARRIED..] {
        *value = rng.field();
    }

    // Predecessor opening.
    let core = values[..M7_CORE_FIELDS]
        .iter()
        .map(|value| ctx.load_witness(*value))
        .collect::<Vec<_>>();
    let (rest, rest_digest, state_domain) = match layout {
        M7Layout::Flat => (
            values[M7_CORE_FIELDS..]
                .iter()
                .map(|value| ctx.load_witness(*value))
                .collect::<Vec<_>>(),
            None,
            KAGEMUSHA_STATE_DOMAIN_V1,
        ),
        M7Layout::TwoLevel => (
            Vec::new(),
            Some(ctx.load_witness(kagemusha_native_poseidon_hash::<F>(
                M7_REST_DOMAIN,
                &values[M7_CORE_FIELDS..],
            ))),
            M7_CORE_DOMAIN,
        ),
    };
    gate.assert_is_const(ctx, &core[M7_LIFECYCLE], &F::from(M7_LIFECYCLE_ACTIVE));
    let predecessor = hasher.hash(
        ctx,
        &range,
        state_domain,
        &m7_state_inputs(&core, &rest, rest_digest),
    );
    let scheme_id = m7_identity(ctx, &rest, &values, M7_SCHEME_ID);
    let asset = m7_identity(ctx, &rest, &values, M7_ASSET);
    let credential = m7_identity(ctx, &rest, &values, M7_CREDENTIAL);
    marks.push(("predecessor_open", ctx.advice_len()));

    // Checked u128 arithmetic shared by both steps.
    for index in [M7_BALANCE, M7_SEQUENCE] {
        range.range_check(ctx, core[index], M7_U128_BITS);
    }
    let amount_cell = ctx.load_witness(F::from_u128(amount));
    range.range_check(ctx, amount_cell, M7_U128_BITS);
    let amount_is_zero = gate.is_zero(ctx, amount_cell);
    gate.assert_is_const(ctx, &amount_is_zero, &F::ZERO);
    let sequence_after = gate.add(ctx, core[M7_SEQUENCE], Constant(F::ONE));
    range.range_check(ctx, sequence_after, M7_U128_BITS);
    let credit_id = m7_limbs(ctx, &mut rng);
    // Receiver wallet (sigma_send) or payer wallet (sigma_recv).
    let counterparty = m7_limbs(ctx, &mut rng);
    let mut successor = core.clone();
    successor[M7_SEQUENCE] = sequence_after;
    successor[M7_STATE_NONCE] = ctx.load_witness(rng.field());

    let mut request_digest_public = None;
    let mut effect = match step {
        M7Step::Send => {
            let fee_cell = ctx.load_witness(F::from_u128(fee));
            range.range_check(ctx, fee_cell, M7_U128_BITS);
            let debit = gate.add(ctx, amount_cell, fee_cell);
            let balance_after = gate.sub(ctx, core[M7_BALANCE], debit);
            range.range_check(ctx, balance_after, M7_U128_BITS);
            range.range_check(ctx, core[M7_NEXT_SEND], M7_U128_BITS);
            let next_send_after = gate.add(ctx, core[M7_NEXT_SEND], Constant(F::ONE));
            range.range_check(ctx, next_send_after, M7_U128_BITS);
            // Send rule: the Request's policy epoch is not newer than the payer's.
            let request_epoch = match mutation {
                M7Mutation::StaleEpoch => policy_epoch + 1,
                _ => policy_epoch - (rng.next_u64() & 0xff),
            };
            range.range_check(ctx, core[M7_POLICY_EPOCH], M7_U64_BITS);
            let request_epoch_cell = m7_u64_cell(ctx, &range, request_epoch);
            m7_assert_at_most(
                ctx,
                &range,
                request_epoch_cell,
                core[M7_POLICY_EPOCH],
                M7_U64_BITS,
            );
            // Accepted-time window: max(floor, Request time) <= lower <= upper.
            let request_time = time_floor + (rng.next_u64() & 0xffff);
            let lower = match mutation {
                M7Mutation::EarlyTime => time_floor - 1,
                _ => request_time + (rng.next_u64() & 0xffff),
            };
            let upper = lower + 600_000;
            range.range_check(ctx, core[M7_TIME_FLOOR], M7_U64_BITS);
            let request_time_cell = m7_u64_cell(ctx, &range, request_time);
            let lower_cell = m7_u64_cell(ctx, &range, lower);
            let upper_cell = m7_u64_cell(ctx, &range, upper);
            m7_assert_at_most(ctx, &range, core[M7_TIME_FLOOR], lower_cell, M7_U64_BITS);
            m7_assert_at_most(ctx, &range, request_time_cell, lower_cell, M7_U64_BITS);
            m7_assert_at_most(ctx, &range, lower_cell, upper_cell, M7_U64_BITS);
            marks.push(("arithmetic", ctx.advice_len()));

            // send_chain append of the pending-outgoing leaf fields.
            let request_digest = m7_limbs(ctx, &mut rng);
            let send_chain_entry: [AssignedValue<F>; M7_SEND_CHAIN_FIELDS] = [
                core[M7_SEND_CHAIN],
                credit_id[0],
                credit_id[1],
                counterparty[0],
                counterparty[1],
                core[M7_NEXT_SEND],
                amount_cell,
                fee_cell,
                request_digest[0],
                request_digest[1],
            ];
            let send_chain_after =
                hasher.hash(ctx, &range, M7_SEND_CHAIN_DOMAIN, &send_chain_entry);
            marks.push(("chain_append", ctx.advice_len()));

            // Request body (credit_id preimage) as field elements, tied to effect and state.
            let version = ctx.load_constant(F::from(M7_VERSION));
            let payer = m7_identity(ctx, &rest, &values, M7_WALLET_ID);
            let fee_schedule = m7_identity(ctx, &rest, &values, M7_FEE_SCHEDULE);
            let receiver_credential = m7_limbs(ctx, &mut rng);
            let scheme_policy = m7_limbs(ctx, &mut rng);
            let certificates = m7_limbs(ctx, &mut rng);
            let nonce = m7_limbs(ctx, &mut rng);
            let request: [AssignedValue<F>; M7_REQUEST_FIELDS] = [
                version,
                scheme_id[0],
                scheme_id[1],
                asset[0],
                asset[1],
                payer[0],
                payer[1],
                counterparty[0],
                counterparty[1],
                core[M7_NEXT_SEND],
                receiver_credential[0],
                receiver_credential[1],
                amount_cell,
                fee_schedule[0],
                fee_schedule[1],
                fee_cell,
                request_epoch_cell,
                scheme_policy[0],
                scheme_policy[1],
                request_time_cell,
                certificates[0],
                certificates[1],
                nonce[0],
                nonce[1],
            ];
            request_digest_public = Some(hasher.hash(ctx, &range, M7_REQUEST_DOMAIN, &request));
            marks.push(("request_binding", ctx.advice_len()));

            successor[M7_BALANCE] = balance_after;
            successor[M7_NEXT_SEND] = next_send_after;
            successor[M7_SEND_CHAIN] = send_chain_after;
            let dependencies = m7_limbs(ctx, &mut rng);
            vec![
                credit_id[0],
                credit_id[1],
                counterparty[0],
                counterparty[1],
                core[M7_NEXT_SEND],
                amount_cell,
                fee_cell,
                request_digest[0],
                request_digest[1],
                dependencies[0],
                dependencies[1],
                lower_cell,
                upper_cell,
            ]
        }
        M7Step::Receive => {
            let balance_after = gate.add(ctx, core[M7_BALANCE], amount_cell);
            range.range_check(ctx, balance_after, M7_U128_BITS);
            marks.push(("arithmetic", ctx.advice_len()));
            let recv_chain_entry: [AssignedValue<F>; M7_RECV_CHAIN_FIELDS] = [
                core[M7_RECV_CHAIN],
                credit_id[0],
                credit_id[1],
                counterparty[0],
                counterparty[1],
                amount_cell,
            ];
            let recv_chain_after =
                hasher.hash(ctx, &range, M7_RECV_CHAIN_DOMAIN, &recv_chain_entry);
            marks.push(("chain_append", ctx.advice_len()));
            successor[M7_BALANCE] = balance_after;
            successor[M7_RECV_CHAIN] = recv_chain_after;
            // Precomputed sigma_recv: the payment digest is zero; the lineage relation binds it.
            let zero = ctx.load_constant(F::ZERO);
            vec![
                credit_id[0],
                credit_id[1],
                counterparty[0],
                counterparty[1],
                zero,
                zero,
                amount_cell,
            ]
        }
    };

    // Successor commitment.
    let successor_commitment = hasher.hash(
        ctx,
        &range,
        state_domain,
        &m7_state_inputs(&successor, &rest, rest_digest),
    );
    marks.push(("successor_commit", ctx.advice_len()));

    // Poseidon digest of the G1 statement field encoding.
    let (tag, relation_id) = match step {
        M7Step::Send => (M7_EFFECT_SEND, M7_RELATION_SEND),
        M7Step::Receive => (M7_EFFECT_RECEIVE, M7_RELATION_RECEIVE),
    };
    let zero = ctx.load_constant(F::ZERO);
    effect.resize(M7_EFFECT_UNION_FIELDS, zero);
    let version = ctx.load_constant(F::from(M7_VERSION));
    let relation = [ctx.load_constant(F::from_u128(relation_id)), zero];
    // The other parity's commitment components are carried as 128-bit limb pairs.
    let predecessor_other = m7_limbs(ctx, &mut rng);
    let successor_other = m7_limbs(ctx, &mut rng);
    let tag = ctx.load_constant(F::from(tag));
    let mut statement = vec![
        version,
        scheme_id[0],
        scheme_id[1],
        relation[0],
        relation[1],
        credential[0],
        credential[1],
        asset[0],
        asset[1],
        core[M7_LIFECYCLE],
        sequence_after,
        core[M7_NEXT_LOAD],
        predecessor,
        predecessor_other[0],
        predecessor_other[1],
        successor_commitment,
        successor_other[0],
        successor_other[1],
        tag,
    ];
    statement.extend(effect);
    assert_eq!(
        statement.len(),
        M7_STATEMENT_FIELDS,
        "G1 statement encoding"
    );
    let statement_digest = hasher.hash(ctx, &range, M7_STATEMENT_DOMAIN, &statement);
    marks.push(("statement_digest", ctx.advice_len()));

    let permutations = hasher.permutations;
    assert_eq!(
        permutations,
        m7_permutations(step, layout),
        "M7 permutation inventory"
    );
    let mut public = vec![statement_digest];
    public.extend(request_digest_public);
    builder.assigned_instances[0].extend(public.iter().copied());
    M7Built {
        public: public.iter().map(|cell| *cell.value()).collect(),
        permutations,
        marks,
    }
}

/// M7 native-lane circuit parameters: Base parameters plus the lane count.
#[derive(Clone, Debug, Default)]
struct M7NativeParams {
    base: BaseCircuitParams,
    lanes: usize,
}

/// Base plus native Pasta Poseidon lane configuration.
#[derive(Clone, Debug)]
struct M7NativeConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

/// A step relation whose permutations run in the native Pasta Poseidon lanes.
#[derive(Clone)]
struct M7NativeCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
    lanes: usize,
    permutations: usize,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for M7NativeCircuit<F> {
    type Config = M7NativeConfig<F>;
    type FloorPlanner = V1;
    type Params = M7NativeParams;

    fn params(&self) -> Self::Params {
        M7NativeParams {
            base: self.base.config_params.clone(),
            lanes: self.lanes,
        }
    }

    fn without_witnesses(&self) -> Self {
        Self {
            base: self.base.deep_clone().unknown(true),
            jobs: self.jobs.clone().unknown(),
            lanes: self.lanes,
            permutations: self.permutations,
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("M7 native circuits are configured from parameters")
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.base.k) - MINIMUM_ROWS;
        let mut base = BaseConfig::configure(meta, params.base);
        base.set_usable_rows(usable_rows);
        M7NativeConfig {
            base,
            native: PastaNativePoseidonConfigV1::configure::<F>(meta, params.lanes),
        }
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let result = self.synthesize(config, layouter);
        self.base.reset_synthesis_state();
        result
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let usable_rows = (1_usize << self.base.config_params.k) - MINIMUM_ROWS;
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.base,
            config.base,
            layouter.namespace(|| "M7 Base"),
        )?;
        self.jobs.synthesize(
            &config.native,
            &mut layouter,
            &self.base.core().copy_manager,
            self.base.witness_gen_only(),
            usable_rows,
        )
    }
}

impl<F: KagemushaPoseidonFieldV1> G3MeasuredCircuit<F> for M7NativeCircuit<F> {
    fn base(&self) -> &BaseCircuitBuilder<F> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseCircuitBuilder<F> {
        &mut self.base
    }

    fn extra_fields(&self) -> String {
        let rows = self
            .jobs
            .required_rows()
            .expect("native Poseidon inventory");
        format!(
            " native_lanes={} native_permutations={} native_rows={rows} native_cells={}",
            self.lanes,
            self.permutations,
            rows * (3 * self.lanes + 1)
        )
    }
}

/// Build one native-lane step circuit into `base`.
fn m7_native_circuit<F: KagemushaPoseidonFieldV1>(
    mut base: BaseCircuitBuilder<F>,
    k: u32,
    lanes: usize,
    seed: u64,
    step: M7Step,
    layout: M7Layout,
    mutation: M7Mutation,
) -> (M7NativeCircuit<F>, Vec<F>) {
    let mut jobs = PastaNativePoseidonJobsV1::new(lanes, (1_usize << k) - MINIMUM_ROWS)
        .unwrap_or_else(|error| panic!("native Poseidon lane envelope: {error}"));
    let built = m7_step(&mut base, Some(&mut jobs), seed, step, layout, mutation);
    let circuit = M7NativeCircuit {
        base,
        jobs,
        lanes,
        permutations: built.permutations,
    };
    (circuit, built.public)
}

/// Prove and verify one step relation at `k` and print its `G3_SCALING` line.
fn m7_measure<C>(step: M7Step, backend: M7Backend, layout: M7Layout, k: u32)
where
    C: SerdeCurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
{
    let case = m7_case(step, backend, layout);
    let lookup_bits = Some(k as usize - 1);
    match backend {
        M7Backend::Base => drop(measure_impl::<C>(
            &case,
            k,
            lookup_bits,
            None,
            |builder, seed| m7_step(builder, None, seed, step, layout, M7Mutation::None).public,
        )),
        M7Backend::Native => {
            let lanes = m7_native_lanes(k, m7_permutations(step, layout))
                .unwrap_or_else(|| panic!("{case}: native lanes do not fit at k = {k}"));
            drop(measure_circuit::<C, M7NativeCircuit<C::ScalarExt>>(
                &case,
                k,
                lookup_bits,
                None,
                |base, seed| {
                    m7_native_circuit(base, k, lanes, seed, step, layout, M7Mutation::None)
                },
            ));
        }
    }
}

/// Synthesis, verifying key and exact canonical proof length of one step circuit at `k`.
///
/// Returns the public values, or `None` when the native lanes cannot hold the relation.
fn m7_inventory_line(
    params: &ParamsIPA<EqAffine>,
    step: M7Step,
    backend: M7Backend,
    layout: M7Layout,
    k: u32,
) -> Option<Vec<Fp>> {
    let case = m7_case(step, backend, layout);
    let permutations = m7_permutations(step, layout);
    let lanes = match backend {
        M7Backend::Base => 0,
        M7Backend::Native => {
            let Some(lanes) = m7_native_lanes(k, permutations) else {
                println!(
                    "M7_INVENTORY case={case} k={k} permutations={permutations} lanes=none (exceeds two native lanes)"
                );
                return None;
            };
            lanes
        }
    };
    let started = Instant::now();
    let mut base = BaseCircuitBuilder::<Fp>::new(false)
        .use_k(k as usize)
        .use_instance_columns(1);
    base.set_lookup_bits(k as usize - 1);
    let mut jobs = (lanes > 0).then(|| {
        PastaNativePoseidonJobsV1::new(lanes, (1_usize << k) - MINIMUM_ROWS)
            .expect("native Poseidon lane envelope")
    });
    let built = m7_step(
        &mut base,
        jobs.as_mut(),
        KEYGEN_SEED,
        step,
        layout,
        M7Mutation::None,
    );
    let circuit_params = base.calculate_params(Some(MINIMUM_ROWS));
    let statistics = base.statistics();
    let (vk, native_rows) = match jobs {
        None => (
            keygen_vk_custom(params, &base, true).expect("M7 Base verifying key"),
            0,
        ),
        Some(jobs) => {
            let rows = jobs.required_rows().expect("native Poseidon inventory");
            let circuit = M7NativeCircuit {
                base,
                jobs,
                lanes,
                permutations,
            };
            (
                keygen_vk_custom(params, &circuit, true).expect("M7 native verifying key"),
                rows,
            )
        }
    };
    let protocol = compile(
        params,
        &vk,
        Config::ipa().with_num_instance(vec![built.public.len()]),
    );
    let proof_bytes = canonical_ordinary_proof_bytes(k, &protocol);
    let cs = vk.cs();
    let components = built
        .marks
        .windows(2)
        .map(|pair| format!("{}={}", pair[1].0, pair[1].1 - pair[0].1))
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "M7_INVENTORY case={case} k={k} lanes={lanes} permutations={} base_advice_cells={} lookup_cells={} advice_cols={} lookup_cols={} total_advice_cols={} fixed_cols={} degree={} native_rows={native_rows} native_cells={} proof_bytes={proof_bytes} meets_size_gate={} build_vk_ms={:.0} cells_setup={} {components}",
        built.permutations,
        statistics.gate.total_advice_per_phase[0],
        statistics.total_lookup_advice_per_phase[0],
        circuit_params.num_advice_per_phase[0],
        circuit_params.num_lookup_advice_per_phase[0],
        cs.num_advice_columns(),
        cs.num_fixed_columns(),
        cs.degree(),
        native_rows * (3 * lanes + 1),
        proof_bytes <= M7_PROOF_BYTES_GATE,
        elapsed_ms(started),
        built.marks[0].1,
    );
    Some(built.public)
}

/// Measurement harness (M7): exact cells, columns, permutations and canonical proof bytes of
/// every step circuit for `k` = 11..=18 (Base gates from 13, native lanes up to 16), plus a
/// cross-check that both Poseidon backends produce identical public digests.
#[test]
#[ignore = "M7 split-lineage measurement harness; run explicitly in release"]
fn m7_inventory() {
    let params = (11..=18_u32)
        .map(|k| (k, ParamsIPA::<EqAffine>::new(k)))
        .collect::<BTreeMap<_, _>>();
    for step in [M7Step::Send, M7Step::Receive] {
        for layout in [M7Layout::Flat, M7Layout::TwoLevel] {
            for (&k, params) in &params {
                let base = (k >= 13)
                    .then(|| m7_inventory_line(params, step, M7Backend::Base, layout, k))
                    .flatten();
                let native = (k <= M7_NATIVE_MAX_K)
                    .then(|| m7_inventory_line(params, step, M7Backend::Native, layout, k))
                    .flatten();
                if let (Some(base), Some(native)) = (base, native) {
                    assert_eq!(
                        base, native,
                        "both Poseidon backends compute the same digests"
                    );
                }
            }
        }
    }
}

/// MockProver acceptance of one step relation witness at `k`.
fn m7_mock_accepts(
    step: M7Step,
    backend: M7Backend,
    layout: M7Layout,
    mutation: M7Mutation,
    k: u32,
) -> bool {
    let mut base = BaseCircuitBuilder::<Fp>::new(false)
        .use_k(k as usize)
        .use_instance_columns(1);
    base.set_lookup_bits(k as usize - 1);
    match backend {
        M7Backend::Base => {
            let built = m7_step(&mut base, None, PROVER_SEED, step, layout, mutation);
            base.calculate_params(Some(MINIMUM_ROWS));
            MockProver::run(k, &base, vec![built.public])
                .expect("M7 Base mock synthesis")
                .verify()
                .is_ok()
        }
        M7Backend::Native => {
            let lanes = m7_native_lanes(k, m7_permutations(step, layout))
                .expect("native lanes fit the check k");
            let (mut circuit, public) =
                m7_native_circuit(base, k, lanes, PROVER_SEED, step, layout, mutation);
            circuit.base.calculate_params(Some(MINIMUM_ROWS));
            MockProver::run(k, &circuit, vec![public])
                .expect("M7 native mock synthesis")
                .verify()
                .is_ok()
        }
    }
}

/// Measurement harness (M7): each step relation accepts an honest witness and rejects an
/// overdraft, a u128 overflow, a newer Request policy epoch and an early accepted time, in
/// both Poseidon backends and both state layouts (MockProver at `k = 16`).
#[test]
#[ignore = "M7 split-lineage measurement harness; run explicitly in release"]
fn m7_relation_checks() {
    const CHECK_K: u32 = 16;
    let cases = [
        (M7Step::Send, M7Mutation::None, true),
        (M7Step::Send, M7Mutation::Overdraft, false),
        (M7Step::Send, M7Mutation::StaleEpoch, false),
        (M7Step::Send, M7Mutation::EarlyTime, false),
        (M7Step::Receive, M7Mutation::None, true),
        (M7Step::Receive, M7Mutation::Overflow, false),
    ];
    for backend in [M7Backend::Base, M7Backend::Native] {
        for layout in [M7Layout::Flat, M7Layout::TwoLevel] {
            for (step, mutation, expected) in cases {
                let accepted = m7_mock_accepts(step, backend, layout, mutation, CHECK_K);
                println!(
                    "M7_CHECK case={} k={CHECK_K} mutation={mutation:?} accepted={accepted} expected={expected}",
                    m7_case(step, backend, layout)
                );
                assert_eq!(accepted, expected, "{mutation:?} acceptance");
            }
        }
    }
}

macro_rules! m7_step_tests {
    ($($name:ident: $step:ident, $backend:ident, $layout:ident, $k:expr;)+) => {$(
        /// Measurement harness (M7): one split-lineage step relation on the Eq side;
        /// proves, verifies and prints one `G3_SCALING` line.
        #[test]
        #[ignore = "M7 split-lineage measurement harness; run explicitly in release"]
        fn $name() {
            m7_measure::<EqAffine>(M7Step::$step, M7Backend::$backend, M7Layout::$layout, $k);
        }
    )+};
}

m7_step_tests! {
    m7_send_base_flat_k14: Send, Base, Flat, 14;
    m7_send_base_flat_k15: Send, Base, Flat, 15;
    m7_send_base_flat_k16: Send, Base, Flat, 16;
    m7_send_base_flat_k17: Send, Base, Flat, 17;
    m7_send_base_flat_k18: Send, Base, Flat, 18;
    m7_send_base_two_level_k14: Send, Base, TwoLevel, 14;
    m7_send_base_two_level_k15: Send, Base, TwoLevel, 15;
    m7_send_base_two_level_k16: Send, Base, TwoLevel, 16;
    m7_send_base_two_level_k17: Send, Base, TwoLevel, 17;
    m7_send_base_two_level_k18: Send, Base, TwoLevel, 18;
    m7_recv_base_flat_k14: Receive, Base, Flat, 14;
    m7_recv_base_flat_k15: Receive, Base, Flat, 15;
    m7_recv_base_flat_k16: Receive, Base, Flat, 16;
    m7_recv_base_flat_k17: Receive, Base, Flat, 17;
    m7_recv_base_flat_k18: Receive, Base, Flat, 18;
    m7_recv_base_two_level_k14: Receive, Base, TwoLevel, 14;
    m7_recv_base_two_level_k15: Receive, Base, TwoLevel, 15;
    m7_recv_base_two_level_k16: Receive, Base, TwoLevel, 16;
    m7_recv_base_two_level_k17: Receive, Base, TwoLevel, 17;
    m7_recv_base_two_level_k18: Receive, Base, TwoLevel, 18;
    m7_send_native_flat_k11: Send, Native, Flat, 11;
    m7_send_native_flat_k12: Send, Native, Flat, 12;
    m7_send_native_flat_k13: Send, Native, Flat, 13;
    m7_send_native_flat_k14: Send, Native, Flat, 14;
    m7_send_native_flat_k15: Send, Native, Flat, 15;
    m7_send_native_flat_k16: Send, Native, Flat, 16;
    m7_send_native_two_level_k11: Send, Native, TwoLevel, 11;
    m7_send_native_two_level_k12: Send, Native, TwoLevel, 12;
    m7_send_native_two_level_k13: Send, Native, TwoLevel, 13;
    m7_send_native_two_level_k14: Send, Native, TwoLevel, 14;
    m7_send_native_two_level_k15: Send, Native, TwoLevel, 15;
    m7_send_native_two_level_k16: Send, Native, TwoLevel, 16;
    m7_recv_native_flat_k11: Receive, Native, Flat, 11;
    m7_recv_native_flat_k12: Receive, Native, Flat, 12;
    m7_recv_native_flat_k13: Receive, Native, Flat, 13;
    m7_recv_native_flat_k14: Receive, Native, Flat, 14;
    m7_recv_native_flat_k15: Receive, Native, Flat, 15;
    m7_recv_native_flat_k16: Receive, Native, Flat, 16;
    m7_recv_native_two_level_k11: Receive, Native, TwoLevel, 11;
    m7_recv_native_two_level_k12: Receive, Native, TwoLevel, 12;
    m7_recv_native_two_level_k13: Receive, Native, TwoLevel, 13;
    m7_recv_native_two_level_k14: Receive, Native, TwoLevel, 14;
    m7_recv_native_two_level_k15: Receive, Native, TwoLevel, 15;
    m7_recv_native_two_level_k16: Receive, Native, TwoLevel, 16;
}

// ---------------------------------------------------------------------------
// M8: custom PLONKish gates (raw halo2-axiom `Circuit` API) versus halo2-base.
//
// Each building block is proved for real in the KAGEMUSHA IPA proof format on the Eq side
// (circuit field Fp) and compared at equal work:
//
// 1. Poseidon (width 3, rate 2, RF 8 / RP 57: the KAGEMUSHA V1 Pasta parameters). N raw sponge
//    permutations in halo2-base gates (`PoseidonHasher`), in the repository native lanes
//    (`pasta_native_poseidon.rs`) and in a custom Pow5-style chip: three state columns plus one
//    auxiliary column per lane, two partial rounds per row, absorption fused into the first full
//    round, 37 rows per permutation, degree 6.
// 2. Checked u128 arithmetic. N chained `acc' = acc +/- x` operations with `x` and `acc'`
//    range-checked to 128 bits: halo2-base `RangeChip` (lookup bits k - 1) versus a custom
//    running-sum chip (one advice column and one lookup argument per lane, (k - 1)-bit limbs, a
//    shifted top-limb check and an add/sub gate that reaches back by rotation).
// 3. Native-curve variable-base scalar multiplication on the Pasta curve whose base field is the
//    circuit field (Pallas in Fp): halo2-ecc `scalar_mult` (4-bit windows over the M6 native
//    field chip) versus a custom double-and-add gate (Orchard-style incomplete addition, one row
//    per scalar bit, seven columns, plus one load/on-curve/doubling row).
//
// The custom chips are measurement prototypes. The Poseidon and u128 chips constrain their
// complete relation. The scalar multiplication omits exceptional-case handling (witness
// generation asserts that none occurs), the scalar canonicity check and the in-circuit scalar
// transform `t = (s - 2^255 - 1) / 2`.
// ---------------------------------------------------------------------------

/// M8 custom-gate prototypes and their halo2-base and native-lane counterparts.
mod m8 {
    use halo2_base::{
        gates::flex_gate::threads::SinglePhaseCoreManager,
        poseidon::hasher::spec::OptimizedPoseidonSpec,
        utils::{biguint_to_fe, fe_to_biguint},
    };
    use halo2_proofs::{
        circuit::{SimpleFloorPlanner, Value},
        plonk::{Advice, Column, Expression, Fixed, Instance, Selector, TableColumn, keygen_pk2},
        poly::Rotation,
    };

    use super::*;

    /// Permutations of the N = 1,000 Poseidon cases.
    const POSEIDON_PERMUTATIONS: usize = 1_000;
    /// Checked add/sub operations of the u128 cases (two 128-bit range checks each).
    const U128_OPERATIONS: usize = 1_000;
    /// Variable-base scalar multiplications of the curve cases.
    const SCALAR_MULS: usize = 16;
    const WIDTH: usize = 3;
    const FULL_ROUNDS: usize = 8;
    const PARTIAL_ROUNDS: usize = 57;
    const ROUNDS: usize = FULL_ROUNDS + PARTIAL_ROUNDS;
    const HALF_FULL: usize = FULL_ROUNDS / 2;
    /// Partial rounds handled two per row; the 57th partial round has its own row.
    const PARTIAL_PAIRS: usize = PARTIAL_ROUNDS / 2;
    /// Rows per permutation of the custom chip: 4 full + 28 pair + 1 partial + 4 full rows.
    const POSEIDON_ROWS: usize = FULL_ROUNDS + PARTIAL_PAIRS + 1;
    /// Native lane rows per permutation (`pasta_native_poseidon.rs`).
    const NATIVE_LANE_ROWS: usize = 66;
    /// Scalar bits consumed by the custom double-and-add chain.
    const SCALAR_BITS: usize = 255;
    /// Rows per custom scalar multiplication: load/double row, one row per bit, result row.
    const MUL_ROWS: usize = SCALAR_BITS + 2;
    const MUL_COLUMNS: usize = 8;
    const XA: usize = 0;
    const YA: usize = 1;
    const L1: usize = 2;
    const L2: usize = 3;
    const Z: usize = 4;
    const XP: usize = 5;
    const YP: usize = 6;
    /// Equality-enabled input/output column: `x_P`, `y_P`, then `z`, `y_A`, `x_A` of the result.
    const IO: usize = 7;

    fn witness<F: KagemushaPoseidonFieldV1>(known: bool, value: F) -> Value<F> {
        if known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }

    fn constant<F: KagemushaPoseidonFieldV1>(value: F) -> Expression<F> {
        Expression::Constant(value)
    }

    fn pow5<F: KagemushaPoseidonFieldV1>(value: F) -> F {
        value.square().square() * value
    }

    fn pow5_expression<F: KagemushaPoseidonFieldV1>(value: Expression<F>) -> Expression<F> {
        let square = value.clone() * value.clone();
        square.clone() * square * value
    }

    /// `sum_j row[j] * values[j]` for one MDS row.
    fn linear<F: KagemushaPoseidonFieldV1>(
        row: &[F; WIDTH],
        values: &[Expression<F>; WIDTH],
    ) -> Expression<F> {
        values
            .iter()
            .zip(row)
            .map(|(value, coefficient)| value.clone() * constant(*coefficient))
            .reduce(|sum, term| sum + term)
            .expect("Poseidon width is nonzero")
    }

    /// Parameters reused across runs when `G3_PARAMS_CACHE_DIR` names a directory.
    ///
    /// `ParamsIPA::new` is deterministic, so a cached copy is the identical SRS; caching only
    /// removes the parameter generation (about 75 s at k = 16 on one thread) from repeated runs.
    fn cached_params<C>(k: u32) -> ParamsIPA<C>
    where
        C: SerdeCurveAffine,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        let Some(dir) = std::env::var_os("G3_PARAMS_CACHE_DIR") else {
            return ParamsIPA::new(k);
        };
        let parity = if <C::ScalarExt as KagemushaPoseidonFieldV1>::IS_EQ_PARITY {
            "eq"
        } else {
            "ep"
        };
        let path = std::path::Path::new(&dir).join(format!("{parity}_k{k}.params"));
        if let Ok(file) = std::fs::File::open(&path) {
            let params = ParamsIPA::<C>::read(&mut io::BufReader::new(file))
                .unwrap_or_else(|error| panic!("cached params {}: {error}", path.display()));
            assert_eq!(params.k(), k, "cached params have the wrong k");
            return params;
        }
        let params = ParamsIPA::<C>::new(k);
        let temporary = path.with_extension(format!("tmp{}", std::process::id()));
        {
            let mut writer = io::BufWriter::new(
                std::fs::File::create(&temporary).expect("create cached params file"),
            );
            params.write(&mut writer).expect("write cached params");
            writer.flush().expect("flush cached params");
        }
        std::fs::rename(&temporary, &path).expect("publish cached params");
        params
    }

    /// A raw custom-gate circuit measured by [`measure_raw`].
    trait RawCircuit<F: KagemushaPoseidonFieldV1>: Circuit<F> {
        /// Rows occupied by the layout (excluding blinding rows).
        fn used_rows(&self) -> usize;
        /// Extra ` key=value` fields for the `G3_SCALING` line.
        fn layout_fields(&self) -> String;
    }

    /// Prove and verify one raw circuit exactly as `measure_circuit` does for Base circuits.
    fn measure_raw<C, Circ>(case: &str, k: u32, make: impl Fn(u64) -> (Circ, Vec<C::ScalarExt>))
    where
        C: SerdeCurveAffine,
        C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3> + SerdePrimeField,
        Circ: RawCircuit<C::ScalarExt>,
    {
        let field = if <C::ScalarExt as KagemushaPoseidonFieldV1>::IS_EQ_PARITY {
            "eq"
        } else {
            "ep"
        };
        let threads = std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".to_owned());
        let rss = RssSampler::start();

        rss.enter(PHASE_PARAMS);
        let started = Instant::now();
        let params = cached_params::<C>(k);
        let params_ms = elapsed_ms(started);

        rss.enter(PHASE_KEYGEN);
        let started = Instant::now();
        let keygen_cpu_started = process_cpu_ms();
        let (keygen_circuit, keygen_public) = make(KEYGEN_SEED);
        let keygen_build_ms = elapsed_ms(started);
        let used_rows = keygen_circuit.used_rows();
        let layout = keygen_circuit.layout_fields();
        let pk = keygen_pk2(&params, &keygen_circuit, true)
            .unwrap_or_else(|error| panic!("{case}: keygen failed: {error}"));
        drop(keygen_circuit);
        halo2_proofs::release_allocator_slack();
        let keygen_ms = elapsed_ms(started);
        let keygen_cpu_ms = process_cpu_ms() - keygen_cpu_started;

        rss.enter(PHASE_PROVE);
        let started = Instant::now();
        let prove_cpu_started = process_cpu_ms();
        let (prover, public) = make(PROVER_SEED);
        assert_eq!(public.len(), keygen_public.len(), "{case}: instance shape");
        let witness_ms = elapsed_ms(started);
        let started = Instant::now();
        let raw = {
            let columns: [&[C::ScalarExt]; 1] = [&public];
            let instances: [&[&[C::ScalarExt]]; 1] = [&columns];
            let mut transcript = KagemushaTranscript::<C, _>::new::<
                KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
            >(Vec::<u8>::new());
            create_proof::<IPACommitmentScheme<C>, ProverIPA<'_, C>, ChallengeScalar<C>, _, _, _>(
                &params,
                &pk,
                &[prover],
                &instances,
                OsRng,
                &mut transcript,
            )
            .unwrap_or_else(|error| panic!("{case}: create_proof failed: {error}"));
            transcript.finalize()
        };
        halo2_proofs::release_allocator_slack();
        let create_proof_ms = elapsed_ms(started);
        let started = Instant::now();
        let folded_generator = verified_folded_generator(&params, pk.get_vk(), &raw, &public)
            .unwrap_or_else(|error| panic!("{case}: folded-generator derivation failed: {error}"));
        let mut proof = raw.clone();
        proof.extend_from_slice(folded_generator.to_bytes().as_ref());
        let augment_ms = elapsed_ms(started);
        let prove_ms = witness_ms + create_proof_ms + augment_ms;
        let prove_cpu_ms = process_cpu_ms() - prove_cpu_started;

        rss.enter(PHASE_VERIFY);
        let started = Instant::now();
        let verify_cpu_started = process_cpu_ms();
        let (raw_part, generator_part) = proof.split_at(proof.len() - 32);
        let verified = verified_folded_generator(&params, pk.get_vk(), raw_part, &public)
            .unwrap_or_else(|error| panic!("{case}: verification failed: {error}"));
        let verified_ok = verified.to_bytes().as_ref() == generator_part;
        let verify_ms = elapsed_ms(started);
        let verify_cpu_ms = process_cpu_ms() - verify_cpu_started;
        let mut wrong_public = public.clone();
        wrong_public[0] += C::ScalarExt::ONE;
        let wrong_public_rejected =
            verified_folded_generator(&params, pk.get_vk(), raw_part, &wrong_public).is_err();
        let peaks = rss.finish();

        let vk = pk.get_vk();
        let cs = vk.cs();
        let mut counter = ByteCounter::default();
        pk.write(&mut counter, SerdeFormat::Processed)
            .expect("count proving-key bytes");
        let pk_bytes = counter.0;
        let mut counter = ByteCounter::default();
        vk.write(&mut counter, SerdeFormat::Processed)
            .expect("count verifying-key bytes");
        let vk_bytes = counter.0;
        let protocol = compile(
            &params,
            vk,
            Config::ipa().with_num_instance(vec![public.len()]),
        );
        let expected_proof_bytes = canonical_ordinary_proof_bytes(k, &protocol);
        let advice_cols = cs.num_advice_columns();
        let cells = used_rows * advice_cols;
        println!(
            "G3_SCALING k={k} field={field} advice_cols={advice_cols} lookup_cols=0 cells={cells} keygen_ms={keygen_ms:.0} pk_bytes={pk_bytes} vk_bytes={vk_bytes} prove_ms={prove_ms:.0} verify_ms={verify_ms:.1} proof_bytes={} case={case} threads={threads} lookup_cells=0 lookup_args={} fixed_cols={} perm_cols={} degree={} quotient_chunks={} instances={} raw_proof_bytes={} expected_proof_bytes={expected_proof_bytes} params_ms={params_ms:.0} keygen_build_ms={keygen_build_ms:.0} witness_ms={witness_ms:.0} create_proof_ms={create_proof_ms:.0} augment_ms={augment_ms:.0} rss_mib_params={} rss_mib_keygen={} rss_mib_prove={} rss_mib_verify={} keygen_cpu_ms={keygen_cpu_ms:.0} prove_cpu_ms={prove_cpu_ms:.0} verify_cpu_ms={verify_cpu_ms:.1} total_advice_cols={advice_cols} design=custom used_rows={used_rows} minimum_rows={}{layout}",
            proof.len(),
            cs.lookups().len(),
            cs.num_fixed_columns(),
            cs.permutation().get_columns().len(),
            cs.degree(),
            protocol.quotient.num_chunk(),
            public.len(),
            raw.len(),
            mib(peaks[PHASE_PARAMS]),
            mib(peaks[PHASE_KEYGEN]),
            mib(peaks[PHASE_PROVE]),
            mib(peaks[PHASE_VERIFY]),
            cs.minimum_rows(),
        );
        assert!(verified_ok, "{case}: appended folded generator mismatch");
        assert!(wrong_public_rejected, "{case}: wrong public input accepted");
        assert_eq!(
            proof.len(),
            expected_proof_bytes,
            "{case}: proof is not the canonical KAGEMUSHA ordinary length"
        );
    }

    // -----------------------------------------------------------------------
    // (1) Poseidon.
    // -----------------------------------------------------------------------

    /// Unoptimized KAGEMUSHA V1 Pasta Poseidon constants and the MDS inverse.
    #[derive(Clone, Debug)]
    struct PoseidonSpec<F> {
        constants: Vec<[F; WIDTH]>,
        mds: [[F; WIDTH]; WIDTH],
        mds_inverse: [[F; WIDTH]; WIDTH],
    }

    fn full_round(round: usize) -> bool {
        round < HALF_FULL || round >= HALF_FULL + PARTIAL_ROUNDS
    }

    /// Inverse of a 3x3 matrix by cyclic cofactors, checked against the identity.
    fn invert_3x3<F: KagemushaPoseidonFieldV1>(matrix: [[F; WIDTH]; WIDTH]) -> [[F; WIDTH]; WIDTH] {
        let cofactor = |row: usize, column: usize| {
            let (r1, r2) = ((row + 1) % WIDTH, (row + 2) % WIDTH);
            let (c1, c2) = ((column + 1) % WIDTH, (column + 2) % WIDTH);
            matrix[r1][c1] * matrix[r2][c2] - matrix[r1][c2] * matrix[r2][c1]
        };
        let determinant = (0..WIDTH).fold(F::ZERO, |sum, column| {
            sum + matrix[0][column] * cofactor(0, column)
        });
        let inverse_determinant =
            Option::<F>::from(determinant.invert()).expect("Poseidon MDS is invertible");
        let inverse: [[F; WIDTH]; WIDTH] =
            std::array::from_fn(|i| std::array::from_fn(|j| cofactor(j, i) * inverse_determinant));
        for (i, row) in matrix.iter().enumerate() {
            for j in 0..WIDTH {
                let product = (0..WIDTH).fold(F::ZERO, |sum, l| sum + row[l] * inverse[l][j]);
                assert_eq!(
                    product,
                    if i == j { F::ONE } else { F::ZERO },
                    "MDS inverse"
                );
            }
        }
        inverse
    }

    impl<F: KagemushaPoseidonFieldV1> PoseidonSpec<F> {
        fn new() -> Self {
            let (constants, mds) = OptimizedPoseidonSpec::<F, WIDTH, 2>::unoptimized_constants::<
                FULL_ROUNDS,
                PARTIAL_ROUNDS,
                0,
            >();
            assert_eq!(constants.len(), ROUNDS, "fixed Pasta round count");
            Self {
                mds_inverse: invert_3x3(mds),
                constants,
                mds,
            }
        }

        fn round(&self, state: [F; WIDTH], round: usize) -> [F; WIDTH] {
            let mut powered: [F; WIDTH] =
                std::array::from_fn(|i| state[i] + self.constants[round][i]);
            for (i, value) in powered.iter_mut().enumerate() {
                if full_round(round) || i == 0 {
                    *value = pow5(*value);
                }
            }
            std::array::from_fn(|i| {
                (0..WIDTH).fold(F::ZERO, |sum, j| sum + self.mds[i][j] * powered[j])
            })
        }
    }

    /// One lane's precomputed trace: state rows, the auxiliary column and the digest.
    #[derive(Clone, Debug)]
    struct PoseidonLaneTrace<F> {
        state: Vec<[F; WIDTH]>,
        aux: Vec<F>,
        output: F,
    }

    /// Trace of the raw KAGEMUSHA sponge over `inputs` (one permutation per two padded inputs).
    fn poseidon_lane_trace<F: KagemushaPoseidonFieldV1>(
        spec: &PoseidonSpec<F>,
        inputs: &[F],
    ) -> PoseidonLaneTrace<F> {
        let mut padded = inputs.to_vec();
        padded.push(F::ONE);
        if padded.len() % 2 != 0 {
            padded.push(F::ZERO);
        }
        let permutations = padded.len() / 2;
        let rows = POSEIDON_ROWS * permutations + 1;
        let mut state_rows = Vec::with_capacity(rows);
        let mut aux = vec![F::ZERO; rows];
        let mut state = [F::from_u128(1_u128 << 64), F::ZERO, F::ZERO];
        for (permutation, chunk) in padded.chunks_exact(2).enumerate() {
            let base = POSEIDON_ROWS * permutation;
            state_rows.push(state);
            aux[base] = chunk[0];
            aux[base + 1] = chunk[1];
            state[1] += chunk[0];
            state[2] += chunk[1];
            for round in 0..HALF_FULL {
                state = spec.round(state, round);
                state_rows.push(state);
            }
            for pair in 0..PARTIAL_PAIRS {
                let first = HALF_FULL + 2 * pair;
                aux[base + HALF_FULL + pair] = pow5(state[0] + spec.constants[first][0]);
                state = spec.round(state, first);
                state = spec.round(state, first + 1);
                state_rows.push(state);
            }
            state = spec.round(state, HALF_FULL + PARTIAL_ROUNDS - 1);
            state_rows.push(state);
            for round in HALF_FULL + PARTIAL_ROUNDS..ROUNDS {
                state = spec.round(state, round);
                if round + 1 < ROUNDS {
                    state_rows.push(state);
                }
            }
        }
        state_rows.push(state);
        aux[rows - 1] = state[1];
        assert_eq!(state_rows.len(), rows, "custom Poseidon trace rows");
        assert_eq!(
            state[1],
            F::kagemusha_poseidon_hash_v1(inputs),
            "custom Pow5 sponge equals the native KAGEMUSHA Poseidon"
        );
        PoseidonLaneTrace {
            state: state_rows,
            aux,
            output: state[1],
        }
    }

    /// Gate active on one row of a permutation, with its round constants.
    #[derive(Clone, Copy, Debug)]
    enum PoseidonRow {
        Absorb,
        Full(usize),
        Pair(usize),
        Partial(usize),
    }

    fn poseidon_row(offset: usize) -> PoseidonRow {
        match offset {
            0 => PoseidonRow::Absorb,
            offset if offset < HALF_FULL => PoseidonRow::Full(offset),
            offset if offset < HALF_FULL + PARTIAL_PAIRS => {
                PoseidonRow::Pair(HALF_FULL + 2 * (offset - HALF_FULL))
            }
            offset if offset == HALF_FULL + PARTIAL_PAIRS => {
                PoseidonRow::Partial(HALF_FULL + PARTIAL_ROUNDS - 1)
            }
            offset => PoseidonRow::Full(
                offset - HALF_FULL - PARTIAL_PAIRS - 1 + HALF_FULL + PARTIAL_ROUNDS,
            ),
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct PoseidonParams {
        lanes: usize,
    }

    #[derive(Clone, Debug)]
    struct PoseidonConfig {
        lanes: Vec<[Column<Advice>; WIDTH + 1]>,
        round_a: [Column<Fixed>; WIDTH],
        round_b: [Column<Fixed>; WIDTH],
        q_init: Selector,
        q_absorb: Selector,
        q_full: Selector,
        q_pair: Selector,
        q_partial: Selector,
        q_out: Selector,
        instance: Column<Instance>,
    }

    /// Custom Pow5 Poseidon lanes, each one raw sponge over `2n - 1` inputs (n permutations).
    #[derive(Clone, Debug)]
    struct PoseidonCircuit<F: KagemushaPoseidonFieldV1> {
        spec: PoseidonSpec<F>,
        lanes: Vec<PoseidonLaneTrace<F>>,
        permutations_per_lane: usize,
        known: bool,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for PoseidonCircuit<F> {
        type Config = PoseidonConfig;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = PoseidonParams;

        fn without_witnesses(&self) -> Self {
            Self {
                known: false,
                ..self.clone()
            }
        }

        fn params(&self) -> Self::Params {
            PoseidonParams {
                lanes: self.lanes.len(),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("M8 circuits are configured from parameters")
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let spec = PoseidonSpec::<F>::new();
            let lanes = (0..params.lanes)
                .map(|_| std::array::from_fn(|_| meta.advice_column()))
                .collect::<Vec<[Column<Advice>; WIDTH + 1]>>();
            let round_a = std::array::from_fn(|_| meta.fixed_column());
            let round_b = std::array::from_fn(|_| meta.fixed_column());
            let q_init = meta.selector();
            let q_absorb = meta.selector();
            let q_full = meta.selector();
            let q_pair = meta.selector();
            let q_partial = meta.selector();
            let q_out = meta.selector();
            let instance = meta.instance_column();
            meta.enable_equality(instance);
            let (mds, inverse) = (spec.mds, spec.mds_inverse);
            for lane in &lanes {
                let lane = *lane;
                // Inputs arrive and the digest leaves through copy constraints on the auxiliary
                // column only; the final row copies the digest `s1` into it.
                meta.enable_equality(lane[WIDTH]);
                meta.create_gate("M8 Poseidon digest output", |meta| {
                    let q = meta.query_selector(q_out);
                    vec![
                        q * (meta.query_advice(lane[WIDTH], Rotation::cur())
                            - meta.query_advice(lane[1], Rotation::cur())),
                    ]
                });
                meta.create_gate("M8 Poseidon initial sponge state", |meta| {
                    let q = meta.query_selector(q_init);
                    let s: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::cur()));
                    let [s0, s1, s2] = s;
                    vec![
                        q.clone() * (s0 - constant(F::from_u128(1_u128 << 64))),
                        q.clone() * s1,
                        q * s2,
                    ]
                });
                for (name, selector, absorb) in [
                    ("M8 Poseidon absorb and full round", q_absorb, true),
                    ("M8 Poseidon full round", q_full, false),
                ] {
                    meta.create_gate(name, |meta| {
                        let q = meta.query_selector(selector);
                        let cur: [Expression<F>; WIDTH] =
                            std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::cur()));
                        let next: [Expression<F>; WIDTH] =
                            std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::next()));
                        let a: [Expression<F>; WIDTH] =
                            std::array::from_fn(|i| meta.query_fixed(round_a[i], Rotation::cur()));
                        let mut input = cur.clone();
                        if absorb {
                            input[1] =
                                input[1].clone() + meta.query_advice(lane[WIDTH], Rotation::cur());
                            input[2] =
                                input[2].clone() + meta.query_advice(lane[WIDTH], Rotation::next());
                        }
                        let sbox: [Expression<F>; WIDTH] = std::array::from_fn(|j| {
                            pow5_expression(input[j].clone() + a[j].clone())
                        });
                        (0..WIDTH)
                            .map(|i| q.clone() * (next[i].clone() - linear(&mds[i], &sbox)))
                            .collect::<Vec<_>>()
                    });
                }
                meta.create_gate("M8 Poseidon two partial rounds", |meta| {
                    let q = meta.query_selector(q_pair);
                    let cur: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::cur()));
                    let next: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::next()));
                    let a: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_fixed(round_a[i], Rotation::cur()));
                    let b: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_fixed(round_b[i], Rotation::cur()));
                    let mid = meta.query_advice(lane[WIDTH], Rotation::cur());
                    let first = [
                        mid.clone(),
                        cur[1].clone() + a[1].clone(),
                        cur[2].clone() + a[2].clone(),
                    ];
                    // State before the second round's S-box, and the same quantity recovered
                    // from the next row through the inverse MDS matrix.
                    let second: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| linear(&mds[i], &first) + b[i].clone());
                    let recovered: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| linear(&inverse[i], &next));
                    vec![
                        q.clone() * (mid - pow5_expression(cur[0].clone() + a[0].clone())),
                        q.clone() * (recovered[0].clone() - pow5_expression(second[0].clone())),
                        q.clone() * (recovered[1].clone() - second[1].clone()),
                        q * (recovered[2].clone() - second[2].clone()),
                    ]
                });
                meta.create_gate("M8 Poseidon single partial round", |meta| {
                    let q = meta.query_selector(q_partial);
                    let cur: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::cur()));
                    let next: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_advice(lane[i], Rotation::next()));
                    let a: [Expression<F>; WIDTH] =
                        std::array::from_fn(|i| meta.query_fixed(round_a[i], Rotation::cur()));
                    let sbox = [
                        pow5_expression(cur[0].clone() + a[0].clone()),
                        cur[1].clone() + a[1].clone(),
                        cur[2].clone() + a[2].clone(),
                    ];
                    (0..WIDTH)
                        .map(|i| q.clone() * (next[i].clone() - linear(&mds[i], &sbox)))
                        .collect::<Vec<_>>()
                });
            }
            PoseidonConfig {
                lanes,
                round_a,
                round_b,
                q_init,
                q_absorb,
                q_full,
                q_pair,
                q_partial,
                q_out,
                instance,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            let rows = POSEIDON_ROWS * self.permutations_per_lane + 1;
            let outputs = layouter.assign_region(
                || "M8 Poseidon lanes",
                |mut region| {
                    for permutation in 0..self.permutations_per_lane {
                        let base = POSEIDON_ROWS * permutation;
                        if permutation == 0 {
                            config.q_init.enable(&mut region, base)?;
                        }
                        for offset in 0..POSEIDON_ROWS {
                            let row = base + offset;
                            let (selector, first, second) = match poseidon_row(offset) {
                                PoseidonRow::Absorb => (config.q_absorb, 0, None),
                                PoseidonRow::Full(round) => (config.q_full, round, None),
                                PoseidonRow::Pair(round) => (config.q_pair, round, Some(round + 1)),
                                PoseidonRow::Partial(round) => (config.q_partial, round, None),
                            };
                            selector.enable(&mut region, row)?;
                            for i in 0..WIDTH {
                                region.assign_fixed(
                                    config.round_a[i],
                                    row,
                                    self.spec.constants[first][i],
                                );
                            }
                            if let Some(second) = second {
                                for i in 0..WIDTH {
                                    region.assign_fixed(
                                        config.round_b[i],
                                        row,
                                        self.spec.constants[second][i],
                                    );
                                }
                            }
                        }
                    }
                    config.q_out.enable(&mut region, rows - 1)?;
                    let mut outputs = Vec::with_capacity(self.lanes.len());
                    for (columns, trace) in config.lanes.iter().zip(&self.lanes) {
                        assert_eq!(trace.state.len(), rows, "lane trace length");
                        let mut output = None;
                        for row in 0..rows {
                            for i in 0..WIDTH {
                                region.assign_advice_discarding_value(
                                    columns[i],
                                    row,
                                    witness(self.known, trace.state[row][i]),
                                );
                            }
                            let cell = region.assign_advice_discarding_value(
                                columns[WIDTH],
                                row,
                                witness(self.known, trace.aux[row]),
                            );
                            if row + 1 == rows {
                                output = Some(cell);
                            }
                        }
                        outputs.push(output.expect("final lane state"));
                    }
                    Ok(outputs)
                },
            )?;
            for (index, cell) in outputs.into_iter().enumerate() {
                layouter.constrain_instance(cell, config.instance, index);
            }
            Ok(())
        }
    }

    impl<F: KagemushaPoseidonFieldV1> RawCircuit<F> for PoseidonCircuit<F> {
        fn used_rows(&self) -> usize {
            POSEIDON_ROWS * self.permutations_per_lane + 1
        }

        fn layout_fields(&self) -> String {
            format!(
                " m8_lanes={} m8_permutations={} m8_rows_per_permutation={POSEIDON_ROWS}",
                self.lanes.len(),
                self.lanes.len() * self.permutations_per_lane
            )
        }
    }

    /// Custom Poseidon circuit: `lanes` independent sponges sharing `permutations`.
    fn poseidon_custom<F: KagemushaPoseidonFieldV1>(
        seed: u64,
        lanes: usize,
        permutations: usize,
    ) -> (PoseidonCircuit<F>, Vec<F>) {
        assert_eq!(
            permutations % lanes,
            0,
            "permutations split evenly over lanes"
        );
        let per_lane = permutations / lanes;
        let spec = PoseidonSpec::<F>::new();
        let mut rng = SplitMix64(seed);
        let traces = (0..lanes)
            .map(|_| {
                let inputs = (0..2 * per_lane - 1)
                    .map(|_| rng.field::<F>())
                    .collect::<Vec<_>>();
                poseidon_lane_trace(&spec, &inputs)
            })
            .collect::<Vec<_>>();
        let public = traces.iter().map(|trace| trace.output).collect();
        (
            PoseidonCircuit {
                spec,
                lanes: traces,
                permutations_per_lane: per_lane,
                known: true,
            },
            public,
        )
    }

    /// The same raw sponge in halo2-base gates (one `PoseidonHasher` over `2n - 1` inputs).
    fn poseidon_base<F: KagemushaPoseidonFieldV1>(
        builder: &mut BaseCircuitBuilder<F>,
        seed: u64,
        permutations: usize,
    ) -> Vec<F> {
        let gate = GateChip::<F>::default();
        let mut rng = SplitMix64(seed);
        let inputs = (0..2 * permutations - 1)
            .map(|_| rng.field::<F>())
            .collect::<Vec<_>>();
        let ctx = builder.main(0);
        let mut hasher =
            PoseidonHasher::<F, WIDTH, 2>::new(F::kagemusha_poseidon_spec_v1().clone());
        hasher.initialize_consts(ctx, &gate);
        let cells = inputs
            .iter()
            .map(|value| ctx.load_witness(*value))
            .collect::<Vec<_>>();
        let digest = hasher.hash_fix_len_array(ctx, &gate, &cells);
        assert_eq!(
            *digest.value(),
            F::kagemusha_poseidon_hash_v1(&inputs),
            "halo2-base sponge equals the native KAGEMUSHA Poseidon"
        );
        builder.assigned_instances[0].push(digest);
        vec![*digest.value()]
    }

    /// The same raw sponge in the repository native Pasta Poseidon lanes.
    fn poseidon_native<F: KagemushaPoseidonFieldV1>(
        mut base: BaseCircuitBuilder<F>,
        k: u32,
        seed: u64,
        permutations: usize,
    ) -> (M7NativeCircuit<F>, Vec<F>) {
        let lanes = m7_native_lanes(k, permutations)
            .unwrap_or_else(|| panic!("{permutations} permutations exceed two lanes at k = {k}"));
        let mut jobs = PastaNativePoseidonJobsV1::new(lanes, (1_usize << k) - MINIMUM_ROWS)
            .unwrap_or_else(|error| panic!("native Poseidon lane envelope: {error}"));
        let gate = GateChip::<F>::default();
        let mut rng = SplitMix64(seed);
        let inputs = (0..2 * permutations - 1)
            .map(|_| rng.field::<F>())
            .collect::<Vec<_>>();
        let ctx = base.main(0);
        let cells = inputs
            .iter()
            .map(|value| ctx.load_witness(*value))
            .collect::<Vec<_>>();
        let digest = jobs
            .queue_raw(ctx, &gate, cells, &[])
            .unwrap_or_else(|error| panic!("native Poseidon lanes: {error}"));
        assert_eq!(
            *digest.value(),
            F::kagemusha_poseidon_hash_v1(&inputs),
            "native lanes equal the native KAGEMUSHA Poseidon"
        );
        base.assigned_instances[0].push(digest);
        let public = vec![*digest.value()];
        (
            M7NativeCircuit {
                base,
                jobs,
                lanes,
                permutations,
            },
            public,
        )
    }

    fn measure_poseidon_base(permutations: usize, k: u32) {
        drop(measure_impl::<EqAffine>(
            &format!("m8_poseidon_base_n{permutations}"),
            k,
            None,
            Some(cached_params::<EqAffine>(k)),
            |builder, seed| poseidon_base(builder, seed, permutations),
        ));
    }

    fn measure_poseidon_native(permutations: usize, k: u32) {
        drop(measure_circuit::<EqAffine, M7NativeCircuit<Fp>>(
            &format!("m8_poseidon_native_n{permutations}"),
            k,
            None,
            Some(cached_params::<EqAffine>(k)),
            |base, seed| poseidon_native(base, k, seed, permutations),
        ));
    }

    fn measure_poseidon_custom(permutations: usize, lanes: usize, k: u32) {
        measure_raw::<EqAffine, PoseidonCircuit<Fp>>(
            &format!("m8_poseidon_custom_n{permutations}_l{lanes}"),
            k,
            |seed| poseidon_custom(seed, lanes, permutations),
        );
    }

    // -----------------------------------------------------------------------
    // (2) Checked u128 arithmetic.
    // -----------------------------------------------------------------------

    /// Running-sum decomposition of one u128 into `limbs` limbs of `limb_bits` bits.
    #[derive(Clone, Copy, Debug, Default)]
    struct U128Shape {
        limb_bits: usize,
        limbs: usize,
        top_bits: usize,
        /// Rows per range-checked u128: `limbs`, plus one shifted top-limb row when needed.
        rows: usize,
    }

    impl U128Shape {
        fn new(limb_bits: usize) -> Self {
            let limbs = 128_usize.div_ceil(limb_bits);
            let top_bits = 128 - (limbs - 1) * limb_bits;
            Self {
                limb_bits,
                limbs,
                top_bits,
                rows: limbs + usize::from(top_bits < limb_bits),
            }
        }

        /// Running-sum column entries of one value (honest for values below `2^128`).
        fn block<F: KagemushaPoseidonFieldV1>(&self, value: F) -> Vec<F> {
            let integer = fe_to_biguint(&value);
            let mut entries = (0..self.limbs)
                .map(|limb| biguint_to_fe::<F>(&(&integer >> (self.limb_bits * limb))))
                .collect::<Vec<_>>();
            if self.top_bits < self.limb_bits {
                let top = entries[self.limbs - 1];
                entries.push(top * F::from(1_u64 << (self.limb_bits - self.top_bits)));
            }
            entries
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct U128Params {
        columns: usize,
        limb_bits: usize,
    }

    #[derive(Clone, Debug)]
    struct U128Config {
        columns: Vec<Column<Advice>>,
        table: TableColumn,
        q_step: Selector,
        q_limb: Selector,
        q_shift: Selector,
        q_add: Selector,
        q_sub: Selector,
        instance: Column<Instance>,
    }

    /// Deliberately invalid u128 witnesses for the relation checks.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum U128Mutation {
        None,
        /// The first operation (an addition) produces exactly `2^128`.
        Overflow,
        /// The second operation (a subtraction) goes below zero.
        Underflow,
        /// The first result is decomposed honestly but is `acc + x + 1`.
        WrongSum,
    }

    /// Custom running-sum u128 lanes; each lane is one chain of checked add/sub operations.
    #[derive(Clone, Debug)]
    struct U128Circuit<F: KagemushaPoseidonFieldV1> {
        shape: U128Shape,
        columns: Vec<Vec<F>>,
        operations_per_column: usize,
        known: bool,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for U128Circuit<F> {
        type Config = U128Config;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = U128Params;

        fn without_witnesses(&self) -> Self {
            Self {
                known: false,
                ..self.clone()
            }
        }

        fn params(&self) -> Self::Params {
            U128Params {
                columns: self.columns.len(),
                limb_bits: self.shape.limb_bits,
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("M8 circuits are configured from parameters")
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let shape = U128Shape::new(params.limb_bits);
            let columns = (0..params.columns)
                .map(|_| meta.advice_column())
                .collect::<Vec<_>>();
            let table = meta.lookup_table_column();
            let q_step = meta.complex_selector();
            let q_limb = meta.complex_selector();
            let q_shift = meta.selector();
            let q_add = meta.selector();
            let q_sub = meta.selector();
            let instance = meta.instance_column();
            meta.enable_equality(instance);
            let radix = F::from(1_u64 << shape.limb_bits);
            let shift = F::from(1_u64 << (shape.limb_bits - shape.top_bits));
            let block = i32::try_from(shape.rows).expect("block rows fit i32");
            for &column in &columns {
                meta.enable_equality(column);
                meta.lookup("M8 u128 running-sum limb", |meta| {
                    let cur = meta.query_advice(column, Rotation::cur());
                    let next = meta.query_advice(column, Rotation::next());
                    let step = meta.query_selector(q_step);
                    let limb = meta.query_selector(q_limb);
                    vec![(
                        step * (cur.clone() - next * constant(radix)) + limb * cur,
                        table,
                    )]
                });
                if shape.top_bits < shape.limb_bits {
                    meta.create_gate("M8 u128 shifted top limb", |meta| {
                        let q = meta.query_selector(q_shift);
                        let cur = meta.query_advice(column, Rotation::cur());
                        let top = meta.query_advice(column, Rotation::prev());
                        vec![q * (cur - top * constant(shift))]
                    });
                }
                meta.create_gate("M8 checked u128 add/sub", |meta| {
                    let add = meta.query_selector(q_add);
                    let sub = meta.query_selector(q_sub);
                    let result = meta.query_advice(column, Rotation::cur());
                    let operand = meta.query_advice(column, Rotation(-block));
                    let accumulator = meta.query_advice(column, Rotation(-2 * block));
                    vec![
                        add * (result.clone() - accumulator.clone() - operand.clone()),
                        sub * (result - accumulator + operand),
                    ]
                });
            }
            U128Config {
                columns,
                table,
                q_step,
                q_limb,
                q_shift,
                q_add,
                q_sub,
                instance,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            let shape = self.shape;
            layouter.assign_table(
                || "M8 u128 limb table",
                |mut table| {
                    for value in 0..(1_usize << shape.limb_bits) {
                        table.assign_cell(
                            || "limb",
                            config.table,
                            value,
                            || Value::known(F::from(value as u64)),
                        )?;
                    }
                    Ok(())
                },
            )?;
            let blocks = 1 + 2 * self.operations_per_column;
            let outputs = layouter.assign_region(
                || "M8 u128 lanes",
                |mut region| {
                    for block in 0..blocks {
                        let start = block * shape.rows;
                        for limb in 0..shape.limbs - 1 {
                            config.q_step.enable(&mut region, start + limb)?;
                        }
                        config.q_limb.enable(&mut region, start + shape.limbs - 1)?;
                        if shape.top_bits < shape.limb_bits {
                            config.q_limb.enable(&mut region, start + shape.limbs)?;
                            config.q_shift.enable(&mut region, start + shape.limbs)?;
                        }
                        if block >= 2 && block % 2 == 0 {
                            let operation = block / 2 - 1;
                            let selector = if operation % 2 == 0 {
                                config.q_add
                            } else {
                                config.q_sub
                            };
                            selector.enable(&mut region, start)?;
                        }
                    }
                    let mut outputs = Vec::with_capacity(self.columns.len());
                    for (&column, values) in config.columns.iter().zip(&self.columns) {
                        assert_eq!(values.len(), blocks * shape.rows, "u128 lane length");
                        let mut output = None;
                        for (row, value) in values.iter().enumerate() {
                            let cell = region.assign_advice_discarding_value(
                                column,
                                row,
                                witness(self.known, *value),
                            );
                            if row == (blocks - 1) * shape.rows {
                                output = Some(cell);
                            }
                        }
                        outputs.push(output.expect("final accumulator"));
                    }
                    Ok(outputs)
                },
            )?;
            for (index, cell) in outputs.into_iter().enumerate() {
                layouter.constrain_instance(cell, config.instance, index);
            }
            Ok(())
        }
    }

    impl<F: KagemushaPoseidonFieldV1> RawCircuit<F> for U128Circuit<F> {
        fn used_rows(&self) -> usize {
            (1 + 2 * self.operations_per_column) * self.shape.rows
        }

        fn layout_fields(&self) -> String {
            format!(
                " m8_lanes={} m8_operations={} m8_limb_bits={} m8_rows_per_u128={} m8_table_rows={}",
                self.columns.len(),
                self.columns.len() * self.operations_per_column,
                self.shape.limb_bits,
                self.shape.rows,
                1_usize << self.shape.limb_bits
            )
        }
    }

    /// Starting accumulator near `2^127` and operands below `2^100`; operations alternate
    /// add (even index) and subtract (odd index), so every honest value stays in `[0, 2^128)`.
    fn u128_values(seed: u64, operations: usize) -> (u128, Vec<u128>) {
        let mut rng = SplitMix64(seed);
        let start = (1_u128 << 127) + (m6_u128(&mut rng) >> 28);
        let operands = (0..operations).map(|_| m6_u128(&mut rng) >> 28).collect();
        (start, operands)
    }

    fn u128_custom<F: KagemushaPoseidonFieldV1>(
        seed: u64,
        columns: usize,
        limb_bits: usize,
        operations: usize,
        mutation: U128Mutation,
    ) -> (U128Circuit<F>, Vec<F>) {
        assert_eq!(
            operations % columns,
            0,
            "operations split evenly over lanes"
        );
        let per_column = operations / columns;
        let shape = U128Shape::new(limb_bits);
        let mut public = Vec::with_capacity(columns);
        let lanes = (0..columns)
            .map(|column| {
                let (start, operands) =
                    u128_values(seed ^ (0x9e37 * (column as u64 + 1)), per_column);
                let two_128 = m6_two_pow_128::<F>();
                let mut accumulator = F::from_u128(start);
                let mut entries = shape.block(accumulator);
                for (index, operand) in operands.iter().enumerate() {
                    let mut x = F::from_u128(*operand);
                    if column == 0 && index == 0 && mutation == U128Mutation::Overflow {
                        x = two_128 - accumulator;
                    }
                    if column == 0 && index == 1 && mutation == U128Mutation::Underflow {
                        x = accumulator + F::ONE;
                    }
                    accumulator = if index % 2 == 0 {
                        accumulator + x
                    } else {
                        accumulator - x
                    };
                    let mut result = accumulator;
                    if column == 0 && index == 0 && mutation == U128Mutation::WrongSum {
                        result += F::ONE;
                    }
                    entries.extend(shape.block(x));
                    entries.extend(shape.block(result));
                }
                public.push(accumulator);
                entries
            })
            .collect::<Vec<_>>();
        if mutation == U128Mutation::None {
            for value in &public {
                assert!(
                    fe_to_biguint(value).bits() <= 128,
                    "honest accumulator stays a u128"
                );
            }
        }
        (
            U128Circuit {
                shape,
                columns: lanes,
                operations_per_column: per_column,
                known: true,
            },
            public,
        )
    }

    /// The same checked chain in halo2-base (`RangeChip::range_check(_, 128)`).
    fn u128_base<F: KagemushaPoseidonFieldV1>(
        builder: &mut BaseCircuitBuilder<F>,
        seed: u64,
        operations: usize,
    ) -> Vec<F> {
        let range = builder.range_chip();
        let gate = range.gate();
        let (start, operands) = u128_values(seed, operations);
        let ctx = builder.main(0);
        let mut value = start;
        let mut accumulator = ctx.load_witness(F::from_u128(start));
        range.range_check(ctx, accumulator, 128);
        for (index, operand) in operands.into_iter().enumerate() {
            let cell = ctx.load_witness(F::from_u128(operand));
            range.range_check(ctx, cell, 128);
            accumulator = if index % 2 == 0 {
                value += operand;
                gate.add(ctx, accumulator, cell)
            } else {
                value -= operand;
                gate.sub(ctx, accumulator, cell)
            };
            range.range_check(ctx, accumulator, 128);
        }
        assert_eq!(
            *accumulator.value(),
            F::from_u128(value),
            "halo2-base u128 chain"
        );
        builder.assigned_instances[0].push(accumulator);
        vec![*accumulator.value()]
    }

    fn measure_u128_base(k: u32) {
        drop(measure_impl::<EqAffine>(
            &format!("m8_u128_base_n{U128_OPERATIONS}"),
            k,
            Some(k as usize - 1),
            Some(cached_params::<EqAffine>(k)),
            |builder, seed| u128_base(builder, seed, U128_OPERATIONS),
        ));
    }

    fn measure_u128_custom(columns: usize, k: u32) {
        measure_raw::<EqAffine, U128Circuit<Fp>>(
            &format!("m8_u128_custom_n{U128_OPERATIONS}_l{columns}"),
            k,
            |seed| {
                u128_custom(
                    seed,
                    columns,
                    k as usize - 1,
                    U128_OPERATIONS,
                    U128Mutation::None,
                )
            },
        );
    }

    // -----------------------------------------------------------------------
    // (3) Native-curve variable-base scalar multiplication.
    // -----------------------------------------------------------------------

    #[derive(Clone, Copy, Debug, Default)]
    struct MulParams {
        lanes: usize,
    }

    #[derive(Clone, Debug)]
    struct MulConfig {
        lanes: Vec<[Column<Advice>; MUL_COLUMNS]>,
        q_double: Selector,
        q_bit: Selector,
        q_out: Selector,
        instance: Column<Instance>,
    }

    /// One scalar multiplication's rows: `[x_a, y_a, lambda1, lambda2, z, x_p, y_p, io]`.
    #[derive(Clone, Debug)]
    struct MulTrace<F> {
        rows: Vec<[F; MUL_COLUMNS]>,
        output_x: F,
    }

    /// Custom double-and-add lanes; every lane holds the same number of multiplications.
    #[derive(Clone, Debug)]
    struct MulCircuit<F: KagemushaPoseidonFieldV1> {
        lanes: Vec<Vec<MulTrace<F>>>,
        known: bool,
    }

    /// Deliberately invalid scalar-multiplication witnesses for the relation checks.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MulMutation {
        None,
        /// The running sum gains one at bit row 100, so two bits leave {0, 1}.
        RunningSum,
        /// The base point leaves the curve.
        OffCurve,
        /// One addition slope is wrong.
        Slope,
    }

    /// `[s]P` through the custom double-and-add chain.
    ///
    /// The chain starts at `2P` and adds `+P` or `-P` per bit of `t` (most significant first),
    /// ending at `[2t + 2^255 + 1]P`; with `t = (s - 2^255 - 1) / 2` modulo the curve order this
    /// is `[s]P`, asserted natively.
    fn mul_trace<F: M6NativeCurve>(
        point: F::Curve,
        scalar: F::Scalar,
        mutation: MulMutation,
    ) -> MulTrace<F> {
        let two = F::Scalar::from(2);
        let shift = two.pow_vartime([SCALAR_BITS as u64]);
        let t = (scalar - shift - F::Scalar::ONE)
            * Option::<F::Scalar>::from(two.invert()).expect("odd curve order");
        let repr = t.to_repr();
        let bytes = repr.as_ref();
        let bit = |index: usize| (bytes[index / 8] >> (index % 8)) & 1 == 1;
        assert!(!bit(SCALAR_BITS), "t fits 255 bits");
        let (xp, yp) = point.into_coordinates();
        let mut rows = vec![[F::ZERO; MUL_COLUMNS]; MUL_ROWS];
        let lambda = F::from(3)
            * xp.square()
            * Option::<F>::from((yp + yp).invert()).expect("y_P is nonzero");
        rows[0][XP] = xp;
        rows[0][YP] = yp;
        rows[0][L1] = lambda;
        let mut xa = lambda.square() - xp - xp;
        let mut ya = lambda * (xp - xa) - yp;
        let mut z = F::ZERO;
        for row in 1..=SCALAR_BITS {
            let set = bit(SCALAR_BITS - row);
            let y_t = if set { yp } else { -yp };
            let dx = xa - xp;
            assert!(dx != F::ZERO, "incomplete addition hit x_A = x_P");
            let l1 = (ya - y_t) * Option::<F>::from(dx.invert()).expect("nonzero");
            let x_r = l1.square() - xa - xp;
            let dr = xa - x_r;
            assert!(dr != F::ZERO, "incomplete addition hit x_A = x_R");
            let l2 = (ya + ya) * Option::<F>::from(dr.invert()).expect("nonzero") - l1;
            let next_x = l2.square() - xa - x_r;
            let next_y = l2 * (xa - next_x) - ya;
            rows[row] = [xa, ya, l1, l2, z, xp, yp, F::ZERO];
            z = z + z + if set { F::ONE } else { F::ZERO };
            xa = next_x;
            ya = next_y;
        }
        rows[SCALAR_BITS + 1] = [xa, ya, F::ZERO, F::ZERO, z, xp, yp, xa];
        rows[0][IO] = xp;
        rows[1][IO] = yp;
        rows[SCALAR_BITS][IO] = ya;
        rows[SCALAR_BITS - 1][IO] = z;
        let (expected_x, expected_y) = (point * scalar).to_affine().into_coordinates();
        assert_eq!(
            (xa, ya),
            (expected_x, expected_y),
            "custom double-and-add computes [s]P"
        );
        match mutation {
            MulMutation::None => {}
            MulMutation::RunningSum => rows[100][Z] += F::ONE,
            MulMutation::OffCurve => {
                for row in &mut rows {
                    row[YP] += F::ONE;
                }
            }
            MulMutation::Slope => rows[50][L1] += F::ONE,
        }
        MulTrace { rows, output_x: xa }
    }

    impl<F: M6NativeCurve> Circuit<F> for MulCircuit<F> {
        type Config = MulConfig;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = MulParams;

        fn without_witnesses(&self) -> Self {
            Self {
                known: false,
                ..self.clone()
            }
        }

        fn params(&self) -> Self::Params {
            MulParams {
                lanes: self.lanes.len(),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("M8 circuits are configured from parameters")
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let lanes = (0..params.lanes)
                .map(|_| std::array::from_fn(|_| meta.advice_column()))
                .collect::<Vec<[Column<Advice>; MUL_COLUMNS]>>();
            let q_double = meta.selector();
            let q_bit = meta.selector();
            let q_out = meta.selector();
            let instance = meta.instance_column();
            meta.enable_equality(instance);
            let curve_b = <F::Curve as CurveAffine>::b();
            for lane in &lanes {
                let lane = *lane;
                // Base point, scalar and result enter or leave through copy constraints on the
                // single IO column: x_P and y_P on the first two rows, then z, y_A and x_A on the
                // last three.
                meta.enable_equality(lane[IO]);
                meta.create_gate("M8 scalar multiplication result output", |meta| {
                    let q = meta.query_selector(q_out);
                    vec![
                        q.clone()
                            * (meta.query_advice(lane[IO], Rotation::cur())
                                - meta.query_advice(lane[XA], Rotation::cur())),
                        q.clone()
                            * (meta.query_advice(lane[IO], Rotation::prev())
                                - meta.query_advice(lane[YA], Rotation::cur())),
                        q * (meta.query_advice(lane[IO], Rotation(-2))
                            - meta.query_advice(lane[Z], Rotation::cur())),
                    ]
                });
                meta.create_gate("M8 load P, on-curve check and doubling", |meta| {
                    let q = meta.query_selector(q_double);
                    let xp = meta.query_advice(lane[XP], Rotation::cur());
                    let yp = meta.query_advice(lane[YP], Rotation::cur());
                    let lambda = meta.query_advice(lane[L1], Rotation::cur());
                    let next_x = meta.query_advice(lane[XA], Rotation::next());
                    let next_y = meta.query_advice(lane[YA], Rotation::next());
                    let next_z = meta.query_advice(lane[Z], Rotation::next());
                    let next_xp = meta.query_advice(lane[XP], Rotation::next());
                    let next_yp = meta.query_advice(lane[YP], Rotation::next());
                    let io = meta.query_advice(lane[IO], Rotation::cur());
                    let next_io = meta.query_advice(lane[IO], Rotation::next());
                    let two = constant(F::from(2));
                    let three = constant(F::from(3));
                    vec![
                        q.clone() * (io - xp.clone()),
                        q.clone() * (next_io - yp.clone()),
                        q.clone()
                            * (yp.clone() * yp.clone()
                                - xp.clone() * xp.clone() * xp.clone()
                                - constant(curve_b)),
                        q.clone()
                            * (two.clone() * yp.clone() * lambda.clone()
                                - three * xp.clone() * xp.clone()),
                        q.clone()
                            * (next_x.clone()
                                - (lambda.clone() * lambda.clone() - two * xp.clone())),
                        q.clone() * (next_y - (lambda * (xp.clone() - next_x) - yp.clone())),
                        q.clone() * next_z,
                        q.clone() * (next_xp - xp),
                        q * (next_yp - yp),
                    ]
                });
                meta.create_gate("M8 double-and-add bit", |meta| {
                    let q = meta.query_selector(q_bit);
                    let xa = meta.query_advice(lane[XA], Rotation::cur());
                    let ya = meta.query_advice(lane[YA], Rotation::cur());
                    let l1 = meta.query_advice(lane[L1], Rotation::cur());
                    let l2 = meta.query_advice(lane[L2], Rotation::cur());
                    let z = meta.query_advice(lane[Z], Rotation::cur());
                    let xp = meta.query_advice(lane[XP], Rotation::cur());
                    let yp = meta.query_advice(lane[YP], Rotation::cur());
                    let next_x = meta.query_advice(lane[XA], Rotation::next());
                    let next_y = meta.query_advice(lane[YA], Rotation::next());
                    let next_z = meta.query_advice(lane[Z], Rotation::next());
                    let next_xp = meta.query_advice(lane[XP], Rotation::next());
                    let next_yp = meta.query_advice(lane[YP], Rotation::next());
                    let one = constant(F::ONE);
                    let two = constant(F::from(2));
                    let bit = next_z - two.clone() * z;
                    let y_t = (two.clone() * bit.clone() - one.clone()) * yp.clone();
                    let x_r = l1.clone() * l1.clone() - xa.clone() - xp.clone();
                    vec![
                        q.clone() * (bit.clone() * (bit - one)),
                        q.clone() * (l1.clone() * (xa.clone() - xp.clone()) - (ya.clone() - y_t)),
                        q.clone()
                            * ((l1 + l2.clone()) * (xa.clone() - x_r.clone()) - two * ya.clone()),
                        q.clone() * (next_x.clone() - (l2.clone() * l2.clone() - xa.clone() - x_r)),
                        q.clone() * (next_y - (l2 * (xa - next_x) - ya)),
                        q.clone() * (next_xp - xp),
                        q * (next_yp - yp),
                    ]
                });
            }
            MulConfig {
                lanes,
                q_double,
                q_bit,
                q_out,
                instance,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            let per_lane = self.lanes.first().map_or(0, Vec::len);
            let outputs = layouter.assign_region(
                || "M8 scalar multiplication lanes",
                |mut region| {
                    for mul in 0..per_lane {
                        let base = MUL_ROWS * mul;
                        config.q_double.enable(&mut region, base)?;
                        for row in 1..=SCALAR_BITS {
                            config.q_bit.enable(&mut region, base + row)?;
                        }
                        config.q_out.enable(&mut region, base + SCALAR_BITS + 1)?;
                    }
                    let mut outputs = Vec::with_capacity(self.lanes.len() * per_lane);
                    for (columns, muls) in config.lanes.iter().zip(&self.lanes) {
                        assert_eq!(muls.len(), per_lane, "equal multiplications per lane");
                        for (mul, trace) in muls.iter().enumerate() {
                            let base = MUL_ROWS * mul;
                            for (row, values) in trace.rows.iter().enumerate() {
                                for (column, value) in values.iter().enumerate() {
                                    let cell = region.assign_advice_discarding_value(
                                        columns[column],
                                        base + row,
                                        witness(self.known, *value),
                                    );
                                    if column == IO && row == SCALAR_BITS + 1 {
                                        outputs.push(cell);
                                    }
                                }
                            }
                        }
                    }
                    Ok(outputs)
                },
            )?;
            for (index, cell) in outputs.into_iter().enumerate() {
                layouter.constrain_instance(cell, config.instance, index);
            }
            Ok(())
        }
    }

    impl<F: M6NativeCurve> RawCircuit<F> for MulCircuit<F> {
        fn used_rows(&self) -> usize {
            MUL_ROWS * self.lanes.first().map_or(0, Vec::len)
        }

        fn layout_fields(&self) -> String {
            format!(
                " m8_lanes={} m8_scalar_muls={} m8_rows_per_mul={MUL_ROWS} m8_scalar_bits={SCALAR_BITS}",
                self.lanes.len(),
                self.lanes.iter().map(Vec::len).sum::<usize>()
            )
        }
    }

    fn random_point_and_scalar<F: M6NativeCurve>(rng: &mut SplitMix64) -> (F::Curve, F::Scalar) {
        let point = (F::Curve::generator() * rng.field::<F::Scalar>()).to_affine();
        (point, rng.field::<F::Scalar>())
    }

    fn mul_custom<F: M6NativeCurve>(
        seed: u64,
        lanes: usize,
        muls: usize,
        mutation: MulMutation,
    ) -> (MulCircuit<F>, Vec<F>) {
        assert_eq!(muls % lanes, 0, "multiplications split evenly over lanes");
        let mut rng = SplitMix64(seed);
        let traces = (0..lanes)
            .map(|lane| {
                (0..muls / lanes)
                    .map(|mul| {
                        let (point, scalar) = random_point_and_scalar::<F>(&mut rng);
                        let mutation = if lane == 0 && mul == 0 {
                            mutation
                        } else {
                            MulMutation::None
                        };
                        mul_trace::<F>(point, scalar, mutation)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let public = traces
            .iter()
            .flatten()
            .map(|trace| trace.output_x)
            .collect();
        (
            MulCircuit {
                lanes: traces,
                known: true,
            },
            public,
        )
    }

    /// The same multiplications through halo2-ecc (`load_private` checks the curve equation).
    fn mul_base<F: M6NativeCurve>(
        builder: &mut BaseCircuitBuilder<F>,
        seed: u64,
        muls: usize,
    ) -> Vec<F> {
        let range = builder.range_chip();
        let native = M6NativeFieldChip::new(&range);
        let ecc = EccChip::new(&native);
        let mut rng = SplitMix64(seed);
        let ctx = builder.main(0);
        let mut outputs = Vec::with_capacity(muls);
        for _ in 0..muls {
            let (point, scalar) = random_point_and_scalar::<F>(&mut rng);
            let loaded = ecc.load_private::<F::Curve>(ctx, point.into_coordinates());
            let (low, high) = m6_u128_halves(scalar.to_repr().as_ref());
            let limbs = vec![
                ctx.load_witness(F::from_u128(low)),
                ctx.load_witness(F::from_u128(high)),
            ];
            let result = ecc.scalar_mult::<F::Curve>(ctx, loaded, limbs, 128, M6_EC_WINDOW_BITS);
            let (expected_x, _) = (point * scalar).to_affine().into_coordinates();
            assert_eq!(*result.x().0.value(), expected_x, "halo2-ecc computes [s]P");
            outputs.push(result.x().0);
        }
        builder.assigned_instances[0].extend(outputs.iter().copied());
        outputs.iter().map(|cell| *cell.value()).collect()
    }

    fn measure_mul_base(k: u32) {
        drop(measure_impl::<EqAffine>(
            &format!("m8_varmul_base_n{SCALAR_MULS}"),
            k,
            Some(k as usize - 1),
            Some(cached_params::<EqAffine>(k)),
            |builder, seed| mul_base(builder, seed, SCALAR_MULS),
        ));
    }

    fn measure_mul_custom(lanes: usize, k: u32) {
        measure_raw::<EqAffine, MulCircuit<Fp>>(
            &format!("m8_varmul_custom_n{SCALAR_MULS}_l{lanes}"),
            k,
            |seed| mul_custom(seed, lanes, SCALAR_MULS, MulMutation::None),
        );
    }

    // -----------------------------------------------------------------------
    // Relation checks, inventory and the recursion scalar-half breakdown.
    // -----------------------------------------------------------------------

    fn mock_accepts<Circ: Circuit<Fp>>(k: u32, circuit: &Circ, public: Vec<Fp>) -> bool {
        MockProver::run(k, circuit, vec![public])
            .expect("M8 mock synthesis")
            .verify()
            .is_ok()
    }

    /// Measurement harness (M8): every custom chip accepts honest witnesses whose public outputs
    /// equal the native computation, and rejects tampered traces (MockProver, small k).
    #[test]
    #[ignore = "M8 custom-gate measurement harness; run explicitly in release"]
    fn m8_custom_gate_checks() {
        // Poseidon: two lanes of three permutations each; outputs equal the native sponge.
        let (honest, public) = poseidon_custom::<Fp>(PROVER_SEED, 2, 6);
        assert!(
            mock_accepts(9, &honest, public.clone()),
            "honest Poseidon lanes"
        );
        let tampered_rows = [
            ("initial state", 0_usize, 0_usize),
            ("full round", 2, 1),
            ("pair round", 10, 2),
            ("pair S-box", 12, WIDTH),
            ("single partial round", 33, 0),
            ("absorbed input", POSEIDON_ROWS, WIDTH),
            ("final state", 3 * POSEIDON_ROWS, 2),
        ];
        for (label, row, column) in tampered_rows {
            let mut changed = honest.clone();
            if column == WIDTH {
                changed.lanes[1].aux[row] += Fp::ONE;
            } else {
                changed.lanes[1].state[row][column] += Fp::ONE;
            }
            let accepted = mock_accepts(9, &changed, public.clone());
            println!("M8_CHECK chip=poseidon mutation={label} accepted={accepted}");
            assert!(!accepted, "Poseidon {label} mutation must fail");
        }
        let mut wrong = public.clone();
        wrong[0] += Fp::ONE;
        assert!(!mock_accepts(9, &honest, wrong), "wrong Poseidon digest");

        // u128: two lanes of four operations, 9-bit limbs (k = 10).
        for mutation in [
            U128Mutation::None,
            U128Mutation::Overflow,
            U128Mutation::Underflow,
            U128Mutation::WrongSum,
        ] {
            let (circuit, public) = u128_custom::<Fp>(PROVER_SEED, 2, 9, 8, mutation);
            let accepted = mock_accepts(10, &circuit, public);
            println!("M8_CHECK chip=u128 mutation={mutation:?} accepted={accepted}");
            assert_eq!(
                accepted,
                mutation == U128Mutation::None,
                "u128 {mutation:?}"
            );
        }

        // Scalar multiplication: two lanes of one multiplication each.
        for mutation in [
            MulMutation::None,
            MulMutation::RunningSum,
            MulMutation::OffCurve,
            MulMutation::Slope,
        ] {
            let (circuit, public) = mul_custom::<Fp>(PROVER_SEED, 2, 2, mutation);
            let accepted = mock_accepts(10, &circuit, public);
            println!("M8_CHECK chip=varmul mutation={mutation:?} accepted={accepted}");
            assert_eq!(
                accepted,
                mutation == MulMutation::None,
                "scalar mul {mutation:?}"
            );
        }
        let (circuit, mut public) = mul_custom::<Fp>(PROVER_SEED, 2, 2, MulMutation::None);
        public[1] += Fp::ONE;
        assert!(
            !mock_accepts(10, &circuit, public),
            "wrong scalar-mul output"
        );

        // Halo2-base and native-lane counterparts compute identical digests and accept.
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(12)
            .use_instance_columns(1);
        let base_public = poseidon_base(&mut builder, PROVER_SEED, 3);
        builder.calculate_params(Some(MINIMUM_ROWS));
        assert!(
            mock_accepts(12, &builder, base_public.clone()),
            "halo2-base Poseidon"
        );
        let (custom, custom_public) = poseidon_custom::<Fp>(PROVER_SEED, 1, 3);
        let native_base = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(12)
            .use_instance_columns(1);
        let (mut native, native_public) = poseidon_native(native_base, 12, PROVER_SEED, 3);
        native.base.calculate_params(Some(MINIMUM_ROWS));
        assert!(
            mock_accepts(12, &native, native_public.clone()),
            "native lanes"
        );
        assert!(
            mock_accepts(9, &custom, custom_public.clone()),
            "custom lane"
        );
        assert_eq!(
            base_public, native_public,
            "halo2-base and native lanes agree"
        );
        assert_eq!(
            base_public, custom_public,
            "halo2-base and custom chip agree"
        );
        println!("M8_CHECK all=passed");
    }

    /// Exact halo2-base advice and lookup cells of `build` on a fresh builder.
    fn base_cells(
        k: u32,
        lookup_bits: Option<usize>,
        build: impl FnOnce(&mut BaseCircuitBuilder<Fp>),
    ) -> (usize, usize) {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(k as usize)
            .use_instance_columns(1);
        if let Some(bits) = lookup_bits {
            builder.set_lookup_bits(bits);
        }
        build(&mut builder);
        let statistics = builder.statistics();
        (
            statistics.gate.total_advice_per_phase[0],
            statistics.total_lookup_advice_per_phase[0],
        )
    }

    /// Measurement harness (M8): exact unit costs of every building block in each design.
    #[test]
    #[ignore = "M8 custom-gate measurement harness; run explicitly in release"]
    fn m8_inventory() {
        // Poseidon: halo2-base cells of raw sponges (including the hasher's constant setup).
        let mut previous = None;
        for permutations in [1_usize, 2, 54, 84, 1_000] {
            let (cells, _) = base_cells(16, None, |builder| {
                poseidon_base(builder, KEYGEN_SEED, permutations);
            });
            println!(
                "M8_INVENTORY block=poseidon design=halo2_base permutations={permutations} advice_cells={cells} custom_area_cells={} native_lane_rows={}",
                (POSEIDON_ROWS * permutations + 1) * (WIDTH + 1),
                NATIVE_LANE_ROWS * permutations
            );
            if let Some((count, total)) = previous {
                if permutations == 1_000 {
                    println!(
                        "M8_INVENTORY block=poseidon design=halo2_base cells_per_permutation={:.1}",
                        (cells - total) as f64 / (permutations - count) as f64
                    );
                }
            } else {
                previous = Some((permutations, cells));
            }
        }
        let native_base = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(16)
            .use_instance_columns(1);
        let (native, _) = poseidon_native(native_base, 16, KEYGEN_SEED, POSEIDON_PERMUTATIONS);
        println!(
            "M8_INVENTORY block=poseidon design=native_lanes permutations={POSEIDON_PERMUTATIONS} lanes={} lane_rows={} base_glue_cells={} lane_area_cells={}",
            native.lanes,
            native.jobs.required_rows().expect("native rows"),
            native.base.statistics().gate.total_advice_per_phase[0],
            native.jobs.required_rows().expect("native rows") * (3 * native.lanes + 1),
        );

        // u128: halo2-base cells of one 128-bit range check and one checked operation.
        for bits in 8..=17_usize {
            let k = bits as u32 + 1;
            let (check_cells, check_lookups) = base_cells(k, Some(bits), |builder| {
                let range = builder.range_chip();
                let ctx = builder.main(0);
                let cell = ctx.load_witness(Fp::from_u128(u128::MAX));
                range.range_check(ctx, cell, 128);
            });
            let (chain_cells, chain_lookups) = base_cells(k, Some(bits), |builder| {
                u128_base(builder, KEYGEN_SEED, 100);
            });
            let (start_cells, start_lookups) = base_cells(k, Some(bits), |builder| {
                u128_base(builder, KEYGEN_SEED, 0);
            });
            let shape = U128Shape::new(bits);
            println!(
                "M8_INVENTORY block=u128 limb_bits={bits} halo2_base_range_check_advice={check_cells} halo2_base_range_check_lookup={check_lookups} halo2_base_op_advice={:.1} halo2_base_op_lookup={:.1} custom_rows_per_u128={} custom_op_rows={}",
                (chain_cells - start_cells) as f64 / 100.0,
                (chain_lookups - start_lookups) as f64 / 100.0,
                shape.rows,
                2 * shape.rows
            );
        }

        // Native-curve operations in halo2-ecc over the M6 native field chip.
        let point_cells = |operation: &str| {
            base_cells(16, Some(15), |builder| {
                let range = builder.range_chip();
                let native = M6NativeFieldChip::new(&range);
                let ecc = EccChip::new(&native);
                let mut rng = SplitMix64(KEYGEN_SEED);
                let (point, scalar) = random_point_and_scalar::<Fp>(&mut rng);
                let (other, _) = random_point_and_scalar::<Fp>(&mut rng);
                let ctx = builder.main(0);
                let before = ctx.advice_len();
                let loaded = ecc.load_private::<EpAffine>(ctx, point.into_coordinates());
                let after_load = ctx.advice_len();
                match operation {
                    "load_on_curve" => {}
                    "double" => {
                        let _ = ecc.double(ctx, loaded.clone());
                    }
                    "add_unequal" => {
                        let other = ecc.load_private::<EpAffine>(ctx, other.into_coordinates());
                        let start = ctx.advice_len();
                        let _ = ecc.add_unequal(ctx, &loaded, &other, true);
                        println!(
                            "M8_INVENTORY block=varmul design=halo2_base operation=add_unequal_strict advice_cells={}",
                            ctx.advice_len() - start
                        );
                    }
                    _ => {
                        let (low, high) = m6_u128_halves(scalar.to_repr().as_ref());
                        let limbs = vec![
                            ctx.load_witness(Fp::from_u128(low)),
                            ctx.load_witness(Fp::from_u128(high)),
                        ];
                        let _ =
                            ecc.scalar_mult::<EpAffine>(ctx, loaded, limbs, 128, M6_EC_WINDOW_BITS);
                    }
                }
                println!(
                    "M8_INVENTORY block=varmul design=halo2_base operation={operation} load_cells={} operation_cells={}",
                    after_load - before,
                    ctx.advice_len() - after_load
                );
            })
        };
        for operation in ["load_on_curve", "double", "add_unequal", "scalar_mult_w4"] {
            let (cells, lookups) = point_cells(operation);
            println!(
                "M8_INVENTORY block=varmul design=halo2_base operation={operation} total_advice_cells={cells} lookup_cells={lookups}"
            );
        }
        println!(
            "M8_INVENTORY block=varmul design=custom rows_per_mul={MUL_ROWS} columns={MUL_COLUMNS} area_cells_per_mul={}",
            MUL_ROWS * MUL_COLUMNS
        );
    }

    /// Total advice cells in a loader pool (all virtual threads).
    fn pool_cells(pool: &SinglePhaseCoreManager<Fp>) -> usize {
        pool.threads.iter().map(Context::advice_len).sum()
    }

    /// Scalar half of one recursive verification (as `m6_scalar_half`), returning the advice
    /// cells after each stage.
    fn scalar_half_stages(
        builder: &mut BaseCircuitBuilder<Fp>,
        inner: &MeasuredProof<EqAffine>,
        svk: &IpaSuccinctVerifyingKey<EqAffine>,
        protocol: &PlonkProtocol<EqAffine>,
    ) -> Vec<(&'static str, usize)> {
        let range = builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
        let loader: M6DeferredLoader<'_, EqAffine> = Halo2Loader::new(
            DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer),
            std::mem::take(builder.pool(0)),
        );
        let mut marks = vec![("start", pool_cells(&loader.ctx()))];
        {
            let loaded = protocol.loaded(&loader);
            let instances = vec![
                inner
                    .public
                    .iter()
                    .map(|value| loader.assign_scalar(*value))
                    .collect::<Vec<_>>(),
            ];
            marks.push(("protocol_and_instances", pool_cells(&loader.ctx())));
            let position = Rc::new(Cell::new(0));
            let reader = M6CountingReader {
                bytes: &inner.proof,
                position: Rc::clone(&position),
            };
            let mut transcript =
                M6DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&loader, reader);
            let parsed = PlonkSuccinctVerifier::<IpaAs<EqAffine, Bgh19>>::read_proof(
                svk,
                &loaded,
                &instances,
                &mut transcript,
            )
            .expect("in-circuit proof parse");
            marks.push(("read_proof_transcript", pool_cells(&loader.ctx())));
            let accumulators = PlonkSuccinctVerifier::<IpaAs<EqAffine, Bgh19>>::verify(
                svk, &loaded, &instances, &parsed,
            )
            .expect("in-circuit succinct verification");
            marks.push(("succinct_verify_arithmetic", pool_cells(&loader.ctx())));
            let binding = transcript.squeeze_challenge().into_assigned();
            drop(transcript);
            assert_eq!(position.get(), inner.proof.len(), "whole proof consumed");
            marks.push(("binding_squeeze", pool_cells(&loader.ctx())));
            let equation_count = loader.ecc_chip().equation_count();
            let selectors = {
                let mut ctx = loader.ctx_mut();
                (0..equation_count)
                    .map(|_| ctx.main().load_constant(Fp::ONE))
                    .collect::<Vec<_>>()
            };
            let tags = vec![1_u32; equation_count];
            let elements = {
                let chip = loader.ecc_chip();
                let mut ctx = loader.ctx_mut();
                chip.assigned_equation_poseidon_elements_v1(&mut ctx, &tags, &selectors)
                    .expect("deferred audit elements")
            };
            let element_count = elements.len();
            marks.push(("audit_elements", pool_cells(&loader.ctx())));
            let digest = m6_loader_poseidon_digest(&loader, elements);
            marks.push(("audit_digest", pool_cells(&loader.ctx())));
            let witness = loader.ecc_chip().witness();
            println!(
                "M8_BREAKDOWN_SHAPE sources={} equations={} audit_elements={element_count} audit_digest_permutations={}",
                witness.sources.len(),
                witness.equations.len(),
                element_count / 2 + 1,
            );
            let _ = (binding, digest);
            drop(accumulators);
            drop(parsed);
            drop(instances);
            drop(loaded);
        }
        *builder.pool(0) = loader.take_ctx();
        marks
    }

    /// Loader (snark-verifier) Poseidon cells per permutation on the scalar-half loader.
    fn loader_poseidon_cells_per_permutation() -> f64 {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(16)
            .use_instance_columns(1);
        builder.set_lookup_bits(K16_LOOKUP_BITS);
        let range = builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
        let loader: M6DeferredLoader<'_, EqAffine> = Halo2Loader::new(
            DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer),
            std::mem::take(builder.pool(0)),
        );
        let load = |count: usize| {
            let mut ctx = loader.ctx_mut();
            (0..count)
                .map(|i| ctx.main().load_witness(Fp::from(i as u64 + 7)))
                .collect::<Vec<_>>()
        };
        let (short, long) = (load(2), load(202));
        let start = pool_cells(&loader.ctx());
        let _ = m6_loader_poseidon_digest(&loader, short);
        let middle = pool_cells(&loader.ctx());
        let _ = m6_loader_poseidon_digest(&loader, long);
        let end = pool_cells(&loader.ctx());
        drop(loader.take_ctx());
        // 2 elements take 2 permutations; 202 elements take 102.
        ((end - middle) as f64 - (middle - start) as f64) / 100.0
    }

    /// Measurement harness (M8): advice cells of each stage of the recursion scalar half, the
    /// loader Poseidon cost per permutation and the inner proof's transcript shape.
    #[test]
    #[ignore = "M8 custom-gate measurement harness; run explicitly in release"]
    fn m8_scalar_half_breakdown() {
        let per_permutation = loader_poseidon_cells_per_permutation();
        println!("M8_BREAKDOWN loader_poseidon_cells_per_permutation={per_permutation:.1}");
        for inner_kind in [M6Inner::Synthetic(1), M6Inner::TransitionOnePermutation] {
            let label = inner_kind.label();
            let inner = measure_impl::<EqAffine>(
                &format!("m8_breakdown_inner_{label}"),
                16,
                Some(K16_LOOKUP_BITS),
                Some(cached_params::<EqAffine>(16)),
                |builder, seed| match inner_kind {
                    M6Inner::Synthetic(columns) => {
                        chained_mul_add(builder, 16, columns, true, seed)
                    }
                    M6Inner::TransitionOnePermutation => {
                        m6_transition(builder, seed, M6Node::OnePermutationStandIn, false)
                    }
                },
            );
            let svk = m6_succinct_vk(&inner.params);
            let protocol = compile(
                &inner.params,
                &inner.vk,
                Config::ipa().with_num_instance(vec![inner.public.len()]),
            );
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(16)
                .use_instance_columns(1);
            builder.set_lookup_bits(K16_LOOKUP_BITS);
            let marks = scalar_half_stages(&mut builder, &inner, &svk, &protocol);
            let statistics = builder.statistics();
            let stages = marks
                .windows(2)
                .map(|pair| format!("{}={}", pair[1].0, pair[1].1 - pair[0].1))
                .collect::<Vec<_>>()
                .join(" ");
            let witness_commitments: usize = protocol.num_witness.iter().sum();
            let mut rotations = BTreeMap::<usize, BTreeSet<i32>>::new();
            for query in &protocol.queries {
                rotations
                    .entry(query.poly)
                    .or_default()
                    .insert(query.rotation.0);
            }
            let rotation_sets = rotations.into_values().collect::<BTreeSet<_>>().len();
            let k = 16_usize;
            // Proof items: commitments (W + quotient chunks), Bgh19 f, IPA s, 2k L/R and the
            // folded generator are points; evaluations, q_evals, c and blind are scalars.
            let points = witness_commitments + protocol.quotient.num_chunk() + 2 * k + 3;
            let scalars = protocol.evaluations.len() + rotation_sets + 2;
            let challenges: usize = protocol.num_challenge.iter().sum();
            println!(
                "M8_BREAKDOWN inner={label} inner_proof_bytes={} total_advice_cells={} lookup_cells={} {stages} proof_points={points} proof_scalars={scalars} instances={} plonk_challenges={challenges} rotation_sets={rotation_sets}",
                inner.proof.len(),
                statistics.gate.total_advice_per_phase[0],
                statistics.total_lookup_advice_per_phase[0],
                inner.public.len(),
            );
        }
    }

    macro_rules! m8_cases {
        ($($name:ident => $body:expr;)+) => {$(
            /// Measurement harness (M8): one building block in one design on the Eq side;
            /// proves, verifies and prints one `G3_SCALING` line.
            #[test]
            #[ignore = "M8 custom-gate measurement harness; run explicitly in release"]
            fn $name() {
                $body;
            }
        )+};
    }

    m8_cases! {
        m8_poseidon_base_n1000_k16 => measure_poseidon_base(POSEIDON_PERMUTATIONS, 16);
        m8_poseidon_base_n1000_k17 => measure_poseidon_base(POSEIDON_PERMUTATIONS, 17);
        m8_poseidon_base_n1000_k18 => measure_poseidon_base(POSEIDON_PERMUTATIONS, 18);
        m8_poseidon_native_n1000_k16 => measure_poseidon_native(POSEIDON_PERMUTATIONS, 16);
        m8_poseidon_custom_n1000_l1_k16 => measure_poseidon_custom(POSEIDON_PERMUTATIONS, 1, 16);
        m8_poseidon_custom_n1000_l2_k15 => measure_poseidon_custom(POSEIDON_PERMUTATIONS, 2, 15);
        m8_poseidon_custom_n1000_l4_k14 => measure_poseidon_custom(POSEIDON_PERMUTATIONS, 4, 14);
        m8_poseidon_custom_n1000_l8_k13 => measure_poseidon_custom(POSEIDON_PERMUTATIONS, 8, 13);
        m8_poseidon_base_n54_k15 => measure_poseidon_base(54, 15);
        m8_poseidon_base_n54_k16 => measure_poseidon_base(54, 16);
        m8_poseidon_native_n54_k11 => measure_poseidon_native(54, 11);
        m8_poseidon_custom_n54_l1_k11 => measure_poseidon_custom(54, 1, 11);
        m8_poseidon_custom_n54_l2_k10 => measure_poseidon_custom(54, 2, 10);
        m8_poseidon_native_n84_k12 => measure_poseidon_native(84, 12);
        m8_poseidon_custom_n84_l1_k12 => measure_poseidon_custom(84, 1, 12);
        m8_poseidon_custom_n84_l2_k11 => measure_poseidon_custom(84, 2, 11);
        m8_u128_base_k16 => measure_u128_base(16);
        m8_u128_base_k15 => measure_u128_base(15);
        m8_u128_base_k14 => measure_u128_base(14);
        m8_u128_base_k13 => measure_u128_base(13);
        m8_u128_base_k12 => measure_u128_base(12);
        m8_u128_custom_l1_k16 => measure_u128_custom(1, 16);
        m8_u128_custom_l1_k15 => measure_u128_custom(1, 15);
        m8_u128_custom_l2_k14 => measure_u128_custom(2, 14);
        m8_u128_custom_l4_k13 => measure_u128_custom(4, 13);
        m8_u128_custom_l8_k12 => measure_u128_custom(8, 12);
        m8_varmul_base_k16 => measure_mul_base(16);
        m8_varmul_base_k15 => measure_mul_base(15);
        m8_varmul_base_k14 => measure_mul_base(14);
        m8_varmul_custom_l1_k13 => measure_mul_custom(1, 13);
        m8_varmul_custom_l2_k12 => measure_mul_custom(2, 12);
        m8_varmul_custom_l4_k11 => measure_mul_custom(4, 11);
    }

    // -----------------------------------------------------------------------
    // Prover floor: the cheapest possible full-height circuit at each k.
    // -----------------------------------------------------------------------

    #[derive(Clone, Debug)]
    struct FloorConfig {
        value: Column<Advice>,
        q_step: Selector,
        instance: Column<Instance>,
    }

    /// One advice column holding `start, start + 1, ...` over `rows` rows (one degree-2 gate,
    /// equality on the column) with the last value public: the per-k floor of this prover.
    #[derive(Clone, Debug)]
    struct FloorCircuit<F: KagemushaPoseidonFieldV1> {
        rows: usize,
        start: F,
        known: bool,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for FloorCircuit<F> {
        type Config = FloorConfig;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self {
                known: false,
                ..self.clone()
            }
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let value = meta.advice_column();
            let q_step = meta.selector();
            let instance = meta.instance_column();
            meta.enable_equality(value);
            meta.enable_equality(instance);
            meta.create_gate("M8 floor increment", |meta| {
                let q = meta.query_selector(q_step);
                let cur = meta.query_advice(value, Rotation::cur());
                let next = meta.query_advice(value, Rotation::next());
                vec![q * (next - cur - constant(F::ONE))]
            });
            FloorConfig {
                value,
                q_step,
                instance,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            let output = layouter.assign_region(
                || "M8 floor column",
                |mut region| {
                    let mut value = self.start;
                    let mut last = None;
                    for row in 0..self.rows {
                        if row + 1 < self.rows {
                            config.q_step.enable(&mut region, row)?;
                        }
                        last = Some(region.assign_advice_discarding_value(
                            config.value,
                            row,
                            witness(self.known, value),
                        ));
                        value += F::ONE;
                    }
                    Ok(last.expect("floor rows"))
                },
            )?;
            layouter.constrain_instance(output, config.instance, 0);
            Ok(())
        }
    }

    impl<F: KagemushaPoseidonFieldV1> RawCircuit<F> for FloorCircuit<F> {
        fn used_rows(&self) -> usize {
            self.rows
        }

        fn layout_fields(&self) -> String {
            " m8_lanes=1".to_owned()
        }
    }

    fn measure_floor(k: u32) {
        measure_raw::<EqAffine, FloorCircuit<Fp>>(&format!("m8_floor_custom_k{k}"), k, |seed| {
            let rows = (1_usize << k) - 16;
            let start = SplitMix64(seed).field::<Fp>();
            let last = start + Fp::from(rows as u64 - 1);
            (
                FloorCircuit {
                    rows,
                    start,
                    known: true,
                },
                vec![last],
            )
        });
    }

    m8_cases! {
        m8_floor_custom_k10 => measure_floor(10);
        m8_floor_custom_k11 => measure_floor(11);
        m8_floor_custom_k12 => measure_floor(12);
        m8_floor_custom_k13 => measure_floor(13);
        m8_floor_custom_k14 => measure_floor(14);
        m8_floor_custom_k15 => measure_floor(15);
        m8_floor_custom_k16 => measure_floor(16);
    }
}
