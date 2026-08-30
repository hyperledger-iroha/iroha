//! First-party Microsoft Vega-MC compatibility implementation.
use once_cell::sync::OnceCell;
use parking_lot::Mutex;

use super::{
    VegaMdlProofDimensionsV1, VegaT256ScalarV1 as Scalar,
    engine::{VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VegaRandomSourceErrorV1, VegaRandomSourceV1},
    figure9::Figure9McMaterial,
};
type ValidatedFixture = (
    [u8; 32],
    VegaMdlProofDimensionsV1,
    Vec<Vec<Scalar>>,
    Vec<Scalar>,
);
#[path = "microsoft_mc/application_prep.rs"]
mod application_prep;
#[path = "microsoft_mc/final_opening.rs"]
mod final_opening;
#[path = "microsoft_mc/prover_key.rs"]
mod prover_key;
#[path = "microsoft_mc/random_nova.rs"]
mod random_nova;
#[path = "microsoft_mc/relaxed_spartan.rs"]
mod relaxed_spartan;
#[path = "microsoft_mc/rng.rs"]
mod rng;
#[path = "microsoft_mc/semantic_engine.rs"]
mod semantic_engine;
#[path = "microsoft_mc/sha256.rs"]
mod sha256;
#[path = "microsoft_mc/split_adapter.rs"]
mod split_adapter;
#[path = "microsoft_mc/verifier_key.rs"]
mod verifier_key;
#[path = "microsoft_mc/verify.rs"]
mod verify;
#[path = "microsoft_mc/wire.rs"]
mod wire;

/// Test-only access to the compatibility boundary's dependency-free SHA-256.
///
/// Keeping this adapter here prevents sibling protocol tests from depending on
/// the private Microsoft wire implementation or introducing a second hashing
/// dependency merely to freeze deterministic fixtures.
#[cfg(test)]
pub(in crate::vega) fn dependency_free_sha256_for_tests(input: &[u8]) -> [u8; 32] {
    sha256::sha256(input).expect("in-memory test fixture fits SHA-256 length bounds")
}

static GOVERNED_FIGURE9_ARTIFACTS: GovernedFigure9ArtifactSlots =
    GovernedFigure9ArtifactSlots::new();

/// Failure while installing governed Figure 9 key artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9ArtifactInstallError {
    /// An artifact was not one exact canonical Microsoft key value.
    InvalidArtifact,
    /// The canonical pair did not match each other or the released profile.
    ProfileMismatch,
    /// A different artifact was already installed and cannot be replaced.
    AlreadyInstalled,
}

/// Failure while revalidating the governed artifacts before proving starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9ProverPreflightError {
    /// A governed proving-key/verifier-key pair has not been installed.
    MissingGovernedArtifacts,
    /// Installed artifacts no longer satisfy the complete governed profile.
    InvalidGovernedArtifacts,
}

/// Failure while validating the exact native-to-Microsoft Figure 9 split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9SplitAdapterError {
    /// Byte, state, row, or public-value metadata drifted from the pinned split.
    InvalidMetadata,
    /// Native and Microsoft padded dimensions cannot describe the pinned split.
    InvalidShape,
    /// A selected compression transition does not match the shared assignment.
    UnsatisfiedStep,
    /// A replayed non-SHA core row does not match the shared assignment.
    UnsatisfiedCore,
}

/// Failure while preparing the nine governed application commitments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9ApplicationPrepError {
    /// Native-to-Microsoft projection or a governed equation failed.
    Split(Figure9SplitAdapterError),
    /// PK/VK pairing, generator derivation, or an exact dimension failed.
    InvalidGovernedKey,
    /// The injected external seed source failed.
    RandomSource(VegaRandomSourceErrorV1),
    /// The external seed was constant or immediately reused.
    DegenerateRandomness,
    /// A bounded row commitment could not be constructed canonically.
    Commitment,
    /// A constructed split instance failed the verifier's transcript replay.
    Transcript,
    /// The exact semantic NIFS/sum-check/verifier-circuit stage failed closed.
    Semantic(Figure9SemanticEngineError),
    /// The fresh random verifier-instance mask or Nova fold failed closed.
    RandomNova(Figure9RandomNovaError),
    /// The folded verifier instance failed its relaxed-Spartan proof or replay.
    RelaxedSpartan(Figure9RelaxedSpartanError),
    /// The final Hyrax opening, linear IPA, encoding, or self-verification failed.
    FinalOpening(Figure9FinalOpeningError),
}

