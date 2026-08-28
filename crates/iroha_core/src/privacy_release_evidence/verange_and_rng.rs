fn run_verange_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let profile = if maximum {
        VeRangeBitLengthV1::Bits64
    } else {
        VeRangeBitLengthV1::Bits32
    };
    let values: Vec<u64> = if maximum {
        vec![0, 1, 2, 3, 4, 5, u32::MAX.into(), u64::MAX]
    } else {
        vec![42]
    };
    let scalar_values: &[u8] = if maximum {
        &[3, 5, 7, 11, 13, 17, 19, 23]
    } else {
        &[7]
    };
    let blindings: Vec<SecretScalarV1> = scalar_values
        .iter()
        .map(|value| {
            let mut bytes = [0_u8; 32];
            bytes[31] = *value;
            SecretScalarV1::from_bytes(bytes)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<_, _>>()?;
    let commitments = values
        .iter()
        .zip(&blindings)
        .map(|(value, blinding)| {
            commit(profile, *value, blinding)
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let binding = verange_binding_v1(profile, [0x22; 32])?;
    let statement = VeRangeType1BatchStatementV1::new(profile, commitments.clone(), binding)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let mut rng = EvidenceRng06::new(stage_seed_v1(
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        case_kind,
    ));
    let proof = prove_batch(&statement, &values, &blindings, &mut rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    let proof_bytes = proof.encode();
    verify_batch_encoded(&statement, &proof_bytes)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            verange_statement_material_v1(profile, &commitments, &binding),
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut mutated_digest = binding.statement_digest;
            mutated_digest[0] ^= 0x80;
            let mutated_binding = verange_binding_v1(profile, mutated_digest)?;
            let mutated =
                VeRangeType1BatchStatementV1::new(profile, commitments.clone(), mutated_binding)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            if verify_batch_encoded(&mutated, &proof_bytes).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                verange_statement_material_v1(profile, &commitments, &mutated_binding),
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let mut corrupt = proof_bytes.clone();
            let first = corrupt
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_batch_encoded(&statement, &corrupt).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof_bytes.clone();
            let interior_index = corrupt_interior.len() / 2;
            let interior = corrupt_interior
                .get_mut(interior_index)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *interior ^= 0x01;
            if verify_batch_encoded(&statement, &corrupt_interior).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated = proof_bytes
                .get(..proof_bytes.len().saturating_sub(1))
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_batch_encoded(&statement, truncated).is_ok() {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                verange_statement_material_v1(profile, &commitments, &binding),
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof_bytes,
            u64::try_from(MAX_VERANGE_TYPE1_BATCH_PROOF_BYTES_V1)
                .expect("fixed VeRange proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(values.len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(MAX_VERANGE_TYPE1_BATCH_COMMITMENTS_V1)
                .expect("fixed VeRange batch ceiling fits u64"),
            secondary_units: u64::from(profile.bits()),
            secondary_ceiling: 64,
            relation_depth: u64::try_from(profile.rows())
                .expect("fixed VeRange matrix row count fits u64"),
            relation_depth_ceiling: 8,
        },
        failure_class,
    })
}
fn verange_binding_v1(
    profile: VeRangeBitLengthV1,
    statement_digest: [u8; 32],
) -> Result<TranscriptBindingV1<'static>, PrivacyReleaseEvidenceErrorClassV1> {
    let parameters = VeRangeParametersV1::for_profile(profile)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    if compiled.parameter_digest.as_bytes() != &parameters.parameter_digest() {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(verange_binding_from_compiled_profile_v1(
        statement_digest,
        parameters.generator_digest(),
        &compiled,
    ))
}
fn verange_binding_from_compiled_profile_v1(
    statement_digest: [u8; 32],
    generator_digest: [u8; 32],
    profile: &CompiledPrivacyProfileV1,
) -> TranscriptBindingV1<'static> {
    TranscriptBindingV1 {
        network_id: &[0x11; 32],
        genesis_hash: [0x11; 32],
        action_index: 3,
        statement_digest,
        parameter_id: *profile.parameter_id.as_bytes(),
        parameter_digest: *profile.parameter_digest.as_bytes(),
        verifier_digest: *profile.verifier_digest.as_bytes(),
        statement_schema_digest: *profile.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *profile.engine_manifest_digest.as_bytes(),
        generator_digest,
    }
}
fn verange_statement_material_v1(
    profile: VeRangeBitLengthV1,
    commitments: &[crate::privacy_engines::p256::CompressedPointV1],
    binding: &TranscriptBindingV1<'_>,
) -> Vec<u8> {
    let mut material = Vec::with_capacity(384);
    material.extend_from_slice(b"iroha.privacy.release.verange.public-statement.v1");
    material.extend_from_slice(&profile.bits().to_be_bytes());
    material.extend_from_slice(
        &u32::try_from(commitments.len())
            .expect("VeRange commitment ceiling fits u32")
            .to_be_bytes(),
    );
    for commitment in commitments {
        material.extend_from_slice(commitment.as_bytes());
    }
    append_p256_binding_material_v1(&mut material, binding);
    material
}
/// Return the exact canonical release descriptor for one closed protocol.
///
/// Runner-side aggregate validation compares this byte-for-byte; a stage
/// cannot substitute a self-consistent free-form description.
#[must_use]
pub const fn privacy_release_protocol_descriptor_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> &'static str {
    match protocol_id {
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV1 => {
            "zk-ace-pq-authorization-v1; activation=disabled; candidate-permutation=dense-mds-poseidon-goldilocks-x7; candidate-output=4-sequential-state0-words-from-one-capacity1-sponge; commitment-binding-ceiling=32-bits; required-remediation=4-independent-lane-domain-hashes+air-fri-recertification; prover=fail-closed; verifier=fail-closed; candidate-fixed-primary=4096 execution-trace rows; candidate-fixed-secondary=108 unique verifier queries; candidate-fixed-depth=12 Fp4 FRI rounds; candidate-proof-cap=1341142 exact bytes; candidate-proof-soundness=classical-ROM-work-normalized-128-bits-does-not-raise-commitment-binding; qROM-not-claimed"
        }
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
            "anonymous-pgc-k-out-of-n-v1; prover=anonymous_pgc::bootstrap::prove_bootstrap+anonymous_pgc::payment::prove_payment; verifier=anonymous_pgc::bootstrap::verify_bootstrap_encoded+anonymous_pgc::payment::verify_payment_encoded; positive-and-maximum-artifact-order=account-bootstrap,payment; payment-invariant=verified-bootstrap-payload-and-proof-digests; authoritative-root-effect=canonical-epoch1-to-epoch2-complete-account-table-successor-validation; max-primary=64 anonymity-set members; max-secondary=8 recipients; max-depth=32 range-proof bits; artifact-caps=MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,MAX_PGC_PAYMENT_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
            "verange-transparent-range-v1; prover=verange::prove_batch; verifier=verange::verify_batch_encoded; max-primary=8 commitments; max-secondary=64 range bits; max-depth=8 Figure-1 matrix rows; proof-cap=MAX_VERANGE_TYPE1_BATCH_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::IrohaZkAmsV1 => {
            "iroha-zk-ams-v1; prover=zk_ams::prove_zk_ams_batch_admission_v1+zk_ams::sign_zk_ams_provision_statement_v1; verifier=privacy_verifier::verify_privacy_envelope_v1; native-verifier=zk_ams::verify_zk_ams_batch_admission_v1+zk_ams::verify_zk_ams_provision_statement_v1; batch-wire=independent-version+exact-count+fixed-eight-slots+canonical-zero-unused-tail; batch-relation=homogeneous-issuer-policy-registry-epoch-lineage+unique-credential-digests+unique-seed-public-keys+ordered-root-chain; all-case-artifact-order=batch8-admission,successor-root-provisioning; lineage=two-sequential-single-action-transactions+distinct-intent-digests+authoritative-prestate-record-digest-to-batch-successor-record-digest-to-full-admitted-ring; max-primary=64 admitted ring members; max-secondary=8 ordered admission anchors; max-depth=64 LSAG cyclic responses; artifact-caps=MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1,MAX_ZK_AMS_LSAG_PROOF_BYTES_V1"
        }
        PrivacyProtocolIdV1::VegaExistingCredentialZkV1 => {
            "vega-existing-credential-zk-v1; prover=vega::prove_mdl_figure9_v1; verifier=privacy_verifier::verify_privacy_envelope_v1; verifier-state=privacy_verifier::validate_vega_authoritative_issuer_binding_v1+vega::verify_mdl_figure9_v1; canonical-profile=Microsoft-Vega_MC-Figure9@c0ee259053cd12eaf43ed71b5cde375452b3ee4d; wire=canonical-mc-2-plus-6-sha256-steps; signature-preflight=P1363-nonzero-scalars+low-S-required+reject-high-S-without-normalization+verify-prehash-before-inverse; fixed-primary=2359296 total application constraints; fixed-secondary=1048576 maximum circuit variables; fixed-public-inputs=14; fixed-depth=21 relaxed sumcheck rounds; proof-cap=524288 canonical bytes; issuer-state=current active self-digested append-only revision+permanent-global-p256-key-ownership+retired-p256-key-never-reactivated"
        }
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 => {
            "iroha-zk-x509-stark-p256-v1; prover=zk_x509::engine::prove_zk_x509_credential_proof_v1_with_rng; verifier=privacy_verifier::verify_privacy_envelope_v1; native-verifier=zk_x509::engine::verify_zk_x509_credential_proof_v1; wire=X5S1-containing-exactly-one-X5M1-main-and-one-X5C1-compact-ca-no-legacy; trusted-state=active-trust-anchor+active-certificate-policy+current-complete-signed-crl+current-retained-ca-root+certificate-nullifier-replay; max-primary=3 certificate-chain members; max-secondary=4 disclosed subject attributes; max-depth=64 complete-CRL entries; fixed-main=49 logical registrations across six trace groups; proof-artifact-cap=8212538 exact X5S1 bytes; outer-action-proof-cap=9437184 bytes; process-cap=300000ms+12884901888-byte-peak-rss+34359738368-byte-address-space"
        }
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1 => {
            "iroha-jindo-polynomial-commitment-v1; prover-state=jindo::prepare_jindo_privacy_action_with_rng_v1; verifier=privacy_verifier::verify_privacy_envelope_v1; native-verifier=jindo::verify_batched_evaluation_v1; exact-primary=4 polynomials; max-secondary=256 coefficients each; fixed-depth=1024 ring degree; phases=PiSplit+32x(PiAgg+PiQuad); challenge=32-independent-uniform-signed-monomials-per-phase-cardinality2048; extraction=all-distinct-signed-monomial-differences-machine-checked-units-in-four-compiled-RNS-factors; qrom-production-certificate=unavailable-pending-pinned-parallel-fiat-shamir-extractor-loss-theorem; split-challenge=uniform-nonzero-Fp-star; wire=IJP3-no-IJP1-no-IJP2; proof-cap=7159944-exact-bytes"
        }
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
            "iroha-bootle-lantern-anoncred-v1; native-lifecycle=bootle_lantern::issuer::BootleLanternIssuerKeyPairV1::generate_with_rng_v1+issuer_authorize_blind_issuance_with_rng_v1+holder_prepare_blind_issuance_with_rng_v1+issuer_blind_issue_once_encoded_with_rng_v1+holder_finalize_blind_issuance_v1+bootle_lantern::prove_bound_presentation_v1; issuance-wires=ILA1-320-byte-authorization+ILQ1-71576-byte-complete-request-with-ILB1-70344-byte-P1+ILR1-3176-byte-response; issuance-store=ILS1-strict-file-store+canonical-directory-process-lease+held-unix-exclusive-flock+bounded-worst-case-record-reservation+explicit-height-pruning+nonmutating-preflight-before-P1+atomic-Fresh-to-Processing-before-issuer-rng+durable-Completed-exact-ILR1-cache+terminal-Failed-no-reset+same-request-completed-retry-after-process-reopen-and-expiry-with-zero-rng; issuance-rng=one-health-checked-master64-per-holder-or-issuer-operation+closed-context-bound-purpose-substreams; presentation-wire=ILN1-70344-byte-P2; verifier-state=bootle_lantern::verify_bound_presentation_encoded_v1; exact-caps=ILQ1-request-bytes71576+keygen-candidates4096+keygen-parity-attempts128+authorization-id-attempts4+authorization-lifetime-blocks4096+holder-vector-attempts64+holder-coefficient-proposals256+scope-coefficient-attempts4096+issuer-preimage-attempts64+falcon-preimage-proposals-per-coefficient256+falcon-preimage-total-proposals-per-attempt262144; max-primary=8 disclosed attributes; max-secondary=32 governed allowed values per required attribute; max-depth=8 module rank; proof-cap=70344 exact ILN1 bytes; no-direct-or-trusted-issuance"
        }
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
            "orchard-halo2-actions-v1; prover=orchard::prepare_orchard_bundle_v1_with_rng+orchard::authorize_orchard_bundle_v1; verifier=orchard::verify_orchard_bundle_v1; authorization=consuming-two-phase+native-consensus-binding; max-primary=2 actions; max-secondary=2 spends; max-depth=32 note-tree levels; proof-cap=orchard::orchard_authorization_wire_size_v1(2)"
        }
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
            "monero-fcmp-plus-plus-v1; prover=fcmp_plus_plus::prove_fcmp_plus_plus_v1; verifier=fcmp_plus_plus::verify_fcmp_transaction_v1; max-primary=2 inputs; max-secondary=4 strictly-positive outputs; max-depth=32 alternating Selene/Helios curve-tree layers; proof-cap=12520 exact max-shape IFC1 bytes; bounded-challenge-and-full-proof-retry=128"
        }
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
            "iroha-ivm-private-note-stark-v1; prover=ivm_private_note::prove_ivm_private_note_v1_with_rng; verifier=ivm_private_note::verify_ivm_private_note_v1; public-input=statement+mandatory-native-consensus-binding; max-primary=2 consumed notes; max-secondary=2 created notes; fixed-depth=32 SHA-256 note-tree levels; fixed-trace=16384 rows; fixed-queries=60; proof-cap=8388608 IPS1 bytes; wallet=X25519+XChaCha20Poly1305"
        }
        PrivacyProtocolIdV1::PqMaspStarkV1 => {
            "pq-masp-stark-v1; prover=pq_masp::prove_pq_masp_v1_with_rng; verifier=pq_masp::verify_pq_masp_v1; public-input=statement+mandatory-native-consensus-binding; max-primary=2 consumed notes; max-secondary=2 created notes; fixed-depth=32 SHA-256 note-tree levels; fixed-trace=16384 rows; fixed-queries=60; proof-cap=9437184 complete PQA1 bytes; authorization=ML-DSA-65-over-statement+binding+inner-proof; wallet=ML-KEM-768+XChaCha20Poly1305"
        }
    }
}
fn stage_seed_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"iroha.privacy.release.evidence.internal-seed.v1");
    hash.update(protocol_id.canonical_label().as_bytes());
    hash.update(case_kind.canonical_label().as_bytes());
    hash.finalize().into()
}
fn stage_purpose_seed_v1(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
    purpose: &[u8],
) -> Result<[u8; 32], PrivacyReleaseEvidenceErrorClassV1> {
    let purpose_length = u64::try_from(purpose.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let mut hash = Sha256::new();
    hash.update(b"iroha.privacy.release.evidence.purpose-seed.v1");
    hash.update(stage_seed_v1(protocol_id, case_kind));
    hash.update(purpose_length.to_be_bytes());
    hash.update(purpose);
    let seed: [u8; 32] = hash.finalize().into();
    if seed.iter().all(|byte| *byte == 0) {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    Ok(seed)
}
fn sha256_v1(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
struct UnavailableIssuanceRngV1;
impl RngCore for UnavailableIssuanceRngV1 {
    fn next_u32(&mut self) -> u32 {
        0
    }
    fn next_u64(&mut self) -> u64 {
        0
    }
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        destination.fill(0);
    }
    fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), RngError06> {
        Err(RngError06::new(
            "cached Bootle/Lantern issuance must not read randomness",
        ))
    }
}
impl CryptoRng for UnavailableIssuanceRngV1 {}
struct EvidenceRng06 {
    seed: [u8; 32],
    counter: u64,
}
impl EvidenceRng06 {
    const fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}
impl RngCore for EvidenceRng06 {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_be_bytes(bytes)
    }
    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_be_bytes(bytes)
    }
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let mut offset = 0;
        while offset < destination.len() {
            let mut hash = Sha256::new();
            hash.update(b"iroha.privacy.release.evidence.rng06.v1");
            hash.update(self.seed);
            hash.update(self.counter.to_be_bytes());
            self.counter = self.counter.wrapping_add(1);
            let block: [u8; 32] = hash.finalize().into();
            let take = (destination.len() - offset).min(block.len());
            destination[offset..offset + take].copy_from_slice(&block[..take]);
            offset += take;
        }
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError06> {
        self.fill_bytes(destination);
        Ok(())
    }
}
impl CryptoRng for EvidenceRng06 {}
struct EvidenceRng09 {
    seed: [u8; 32],
    counter: u64,
}
impl EvidenceRng09 {
    const fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}
impl rand::TryRngCore for EvidenceRng09 {
    type Error = core::convert::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0_u8; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_be_bytes(bytes))
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0_u8; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_be_bytes(bytes))
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        let mut offset = 0;
        while offset < destination.len() {
            let mut hash = Sha256::new();
            hash.update(b"iroha.privacy.release.evidence.rng09.v1");
            hash.update(self.seed);
            hash.update(self.counter.to_be_bytes());
            self.counter = self.counter.wrapping_add(1);
            let block: [u8; 32] = hash.finalize().into();
            let take = (destination.len() - offset).min(block.len());
            destination[offset..offset + take].copy_from_slice(&block[..take]);
            offset += take;
        }
        Ok(())
    }
}
impl rand::TryCryptoRng for EvidenceRng09 {}
