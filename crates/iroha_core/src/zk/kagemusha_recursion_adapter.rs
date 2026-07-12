//! Fail-closed boundary for circuit-authenticated Kagemusha recursion.
//!
//! A native Halo2 verifier result, proof hash, or metadata-bound polynomial
//! opening is not a recursive proof. The only acceptable future implementation
//! of [`CircuitAuthenticatedRecursionAdapter`] must parse the complete Halo2
//! proof as private circuit advice, recompute its Fiat--Shamir transcript,
//! constrain the verification-key and public-instance commitments, evaluate
//! every PLONK argument, and return a succinct IPA accumulator that is decided
//! by the canonical parameter set.
//!
//! The production proof builder still uses Axiom's Blake2b/Challenge255
//! transcript and therefore remains outside the circuit verifier.  A focused
//! test adapter below exercises the compatible circuit-native wire: Axiom 0.5
//! with a Poseidon transcript emits an opening proof ending in `(c, f)`, then
//! the prover appends the folded generator returned by a complete native
//! verification.  `snark-verifier`'s BGH19 reader consumes exactly that
//! augmented sequence and returns the IPA accumulator instead of pretending a
//! host verification receipt is recursive authority.
//!
//! The compatible adapter appends the folded-generator point after `(c, f)`,
//! constrains the full residual opening equation including `[-c] G_folded`,
//! and emits `(G_folded, u_0..u_{k-1})`. Terminal verification must decide that
//! accumulator against the canonical `ParamsIPA` generators; carrying the
//! point or hashing it is not a decision. See
//! `docs/source/offline_kagemusha_recursion_adapter.md` for the exact wire and
//! residual equation.
//!
//! The alternative BN254/KZG `Halo2Loader` experiment is test-only. A complete
//! standalone two-proof run measured a 21,312-byte outer proof and more than
//! 3.35 GiB incremental RSS at degree 16, so it cannot become a production
//! fallback for Kagemusha.

#![allow(dead_code)]

use ff::PrimeField as _;
use halo2_proofs::halo2curves::pasta::Fp;
use sha2::{Digest as _, Sha256};

use super::halo2_backend::{
    PastaParams, VerifyingKey, verify_ipa_proof, verifying_key_to_processed_bytes,
};

/// Transcript profile declared by a proof artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptProfile {
    /// Current Axiom Halo2 `Blake2bRead/Write<Challenge255>` transcript.
    AxiomBlake2bChallenge255V1,
    /// Required circuit-native Poseidon transcript for a future recursion ABI.
    CircuitPoseidonV1,
}

/// Immutable proof-profile data selected by a content-addressed artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AxiomIpaProofProfile {
    /// Halo2 evaluation-domain exponent.
    pub ipa_k: u32,
    /// Transcript implementation committed by the artifact.
    pub transcript: TranscriptProfile,
}

/// Frozen dimensions for the isolated BN254/KZG circuit-verifier experiment.
///
/// This is deliberately a proof-of-concept descriptor, not an artifact ABI and
/// not a production-availability signal. It authenticates exactly one proof and
/// one public instance to exercise the primitive needed by a future recursion
/// backend; it does not implement the full Kagemusha recursion transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KzgCircuitVerifierPocProfile {
    /// Number of application proofs authenticated by the outer circuit.
    pub inner_proof_count: usize,
    /// Public field elements re-exposed for each application proof.
    pub statement_elements_per_proof: usize,
    /// Number of scalar limbs used for each base-field coordinate.
    pub accumulator_limbs: usize,
    /// Bit width of each accumulator limb.
    pub accumulator_limb_bits: usize,
    /// Total public field elements encoding the KZG accumulator.
    pub accumulator_instance_elements: usize,
    /// Transcript profile required by both inner and outer proofs.
    pub transcript: TranscriptProfile,
}

/// Exact structural contract exercised by the one-proof circuit-verifier POC.
pub(crate) const KZG_CIRCUIT_VERIFIER_POC_PROFILE: KzgCircuitVerifierPocProfile =
    KzgCircuitVerifierPocProfile {
        inner_proof_count: 1,
        statement_elements_per_proof: 1,
        accumulator_limbs: 3,
        accumulator_limb_bits: 88,
        accumulator_instance_elements: 12,
        transcript: TranscriptProfile::CircuitPoseidonV1,
    };

/// Why the isolated recursive adapter refused to produce an accumulator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RecursionAdapterError {
    /// The artifact and parameter domain exponents differ.
    IpaDomainMismatch { expected: u32, actual: u32 },
    /// Native verification only supports the repository's current Axiom transcript.
    NativePreflightTranscriptMismatch,
    /// Full native verification rejected proof bytes, the VK, or public instances.
    NativeVerificationFailed,
    /// Blake2b/Challenge255 has no circuit implementation in the recursive adapter.
    TranscriptNotCircuitNative,
    /// The Axiom-0.5 PLONK + IPA circuit verifier/accumulator is not implemented.
    CircuitVerifierUnavailable,
    /// Hashes or host-verification receipts cannot authorize recursive lineage.
    HostCertificateRejected,
}

/// Diagnostic output from full native verification.
///
/// This type is intentionally not a recursive accumulator and none of its
/// fields are accepted by the recursive adapter. The digests are useful only
/// for logs/tests that do not contain secret witness material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativePreflightReceipt {
    proof_digest: [u8; 32],
    verifier_key_digest: [u8; 32],
    public_instances_digest: [u8; 32],
    profile: AxiomIpaProofProfile,
}