/// Failure in the exact 47-round Figure 9 semantic prover core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9SemanticEngineError {
    /// The governed verifier/application key pair did not match exactly.
    InvalidKey,
    /// A fixed Figure 9 dimension or round shape drifted.
    InvalidShape,
    /// One split application instance could not become its regular form.
    InvalidApplicationInstance,
    /// A bounded secret table could not be reserved.
    ResourceExhausted,
    /// One application or verifier witness commitment failed.
    Commitment,
    /// The exact Fiat--Shamir schedule could not be replayed.
    Transcript,
    /// A required nonzero field denominator was zero.
    DivisionByZero,
    /// A verifier round emitted the wrong exact auxiliary assignment length.
    InvalidRoundWitness,
    /// The eight-way NeutronNova folding identity failed.
    InvalidNifs,
    /// The batched cubic outer sum-check identity failed.
    InvalidOuterSumcheck,
    /// The batched quadratic inner sum-check identity failed.
    InvalidInnerSumcheck,
    /// The completed 47-round witness did not satisfy its governed matrices.
    UnsatisfiedVerifierCircuit,
}

/// Failure in the exact random-mask and verifier-instance Nova stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9RandomNovaError {
    /// The governed PK/VK or commitment generator set no longer matches.
    InvalidKey,
    /// The fixed 1,504-variable/512-constraint verifier geometry drifted.
    InvalidShape,
    /// A bounded secret owner or sparse-shape adapter could not reserve storage.
    ResourceExhausted,
    /// The retained semantic witness or a freshly sampled mask was unsatisfied.
    UnsatisfiedWitness,
    /// A witness, error, or cross-term commitment could not be constructed.
    Commitment,
    /// The exact post-semantic transcript state was absent or inconsistent.
    Transcript,
    /// The random mask or Nova fold failed its complete replay checks.
    InvalidNova,
}

/// Failure in the exact relaxed-Spartan tail for the folded verifier circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9RelaxedSpartanError {
    /// The governed proving/verifying keys or generator set no longer match.
    InvalidKey,
    /// The fixed 1,504-variable Spartan geometry drifted.
    InvalidShape,
    /// The continued transcript or proof-scoped RNG owner was absent.
    MissingState,
    /// The exact relaxed-Spartan prover rejected its folded witness.
    Proof,
    /// The canonical wire failed the compatibility verifier replay.
    SelfVerification,
}

/// Failure in the complete final Hyrax opening and proof-emission stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9FinalOpeningError {
    /// The governed proving/verifying keys or generator sets no longer match.
    InvalidKey,
    /// A fixed Figure 9 row, point, commitment, or proof dimension drifted.
    InvalidShape,
    /// The continued transcript or proof-scoped RNG owner was absent.
    MissingState,
    /// A bounded secret table could not reserve its exact storage.
    ResourceExhausted,
    /// A folded, bound-row, evaluation, or mask commitment was invalid.
    Commitment,
    /// The exact final Fiat--Shamir schedule could not be continued.
    Transcript,
    /// The streamed witness/evaluation did not open the retained commitments.
    EvaluationMismatch,
    /// The emitted wire failed a first-party verifier equation or public output check.
    SelfVerification,
    /// Canonical proof encoding or exact decoding failed.
    Encoding,
}

/// Failure while verifying under the governed Figure 9 verifier key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9VerificationError {
    /// No governed key has been installed in this process.
    MissingGovernedVerifierKey,
    /// The proof was not the one exact canonical Microsoft wire value.
    InvalidProofEncoding,
    /// At least one Microsoft proof equation failed.
    VerificationFailed,
}

