//! Independent snark-verifier succinct challenge and BGH19 obligation capture.
//!
//! These historical oracle transcripts deliberately retain the old scalar-field
//! framing. They do not assert byte equality with the distinct PIPA-AS-v1 fold.
//! Every successful cheap verification is followed by both generator decisions.

use std::{
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
};

use halo2_axiom::{
    halo2curves::{
        CurveAffine,
        ff::{Field, PrimeField},
        group::GroupEncoding,
    },
    plonk::verify_proof,
    poly::{
        VerificationStrategy,
        commitment::{Params, ParamsProver},
        ipa::{commitment::IPACommitmentScheme, multiopen::VerifierIPA, strategy::SingleStrategy},
    },
};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::verifier::accumulate_succinct_oracle;
use iroha_plonk_oracle::{
    convert::{CurveBridge, Pallas, Vesta, native_scalar},
    vendored::Recording,
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::{
        AccumulationDecider,
        ipa::{Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaSuccinctVerifyingKey},
    },
    system::halo2::{Config, compile, transcript::halo2::ChallengeScalar},
    util::transcript::{Transcript, TranscriptRead},
    verifier::{SnarkVerifier, plonk::PlonkSuccinctVerifier},
};

use crate::{
    cases::{Family, Setup, setup},
    golden_proof_bytes::digest_hex,
    kagemusha_parity::kagemusha_keys,
    kagemusha_vendored::KagemushaTranscript,
};

/// Forward every snark-verifier transcript call and retain its challenges.
struct Trace<C: CurveAffine, T> {
    inner: T,
    challenges: Vec<C::Scalar>,
}

impl<C: CurveAffine, T: Transcript<C, NativeLoader>> Transcript<C, NativeLoader> for Trace<C, T> {
    fn loader(&self) -> &NativeLoader {
        self.inner.loader()
    }
    fn squeeze_challenge(&mut self) -> C::Scalar {
        let value = self.inner.squeeze_challenge();
        self.challenges.push(value);
        value
    }
    fn common_ec_point(&mut self, point: &C) -> Result<(), snark_verifier::Error> {
        self.inner.common_ec_point(point)
    }
    fn common_scalar(&mut self, scalar: &C::Scalar) -> Result<(), snark_verifier::Error> {
        self.inner.common_scalar(scalar)
    }
}

impl<C: CurveAffine, T: TranscriptRead<C, NativeLoader>> TranscriptRead<C, NativeLoader>
    for Trace<C, T>
{
    fn read_scalar(&mut self) -> Result<C::Scalar, snark_verifier::Error> {
        self.inner.read_scalar()
    }
    fn read_ec_point(&mut self) -> Result<C, snark_verifier::Error> {
        self.inner.read_ec_point()
    }
}

/// Canonical compressed point and round scalars, with every squeezed challenge.
struct Observation<B: CurveBridge> {
    accumulator: IpaAccumulator<B::Vendored, NativeLoader>,
    challenges: Vec<B::VScalar>,
}

/// Original snark-verifier protocol and BGH19 implementation, with exact input consumption.
fn succinct<B: CurveBridge>(
    source: &Setup<B>,
    instances: &[Vec<B::VScalar>],
    proof: &[u8],
) -> Result<Observation<B>, String> {
    let protocol = compile(
        &source.vendored_params,
        source.vendored_pk.get_vk(),
        Config::ipa().with_num_instance(instances.iter().map(Vec::len).collect()),
    );
    // Read H from the original ParamsIPA codec, not from a native derivation.
    let mut encoded = Vec::new();
    source
        .vendored_params
        .write(&mut encoded)
        .expect("original parameters");
    let encoded_h: [u8; 32] = encoded[encoded.len() - 32..].try_into().expect("H width");
    let h = Option::<B::Vendored>::from(B::Vendored::from_bytes(&encoded_h)).expect("original H");
    let svk = IpaSuccinctVerifyingKey::new(
        protocol.domain.clone(),
        source.vendored_params.get_g()[0],
        h,
        Some(source.vendored_params.get_blind_base()),
    );
    let mut cursor = Cursor::new(proof);
    let mut transcript = Trace {
        inner: KagemushaTranscript::<B::Vendored, _>::new::<0>(&mut cursor),
        challenges: Vec::new(),
    };
    type Verifier<C> = PlonkSuccinctVerifier<IpaAs<C, Bgh19>>;
    let parsed = Verifier::<B::Vendored>::read_proof(&svk, &protocol, instances, &mut transcript)
        .map_err(|e| format!("read: {e:?}"))?;
    let challenges = transcript.challenges;
    let mut accumulators = Verifier::<B::Vendored>::verify(&svk, &protocol, instances, &parsed)
        .map_err(|e| format!("succinct: {e:?}"))?;
    if cursor.position() != proof.len() as u64 || accumulators.len() != 1 {
        return Err("trailing bytes or unexpected obligations".into());
    }
    let accumulator = accumulators.remove(0);
    IpaAs::<B::Vendored, Bgh19>::decide(
        &IpaDecidingKey::new(svk, source.vendored_params.get_g().to_vec()),
        accumulator.clone(),
    )
    .map_err(|e| format!("decide: {e:?}"))?;
    Ok(Observation {
        accumulator,
        challenges,
    })
}

