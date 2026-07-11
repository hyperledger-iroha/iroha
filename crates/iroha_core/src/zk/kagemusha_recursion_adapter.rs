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
//! The current Axiom 0.5 IPA proof uses a Blake2b/Challenge255 transcript and an
//! opening proof ending in `(c, f)`. The generic `snark-verifier` IPA adapter
//! targets the older PSE proof layout ending in a folded generator point and a
//! scalar. Treating those formats as interchangeable would be unsound. This
//! module therefore exposes native preflight only as a diagnostic receipt and
//! deliberately provides no conversion from that receipt to a recursive
//! accumulator.

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
                    assign_advice_compat(
                        &mut region,
                        || "value",
                        advice,
                        0,
                        || Value::known(self.value),
                    )
                },
            )?;
            layouter.constrain_instance(cell.cell(), instance, 0)
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
                    Ok((left_cell, right_cell))
                },
            )?;
            layouter.constrain_instance(left_cell.cell(), instance, 0)?;
            layouter.constrain_instance(right_cell.cell(), instance, 1)
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
}
