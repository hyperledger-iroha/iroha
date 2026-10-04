//! Golden proof bytes for the production KAGEMUSHA proving path.
//!
//! Every case builds a real circuit, generates its key with Halo2's consuming
//! keygen, and proves through the production state-carrier path
//! (`create_eq_proof_with_key_v1` / `create_ep_proof_with_key_v1`). That path
//! uses the recovery-seed ChaCha stream, the KAGEMUSHA Poseidon transcript and
//! the folded-generator augmentation. The SHA-256 of the augmented bytes must
//! equal a constant recorded from the unmodified prover on 2026-10-04.
//!
//! Each proof is also verified natively in full here. The test recomputes the
//! appended folded generator and rejects a wrong public input, so the full
//! check remains when augmentation itself becomes a succinct check.
//!
//! Prover kernel work must keep every constant unchanged. A changed constant
//! means a changed proof format, not a test to update.
//!
//! - `sigma_native_k11` (debug, 3,296 B): a sigma-send-shaped two-level state
//!   transition. It has 34 Poseidon permutations in two native Pasta lanes
//!   (degree 7), a halo2-base column with range lookups, copy constraints
//!   between the two, and two public instances.
//! - `p256_k16` (ignored, release, 10,112 B): one full-width low-S P-256 ECDSA
//!   verification.
//! - `rec_*_w1_k16` (ignored, release, 2,272 B and 4,768 B): a synthetic one-column
//!   k = 16 inner proof and the in-circuit scalar half of its succinct verification
//!   (snark-verifier loader, deferred curve equations, deferred-audit Poseidon digest).
//!
//! Run `cargo test -p iroha_core_zk --lib prover_golden_tests` for the debug case and
//! `cargo test --release -p iroha_core_zk --lib prover_golden_tests -- --include-ignored`
//! for all cases. Debug and release builds must produce the same bytes.

use std::{
    cell::Cell,
    convert::Infallible,
    io::{self, Cursor, Read},
    rc::Rc,
};

use halo2_base::{
    AssignedValue, Context,
    QuantumCell::{Constant, Witness},
    gates::{
        GateChip, GateInstructions, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt, ScalarField},
};
use halo2_ecc::{
    ecc::EcPoint,
    fields::{FieldChip, fp::FpChip},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::{
        CurveAffine, CurveExt,
        ff::{Field as _, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
        secp256r1::{Fp as P256Base, Fq as P256Scalar, Secp256r1Affine},
    },
    plonk::{
        Circuit, ConstraintSystem, Error as PlonkError, KeygenWithExtractorError, ProvingKey,
        VerifyingKey, keygen_pk2_consuming_with_profile, verify_proof,
    },
    poly::{
        VerificationStrategy,
        commitment::{MSM as _, Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            msm::MSMIPA,
            multiopen::VerifierIPA,
            strategy::GuardIPA,
        },
    },
};
use iroha_crypto::kagemusha::KagemushaRecoverySeedV1;
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::{halo2::Halo2Loader, native::NativeLoader},
    pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
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
use crate::kagemusha_v1_poseidon::hash as kagemusha_native_poseidon_hash;
use crate::kagemusha_v1_recursion::{
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KagemushaPoseidonFieldV1,
    create_state_carrier_ep_proof_for_golden_test_v1,
    create_state_carrier_eq_proof_for_golden_test_v1,
};
use crate::pasta_cycle_loader::{DeferredScalarEccChip, LIMB_BITS, LIMBS};
use crate::pasta_native_poseidon::{PastaNativePoseidonConfigV1, PastaNativePoseidonJobsV1};

/// SHA-256 of the augmented proof bytes recorded from the unmodified prover, keyed by case.
const GOLDEN_SHA256: &[(&str, &str)] = &[
    (
        "sigma_native_k11/eq",
        "98fea851f675250f16b9868bfd9c43e0d27eb648655ba68efb143a961e03c7f8",
    ),
    (
        "sigma_native_k11/ep",
        "1f795f96f8deda5e578f847ef0bfeeac9412ffea6a97cdf3085067c20e5f8a90",
    ),
    (
        "p256_k16/eq",
        "9e67d9164b86f06005fdde46356bd16883a4ea399582dad0924f456ad766a37c",
    ),
    (
        "p256_k16/ep",
        "e59e72fcb3280b3d3f05e82674981d2fd0f81e680a86fd882ecb08106988f817",
    ),
    (
        "rec_inner_w1_k16/eq",
        "b6fc7a0bd52e61c0e391159d9baaa55160aa5c5536f1ea5c8bb26fec05024e74",
    ),
    (
        "rec_scalar_half_w1_k16/eq",
        "aa0f10e0d00bdfaf29909526510d5fdd73f1bcbe147f94763ec29764df91cbd7",
    ),
    (
        "rec_inner_w1_k16/ep",
        "121aa6083eafe55ddf727aaac0ddde2a24fe2577a533e56f03d25d1e5aee577c",
    ),
    (
        "rec_scalar_half_w1_k16/ep",
        "850f41a1d817065712bf3bba88892df986ed5f63065b1bbbb3e264607641ca81",
    ),
];

/// Public fixture recovery seed; every golden proof uses the state-carrier label.
const GOLDEN_RECOVERY_SEED: [u8; 32] = [7; 32];
/// Rows reserved for blinding, as in the production and measurement circuits.
const MINIMUM_ROWS: usize = 9;
/// Witness seed of the keygen-mode build; keys must not depend on witnesses.
const KEYGEN_SEED: u64 = 0x676f_6c64_6b65_7967;
/// Witness seed of the proving build.
const PROVER_SEED: u64 = 0x676f_6c64_7072_6f76;
/// Compressed Pasta point appended by the augmentation.
const FOLDED_GENERATOR_BYTES: usize = 32;

/// Domain size of the sigma-shaped native-lane case.
const SIGMA_K: u32 = 11;
/// Native Poseidon lanes of the sigma case (34 permutations exceed one k11 lane).
const SIGMA_LANES: usize = 2;
/// Fields of the opened two-level state core.
const SIGMA_CORE_FIELDS: usize = 10;
const CORE_BALANCE: usize = 0;
const CORE_SEQUENCE: usize = 1;
const CORE_NEXT_SEND: usize = 2;
const CORE_SEND_CHAIN: usize = 4;
const CORE_STATE_NONCE: usize = 6;
const CORE_LIFECYCLE: usize = 7;
const CORE_POLICY_EPOCH: usize = 8;
const CORE_TIME_FLOOR: usize = 9;
const U64_BITS: usize = 64;
const U128_BITS: usize = 128;
const GOLDEN_STATE_DOMAIN: u64 = u64::from_le_bytes(*b"gldstat1");
const GOLDEN_REQUEST_DOMAIN: u64 = u64::from_le_bytes(*b"gldreqs1");
const GOLDEN_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"gldchan1");
const GOLDEN_STATEMENT_DOMAIN: u64 = u64::from_le_bytes(*b"gldstmt1");

