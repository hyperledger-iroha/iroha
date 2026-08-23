//! Released Microsoft Vega-MC proof boundary for the Figure 9 mDL relation.
use super::{
    MAX_VEGA_PROOF_BYTES_V1, VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaT256ScalarV1 as Scalar,
    figure9::{VEGA_MDL_FIGURE9_SHA256_STEPS_V1, VegaMdlFigure9WitnessV1},
};
use thiserror::Error;
/// Exact external privacy protocol label absorbed through the core public context values.
pub const VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1: &[u8] = b"vega-existing-credential-zk-v0";
/// Sole privacy-action index admitted by the first-release Vega profile.
pub const VEGA_MDL_ACTION_INDEX_V1: u32 = 0;
/// Exact transcript persona used by the pinned Microsoft implementation.
pub const VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1: &[u8] = b"neutronnova_prove";
/// Keccak-256 digest of the complete Figure 9 relation before its canonical two-hash MC split.
pub const VEGA_MDL_CANONICAL_RELATION_DIGEST_V1: [u8; 32] = [
    0x8b, 0xf6, 0xa3, 0x11, 0x20, 0x6e, 0xf6, 0x78, 0x9b, 0x2b, 0x3d, 0x61, 0x3b, 0x4e, 0x98, 0xb9,
    0xfd, 0xc5, 0x8a, 0xcd, 0x02, 0x37, 0x3a, 0x9d, 0xbc, 0x2b, 0x7b, 0x64, 0xcb, 0x7e, 0xdf, 0xbc,
];
/// SHA-256 digest of the exact canonical Microsoft Vega-MC verifier key.
pub const VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1: [u8; 32] = [
    0x86, 0xd0, 0xce, 0x5b, 0x22, 0xf4, 0x63, 0x78, 0x5d, 0x07, 0x93, 0x60, 0x34, 0xdd, 0xef, 0xc4,
    0x87, 0x46, 0x16, 0x34, 0xf2, 0xfe, 0x2c, 0xfc, 0x47, 0x0b, 0xf3, 0x0b, 0xde, 0x3d, 0x68, 0x27,
];
/// Canonical, newline-delimited manifest whose Keccak-256 is the compiled profile digest.
pub const VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1: &[u8] = concat!(
    "iroha.vega.figure9.microsoft-mc.compiled-profile.v1\n",
    "upstream_commit=c0ee259053cd12eaf43ed71b5cde375452b3ee4d\n",
    "upstream_tree=7226b6cbfbfe8613dd2d5ee831096b7578a5c115\n",
    "vendor_manifest_sha256=539c54251c8853fa99673e71d777966a3e3e238e64028d47b3e683329023236f\n",
    "relation_keccak256=8bf6a311206ef6789b2b3d613b4e98b9fdc58acd02373a9dbc2b7b64cb7edfbc\n",
    "adapter=shared-witness-core+8-uniform-sha256-compression-steps\n",
    "sha256_steps=birth:2,issuer:6,total:8\n",
    "proof_wire=bincode-1.3.3-fixed-little-endian\n",
    "envelope=IROVEGMC,version:1,context-keccak256:32\n",
    "max_envelope_bytes=524288\n",
    "verifier_sha256=86d0ce5b22f463785d07936034ddefc487461634f2fe2cfc470bf30bde3d6827\n",
    "dimensions=steps:8,shared-vars:524288,step-pre-vars:2048,step-rest-vars:522240,core-pre-vars:2048,core-rest-vars:522240,step-cons:262144,step-vars:1048576,core-cons:262144,core-vars:1048576,shared-points:256,step-pre-points:1,step-rest-points:255,step-public:1,step-challenges:0,core-pre-points:1,core-rest-points:255,core-public:18,core-challenges:0,ipa-response-scalars:2048,verifier-rounds:47,verifier-public:6,nova-cross-points:16,random-witness-points:47,random-error-points:16,random-public:49,verifier-cons:512,verifier-vars:1504,outer-rounds:9,outer-coefficients:3,inner-rounds:12,inner-coefficients:2,opening-scalars:32\n",
    "verifier-round-points=1*47\n",
    "verifier-challenges=1,1,1,0,1*40,0,0,0\n",
    "rng=fallible-external-seed32+proof-scoped-std-rng+rayon-shared+no-ambient-fallback+constant-or-immediate-reuse-reject+panic-to-error\n",
)
.as_bytes();
/// Digest of the released canonical MC adapter/profile manifest.
///
/// This value is intentionally versioned independently from the full relation
/// digest because the 2+6 uniform SHA split, verifier circuit, proof wire, and
/// patched fallible RNG boundary are part of the compiled profile.
pub const VEGA_MDL_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0xe7, 0x54, 0xae, 0xbc, 0x68, 0xf6, 0x44, 0x01, 0xb5, 0x89, 0x19, 0x83, 0xfb, 0xae, 0xff, 0x81,
    0xbc, 0x4b, 0x4d, 0x59, 0x92, 0x1a, 0x72, 0xc3, 0xa6, 0x2a, 0xa9, 0x9a, 0x19, 0x26, 0x0a, 0x2a,
];
/// Hard release cap for caller-selected Vega workers.
pub const MAX_VEGA_PROVER_WORKERS_V1: usize = 20;
/// Conservative resident-memory admission budget for one canonical MC proof.
pub const VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1: usize = 2 * 1024 * 1024 * 1024;
const PROVER_WORKER_MEMORY_BOUND_BYTES: usize = 768 * 1024;
/// Largest released per-proof memory ceiling at twenty workers.
pub const MAX_VEGA_PROVER_RELEASE_MEMORY_CEILING_BYTES_V1: usize =
    VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1
        + MAX_VEGA_PROVER_WORKERS_V1 * PROVER_WORKER_MEMORY_BOUND_BYTES;