struct GovernedFigure9ArtifactSlots {
    verifier_key: OnceCell<verifier_key::McVerifierKeyWire>,
    prover_key: OnceCell<prover_key::McProverKeyWire>,
    install_lock: Mutex<()>,
}

impl GovernedFigure9ArtifactSlots {
    const fn new() -> Self {
        Self {
            verifier_key: OnceCell::new(),
            prover_key: OnceCell::new(),
            install_lock: Mutex::new(()),
        }
    }

    fn decode_verifier(
        artifact: &[u8],
        expected_digest: [u8; 32],
        expected_dimensions: &VegaMdlProofDimensionsV1,
    ) -> Result<verifier_key::McVerifierKeyWire, Figure9ArtifactInstallError> {
        let candidate = verifier_key::McVerifierKeyWire::decode(artifact)
            .map_err(|_| Figure9ArtifactInstallError::InvalidArtifact)?;
        let canonical = candidate
            .encode()
            .map_err(|_| Figure9ArtifactInstallError::InvalidArtifact)?;
        if canonical != artifact {
            return Err(Figure9ArtifactInstallError::InvalidArtifact);
        }
        let digest = candidate
            .digest()
            .map_err(|_| Figure9ArtifactInstallError::InvalidArtifact)?;
        let dimensions = candidate
            .proof_dimensions()
            .map_err(|_| Figure9ArtifactInstallError::InvalidArtifact)?;
        if digest != expected_digest || &dimensions != expected_dimensions {
            return Err(Figure9ArtifactInstallError::ProfileMismatch);
        }
        Ok(candidate)
    }

    fn install_verifier(
        &self,
        artifact: &[u8],
        expected_digest: [u8; 32],
        expected_dimensions: &VegaMdlProofDimensionsV1,
    ) -> Result<(), Figure9ArtifactInstallError> {
        let candidate = Self::decode_verifier(artifact, expected_digest, expected_dimensions)?;
        let _guard = self.install_lock.lock();
        if let Some(installed) = self.verifier_key.get() {
            return if installed == &candidate {
                Ok(())
            } else {
                Err(Figure9ArtifactInstallError::AlreadyInstalled)
            };
        }
        self.verifier_key
            .set(candidate)
            .map_err(|_| Figure9ArtifactInstallError::AlreadyInstalled)
    }

    fn install_pair(
        &self,
        proving_key_artifact: &[u8],
        verifier_key_artifact: &[u8],
        expected_digest: [u8; 32],
        expected_dimensions: &VegaMdlProofDimensionsV1,
    ) -> Result<(), Figure9ArtifactInstallError> {
        // Decode, canonicalize, and cross-bind both candidates before taking
        // the mutation lock. Malformed input can never partially install.
        let verifier_key =
            Self::decode_verifier(verifier_key_artifact, expected_digest, expected_dimensions)?;
        let proving_key = prover_key::McProverKeyWire::decode(proving_key_artifact)
            .map_err(|_| Figure9ArtifactInstallError::InvalidArtifact)?;
        if proving_key
            .encode()
            .map_err(|_| Figure9ArtifactInstallError::InvalidArtifact)?
            != proving_key_artifact
        {
            return Err(Figure9ArtifactInstallError::InvalidArtifact);
        }
        proving_key
            .validate_against(&verifier_key)
            .map_err(|_| Figure9ArtifactInstallError::ProfileMismatch)?;

        let _guard = self.install_lock.lock();
        if self
            .verifier_key
            .get()
            .is_some_and(|installed| installed != &verifier_key)
            || self
                .prover_key
                .get()
                .is_some_and(|installed| installed != &proving_key)
        {
            return Err(Figure9ArtifactInstallError::AlreadyInstalled);
        }
        if self.prover_key.get().is_none() {
            self.prover_key
                .set(proving_key)
                .map_err(|_| Figure9ArtifactInstallError::AlreadyInstalled)?;
        }
        if self.verifier_key.get().is_none() {
            self.verifier_key
                .set(verifier_key)
                .map_err(|_| Figure9ArtifactInstallError::AlreadyInstalled)?;
        }
        Ok(())
    }