/// Domain size of the release-only goldens.
const K16: u32 = 16;
/// Lookup-table bits used by the k = 16 cases.
const K16_LOOKUP_BITS: usize = 15;
/// Percentage of the single synthetic column left unused (break-point copies included).
const UNUSED_PERCENT_OF_ONE_COLUMN: usize = 2;
/// One range-checked multiplicand every this many synthetic chain steps.
const LOOKUP_EVERY_STEPS: usize = 2;

type KagemushaTranscript<C, S> = PoseidonTranscript<
    C,
    NativeLoader,
    S,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

/// Deterministic witness source (never the prover RNG).
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn next_u128(&mut self) -> u128 {
        (u128::from(self.next_u64()) << 64) | u128::from(self.next_u64())
    }

    fn field<F: FromUniformBytes<64>>(&mut self) -> F {
        let mut bytes = [0_u8; 64];
        for chunk in bytes.chunks_exact_mut(8) {
            chunk.copy_from_slice(&self.next_u64().to_le_bytes());
        }
        F::from_uniform_bytes(&bytes)
    }
}

/// The fixed public recovery seed of every golden proof.
fn golden_recovery_seed() -> KagemushaRecoverySeedV1 {
    KagemushaRecoverySeedV1::from_unsealed(GOLDEN_RECOVERY_SEED).expect("nonzero fixture seed")
}

