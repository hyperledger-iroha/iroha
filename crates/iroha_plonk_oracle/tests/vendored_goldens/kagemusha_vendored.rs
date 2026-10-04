//! The vendored KAGEMUSHA proving path, as `iroha_core_zk` runs it
//! (`create_*_proof_with_key_v1` and `verify_augmented` in
//! `crates/iroha_core_zk/src/prover_golden_tests.rs`), over the golden
//! circuits (every build):
//!
//! - the halo2-axiom prover with the snark-verifier `PoseidonTranscript<C,
//!   NativeLoader, _, 3, 2, 8, 57>` (secure MDS 0) and Committed instances;
//! - the folded-generator augmentation: the vendored IPA verifier's `G'_0`
//!   (`GuardIPA::compute_g`) appended to the raw transcript, unabsorbed;
//! - the augmented verdict: the raw transcript verifies, is consumed
//!   exactly, and the appended point equals `G'_0`.
//!
//! The KAGEMUSHA production randomness is a recovery-seed `ChaCha20` stream;
//! here the vendored prover takes a `ChaCha20` stream from a fixed seed, the
//! same stream `ProverRandomness::from_rng_for_tests` gives the native
//! prover, which is what proof-byte parity needs. `kagemusha_parity` (oracle
//! builds) compares the native proofs with these.

use std::{
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
};

use halo2_axiom::{
    halo2curves::{CurveAffine, group::GroupEncoding},
    plonk::{Circuit, Error as VError, ProvingKey, VerifyingKey, create_proof, verify_proof},
    poly::{
        VerificationStrategy,
        commitment::MSM as _,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            msm::MSMIPA,
            multiopen::{ProverIPA, VerifierIPA},
            strategy::GuardIPA,
        },
    },
};
use iroha_plonk_oracle::convert::CurveBridge;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use snark_verifier::{
    loader::native::NativeLoader,
    system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    util::arithmetic::FieldExt,
};

/// Width of the KAGEMUSHA Poseidon permutation.
const POSEIDON_T: usize = 3;
/// Rate of the KAGEMUSHA Poseidon sponge.
const POSEIDON_RATE: usize = 2;
/// Full rounds.
const POSEIDON_FULL_ROUNDS: usize = 8;
/// Partial rounds.
const POSEIDON_PARTIAL_ROUNDS: usize = 57;
/// The secure-MDS selector of `iroha_core_zk`.
const POSEIDON_SECURE_MDS: usize = 0;
/// The compressed folded generator appended by the augmentation.
pub const FOLDED_GENERATOR_BYTES: usize = 32;

/// The KAGEMUSHA proof transcript (`iroha_core_zk` `KAGEMUSHA_IPA_POSEIDON_*_V1`).
pub type KagemushaTranscript<C, S> = PoseidonTranscript<
    C,
    NativeLoader,
    S,
    POSEIDON_T,
    POSEIDON_RATE,
    POSEIDON_FULL_ROUNDS,
    POSEIDON_PARTIAL_ROUNDS,
>;

/// Full IPA verification that returns the folded generator `G'_0`
/// (`iroha_core_zk` `GoldenFoldedGenerator`).
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
        verifier: impl FnOnce(MSMIPA<'params, C>) -> Result<GuardIPA<'params, C>, VError>,
    ) -> Result<Self::Output, VError> {
        let guard = verifier(MSMIPA::new(self.params))?;
        let folded = guard.compute_g();
        let (check, _) = guard.use_g(folded);
        if check.check() {
            Ok(folded)
        } else {
            Err(VError::ConstraintSystemFailure)
        }
    }

    fn finalize(self) -> bool {
        true
    }
}