    fn get_verifier(&self) -> Option<&verifier_key::McVerifierKeyWire> {
        self.verifier_key.get()
    }

    fn get_prover(&self) -> Option<&prover_key::McProverKeyWire> {
        self.prover_key.get()
    }

    fn validate_pair(
        &self,
        expected_digest: [u8; 32],
        expected_dimensions: &VegaMdlProofDimensionsV1,
    ) -> Result<(), Figure9ProverPreflightError> {
        let verifier_key = self
            .verifier_key
            .get()
            .ok_or(Figure9ProverPreflightError::MissingGovernedArtifacts)?;
        let prover_key = self
            .prover_key
            .get()
            .ok_or(Figure9ProverPreflightError::MissingGovernedArtifacts)?;
        if verifier_key.digest().ok() != Some(expected_digest)
            || verifier_key.proof_dimensions().ok().as_ref() != Some(expected_dimensions)
            || prover_key.validate_against(verifier_key).is_err()
        {
            return Err(Figure9ProverPreflightError::InvalidGovernedArtifacts);
        }
        Ok(())
    }
}
/// Return the released Figure 9 dimensions independently of runtime setup.
///
/// These values were derived from the canonical Microsoft verifier key and
/// are governed by the compiled-profile manifest. Keeping the owned values
/// here makes proof admission deterministic and lockfile-independent.
pub(super) fn canonical_figure9_dimensions() -> VegaMdlProofDimensionsV1 {
    let mut verifier_challenges_per_round = vec![1; 47];
    for index in [3, 44, 45, 46] {
        verifier_challenges_per_round[index] = 0;
    }
    VegaMdlProofDimensionsV1 {
        num_steps: 8,
        shared_variables: 524_288,
        step_precommitted_variables: 2_048,
        step_rest_variables: 522_240,
        core_precommitted_variables: 2_048,
        core_rest_variables: 522_240,
        step_constraints: 262_144,
        step_variables: 1_048_576,
        core_constraints: 262_144,
        core_variables: 1_048_576,
        shared_commitment_points: 256,
        step_precommitted_points: 1,
        step_rest_points: 255,
        step_public_values: 1,
        step_challenges: 0,
        core_precommitted_points: 1,
        core_rest_points: 255,
        core_public_values: 18,
        core_challenges: 0,
        evaluation_response_scalars: 2_048,
        verifier_round_commitment_points: vec![1; 47],
        verifier_public_values: 6,
        verifier_challenges_per_round,
        nova_cross_term_points: 16,
        random_witness_commitment_points: 47,
        random_error_commitment_points: 16,
        random_public_values: 49,
        verifier_constraints: 512,
        verifier_variables: 1_504,
        relaxed_outer_rounds: 9,
        relaxed_outer_coefficients: 3,
        relaxed_inner_rounds: 12,
        relaxed_inner_coefficients: 2,
        relaxed_opening_scalars: 32,
    }
}
/// Return the governed digest of the canonical Figure 9 Microsoft key.
pub(super) const fn canonical_figure9_verifier_digest() -> [u8; 32] {
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
}
/// Install the exact governed Figure 9 verifier-key artifact once.
///
/// The candidate is decoded under absolute bounds, re-encoded byte for byte,
/// and matched against both the released Microsoft key digest and every
/// verifier-key-derived proof dimension before it becomes visible. The slot
/// cannot be cleared or replaced.
pub(super) fn install_governed_figure9_verifier_key(
    artifact: &[u8],
) -> Result<(), Figure9ArtifactInstallError> {
    GOVERNED_FIGURE9_ARTIFACTS.install_verifier(
        artifact,
        canonical_figure9_verifier_digest(),
        &canonical_figure9_dimensions(),
    )
}