/// Hex-encoded SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Compare `(case, proof)` digests with [`GOLDEN_SHA256`], reporting every mismatch at once.
fn assert_goldens(proofs: &[(&str, &[u8])]) {
    let mut mismatches = Vec::new();
    for (case, proof) in proofs {
        let digest = sha256_hex(proof);
        println!(
            "PROVER_GOLDEN case={case} bytes={} sha256={digest}",
            proof.len()
        );
        let expected = GOLDEN_SHA256
            .iter()
            .find(|(name, _)| name == case)
            .map(|(_, value)| *value);
        if expected != Some(digest.as_str()) {
            mismatches.push(format!("{case}: expected {expected:?}, got {digest}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "golden proof bytes changed:\n{}",
        mismatches.join("\n")
    );
}

/// Folded-generator strategy: full IPA verification that returns the folded SRS generator.
struct GoldenFoldedGenerator<'params, C: CurveAffine> {
    params: &'params ParamsIPA<C>,
}

impl<'params, C: CurveAffine>
    VerificationStrategy<'params, IPACommitmentScheme<C>, VerifierIPA<'params, C>>
    for GoldenFoldedGenerator<'params, C>
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

/// Fully verify an augmented proof and return whether its appended generator matches.
fn verify_augmented<C>(
    params: &ParamsIPA<C>,
    vk: &VerifyingKey<C>,
    proof: &[u8],
    public: &[C::ScalarExt],
) -> Result<bool, PlonkError>
where
    C: CurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
{
    let raw_len = proof
        .len()
        .checked_sub(FOLDED_GENERATOR_BYTES)
        .ok_or(PlonkError::ConstraintSystemFailure)?;
    let (raw, generator) = proof.split_at(raw_len);
    let columns: [&[C::ScalarExt]; 1] = [public];
    let instances: [&[&[C::ScalarExt]]; 1] = [&columns];
    let mut cursor = Cursor::new(raw);
    let mut transcript =
        KagemushaTranscript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    let folded = verify_proof::<
        IPACommitmentScheme<C>,
        VerifierIPA<'_, C>,
        ChallengeScalar<C>,
        _,
        GoldenFoldedGenerator<'_, C>,
    >(
        params,
        vk,
        GoldenFoldedGenerator { params },
        &instances,
        &mut transcript,
    )?;
    drop(transcript);
    let consumed_all = usize::try_from(cursor.position()).ok() == Some(raw_len);
    Ok(consumed_all && folded.to_bytes().as_ref() == generator)
}

/// One golden proof with what its verification and recursion need.
struct GoldenProof<C: CurveAffine> {
    params: ParamsIPA<C>,
    vk: VerifyingKey<C>,
    /// Augmented proof: raw KAGEMUSHA transcript plus the folded generator.
    proof: Vec<u8>,
    public: Vec<C::ScalarExt>,
}

/// A golden circuit: a Base builder plus any extra synthesized regions.
trait GoldenCircuit<F: ScalarField>: Circuit<F> {
    /// The Base builder holding the virtual graph, break points and public instances.
    fn base(&self) -> &BaseCircuitBuilder<F>;
    /// Mutable Base builder, used to calculate parameters before key generation.
    fn base_mut(&mut self) -> &mut BaseCircuitBuilder<F>;
}

impl<F: ScalarField> GoldenCircuit<F> for BaseCircuitBuilder<F> {
    fn base(&self) -> &BaseCircuitBuilder<F> {
        self
    }

    fn base_mut(&mut self) -> &mut BaseCircuitBuilder<F> {
        self
    }
}

/// Generate keys, prove through `prove` and verify the augmented proof natively.
///
/// `make` builds the relation into a keygen-mode Base builder, then into a witness-only
/// builder pinned to the keygen parameters and break points.
fn golden_proof<C, Circ>(
    k: u32,
    lookup_bits: usize,
    params: Option<ParamsIPA<C>>,
    make: impl Fn(BaseCircuitBuilder<C::ScalarExt>, u64) -> (Circ, Vec<C::ScalarExt>),
    prove: impl FnOnce(&ParamsIPA<C>, &ProvingKey<C>, Circ, &[C::ScalarExt]) -> Vec<u8>,
) -> GoldenProof<C>
where
    C: CurveAffine,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
    Circ: GoldenCircuit<C::ScalarExt>,
{
    let params = params.unwrap_or_else(|| ParamsIPA::<C>::new(k));
    assert_eq!(params.k(), k, "golden parameters have the wrong k");
    let mut keygen_base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(usize::try_from(k).expect("k fits usize"))
        .use_instance_columns(1);
    keygen_base.set_lookup_bits(lookup_bits);
    let (mut keygen_circuit, keygen_public) = make(keygen_base, KEYGEN_SEED);
    let circuit_params = keygen_circuit
        .base_mut()
        .calculate_params(Some(MINIMUM_ROWS));
    let (pk, break_points) = match keygen_pk2_consuming_with_profile(
        &params,
        keygen_circuit,
        true,
        |circuit: &Circ, _profile| Ok::<_, Infallible>(circuit.base().break_points()),
    ) {
        Ok(generated) => generated,
        Err(KeygenWithExtractorError::Keygen(error)) => panic!("golden keygen failed: {error}"),
        Err(KeygenWithExtractorError::Extractor(never)) => match never {},
    };
    let (prover, public) = make(
        BaseCircuitBuilder::<C::ScalarExt>::prover(circuit_params, break_points),
        PROVER_SEED,
    );
    assert_eq!(public.len(), keygen_public.len(), "golden instance shape");
    let proof = prove(&params, &pk, prover, &public);
    let vk = pk.get_vk().clone();
    drop(pk);
    assert!(
        matches!(verify_augmented(&params, &vk, &proof, &public), Ok(true)),
        "golden proof must verify with its appended folded generator"
    );
    let mut wrong_public = public.clone();
    wrong_public[0] += C::ScalarExt::ONE;
    assert!(
        !matches!(
            verify_augmented(&params, &vk, &proof, &wrong_public),
            Ok(true)
        ),
        "golden proof must reject a wrong public input"
    );
    GoldenProof {
        params,
        vk,
        proof,
        public,
    }
}

/// Prove an Eq circuit through the production state-carrier path.
fn prove_eq<Circ: Circuit<Fp>>(
    seed: &KagemushaRecoverySeedV1,
) -> impl FnOnce(&ParamsIPA<EqAffine>, &ProvingKey<EqAffine>, Circ, &[Fp]) -> Vec<u8> + '_ {
    move |params, pk, circuit, public| {
        create_state_carrier_eq_proof_for_golden_test_v1(params, pk, circuit, public, seed)
            .unwrap_or_else(|error| panic!("golden Eq proof: {error}"))
    }
}

/// Prove an Ep circuit through the production state-carrier path.
fn prove_ep<Circ: Circuit<Fq>>(
    seed: &KagemushaRecoverySeedV1,
) -> impl FnOnce(&ParamsIPA<EpAffine>, &ProvingKey<EpAffine>, Circ, &[Fq]) -> Vec<u8> + '_ {
    move |params, pk, circuit, public| {
        create_state_carrier_ep_proof_for_golden_test_v1(params, pk, circuit, public, seed)
            .unwrap_or_else(|error| panic!("golden Ep proof: {error}"))
    }
}

