// Collective evaluated-key tests included from the parent module.
use std::{cell::Cell, collections::BTreeSet};
use super::*;
const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
fn tracked_liveness_peak<T>(operation: impl FnOnce() -> T) -> (T, u64) {
    SEEKABLE_LIVENESS_HIGH_WATER_V1.with(|peak| peak.set(0));
    TRACK_SEEKABLE_LIVENESS_V1.with(|enabled| {
        assert!(!enabled.replace(true), "nested liveness tracking");
    });
    let output = operation();
    TRACK_SEEKABLE_LIVENESS_V1.with(|enabled| assert!(enabled.replace(false)));
    let peak = SEEKABLE_LIVENESS_HIGH_WATER_V1.with(Cell::get);
    (output, peak)
}
fn test_profile() -> BgvProfile {
    BgvProfile {
        profile_id: [0x79; 32],
        ring_degree: 8,
        moduli: &TEST_MODULI,
        negacyclic_roots: &TEST_ROOTS,
        plaintext_modulus: super::super::PlaintextModulus::Tiny(17),
        error_eta: 2,
        hybrid_rns_decomposition: false,
        gadget_base_log: 8,
        gadget_digits: 8,
        max_ciphertext_bytes: 1 << 20,
        max_evaluated_key_bytes: 16 << 20,
        max_round_bytes: 16 << 20,
        max_share_bytes: 4 << 20,
        max_workspace_bytes: 16 << 20,
        max_work_units: 1 << 20,
    }
}
fn hybrid_test_profile() -> BgvProfile {
    BgvProfile {
        hybrid_rns_decomposition: true,
        gadget_base_log: 60,
        gadget_digits: TEST_MODULI.len(),
        max_workspace_bytes: 16 << 20,
        max_work_units: 16 << 20,
        ..test_profile()
    }
}
fn release_limb_test_profile() -> BgvProfile {
    let release = release_profile_v1();
    let root_exponent = release.ring_degree / 8;
    let roots = release
        .negacyclic_roots
        .iter()
        .zip(release.moduli)
        .map(|(&root, &modulus)| super::super::mod_pow(root, root_exponent as u64, modulus))
        .collect::<Vec<_>>()
        .leak();
    BgvProfile {
        profile_id: [0x7a; 32],
        ring_degree: 8,
        moduli: release.moduli,
        negacyclic_roots: roots,
        max_ciphertext_bytes: 1 << 20,
        max_evaluated_key_bytes: 1 << 30,
        max_round_bytes: 1 << 30,
        max_share_bytes: 1 << 30,
        max_workspace_bytes: 1 << 30,
        max_work_units: 100_000_000_000,
        ..release
    }
}
#[derive(Clone)]
struct TestArtifact {
    bytes: Vec<u8>,
    cursor: u64,
    publication_identity: [u8; 32],
    provider_identity: [u8; 32],
    /// Current selected entry's publication snapshot.
    snapshot_identity: Option<[u8; 32]>,
    /// Whole-artifact snapshot, unavailable until `attach_pointer` seals it.
    provider_snapshot_identity: Option<[u8; 32]>,
    artifact_sealed: bool,
    pointer: Option<ZkAmsMkheEvaluatedKeySorafsPointerV1>,
    staging: Option<ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1>,
    committed: Option<ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1>,
    finalized_headers: Vec<ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1>,
    finalized_snapshots: Vec<[u8; 32]>,
    aborted: bool,
    write_calls: usize,
    read_calls: usize,
    seek_calls: usize,
    provider_identity_calls: Cell<usize>,
    provider_pointer_calls: Cell<usize>,
    provider_snapshot_calls: usize,
    provider_payload_len_calls: usize,
    max_write: usize,
    max_read: usize,
    artifact_len_override: Option<u64>,
    short_write_call: Option<usize>,
    short_read_call: Option<usize>,
    eof_read_call: Option<usize>,
    fail_seek_call: Option<usize>,
    bias_seek_call: Option<usize>,
    fail_begin_after_staging: bool,
    fail_backfill: bool,
    fail_finalize: bool,
    substitute_on_read_call: Option<usize>,
    mutate_snapshot_on_read_call: Option<usize>,
}
impl TestArtifact {
    fn new(profile: &BgvProfile) -> Self {
        let layout = seekable_evaluated_key_layout(profile).unwrap();
        let artifact_bytes = layout.payload_bytes * 32;
        Self {
            bytes: vec![0; usize::try_from(artifact_bytes).unwrap()],
            cursor: 0,
            publication_identity: [0x31; 32],
            provider_identity: [0x51; 32],
            snapshot_identity: None,
            provider_snapshot_identity: None,
            artifact_sealed: false,
            pointer: None,
            staging: None,
            committed: None,
            finalized_headers: Vec::new(),
            finalized_snapshots: Vec::new(),
            aborted: false,
            write_calls: 0,
            read_calls: 0,
            seek_calls: 0,
            provider_identity_calls: Cell::new(0),
            provider_pointer_calls: Cell::new(0),
            provider_snapshot_calls: 0,
            provider_payload_len_calls: 0,
            max_write: 0,
            max_read: 0,
            artifact_len_override: None,
            short_write_call: None,
            short_read_call: None,
            eof_read_call: None,
            fail_seek_call: None,
            bias_seek_call: None,
            fail_begin_after_staging: false,
            fail_backfill: false,
            fail_finalize: false,
            substitute_on_read_call: None,
            mutate_snapshot_on_read_call: None,
        }
    }
    fn attach_pointer(&mut self) -> ZkAmsMkheEvaluatedKeySorafsPointerV1 {
        assert!(self.staging.is_none(), "cannot expose a staging artifact");
        let payload_blake3 = blake3_hash(&self.bytes);
        let pointer = ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
            payload_blake3,
            self.bytes.len() as u64,
            [0xa1; 32],
            [0xa2; 32],
            [0xa3; 32],
        )
        .unwrap();
        self.provider_snapshot_identity = Some(payload_blake3);
        self.artifact_sealed = true;
        self.pointer = Some(pointer);
        pointer
    }
    fn copy_into_cursor(&mut self, source: &[u8]) -> usize {
        let start = usize::try_from(self.cursor).unwrap_or(usize::MAX);
        let available = self.bytes.len().saturating_sub(start);
        let take = source.len().min(available);
        if take != 0 {
            self.bytes[start..start + take].copy_from_slice(&source[..take]);
            self.cursor += take as u64;
        }
        take
    }
    fn copy_from_cursor(&mut self, destination: &mut [u8]) -> usize {
        let start = usize::try_from(self.cursor).unwrap_or(usize::MAX);
        let available = self.bytes.len().saturating_sub(start);
        let take = destination.len().min(available);
        if take != 0 {
            destination[..take].copy_from_slice(&self.bytes[start..start + take]);
            self.cursor += take as u64;
        }
        take
    }
}
impl ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 for TestArtifact {
    fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }
    fn artifact_len(&mut self) -> Result<u64, ZkAmsMkheErrorV1> {
        Ok(self
            .artifact_len_override
            .unwrap_or(self.bytes.len() as u64))
    }
    fn begin_entry(
        &mut self,
        header: ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let end = header
            .payload_offset()
            .checked_add(header.payload_bytes())
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let overlaps_finalized = self.finalized_headers.iter().any(|finalized| {
            let finalized_end = finalized
                .payload_offset()
                .checked_add(finalized.payload_bytes())
                .expect("test finalized range");
            header.payload_offset() < finalized_end && finalized.payload_offset() < end
        });
        if self.staging.is_some()
            || overlaps_finalized
            || self.artifact_sealed
            || header.artifact_bytes() != self.bytes.len() as u64
            || end > header.artifact_bytes()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.staging = Some(header);
        self.snapshot_identity = None;
        self.committed = None;
        self.cursor = header.payload_offset();
        self.aborted = false;
        if self.fail_begin_after_staging {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn position(&mut self) -> Result<u64, ZkAmsMkheErrorV1> {
        Ok(self.cursor)
    }
    fn seek(&mut self, absolute_offset: u64) -> Result<(), ZkAmsMkheErrorV1> {
        self.seek_calls += 1;
        if self.fail_seek_call == Some(self.seek_calls) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.cursor = if self.bias_seek_call == Some(self.seek_calls) {
            absolute_offset.saturating_add(1)
        } else {
            absolute_offset
        };
        Ok(())
    }
    fn write(&mut self, source: &[u8]) -> Result<usize, ZkAmsMkheErrorV1> {
        self.write_calls += 1;
        self.max_write = self.max_write.max(source.len());
        if self.fail_backfill && source.starts_with(&SEEKABLE_EVALUATED_KEY_TAG_V1) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let requested = if self.short_write_call == Some(self.write_calls) {
            source.len().saturating_sub(1)
        } else {
            source.len()
        };
        Ok(self.copy_into_cursor(&source[..requested]))
    }
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, ZkAmsMkheErrorV1> {
        self.read_calls += 1;
        self.max_read = self.max_read.max(destination.len());
        if self.substitute_on_read_call == Some(self.read_calls) {
            let index = usize::try_from(self.cursor)
                .unwrap()
                .saturating_add(destination.len().saturating_sub(1));
            if let Some(byte) = self.bytes.get_mut(index) {
                *byte ^= 1;
            }
        }
        if self.mutate_snapshot_on_read_call == Some(self.read_calls) {
            self.provider_snapshot_identity = Some([0x77; 32]);
        }
        if self.eof_read_call == Some(self.read_calls) {
            return Ok(0);
        }
        let requested = if self.short_read_call == Some(self.read_calls) {
            destination.len().saturating_sub(1)
        } else {
            destination.len()
        };
        Ok(self.copy_from_cursor(&mut destination[..requested]))
    }
    fn flush_and_finalize_entry(
        &mut self,
        footer: ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.fail_finalize {
            return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
        }
        if self.staging != Some(footer.header()) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let snapshot = footer.payload_blake3();
        self.snapshot_identity = Some(snapshot);
        self.committed = Some(footer);
        self.finalized_headers.push(footer.header());
        self.finalized_snapshots.push(snapshot);
        self.staging = None;
        Ok(snapshot)
    }
    fn finalized_snapshot_identity(&mut self) -> Result<Option<[u8; 32]>, ZkAmsMkheErrorV1> {
        Ok(self.snapshot_identity)
    }
    fn abort_entry(&mut self, header: ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1) {
        if self.staging == Some(header) {
            self.staging = None;
            self.snapshot_identity = None;
            self.committed = None;
        }
        self.aborted = true;
    }
}
impl ZkAmsMkheCollectiveEvaluatedKeyProviderV1 for TestArtifact {
    fn provider_identity(&self) -> [u8; 32] {
        self.provider_identity_calls
            .set(self.provider_identity_calls.get() + 1);
        self.provider_identity
    }
    fn sorafs_pointer(&self) -> ZkAmsMkheEvaluatedKeySorafsPointerV1 {
        self.provider_pointer_calls
            .set(self.provider_pointer_calls.get() + 1);
        self.pointer.expect("test provider pointer")
    }
    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.provider_snapshot_calls += 1;
        Ok(self.provider_snapshot_identity.unwrap_or([0; 32]))
    }
    fn payload_len(&mut self) -> Result<u64, ZkAmsMkheErrorV1> {
        self.provider_payload_len_calls += 1;
        Ok(self
            .artifact_len_override
            .unwrap_or(self.bytes.len() as u64))
    }
    fn seek(&mut self, absolute_offset: u64) -> Result<(), ZkAmsMkheErrorV1> {
        <Self as ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1>::seek(self, absolute_offset)
    }
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, ZkAmsMkheErrorV1> {
        <Self as ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1>::read(self, destination)
    }
}
fn published_test_key_at(
    profile: &BgvProfile,
    sink: &mut TestArtifact,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
) -> Result<
    (
        ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1,
        Vec<RnsPolynomial>,
    ),
    ZkAmsMkheErrorV1,