/// Install an exact, mutually bound Microsoft Figure 9 PK/VK pair once.
///
/// Both artifacts are fully decoded, canonically re-encoded, matched to each
/// other, and matched to the released digest and dimensions before either can
/// become visible. The pair is supplied explicitly; no ambient lookup or key
/// generation exists at this boundary.
pub(super) fn install_governed_figure9_prover_artifacts(
    proving_key: &[u8],
    verifier_key: &[u8],
) -> Result<(), Figure9ArtifactInstallError> {
    GOVERNED_FIGURE9_ARTIFACTS.install_pair(
        proving_key,
        verifier_key,
        canonical_figure9_verifier_digest(),
        &canonical_figure9_dimensions(),
    )
}

/// Revalidate the complete governed PK/VK pair before any prover randomness.
pub(super) fn preflight_governed_figure9_prover_artifacts()
-> Result<(), Figure9ProverPreflightError> {
    GOVERNED_FIGURE9_ARTIFACTS.validate_pair(
        canonical_figure9_verifier_digest(),
        &canonical_figure9_dimensions(),
    )
}

/// Produce one complete, canonically encoded, self-verified Figure 9 proof.
///
/// This validates all nine exact W assignments, exact-matches the installed
/// application key, consumes one health-checked external seed, commits the
/// split application and verifier-circuit sections, executes the exact
/// eight-way NIFS plus batched Spartan sum-checks, and checks all verifier R1CS
/// equations. It then continues the same proof-scoped RNG and transcript
/// through the satisfying random verifier instance, one exact Nova fold, the
/// complete relaxed-Spartan proof, and the streamed final Hyrax/linear-IPA
/// opening. The exact Microsoft wire is returned only after canonical
/// decode/re-encode and complete first-party verifier replay both succeed.
pub(super) fn prepare_governed_figure9_application<R: VegaRandomSourceV1>(
    material: &Figure9McMaterial,
    step_public_values: &[Vec<Scalar>],
    core_public_values: &[Scalar],
    worker_count: usize,
    random: &mut R,
) -> Result<Vec<u8>, Figure9ApplicationPrepError> {
    let prover_key = GOVERNED_FIGURE9_ARTIFACTS
        .get_prover()
        .ok_or(Figure9ApplicationPrepError::InvalidGovernedKey)?;
    let verifier_key = GOVERNED_FIGURE9_ARTIFACTS
        .get_verifier()
        .ok_or(Figure9ApplicationPrepError::InvalidGovernedKey)?;
    let prepared = application_prep::prepare(
        material,
        step_public_values,
        core_public_values,
        prover_key,
        verifier_key,
        worker_count,
        random,
    )?;
    let semantic = semantic_engine::build(prepared, prover_key, verifier_key)
        .map_err(Figure9ApplicationPrepError::Semantic)?;
    let random_nova = random_nova::build(semantic, prover_key, verifier_key)
        .map_err(Figure9ApplicationPrepError::RandomNova)?;
    let relaxed_spartan = relaxed_spartan::build(random_nova, prover_key, verifier_key)
        .map_err(Figure9ApplicationPrepError::RelaxedSpartan)?;
    final_opening::build(relaxed_spartan, prover_key, verifier_key)
        .map_err(Figure9ApplicationPrepError::FinalOpening)
}

/// Decode and verify one canonical Figure 9 proof under the installed key.
pub(super) fn verify_governed_figure9_proof(
    proof: &[u8],
) -> Result<(Vec<Vec<Scalar>>, Vec<Scalar>), Figure9VerificationError> {
    let key = GOVERNED_FIGURE9_ARTIFACTS
        .get_verifier()
        .ok_or(Figure9VerificationError::MissingGovernedVerifierKey)?;
    let decoded = wire::McProofWire::decode(proof, &canonical_figure9_dimensions())
        .map_err(|_| Figure9VerificationError::InvalidProofEncoding)?;
    let canonical = decoded
        .encode()
        .map_err(|_| Figure9VerificationError::InvalidProofEncoding)?;
    if canonical != proof {
        return Err(Figure9VerificationError::InvalidProofEncoding);
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        verify::verify(&decoded, key, canonical_figure9_dimensions().num_steps)
    }))
    .map_err(|_| Figure9VerificationError::VerificationFailed)?
    .map_err(|_| Figure9VerificationError::VerificationFailed)
}