// ---------------------------------------------------------------------------
// Sigma-shaped native-lane composite (k = 11).
// ---------------------------------------------------------------------------

/// Base parameters plus the native lane count.
#[derive(Clone, Debug, Default)]
struct GoldenSigmaParams {
    base: BaseCircuitParams,
    lanes: usize,
}

/// Base plus native Pasta Poseidon lane configuration.
#[derive(Clone, Debug)]
struct GoldenSigmaConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

/// A sigma-send-shaped relation whose permutations run in the native Pasta Poseidon lanes.
#[derive(Clone)]
struct GoldenSigmaCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
    lanes: usize,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for GoldenSigmaCircuit<F> {
    type Config = GoldenSigmaConfig<F>;
    type FloorPlanner = V1;
    type Params = GoldenSigmaParams;

    fn params(&self) -> Self::Params {
        GoldenSigmaParams {
            base: self.base.config_params.clone(),
            lanes: self.lanes,
        }
    }

    fn without_witnesses(&self) -> Self {
        Self {
            base: self.base.deep_clone().unknown(true),
            jobs: self.jobs.clone().unknown(),
            lanes: self.lanes,
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("golden sigma circuits are configured from parameters")
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.base.k) - MINIMUM_ROWS;
        let mut base = BaseConfig::configure(meta, params.base);
        base.set_usable_rows(usable_rows);
        GoldenSigmaConfig {
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
            layouter.namespace(|| "golden sigma Base"),
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

impl<F: KagemushaPoseidonFieldV1> GoldenCircuit<F> for GoldenSigmaCircuit<F> {
    fn base(&self) -> &BaseCircuitBuilder<F> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseCircuitBuilder<F> {
        &mut self.base
    }
}

/// `H(domain, arity, inputs)` in the native lanes, asserted against the native hash.
fn golden_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &GateChip<F>,
    jobs: &mut PastaNativePoseidonJobsV1<F>,
    domain: u64,
    inputs: &[AssignedValue<F>],
) -> AssignedValue<F> {
    let arity = u64::try_from(inputs.len()).expect("bounded Poseidon arity");
    let prefix = [F::from(domain), F::from(arity)];
    let capacity = inputs
        .len()
        .checked_add(prefix.len())
        .expect("bounded Poseidon input length");
    let mut cells = Vec::with_capacity(capacity);
    cells.extend(prefix.map(|value| ctx.load_constant(value)));
    cells.extend_from_slice(inputs);
    let digest = jobs
        .queue_raw(ctx, gate, cells, &prefix)
        .unwrap_or_else(|error| panic!("native Poseidon lanes: {error}"));
    let values = inputs.iter().map(|cell| *cell.value()).collect::<Vec<_>>();
    assert_eq!(
        *digest.value(),
        kagemusha_native_poseidon_hash::<F>(domain, &values),
        "in-circuit Poseidon digest equals the native hash"
    );
    digest
}

/// Two 128-bit limbs bound only through the public digests.
fn golden_limbs<F: ScalarField>(
    ctx: &mut Context<F>,
    rng: &mut SplitMix64,
) -> [AssignedValue<F>; 2] {
    [rng.next_u128(), rng.next_u128()].map(|limb| ctx.load_witness(F::from_u128(limb)))
}

/// Constrain `low <= high` for cells already range checked to 64 bits.
fn assert_at_most<F: ScalarField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    low: AssignedValue<F>,
    high: AssignedValue<F>,
) {
    let difference = range.gate().sub(ctx, high, low);
    range.range_check(ctx, difference, U64_BITS);
}

/// Sigma-send-shaped relation over a two-level state (one parity).
///
/// It opens the predecessor `H(core10, rest)`, checks u128 debit arithmetic, a u64 sequence
/// and send-ordinal increment, a request policy epoch and time window, then binds a request
/// digest, a send-chain append, the successor commitment and a statement digest. Public: the
/// statement digest and the request digest.
fn sigma_send_relation<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaNativePoseidonJobsV1<F>,
    seed: u64,
) -> Vec<F> {
    let range = builder.range_chip();
    let gate = range.gate();
    let mut rng = SplitMix64(seed);
    let ctx = builder.main(0);

    let amount = u128::from(rng.next_u64() >> 8) + 1;
    let fee = u128::from(rng.next_u64() >> 40);
    let balance = (1_u128 << 100) | (rng.next_u128() >> 30);
    let policy_epoch = (rng.next_u64() >> 24) + 256;
    let time_floor = (rng.next_u64() >> 24) + 1;
    let core_values: [F; SIGMA_CORE_FIELDS] = [
        F::from_u128(balance),
        F::from(rng.next_u64() >> 1),
        F::from(rng.next_u64() >> 1),
        F::from(rng.next_u64()),
        rng.field(),
        rng.field(),
        rng.field(),
        F::ONE,
        F::from(policy_epoch),
        F::from(time_floor),
    ];
    let core = core_values.map(|value| ctx.load_witness(value));
    let rest_digest = ctx.load_witness(rng.field());
    gate.assert_is_const(ctx, &core[CORE_LIFECYCLE], &F::ONE);
    let mut opening = core.to_vec();
    opening.push(rest_digest);
    let predecessor = golden_hash(ctx, gate, jobs, GOLDEN_STATE_DOMAIN, &opening);

    range.range_check(ctx, core[CORE_BALANCE], U128_BITS);
    for index in [
        CORE_SEQUENCE,
        CORE_NEXT_SEND,
        CORE_POLICY_EPOCH,
        CORE_TIME_FLOOR,
    ] {
        range.range_check(ctx, core[index], U64_BITS);
    }
    let amount_cell = ctx.load_witness(F::from_u128(amount));
    range.range_check(ctx, amount_cell, U128_BITS);
    let amount_is_zero = gate.is_zero(ctx, amount_cell);
    gate.assert_is_const(ctx, &amount_is_zero, &F::ZERO);
    let fee_cell = ctx.load_witness(F::from_u128(fee));
    range.range_check(ctx, fee_cell, U128_BITS);
    let debit = gate.add(ctx, amount_cell, fee_cell);
    let balance_after = gate.sub(ctx, core[CORE_BALANCE], debit);
    range.range_check(ctx, balance_after, U128_BITS);
    let sequence_after = gate.add(ctx, core[CORE_SEQUENCE], Constant(F::ONE));
    range.range_check(ctx, sequence_after, U64_BITS);
    let next_send_after = gate.add(ctx, core[CORE_NEXT_SEND], Constant(F::ONE));
    range.range_check(ctx, next_send_after, U64_BITS);
    let request_epoch = ctx.load_witness(F::from(policy_epoch - (rng.next_u64() & 0xff)));
    range.range_check(ctx, request_epoch, U64_BITS);
    assert_at_most(ctx, &range, request_epoch, core[CORE_POLICY_EPOCH]);
    let request_time = ctx.load_witness(F::from(time_floor + (rng.next_u64() & 0xffff)));
    range.range_check(ctx, request_time, U64_BITS);
    assert_at_most(ctx, &range, core[CORE_TIME_FLOOR], request_time);

    let version = ctx.load_constant(F::ONE);
    let asset = golden_limbs(ctx, &mut rng);
    let payer = golden_limbs(ctx, &mut rng);
    let receiver = golden_limbs(ctx, &mut rng);
    let nonce = golden_limbs(ctx, &mut rng);
    let request = [
        version,
        asset[0],
        asset[1],
        payer[0],
        payer[1],
        receiver[0],
        receiver[1],
        core[CORE_NEXT_SEND],
        amount_cell,
        fee_cell,
        request_epoch,
        request_time,
        nonce[0],
        nonce[1],
    ];
    let request_digest = golden_hash(ctx, gate, jobs, GOLDEN_REQUEST_DOMAIN, &request);
    let send_chain_after = golden_hash(
        ctx,
        gate,
        jobs,
        GOLDEN_CHAIN_DOMAIN,
        &[
            core[CORE_SEND_CHAIN],
            request_digest,
            receiver[0],
            receiver[1],
            core[CORE_NEXT_SEND],
            amount_cell,
            fee_cell,
        ],
    );
    let mut successor = core;
    successor[CORE_BALANCE] = balance_after;
    successor[CORE_SEQUENCE] = sequence_after;
    successor[CORE_NEXT_SEND] = next_send_after;
    successor[CORE_SEND_CHAIN] = send_chain_after;
    successor[CORE_STATE_NONCE] = ctx.load_witness(rng.field());
    let mut successor_opening = successor.to_vec();
    successor_opening.push(rest_digest);
    let successor_digest = golden_hash(ctx, gate, jobs, GOLDEN_STATE_DOMAIN, &successor_opening);
    let statement = golden_hash(
        ctx,
        gate,
        jobs,
        GOLDEN_STATEMENT_DOMAIN,
        &[
            version,
            predecessor,
            successor_digest,
            request_digest,
            amount_cell,
            fee_cell,
            receiver[0],
            receiver[1],
            sequence_after,
        ],
    );
    builder.assigned_instances[0].extend([statement, request_digest]);
    vec![*statement.value(), *request_digest.value()]
}