/// A prohibited host certificate shape used to make the rejection boundary explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCertificate {
    /// Hash of proof transcript bytes.
    pub proof_digest: [u8; 32],
    /// Hash of processed verifier-key bytes.
    pub verifier_key_digest: [u8; 32],
    /// Hash of canonical public-instance columns.
    pub public_instances_digest: [u8; 32],
    /// Hash or projection claimed to represent the transcript.
    pub transcript_digest: [u8; 32],
}

/// Private marker that cannot be constructed from native verification output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CircuitAuthenticatedAccumulator {
    _private: (),
}

/// Required surface for a sound recursive verifier implementation.
pub(crate) trait CircuitAuthenticatedRecursionAdapter {
    /// Verify complete proofs and return a circuit-authenticated accumulator.
    ///
    /// An implementation must constrain proof parsing, transcript challenges,
    /// the complete processed VK, exact instance columns, PLONK verification,
    /// and the Axiom IPA opening/accumulation equations in one circuit.
    fn verify_and_accumulate(
        &self,
        profile: AxiomIpaProofProfile,
        previous_recursive_proof: &[u8],
        current_transition_proof: &[u8],
    ) -> Result<CircuitAuthenticatedAccumulator, RecursionAdapterError>;
}

/// Placeholder for the future Axiom-0.5 circuit verifier.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Axiom05CircuitRecursionAdapter;