> {
    let mut publication = SeekableEvaluatedKeyPublicationTransactionV1::begin(
        profile, purpose, ordinal, exponent, sink,
    )?;
    let mut digits = Vec::new();
    for digit_index in 0..profile.gadget_digits {
        let values: [u64; 8] = std::array::from_fn(|coefficient| {
            (digit_index as u64 + 3) * (coefficient as u64 + 7) + u64::from(ordinal) + 11
        });
        let digit = RnsPolynomial::from_unsigned(profile, &values)?;
        publication.write_digit(profile, digit_index, &digit.coefficients)?;
        digits.push(digit);
    }
    let generated = publication.finish(
        SeekablePublicationFinishContextV1 {
            profile,
            profile_digest: profile.digest()?,
            roster_digest: [0x62; 32],
            epoch: 9,
            transcript_digest: [0x63; 32],
            collective_key_digest: [0x64; 32],
        },
        [0x65_u8.wrapping_add(ordinal); 32],
        [0x66; 32],
        [0x67; 32],
    )?;
    Ok((generated, digits))
}
fn published_test_key(
    profile: &BgvProfile,
    sink: &mut TestArtifact,
) -> Result<
    (
        ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1,
        Vec<RnsPolynomial>,
    ),
    ZkAmsMkheErrorV1,