/// Build the sigma composite at k = 11 into `base`.
fn sigma_circuit<F: KagemushaPoseidonFieldV1>(
    mut base: BaseCircuitBuilder<F>,
    seed: u64,
) -> (GoldenSigmaCircuit<F>, Vec<F>) {
    let usable_rows = (1_usize << SIGMA_K) - MINIMUM_ROWS;
    let mut jobs = PastaNativePoseidonJobsV1::new(SIGMA_LANES, usable_rows)
        .unwrap_or_else(|error| panic!("native Poseidon lane envelope: {error}"));
    let public = sigma_send_relation(&mut base, &mut jobs, seed);
    let circuit = GoldenSigmaCircuit {
        base,
        jobs,
        lanes: SIGMA_LANES,
    };
    (circuit, public)
}

/// Lookup bits of the sigma case (halo2-base single-column lookups at k - 1).
fn sigma_lookup_bits() -> usize {
    usize::try_from(SIGMA_K - 1).expect("k fits usize")
}

#[test]
fn sigma_native_k11_augmented_proof_bytes_are_golden() {
    let seed = golden_recovery_seed();
    let eq = golden_proof::<EqAffine, _>(
        SIGMA_K,
        sigma_lookup_bits(),
        None,
        sigma_circuit::<Fp>,
        prove_eq(&seed),
    );
    let ep = golden_proof::<EpAffine, _>(
        SIGMA_K,
        sigma_lookup_bits(),
        None,
        sigma_circuit::<Fq>,
        prove_ep(&seed),
    );
    assert_goldens(&[
        ("sigma_native_k11/eq", eq.proof.as_slice()),
        ("sigma_native_k11/ep", ep.proof.as_slice()),
    ]);
}