/// Explicit failure returned by an injected cryptographic random source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaRandomSourceErrorV1 {
    /// The operating-system or hardware random source was unavailable.
    #[error("Vega cryptographic random source is unavailable")]
    Unavailable,
}
/// Fallible random-byte source used to seed the proof-scoped canonical Vega CSPRNG.
pub trait VegaRandomSourceV1 {
    /// Fill the entire destination or return an error.
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1>;
}
/// Explicit bounded worker configuration for the canonical Vega-MC prover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlProverConfigV1 {
    worker_count: usize,
}
impl VegaMdlProverConfigV1 {
    /// Select the exact Rayon pool size used by setup-independent preparation and proving work.
    ///
    /// # Errors
    ///
    /// Counts outside `1..=20` are rejected.
    pub const fn new(worker_count: usize) -> Result<Self, VegaMdlProofErrorV1> {
        if worker_count == 0 || worker_count > MAX_VEGA_PROVER_WORKERS_V1 {
            return Err(VegaMdlProofErrorV1::InvalidWorkerCount {
                actual: worker_count,
                min: 1,
                max: MAX_VEGA_PROVER_WORKERS_V1,
            });
        }
        Ok(Self { worker_count })
    }
    /// Exact number of prover workers.
    #[must_use]
    pub const fn worker_count(self) -> usize {
        self.worker_count
    }
    /// Conservative worker-local scratch bound.
    #[must_use]
    pub const fn commitment_worker_scratch_bound_bytes(self) -> usize {
        self.worker_count * PROVER_WORKER_MEMORY_BOUND_BYTES
    }
    /// Conservative release-mode resident-memory admission ceiling.
    #[must_use]
    pub const fn release_memory_ceiling_bytes(self) -> usize {
        VEGA_PROVER_SHARED_MEMORY_BOUND_BYTES_V1 + self.commitment_worker_scratch_bound_bytes()
    }
}
/// Consensus context bound as four exact public scalars in the MC core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlProofContextV1<'a> {
    /// Exact chain identifier bytes.
    pub chain_id: &'a [u8],
    /// Independently trusted genesis hash.
    pub genesis_hash: [u8; 32],
    /// Zero-based privacy action index; the released value is exactly zero.
    pub action_index: u32,
    /// Governed parameter identifier.
    pub parameter_id: [u8; 32],
    /// Governed parameter digest.
    pub parameter_digest: [u8; 32],
    /// Digest of the exact canonical Microsoft Vega-MC verifier key.
    pub verifier_digest: [u8; 32],
    /// Digest of the typed statement schema.
    pub statement_schema_digest: [u8; 32],
    /// Digest of the native engine manifest.
    pub engine_manifest_digest: [u8; 32],
}
/// Failure while proving, decoding, or verifying the released Vega proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaMdlProofErrorV1 {
    /// The requested prover worker count is outside the released bound.
    #[error("Vega prover worker count {actual} is outside {min}..={max}")]
    InvalidWorkerCount {
        /// Requested workers.
        actual: usize,
        /// Inclusive minimum.
        min: usize,
        /// Inclusive maximum.
        max: usize,
    },
    /// A consensus context field is empty, oversized, zero, or mismatched.
    #[error("invalid Vega proof consensus context")]
    InvalidContext,
    /// The private assignment failed the complete Figure 9 relation.
    #[error("Vega Figure 9 witness is unsatisfied")]
    UnsatisfiedWitness,
    /// The injected source failed.
    #[error(transparent)]
    RandomSource(#[from] VegaRandomSourceErrorV1),
    /// The source returned a constant seed or immediately reused a seed.
    #[error("Vega prover randomness is degenerate or reused")]
    DegenerateRandomness,
    /// Deterministic synthesis, setup, or the governed compiled profile failed.
    #[error("Vega compiled proof profile is invalid")]
    InvalidCompiledProfile,
    /// The proof exceeded the Vega-specific hard cap.
    #[error("Vega proof length {actual} exceeds hard maximum {max}")]
    ProofTooLarge {
        /// Actual encoded length.
        actual: usize,
        /// Released maximum.
        max: usize,
    },
    /// Structural pre-scan, bincode decode, or canonical re-encoding failed.
    #[error("invalid canonical Vega proof encoding")]
    InvalidProofEncoding,
    /// The canonical proof equations or public-output gates failed.
    #[error("Vega proof verification failed")]
    VerificationFailed,
}
/// Exact verifier-key-derived Microsoft Vega-MC proof dimensions.
///
/// This is an owned first-party view of every sequence bound carried by the canonical proof.
/// Keeping the dimensions in the public facade avoids making the released API depend on the
/// implementation crate used to validate the original Microsoft vectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VegaMdlProofDimensionsV1 {
    /// Number of uniform step instances.
    pub num_steps: usize,
    /// Padded shared-witness variables reused by every instance.
    pub shared_variables: usize,
    /// Padded precommitted variables in each step instance.
    pub step_precommitted_variables: usize,
    /// Padded remaining variables in each step instance.
    pub step_rest_variables: usize,
    /// Padded precommitted variables in the core instance.
    pub core_precommitted_variables: usize,
    /// Padded remaining variables in the core instance.
    pub core_rest_variables: usize,
    /// Padded constraints in the uniform step shape.
    pub step_constraints: usize,
    /// Padded variables in the uniform step shape.
    pub step_variables: usize,
    /// Padded constraints in the core shape.
    pub core_constraints: usize,
    /// Padded variables in the core shape.
    pub core_variables: usize,
    /// Points in the hoisted shared-witness commitment.
    pub shared_commitment_points: usize,
    /// Points in each step precommitted commitment.
    pub step_precommitted_points: usize,
    /// Points in each step remaining-witness commitment.
    pub step_rest_points: usize,
    /// Public values in each step instance.
    pub step_public_values: usize,
    /// Fiat--Shamir challenges in each step instance.
    pub step_challenges: usize,
    /// Points in the core precommitted commitment.
    pub core_precommitted_points: usize,
    /// Points in the core remaining-witness commitment.
    pub core_rest_points: usize,
    /// Public values in the core instance.
    pub core_public_values: usize,
    /// Fiat--Shamir challenges in the core instance.
    pub core_challenges: usize,
    /// Scalars in the Hyrax linear IPA response vector.
    pub evaluation_response_scalars: usize,
    /// Points in each verifier-circuit round commitment.
    pub verifier_round_commitment_points: Vec<usize>,
    /// Public values in the verifier-circuit instance.
    pub verifier_public_values: usize,
    /// Challenges in each verifier-circuit round.
    pub verifier_challenges_per_round: Vec<usize>,
    /// Points in the Nova cross-term commitment.
    pub nova_cross_term_points: usize,
    /// Points in the random relaxed witness commitment.
    pub random_witness_commitment_points: usize,
    /// Points in the random relaxed error commitment.
    pub random_error_commitment_points: usize,
    /// Public scalars in the random relaxed instance.
    pub random_public_values: usize,
    /// Padded constraints in the verifier circuit's regular shape.
    pub verifier_constraints: usize,
    /// Padded variables in the verifier circuit's regular shape.
    pub verifier_variables: usize,
    /// Rounds in the relaxed-Spartan outer sum-check.
    pub relaxed_outer_rounds: usize,
    /// Stored scalars per relaxed-Spartan outer-round polynomial.
    pub relaxed_outer_coefficients: usize,
    /// Rounds in the relaxed-Spartan inner sum-check.
    pub relaxed_inner_rounds: usize,
    /// Stored scalars per relaxed-Spartan inner-round polynomial.
    pub relaxed_inner_coefficients: usize,
    /// Scalars in each relaxed-Spartan direct opening.
    pub relaxed_opening_scalars: usize,
}
/// Return the exact canonical MC dimensions.
///
/// # Errors
///
/// Reserved for a future governed-profile derivation failure; the released V1
/// dimensions are compiled into the profile and checked again at key install.
pub fn vega_mdl_proof_dimensions_v1() -> Result<VegaMdlProofDimensionsV1, VegaMdlProofErrorV1> {
    super::canonical_mc::proof_dimensions()
}
/// Return the pinned digest of the complete Figure 9 relation.
#[must_use]
pub const fn vega_mdl_canonical_relation_digest_v1() -> [u8; 32] {
    VEGA_MDL_CANONICAL_RELATION_DIGEST_V1
}
/// Return the digest of the released MC adapter/profile manifest.
#[must_use]
pub const fn vega_mdl_compiled_profile_digest_v1() -> [u8; 32] {
    VEGA_MDL_COMPILED_PROFILE_DIGEST_V1
}
/// Return the SHA-256 digest of the canonical Microsoft Vega-MC verifier key.
///
/// This returns the governed identity independently of process-local artifact
/// installation. [`install_vega_mdl_figure9_verifier_key_v1`] checks supplied
/// artifact bytes against this identity before verification can use them.
///
/// # Errors
///
/// Reserved for a future governed-profile derivation failure; the released V1
/// identity is compiled into the profile.
pub fn vega_mdl_verifier_digest_v1() -> Result<[u8; 32], VegaMdlProofErrorV1> {
    super::canonical_mc::verifier_digest()
}
/// Install the canonical Microsoft Figure 9 verifier-key artifact once.
///
/// Installation performs bounded strict decoding, byte-for-byte canonical
/// re-encoding, and exact released digest and dimension checks. A successful
/// installation is process-global, idempotent for the same artifact, and
/// cannot be cleared or replaced. This function performs no filesystem or
/// network lookup; governance must supply the artifact bytes explicitly.
///
/// # Errors
///
/// Returns [`VegaMdlProofErrorV1::InvalidCompiledProfile`] for a malformed,
/// noncanonical, wrong-profile, or conflicting artifact.
pub fn install_vega_mdl_figure9_verifier_key_v1(
    verifier_key: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    super::canonical_mc::install_figure9_verifier_key(verifier_key)
}
/// Install the canonical Microsoft Figure 9 proving-key/verifier-key pair once.
///
/// Both artifacts are bounded-decoded and canonically re-encoded, then every
/// proving-key component and its embedded verifier digest are matched against
/// the supplied VK and the released Figure 9 profile. Validation completes
/// before either artifact becomes visible. Installation is process-global,
/// idempotent only for the same pair, and neither artifact can be replaced.
/// This API performs no filesystem lookup, setup, or key generation.
///
/// Installing the pair authorizes only the exact first-party Figure 9 prover
/// path. Proof production still fails closed unless the mutually bound
/// artifacts pass their complete profile preflight before caller randomness.
///
/// # Errors
///
/// Returns [`VegaMdlProofErrorV1::InvalidCompiledProfile`] for malformed,
/// noncanonical, mismatched, wrong-profile, or conflicting artifacts.
pub fn install_vega_mdl_figure9_prover_artifacts_v1(
    proving_key: &[u8],
    verifier_key: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    super::canonical_mc::install_figure9_prover_artifacts(proving_key, verifier_key)
}
/// Validate one independent canonical Microsoft verifier-key/proof fixture.
///
/// This low-level conformance hook exists for release-vector tests. Production callers should use
/// [`verify_vega_mdl_figure9_v1`] with the governed Figure 9 profile instead.
///
/// # Errors
///
/// Rejects noncanonical key or proof bytes and any failed Microsoft proof equation.
#[doc(hidden)]
pub fn vega_microsoft_fixture_conformance_v1(
    verifier_key: &[u8],
    proof: &[u8],
) -> Result<([u8; 32], VegaMdlProofDimensionsV1, usize, usize), VegaMdlProofErrorV1> {
    super::canonical_mc::validate_microsoft_fixture(verifier_key, proof)
}
/// Attempt the pinned Figure 9 Vega-MC prover path.
///
/// The dependency-locked implementation validates context and the installed
/// PK/VK pair before caller randomness, synthesizes the exact Microsoft split
/// step/core witness, and returns a proof only after canonical re-encoding and
/// a complete first-party verifier replay.
///
/// # Errors
///
/// Fails closed on invalid context, witness, randomness, setup, or size.
pub fn prove_vega_mdl_figure9_v1<R: VegaRandomSourceV1>(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
    config: VegaMdlProverConfigV1,
    random: &mut R,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    super::canonical_mc::prove_figure9_mc(context, public_inputs, witness, config, random)
}
/// Verify one bounded, canonical, context-bound Vega-MC Figure 9 proof.
///
/// # Errors
///
/// Rejects malformed proof bytes, context/VK mismatches, wrong step order,
/// wrong public values, and any failed proof equation.
pub fn verify_vega_mdl_figure9_v1(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    proof_bytes: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    super::canonical_mc::verify_figure9_mc(context, public_inputs, proof_bytes)
}
/// Number of uniform SHA-256 compression instances in the released split.
pub const VEGA_MDL_MC_STEP_COUNT_V1: usize = VEGA_MDL_FIGURE9_SHA256_STEPS_V1;
const _: () = assert!(MAX_VEGA_PROOF_BYTES_V1 == 524_288);
