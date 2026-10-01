//! Fixed-topology SHA identity for the exact platform evidence already constrained by P-256.
//!
//! This is a proof transcript, never a second signing subject. Its caller must provide the
//! stream returned by the original DER/CBOR equation; unconstrained byte streams cannot stand
//! in for platform approval. Native admission compares the output to its held verified original.

use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_APP_APPROVAL_PROOF_BINDING_DOMAIN_V1,
    KagemushaAppOperationApprovalSigningLayoutV1 as A,
};

use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    guard_bundle::{constant_bytes, hash},
};

/// Bind the platform original to the exact full lease retained by the Native reservation.
/// The platform digest remains independently available; this digest is the third Guard column.
/// `None` is the initial-verdict/Apple branch only, represented by an exact zero lease slot.
pub(super) fn constrain_ordinary_authorization_proof_binding_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    platform: &[PastaSha256ByteV1<F>; 32],
    lease: Option<&[PastaSha256ByteV1<F>; 32]>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let mut bytes = constant_bytes(b"iroha:kagemusha:v1:ordinary-authorization-proof-binding\0");
    bytes.extend(constant_bytes(&64_u64.to_le_bytes()));
    bytes.extend_from_slice(platform);
    bytes.extend_from_slice(lease.unwrap_or(&[PastaSha256ByteV1::constant(0); 32]));
    hash(builder.main(0), jobs, bytes)
}
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};

/// Hash the same wrapper and whole original evidence that the signature equation consumed.
/// The evidence tag is fixed by the release-selected platform circuit, not an assertion response.
/// Variable DER widths/CBOR ordering retain one fixed 311-byte evidence capacity and SHA graph.
pub(super) fn constrain_ordinary_approval_proof_binding_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    wrapper: &[PastaSha256ByteV1<F>; A::TOTAL_BYTES],
    evidence_tag: u8,
    original: &KagemushaBoundedByteStreamV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if !matches!(evidence_tag, 1 | 2) {
        return Err("ordinary approval proof transcript platform/capacity differs".into());
    }
    let tag = builder
        .main(0)
        .load_constant(F::from(u64::from(evidence_tag)));
    constrain_ordinary_selected_approval_proof_binding_v1(builder, jobs, wrapper, tag, original)
}