// ---------------------------------------------------------------------------
// P-256 ECDSA at k = 16 (release).
// ---------------------------------------------------------------------------

/// One full-width low-S P-256 ECDSA verification (the gadget inventory construction).
///
/// `r = s = z = x(2G)` with public key `Q = G` gives `u1 = u2 = 1`, so `R = 2G` and
/// `x(R) = r`: a valid low-S signature over the 32-byte digest.
fn p256_ecdsa_low_s<F: BigPrimeField>(builder: &mut BaseCircuitBuilder<F>) -> Vec<F> {
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

/// Build the P-256 relation into `builder`; the witness is fixed, so the seed is unused.
fn p256_circuit<F: BigPrimeField>(
    mut builder: BaseCircuitBuilder<F>,
    _seed: u64,
) -> (BaseCircuitBuilder<F>, Vec<F>) {
    let public = p256_ecdsa_low_s(&mut builder);
    (builder, public)
}

#[test]
#[ignore = "k = 16 prover golden; run in release"]
fn p256_k16_augmented_proof_bytes_are_golden() {
    let seed = golden_recovery_seed();
    let eq = golden_proof::<EqAffine, _>(
        K16,
        K16_LOOKUP_BITS,
        None,
        p256_circuit::<Fp>,
        prove_eq(&seed),
    );
    let ep = golden_proof::<EpAffine, _>(
        K16,
        K16_LOOKUP_BITS,
        None,
        p256_circuit::<Fq>,
        prove_ep(&seed),
    );
    assert_goldens(&[
        ("p256_k16/eq", eq.proof.as_slice()),
        ("p256_k16/ep", ep.proof.as_slice()),
    ]);
}

// ---------------------------------------------------------------------------
// Recursion scalar half of a one-column k = 16 inner proof (release).
// ---------------------------------------------------------------------------

/// Chained `acc + x * y = acc'` gates filling `width` Base columns, with range lookups.
fn chained_mul_add<F: BigPrimeField>(
    builder: &mut BaseCircuitBuilder<F>,
    k: u32,
    width: usize,
    seed: u64,
) -> Vec<F> {
    let usable_rows = (1_usize << k) - MINIMUM_ROWS;
    let unused_rows = usable_rows * UNUSED_PERCENT_OF_ONE_COLUMN / 100;
    let steps = (width * usable_rows - unused_rows - 1) / 3;
    let lookup_mask = (1_u64 << K16_LOOKUP_BITS) - 1;
    let range = builder.range_chip();
    let mut rng = SplitMix64(seed);
    let ctx = builder.main(0);
    let mut acc = F::from(7_u64);
    let _constant = ctx.load_constant(acc);
    for step in 0..steps {
        let checked = step % LOOKUP_EVERY_STEPS == 0;
        let x = if checked {
            F::from(rng.next_u64() & lookup_mask)
        } else {
            rng.field()
        };
        let y: F = rng.field();
        acc += x * y;
        ctx.assign_region([Witness(x), Witness(y), Witness(acc)], [-1]);
        if checked {
            let x_cell = ctx.get(-3);
            range.range_check(ctx, x_cell, K16_LOOKUP_BITS);
        }
    }
    let output = ctx.get(-1);
    builder.assigned_instances[0].push(output);
    vec![acc]
}

/// Build the one-column synthetic inner relation into `builder`.
fn synthetic_w1_circuit<F: BigPrimeField>(
    mut builder: BaseCircuitBuilder<F>,
    seed: u64,
) -> (BaseCircuitBuilder<F>, Vec<F>) {
    let public = chained_mul_add(&mut builder, K16, 1, seed);
    (builder, public)
}

/// Succinct verifying key of the KAGEMUSHA IPA parameters.
fn succinct_vk<C: CurveAffine>(params: &ParamsIPA<C>) -> IpaSuccinctVerifyingKey<C> {
    let hash_to_curve = <C::CurveExt as CurveExt>::hash_to_curve("Halo2-Parameters");
    IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

/// Proof reader recording how many bytes the in-circuit transcript consumed.
struct CountingReader<'proof> {
    bytes: &'proof [u8],
    position: Rc<Cell<usize>>,
}

impl Read for CountingReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let start = self.position.get();
        let available = &self.bytes[start..];
        let len = available.len().min(output.len());
        output[..len].copy_from_slice(&available[..len]);
        self.position.set(start + len);
        Ok(len)
    }
}