/// Challenges from the original Halo2 full verifier, independently of snark-verifier.
fn halo2_challenges<B: CurveBridge>(source: &Setup<B>, proof: &[u8]) -> Vec<B::VScalar> {
    let instances = source.vendored_instances();
    let columns: Vec<_> = instances.iter().map(Vec::as_slice).collect();
    let per_proof: [&[&[B::VScalar]]; 1] = [&columns];
    let raw = &proof[..proof.len() - 32];
    let mut cursor = Cursor::new(raw);
    let mut transcript =
        Recording::new(KagemushaTranscript::<B::Vendored, _>::new::<0>(&mut cursor));
    verify_proof::<
        IPACommitmentScheme<B::Vendored>,
        VerifierIPA<'_, B::Vendored>,
        ChallengeScalar<B::Vendored>,
        _,
        SingleStrategy<'_, B::Vendored>,
    >(
        &source.vendored_params,
        source.vendored_pk.get_vk(),
        SingleStrategy::new(&source.vendored_params),
        &per_proof,
        &mut transcript,
    )
    .expect("original full verifier");
    let challenges = transcript.challenges();
    drop(transcript);
    assert_eq!(cursor.position(), raw.len() as u64);
    challenges
}

/// Both curves, both source families, two fixed independent prover seeds and adversarial originals.
fn compare<B: CurveBridge>(family: Family, k: u32) {
    let source = setup::<B>(family, k);
    let keys = kagemusha_keys(&source);
    for seed in [42, 43] {
        let proof = source.prove_vendored_kagemusha([seed; 32]);
        let observed = succinct::<B>(&source, &source.vendored_instances(), &proof)
            .expect("snark-verifier full decision");
        assert_eq!(
            observed.challenges,
            halo2_challenges(&source, &proof),
            "every succinct challenge"
        );
        let native = accumulate_succinct_oracle(
            &source.params,
            keys.pk.binding(),
            keys.pk.vk(),
            &source.native_instances(),
            &proof,
            MemoryBudget::DEFAULT,
            source.transcript_repr,
        )
        .expect("native cheap verification");
        assert_eq!(
            native.g().to_bytes().as_ref(),
            observed.accumulator.u.to_bytes().as_ref()
        );
        assert_eq!(
            native.challenges(),
            observed
                .accumulator
                .xi
                .iter()
                .map(native_scalar::<B>)
                .collect::<Vec<_>>()
        );
        native
            .decide(&source.params, MemoryBudget::DEFAULT)
            .expect("native generator decision");
        let mut tape = Vec::new();
        for value in &observed.challenges {
            tape.extend_from_slice(value.to_repr().as_ref());
        }
        tape.extend_from_slice(observed.accumulator.u.to_bytes().as_ref());
        for value in &observed.accumulator.xi {
            tape.extend_from_slice(value.to_repr().as_ref());
        }
        println!(
            "SNARK_SUCCINCT curve={} family={family:?} k={k} seed={seed} challenges={} proof_sha256={} tape_sha256={}",
            B::NAME,
            observed.challenges.len(),
            digest_hex(&proof),
            digest_hex(&tape)
        );

        let mut wrong_instances = source.vendored_instances();
        wrong_instances[0][0] += B::VScalar::ONE;
        let mut altered = proof.clone();
        altered[32] ^= 1;
        let mut suffix = proof.clone();
        let last = suffix.len() - 32;
        suffix[last..].copy_from_slice(source.vendored_params.get_g()[0].to_bytes().as_ref());
        let mut trailing = proof.clone();
        trailing.push(0);
        for (label, instances, bytes) in [
            ("instance", wrong_instances, proof.clone()),
            ("proof", source.vendored_instances(), altered),
            ("suffix", source.vendored_instances(), suffix),
            ("trailing", source.vendored_instances(), trailing),
            (
                "truncated",
                source.vendored_instances(),
                proof[..proof.len() - 1].to_vec(),
            ),
        ] {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                succinct::<B>(&source, &instances, &bytes)
            }));
            assert!(outcome.is_ok(), "{label}: original verifier panicked");
            assert!(
                outcome.unwrap().is_err(),
                "{label}: original verifier accepted"
            );
            let native_instances = instances
                .iter()
                .map(|column| column.iter().map(native_scalar::<B>).collect())
                .collect::<Vec<_>>();
            let accepted = accumulate_succinct_oracle(
                &source.params,
                keys.pk.binding(),
                keys.pk.vk(),
                &native_instances,
                &bytes,
                MemoryBudget::DEFAULT,
                source.transcript_repr,
            )
            .ok()
            .is_some_and(|claim| claim.decide(&source.params, MemoryBudget::DEFAULT).is_ok());
            assert!(!accepted, "{label}: native verifier accepted");
        }
    }
}

#[test]
fn sigma_eq_succinct_matches_snark_verifier() {
    compare::<Vesta>(Family::Sigma, 6);
}
#[test]
fn sigma_ep_succinct_matches_snark_verifier() {
    compare::<Pallas>(Family::Sigma, 6);
}
#[test]
fn wide_eq_succinct_matches_snark_verifier() {
    compare::<Vesta>(Family::Wide, 8);
}
#[test]
fn wide_ep_succinct_matches_snark_verifier() {
    compare::<Pallas>(Family::Wide, 8);
}