> {
    published_test_key_at(
        profile,
        sink,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
        0,
        0,
    )
}
fn expected_published_test_key(
    profile: &BgvProfile,
    generated: &ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1,
    pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
) -> SeekableEvaluatedKeyExpectedV1 {
    let entry = generated.manifest_entry().unwrap();
    let cks_compact_output_set_digest = manual_cks_compact_output_set_digest_v1(
        entry.purpose(),
        entry.ordinal(),
        entry.galois_exponent(),
        [0x64; 32],
        (0..profile.gadget_digits).map(|digit_index| {
            let values: [u64; 8] = std::array::from_fn(|coefficient| {
                (digit_index as u64 + 3) * (coefficient as u64 + 7)
                    + u64::from(entry.ordinal())
                    + 11
            });
            let digit = RnsPolynomial::from_unsigned(profile, &values).unwrap();
            let mut digest =
                new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, digit.coefficients.len())
                    .unwrap();
            update_rns_digest_hasher(&mut digest, &digit.coefficients);
            (u32::try_from(digit_index).unwrap(), digest.finalize())
        }),
        u32::try_from(profile.gadget_digits).unwrap(),
    );
    SeekableEvaluatedKeyExpectedV1 {
        entry,
        pointer,
        artifact_key_count: 32,
        profile_digest: profile.digest().unwrap(),
        roster_digest: [0x62; 32],
        epoch: 9,
        transcript_digest: [0x63; 32],
        collective_key_digest: [0x64; 32],
        contribution_proof_digest: evaluated_key_evidence_digest(
            generated.purpose(),
            generated.ordinal(),
            generated.galois_exponent(),
            [0x64; 32],
            [0x66; 32],
            [0x67; 32],
        )
        .unwrap(),
        cks_compact_output_set_digest,
    }
}
fn validated_test_key(
    expected: SeekableEvaluatedKeyExpectedV1,
    validation: SeekableEvaluatedKeyValidationV1,
) -> ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    let runtime_context_digest = [0x71; 32];
    let evidence_set_capability_seal = [0x72; 32];
    let provider_binding_digest = seekable_provider_binding_digest(
        runtime_context_digest,
        expected.entry,
        validation.state,
        validation.a_master_seed,
        validation.contribution_proof_digest,
        evidence_set_capability_seal,
        expected.cks_compact_output_set_digest,
        validation.limb_index_digest,
    );
    ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
        runtime_context_digest,
        entry: expected.entry,
        sorafs_pointer: expected.pointer,
        provider_identity: validation.state.provider_identity,
        snapshot_identity: validation.state.snapshot_identity,
        provider_binding_digest,
        limb_index_digest: validation.limb_index_digest,
        a_master_seed: validation.a_master_seed,
        contribution_proof_digest: validation.contribution_proof_digest,
        evidence_set_capability_seal,
        cks_compact_output_set_digest: expected.cks_compact_output_set_digest,
        digits: validation.digits,
        limbs: validation.limbs,
    }
}
fn rebind_test_artifact_content(
    mut expected: SeekableEvaluatedKeyExpectedV1,
    provider: &mut TestArtifact,
) -> SeekableEvaluatedKeyExpectedV1 {
    let start = usize::try_from(expected.entry.payload_offset()).unwrap();
    let end = start + usize::try_from(expected.entry.payload_bytes()).unwrap();
    expected.entry = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
        expected.entry.ordinal(),
        expected.entry.purpose(),
        expected.entry.galois_exponent(),
        expected.entry.payload_offset(),
        expected.entry.payload_bytes(),
        blake3_hash(&provider.bytes[start..end]),
        expected.entry.source_proof_set_digest(),
        expected.entry.cks_proof_set_digest(),
    )
    .unwrap();
    expected.pointer = provider.attach_pointer();
    expected
}
#[allow(clippy::too_many_arguments)]
fn replace_test_entry_layout(
    mut expected: SeekableEvaluatedKeyExpectedV1,
    ordinal: u8,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    exponent: u32,
    payload_offset: u64,
    payload_bytes: u64,
    payload_blake3: [u8; 32],
) -> SeekableEvaluatedKeyExpectedV1 {
    expected.entry = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
        ordinal,
        purpose,
        exponent,
        payload_offset,
        payload_bytes,
        payload_blake3,
        expected.entry.source_proof_set_digest(),
        expected.entry.cks_proof_set_digest(),
    )
    .unwrap();
    expected
}
fn assert_rebound_content_rejected(
    profile: &BgvProfile,
    expected: SeekableEvaluatedKeyExpectedV1,
    mut provider: TestArtifact,
    case: &str,
) {
    let rebound = rebind_test_artifact_content(expected, &mut provider);
    assert!(
        validate_seekable_evaluated_key(profile, rebound, &mut provider).is_err(),
        "tampered case accepted after rebinding both content hashes: {case}"
    );
}
fn signed(profile: &BgvProfile, values: &[i64; 8]) -> RnsPolynomial {
    RnsPolynomial::from_signed(profile, values).unwrap()
}
fn deterministic_a(profile: &BgvProfile, digit_index: usize) -> RnsPolynomial {
    let values: [u64; 8] = std::array::from_fn(|coefficient| {
        u64::try_from((digit_index + 3) * (coefficient + 5) + coefficient * coefficient + 1)
            .unwrap()
    });
    RnsPolynomial::from_unsigned(profile, &values).unwrap()
}
fn fill_test_limb(
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
    limb_index: usize,
    output: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    if limb_index >= profile.moduli.len() || output.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    output.copy_from_slice(polynomial.limb(profile, limb_index));
    Ok(())
}
fn exact_compact_digits(
    profile: &BgvProfile,
    secret: &RnsPolynomial,
    encrypted_target: &RnsPolynomial,
) -> Vec<(RnsPolynomial, RnsPolynomial)> {
    (0..profile.gadget_digits)
        .map(|digit_index| {
            let a = deterministic_a(profile, digit_index);
            let b = encrypted_target
                .scale_gadget(digit_index, profile)
                .unwrap()
                .sub(&a.mul(secret, profile).unwrap(), profile)
                .unwrap();
            (b, a)
        })
        .collect()
}
#[test]
fn all_38_streamed_digits_match_full_balanced_reference_at_boundaries() {
    let profile = release_limb_test_profile();
    profile.validate().unwrap();
    assert_eq!(profile.gadget_digits, 38);
    let modulus = modulus_product(profile.moduli).unwrap();
    let base = 1_u64 << 60;
    let half = base / 2;
    let positive = [0, half - 1, half, half + 1, base - 1, base, base + half];
    let mut canonical = positive
        .into_iter()
        .map(|value| {
            WideUint::zero()
                .checked_add_mul_u64(WideUint::one(), value)
                .unwrap()
        })
        .collect::<Vec<_>>();
    canonical.push(
        modulus
            .checked_sub(
                WideUint::zero()
                    .checked_add_mul_u64(WideUint::one(), half + 1)
                    .unwrap(),
            )
            .unwrap(),
    );
    let residues = profile
        .moduli
        .iter()
        .flat_map(|&limb_modulus| {
            canonical
                .iter()
                .map(move |value| value.mod_u64(limb_modulus))
        })
        .collect::<Vec<_>>();
    let polynomial = RnsPolynomial::from_flat(&profile, residues).unwrap();
    let full = gadget_decompose(&profile, &polynomial).unwrap();
    let mut recomposed = RnsPolynomial::zero(&profile);
    reset_hoisted_residue_reads_v1();
    let mut first_digit = 0_usize;
    let mut batch_count = 0_usize;
    while first_digit < profile.gadget_digits {
        let digit_count =
            (profile.gadget_digits - first_digit).min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1);
        let streamed =
            HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, first_digit, digit_count)
                .unwrap();
        let end_digit = first_digit + digit_count;
        for digit_index in first_digit..end_digit {
            let observed = streamed.digit(digit_index).unwrap();
            assert_eq!(&observed, &full[digit_index], "hybrid digit {digit_index}");
            recomposed = recomposed
                .add(
                    &observed.scale_gadget(digit_index, &profile).unwrap(),
                    &profile,
                )
                .unwrap();
        }
        first_digit = end_digit;
        batch_count += 1;
    }
    assert_eq!(batch_count, 8);
    assert_eq!(
        hoisted_residue_reads_v1(),
        profile.ring_degree * profile.moduli.len() * batch_count,
        "each batch must perform exactly one coefficient-major CRT source pass"
    );
    assert_eq!(recomposed, polynomial);
    let exponent = 5;
    let materialized = polynomial.automorphism(exponent, &profile).unwrap();
    let transformed_reference = gadget_decompose(&profile, &materialized).unwrap();
    let mut first_digit = 0_usize;
    while first_digit < profile.gadget_digits {
        let digit_count =
            (profile.gadget_digits - first_digit).min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1);
        let viewed = HoistedHybridDigitBatchV1::new_automorphed_batch(
            &profile,
            &polynomial,
            exponent,
            first_digit,
            digit_count,
        )
        .unwrap();
        let end_digit = first_digit + digit_count;
        for digit_index in first_digit..end_digit {
            assert_eq!(
                viewed.digit(digit_index).unwrap(),
                transformed_reference[digit_index],
                "automorphed hybrid digit {digit_index}"
            );
        }
        first_digit = end_digit;
    }
    assert_eq!(
        full[0].limb(&profile, 0)[2],
        super::super::signed_mod(-(half as i64), profile.moduli[0])
    );
    assert_eq!(full[1].limb(&profile, 0)[2], 1);
    assert_eq!(full[0].limb(&profile, 0)[5], 0);
    assert_eq!(full[1].limb(&profile, 0)[5], 1);
    assert_eq!(full[1].limb(&profile, 0)[7], profile.moduli[0] - 1);
}
#[test]
fn every_release_batch_edge_materializes_a_nonzero_digit() {
    let profile = release_limb_test_profile();
    let base = 1_u64 << profile.gadget_base_log;
    let batch_edges = [
        (0_usize, 4_usize),
        (5, 9),
        (10, 14),
        (15, 19),
        (20, 24),
        (25, 29),
        (30, 34),
        (35, 37),
    ];
    let gadget_power = |digit_index| {
        (0..digit_index).fold(WideUint::one(), |value, _| {
            value.checked_mul_u64(base).unwrap()
        })
    };
    let canonical = batch_edges
        .into_iter()
        .map(|(first_digit, last_digit)| {
            gadget_power(first_digit)
                .checked_add_mul_u64(gadget_power(last_digit), 1)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let half_modulus = modulus_product(profile.moduli).unwrap().shr_one();
    assert!(canonical.iter().all(|value| *value <= half_modulus));
    let residues = profile
        .moduli
        .iter()
        .flat_map(|&limb_modulus| {
            canonical
                .iter()
                .map(move |value| value.mod_u64(limb_modulus))
        })
        .collect::<Vec<_>>();
    let polynomial = RnsPolynomial::from_flat(&profile, residues).unwrap();
    let reference = gadget_decompose(&profile, &polynomial).unwrap();
    let mut first_digit = 0_usize;
    while first_digit < profile.gadget_digits {
        let digit_count =
            (profile.gadget_digits - first_digit).min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1);
        let batch =
            HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, first_digit, digit_count)
                .unwrap();
        let end_digit = first_digit + digit_count;
        for digit_index in first_digit..end_digit {
            assert_eq!(
                batch.digit(digit_index).unwrap(),
                reference[digit_index],
                "release batch digit {digit_index}"
            );
        }
        first_digit = end_digit;
    }
    for (coefficient, (first_digit, last_digit)) in batch_edges.into_iter().enumerate() {
        for digit_index in [first_digit, last_digit] {
            assert_eq!(
                reference[digit_index].limb(&profile, 0)[coefficient],
                1,
                "digit {digit_index} must be nonzero at its batch-edge witness"
            );
        }
    }
}
#[test]
fn hoisted_automorphed_decomposition_matches_materialized_view() {
    let profile = hybrid_test_profile();
    let polynomial = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    let twice_degree = profile.ring_degree * 2;
    for exponent in (1..twice_degree).step_by(2) {
        let materialized = polynomial.automorphism(exponent, &profile).unwrap();
        let reference = HoistedHybridDigitBatchV1::new(&profile, &materialized).unwrap();
        let viewed =
            HoistedHybridDigitBatchV1::new_automorphed(&profile, &polynomial, exponent).unwrap();
        for digit_index in 0..profile.gadget_digits {
            assert_eq!(
                viewed.digit(digit_index).unwrap(),
                reference.digit(digit_index).unwrap(),
                "automorphism exponent {exponent}, digit {digit_index}"
            );
        }
        assert_eq!(
            exponent * inverse_odd_mod_power_of_two(exponent, twice_degree).unwrap() % twice_degree,
            1
        );
    }
    for exponent in [0, 2, twice_degree] {
        assert!(
            HoistedHybridDigitBatchV1::new_automorphed(&profile, &polynomial, exponent,).is_err()
        );
    }
    let constant = signed(&profile, &[4, -7, 9, 0, -3, 12, 1, -5]);
    let linear = signed(&profile, &[-2, 5, 0, 8, -11, 3, 7, 1]);
    let keys = (0..profile.gadget_digits)
        .map(|digit_index| {
            (
                deterministic_a(&profile, digit_index + 11),
                deterministic_a(&profile, digit_index + 29),
            )
        })
        .collect::<Vec<_>>();
    let exponent = 5;
    let materialized = polynomial.automorphism(exponent, &profile).unwrap();
    let reference = apply_compact_switch_streamed_core(
        &profile,
        clone_rns_exact(&profile, &constant).unwrap(),
        clone_rns_exact(&profile, &linear).unwrap(),
        &materialized,
        |digit_index, limb_index, output| {
            fill_test_limb(&profile, &keys[digit_index].0, limb_index, output)
        },
        |digit_index, limb_index, output| {
            fill_test_limb(&profile, &keys[digit_index].1, limb_index, output)
        },
    )
    .unwrap();
    let accounting = seekable_evaluated_key_accounting(&profile).unwrap();
    let (observed, measured_peak) = tracked_liveness_peak(|| {
        apply_compact_switch_streamed_core_with_automorphism(
            &profile,
            clone_rns_exact(&profile, &constant).unwrap(),
            clone_rns_exact(&profile, &linear).unwrap(),
            &polynomial,
            Some(exponent),
            |digit_index, limb_index, output| {
                fill_test_limb(&profile, &keys[digit_index].0, limb_index, output)
            },
            |digit_index, limb_index, output| {
                fill_test_limb(&profile, &keys[digit_index].1, limb_index, output)
            },
        )
    });
    assert_eq!(observed.unwrap(), reference);
    assert_eq!(measured_peak, accounting.peak_managed_workspace_bytes);
}
#[test]
fn hoisted_decomposition_reads_each_source_residue_once_for_all_digits() {
    let profile = hybrid_test_profile();
    let polynomial = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    reset_hoisted_residue_reads_v1();
    let hoisted = HoistedHybridDigitBatchV1::new(&profile, &polynomial).unwrap();
    let expected_reads = profile.ring_degree * profile.moduli.len();
    assert_eq!(hoisted_residue_reads_v1(), expected_reads);
    for digit_index in (0..profile.gadget_digits).rev() {
        hoisted.digit(digit_index).unwrap();
    }
    assert_eq!(
        hoisted_residue_reads_v1(),
        expected_reads,
        "materializing any digit order must not repeat CRT source reads"
    );
}
#[test]
fn seeded_a_and_signed_digit_limb_fills_match_native_references() {
    let profile = hybrid_test_profile();
    let parties = core::array::from_fn(|index| {
        let mut bytes = [0_u8; 32];
        bytes[31] = u8::try_from(index + 1).unwrap();
        ZkAmsMkhePartyIdV1::new(bytes).unwrap()
    });
    let roster =
        ZkAmsMkheGovernedRosterWireV1::new(release_profile_v1().digest().unwrap(), 9, parties)
            .unwrap();
    let transcript_digest = [0x81; 32];
    let collective_key_digest = [0x82; 32];
    let master_seed = [0x83; 32];
    for digit_index in 0..profile.gadget_digits {
        let mut legacy_context = Vec::with_capacity(192);
        legacy_context.extend_from_slice(&roster.profile_digest());
        legacy_context.extend_from_slice(&roster.roster_digest());
        legacy_context.extend_from_slice(&roster.epoch().to_be_bytes());
        legacy_context.extend_from_slice(&transcript_digest);
        legacy_context.extend_from_slice(&collective_key_digest);
        legacy_context.extend_from_slice(&[
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization as u8,
            0,
        ]);
        legacy_context.extend_from_slice(&0_u32.to_be_bytes());
        legacy_context.extend_from_slice(&master_seed);
        legacy_context.extend_from_slice(&u16::try_from(digit_index).unwrap().to_be_bytes());
        assert_eq!(
            legacy_context.len(),
            EVALUATED_KEY_TARGET_A_CONTEXT_BYTES_V1
        );
        let legacy = derive_uniform_rns_from_context(
            &profile,
            EVALUATED_KEY_TARGET_A_DOMAIN_V1,
            &legacy_context,
        )
        .unwrap();
        let full = derive_target_a(
            &profile,
            &roster,
            transcript_digest,
            collective_key_digest,
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            0,
            0,
            master_seed,
            digit_index,
        )
        .unwrap();
        assert_eq!(
            full, legacy,
            "target-a transcript changed at digit {digit_index}"
        );
        for limb_index in 0..profile.moduli.len() {
            let mut limb = vec![0_u64; profile.ring_degree];
            derive_target_a_limb(
                &profile,
                &roster,
                transcript_digest,
                collective_key_digest,
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
                0,
                0,
                master_seed,
                digit_index,
                limb_index,
                &mut limb,
            )
            .unwrap();
            assert_eq!(limb.as_slice(), full.limb(&profile, limb_index));
        }
    }
    let polynomial = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    let batch = HoistedHybridDigitBatchV1::new(&profile, &polynomial).unwrap();
    for digit_index in 0..profile.gadget_digits {
        let full = batch.digit(digit_index).unwrap();
        for limb_index in 0..profile.moduli.len() {
            let mut limb = vec![0_u64; profile.ring_degree];
            batch
                .fill_digit_limb(digit_index, limb_index, &mut limb)
                .unwrap();
            assert_eq!(limb.as_slice(), full.limb(&profile, limb_index));
        }
    }
}
#[test]
fn release_hoisted_batches_cover_every_digit_with_frozen_workspace_bound() {
    let profile = release_profile_v1();
    let mut first_digit = 0_usize;
    let mut batches = Vec::new();
    while first_digit < profile.gadget_digits {
        let count = (profile.gadget_digits - first_digit).min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1);
        batches.push((first_digit, count));
        first_digit += count;
    }
    assert_eq!(batches.len(), 8);
    assert_eq!(
        batches,
        vec![
            (0, 5),
            (5, 5),
            (10, 5),
            (15, 5),
            (20, 5),
            (25, 5),
            (30, 5),
            (35, 3)
        ]
    );
    assert_eq!(hoisted_hybrid_decomposition_passes(&profile).unwrap(), 662);
    let accounting = seekable_evaluated_key_accounting(&profile).unwrap();
    assert_eq!(accounting.signed_decomposition_scratch_bytes, 5_242_880);
    let workspace_ceiling = u64::try_from(profile.max_workspace_bytes).unwrap();
    assert_eq!(workspace_ceiling, 167_772_160);
    assert_eq!(accounting.peak_managed_workspace_bytes, 87_042_112);
    assert_eq!(
        workspace_ceiling - accounting.peak_managed_workspace_bytes,
        80_730_048
    );
    assert!(
        accounting.peak_managed_workspace_bytes <= workspace_ceiling,
        "the frozen five-digit schedule must retain honest allocator headroom"
    );
}
#[test]
fn hoisted_batch_bounds_fail_closed() {
    let profile = release_limb_test_profile();
    let polynomial = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    assert!(matches!(
        HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, 0, 0),
        Err(ZkAmsMkheErrorV1::InvalidProfile)
    ));
    assert!(matches!(
        HoistedHybridDigitBatchV1::new_batch(
            &profile,
            &polynomial,
            0,
            HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1 + 1,
        ),
        Err(ZkAmsMkheErrorV1::InvalidProfile)
    ));
    assert!(matches!(
        HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, profile.gadget_digits, 1,),
        Err(ZkAmsMkheErrorV1::InvalidProfile)
    ));
    assert!(matches!(
        HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, profile.gadget_digits - 3, 4,),
        Err(ZkAmsMkheErrorV1::InvalidProfile)
    ));
    assert!(matches!(
        HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, usize::MAX, 1),
        Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    ));
    let last =
        HoistedHybridDigitBatchV1::new_batch(&profile, &polynomial, profile.gadget_digits - 3, 3)
            .unwrap();
    assert_eq!(
        last.digit(profile.gadget_digits - 4),
        Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
    );
    assert_eq!(
        last.digit(profile.gadget_digits),
        Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
    );
    assert_eq!(
        last.digit(usize::MAX),
        Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
    );
}
#[test]
fn streamed_switch_runtime_liveness_high_water_matches_exact_certificate() {
    let profile = hybrid_test_profile();
    profile.validate().unwrap();
    let constant = signed(&profile, &[4, -7, 9, 0, -3, 12, 1, -5]);
    let linear = signed(&profile, &[-2, 5, 0, 8, -11, 3, 7, 1]);
    let switched = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    let keys = (0..profile.gadget_digits)
        .map(|digit_index| {
            (
                deterministic_a(&profile, digit_index + 11),
                deterministic_a(&profile, digit_index + 29),
            )
        })
        .collect::<Vec<_>>();
    let accounting = seekable_evaluated_key_accounting(&profile).unwrap();
    let (result, measured_peak) = tracked_liveness_peak(|| {
        apply_compact_switch_streamed_core(
            &profile,
            clone_rns_exact(&profile, &constant).unwrap(),
            clone_rns_exact(&profile, &linear).unwrap(),
            &switched,
            |digit_index, limb_index, output| {
                fill_test_limb(&profile, &keys[digit_index].0, limb_index, output)
            },
            |digit_index, limb_index, output| {
                fill_test_limb(&profile, &keys[digit_index].1, limb_index, output)
            },
        )
    });
    result.unwrap();
    assert_eq!(
        measured_peak, accounting.peak_managed_workspace_bytes,
        "the runtime liveness high water must equal the exact phase proof"
    );
    assert!(accounting.peak_managed_workspace_bytes < profile.max_workspace_bytes as u64);
    let mut one_byte_short = profile.clone();
    one_byte_short.max_workspace_bytes =
        usize::try_from(accounting.peak_managed_workspace_bytes - 1).unwrap();
    let provider_calls = Cell::new(0_usize);
    assert_eq!(
        apply_compact_switch_streamed_core(
            &one_byte_short,
            clone_rns_exact(&profile, &constant).unwrap(),
            clone_rns_exact(&profile, &linear).unwrap(),
            &switched,
            |_, _, _| {
                provider_calls.set(provider_calls.get() + 1);
                Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
            },
            |_, _, _| Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        ),
        Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    );
    assert_eq!(provider_calls.get(), 0, "resource rejection precedes I/O");
}
#[test]
fn key_switch_limb_workspace_zeroizes_on_success_error_and_unwind() {
    let profile = hybrid_test_profile();
    reset_key_switch_limb_workspace_zeroized_drops_v1();
    {
        let mut workspace = KeySwitchLimbWorkspaceV1::new(&profile).unwrap();
        workspace.evaluated_key.fill(7);
        workspace.signed_digit.fill(9);
    }
    assert_eq!(key_switch_limb_workspace_zeroized_drops_v1(), 1);
    let constant = signed(&profile, &[4, -7, 9, 0, -3, 12, 1, -5]);
    let linear = signed(&profile, &[-2, 5, 0, 8, -11, 3, 7, 1]);
    let switched = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    let error = apply_compact_switch_streamed_core(
        &profile,
        clone_rns_exact(&profile, &constant).unwrap(),
        clone_rns_exact(&profile, &linear).unwrap(),
        &switched,
        |_, _, output| {
            output.fill(11);
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        },
        |_, _, _| Ok(()),
    );
    assert_eq!(error, Err(ZkAmsMkheErrorV1::InvalidKeyMaterial));
    assert_eq!(key_switch_limb_workspace_zeroized_drops_v1(), 2);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = apply_compact_switch_streamed_core(
            &profile,
            clone_rns_exact(&profile, &constant).unwrap(),
            clone_rns_exact(&profile, &linear).unwrap(),
            &switched,
            |_, _, output| {
                output.fill(13);
                panic!("exercise key-switch limb workspace unwind");
            },
            |_, _, _| Ok(()),
        );
    }));
    assert!(unwind.is_err());
    assert_eq!(key_switch_limb_workspace_zeroized_drops_v1(), 3);
    let source = include_str!("collective_eval_keys/runtime.rs");
    let drop_body = source
        .split_once("impl Drop for KeySwitchLimbWorkspaceV1")
        .unwrap()
        .1
        .split_once("fn multiply_accumulate_limb_in_place")
        .unwrap()
        .0;
    for required in ["black_box", ".fill(0)", "compiler_fence"] {
        assert!(drop_body.contains(required));
    }
}
#[test]
fn release_seekable_accounting_is_exact_and_io_is_not_arithmetic_work() {
    let profile = release_profile_v1();
    let accounting = seekable_evaluated_key_accounting(&profile).unwrap();
    assert_eq!(accounting.canonical_payload_bytes, 1_514_144_113);
    assert_eq!(accounting.canonical_digit_record_bytes, 39_845_893);
    assert_eq!(accounting.incremental_validation_read_bytes, 1_514_144_113);
    assert_eq!(accounting.per_key_switch_read_bytes, 1_514_143_744);
    assert_eq!(accounting.native_polynomial_allocation_bytes, 39_845_888);
    assert_eq!(accounting.native_limb_allocation_bytes, 1_048_576);
    assert_eq!(accounting.output_accumulator_bytes, 79_691_776);
    assert_eq!(accounting.signed_decomposition_scratch_bytes, 5_242_880);
    assert_eq!(accounting.crt_residue_scratch_bytes, 304);
    assert_eq!(accounting.ntt_limb_scratch_bytes, 2_097_152);
    assert_eq!(accounting.provider_read_buffer_bytes, 8_192);
    assert_eq!(accounting.provider_hash_state_bytes, 1_920);
    assert_eq!(core::mem::size_of::<SeekableEvaluatedKeyDigitV1>(), 48);
    assert_eq!(core::mem::size_of::<SeekableEvaluatedKeyLimbV1>(), 48);
    assert_eq!(accounting.validation_metadata_bytes, 71_136);
    assert_eq!(accounting.decomposition_phase_bytes, 87_032_112);
    assert_eq!(accounting.provider_read_phase_bytes, 87_041_920);
    assert_eq!(accounting.peak_heap_allocation_bytes, 87_031_808);
    assert_eq!(accounting.multiplication_phase_bytes, 87_042_112);
    assert_eq!(
        accounting.multiplication_phase_bytes - accounting.provider_read_phase_bytes,
        192,
        "batch/output/limb owners, limb views, and automorphism control are explicit"
    );
    assert_eq!(accounting.peak_managed_workspace_bytes, 87_042_112);
    assert_eq!(accounting.balanced_decomposition_work_units, 3_297_247_232);
    assert_eq!(accounting.ring_multiplication_work_units, 6_813_646_848);
    assert_eq!(accounting.accumulator_addition_work_units, 378_535_936);
    assert_eq!(accounting.total_key_switch_work_units, 10_489_430_016);
    assert_eq!(
        accounting.total_key_switch_work_units,
        accounting.balanced_decomposition_work_units
            + accounting.ring_multiplication_work_units
            + accounting.accumulator_addition_work_units
    );
    assert_ne!(
        accounting.total_key_switch_work_units,
        accounting.balanced_decomposition_work_units
            + accounting.ring_multiplication_work_units
            + accounting.accumulator_addition_work_units
            + accounting.incremental_validation_read_bytes
            + accounting.per_key_switch_read_bytes
    );
    let mut one_byte_short = profile;
    one_byte_short.max_workspace_bytes =
        usize::try_from(accounting.peak_managed_workspace_bytes - 1).unwrap();
    assert_eq!(
        seekable_evaluated_key_accounting(&one_byte_short),
        Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    );
}
#[test]
fn production_seekable_switch_source_forbids_full_digit_materialization() {
    let source = include_str!("collective_eval_keys/runtime.rs");
    let core = source
        .split_once("fn apply_compact_switch_streamed_core_with_automorphism")
        .unwrap()
        .1
        .split_once("fn apply_compact_switch_with_seekable_provider")
        .unwrap()
        .0;
    for forbidden in [
        "let plaintext_digit",
        "stored_b_digit(",
        "seeded_a_digit(",
        "multiply_accumulate_in_place",
        "negacyclic_multiply_exact",
    ] {
        assert!(
            !core.contains(forbidden),
            "forbidden full-P path: {forbidden}"
        );
    }
    for required in [
        "KeySwitchLimbWorkspaceV1::new",
        "fill_digit_limb",
        "multiply_accumulate_limb_in_place",
    ] {
        assert!(core.contains(required), "missing limb kernel: {required}");
    }
    assert!(source.contains("read_seekable_evaluated_key_limb"));
    assert!(source.contains("derive_target_a_limb"));
    let stored_limb = source
        .split_once("fn stored_b_limb")
        .unwrap()
        .1
        .split_once("fn seeded_a_limb")
        .unwrap()
        .0;
    assert!(stored_limb.contains("validate_bound_seekable_provider_state"));
    assert!(!stored_limb.contains("self.validate_provider_state"));
}
#[test]
fn production_streaming_automorphism_source_is_direct_fail_closed_and_bounded() {
    let source = include_str!("collective_eval_keys/streaming_automorphism.rs");
    let operation = source
        .split_once("pub fn automorphism_switch_zk_ams_mkhe_collective_streaming_v1")
        .unwrap()
        .1
        .split_once("pub struct ZkAmsMkheStreamingCollectiveAutomorphismAccountingV1")
        .unwrap()
        .0;
    for forbidden in [
        "RnsPolynomial::zero",
        "Vec::with_capacity",
        "plaintext_digit",
        "stored_b_digit",
        "seeded_a_digit",
    ] {
        assert!(
            !operation.contains(forbidden),
            "forbidden owner: {forbidden}"
        );
    }
    for required in [
        "whole_operation_managed_peak_bytes",
        "prepare_zk_ams_mkhe_streaming_collective_automorphism_output_v1",
        "read_automorphed_constant_v1",
        "read_automorphed_linear_batch_v1",
        "stored_b_limb",
        "seeded_a_limb",
        "multiply_accumulate_limb_in_place",
        "publish_constant_limb_v1",
        "publish_linear_limb_v1",
        "finish_v1",
    ] {
        assert!(
            operation.contains(required),
            "missing direct seam: {required}"
        );
    }
    let last_input_authentication = operation
        .rfind("runtime.validate_provider_state")
        .expect("final provider authentication");
    let first_publication = operation
        .find("output_publication.publish_constant_limb_v1")
        .expect("first output publication");
    assert!(last_input_authentication < first_publication);
    assert!(source.contains("try_reserve_exact"));
    assert!(source.contains("impl Drop for StreamingAutomorphismInputScratchV1"));
    assert!(source.contains("impl Drop for StreamingAutomorphismHybridBatchV1"));
    let runtime = include_str!("collective_eval_keys/runtime.rs");
    let lineage = runtime
        .split_once("fn output_lineage")
        .unwrap()
        .1
        .split_once("fn read_seekable_evaluated_key_limb")
        .unwrap()
        .0;
    assert!(lineage.contains("Keccak256::new"));
    assert!(!lineage.contains("Vec::with_capacity"));
}
#[test]
fn streaming_automorphism_is_the_only_production_facade_surface() {
    let facades = [
        include_str!("../mkhe.rs"),
        include_str!("../../zk_ams.rs"),
        include_str!("../../../vega.rs"),
    ];
    let production = [
        "ZkAmsMkheStreamingCollectiveAutomorphismAccountingV1",
        "automorphism_switch_zk_ams_mkhe_collective_streaming_v1",
        "zk_ams_mkhe_streaming_collective_automorphism_accounting_v1",
    ];
    let legacy = [
        "automorphism_switch_zk_ams_mkhe_collective_v1",
        "relinearize_zk_ams_mkhe_collective_v1",
    ];
    for source in facades {
        for name in production {
            let position = source.find(name).expect("production streaming facade item");
            let use_start = source[..position]
                .rfind("pub use ")
                .expect("streaming facade pub use");
            let preceding_line = source[..use_start]
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::trim);
            assert_ne!(preceding_line, Some("#[cfg(test)]"), "{name} is test-only");
        }
        for name in legacy {
            assert_eq!(source.matches(name).count(), 1, "legacy facade item {name}");
            let position = source.find(name).expect("legacy facade item");
            let gate = source[..position]
                .rfind("#[cfg(test)]")
                .expect("legacy facade cfg(test)");
            let gated_use = &source[gate..position];
            assert!(gated_use.contains("pub use "));
            assert!(gated_use.len() < 192, "detached cfg(test) for {name}");
        }
    }
}
#[test]
fn generate_publish_validate_and_streamed_switch_are_exact_end_to_end() {
    let profile = hybrid_test_profile();
    let mut publication = TestArtifact::new(&profile);
    let mut stale_provider = publication.clone();
    let (generated, stored_digits) = published_test_key(&profile, &mut publication).unwrap();
    assert!(publication.staging.is_none());
    assert!(!publication.aborted);
    assert_eq!(
        publication.snapshot_identity,
        Some(generated.snapshot_identity())
    );
    assert_eq!(
        publication.publication_identity,
        generated.publication_identity()
    );
    let committed = publication.committed.expect("finalized publication footer");
    assert_eq!(committed.payload_blake3(), generated.payload_blake3());
    assert_eq!(
        committed.source_proof_set_digest(),
        generated.source_proof_set_digest()
    );
    assert_eq!(
        committed.cks_proof_set_digest(),
        generated.cks_proof_set_digest()
    );
    assert_eq!(
        committed.header().payload_offset(),
        generated.payload_offset()
    );
    assert_eq!(
        committed.header().payload_bytes(),
        generated.payload_bytes()
    );
    assert!(core::mem::size_of_val(&generated) < 512);
    assert!(publication.max_write <= SEEKABLE_EVALUATED_KEY_READ_BYTES_V1);
    assert!(publication.max_read <= SEEKABLE_EVALUATED_KEY_READ_BYTES_V1);
    let pointer = publication.attach_pointer();
    stale_provider.pointer = Some(pointer);
    let expected = expected_published_test_key(&profile, &generated, pointer);
    assert!(validate_seekable_evaluated_key(&profile, expected, &mut stale_provider).is_err());
    let mut provider = publication.clone();
    let validation = validate_seekable_evaluated_key(&profile, expected, &mut provider).unwrap();
    assert_eq!(validation.digits.len(), profile.gadget_digits);
    assert_eq!(
        validation.limbs.len(),
        profile.gadget_digits * profile.moduli.len()
    );
    assert_ne!(validation.limb_index_digest, [0; 32]);
    let first_limb = validation.limbs[0];
    let first_limb_start = usize::try_from(first_limb.absolute_offset).unwrap();
    let first_limb_end = first_limb_start + usize::try_from(first_limb.canonical_bytes).unwrap();
    assert_ne!(
        first_limb.blake3,
        blake3_hash(&provider.bytes[first_limb_start..first_limb_end]),
        "limb authentication must include its domain/index/modulus/range frame"
    );
    assert!(validation.digits.windows(2).all(|pair| {
        pair[0].absolute_offset + pair[0].canonical_bytes == pair[1].absolute_offset
    }));
    assert!(validation.limbs.windows(2).all(|pair| {
        pair[0].absolute_offset + pair[0].canonical_bytes == pair[1].absolute_offset
            || pair[0].absolute_offset
                + pair[0].canonical_bytes
                + SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1 as u64
                == pair[1].absolute_offset
    }));
    let mut key = validated_test_key(expected, validation);
    let original_limb_offset = key.limbs[0].absolute_offset;
    key.limbs[0].absolute_offset ^= 8;
    let mut output = vec![0_u64; profile.ring_degree];
    assert!(
        read_seekable_evaluated_key_limb(&profile, &key, &mut provider, 0, 0, &mut output).is_err(),
        "mutated limb metadata must fail its handle-bound index digest"
    );
    key.limbs[0].absolute_offset = original_limb_offset;
    assert_eq!(
        key.limb_index_digest,
        seekable_evaluated_key_limb_index_digest(key.entry, &key.limbs).unwrap()
    );
    for (digit_index, stored) in stored_digits.iter().enumerate() {
        assert_eq!(
            read_seekable_evaluated_key_digit(&profile, &key, &mut provider, digit_index).unwrap(),
            *stored
        );
        for limb_index in 0..profile.moduli.len() {
            let mut observed = vec![0_u64; profile.ring_degree];
            read_seekable_evaluated_key_limb(
                &profile,
                &key,
                &mut provider,
                digit_index,
                limb_index,
                &mut observed,
            )
            .unwrap();
            assert_eq!(observed.as_slice(), stored.limb(&profile, limb_index));
        }
    }
    let constant = signed(&profile, &[4, -7, 9, 0, -3, 12, 1, -5]);
    let linear = signed(&profile, &[-2, 5, 0, 8, -11, 3, 7, 1]);
    let switched = signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]);
    let seeded_digits = (0..profile.gadget_digits)
        .map(|digit_index| deterministic_a(&profile, digit_index + 37))
        .collect::<Vec<_>>();
    let full_pairs = stored_digits
        .iter()
        .cloned()
        .zip(seeded_digits.iter().cloned())
        .collect::<Vec<_>>();
    let reference = apply_compact_switch_with_provider(
        &profile,
        &constant,
        &linear,
        &switched,
        profile.gadget_digits,
        |digit_index| Ok(full_pairs[digit_index].clone()),
    )
    .unwrap();
    let observed = apply_compact_switch_streamed_core(
        &profile,
        clone_rns_exact(&profile, &constant).unwrap(),
        clone_rns_exact(&profile, &linear).unwrap(),
        &switched,
        |digit_index, limb_index, output| {
            read_seekable_evaluated_key_limb(
                &profile,
                &key,
                &mut provider,
                digit_index,
                limb_index,
                output,
            )
        },
        |digit_index, limb_index, output| {
            fill_test_limb(&profile, &seeded_digits[digit_index], limb_index, output)
        },
    )
    .unwrap();
    assert_eq!(observed, reference);
    assert!(provider.max_read <= SEEKABLE_EVALUATED_KEY_READ_BYTES_V1);
}
#[test]
fn one_session_freezes_retries_and_reopens_all_32_canonical_entries() {
    let profile = hybrid_test_profile();
    let layout = seekable_evaluated_key_layout(&profile).unwrap();
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    assert_eq!(schedule.entries.len() + 1, 32);
    let mut publication = TestArtifact::new(&profile);
    let mut published = Vec::new();
    for ordinal in 0_u8..32 {
        let (purpose, exponent) = if ordinal == 0 {
            (ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization, 0)
        } else {
            (
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
                schedule.entries[usize::from(ordinal) - 1].exponent,
            )
        };
        if ordinal == 7 {
            let frozen_end = usize::try_from(layout.payload_bytes * u64::from(ordinal)).unwrap();
            let frozen_prefix = publication.bytes[..frozen_end].to_vec();
            let frozen_headers = publication.finalized_headers.clone();
            let frozen_snapshots = publication.finalized_snapshots.clone();
            publication.short_write_call = Some(publication.write_calls + 3);
            assert!(
                published_test_key_at(&profile, &mut publication, purpose, ordinal, exponent,)
                    .is_err()
            );
            publication.short_write_call = None;
            assert_eq!(publication.bytes[..frozen_end], frozen_prefix);
            assert_eq!(publication.finalized_headers, frozen_headers);
            assert_eq!(publication.finalized_snapshots, frozen_snapshots);
            assert!(publication.staging.is_none());
            assert!(publication.snapshot_identity.is_none());
            assert!(publication.provider_snapshot_identity.is_none());
        }
        let value =
            published_test_key_at(&profile, &mut publication, purpose, ordinal, exponent).unwrap();
        assert_eq!(
            value.0.payload_offset(),
            layout.payload_bytes * u64::from(ordinal)
        );
        assert!(publication.provider_snapshot_identity.is_none());
        published.push(value);
    }
    assert_eq!(publication.finalized_headers.len(), 32);
    assert_eq!(publication.finalized_snapshots.len(), 32);
    assert_eq!(
        publication
            .finalized_snapshots
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        32,
        "every frozen entry has a distinct immutable snapshot identity"
    );
    assert!(publication.finalized_headers.windows(2).all(|pair| {
        pair[0].payload_offset() + pair[0].payload_bytes() == pair[1].payload_offset()
    }));
    let frozen_bytes = publication.bytes.clone();
    let frozen_headers = publication.finalized_headers.clone();
    let frozen_snapshots = publication.finalized_snapshots.clone();
    let selected_snapshot = publication.snapshot_identity;
    let selected_footer = publication.committed;
    assert!(
        published_test_key_at(
            &profile,
            &mut publication,
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            0,
            0,
        )
        .is_err(),
        "duplicate ordinal was reopened"
    );
    assert_eq!(publication.bytes, frozen_bytes);
    assert_eq!(publication.finalized_headers, frozen_headers);
    assert_eq!(publication.finalized_snapshots, frozen_snapshots);
    assert_eq!(publication.snapshot_identity, selected_snapshot);
    assert_eq!(publication.committed, selected_footer);
    assert!(
        SeekableEvaluatedKeyPublicationTransactionV1::begin(
            &profile,
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
            32,
            3,
            &mut publication,
        )
        .is_err(),
        "a 33rd entry was accepted"
    );
    assert_eq!(publication.bytes, frozen_bytes);
    assert_eq!(publication.finalized_headers, frozen_headers);
    assert_eq!(publication.finalized_snapshots, frozen_snapshots);
    assert_eq!(publication.snapshot_identity, selected_snapshot);
    let alias = ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1 {
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        ordinal: 1,
        galois_exponent: schedule.entries[0].exponent,
        payload_offset: 0,
        payload_bytes: layout.payload_bytes,
        artifact_bytes: layout.payload_bytes * 32,
    };
    assert!(
        ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1::begin_entry(&mut publication, alias,)
            .is_err(),
        "an alias into a finalized region was accepted"
    );
    assert_eq!(publication.bytes, frozen_bytes);
    assert_eq!(publication.finalized_headers, frozen_headers);
    assert_eq!(publication.finalized_snapshots, frozen_snapshots);
    assert_eq!(publication.snapshot_identity, selected_snapshot);
    assert!(publication.provider_snapshot_identity.is_none());
    let pointer = publication.attach_pointer();
    assert_eq!(
        publication.provider_snapshot_identity,
        Some(pointer.payload_blake3())
    );
    let mut provider = publication.clone();
    for (generated, stored_digits) in &published {
        let expected = expected_published_test_key(&profile, generated, pointer);
        let validation =
            validate_seekable_evaluated_key(&profile, expected, &mut provider).unwrap();
        let key = validated_test_key(expected, validation);
        for (digit_index, stored) in stored_digits.iter().enumerate() {
            assert_eq!(
                read_seekable_evaluated_key_digit(&profile, &key, &mut provider, digit_index,)
                    .unwrap(),
                *stored
            );
        }
    }
    let sealed_bytes = publication.bytes.clone();
    let sealed_provider_snapshot = publication.provider_snapshot_identity;
    assert!(
        published_test_key_at(
            &profile,
            &mut publication,
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            0,
            0,
        )
        .is_err(),
        "a sealed artifact accepted a new publication"
    );
    assert_eq!(publication.bytes, sealed_bytes);
    assert_eq!(
        publication.provider_snapshot_identity,
        sealed_provider_snapshot
    );
}
#[test]
fn publication_transaction_aborts_every_partial_or_substituted_artifact() {
    let profile = hybrid_test_profile();
    let mut cases = Vec::new();
    let mut failed_begin = TestArtifact::new(&profile);
    failed_begin.fail_begin_after_staging = true;
    cases.push(failed_begin);
    let mut short_write = TestArtifact::new(&profile);
    short_write.short_write_call = Some(1);
    cases.push(short_write);
    let mut short_digit_body = TestArtifact::new(&profile);
    short_digit_body.short_write_call = Some(3);
    cases.push(short_digit_body);
    let mut short_backfill = TestArtifact::new(&profile);
    short_backfill.short_write_call = Some(2 + 2 * profile.gadget_digits);
    cases.push(short_backfill);
    let mut seek_misplacement = TestArtifact::new(&profile);
    seek_misplacement.bias_seek_call = Some(1);
    cases.push(seek_misplacement);
    let mut seek_failure = TestArtifact::new(&profile);
    seek_failure.fail_seek_call = Some(1);
    cases.push(seek_failure);
    let mut backfill_failure = TestArtifact::new(&profile);
    backfill_failure.fail_backfill = true;
    cases.push(backfill_failure);
    let mut short_reread = TestArtifact::new(&profile);
    short_reread.short_read_call = Some(1);
    cases.push(short_reread);
    let mut eof_reread = TestArtifact::new(&profile);
    eof_reread.eof_read_call = Some(1);
    cases.push(eof_reread);
    let mut substituted_reread = TestArtifact::new(&profile);
    substituted_reread.substitute_on_read_call = Some(2);
    cases.push(substituted_reread);
    let mut substituted_digit_body = TestArtifact::new(&profile);
    substituted_digit_body.substitute_on_read_call = Some(3);
    cases.push(substituted_digit_body);
    let mut finalize_failure = TestArtifact::new(&profile);
    finalize_failure.fail_finalize = true;
    cases.push(finalize_failure);
    for mut sink in cases {
        assert!(published_test_key(&profile, &mut sink).is_err());
        assert!(sink.aborted, "failed transaction must be poisoned");
        assert!(sink.staging.is_none());
        assert!(sink.snapshot_identity.is_none());
        assert!(sink.committed.is_none());
    }
    let mut wrong_length = TestArtifact::new(&profile);
    wrong_length.artifact_len_override = Some(wrong_length.bytes.len() as u64 - 1);
    assert!(published_test_key(&profile, &mut wrong_length).is_err());
    assert!(wrong_length.snapshot_identity.is_none());
    assert!(wrong_length.committed.is_none());
}
#[test]
fn provider_rechecks_identity_snapshot_pointer_length_and_digest_for_every_digit() {
    let profile = hybrid_test_profile();
    let mut publication = TestArtifact::new(&profile);
    let (generated, _) = published_test_key(&profile, &mut publication).unwrap();
    let pointer = publication.attach_pointer();
    let expected = expected_published_test_key(&profile, &generated, pointer);
    let mut provider = publication.clone();
    let validation = validate_seekable_evaluated_key(&profile, expected, &mut provider).unwrap();
    let key = validated_test_key(expected, validation);
    let mut substituted_session = provider.clone();
    substituted_session.provider_identity[0] ^= 1;
    assert!(validate_bound_seekable_provider_state(&key, &mut substituted_session).is_err());
    let mut substituted_snapshot = provider.clone();
    substituted_snapshot.provider_snapshot_identity = Some([0x91; 32]);
    assert!(validate_bound_seekable_provider_state(&key, &mut substituted_snapshot).is_err());
    let mut substituted_pointer = provider.clone();
    substituted_pointer.pointer = Some(
        ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
            pointer.payload_blake3(),
            pointer.payload_bytes(),
            [0xb1; 32],
            pointer.sorafs_manifest_blake3(),
            pointer.chunker_profile_digest(),
        )
        .unwrap(),
    );
    assert!(validate_bound_seekable_provider_state(&key, &mut substituted_pointer).is_err());
    let mut substituted_length = provider.clone();
    substituted_length.artifact_len_override = Some(pointer.payload_bytes() - 1);
    assert!(validate_bound_seekable_provider_state(&key, &mut substituted_length).is_err());
    let digit_body =
        SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1 + SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1;
    let mut stable_snapshot_substitution = provider.clone();
    stable_snapshot_substitution.bytes[digit_body] ^= 1;
    assert!(
        read_seekable_evaluated_key_digit(&profile, &key, &mut stable_snapshot_substitution, 0)
            .is_err()
    );
    let mut limb_substitution = provider.clone();
    limb_substitution.bytes[digit_body] ^= 1;
    let mut limb_output = vec![0_u64; profile.ring_degree];
    assert!(
        read_seekable_evaluated_key_limb(
            &profile,
            &key,
            &mut limb_substitution,
            0,
            0,
            &mut limb_output,
        )
        .is_err()
    );
    let mut short_read = provider.clone();
    short_read.short_read_call = Some(short_read.read_calls + 1);
    assert!(read_seekable_evaluated_key_digit(&profile, &key, &mut short_read, 0).is_err());
    let mut eof = provider.clone();
    eof.eof_read_call = Some(eof.read_calls + 1);
    assert!(read_seekable_evaluated_key_digit(&profile, &key, &mut eof, 0).is_err());
    let mut failed_seek = provider.clone();
    failed_seek.fail_seek_call = Some(failed_seek.seek_calls + 1);
    assert!(read_seekable_evaluated_key_digit(&profile, &key, &mut failed_seek, 0).is_err());
    let mut biased_seek = provider.clone();
    biased_seek.bias_seek_call = Some(biased_seek.seek_calls + 1);
    assert!(read_seekable_evaluated_key_digit(&profile, &key, &mut biased_seek, 0).is_err());
    let mut snapshot_toctou = provider.clone();
    snapshot_toctou.mutate_snapshot_on_read_call = Some(snapshot_toctou.read_calls + 1);
    assert!(read_seekable_evaluated_key_digit(&profile, &key, &mut snapshot_toctou, 0).is_err());
    let mut stable_snapshot_toctou = provider;
    stable_snapshot_toctou.substitute_on_read_call = Some(stable_snapshot_toctou.read_calls + 2);
    assert!(
        read_seekable_evaluated_key_digit(&profile, &key, &mut stable_snapshot_toctou, 0).is_err()
    );
}
#[test]
fn canonical_seekable_validation_rejects_rehashed_header_and_digit_attacks() {
    let profile = hybrid_test_profile();
    let mut publication = TestArtifact::new(&profile);
    let (generated, _) = published_test_key(&profile, &mut publication).unwrap();
    let pointer = publication.attach_pointer();
    let expected = expected_published_test_key(&profile, &generated, pointer);
    let header_mutations = [
        (0_usize, "tag"),
        (4, "version"),
        (5, "profile digest"),
        (37, "roster digest"),
        (76, "epoch"),
        (77, "transcript digest"),
        (112, "ordinal"),
        (113, "level marker"),
        (146, "contribution proof digest"),
        (178, "digit count"),
    ];
    for (offset, case) in header_mutations {
        let mut tampered = publication.clone();
        tampered.bytes[offset] ^= 1;
        assert_rebound_content_rejected(&profile, expected, tampered, case);
    }
    let mut zero_seed = publication.clone();
    zero_seed.bytes[114..146].fill(0);
    assert_rebound_content_rejected(&profile, expected, zero_seed, "zero master seed");
    let layout = seekable_evaluated_key_layout(&profile).unwrap();
    let first_prefix = SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1;
    let mut wrong_residue_count = publication.clone();
    wrong_residue_count.bytes[first_prefix + 1..first_prefix + 5].copy_from_slice(
        &u32::try_from(layout.residue_count + 1)
            .unwrap()
            .to_be_bytes(),
    );
    assert_rebound_content_rejected(
        &profile,
        expected,
        wrong_residue_count,
        "digit residue count",
    );
    let first_residue = first_prefix + SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1;
    let mut out_of_range_residue = publication.clone();
    out_of_range_residue.bytes[first_residue..first_residue + 8]
        .copy_from_slice(&profile.moduli[0].to_be_bytes());
    assert_rebound_content_rejected(
        &profile,
        expected,
        out_of_range_residue,
        "out-of-range residue",
    );
    let record_bytes = usize::try_from(layout.digit_record_bytes).unwrap();
    let first_record = first_prefix..first_prefix + record_bytes;
    let second_record = first_record.end..first_record.end + record_bytes;
    let first = publication.bytes[first_record.clone()].to_vec();
    let second = publication.bytes[second_record.clone()].to_vec();
    let mut reordered = publication.clone();
    reordered.bytes[first_record.clone()].copy_from_slice(&second);
    reordered.bytes[second_record.clone()].copy_from_slice(&first);
    assert_rebound_content_rejected(&profile, expected, reordered, "record reorder");
    let mut duplicated = publication.clone();
    duplicated.bytes[second_record].copy_from_slice(&first);
    assert_rebound_content_rejected(&profile, expected, duplicated, "record duplication");
    let mut wrong_entry_hash = expected;
    let mut substituted_hash = wrong_entry_hash.entry.payload_blake3();
    substituted_hash[0] ^= 1;
    wrong_entry_hash = replace_test_entry_layout(
        wrong_entry_hash,
        wrong_entry_hash.entry.ordinal(),
        wrong_entry_hash.entry.purpose(),
        wrong_entry_hash.entry.galois_exponent(),
        wrong_entry_hash.entry.payload_offset(),
        wrong_entry_hash.entry.payload_bytes(),
        substituted_hash,
    );
    let mut provider = publication;
    assert!(
        validate_seekable_evaluated_key(&profile, wrong_entry_hash, &mut provider).is_err(),
        "entry-hash substitution was accepted"
    );
}
#[test]
fn canonical_seekable_layout_rejects_truncation_extension_gap_overlap_and_alias() {
    let profile = hybrid_test_profile();
    let layout = seekable_evaluated_key_layout(&profile).unwrap();
    let mut publication = TestArtifact::new(&profile);
    let (generated, _) = published_test_key(&profile, &mut publication).unwrap();
    let pointer = publication.attach_pointer();
    let expected = expected_published_test_key(&profile, &generated, pointer);
    let mut truncated = publication.clone();
    truncated.bytes.pop();
    let mut truncated_expected = expected;
    truncated_expected.pointer = truncated.attach_pointer();
    assert!(validate_seekable_evaluated_key(&profile, truncated_expected, &mut truncated).is_err());
    let mut extended = publication.clone();
    extended.bytes.push(0);
    let mut extended_expected = expected;
    extended_expected.pointer = extended.attach_pointer();
    assert!(validate_seekable_evaluated_key(&profile, extended_expected, &mut extended).is_err());
    for (case, offset) in [
        ("gap", layout.payload_bytes + 1),
        ("overlap", layout.payload_bytes - 1),
        ("alias", 0),
    ] {
        let malformed = replace_test_entry_layout(
            expected,
            1,
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
            3,
            offset,
            layout.payload_bytes,
            expected.entry.payload_blake3(),
        );
        let mut provider = publication.clone();
        assert!(
            validate_seekable_evaluated_key(&profile, malformed, &mut provider).is_err(),
            "{case} layout accepted"
        );
    }
    for (case, payload_bytes) in [
        ("short entry", layout.payload_bytes - 1),
        ("long entry", layout.payload_bytes + 1),
    ] {
        let malformed = replace_test_entry_layout(
            expected,
            0,
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            0,
            0,
            payload_bytes,
            expected.entry.payload_blake3(),
        );
        let mut provider = publication.clone();
        assert!(
            validate_seekable_evaluated_key(&profile, malformed, &mut provider).is_err(),
            "{case} layout accepted"
        );
    }
    for (case, artifact_key_count) in [("total gap", 31), ("total extension", 33)] {
        let mut malformed = expected;
        malformed.artifact_key_count = artifact_key_count;
        let mut provider = publication.clone();
        assert!(
            validate_seekable_evaluated_key(&profile, malformed, &mut provider).is_err(),
            "{case} layout accepted"
        );
    }
}
#[test]
fn compact_relinearization_matches_direct_tiny_decryption_with_balanced_digits() {
    let profile = test_profile();
    profile.validate().unwrap();
    let secret = signed(&profile, &[-1, 0, 1, 1, 0, -1, 1, 0]);
    let constant = signed(&profile, &[4, -7, 9, 0, -3, 12, 1, -5]);
    let linear = signed(&profile, &[-2, 5, 0, 8, -11, 3, 7, 1]);
    let quadratic = signed(&profile, &[127, -129, 255, -257, 513, -769, 31, -63]);
    let secret_squared = secret.mul(&secret, &profile).unwrap();
    let digits = exact_compact_digits(&profile, &secret, &secret_squared);
    let (switched_constant, switched_linear) =
        apply_compact_switch(&profile, &constant, &linear, &quadratic, &digits).unwrap();
    let observed = switched_constant
        .add(&switched_linear.mul(&secret, &profile).unwrap(), &profile)
        .unwrap();
    let expected = constant
        .add(&linear.mul(&secret, &profile).unwrap(), &profile)
        .unwrap()
        .add(&quadratic.mul(&secret_squared, &profile).unwrap(), &profile)
        .unwrap();
    assert_eq!(observed, expected);
    assert_eq!(
        apply_compact_switch(
            &profile,
            &constant,
            &linear,
            &quadratic,
            &digits[..digits.len() - 1],
        ),
        Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
    );
}
#[test]
fn compact_galois_switch_matches_direct_decryption_for_every_tiny_automorphism() {
    let profile = test_profile();
    let secret = signed(&profile, &[-1, 1, 0, 1, -1, 0, 0, 1]);
    let constant = signed(&profile, &[11, -13, 17, 19, -23, 29, -31, 37]);
    let linear = signed(&profile, &[-41, 43, 47, -53, 59, 61, -67, 71]);
    let decrypted = constant
        .add(&linear.mul(&secret, &profile).unwrap(), &profile)
        .unwrap();
    for exponent in (1..(2 * profile.ring_degree)).step_by(2) {
        let transformed_secret = secret.automorphism(exponent, &profile).unwrap();
        let transformed_constant = constant.automorphism(exponent, &profile).unwrap();
        let transformed_linear = linear.automorphism(exponent, &profile).unwrap();
        let digits = exact_compact_digits(&profile, &secret, &transformed_secret);
        let (switched_constant, switched_linear) = apply_compact_switch(
            &profile,
            &transformed_constant,
            &RnsPolynomial::zero(&profile),
            &transformed_linear,
            &digits,
        )
        .unwrap();
        let observed = switched_constant
            .add(&switched_linear.mul(&secret, &profile).unwrap(), &profile)
            .unwrap();
        assert_eq!(
            observed,
            decrypted.automorphism(exponent, &profile).unwrap(),
            "odd exponent {exponent}"
        );
    }
    assert!(secret.automorphism(2, &profile).is_err());
}
#[test]
fn unordered_pair_aggregation_has_eight_diagonal_and_28_doubled_off_diagonal_terms() {
    let profile = test_profile();
    let secrets = [
        [-1, 0, 1, 0, 0, 0, 0, 0],
        [0, 1, 0, -1, 0, 0, 0, 0],
        [1, 0, 0, 0, 1, 0, 0, 0],
        [0, -1, 0, 0, 0, 1, 0, 0],
        [0, 0, 1, 0, 0, 0, -1, 0],
        [0, 0, 0, 1, 0, 0, 0, -1],
        [1, -1, 0, 0, 0, 0, 0, 0],
        [0, 0, 1, -1, 0, 0, 0, 0],
    ]
    .map(|values| signed(&profile, &values));
    let collective = secrets
        .iter()
        .try_fold(RnsPolynomial::zero(&profile), |sum, secret| {
            sum.add(secret, &profile)
        })
        .unwrap();
    let mut source_constant = RnsPolynomial::zero(&profile);
    let mut source_linear = RnsPolynomial::zero(&profile);
    let mut diagonal = 0;
    let mut off_diagonal = 0;
    for left in 0..secrets.len() {
        for right in left..secrets.len() {
            let pair = secrets[left].mul(&secrets[right], &profile).unwrap();
            add_weighted_pair_source(
                &profile,
                left == right,
                &mut source_constant,
                &mut source_linear,
                &pair,
                &pair,
            )
            .unwrap();
            if left == right {
                diagonal += 1;
            } else {
                off_diagonal += 1;
            }
        }
    }
    let collective_squared = collective.mul(&collective, &profile).unwrap();
    assert_eq!(diagonal, 8);
    assert_eq!(off_diagonal, 28);
    assert_eq!(source_constant, collective_squared);
    assert_eq!(source_linear, collective_squared);
}
fn test_evidence_digest(
    records: &[(u32, &[u8])],
    expected_records: u32,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut sink = NoopEvidenceSink;
    let mut hasher = EvidenceHasher::new(
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
        0,
        0,
        [0x55; 32],
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        &mut sink,
    )?;
    for (index, bytes) in records {
        hasher.test_record(*index, bytes)?;
    }
    hasher.finish(expected_records, &mut sink)
}
struct NoopEvidenceSink;
impl ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1 for NoopEvidenceSink {
    fn begin_evidence_set(
        &mut self,
        _header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
    fn begin_evidence_record(
        &mut self,
        _header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
    fn write_evidence_record_chunk(
        &mut self,
        _header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
        _chunk_index: u32,
        _bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
    fn finish_evidence_record(
        &mut self,
        _footer: ZkAmsMkheCollectiveEvidenceRecordFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
    fn finish_evidence_set(
        &mut self,
        _footer: ZkAmsMkheCollectiveEvidenceSetFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
}
#[derive(Default)]
struct RecordingEvidenceSink {
    current: Option<ZkAmsMkheCollectiveEvidenceRecordHeaderV1>,
    chunks: Vec<Vec<u8>>,
    footer: Option<ZkAmsMkheCollectiveEvidenceRecordFooterV1>,
    fail_chunk: Option<u32>,
}
impl ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1 for RecordingEvidenceSink {
    fn begin_evidence_set(
        &mut self,
        _header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
    fn begin_evidence_record(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.current.replace(header).is_some()
            || !self.chunks.is_empty()
            || self.footer.is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn write_evidence_record_chunk(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
        chunk_index: u32,
        bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.current != Some(header)
            || usize::try_from(chunk_index).ok() != Some(self.chunks.len())
            || bytes.is_empty()
            || bytes.len() > ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if self.fail_chunk == Some(chunk_index) {
            return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
        }
        self.chunks.push(bytes.to_vec());
        Ok(())
    }
    fn finish_evidence_record(
        &mut self,
        footer: ZkAmsMkheCollectiveEvidenceRecordFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.current != Some(footer.header())
            || usize::try_from(footer.chunk_count()).ok() != Some(self.chunks.len())
            || self.footer.replace(footer).is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn finish_evidence_set(
        &mut self,
        _footer: ZkAmsMkheCollectiveEvidenceSetFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        Ok(())
    }
}
fn canonical_test_body(label: u8) -> Vec<u8> {
    let payload_bytes = 2 * ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1 + 137;
    let canonical_bytes = 4 + 1 + 8 + payload_bytes + EVIDENCE_RECORD_DIGEST_BYTES_V1;
    let mut body = Vec::with_capacity(canonical_bytes - EVIDENCE_RECORD_DIGEST_BYTES_V1);
    body.extend_from_slice(b"ZTST");
    body.push(MKHE_VERSION_V1);
    body.extend_from_slice(&(canonical_bytes as u64).to_be_bytes());
    body.extend((0..payload_bytes).map(|index| label.wrapping_add(index as u8)));
    body
}
fn validate_canonical_test_record(bytes: &[u8]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if bytes.len() < 4 + 1 + 8 + EVIDENCE_RECORD_DIGEST_BYTES_V1
        || bytes[..4] != *b"ZTST"
        || bytes[4] != MKHE_VERSION_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let declared = usize::try_from(u64::from_be_bytes(
        bytes[5..13]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    ))
    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if declared != bytes.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let body_bytes = bytes
        .len()
        .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let digest = keccak256(&bytes[..body_bytes]);
    if bytes[body_bytes..] != digest {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(digest)
}
fn test_record_header(canonical_bytes: usize) -> ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
    ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
        set: ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
            kind: ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
            purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            ordinal: 0,
            galois_exponent: 0,
            collective_key_digest: [0x55; 32],
        },
        kind: ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne,
        record_index: 0,
        canonical_bytes: canonical_bytes as u64,
    }
}
#[test]
fn canonical_record_fanout_is_identical_bounded_and_rejects_every_transport_splice() {
    let body = canonical_test_body(0x21);
    let canonical_bytes = body.len() + EVIDENCE_RECORD_DIGEST_BYTES_V1;
    let header = test_record_header(canonical_bytes);
    let mut sink = RecordingEvidenceSink::default();
    let mut writer = CanonicalRecordFanout::new(&mut sink, header).unwrap();
    writer.write_body(&body).unwrap();
    let digest = writer.finish().unwrap();
    let encoded = sink.chunks.concat();
    assert_eq!(encoded.len(), canonical_bytes);
    assert_eq!(validate_canonical_test_record(&encoded).unwrap(), digest);
    let mut descriptor_set = evidence_set::EvidenceSetDigestV1::new(header.set).unwrap();
    descriptor_set
        .absorb_record(
            header.record_index,
            header.kind,
            header.canonical_bytes,
            digest,
        )
        .unwrap();
    assert_ne!(descriptor_set.finish(1).unwrap(), keccak256(&encoded));
    assert_eq!(sink.footer.unwrap().canonical_digest(), digest);
    assert!(
        sink.chunks
            .iter()
            .all(|chunk| chunk.len() <= ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1)
    );
    let mut truncated = encoded.clone();
    truncated.pop();
    assert!(validate_canonical_test_record(&truncated).is_err());
    let mut extended = encoded.clone();
    extended.push(0);
    assert!(validate_canonical_test_record(&extended).is_err());
    let mut wrong_length = encoded.clone();
    wrong_length[12] ^= 1;
    assert!(validate_canonical_test_record(&wrong_length).is_err());
    let mut wrong_digest = encoded.clone();
    *wrong_digest.last_mut().unwrap() ^= 1;
    assert!(validate_canonical_test_record(&wrong_digest).is_err());
    let omitted = sink
        .chunks
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .flat_map(|(_, chunk)| chunk.iter().copied())
        .collect::<Vec<_>>();
    assert!(validate_canonical_test_record(&omitted).is_err());
    let duplicated = sink
        .chunks
        .iter()
        .enumerate()
        .flat_map(|(index, chunk)| {
            if index == 1 {
                vec![chunk.as_slice(), chunk.as_slice()]
            } else {
                vec![chunk.as_slice()]
            }
        })
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    assert!(validate_canonical_test_record(&duplicated).is_err());
    let mut reordered_chunks = sink.chunks.clone();
    reordered_chunks.swap(1, 2);
    assert!(validate_canonical_test_record(&reordered_chunks.concat()).is_err());
    let other_body = canonical_test_body(0xa7);
    let other_header = test_record_header(other_body.len() + EVIDENCE_RECORD_DIGEST_BYTES_V1);
    let mut other_sink = RecordingEvidenceSink::default();
    let mut other_writer = CanonicalRecordFanout::new(&mut other_sink, other_header).unwrap();
    other_writer.write_body(&other_body).unwrap();
    other_writer.finish().unwrap();
    let mut spliced_chunks = sink.chunks.clone();
    spliced_chunks[1] = other_sink.chunks[1].clone();
    assert!(validate_canonical_test_record(&spliced_chunks.concat()).is_err());
}
#[test]
fn canonical_fanout_propagates_sink_error_before_record_commit() {
    let body = canonical_test_body(0x42);
    let header = test_record_header(body.len() + EVIDENCE_RECORD_DIGEST_BYTES_V1);
    let mut sink = RecordingEvidenceSink {
        fail_chunk: Some(1),
        ..RecordingEvidenceSink::default()
    };
    let mut writer = CanonicalRecordFanout::new(&mut sink, header).unwrap();
    assert_eq!(
        writer.write_body(&body),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );
    assert!(sink.footer.is_none());
}
#[test]
fn canonical_source_preflight_rejects_attacker_lengths_before_polynomial_allocation() {
    let oversized = u64::try_from(maximum_source_evidence_record_bytes().unwrap())
        .unwrap()
        .checked_add(1)
        .unwrap();
    assert!(source_stream::source_evidence_body_bytes_v1(oversized).is_err());
    let below_minimum =
        (SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1 + EVIDENCE_RECORD_DIGEST_BYTES_V1 - 1) as u64;
    assert!(source_stream::source_evidence_body_bytes_v1(below_minimum).is_err());
}
#[test]
fn canonical_evidence_stream_rejects_omission_reorder_duplicate_and_splice() {
    let baseline = [
        (0, b"statement-proof-0".as_slice()),
        (1, b"statement-proof-1"),
    ];
    let digest = test_evidence_digest(&baseline, 2).unwrap();
    assert_ne!(digest, [0; 32]);
    assert!(test_evidence_digest(&baseline[..1], 2).is_err());
    assert!(
        test_evidence_digest(&[(1, b"statement-proof-1"), (0, b"statement-proof-0")], 2,).is_err()
    );
    assert!(
        test_evidence_digest(&[(0, b"statement-proof-0"), (0, b"statement-proof-0")], 2,).is_err()
    );
    let mutated =
        test_evidence_digest(&[(0, b"statement-proof-X"), (1, b"statement-proof-1")], 2).unwrap();
    let spliced =
        test_evidence_digest(&[(0, b"statement-proof-0"), (1, b"other-roster-proof")], 2).unwrap();
    assert_ne!(mutated, digest);
    assert_ne!(spliced, digest);
    let mut sink = NoopEvidenceSink;
    let mut other_context = EvidenceHasher::new(
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
        0,
        0,
        [0x56; 32],
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        &mut sink,
    )
    .unwrap();
    for (index, bytes) in baseline {
        other_context.test_record(index, bytes).unwrap();
    }
    assert_ne!(other_context.finish(2, &mut sink).unwrap(), digest);
}
struct EvidenceTestRandom {
    seed: [u8; 32],
    counter: u64,
}
impl EvidenceTestRandom {
    fn new(label: &[u8]) -> Self {
        Self {
            seed: keccak256(label),
            counter: 0,
        }
    }
}
impl MaskedRelaxedRandomSourceV1 for EvidenceTestRandom {
    fn fill_bytes(
        &mut self,
        destination: &mut [u8],
    ) -> Result<(), super::super::MaskedRelaxedRandomErrorV1> {
        let mut written = 0;
        while written < destination.len() {
            let mut frame = [0_u8; 40];
            frame[..32].copy_from_slice(&self.seed);
            frame[32..].copy_from_slice(&self.counter.to_be_bytes());
            let block = keccak256(&frame);
            let take = (destination.len() - written).min(block.len());
            destination[written..written + take].copy_from_slice(&block[..take]);
            written += take;
            self.counter = self.counter.wrapping_add(1);
        }
        Ok(())
    }
}
struct EvidenceCapabilityFixture {
    roster: ZkAmsMkheGovernedActiveRosterV1,
    source_context: ZkAmsMkheTrustedSourceContextV1,
    cks_context: ZkAmsMkheTrustedCksContextV1,
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    counts: evidence_set::EvidenceRecordCountsV1,
}
include!("collective_eval_keys_evidence_tests.rs");
#[test]
fn release_schedule_and_online_work_are_exact_and_roster_independent() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    validate_zk_ams_t256_galois_key_schedule_v1(&schedule).unwrap();
    assert_eq!(schedule.entries.len(), 31);
    let exponents = schedule
        .entries
        .iter()
        .map(|entry| entry.exponent)
        .collect::<BTreeSet<_>>();
    assert_eq!(exponents.len(), 31);
    assert!(exponents.iter().all(|exponent| exponent % 2 == 1));
    assert_eq!(
        zk_ams_mkhe_compact_key_switch_ring_multiplications_v1().unwrap(),
        76
    );
    assert_eq!(test_profile().gadget_digits * 2, 16);
}