type DeferredLoader<'chip, C> = Rc<Halo2Loader<C, DeferredScalarEccChip<'chip, C>>>;
type DeferredScalar<'chip, C> =
    snark_verifier::loader::halo2::Scalar<C, DeferredScalarEccChip<'chip, C>>;
type DeferredTranscript<'chip, 'proof, C> = PoseidonTranscript<
    C,
    DeferredLoader<'chip, C>,
    CountingReader<'proof>,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

/// Field-native Poseidon digest through the scalar-half loader (as `deferred_parent`).
fn loader_poseidon_digest<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    elements: Vec<AssignedValue<C::ScalarExt>>,
) -> AssignedValue<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    let elements = elements
        .into_iter()
        .map(|element| loader.scalar_from_assigned(element))
        .collect::<Vec<_>>();
    let mut poseidon = LoaderPoseidon::<
        C::ScalarExt,
        DeferredScalar<'chip, C>,
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

/// Scalar half of one in-circuit succinct verification of `inner` (same parity).
///
/// Mirrors `deferred_parent::verify_ordinary_proof_v1`: the augmented proof is parsed
/// through the fixed Poseidon transcript, challenges and scalar arithmetic are constrained
/// in this field, and curve operations become deferred equations whose production audit
/// digest is public together with the final transcript squeeze. History folding is not
/// included.
fn scalar_half<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    inner: &GoldenProof<C>,
    svk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C>,
) -> Vec<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let loader: DeferredLoader<'_, C> = Halo2Loader::new(
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
        let reader = CountingReader {
            bytes: &inner.proof,
            position: Rc::clone(&position),
        };
        let mut transcript =
            DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&loader, reader);
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
            "in-circuit transcript must consume the whole augmented proof"
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
        let digest = loader_poseidon_digest(&loader, elements);
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

