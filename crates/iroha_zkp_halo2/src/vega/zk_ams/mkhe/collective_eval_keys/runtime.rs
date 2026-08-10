// Evaluated-key runtime implementation included at parent-module scope.
/// Reusable, non-secret runtime context for the exact 32-key evaluated-key set.
///
/// Construction validates the governed roster, all eight ordered proof-carrying
/// collective-public-key shares, and the complete manifest exactly once. It
/// retains only the small manifest table and verified aggregate CPK; individual
/// ~1.5 GiB evaluated-key payloads remain provider-streamed one at a time.
#[derive(Debug)]
pub(super) struct ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    profile: BgvProfile,
    wire_roster: ZkAmsMkheGovernedRosterWireV1,
    collective_key: ZkAmsMkheCollectivePublicKeyV1,
    transcript_digest: [u8; 32],
    manifest_digest: [u8; 32],
    entries: Vec<ZkAmsMkheCollectiveEvaluatedKeyEntryV1>,
    sorafs_pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    runtime_context_digest: [u8; 32],
}

/// One canonical seekable evaluated key validated for one reusable runtime.
///
/// This wrapper never owns a wire payload. It retains only the authenticated
/// header, provider/snapshot binding, and one fixed offset/digest record per
/// hybrid digit. It cannot be constructed without incrementally hashing and
/// parsing the complete canonical entry.
#[derive(PartialEq, Eq)]
pub(super) struct ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    runtime_context_digest: [u8; 32],
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    sorafs_pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    provider_binding_digest: [u8; 32],
    a_master_seed: [u8; 32],
    contribution_proof_digest: [u8; 32],
    digits: Vec<SeekableEvaluatedKeyDigitV1>,
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