/// Same fixed topology for a platform tag constrained by the admitted credential union.
pub(super) fn constrain_ordinary_selected_approval_proof_binding_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    wrapper: &[PastaSha256ByteV1<F>; A::TOTAL_BYTES],
    evidence_tag: AssignedValue<F>,
    original: &KagemushaBoundedByteStreamV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if original.bytes().len() > 311 {
        return Err("ordinary approval proof transcript capacity differs".into());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut prefix = constant_bytes(KAGEMUSHA_ORDINARY_APP_APPROVAL_PROOF_BINDING_DOMAIN_V1);
    prefix.extend(constant_bytes(&(A::TOTAL_BYTES as u64).to_le_bytes()));
    prefix.extend_from_slice(wrapper);
    let first = range
        .gate()
        .sub(ctx, evidence_tag, halo2_base::QuantumCell::Constant(F::ONE));
    let second = range.gate().sub(
        ctx,
        evidence_tag,
        halo2_base::QuantumCell::Constant(F::from(2)),
    );
    let invalid = range.gate().mul(ctx, first, second);
    range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
    prefix.push(PastaSha256ByteV1::range_checked(ctx, &range, evidence_tag));
    let length_bits = PastaSha256BitV1::decompose(ctx, range.gate(), original.actual_len(), 64);
    prefix.extend(
        length_bits
            .chunks_exact(8)
            .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), bits)),
    );
    let prefix_length = prefix.len();
    let fixed_prefix_length = ctx.load_constant(F::from(prefix_length as u64));
    let fixed = KagemushaBoundedByteStreamV1::constrain(ctx, &range, prefix, fixed_prefix_length)?;
    let transcript = fixed.concat(ctx, &range, original, prefix_length + 311)?;
    let digest =
        jobs.digest_bounded_constrained(ctx, &range, transcript.bytes(), transcript.actual_len())?;
    let mut result = Vec::with_capacity(32);
    for word in digest {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            result.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    result
        .try_into()
        .map_err(|_| "ordinary approval proof SHA width differs".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pasta_sha256::PastaSha256ConfigV1;
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_data_model::kagemusha::{
        KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalEvidenceV1,
        KagemushaAppOperationApprovalPurposeV1, KagemushaAppOperationApprovalV1,
        KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
        kagemusha_ordinary_app_approval_proof_binding_digest_v1,
        kagemusha_ordinary_financial_epoch_id_v1,
    };
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
    use sha2::{Digest as _, Sha256};

    #[derive(Clone)]
    struct BindingCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }
    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for BindingCircuit<F> {
        type Config = (BaseConfig<F>, PastaSha256ConfigV1);
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;
        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows((1 << 17) - 9);
            (base, PastaSha256ConfigV1::configure(meta))
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!()
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.builder
                .synthesize(config.0, layouter.namespace(|| "ordinary binding base"))?;
            self.jobs.synthesize(
                &config.1,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << 17) - 9,
            )
        }
    }

    fn approval(apple: bool) -> KagemushaAppOperationApprovalV1 {
        let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
        let actual = fixture.verify(300).unwrap();
        let credential = actual.app_credential();
        let c = credential.subject();
        let subject = KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: c.release_id,
            provider_policy_root: fixture.release.provider_policy_root(),
            app_policy_digest: credential.static_binding_digest(),
            credential_id: credential.digest(),
            network_id: iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(
                    iroha_crypto::Hash::from_marked_bytes(c.network_id)
                        .expect("marked network identity fixture"),
                ),
            ),
            lane_commitment: c.lane_id,
            hardware_profile_id: c.hardware_profile_id,
            policy_epoch: c.policy_epoch,
            hardware_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(c).unwrap(),
            hardware_epoch_generation: c.hardware_epoch,
            operation_kind: KagemushaOperationKindV1::Bootstrap,
            transition_statement_digest: [61; 32],
            candidate_envelope_digest: [0; 32],
            terminal_body_commitment: [0; 32],
            secure_index_before: 0,
            secure_index_after: 0,
        };
        let challenge = KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
            operation_id: [62; 32],
            nonce: [63; 32],
            account_binding: c.account_binding,
            authority_policy_digest: c.app_authority_policy_digest,
            attested_key_id: c.attested_key_id,
            enrollment_digest: credential.digest(),
            subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap())
                .into(),
            normalized_guard_digest: [64; 32],
            issued_at_ms: 300,
            expires_at_ms: 9000,
            subject,
        };
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let message = challenge.canonical_signing_bytes().unwrap();
        let evidence = if apple {
            let mut auth = [0; 37];
            auth[..32].copy_from_slice(&c.app_signing_identity_digest);
            auth[32] = 0x40;
            auth[33..].copy_from_slice(&17_u32.to_be_bytes());
            let mut nonce = Sha256::new();
            nonce.update(auth);
            nonce.update(Sha256::digest(&message));
            let signature: Signature = key.sign(&nonce.finalize());
            let der = signature.to_der();
            let mut raw = vec![0xa2, 0x69];
            raw.extend_from_slice(b"signature");
            raw.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
            raw.extend_from_slice(der.as_bytes());
            raw.push(0x71);
            raw.extend_from_slice(b"authenticatorData");
            raw.extend_from_slice(&[0x58, 37]);
            raw.extend(auth);
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
        } else {
            let signature: Signature = key.sign(&message);
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            }
        };
        let approval = KagemushaAppOperationApprovalV1 {
            challenge,
            evidence,
        };
        approval
            .authenticate(
                &challenge,
                credential,
                actual.possession().app_attest_counter(),
                301,
            )
            .unwrap();
        approval
    }

    fn check<F: KagemushaPoseidonFieldV1>(apple: bool, mutation: Option<usize>) -> bool {
        let approval = approval(apple);
        let expected = kagemusha_ordinary_app_approval_proof_binding_digest_v1(&approval).unwrap();
        let mut signing = approval.challenge.canonical_signing_bytes().unwrap();
        let (tag, mut raw) = match approval.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                (1, signature_der)
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                (2, raw_assertion)
            }
        };
        if mutation == Some(0) {
            signing[A::NONCE.start] ^= 1;
        }
        if mutation == Some(1) {
            *raw.last_mut().unwrap() ^= 1;
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(17)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let wrapper = std::array::from_fn(|i| {
            let cell = ctx.load_witness(F::from(u64::from(signing[i])));
            PastaSha256ByteV1::range_checked(ctx, &range, cell)
        });
        let original = (0..311)
            .map(|i| {
                let cell = ctx.load_witness(F::from(u64::from(raw.get(i).copied().unwrap_or(0))));
                PastaSha256ByteV1::range_checked(ctx, &range, cell)
            })
            .collect();
        let len = ctx.load_witness(F::from(raw.len() as u64));
        let original = KagemushaBoundedByteStreamV1::constrain(ctx, &range, original, len).unwrap();
        let mut jobs = PastaSha256JobsV1::default();
        let digest = constrain_ordinary_approval_proof_binding_v1(
            &mut builder,
            &mut jobs,
            &wrapper,
            tag,
            &original,
        )
        .unwrap();
        for (cell, byte) in digest.iter().zip(expected) {
            range.gate().assert_is_const(
                builder.main(0),
                &cell.assigned().unwrap(),
                &F::from(u64::from(byte)),
            );
        }
        builder.calculate_params(Some(9));
        let circuit = BindingCircuit { builder, jobs };
        MockProver::run(17, &circuit, vec![vec![]])
            .unwrap()
            .verify()
            .is_ok()
    }
    #[test]
    fn model_original_platform_proof_transcript_matches_both_pasta_fields() {
        for apple in [false, true] {
            assert!(check::<Fp>(apple, None));
            assert!(check::<Fq>(apple, None));
        }
    }
    #[test]
    fn substituted_native_nonce_or_original_evidence_changes_proof_binding() {
        for apple in [false, true] {
            for mutation in [0, 1] {
                assert!(!check::<Fp>(apple, Some(mutation)));
                assert!(!check::<Fq>(apple, Some(mutation)));
            }
        }
    }

    fn check_authorization<F: KagemushaPoseidonFieldV1>(
        selected_lease: Option<[u8; 32]>,
        substituted_platform: bool,
        substituted_lease: bool,
    ) -> bool {
        let platform = [0x35; 32];
        let expected = iroha_data_model::kagemusha::
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
                platform, selected_lease,
            ).unwrap();
        let mut platform_witness = platform;
        if substituted_platform {
            platform_witness[13] ^= 1;
        }
        let mut lease_witness = selected_lease;
        if substituted_lease {
            lease_witness = Some(selected_lease.map_or([0x46; 32], |mut lease| {
                lease[7] ^= 1;
                lease
            }));
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(17)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let platform =
            super::super::guard_bundle::assign_bytes(builder.main(0), &range, &platform_witness)
                .try_into()
                .unwrap();
        let lease = lease_witness.map(|lease| {
            super::super::guard_bundle::assign_bytes(builder.main(0), &range, &lease)
                .try_into()
                .unwrap()
        });
        let mut jobs = PastaSha256JobsV1::default();
        let digest = constrain_ordinary_authorization_proof_binding_v1(
            &mut builder,
            &mut jobs,
            &platform,
            lease.as_ref(),
        )
        .unwrap();
        for (cell, byte) in digest.iter().zip(expected) {
            range.gate().assert_is_const(
                builder.main(0),
                &cell.assigned().unwrap(),
                &F::from(u64::from(byte)),
            );
        }
        builder.calculate_params(Some(9));
        MockProver::run(17, &BindingCircuit { builder, jobs }, vec![vec![]])
            .unwrap()
            .verify()
            .is_ok()
    }

    #[test]
    fn native_selected_lease_authorization_transcript_matches_both_fields() {
        for lease in [None, Some([0x45; 32])] {
            assert!(check_authorization::<Fp>(lease, false, false));
            assert!(check_authorization::<Fq>(lease, false, false));
        }
    }

    #[test]
    fn substituting_platform_or_reservation_lease_fails_both_fields() {
        for lease in [None, Some([0x45; 32])] {
            for (platform, lease_changed) in [(true, false), (false, true)] {
                assert!(!check_authorization::<Fp>(lease, platform, lease_changed));
                assert!(!check_authorization::<Fq>(lease, platform, lease_changed));
            }
        }
    }
}