/// Validate only the canonical structure of one released Figure 9 proof.
///
/// This does not verify equations without the governed Figure 9 verifier key;
/// callers must never treat success here as proof acceptance.
pub(super) fn scan_canonical_figure9_proof(proof: &[u8]) -> Result<(), wire::McCodecError> {
    let decoded = wire::McProofWire::decode(proof, &canonical_figure9_dimensions())?;
    if decoded.encode()? != proof {
        return Err(wire::McCodecError::InvalidEncoding);
    }
    Ok(())
}
/// Decode, re-encode, and verify an independent Microsoft fixture pair.
pub(super) fn validate_fixture(
    verifier_key: &[u8],
    proof: &[u8],
) -> Result<ValidatedFixture, wire::McCodecError> {
    let key = verifier_key::McVerifierKeyWire::decode(verifier_key)?;
    if key.encode()? != verifier_key {
        return Err(wire::McCodecError::InvalidEncoding);
    }
    let dimensions = key.proof_dimensions()?;
    let decoded = wire::McProofWire::decode(proof, &dimensions)?;
    if decoded.encode()? != proof {
        return Err(wire::McCodecError::InvalidEncoding);
    }
    let (steps, core) = verify::verify(&decoded, &key, dimensions.num_steps)?;
    Ok((key.digest()?, dimensions, steps, core))
}
#[cfg(test)]
mod tests {
    use super::*;
    const PYTHON_VK: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
    ));

    fn proving_key_artifact(verifier_key: &verifier_key::McVerifierKeyWire) -> Vec<u8> {
        prover_key::McProverKeyWire {
            application_key: verifier_key.application_key.clone(),
            step_shape: verifier_key.step_shape.clone(),
            core_shape: verifier_key.core_shape.clone(),
            verifier_digest: verifier_key.digest().expect("verifier digest"),
            verifier_shape: verifier_key.verifier_shape.clone(),
            verifier_regular_shape: verifier_key.verifier_regular_shape.clone(),
            verifier_commitment_key: verifier_key.verifier_commitment_key.clone(),
        }
        .encode()
        .expect("canonical proving-key fixture")
    }

    #[test]
    fn governed_figure9_dimensions_match_the_compiled_profile() {
        let dimensions = canonical_figure9_dimensions();
        assert_eq!(dimensions.num_steps, 8);
        assert_eq!(dimensions.shared_variables, 524_288);
        assert_eq!(dimensions.verifier_round_commitment_points, [1; 47]);
        assert_eq!(dimensions.verifier_challenges_per_round[3], 0);
        assert_eq!(dimensions.verifier_challenges_per_round[44..], [0; 3]);
        assert_eq!(dimensions.relaxed_outer_rounds, 9);
        assert_eq!(dimensions.relaxed_inner_rounds, 12);
        assert_eq!(
            canonical_figure9_verifier_digest(),
            VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
        );
    }

    #[test]
    fn governed_key_slot_is_strict_idempotent_and_not_replaceable() {
        let key = verifier_key::McVerifierKeyWire::decode(PYTHON_VK)
            .expect("independent canonical verifier key");
        let digest = key.digest().expect("Microsoft key digest");
        let dimensions = key.proof_dimensions().expect("key dimensions");
        let slot = GovernedFigure9ArtifactSlots::new();
        slot.install_verifier(PYTHON_VK, digest, &dimensions)
            .expect("first exact install");
        slot.install_verifier(PYTHON_VK, digest, &dimensions)
            .expect("same artifact is idempotent");

        let mut replacement = key;
        replacement.num_steps += 1;
        let replacement_artifact = replacement.encode().expect("alternate canonical key");
        let replacement_digest = replacement.digest().expect("alternate key digest");
        let replacement_dimensions = replacement
            .proof_dimensions()
            .expect("alternate key dimensions");
        assert_eq!(
            slot.install_verifier(
                &replacement_artifact,
                replacement_digest,
                &replacement_dimensions,
            ),
            Err(Figure9ArtifactInstallError::AlreadyInstalled)
        );
    }

    #[test]
    fn governed_key_slot_rejects_parse_and_profile_corruption_before_install() {
        let key = verifier_key::McVerifierKeyWire::decode(PYTHON_VK)
            .expect("independent canonical verifier key");
        let digest = key.digest().expect("Microsoft key digest");
        let dimensions = key.proof_dimensions().expect("key dimensions");
        let slot = GovernedFigure9ArtifactSlots::new();

        let mut trailing = PYTHON_VK.to_vec();
        trailing.push(0);
        assert_eq!(
            slot.install_verifier(&trailing, digest, &dimensions),
            Err(Figure9ArtifactInstallError::InvalidArtifact)
        );
        let mut wrong_digest = digest;
        wrong_digest[0] ^= 1;
        assert_eq!(
            slot.install_verifier(PYTHON_VK, wrong_digest, &dimensions),
            Err(Figure9ArtifactInstallError::ProfileMismatch)
        );
        let mut wrong_dimensions = dimensions.clone();
        wrong_dimensions.num_steps += 1;
        assert_eq!(
            slot.install_verifier(PYTHON_VK, digest, &wrong_dimensions),
            Err(Figure9ArtifactInstallError::ProfileMismatch)
        );
        assert!(slot.get_verifier().is_none());
    }

    #[test]
    fn governed_pair_slot_is_exact_idempotent_and_preflight_validated() {
        let mut key = verifier_key::McVerifierKeyWire::decode(PYTHON_VK)
            .expect("independent canonical verifier key");
        let digest = key.digest().expect("Microsoft key digest");
        let dimensions = key.proof_dimensions().expect("key dimensions");
        let proving_key = proving_key_artifact(&key);
        let slot = GovernedFigure9ArtifactSlots::new();
        slot.install_pair(&proving_key, PYTHON_VK, digest, &dimensions)
            .expect("first exact pair install");
        slot.install_pair(&proving_key, PYTHON_VK, digest, &dimensions)
            .expect("same pair is idempotent");
        slot.validate_pair(digest, &dimensions)
            .expect("installed pair remains governed");

        key.num_steps += 1;
        let replacement_vk = key.encode().expect("alternate canonical verifier key");
        let replacement_pk = proving_key_artifact(&key);
        let replacement_digest = key.digest().expect("alternate verifier digest");
        let replacement_dimensions = key.proof_dimensions().expect("alternate dimensions");
        assert_eq!(
            slot.install_pair(
                &replacement_pk,
                &replacement_vk,
                replacement_digest,
                &replacement_dimensions,
            ),
            Err(Figure9ArtifactInstallError::AlreadyInstalled)
        );
        slot.validate_pair(digest, &dimensions)
            .expect("rejected replacement cannot mutate installed pair");
    }

    #[test]
    fn governed_pair_slot_rejects_mismatch_without_partial_install() {
        let key = verifier_key::McVerifierKeyWire::decode(PYTHON_VK)
            .expect("independent canonical verifier key");
        let digest = key.digest().expect("Microsoft key digest");
        let dimensions = key.proof_dimensions().expect("key dimensions");
        let proving_key = proving_key_artifact(&key);
        let mut mismatched =
            prover_key::McProverKeyWire::decode(&proving_key).expect("proving key");
        mismatched.verifier_digest[0] ^= 1;
        let mismatched = mismatched.encode().expect("canonical mismatched key");
        let slot = GovernedFigure9ArtifactSlots::new();
        assert_eq!(
            slot.install_pair(&mismatched, PYTHON_VK, digest, &dimensions),
            Err(Figure9ArtifactInstallError::ProfileMismatch)
        );
        assert!(slot.verifier_key.get().is_none());
        assert!(slot.prover_key.get().is_none());
        assert_eq!(
            slot.validate_pair(digest, &dimensions),
            Err(Figure9ProverPreflightError::MissingGovernedArtifacts)
        );
    }

    #[test]
    fn production_verification_fails_closed_without_the_governed_key() {
        assert_eq!(
            verify_governed_figure9_proof(&[]),
            Err(Figure9VerificationError::MissingGovernedVerifierKey)
        );
    }
}