/// Prove the inner one-column proof and its scalar half; return both augmented proofs.
fn recursion_scalar_half_w1<C>(
    prove_inner: impl FnOnce(
        &ParamsIPA<C>,
        &ProvingKey<C>,
        BaseCircuitBuilder<C::ScalarExt>,
        &[C::ScalarExt],
    ) -> Vec<u8>,
    prove_outer: impl FnOnce(
        &ParamsIPA<C>,
        &ProvingKey<C>,
        BaseCircuitBuilder<C::ScalarExt>,
        &[C::ScalarExt],
    ) -> Vec<u8>,
) -> (Vec<u8>, Vec<u8>)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
{
    let inner = golden_proof::<C, _>(
        K16,
        K16_LOOKUP_BITS,
        None,
        synthetic_w1_circuit::<C::ScalarExt>,
        prove_inner,
    );
    let svk = succinct_vk(&inner.params);
    let protocol = compile(
        &inner.params,
        &inner.vk,
        Config::ipa().with_num_instance(vec![inner.public.len()]),
    );
    let outer = golden_proof::<C, _>(
        K16,
        K16_LOOKUP_BITS,
        Some(inner.params.clone()),
        |mut builder, _seed| {
            let public = scalar_half(&mut builder, &inner, &svk, &protocol);
            (builder, public)
        },
        prove_outer,
    );
    (inner.proof, outer.proof)
}

#[test]
#[ignore = "k = 16 prover golden; run in release"]
fn recursion_scalar_half_w1_k16_augmented_proof_bytes_are_golden() {
    let seed = golden_recovery_seed();
    let (eq_inner, eq_outer) =
        recursion_scalar_half_w1::<EqAffine>(prove_eq(&seed), prove_eq(&seed));
    let (ep_inner, ep_outer) =
        recursion_scalar_half_w1::<EpAffine>(prove_ep(&seed), prove_ep(&seed));
    assert_goldens(&[
        ("rec_inner_w1_k16/eq", eq_inner.as_slice()),
        ("rec_scalar_half_w1_k16/eq", eq_outer.as_slice()),
        ("rec_inner_w1_k16/ep", ep_inner.as_slice()),
        ("rec_scalar_half_w1_k16/ep", ep_outer.as_slice()),
    ]);
}

#[test]
fn golden_table_names_are_unique_and_well_formed() {
    let mut names = GOLDEN_SHA256
        .iter()
        .map(|(name, digest)| {
            assert_eq!(digest.len(), 64, "{name}: SHA-256 hex length");
            assert!(
                digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{name}: SHA-256 hex digits"
            );
            *name
        })
        .collect::<Vec<_>>();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "duplicate golden case names");
}

#[test]
fn golden_witness_stream_is_deterministic_and_seed_separated() {
    let mut first = SplitMix64(KEYGEN_SEED);
    let mut again = SplitMix64(KEYGEN_SEED);
    let mut other = SplitMix64(PROVER_SEED);
    let first_values = [first.next_u64(), first.next_u64()];
    assert_eq!(first_values, [again.next_u64(), again.next_u64()]);
    assert_ne!(first_values, [other.next_u64(), other.next_u64()]);
    assert_eq!(first.field::<Fp>(), again.field::<Fp>());
}