/// The vendored `G'_0` of a raw KAGEMUSHA transcript, after full
/// verification; also whether the transcript was consumed exactly.
fn folded_generator<B: CurveBridge>(
    params: &ParamsIPA<B::Vendored>,
    vk: &VerifyingKey<B::Vendored>,
    raw: &[u8],
    instances: &[Vec<B::VScalar>],
) -> Result<(B::Vendored, bool), String>
where
    B::VScalar: FieldExt,
{
    let columns: Vec<&[B::VScalar]> = instances.iter().map(Vec::as_slice).collect();
    let per_proof: [&[&[B::VScalar]]; 1] = [&columns];
    let mut cursor = Cursor::new(raw);
    let folded = {
        let mut transcript =
            KagemushaTranscript::<B::Vendored, _>::new::<POSEIDON_SECURE_MDS>(&mut cursor);
        verify_proof::<
            IPACommitmentScheme<B::Vendored>,
            VerifierIPA<'_, B::Vendored>,
            ChallengeScalar<B::Vendored>,
            _,
            FoldedGenerator<'_, B::Vendored>,
        >(
            params,
            vk,
            FoldedGenerator { params },
            &per_proof,
            &mut transcript,
        )
        .map_err(|error| format!("{error:?}"))?
    };
    let consumed = usize::try_from(cursor.position()).ok() == Some(raw.len());
    Ok((folded, consumed))
}

/// One vendored KAGEMUSHA proof: the raw Poseidon transcript of the
/// halo2-axiom prover (Committed instances, `ChaCha20` stream from `seed`),
/// then the folded generator.
pub fn prove_augmented<B, Circ>(
    params: &ParamsIPA<B::Vendored>,
    pk: &ProvingKey<B::Vendored>,
    circuit: &Circ,
    instances: &[Vec<B::VScalar>],
    seed: [u8; 32],
) -> Vec<u8>
where
    B: CurveBridge,
    B::VScalar: FieldExt,
    Circ: Circuit<B::VScalar> + Clone,
{
    let columns: Vec<&[B::VScalar]> = instances.iter().map(Vec::as_slice).collect();
    let per_proof: [&[&[B::VScalar]]; 1] = [&columns];
    let mut transcript =
        KagemushaTranscript::<B::Vendored, Vec<u8>>::new::<POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<B::Vendored>,
        ProverIPA<'_, B::Vendored>,
        ChallengeScalar<B::Vendored>,
        _,
        _,
        _,
    >(
        params,
        pk,
        std::slice::from_ref(circuit),
        &per_proof,
        ChaCha20Rng::from_seed(seed),
        &mut transcript,
    )
    .expect("vendored KAGEMUSHA proof");
    let mut proof = transcript.finalize();
    let (folded, consumed) = folded_generator::<B>(params, pk.get_vk(), &proof, instances)
        .expect("the vendored proof verifies");
    assert!(
        consumed,
        "the vendored verifier consumes the raw transcript"
    );
    proof.extend_from_slice(folded.to_bytes().as_ref());
    proof
}

/// The vendored augmented verdict on `proof` (`iroha_core_zk`
/// `verify_augmented`): `Ok`, or the reason (a panic is reported, never
/// propagated).
pub fn verify_augmented<B>(
    params: &ParamsIPA<B::Vendored>,
    vk: &VerifyingKey<B::Vendored>,
    instances: &[Vec<B::VScalar>],
    proof: &[u8],
) -> Result<(), String>
where
    B: CurveBridge,
    B::VScalar: FieldExt,
{
    let Some(raw_len) = proof.len().checked_sub(FOLDED_GENERATOR_BYTES) else {
        return Err("shorter than the folded generator".to_owned());
    };
    let (raw, generator) = proof.split_at(raw_len);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        folded_generator::<B>(params, vk, raw, instances)
    }));
    match outcome {
        Err(_) => Err("vendored verifier panicked".to_owned()),
        Ok(Err(error)) => Err(error),
        Ok(Ok((_, false))) => Err("raw transcript not consumed exactly".to_owned()),
        Ok(Ok((folded, true))) if folded.to_bytes().as_ref() == generator => Ok(()),
        Ok(Ok(_)) => Err("folded generator mismatch".to_owned()),
    }
}