impl ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    /// Validate the aggregate CPK and exact consensus-bound key-set manifest once.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
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
        let wire_roster = roster.to_wire_roster()?;
        let collective_key =
            aggregate_zk_ams_mkhe_collective_public_key_v1(roster, transcript_digest, shares)?;
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
        let mut runtime_frame = Vec::with_capacity(256);
        runtime_frame.extend_from_slice(EVALUATED_KEY_RUNTIME_DOMAIN_V1);
        runtime_frame.push(MKHE_VERSION_V1);
        runtime_frame.extend_from_slice(&wire_roster.profile_digest());
        runtime_frame.extend_from_slice(&wire_roster.roster_digest());
        runtime_frame.extend_from_slice(&wire_roster.epoch().to_be_bytes());
        runtime_frame.extend_from_slice(&transcript_digest);
        runtime_frame.extend_from_slice(&collective_key.digest());
        runtime_frame.extend_from_slice(&expected_manifest_digest);
        let runtime_context_digest = keccak256(&runtime_frame);
        if runtime_context_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            profile,
            wire_roster,
            collective_key,
            transcript_digest,
            manifest_digest: expected_manifest_digest,
            entries: manifest.entries().to_vec(),
            sorafs_pointer: manifest.sorafs(),
            runtime_context_digest,
        })
    }

    /// Verified aggregate collective-public-key digest.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key.digest()
    }

    /// Exact consensus-bound evaluated-key manifest digest.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }

    /// Incrementally authenticate and index one exact manifest entry.
    ///
    /// The full entry is read exactly once in bounded chunks. No complete wire,
    /// encoded copy, or decoded digit vector is ever allocated.
    pub fn validate_seekable_key_provider<P>(
        &self,
        ordinal: usize,
        provider: &mut P,
    ) -> Result<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
    {
        let entry = self.entry(ordinal)?;
        let contribution_proof_digest = evaluated_key_evidence_digest(
            entry.purpose(),
            entry.ordinal(),
            entry.galois_exponent(),
            self.collective_key.digest(),
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
                contribution_proof_digest,
            },
            provider,
        )?;
        let provider_binding_digest = seekable_provider_binding_digest(
            self.runtime_context_digest,
            entry,
            validation.state,
            validation.a_master_seed,
            validation.contribution_proof_digest,
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
            a_master_seed: validation.a_master_seed,
            contribution_proof_digest: validation.contribution_proof_digest,
            digits: validation.digits,
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
            || key.contribution_proof_digest
                != evaluated_key_evidence_digest(
                    key.entry.purpose(),
                    key.entry.ordinal(),
                    key.entry.galois_exponent(),
                    self.collective_key.digest(),
                    key.entry.source_proof_set_digest(),
                    key.entry.cks_proof_set_digest(),
                )?
            || key.digits.len() != self.profile.gadget_digits
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

    fn stored_b_digit<P>(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        provider: &mut P,
        digit_index: usize,
    ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
    {
        self.validate_provider_state(key, provider)?;
        let stored_b =
            read_seekable_evaluated_key_digit(&self.profile, key, provider, digit_index)?;
        self.validate_provider_state(key, provider)?;
        Ok(stored_b)
    }

    fn seeded_a_digit(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        digit_index: usize,
    ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
        let seeded_a = derive_target_a(
            &self.profile,
            &self.wire_roster,
            self.collective_key.transcript_digest(),
            self.collective_key.digest(),
            key.entry.purpose(),
            key.entry.ordinal(),
            key.entry.galois_exponent(),
            key.a_master_seed,
            digit_index,
        )?;
        if seeded_a.coefficients.capacity() != seeded_a.coefficients.len() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(seeded_a)
    }

    fn output_lineage(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        input_digest: [u8; 32],
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.validate_key_context(key)?;
        let mut frame = Vec::with_capacity(192);
        frame.extend_from_slice(EVALUATED_KEY_LINEAGE_DOMAIN_V1);
        frame.extend_from_slice(&[MKHE_VERSION_V1, key.entry.purpose() as u8]);
        frame.extend_from_slice(&key.entry.galois_exponent().to_be_bytes());
        frame.extend_from_slice(&self.collective_key.digest());
        frame.extend_from_slice(&self.manifest_digest);
        frame.extend_from_slice(&key.entry.payload_blake3());
        frame.extend_from_slice(&input_digest);
        Ok(keccak256(&frame))
    }
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

    fn digit(&self, digit_index: usize) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
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
        let signed_digit = self
            .signed_digits
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        rns_from_signed_exact(self.profile, signed_digit)
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

fn negacyclic_multiply_exact(
    left: &[u64],
    right: &[u64],
    modulus: u64,
    psi: u64,
) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
    if left.len() != right.len() || left.is_empty() || !left.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut left_twisted = Vec::new();
    let mut right_twisted = Vec::new();
    left_twisted
        .try_reserve_exact(left.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    right_twisted
        .try_reserve_exact(right.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if left_twisted.capacity() != left.len() || right_twisted.capacity() != right.len() {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let mut twist = 1_u64;
    for (&left, &right) in left.iter().zip(right) {
        left_twisted.push(super::mod_mul(left, twist, modulus));
        right_twisted.push(super::mod_mul(right, twist, modulus));
        twist = super::mod_mul(twist, psi, modulus);
    }
    let root = super::mod_mul(psi, psi, modulus);
    super::cyclic_ntt(&mut left_twisted, root, modulus);
    super::cyclic_ntt(&mut right_twisted, root, modulus);
    for (left, right) in left_twisted.iter_mut().zip(&right_twisted) {
        *left = super::mod_mul(*left, *right, modulus);
    }
    drop(right_twisted);
    super::inverse_cyclic_ntt(&mut left_twisted, root, modulus)?;
    let inverse_psi = super::mod_inverse(psi, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut untwist = 1_u64;
    for value in &mut left_twisted {
        *value = super::mod_mul(*value, untwist, modulus);
        untwist = super::mod_mul(untwist, inverse_psi, modulus);
    }
    Ok(left_twisted)
}

fn multiply_accumulate_in_place(
    profile: &BgvProfile,
    accumulator: &mut RnsPolynomial,
    plaintext_digit: &RnsPolynomial,
    evaluated_key_digit: &RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    accumulator.validate(profile)?;
    plaintext_digit.validate(profile)?;
    evaluated_key_digit.validate(profile)?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if accumulator.coefficients.capacity() != coefficient_count
        || plaintext_digit.coefficients.capacity() != coefficient_count
        || evaluated_key_digit.coefficients.capacity() != coefficient_count
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    for limb in 0..profile.moduli.len() {
        let product = negacyclic_multiply_exact(
            plaintext_digit.limb(profile, limb),
            evaluated_key_digit.limb(profile, limb),
            profile.moduli[limb],
            profile.negacyclic_roots[limb],
        )?;
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (output, contribution) in accumulator.coefficients[start..end].iter_mut().zip(product) {
            *output = super::mod_add(*output, contribution, profile.moduli[limb]);
        }
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
fn apply_compact_switch_streamed_core<StoredB, SeededA>(
    profile: &BgvProfile,
    constant: RnsPolynomial,
    linear: RnsPolynomial,
    switched: &RnsPolynomial,
    stored_b: StoredB,
    seeded_a: SeededA,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>
where
    StoredB: FnMut(usize) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
    SeededA: FnMut(usize) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
{
    apply_compact_switch_streamed_core_with_automorphism(
        profile, constant, linear, switched, None, stored_b, seeded_a,
    )
}

fn apply_compact_switch_streamed_core_with_automorphism<StoredB, SeededA>(
    profile: &BgvProfile,
    mut constant: RnsPolynomial,
    mut linear: RnsPolynomial,
    switched: &RnsPolynomial,
    switched_automorphism: Option<usize>,
    mut stored_b: StoredB,
    mut seeded_a: SeededA,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>
where
    StoredB: FnMut(usize) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
    SeededA: FnMut(usize) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
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
        for digit_index in first_digit..end_digit {
            let plaintext_digit = hoisted.digit(digit_index)?;
            observe_seekable_liveness(accounting.decomposition_phase_bytes);
            {
                observe_seekable_liveness(accounting.provider_read_phase_bytes);
                let stored_b = stored_b(digit_index)?;
                observe_seekable_liveness(accounting.multiplication_phase_bytes);
                multiply_accumulate_in_place(profile, &mut constant, &plaintext_digit, &stored_b)?;
            }
            {
                let seeded_a = seeded_a(digit_index)?;
                observe_seekable_liveness(accounting.multiplication_phase_bytes);
                multiply_accumulate_in_place(profile, &mut linear, &plaintext_digit, &seeded_a)?;
            }
        }
        first_digit = end_digit;
    }
    Ok((constant, linear))
}

fn apply_compact_switch_with_seekable_provider<P>(
    runtime: &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    provider: &mut P,
    base_constant: RnsPolynomial,
    base_linear: RnsPolynomial,
    switched: &RnsPolynomial,
    switched_automorphism: Option<usize>,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    let accounting = seekable_evaluated_key_accounting(&runtime.profile)?;
    if accounting.total_key_switch_work_units > runtime.profile.max_work_units
        || accounting.peak_managed_workspace_bytes
            > u64::try_from(runtime.profile.max_workspace_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    runtime.validate_provider_state(key, provider)?;
    let (constant, linear) = apply_compact_switch_streamed_core_with_automorphism(
        &runtime.profile,
        base_constant,
        base_linear,
        switched,
        switched_automorphism,
        |digit_index| runtime.stored_b_digit(key, provider, digit_index),
        |digit_index| runtime.seeded_a_digit(key, digit_index),
    )?;
    runtime.validate_provider_state(key, provider)?;
    Ok((constant, linear))
}

#[cfg(test)]
fn apply_compact_switch_with_provider(
    profile: &BgvProfile,
    base_constant: &RnsPolynomial,
    base_linear: &RnsPolynomial,
    switched: &RnsPolynomial,
    digit_count: usize,
    mut digit_provider: impl FnMut(usize) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1> {
    base_constant.validate(profile)?;
    base_linear.validate(profile)?;
    switched.validate(profile)?;
    if digit_count != profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    checked_ring_multiplication_work(
        profile,
        digit_count
            .checked_mul(2)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    let decomposition = gadget_decompose(profile, switched)?;
    let mut constant = base_constant.clone();
    let mut linear = base_linear.clone();
    for (digit_index, plaintext_digit) in decomposition.iter().enumerate() {
        let (stored_b, seeded_a) = digit_provider(digit_index)?;
        stored_b.validate(profile)?;
        seeded_a.validate(profile)?;
        constant = constant.add(&plaintext_digit.mul(&stored_b, profile)?, profile)?;
        linear = linear.add(&plaintext_digit.mul(&seeded_a, profile)?, profile)?;
    }
    Ok((constant, linear))
}

#[cfg(test)]
fn apply_compact_switch(
    profile: &BgvProfile,
    base_constant: &RnsPolynomial,
    base_linear: &RnsPolynomial,
    switched: &RnsPolynomial,
    digits: &[(RnsPolynomial, RnsPolynomial)],
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1> {
    apply_compact_switch_with_provider(
        profile,
        base_constant,
        base_linear,
        switched,
        digits.len(),
        |digit_index| {
            digits
                .get(digit_index)
                .cloned()
                .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)
        },
    )
}

/// Relinearize one exact collective level-one ciphertext with a compact key.
///
/// The reusable runtime has already validated the roster, CPK proofs, and
/// manifest. `key` owns only authenticated offsets and digests; `provider`
/// lends one exact stored-`b`/seeded-`a` digit pair at a time.
pub(super) fn relinearize_zk_ams_mkhe_collective_v1<P>(
    runtime: &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    provider: &mut P,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    // Reject the governed ceiling before cloning either output accumulator or
    // touching the external provider.
    seekable_evaluated_key_accounting(&runtime.profile)?;
    runtime.validate_key_context(key)?;
    if key.entry.purpose() != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization
        || key.entry.ordinal() != 0
    {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    ciphertext.validate_for_key(&runtime.collective_key, &runtime.profile)?;
    let (constant, linear) = apply_compact_switch_with_seekable_provider(
        runtime,
        key,
        provider,
        clone_rns_exact(&runtime.profile, ciphertext.constant())?,
        clone_rns_exact(&runtime.profile, ciphertext.linear())?,
        ciphertext.quadratic(),
        None,
    )?;
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        &runtime.profile,
        runtime.collective_key.parties(),
        ciphertext.epoch(),
        runtime.output_lineage(key, ciphertext.digest())?,
        ciphertext.sample_index(),
        1,
        constant,
        linear,
        Some(runtime.collective_key.digest()),
    )
}

/// Apply a frozen Galois automorphism and compactly switch back to `S`.
pub(super) fn automorphism_switch_zk_ams_mkhe_collective_v1<P>(
    runtime: &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    schedule_index: usize,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    provider: &mut P,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    // Reject the governed ceiling before allocating the transformed output
    // accumulator or touching the external provider.
    seekable_evaluated_key_accounting(&runtime.profile)?;
    let ordinal = schedule_index
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    runtime.validate_key_context(key)?;
    if key.entry.purpose() != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois
        || usize::from(key.entry.ordinal()) != ordinal
    {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    validate_compact_for_key(ciphertext, &runtime.collective_key, &runtime.profile)?;
    let exponent = usize::try_from(key.entry.galois_exponent())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let transformed_constant = ciphertext
        .constant()
        .automorphism(exponent, &runtime.profile)?;
    let (constant, linear) = apply_compact_switch_with_seekable_provider(
        runtime,
        key,
        provider,
        transformed_constant,
        RnsPolynomial::zero(&runtime.profile),
        ciphertext.linear(),
        Some(exponent),
    )?;
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        &runtime.profile,
        runtime.collective_key.parties(),
        ciphertext.epoch(),
        runtime.output_lineage(key, ciphertext.digest())?,
        ciphertext.sample_index(),
        ciphertext.level(),
        constant,
        linear,
        Some(runtime.collective_key.digest()),
    )
}