impl CircuitAuthenticatedRecursionAdapter for Axiom05CircuitRecursionAdapter {
    fn verify_and_accumulate(
        &self,
        profile: AxiomIpaProofProfile,
        _previous_recursive_proof: &[u8],
        _current_transition_proof: &[u8],
    ) -> Result<CircuitAuthenticatedAccumulator, RecursionAdapterError> {
        if profile.transcript != TranscriptProfile::CircuitPoseidonV1 {
            return Err(RecursionAdapterError::TranscriptNotCircuitNative);
        }
        Err(RecursionAdapterError::CircuitVerifierUnavailable)
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn canonical_instance_digest(instances: &[&[&[Fp]]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((instances.len() as u64).to_le_bytes());
    for proof_columns in instances {
        hasher.update((proof_columns.len() as u64).to_le_bytes());
        for column in *proof_columns {
            hasher.update((column.len() as u64).to_le_bytes());
            for value in *column {
                hasher.update(value.to_repr().as_ref());
            }
        }
    }
    hasher.finalize().into()
}

/// Fully verify a current Axiom IPA proof natively for diagnostics.
///
/// Success does not confer recursive authority. In particular, the returned
/// receipt cannot be converted into [`CircuitAuthenticatedAccumulator`].
pub(crate) fn native_preflight(
    profile: AxiomIpaProofProfile,
    params: &PastaParams,
    vk: &VerifyingKey,
    proof_payload: &[u8],
    instances: &[&[&[Fp]]],
) -> Result<NativePreflightReceipt, RecursionAdapterError> {
    use halo2_proofs::poly::commitment::Params as _;

    if profile.ipa_k != params.k() {
        return Err(RecursionAdapterError::IpaDomainMismatch {
            expected: profile.ipa_k,
            actual: params.k(),
        });
    }
    if profile.transcript != TranscriptProfile::AxiomBlake2bChallenge255V1 {
        return Err(RecursionAdapterError::NativePreflightTranscriptMismatch);
    }
    verify_ipa_proof(params, vk, proof_payload, instances)
        .map_err(|_| RecursionAdapterError::NativeVerificationFailed)?;
    Ok(NativePreflightReceipt {
        proof_digest: digest(proof_payload),
        verifier_key_digest: digest(&verifying_key_to_processed_bytes(vk)),
        public_instances_digest: canonical_instance_digest(instances),
        profile,
    })
}

/// Reject a host certificate as recursive authorization.
pub(crate) fn reject_host_certificate(
    _certificate: &HostCertificate,
) -> Result<CircuitAuthenticatedAccumulator, RecursionAdapterError> {
    Err(RecursionAdapterError::HostCertificateRejected)
}

/// Refuse to upgrade a native verification receipt into recursive authority.
pub(crate) fn reject_native_receipt(
    _receipt: &NativePreflightReceipt,
) -> Result<CircuitAuthenticatedAccumulator, RecursionAdapterError> {
    Err(RecursionAdapterError::HostCertificateRejected)
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance},
    };

    use super::*;
    use crate::zk::halo2_backend::{
        Scalar, assign_advice_compat, create_ipa_proof, keygen_pk, keygen_vk, params_new,
    };

    #[derive(Clone, Default)]
    struct PublicValue {
        value: Scalar,
    }

    impl Circuit<Scalar> for PublicValue {
        type Config = (Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }

        fn synthesize(
            &self,
            (advice, instance): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let cell = layouter.assign_region(
                || "public value",
                |mut region| {
                    let cell = assign_advice_compat(
                        &mut region,
                        || "value",
                        advice,
                        0,
                        || Value::known(self.value),
                    )?;
                    Ok(cell.cell())
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct TwoPublicValues {
        left: Scalar,
        right: Scalar,
    }

    impl Circuit<Scalar> for TwoPublicValues {
        type Config = (Column<Advice>, Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let left = meta.advice_column();
            let right = meta.advice_column();
            let instance = meta.instance_column();
            for column in [left, right] {
                meta.enable_equality(column);
            }
            meta.enable_equality(instance);
            (left, right, instance)
        }

        fn synthesize(
            &self,
            (left, right, instance): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let (left_cell, right_cell) = layouter.assign_region(
                || "two public values",
                |mut region| {
                    let left_cell = assign_advice_compat(
                        &mut region,
                        || "left",
                        left,
                        0,
                        || Value::known(self.left),
                    )?;
                    let right_cell = assign_advice_compat(
                        &mut region,
                        || "right",
                        right,
                        0,
                        || Value::known(self.right),
                    )?;
                    Ok((left_cell.cell(), right_cell.cell()))
                },
            )?;
            layouter.constrain_instance(left_cell, instance, 0);
            layouter.constrain_instance(right_cell, instance, 1);
            Ok(())
        }
    }

    struct Fixture {
        params: PastaParams,
        vk: VerifyingKey,
        proof: Vec<u8>,
        value: Scalar,
    }

    fn fixture() -> Fixture {
        let params = params_new(5);
        let value = Scalar::from(7);
        let circuit = PublicValue { value };
        let vk = keygen_vk(&params, &circuit).expect("tiny verifier key");
        let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny proving key");
        let column = [value];
        let columns: [&[Scalar]; 1] = [&column];
        let proof =
            create_ipa_proof(&params, &pk, &[circuit], &[&columns]).expect("tiny Axiom IPA proof");
        Fixture {
            params,
            vk,
            proof,
            value,
        }
    }

    fn profile() -> AxiomIpaProofProfile {
        AxiomIpaProofProfile {
            ipa_k: 5,
            transcript: TranscriptProfile::AxiomBlake2bChallenge255V1,
        }
    }

    #[test]
    fn kzg_experiment_profile_cannot_masquerade_as_full_recursion() {
        assert_eq!(KZG_CIRCUIT_VERIFIER_POC_PROFILE.inner_proof_count, 1);
        assert_eq!(
            KZG_CIRCUIT_VERIFIER_POC_PROFILE.statement_elements_per_proof,
            1
        );
        assert_eq!(
            KZG_CIRCUIT_VERIFIER_POC_PROFILE.accumulator_instance_elements,
            4 * KZG_CIRCUIT_VERIFIER_POC_PROFILE.accumulator_limbs
        );
        assert_eq!(KZG_CIRCUIT_VERIFIER_POC_PROFILE.accumulator_limb_bits, 88);
        assert_eq!(
            KZG_CIRCUIT_VERIFIER_POC_PROFILE.transcript,
            TranscriptProfile::CircuitPoseidonV1
        );
        assert_eq!(
            Axiom05CircuitRecursionAdapter.verify_and_accumulate(
                AxiomIpaProofProfile {
                    ipa_k: 16,
                    transcript: TranscriptProfile::CircuitPoseidonV1,
                },
                b"kzg-poc-is-not-previous-recursion",
                b"kzg-poc-is-not-current-transition",
            ),
            Err(RecursionAdapterError::CircuitVerifierUnavailable)
        );
    }

    #[test]
    fn valid_native_preflight_cannot_be_upgraded_to_recursive_authority() {
        let fixture = fixture();
        let column = [fixture.value];
        let columns: [&[Scalar]; 1] = [&column];
        let instances: [&[&[Scalar]]; 1] = [&columns];
        let receipt = native_preflight(
            profile(),
            &fixture.params,
            &fixture.vk,
            &fixture.proof,
            &instances,
        )
        .expect("full native proof verifies");
        assert_eq!(
            reject_native_receipt(&receipt),
            Err(RecursionAdapterError::HostCertificateRejected)
        );
    }

    #[test]
    fn native_preflight_rejects_proof_tampering() {
        let fixture = fixture();
        let mut proof = fixture.proof;
        let index = proof.len() / 2;
        proof[index] ^= 1;
        let column = [fixture.value];
        let columns: [&[Scalar]; 1] = [&column];
        let instances: [&[&[Scalar]]; 1] = [&columns];
        assert_eq!(
            native_preflight(profile(), &fixture.params, &fixture.vk, &proof, &instances),
            Err(RecursionAdapterError::NativeVerificationFailed)
        );
    }

    #[test]
    fn native_preflight_rejects_verifier_key_substitution() {
        let fixture = fixture();
        let other_circuit = TwoPublicValues {
            left: Scalar::from(7),
            right: Scalar::from(9),
        };
        let other_vk = keygen_vk(&fixture.params, &other_circuit).expect("substitute VK");
        let column = [fixture.value];
        let columns: [&[Scalar]; 1] = [&column];
        let instances: [&[&[Scalar]]; 1] = [&columns];
        assert_eq!(
            native_preflight(
                profile(),
                &fixture.params,
                &other_vk,
                &fixture.proof,
                &instances,
            ),
            Err(RecursionAdapterError::NativeVerificationFailed)
        );
    }

    #[test]
    fn native_preflight_rejects_instance_substitution() {
        let fixture = fixture();
        let column = [Scalar::from(8)];
        let columns: [&[Scalar]; 1] = [&column];
        let instances: [&[&[Scalar]]; 1] = [&columns];
        assert_eq!(
            native_preflight(
                profile(),
                &fixture.params,
                &fixture.vk,
                &fixture.proof,
                &instances,
            ),
            Err(RecursionAdapterError::NativeVerificationFailed)
        );
    }

    #[test]
    fn native_preflight_rejects_transcript_profile_substitution() {
        let fixture = fixture();
        let column = [fixture.value];
        let columns: [&[Scalar]; 1] = [&column];
        let instances: [&[&[Scalar]]; 1] = [&columns];
        let substituted = AxiomIpaProofProfile {
            ipa_k: 5,
            transcript: TranscriptProfile::CircuitPoseidonV1,
        };
        assert_eq!(
            native_preflight(
                substituted,
                &fixture.params,
                &fixture.vk,
                &fixture.proof,
                &instances,
            ),
            Err(RecursionAdapterError::NativePreflightTranscriptMismatch)
        );
    }

    #[test]
    fn recursive_adapter_fails_closed_for_both_transcript_profiles() {
        let adapter = Axiom05CircuitRecursionAdapter;
        assert_eq!(
            adapter.verify_and_accumulate(profile(), b"previous", b"current"),
            Err(RecursionAdapterError::TranscriptNotCircuitNative)
        );
        let poseidon = AxiomIpaProofProfile {
            ipa_k: 5,
            transcript: TranscriptProfile::CircuitPoseidonV1,
        };
        assert_eq!(
            adapter.verify_and_accumulate(poseidon, b"previous", b"current"),
            Err(RecursionAdapterError::CircuitVerifierUnavailable)
        );
    }

    #[test]
    fn host_certificate_is_never_recursive_authority() {
        let base = HostCertificate {
            proof_digest: [1; 32],
            verifier_key_digest: [2; 32],
            public_instances_digest: [3; 32],
            transcript_digest: [4; 32],
        };
        for certificate in [
            base.clone(),
            HostCertificate {
                proof_digest: [9; 32],
                ..base.clone()
            },
            HostCertificate {
                verifier_key_digest: [9; 32],
                ..base.clone()
            },
            HostCertificate {
                public_instances_digest: [9; 32],
                ..base.clone()
            },
            HostCertificate {
                transcript_digest: [9; 32],
                ..base
            },
        ] {
            assert_eq!(
                reject_host_certificate(&certificate),
                Err(RecursionAdapterError::HostCertificateRejected)
            );
        }
    }

    /// Compatibility and soundness checks for the Pasta IPA proof wire used by
    /// the circuit verifier.  This module is test-only until the application
    /// circuit emits Poseidon proofs and the recursive verifier circuit is
    /// promoted to an artifact-backed production implementation.
    mod pasta_ipa_poseidon_wire {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Eq, EqAffine, Fp},
            },
            plonk::{ProvingKey, create_proof, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::{Params as _, ParamsProver as _},
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                },
            },
        };
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider,
                ipa::{Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaSuccinctVerifyingKey},
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{
                SnarkVerifier,
                plonk::{PlonkSuccinctVerifier, PlonkVerifier},
            },
        };

        use super::PublicValue;
        use crate::zk::halo2_backend::{Scalar, keygen_pk, keygen_vk, params_new};

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EqAffine, Bgh19>;
        type FullVerifier = PlonkVerifier<As>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EqAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EqAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
            deciding_key: IpaDecidingKey<EqAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fp>>,
        }

