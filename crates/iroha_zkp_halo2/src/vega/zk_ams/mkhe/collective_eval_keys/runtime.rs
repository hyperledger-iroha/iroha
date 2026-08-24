// Evaluated-key runtime implementation included at parent-module scope.
/// Reusable, non-secret runtime context for the exact 32-key evaluated-key set.
///
/// Construction validates the governed roster, compact sealed key binding, and
/// complete evaluated-key manifest exactly once. It retains only fixed key
/// identity digests plus the small manifest table; neither the native `2P`
/// collective key nor any ~1.5 GiB evaluated-key payload remains resident.
#[derive(Debug)]
pub struct ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    profile: BgvProfile,
    wire_roster: ZkAmsMkheGovernedRosterWireV1,
    eval_key_binding: super::collective::ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
    transcript_digest: [u8; 32],
    manifest_digest: [u8; 32],
    entries: Vec<ZkAmsMkheCollectiveEvaluatedKeyEntryV1>,
    sorafs_pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    runtime_context_digest: [u8; 32],
}
/// One canonical seekable evaluated key validated for one reusable runtime.
///
/// This wrapper never owns a wire payload. It retains only the authenticated header,
/// provider/snapshot binding, and fixed offset/digest records for every digit and residue limb. It
/// cannot be constructed without incrementally hashing and parsing the complete canonical entry and
/// proving that each ZARK digit matches the expected compact output committed by the consumed CKS
/// evidence receipts.
#[derive(PartialEq, Eq)]
pub struct ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    runtime_context_digest: [u8; 32],
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    sorafs_pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    provider_binding_digest: [u8; 32],
    limb_index_digest: [u8; 32],
    a_master_seed: [u8; 32],
    contribution_proof_digest: [u8; 32],
    evidence_set_capability_seal: [u8; 32],
    cks_compact_output_set_digest: [u8; 32],
    digits: Vec<SeekableEvaluatedKeyDigitV1>,
    limbs: Vec<SeekableEvaluatedKeyLimbV1>,
}
impl core::fmt::Debug for ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheValidatedCollectiveEvaluatedKeyV1")
            .field(
                "runtime_context_digest",
                &hex::encode(self.runtime_context_digest),
            )
            .field("entry", &self.entry)
            .field("provider_identity", &hex::encode(self.provider_identity))
            .field("snapshot_identity", &hex::encode(self.snapshot_identity))
            .field("stored_digits", &self.digits.len())
            .field("stored_limbs", &self.limbs.len())
            .finish()
    }
}
impl ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    /// Exact canonical manifest entry for this payload.
    #[must_use]
    pub const fn entry(&self) -> ZkAmsMkheCollectiveEvaluatedKeyEntryV1 {
        self.entry
    }
    /// Exact provider session bound during incremental validation.
    #[must_use]
    pub const fn provider_identity(&self) -> [u8; 32] {
        self.provider_identity
    }
    /// Exact immutable content revision bound during incremental validation.
    #[must_use]
    pub const fn snapshot_identity(&self) -> [u8; 32] {
        self.snapshot_identity
    }
}
fn consume_evidence_set_before_provider_v1(
    evidence_set: ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1,
    expected: evidence_set::EvidenceSetRuntimeBindingV1,
) -> Result<evidence_set::VerifiedEvidenceSetRuntimeAdmissionV1, ZkAmsMkheErrorV1> {
    evidence_set.consume_for_runtime_v1(expected)
}
impl ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    /// Construct the reusable runtime from the compact evaluated-key binding.
    /// The native `2P` key was already dropped during CPK finalization.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_from_compact_cpk_v1(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        eval_key_binding: super::collective::ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
        manifest: &ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
        expected_manifest_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_with_eval_key_binding_v1(
            roster,
            transcript_digest,
            eval_key_binding,
            manifest,
            expected_manifest_digest,
        )
    }
    fn new_with_eval_key_binding_v1(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        eval_key_binding: super::collective::ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
        manifest: &ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
        expected_manifest_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if expected_manifest_digest == [0; 32]
            || manifest.manifest_digest() != expected_manifest_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        profile.validate()?;
        eval_key_binding.validate_release_v1()?;
        if eval_key_binding.profile_digest() != roster.profile_digest()
            || eval_key_binding.roster_digest() != roster.roster_digest()
            || eval_key_binding.key_material_digest() != roster.key_material_digest()
            || eval_key_binding.epoch() != roster.epoch()
            || eval_key_binding.transcript_digest() != transcript_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let wire_roster = roster.to_wire_roster()?;
        let manifest_bytes = manifest.encode(&wire_roster)?;
        let decoded = ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(
            &manifest_bytes,
            &wire_roster,
            transcript_digest,
        )?;
        if decoded.manifest_digest() != expected_manifest_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if decoded != *manifest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut runtime_hash = Keccak256::new();
        runtime_hash.update(EVALUATED_KEY_RUNTIME_DOMAIN_V1);
        runtime_hash.update(&[MKHE_VERSION_V1]);
        runtime_hash.update(&wire_roster.profile_digest());
        runtime_hash.update(&wire_roster.roster_digest());
        runtime_hash.update(&wire_roster.epoch().to_be_bytes());
        runtime_hash.update(&transcript_digest);
        runtime_hash.update(&eval_key_binding.key_digest());
        runtime_hash.update(&eval_key_binding.binding_digest());
        runtime_hash.update(&expected_manifest_digest);
        let runtime_context_digest = runtime_hash.finalize();
        if runtime_context_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(manifest.entries().len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if entries.capacity() != manifest.entries().len() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        entries.extend_from_slice(manifest.entries());
        Ok(Self {
            profile,
            wire_roster,
            eval_key_binding,
            transcript_digest,
            manifest_digest: expected_manifest_digest,
            entries,
            sorafs_pointer: manifest.sorafs(),
            runtime_context_digest,
        })
    }
    /// Verified aggregate collective-public-key digest.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.eval_key_binding.key_digest()
    }
    /// Exact consensus-bound evaluated-key manifest digest.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
    /// Incrementally authenticate and index one exact manifest entry.
    ///
    /// The move-only evidence capability is consumed and fully checked before the provider is
    /// inspected. The scan then proves that every streamed ZARK digit equals the corresponding
    /// expected CKS compact output before this validated handle is returned.
    ///
    /// The full entry is read exactly once in bounded chunks. No complete wire,
    /// encoded copy, or decoded digit vector is ever allocated.
    pub fn validate_seekable_key_provider<P>(
        &self,
        ordinal: usize,
        evidence_set: ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1,
        provider: &mut P,
    ) -> Result<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
    {
        let entry = self.entry(ordinal)?;
        let evidence_admission = consume_evidence_set_before_provider_v1(
            evidence_set,
            evidence_set::EvidenceSetRuntimeBindingV1 {
                entry,
                profile_digest: self.wire_roster.profile_digest(),
                roster_digest: self.wire_roster.roster_digest(),
                key_material_digest: self.eval_key_binding.key_material_digest(),
                epoch: self.wire_roster.epoch(),
                transcript_digest: self.transcript_digest,
                collective_key_digest: self.eval_key_binding.key_digest(),
            },
        )?;
        let contribution_proof_digest = evaluated_key_evidence_digest(
            entry.purpose(),
            entry.ordinal(),
            entry.galois_exponent(),
            self.eval_key_binding.key_digest(),
            entry.source_proof_set_digest(),
            entry.cks_proof_set_digest(),
        )?;
        let validation = validate_seekable_evaluated_key(
            &self.profile,
            SeekableEvaluatedKeyExpectedV1 {
                entry,
                pointer: self.sorafs_pointer,
                artifact_key_count: self.entries.len(),
                profile_digest: self.wire_roster.profile_digest(),
                roster_digest: self.wire_roster.roster_digest(),
                epoch: self.wire_roster.epoch(),
                transcript_digest: self.transcript_digest,
                collective_key_digest: self.eval_key_binding.key_digest(),
                contribution_proof_digest,
                cks_compact_output_set_digest: evidence_admission.cks_compact_output_set_digest,
            },
            provider,
        )?;
        let provider_binding_digest = seekable_provider_binding_digest(
            self.runtime_context_digest,
            entry,
            validation.state,
            validation.a_master_seed,
            validation.contribution_proof_digest,
            evidence_admission.capability_seal,
            evidence_admission.cks_compact_output_set_digest,
            validation.limb_index_digest,
        );
        if provider_binding_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
            runtime_context_digest: self.runtime_context_digest,
            entry,
            sorafs_pointer: self.sorafs_pointer,
            provider_identity: validation.state.provider_identity,
            snapshot_identity: validation.state.snapshot_identity,
            provider_binding_digest,
            limb_index_digest: validation.limb_index_digest,
            a_master_seed: validation.a_master_seed,
            contribution_proof_digest: validation.contribution_proof_digest,
            evidence_set_capability_seal: evidence_admission.capability_seal,
            cks_compact_output_set_digest: evidence_admission.cks_compact_output_set_digest,
            digits: validation.digits,
            limbs: validation.limbs,
        })
    }
    fn entry(
        &self,
        ordinal: usize,
    ) -> Result<ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheErrorV1> {
        let entry = *self
            .entries
            .get(ordinal)
            .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
        if usize::from(entry.ordinal()) != ordinal {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        let expected_exponent = match entry.purpose() {
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => {
                if ordinal != 0 || entry.galois_exponent() != 0 {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                0
            }
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => {
                let schedule = zk_ams_t256_galois_key_schedule_v1()?;
                validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
                schedule
                    .entries
                    .get(
                        ordinal
                            .checked_sub(1)
                            .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?,
                    )
                    .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?
                    .exponent
            }
        };
        if entry.galois_exponent() != expected_exponent {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(entry)
    }
    fn validate_key_context(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if key.runtime_context_digest != self.runtime_context_digest
            || self.entry(usize::from(key.entry.ordinal()))? != key.entry
            || key.sorafs_pointer != self.sorafs_pointer
            || key.provider_identity == [0; 32]
            || key.snapshot_identity == [0; 32]
            || key.a_master_seed == [0; 32]
            || key.limb_index_digest == [0; 32]
            || key.evidence_set_capability_seal == [0; 32]
            || key.cks_compact_output_set_digest == [0; 32]
            || key.contribution_proof_digest
                != evaluated_key_evidence_digest(
                    key.entry.purpose(),
                    key.entry.ordinal(),
                    key.entry.galois_exponent(),
                    self.eval_key_binding.key_digest(),
                    key.entry.source_proof_set_digest(),
                    key.entry.cks_proof_set_digest(),
                )?
            || key.digits.len() != self.profile.gadget_digits
            || key.limbs.len()
                != self
                    .profile
                    .gadget_digits
                    .checked_mul(self.profile.moduli.len())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            || key.limb_index_digest
                != seekable_evaluated_key_limb_index_digest(key.entry, &key.limbs)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let state = SeekableProviderStateV1 {
            provider_identity: key.provider_identity,
            snapshot_identity: key.snapshot_identity,
            pointer: key.sorafs_pointer,
            payload_len: key.sorafs_pointer.payload_bytes(),
        };
        if seekable_provider_binding_digest(
            self.runtime_context_digest,
            key.entry,
            state,
            key.a_master_seed,
            key.contribution_proof_digest,
            key.evidence_set_capability_seal,
            key.cks_compact_output_set_digest,
            key.limb_index_digest,
        ) != key.provider_binding_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    fn validate_provider_state<P>(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        provider: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
    {
        self.validate_key_context(key)?;
        validate_bound_seekable_provider_state(key, provider)
    }
    fn stored_b_limb<P>(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        provider: &mut P,
        digit_index: usize,
        limb_index: usize,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
    {
        // The immutable key/context binding is checked once immediately
        // outside the switch core. Repeating it here would allocate its
        // evidence frame at the arithmetic peak for every limb.
        validate_bound_seekable_provider_state(key, provider)?;
        read_seekable_evaluated_key_limb(
            &self.profile,
            key,
            provider,
            digit_index,
            limb_index,
            output,
        )?;
        validate_bound_seekable_provider_state(key, provider)?;
        Ok(())
    }
    fn seeded_a_limb(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        digit_index: usize,
        limb_index: usize,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        derive_target_a_limb(
            &self.profile,
            &self.wire_roster,
            self.eval_key_binding.transcript_digest(),
            self.eval_key_binding.key_digest(),
            key.entry.purpose(),
            key.entry.ordinal(),
            key.entry.galois_exponent(),
            key.a_master_seed,
            digit_index,
            limb_index,
            output,
        )
    }
    fn output_lineage(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        input_digest: [u8; 32],
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.validate_key_context(key)?;
        let mut hash = Keccak256::new();
        hash.update(EVALUATED_KEY_LINEAGE_DOMAIN_V1);
        hash.update(&[MKHE_VERSION_V1, key.entry.purpose() as u8]);
        hash.update(&key.entry.galois_exponent().to_be_bytes());
        hash.update(&self.eval_key_binding.key_digest());
        hash.update(&self.manifest_digest);
        hash.update(&key.entry.payload_blake3());
        hash.update(&input_digest);
        let digest = hash.finalize();
        if digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(digest)
    }
}
fn read_seekable_evaluated_key_limb<P>(
    profile: &BgvProfile,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    provider: &mut P,
    digit_index: usize,
    limb_index: usize,
    output: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    let layout = seekable_evaluated_key_layout(profile)?;
    if digit_index >= profile.gadget_digits
        || limb_index >= profile.moduli.len()
        || output.len() != profile.ring_degree
        || key.limb_index_digest == [0; 32]
        || key.limb_index_digest != seekable_evaluated_key_limb_index_digest(key.entry, &key.limbs)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let limb_bytes = u64::try_from(
        profile
            .ring_degree
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let metadata_index = digit_index
        .checked_mul(profile.moduli.len())
        .and_then(|value| value.checked_add(limb_index))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let metadata = *key
        .limbs
        .get(metadata_index)
        .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
    let expected_offset = key
        .entry
        .payload_offset()
        .checked_add(
            u64::try_from(SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| {
            layout
                .digit_record_bytes
                .checked_mul(u64::try_from(digit_index).ok()?)
                .and_then(|digit| value.checked_add(digit))
        })
        .and_then(|value| {
            value.checked_add(u64::try_from(SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1).ok()?)
        })
        .and_then(|value| {
            limb_bytes
                .checked_mul(u64::try_from(limb_index).ok()?)
                .and_then(|limb| value.checked_add(limb))
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if metadata.absolute_offset != expected_offset || metadata.canonical_bytes != limb_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let expected_state = validated_key_provider_state(key);
    seekable_provider_seek_exact(provider, expected_state, metadata.absolute_offset)?;
    let mut hasher = seekable_evaluated_key_limb_hasher(
        digit_index,
        limb_index,
        profile.moduli[limb_index],
        metadata.absolute_offset,
        metadata.canonical_bytes,
    )?;
    let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
    let mut coefficient = 0_usize;
    let mut remaining = usize::try_from(metadata.canonical_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    while remaining != 0 {
        let take = remaining.min(buffer.len());
        seekable_provider_read_exact(provider, expected_state, &mut buffer[..take])?;
        hasher.update(&buffer[..take]);
        for encoded in buffer[..take].chunks_exact(core::mem::size_of::<u64>()) {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            if residue >= profile.moduli[limb_index] {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            *output
                .get_mut(coefficient)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)? = residue;
            coefficient = coefficient
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        remaining -= take;
    }
    if coefficient != profile.ring_degree || hasher.finalize() != metadata.blake3 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
/// One coefficient-major CRT/radix pass hoisted across a bounded digit batch.
///
/// Only compact signed digits are retained. Evaluated-key records and expanded
/// RNS polynomials remain streamed one digit at a time.
struct HoistedHybridDigitBatchV1<'a> {
    profile: &'a BgvProfile,
    first_digit: usize,
    digit_count: usize,
    /// Digit-major coefficients, exactly `digit_count * ring_degree` values.
    signed_digits: Vec<i64>,
}
#[cfg(test)]
std::thread_local! {
    static HOISTED_RESIDUE_READS_V1: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn reset_hoisted_residue_reads_v1() {
    HOISTED_RESIDUE_READS_V1.with(|count| count.set(0));
}
#[cfg(test)]
fn hoisted_residue_reads_v1() -> usize {
    HOISTED_RESIDUE_READS_V1.with(std::cell::Cell::get)
}
#[cfg(test)]
fn observe_hoisted_residue_read_v1() {
    HOISTED_RESIDUE_READS_V1.with(|count| count.set(count.get().saturating_add(1)));
}
#[cfg(not(test))]
fn observe_hoisted_residue_read_v1() {}
impl<'a> HoistedHybridDigitBatchV1<'a> {
    #[cfg(test)]
    fn new(
        profile: &'a BgvProfile,
        polynomial: &'a RnsPolynomial,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_batch_with_automorphism(profile, polynomial, None, 0, profile.gadget_digits)
    }
    #[cfg(test)]
    fn new_automorphed(
        profile: &'a BgvProfile,
        polynomial: &'a RnsPolynomial,
        exponent: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_batch_with_automorphism(
            profile,
            polynomial,
            Some(exponent),
            0,
            profile.gadget_digits,
        )
    }
    fn new_batch(
        profile: &'a BgvProfile,
        polynomial: &'a RnsPolynomial,
        first_digit: usize,
        digit_count: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_batch_with_automorphism(profile, polynomial, None, first_digit, digit_count)
    }
    fn new_automorphed_batch(
        profile: &'a BgvProfile,
        polynomial: &'a RnsPolynomial,
        exponent: usize,
        first_digit: usize,
        digit_count: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_batch_with_automorphism(
            profile,
            polynomial,
            Some(exponent),
            first_digit,
            digit_count,
        )
    }
    fn new_batch_with_automorphism(
        profile: &'a BgvProfile,
        polynomial: &'a RnsPolynomial,
        exponent: Option<usize>,
        first_digit: usize,
        digit_count: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        polynomial.validate(profile)?;
        if !profile.hybrid_rns_decomposition
            || profile.gadget_base_log != 60
            || profile.gadget_digits != profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        let end_digit = first_digit
            .checked_add(digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if digit_count == 0
            || digit_count > HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1
            || end_digit > profile.gadget_digits
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        let accounting = seekable_evaluated_key_accounting(profile)?;
        checked_coefficient_work(profile, hoisted_hybrid_decomposition_passes(profile)?)?;
        checked_ring_multiplication_work(
            profile,
            profile
                .gadget_digits
                .checked_mul(2)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        if accounting.total_key_switch_work_units > profile.max_work_units {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let ciphertext_modulus = modulus_product(profile.moduli)?;
        let half_modulus = ciphertext_modulus.shr_one();
        let base = 1_u64
            .checked_shl(u32::from(profile.gadget_base_log))
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        let automorphism_inverse = match exponent {
            None => 0,
            Some(exponent) => {
                let twice_degree = profile
                    .ring_degree
                    .checked_mul(2)
                    .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
                if exponent == 0 || exponent >= twice_degree || exponent.is_multiple_of(2) {
                    return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
                }
                inverse_odd_mod_power_of_two(exponent, twice_degree)?
            }
        };
        let signed_count = profile
            .ring_degree
            .checked_mul(digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut signed_digits = Vec::new();
        signed_digits
            .try_reserve_exact(signed_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if signed_digits.capacity() != signed_count {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        signed_digits.resize(signed_count, 0_i64);
        let mut residues = Vec::new();
        residues
            .try_reserve_exact(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if residues.capacity() != profile.moduli.len() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        residues.resize(profile.moduli.len(), 0_u64);
        for coefficient in 0..profile.ring_degree {
            for (limb, residue) in residues.iter_mut().enumerate() {
                *residue = coefficient_residue_with_automorphism_v1(
                    profile,
                    polynomial,
                    automorphism_inverse,
                    limb,
                    coefficient,
                )?;
            }
            let canonical = WideUint::crt(&residues, profile.moduli)?;
            let (negative, magnitude) = if canonical > half_modulus {
                (
                    true,
                    ciphertext_modulus
                        .checked_sub(canonical)
                        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?,
                )
            } else {
                (false, canonical)
            };
            let mut carry = 0_u64;
            for digit in 0..profile.gadget_digits {
                let chunk = magnitude.bits_at(
                    digit
                        .checked_mul(usize::from(profile.gadget_base_log))
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                    usize::from(profile.gadget_base_log),
                )?;
                let with_carry = chunk
                    .checked_add(carry)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
                let (balanced, next_carry) = if with_carry >= base / 2 {
                    (
                        i64::try_from(with_carry).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                            - i64::try_from(base).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                        1,
                    )
                } else {
                    (
                        i64::try_from(with_carry).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                        0,
                    )
                };
                if (first_digit..end_digit).contains(&digit) {
                    let local_digit = digit - first_digit;
                    let offset = local_digit
                        .checked_mul(profile.ring_degree)
                        .and_then(|base| base.checked_add(coefficient))
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                    signed_digits[offset] = if negative { -balanced } else { balanced };
                }
                carry = next_carry;
            }
            if carry != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidProfile);
            }
        }
        Ok(Self {
            profile,
            first_digit,
            digit_count,
            signed_digits,
        })
    }
    fn signed_digit(&self, digit_index: usize) -> Result<&[i64], ZkAmsMkheErrorV1> {
        let end_digit = self
            .first_digit
            .checked_add(self.digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if !(self.first_digit..end_digit).contains(&digit_index) {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        let start = (digit_index - self.first_digit)
            .checked_mul(self.profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(self.profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.signed_digits
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)
    }
    fn fill_digit_limb(
        &self,
        digit_index: usize,
        limb_index: usize,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if limb_index >= self.profile.moduli.len() || output.len() != self.profile.ring_degree {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let modulus = self.profile.moduli[limb_index];
        for (output, value) in output.iter_mut().zip(self.signed_digit(digit_index)?) {
            *output = signed_mod(*value, modulus);
        }
        Ok(())
    }
    #[cfg(test)]
    fn digit(&self, digit_index: usize) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
        rns_from_signed_exact(self.profile, self.signed_digit(digit_index)?)
    }
}
fn coefficient_residue_with_automorphism_v1(
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
    automorphism_inverse: usize,
    limb: usize,
    coefficient: usize,
) -> Result<u64, ZkAmsMkheErrorV1> {
    observe_hoisted_residue_read_v1();
    if automorphism_inverse == 0 {
        return Ok(polynomial.limb(profile, limb)[coefficient]);
    }
    let twice_degree = profile
        .ring_degree
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let coefficient =
        u64::try_from(coefficient).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let inverse = u64::try_from(automorphism_inverse)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let twice_degree =
        u64::try_from(twice_degree).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mapped = coefficient
        .checked_mul(inverse)
        .map(|mapped| mapped % twice_degree)
        .and_then(|mapped| usize::try_from(mapped).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let (source, negate) = if mapped >= profile.ring_degree {
        (mapped - profile.ring_degree, true)
    } else {
        (mapped, false)
    };
    let value = polynomial.limb(profile, limb)[source];
    Ok(if negate && value != 0 {
        profile.moduli[limb] - value
    } else {
        value
    })
}
fn inverse_odd_mod_power_of_two(value: usize, modulus: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    if value == 0 || value.is_multiple_of(2) || modulus < 2 || !modulus.is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let value = u64::try_from(value).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let modulus = u64::try_from(modulus).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut inverse = 1_u64;
    for _ in 0..6 {
        inverse = inverse.wrapping_mul(2_u64.wrapping_sub(value.wrapping_mul(inverse)));
    }
    inverse &= modulus - 1;
    if value.wrapping_mul(inverse) & (modulus - 1) != 1 {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    usize::try_from(inverse).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)
}
#[cfg(test)]
fn rns_from_signed_exact(
    profile: &BgvProfile,
    values: &[i64],
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    if values.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if coefficients.capacity() != coefficient_count {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    for &modulus in profile.moduli {
        coefficients.extend(
            values
                .iter()
                .copied()
                .map(|value| super::signed_mod(value, modulus)),
        );
    }
    RnsPolynomial::from_flat(profile, coefficients)
}
fn clone_rns_exact(
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(polynomial.coefficients.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if coefficients.capacity() != polynomial.coefficients.len() {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    coefficients.extend_from_slice(&polynomial.coefficients);
    RnsPolynomial::from_flat(profile, coefficients)
}
struct KeySwitchLimbWorkspaceV1 {
    evaluated_key: Vec<u64>,
    signed_digit: Vec<u64>,
}
#[cfg(test)]
std::thread_local! {
    static KEY_SWITCH_LIMB_WORKSPACE_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}
#[cfg(test)]
fn reset_key_switch_limb_workspace_zeroized_drops_v1() {
    KEY_SWITCH_LIMB_WORKSPACE_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
}
#[cfg(test)]
fn key_switch_limb_workspace_zeroized_drops_v1() -> usize {
    KEY_SWITCH_LIMB_WORKSPACE_ZEROIZED_DROPS_V1.with(std::cell::Cell::get)
}
impl KeySwitchLimbWorkspaceV1 {
    fn new(profile: &BgvProfile) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut evaluated_key = Vec::new();
        let mut signed_digit = Vec::new();
        evaluated_key
            .try_reserve_exact(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        signed_digit
            .try_reserve_exact(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if evaluated_key.capacity() != profile.ring_degree
            || signed_digit.capacity() != profile.ring_degree
        {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        evaluated_key.resize(profile.ring_degree, 0);
        signed_digit.resize(profile.ring_degree, 0);
        Ok(Self {
            evaluated_key,
            signed_digit,
        })
    }
}
impl Drop for KeySwitchLimbWorkspaceV1 {
    fn drop(&mut self) {
        let evaluated_key = core::hint::black_box(&mut self.evaluated_key);
        evaluated_key.fill(0);
        let signed_digit = core::hint::black_box(&mut self.signed_digit);
        signed_digit.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        if evaluated_key.iter().all(|value| *value == 0)
            && signed_digit.iter().all(|value| *value == 0)
        {
            let _ = KEY_SWITCH_LIMB_WORKSPACE_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
        let _ = core::hint::black_box(&mut *evaluated_key);
        let _ = core::hint::black_box(&mut *signed_digit);
    }
}
fn multiply_accumulate_limb_in_place(
    profile: &BgvProfile,
    accumulator: &mut RnsPolynomial,
    limb_index: usize,
    evaluated_key: &mut [u64],
    signed_digit: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    if limb_index >= profile.moduli.len()
        || evaluated_key.len() != profile.ring_degree
        || signed_digit.len() != profile.ring_degree
        || evaluated_key.is_empty()
        || !evaluated_key.len().is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let modulus = profile.moduli[limb_index];
    if evaluated_key.iter().any(|value| *value >= modulus)
        || signed_digit.iter().any(|value| *value >= modulus)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let psi = profile.negacyclic_roots[limb_index];
    let mut twist = 1_u64;
    for (evaluated_key, signed_digit) in evaluated_key.iter_mut().zip(signed_digit.iter_mut()) {
        *evaluated_key = mod_mul(*evaluated_key, twist, modulus);
        *signed_digit = mod_mul(*signed_digit, twist, modulus);
        twist = mod_mul(twist, psi, modulus);
    }
    let root = mod_mul(psi, psi, modulus);
    super::cyclic_ntt(evaluated_key, root, modulus);
    super::cyclic_ntt(signed_digit, root, modulus);
    for (evaluated_key, signed_digit) in evaluated_key.iter_mut().zip(signed_digit.iter()) {
        *evaluated_key = mod_mul(*evaluated_key, *signed_digit, modulus);
    }
    super::inverse_cyclic_ntt(evaluated_key, root, modulus)?;
    let inverse_psi = super::mod_inverse(psi, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let start = limb_index
        .checked_mul(profile.ring_degree)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(profile.ring_degree)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let accumulator = accumulator
        .coefficients
        .get_mut(start..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
    let mut untwist = 1_u64;
    for (output, contribution) in accumulator.iter_mut().zip(evaluated_key.iter_mut()) {
        *contribution = mod_mul(*contribution, untwist, modulus);
        *output = mod_add(*output, *contribution, modulus);
        untwist = mod_mul(untwist, inverse_psi, modulus);
    }
    Ok(())
}
#[cfg(test)]
std::thread_local! {
    static TRACK_SEEKABLE_LIVENESS_V1: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SEEKABLE_LIVENESS_HIGH_WATER_V1: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn observe_seekable_liveness(bytes: u64) {
    TRACK_SEEKABLE_LIVENESS_V1.with(|enabled| {
        if enabled.get() {
            SEEKABLE_LIVENESS_HIGH_WATER_V1.with(|peak| peak.set(peak.get().max(bytes)));
        }
    });
}
#[cfg(not(test))]
fn observe_seekable_liveness(_bytes: u64) {}
#[cfg(test)]
fn apply_compact_switch_streamed_core<StoredBLimb, SeededALimb>(
    profile: &BgvProfile,
    constant: RnsPolynomial,
    linear: RnsPolynomial,
    switched: &RnsPolynomial,
    stored_b_limb: StoredBLimb,
    seeded_a_limb: SeededALimb,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>
where
    StoredBLimb: FnMut(usize, usize, &mut [u64]) -> Result<(), ZkAmsMkheErrorV1>,
    SeededALimb: FnMut(usize, usize, &mut [u64]) -> Result<(), ZkAmsMkheErrorV1>,
{
    apply_compact_switch_streamed_core_with_automorphism(
        profile,
        constant,
        linear,
        switched,
        None,
        stored_b_limb,
        seeded_a_limb,
    )
}
fn apply_compact_switch_streamed_core_with_automorphism<StoredBLimb, SeededALimb>(
    profile: &BgvProfile,
    mut constant: RnsPolynomial,
    mut linear: RnsPolynomial,
    switched: &RnsPolynomial,
    switched_automorphism: Option<usize>,
    mut stored_b_limb: StoredBLimb,
    mut seeded_a_limb: SeededALimb,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>
where
    StoredBLimb: FnMut(usize, usize, &mut [u64]) -> Result<(), ZkAmsMkheErrorV1>,
    SeededALimb: FnMut(usize, usize, &mut [u64]) -> Result<(), ZkAmsMkheErrorV1>,
{
    let accounting = seekable_evaluated_key_accounting(profile)?;
    if accounting.peak_managed_workspace_bytes
        > u64::try_from(profile.max_workspace_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    constant.validate(profile)?;
    linear.validate(profile)?;
    switched.validate(profile)?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if constant.coefficients.capacity() != coefficient_count
        || linear.coefficients.capacity() != coefficient_count
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    observe_seekable_liveness(accounting.output_accumulator_bytes);
    let mut workspace = KeySwitchLimbWorkspaceV1::new(profile)?;
    let mut first_digit = 0_usize;
    while first_digit < profile.gadget_digits {
        let digit_count =
            (profile.gadget_digits - first_digit).min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1);
        let hoisted = match switched_automorphism {
            Some(exponent) => HoistedHybridDigitBatchV1::new_automorphed_batch(
                profile,
                switched,
                exponent,
                first_digit,
                digit_count,
            )?,
            None => {
                HoistedHybridDigitBatchV1::new_batch(profile, switched, first_digit, digit_count)?
            }
        };
        let end_digit = first_digit
            .checked_add(digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        observe_seekable_liveness(accounting.decomposition_phase_bytes);
        for digit_index in first_digit..end_digit {
            for limb_index in 0..profile.moduli.len() {
                observe_seekable_liveness(accounting.provider_read_phase_bytes);
                stored_b_limb(digit_index, limb_index, &mut workspace.evaluated_key)?;
                hoisted.fill_digit_limb(digit_index, limb_index, &mut workspace.signed_digit)?;
                observe_seekable_liveness(accounting.multiplication_phase_bytes);
                multiply_accumulate_limb_in_place(
                    profile,
                    &mut constant,
                    limb_index,
                    &mut workspace.evaluated_key,
                    &mut workspace.signed_digit,
                )?;
                seeded_a_limb(digit_index, limb_index, &mut workspace.evaluated_key)?;
                hoisted.fill_digit_limb(digit_index, limb_index, &mut workspace.signed_digit)?;
                observe_seekable_liveness(accounting.multiplication_phase_bytes);
                multiply_accumulate_limb_in_place(
                    profile,
                    &mut linear,
                    limb_index,
                    &mut workspace.evaluated_key,
                    &mut workspace.signed_digit,
                )?;
            }
        }
        first_digit = end_digit;
    }
    Ok((constant, linear))
}