        fn canonical_svk(params: &ParamsIPA<EqAffine>) -> IpaSuccinctVerifyingKey<EqAffine> {
            let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn create_poseidon_proof(
            params: &ParamsIPA<EqAffine>,
            pk: &ProvingKey<EqAffine>,
            circuit: PublicValue,
            instances: &[&[&[Scalar]]],
        ) -> Vec<u8> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EqAffine>,
                ProverIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EqAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EqAffine>,
            proof: &[u8],
            instances: &[&[&[Scalar]]],
        ) -> EqAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EqAffine>,
                VerifierIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = params_new(INNER_K);
            let value = Scalar::from(7);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny Pasta proving key");
            let column = [value];
            let columns: [&[Scalar]; 1] = [&column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_folded_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator = folded_generator(
                &params,
                &vk,
                &proof_without_folded_generator,
                &proof_instances,
            );
            let mut augmented_proof = proof_without_folded_generator.clone();
            augmented_proof.extend_from_slice(generator.to_bytes().as_ref());
            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![1]));
            Fixture {
                params,
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EqAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse augmented Axiom IPA proof as BGH19");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify the full PLONK residual and produce an IPA accumulator");
            assert_eq!(accumulators.len(), 1, "one proof yields one accumulator");
            accumulators.pop().expect("one accumulator")
        }

        #[test]
        fn axiom_poseidon_wire_appends_exactly_one_folded_generator() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EqAffine as GroupEncoding>::Repr>(),
                "the recursion wire is the ordinary Axiom proof plus one compressed point"
            );

            let accumulator = succinct_accumulator(&fixture);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                accumulator.clone(),
            )
            .expect("terminal decision recomputes the folded canonical generator basis");

            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = FullVerifier::read_proof(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("full verifier parses augmented proof");
            FullVerifier::verify(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("full verifier includes terminal IPA decision");

            let substituted =
                IpaAccumulator::new(accumulator.xi.clone(), fixture.params.get_g()[1]);
            assert!(
                <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                    &fixture.deciding_key,
                    substituted,
                )
                .is_err(),
                "carrying a substituted accumulator point is not a terminal decision"
            );
        }

        #[test]
        fn folded_generator_is_constrained_by_the_plonk_opening_residual() {
            let fixture = fixture();
            let mut substituted = fixture.augmented_proof.clone();
            let replacement = fixture.params.get_g()[1].to_bytes();
            let offset = substituted.len() - replacement.as_ref().len();
            substituted[offset..].copy_from_slice(replacement.as_ref());

            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(substituted.as_slice());
                let parsed = SuccinctVerifier::read_proof(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &mut transcript,
                )
                .expect("a substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a substituted folded generator must fail the constrained residual"
            );
        }
    }

    mod kzg_poseidon_circuit_verifier {
        use std::{mem, rc::Rc};

        use halo2_base::{
            gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder},
            halo2_proofs::{
                circuit::{Layouter, SimpleFloorPlanner, Value},
                dev::MockProver,
                halo2curves::bn256::{Bn256, Fr, G1Affine},
                plonk::{
                    Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Fixed,
                    Instance, ProvingKey, Selector, create_proof, keygen_pk, keygen_vk,
                },
                poly::kzg::{
                    commitment::{KZGCommitmentScheme, ParamsKZG},
                    multiopen::ProverGWC,
                },
            },
            utils::halo2::raw_assign_fixed,
        };
        use halo2_ecc::{bn254::FpChip, ecc::BaseFieldEccChip};
        use halo2_proofs::poly::commitment::ParamsProver as _;
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::{self, Loader, ScalarLoader, native::NativeLoader},
            pcs::{
                AccumulationDecider, AccumulatorEncoding,
                kzg::{Gwc19, KzgAs, KzgDecidingKey, KzgSuccinctVerifyingKey, LimbsEncoding},
            },
            system::halo2::{
                Config, compile,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript},
            },
            util::{arithmetic::fe_to_fe, hash},
            verifier::{
                SnarkVerifier,
                plonk::{PlonkProtocol, PlonkSuccinctVerifier, PlonkVerifier},
            },
        };

        use super::assign_advice_compat;

        const LIMBS: usize = 3;
        const BITS: usize = 88;
        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;
        const OUTER_K: u32 = 16;
        const OUTER_LOOKUP_BITS: usize = OUTER_K as usize - 1;
        const ACCUMULATOR_INSTANCE_ELEMENTS: usize = 4 * LIMBS;
        const VK_DIGEST_INSTANCE_INDEX: usize = ACCUMULATOR_INSTANCE_ELEMENTS;
        const INNER_INSTANCE_INDEX: usize = VK_DIGEST_INSTANCE_INDEX + 1;
        const OUTER_INSTANCE_ELEMENTS: usize = INNER_INSTANCE_INDEX + 1;

        type As = KzgAs<Bn256, Gwc19>;
        type Encoding = LimbsEncoding<LIMBS, BITS>;
        type FullVerifier = PlonkVerifier<As, Encoding>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As, Encoding>;
        type Svk = KzgSuccinctVerifyingKey<G1Affine>;
        type Dk = KzgDecidingKey<Bn256>;
        type Transcript<L, S> = PoseidonTranscript<G1Affine, L, S, T, RATE, R_F, R_P>;
        type InCircuitLoader<'chip> =
            loader::halo2::Halo2Loader<G1Affine, BaseFieldEccChip<'chip, G1Affine>>;
        type Poseidon<L> = hash::Poseidon<Fr, L, T, RATE>;

        /// A one-instance circuit whose fixed domain separator is committed by its VK.
        ///
        /// Changing `separator` changes the VK without changing the proof layout,
        /// which gives the substitution test a parseable proof under both keys.
        /// The public value uses Halo2's equality argument instead of a direct
        /// selected instance query. Besides being the canonical public-input
        /// binding, the equality argument makes the constrained numerator reach
        /// Halo2's declared minimum quotient degree, so `snark-verifier` and the
        /// native prover agree on the exact number of quotient commitments.
        #[derive(Clone)]
        struct KzgPublicValue {
            value: Fr,
            separator: Fr,
        }

        impl Circuit<Fr> for KzgPublicValue {
            type Config = (
                Column<Advice>,
                Column<Advice>,
                Column<Fixed>,
                Column<Instance>,
                Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;
            type Params = ();

            fn without_witnesses(&self) -> Self {
                Self {
                    value: Fr::from(0),
                    separator: self.separator,
                }
            }

            fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
                let value = meta.advice_column();
                let value_minus_separator = meta.advice_column();
                let separator = meta.fixed_column();
                let instance = meta.instance_column();
                let selector = meta.selector();
                meta.enable_equality(value);
                meta.enable_equality(instance);
                meta.create_gate("KZG public value with VK domain", |meta| {
                    let selector = meta.query_selector(selector);
                    let value =
                        meta.query_advice(value, halo2_base::halo2_proofs::poly::Rotation::cur());
                    let value_minus_separator = meta.query_advice(
                        value_minus_separator,
                        halo2_base::halo2_proofs::poly::Rotation::cur(),
                    );
                    let separator = meta
                        .query_fixed(separator, halo2_base::halo2_proofs::poly::Rotation::cur());
                    vec![selector * (value_minus_separator + separator - value)]
                });
                (value, value_minus_separator, separator, instance, selector)
            }

            fn synthesize(
                &self,
                (value, value_minus_separator, separator, instance, selector): Self::Config,
                mut layouter: impl Layouter<Fr>,
            ) -> Result<(), PlonkError> {
                let value_cell = layouter.assign_region(
                    || "KZG public value with VK domain",
                    |mut region| {
                        selector.enable(&mut region, 0)?;
                        let value_cell = assign_advice_compat(
                            &mut region,
                            || "public value",
                            value,
                            0,
                            || Value::known(self.value),
                        )?;
                        assign_advice_compat(
                            &mut region,
                            || "value minus separator",
                            value_minus_separator,
                            0,
                            || Value::known(self.value - self.separator),
                        )?;
                        raw_assign_fixed(&mut region, separator, 0, self.separator);
                        Ok(value_cell.cell())
                    },
                )?;
                layouter.constrain_instance(value_cell, instance, 0);
                Ok(())
            }
        }

        struct Fixture {
            params: ParamsKZG<Bn256>,
            protocol: PlonkProtocol<G1Affine>,
            wrong_protocol: PlonkProtocol<G1Affine>,
            proof: Vec<u8>,
            substituted_proof: Vec<u8>,
            wrong_vk_proof: Vec<u8>,
            instances: Vec<Vec<Fr>>,
            vk_digest: Fr,
        }

        fn poseidon<L: Loader<G1Affine>>(
            loader: &L,
            inputs: &[L::LoadedScalar],
        ) -> L::LoadedScalar {
            let mut hasher = Poseidon::new::<R_F, R_P, SECURE_MDS>(loader);
            hasher.update(inputs);
            hasher.squeeze()
        }

        fn protocol_digest(protocol: &PlonkProtocol<G1Affine>) -> Fr {
            // Test-only mutation tripwire. This intentionally is not a
            // production verifier-key commitment: `native()` exposes field
            // residues rather than every ranged non-native limb, and the POC
            // omits protocol structure that a release digest must bind.
            let inputs = protocol
                .preprocessed
                .iter()
                .flat_map(|point| [point.x, point.y])
                .map(fe_to_fe)
                .chain(protocol.transcript_initial_state.iter().copied())
                .collect::<Vec<Fr>>();
            poseidon(&NativeLoader, &inputs)
        }

        fn create_inner_proof(
            params: &ParamsKZG<Bn256>,
            pk: &ProvingKey<G1Affine>,
            circuit: KzgPublicValue,
        ) -> Vec<u8> {
            let instances = vec![vec![circuit.value]];
            let instance_columns = instances.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::new());
            create_proof::<
                KZGCommitmentScheme<Bn256>,
                ProverGWC<'_, Bn256>,
                ChallengeScalar<G1Affine>,
                _,
                _,
                _,
            >(
                params,
                pk,
                &[circuit],
                &[instance_columns.as_slice()],
                OsRng,
                &mut transcript,
            )
            .expect("create KZG Poseidon proof");
            transcript.finalize()
        }

        fn fixture() -> Fixture {
            let params = ParamsKZG::<Bn256>::setup(INNER_K, OsRng);
            let circuit = KzgPublicValue {
                value: Fr::from(7),
                separator: Fr::from(1),
            };
            let vk = keygen_vk(&params, &circuit).expect("KZG verifier key");
            let pk = keygen_pk(&params, vk, &circuit).expect("KZG proving key");
            let proof = create_inner_proof(&params, &pk, circuit.clone());
            let substituted_proof = create_inner_proof(
                &params,
                &pk,
                KzgPublicValue {
                    value: Fr::from(8),
                    ..circuit.clone()
                },
            );
            let protocol = compile(
                &params,
                pk.get_vk(),
                Config::kzg().with_num_instance(vec![1]),
            );

            let wrong_vk_circuit = KzgPublicValue {
                value: Fr::from(7),
                separator: Fr::from(2),
            };
            let wrong_vk = keygen_vk(&params, &wrong_vk_circuit).expect("substitute KZG VK");
            let wrong_pk = keygen_pk(&params, wrong_vk, &wrong_vk_circuit)
                .expect("substitute KZG proving key");
            let wrong_vk_proof = create_inner_proof(&params, &wrong_pk, wrong_vk_circuit.clone());
            let wrong_protocol = compile(
                &params,
                wrong_pk.get_vk(),
                Config::kzg().with_num_instance(vec![1]),
            );

            Fixture {
                vk_digest: protocol_digest(&protocol),
                params,
                protocol,
                wrong_protocol,
                proof,
                substituted_proof,
                wrong_vk_proof,
                instances: vec![vec![circuit.value]],
            }
        }

        fn seed_outer_params() -> BaseCircuitParams {
            BaseCircuitParams {
                k: OUTER_K as usize,
                num_advice_per_phase: vec![1],
                num_lookup_advice_per_phase: vec![1],
                num_fixed: 1,
                lookup_bits: Some(OUTER_LOOKUP_BITS),
                num_instance_columns: 1,
            }
        }

        fn build_outer_verifier(
            config: BaseCircuitParams,
            svk: Svk,
            protocol: &PlonkProtocol<G1Affine>,
            inner_instances: &[Vec<Fr>],
            proof: &[u8],
            expected_vk_digest: Fr,
        ) -> BaseCircuitBuilder<Fr> {
            assert_eq!(inner_instances.len(), 1, "one inner instance column");
            assert_eq!(inner_instances[0].len(), 1, "one exact inner instance");

            let mut builder = BaseCircuitBuilder::new(false).use_params(config);
            let range = builder.range_chip();
            let fp_chip = FpChip::<Fr>::new(&range, BITS, LIMBS);
            let ecc_chip = BaseFieldEccChip::<G1Affine>::new(&fp_chip);
            let loader = InCircuitLoader::new(ecc_chip, mem::take(builder.pool(0)));

            let expected_vk_digest = loader.assign_scalar(expected_vk_digest);
            let loaded_protocol = protocol.loaded_preprocessed_as_witness(&loader, false);
            let vk_digest_inputs = loaded_protocol
                .preprocessed
                .iter()
                .flat_map(|point| {
                    let assigned = point.assigned();
                    [assigned.x(), assigned.y()]
                        .map(|coordinate| loader.scalar_from_assigned(*coordinate.native()))
                })
                .chain(loaded_protocol.transcript_initial_state.clone())
                .collect::<Vec<_>>();
            let actual_vk_digest = poseidon(&loader, &vk_digest_inputs);
            loader.assert_eq(
                "inner verifier-key digest",
                &actual_vk_digest,
                &expected_vk_digest,
            );

            let loaded_instances = inner_instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| loader.assign_scalar(*value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let exact_inner_instance = loaded_instances[0][0].clone().into_assigned();
            let mut transcript =
                Transcript::<Rc<InCircuitLoader<'_>>, _>::new::<SECURE_MDS>(&loader, proof);
            let parsed = SuccinctVerifier::read_proof(
                &svk,
                &loaded_protocol,
                &loaded_instances,
                &mut transcript,
            )
            .expect("parse inner KZG Poseidon proof in circuit");
            let mut accumulators =
                SuccinctVerifier::verify(&svk, &loaded_protocol, &loaded_instances, &parsed)
                    .expect("constrain inner KZG Poseidon proof in circuit");
            assert_eq!(accumulators.len(), 1, "one inner proof accumulator");
            let accumulator = accumulators.pop().expect("one accumulator");
            let lhs = accumulator.lhs.into_assigned();
            let rhs = accumulator.rhs.into_assigned();
            let accumulator_limbs = [lhs.x(), lhs.y(), rhs.x(), rhs.y()]
                .into_iter()
                .flat_map(|coordinate| coordinate.limbs().iter().copied())
                .collect::<Vec<_>>();
            assert_eq!(
                accumulator_limbs.len(),
                ACCUMULATOR_INSTANCE_ELEMENTS,
                "canonical KZG accumulator limb count"
            );
            let expected_vk_digest = expected_vk_digest.into_assigned();

            *builder.pool(0) = loader.take_ctx();
            builder.assigned_instances[0] = accumulator_limbs
                .into_iter()
                .chain([expected_vk_digest, exact_inner_instance])
                .collect();
            builder
        }

        fn outer_instances(circuit: &BaseCircuitBuilder<Fr>) -> Vec<Vec<Fr>> {
            circuit
                .assigned_instances
                .iter()
                .map(|column| column.iter().map(|value| *value.value()).collect())
                .collect()
        }

        fn assert_mock_rejects(
            label: &str,
            circuit: &BaseCircuitBuilder<Fr>,
            instances: Vec<Vec<Fr>>,
        ) {
            let prover = MockProver::run(OUTER_K, circuit, instances)
                .unwrap_or_else(|err| panic!("{label} outer MockProver setup failed: {err}"));
            assert!(prover.verify().is_err(), "{label} substitution must reject");
        }

        fn assert_exposed_accumulator_decides(deciding_key: &Dk, limbs: &[Fr]) {
            let limb_refs = limbs.iter().collect::<Vec<_>>();
            let accumulator =
                <Encoding as AccumulatorEncoding<G1Affine, NativeLoader>>::from_repr(&limb_refs)
                    .expect("decode canonical exposed KZG accumulator limbs");
            <As as AccumulationDecider<G1Affine, NativeLoader>>::decide(deciding_key, accumulator)
                .expect("canonical exposed KZG accumulator pairing decision");
        }

        /// Keeps the public-instance fixture binding in the ordinary fast test set.
        #[test]
        fn kzg_public_value_fixture_binds_its_instance() {
            let profile = super::super::KZG_CIRCUIT_VERIFIER_POC_PROFILE;
            assert_eq!(profile.inner_proof_count, 1);
            assert_eq!(profile.statement_elements_per_proof, 1);
            assert_eq!(profile.accumulator_limbs, LIMBS);
            assert_eq!(profile.accumulator_limb_bits, BITS);
            assert_eq!(
                profile.accumulator_instance_elements,
                ACCUMULATOR_INSTANCE_ELEMENTS
            );
            assert_eq!(
                profile.transcript,
                super::super::TranscriptProfile::CircuitPoseidonV1
            );

            let circuit = KzgPublicValue {
                value: Fr::from(7),
                separator: Fr::from(1),
            };
            MockProver::run(INNER_K, &circuit, vec![vec![Fr::from(7)]])
                .expect("canonical inner MockProver")
                .assert_satisfied();
            let substituted = MockProver::run(INNER_K, &circuit, vec![vec![Fr::from(8)]])
                .expect("substituted inner MockProver");
            assert!(
                substituted.verify().is_err(),
                "inner public-instance substitution must reject"
            );
        }

        /// Proves that the pinned Axiom/snark-verifier stack agrees on the exact
        /// KZG + circuit-native Poseidon proof wire. This host check decides the
        /// same accumulator that the in-circuit test exposes below.
        #[test]
        fn axiom_kzg_poseidon_wire_is_verified_by_snark_verifier() {
            let fixture = fixture();
            assert!(
                fixture.proof.len()
                    < iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
                "tiny KZG proof wire unexpectedly exceeds the complete peer budget: {} bytes",
                fixture.proof.len()
            );
            let deciding_key: Dk = (
                fixture.params.get_g()[0],
                fixture.params.g2(),
                fixture.params.s_g2(),
            )
                .into();
            let verify = |protocol: &PlonkProtocol<G1Affine>,
                          proof: &[u8],
                          instances: &[Vec<Fr>]| {
                let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
                let parsed =
                    FullVerifier::read_proof(&deciding_key, protocol, instances, &mut transcript)?;
                FullVerifier::verify(&deciding_key, protocol, instances, &parsed)
            };
            verify(&fixture.protocol, &fixture.proof, &fixture.instances)
                .expect("snark-verifier accepts canonical proof");
            assert!(
                verify(
                    &fixture.protocol,
                    &fixture.substituted_proof,
                    &fixture.instances
                )
                .is_err(),
                "a valid proof for a different instance must reject"
            );
            assert!(
                verify(&fixture.protocol, &fixture.proof, &[vec![Fr::from(8)]]).is_err(),
                "instance substitution must reject"
            );
            assert_ne!(
                fixture.vk_digest,
                protocol_digest(&fixture.wrong_protocol),
                "same-layout verifier keys must have distinct transcript-bound digests"
            );
            verify(
                &fixture.wrong_protocol,
                &fixture.wrong_vk_proof,
                &fixture.instances,
            )
            .expect("the alternate verifier key and its own proof form a valid control");
            assert!(
                verify(
                    &fixture.protocol,
                    &fixture.wrong_vk_proof,
                    &fixture.instances
                )
                .is_err(),
                "a proof created under a substituted verifier key must reject"
            );
            assert!(
                verify(&fixture.wrong_protocol, &fixture.proof, &fixture.instances).is_err(),
                "the canonical proof must reject under a substituted verifier key"
            );
        }

        /// Builds the one-proof `Halo2Loader` verifier primitive, exposes its
        /// accumulator, VK digest, and exact inner instance, then attacks every
        /// binding with parseable proofs and same-shape verifier keys. This is
        /// deliberately not the full Kagemusha recursion backend.
        #[test]
        #[ignore = "BN254/KZG verifier MockProver measured 3.73 GiB RSS at k=16; run only as an explicit host experiment"]
        fn halo2_loader_verifier_binds_proof_vk_accumulator_and_instance() {
            let fixture = fixture();
            let svk: Svk = fixture.params.get_g()[0].into();
            let mut valid = build_outer_verifier(
                seed_outer_params(),
                svk,
                &fixture.protocol,
                &fixture.instances,
                &fixture.proof,
                fixture.vk_digest,
            );
            let outer_params = valid.calculate_params(Some(9));
            let canonical_instances = outer_instances(&valid);
            assert_eq!(canonical_instances.len(), 1);
            assert_eq!(
                canonical_instances[0].len(),
                OUTER_INSTANCE_ELEMENTS,
                "12 accumulator limbs plus VK digest and inner instance"
            );
            MockProver::run(OUTER_K, &valid, canonical_instances.clone())
                .expect("canonical outer MockProver")
                .assert_satisfied();
            let deciding_key: Dk = (
                fixture.params.get_g()[0],
                fixture.params.g2(),
                fixture.params.s_g2(),
            )
                .into();
            assert_exposed_accumulator_decides(
                &deciding_key,
                &canonical_instances[0][..ACCUMULATOR_INSTANCE_ELEMENTS],
            );
            assert_eq!(
                canonical_instances[0][VK_DIGEST_INSTANCE_INDEX],
                fixture.vk_digest
            );
            assert_eq!(
                canonical_instances[0][INNER_INSTANCE_INDEX],
                fixture.instances[0][0]
            );

            for (label, index) in [
                ("accumulator limb", 0),
                ("verifier-key digest", VK_DIGEST_INSTANCE_INDEX),
                ("inner instance", INNER_INSTANCE_INDEX),
            ] {
                let mut tampered = canonical_instances.clone();
                tampered[0][index] += Fr::from(1);
                assert_mock_rejects(label, &valid, tampered);
            }

            let proof_substitution = build_outer_verifier(
                outer_params.clone(),
                svk,
                &fixture.protocol,
                &fixture.instances,
                &fixture.substituted_proof,
                fixture.vk_digest,
            );
            assert_mock_rejects("proof", &proof_substitution, canonical_instances.clone());

            let substituted_instances = vec![vec![Fr::from(8)]];
            let instance_substitution = build_outer_verifier(
                outer_params.clone(),
                svk,
                &fixture.protocol,
                &substituted_instances,
                &fixture.proof,
                fixture.vk_digest,
            );
            let mut instance_substitution_public = canonical_instances.clone();
            instance_substitution_public[0][INNER_INSTANCE_INDEX] = Fr::from(8);
            assert_mock_rejects(
                "instance",
                &instance_substitution,
                instance_substitution_public,
            );

            let vk_substitution = build_outer_verifier(
                outer_params,
                svk,
                &fixture.wrong_protocol,
                &fixture.instances,
                &fixture.wrong_vk_proof,
                fixture.vk_digest,
            );
            assert_mock_rejects("verifier key", &vk_substitution, canonical_instances);
        }
    }
}
