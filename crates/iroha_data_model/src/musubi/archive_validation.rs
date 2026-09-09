//! Archive registration, storage-location, and provider-attestation validation.

use super::*;

impl MusubiArchiveCommitmentV1 {
    /// Validate first-release archive bounds and non-inert commitments.
    ///
    /// # Errors
    ///
    /// Returns an error if an archive size or count is outside its V1 bound, the chunker handle
    /// is overlong, or a required commitment digest is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.content_length == 0 || self.content_length > MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi archive bundle payload length is out of bounds",
            ));
        }
        if self.car_size == 0 || self.car_size > MUSUBI_MAX_CAR_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi archive CAR length is out of bounds",
            ));
        }
        if self.file_count == 0 || self.file_count > MUSUBI_MAX_FILES_V1 {
            return Err(ParseError::new(
                "Musubi archive file count is out of bounds",
            ));
        }
        if self.chunk_count == 0 || self.chunk_count > MUSUBI_MAX_CHUNKS_V1 {
            return Err(ParseError::new(
                "Musubi archive chunk count is out of bounds",
            ));
        }
        if self.chunker.to_handle().len() > 128
            || [
                self.chunk_plan_digest,
                self.por_root,
                self.car_digest,
                self.bundle_digest,
                self.source_tree_digest,
                self.descriptor_digest,
            ]
            .iter()
            .any(MusubiContentDigestV1::is_zero)
        {
            return Err(ParseError::new(
                "Musubi archive contains an invalid or inert commitment",
            ));
        }
        Ok(())
    }
    /// Compute the domain-separated `ArchiveId` from canonical Norito bytes.
    #[must_use]
    pub fn archive_id(&self) -> ArchiveId {
        ArchiveId(domain_hash_value(MUSUBI_ARCHIVE_ID_DOMAIN_V1, self))
    }
}

impl MusubiArtifactDescriptorV1 {
    /// Decode one exact canonical artifact-descriptor bundle file under the shared V1 limits.
    ///
    /// # Errors
    ///
    /// Returns one stable payload-free error when the file is empty, oversized, malformed,
    /// trailing, noncanonical, or fails descriptor validation.
    pub fn decode_canonical_bundle_file(bytes: &[u8]) -> Result<Self, ParseError> {
        decode_canonical_bundle_file_v1(
            bytes,
            MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1,
            MUSUBI_ARTIFACT_DESCRIPTOR_DECODE_LIMITS_V1,
            Self::validate,
            "Musubi artifact descriptor bundle file is invalid or out of bounds",
        )
    }
    /// Construct and validate a first-release artifact descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error if a required digest is zero or the selected source size or file count is
    /// outside its V1 bound.
    pub fn new(
        semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
        source_tree_digest: MusubiContentDigestV1,
        verification_lock_digest: MusubiVerificationLockDigestV1,
        source_bytes: u64,
        source_file_count: u32,
    ) -> Result<Self, ParseError> {
        let descriptor = Self {
            version: MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1,
            semantic_release_manifest_digest,
            source_tree_digest,
            verification_lock_digest,
            source_bytes,
            source_file_count,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }
    /// Validate descriptor version, digest bindings, and first-release source bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if the descriptor version is unsupported, a required digest is zero, or
    /// the selected source size or file count is outside its V1 bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.version != MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1
            || self.semantic_release_manifest_digest.is_zero()
            || self.source_tree_digest.is_zero()
            || self.verification_lock_digest.is_zero()
            || self.source_bytes == 0
            || self.source_bytes > MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1
            || self.source_file_count == 0
            || self.source_file_count > MUSUBI_MAX_FILES_V1
        {
            return Err(ParseError::new(
                "Musubi artifact descriptor is invalid or out of bounds",
            ));
        }
        Ok(())
    }
}

impl MusubiArchiveRegistrationProjectionV1 {
    /// Validate the immutable archive identity and its exact ingress binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the commitment, receipt, or registrant is invalid, or if the archive
    /// identity, receipt fields, and nonzero registration height do not agree.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_archive_registration_fields(
            self.archive_id,
            &self.commitment,
            &self.staging_receipt,
            &self.registered_by,
            self.registered_at_height,
        )
    }
}

impl MusubiArchiveRecordV1 {
    /// Return the immutable registration fields reproducible by every later archive read.
    #[must_use]
    pub fn registration_projection(&self) -> MusubiArchiveRegistrationProjectionV1 {
        MusubiArchiveRegistrationProjectionV1 {
            archive_id: self.archive_id,
            commitment: self.commitment.clone(),
            staging_receipt: self.staging_receipt.clone(),
            registered_by: self.registered_by.clone(),
            registered_at_height: self.registered_at_height,
        }
    }
    /// Validate the commitment and its derived identity.
    ///
    /// # Errors
    ///
    /// Returns an error if immutable registration fields are inconsistent, the location revision
    /// is zero, or location identifiers are oversized, zero, unsorted, or duplicated.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_archive_registration_fields(
            self.archive_id,
            &self.commitment,
            &self.staging_receipt,
            &self.registered_by,
            self.registered_at_height,
        )?;
        if self.location_revision == 0
            || self.location_ids.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || self
                .location_ids
                .iter()
                .any(MusubiArchiveLocationIdV1::is_zero)
            || self.location_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi archive record identity, staging receipt, or revision is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiArchiveLocationKeyV1 {
    /// Construct the canonical ordered location key.
    #[must_use]
    pub const fn new(archive_id: ArchiveId, location_id: MusubiArchiveLocationIdV1) -> Self {
        Self {
            archive_id,
            location_id,
        }
    }
}

impl MusubiPinLocationReferenceV1 {
    /// Validate non-inert pin and location identities.
    ///
    /// # Errors
    ///
    /// Returns an error if the pin-manifest digest, archive identity, or location identity is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if digest_is_zero(self.pin_manifest.as_bytes())
            || self.location.archive_id.is_zero()
            || self.location.location_id.is_zero()
        {
            return Err(ParseError::new(
                "Musubi pin-to-location reverse reference is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiProviderLocationKeyV1 {
    /// Construct an exact provider/location reverse-index key.
    #[must_use]
    pub const fn new(provider_id: ProviderId, location: MusubiArchiveLocationKeyV1) -> Self {
        Self {
            provider_id,
            location,
        }
    }
    /// Return inclusive ordered bounds covering only one provider's location references.
    #[must_use]
    pub fn provider_range(provider_id: ProviderId) -> std::ops::RangeInclusive<Self> {
        let location = |fill| {
            MusubiArchiveLocationKeyV1::new(
                ArchiveId::new([fill; 32]),
                MusubiArchiveLocationIdV1::new([fill; 32]),
            )
        };
        Self::new(provider_id, location(0))..=Self::new(provider_id, location(u8::MAX))
    }
    /// Validate non-inert provider and location identities.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider, archive, or location identity is the all-zero sentinel.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.provider_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.location.archive_id.is_zero()
            || self.location.location_id.is_zero()
        {
            return Err(ParseError::new(
                "Musubi provider-to-location reverse key is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiArchiveLocationV1 {
    /// Return the canonical ordered storage key.
    #[must_use]
    pub const fn key(&self) -> MusubiArchiveLocationKeyV1 {
        MusubiArchiveLocationKeyV1::new(self.archive_id, self.location_id)
    }
    /// Validate provider, renewal, and revision bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if an identity or commitment is zero, providers are empty, oversized,
    /// unsorted, or duplicated, renewal does not precede expiry, or a revision height is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.location_id.is_zero()
            || self.archive_id.is_zero()
            || digest_is_zero(self.pin_manifest.as_bytes())
            || self.providers.is_empty()
            || self.providers.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
            || self.provider_attestation_set_digest.is_zero()
            || self.renew_after_epoch >= self.expires_at_epoch
            || self.finalized_height == 0
            || self.revision == 0
        {
            return Err(ParseError::new("Musubi archive location is invalid"));
        }
        if self.providers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ParseError::new(
                "Musubi archive location providers must be sorted and distinct",
            ));
        }
        Ok(())
    }
}

impl MusubiArchiveAvailabilityV1 {
    /// Validate aggregate consistency and first-release bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive or finalized anchor is inert, replica counts exceed their
    /// V1 capacity, or the availability class does not agree with those counts.
    pub fn validate(&self) -> Result<(), ParseError> {
        let healthy_capacity = usize::from(self.active_locations)
            .checked_mul(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
            .expect("bounded Musubi location capacity cannot overflow usize");
        if self.archive_id.is_zero()
            || usize::from(self.active_locations) > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || usize::from(self.healthy_replicas) > healthy_capacity
            || self.finalized_height == 0
            || self.index_revision == 0
            || digest_is_zero(&self.finalized_block_hash)
        {
            return Err(ParseError::new(
                "Musubi archive availability record is invalid",
            ));
        }
        let expected = if self.healthy_replicas >= MUSUBI_MIN_HEALTHY_REPLICAS_V1 {
            MusubiStorageAvailabilityV1::Selectable
        } else if self.active_locations > 0 && self.healthy_replicas > 0 {
            MusubiStorageAvailabilityV1::BelowQuorum
        } else {
            MusubiStorageAvailabilityV1::Unavailable
        };
        if self.availability != expected {
            return Err(ParseError::new(
                "Musubi archive availability classification is inconsistent with its counts",
            ));
        }
        Ok(())
    }
}

impl MusubiArchiveReverseReferencesV1 {
    /// Validate identity, cardinality, and canonical exact-release order.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive identity is zero, the release list is oversized,
    /// unsorted, or duplicated, or a release identifier is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero()
            || self.releases.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self.releases.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi archive reverse references are invalid or noncanonical",
            ));
        }
        self.releases
            .iter()
            .try_for_each(MusubiReleaseIdV1::validate)
    }
}

impl MusubiSeedIngressReceiptBindingV1 {
    /// Validate every exact deployment, actor, commitment, and anti-replay binding.
    ///
    /// # Errors
    ///
    /// Returns an error if an account identity is invalid, the exact network identity is
    /// malformed, a required identity, digest, or nonce is zero, or the CAR body length is outside
    /// its V1 bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.publisher)?;
        validate_musubi_account_id_v1(&self.ingress_broker)?;
        if self.network_id.as_bytes()[31] & 1 != 1
            || self.seed_provider.as_bytes().iter().all(|byte| *byte == 0)
            || self.semantic_release_manifest_digest.is_zero()
            || self.archive_id.is_zero()
            || self.car_body_digest.is_zero()
            || self.car_body_length == 0
            || self.car_body_length > MUSUBI_MAX_CAR_BYTES_V1
            || digest_is_zero(&self.nonce)
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt binding is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiSeedIngressReceiptPayloadV1 {
    /// Validate the closed schema, exact request binding, and bounded positive lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error if the request binding is invalid, the schema version is unsupported, or
    /// the issue and expiry times do not define a positive lifetime within the V1 bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.binding.validate()?;
        let lifetime = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .filter(|lifetime| *lifetime > 0)
            .ok_or_else(|| ParseError::new("Musubi seed-ingress receipt lifetime is invalid"))?;
        if self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.issued_at_ms == 0
            || lifetime > MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt lifetime or version is invalid",
            ));
        }
        Ok(())
    }
    /// Compute the domain-separated typed hash signed by the ingress broker controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        domain_signing_hash(MUSUBI_SEED_INGRESS_RECEIPT_SIGNATURE_DOMAIN_V1, self)
    }
}

impl MusubiSeedIngressReceiptV1 {
    /// Validate the payload and bounded, strictly ordered controller approval set.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is invalid, approvals are empty, oversized, unsorted, or
    /// duplicated, or an approval signature has an invalid payload length.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.payload.validate()?;
        if self.approvals.is_empty()
            || self.approvals.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt approvals must be bounded, sorted, and unique",
            ));
        }
        self.approvals.iter().try_for_each(|approval| {
            validate_musubi_approval_signature_v1(&approval.public_key, &approval.signature)
        })
    }
    /// Verify the exact request binding, receipt validity window, and broker controller quorum.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the expected binding or validity window does not
    /// match, an approval is not a broker key, a signature fails, or controller quorum is absent.
    pub fn verify(
        &self,
        expected_binding: &MusubiSeedIngressReceiptBindingV1,
        current_time_ms: u64,
    ) -> Result<(), ParseError> {
        self.validate()?;
        if &self.payload.binding != expected_binding
            || current_time_ms < self.payload.issued_at_ms
            || current_time_ms > self.payload.expires_at_ms
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt binding or validity window does not match",
            ));
        }
        let signing_hash = self.payload.signing_hash();
        match self.payload.binding.ingress_broker.controller() {
            AccountController::Single(public_key) => {
                let [approval] = self.approvals.as_slice() else {
                    return Err(ParseError::new(
                        "Musubi single-key ingress broker requires exactly one approval",
                    ));
                };
                if &approval.public_key != public_key {
                    return Err(ParseError::new(
                        "Musubi seed-ingress receipt approval is not a broker key",
                    ));
                }
                approval
                    .signature
                    .verify_hash(public_key, signing_hash)
                    .map_err(|_| ParseError::new("Musubi seed-ingress receipt signature failed"))
            }
            AccountController::Multisig(policy) => {
                let mut approved_weight = 0_u32;
                for approval in &self.approvals {
                    let Some(member) = policy
                        .members()
                        .iter()
                        .find(|member| member.public_key() == &approval.public_key)
                    else {
                        return Err(ParseError::new(
                            "Musubi seed-ingress receipt approval is not a broker key",
                        ));
                    };
                    approval
                        .signature
                        .verify_hash(&approval.public_key, signing_hash)
                        .map_err(|_| {
                            ParseError::new("Musubi seed-ingress receipt signature failed")
                        })?;
                    approved_weight = approved_weight
                        .checked_add(u32::from(member.weight()))
                        .ok_or_else(|| {
                            ParseError::new("Musubi seed-ingress receipt weight overflows")
                        })?;
                }
                if approved_weight < u32::from(policy.threshold()) {
                    return Err(ParseError::new(
                        "Musubi seed-ingress receipt does not meet broker threshold",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl MusubiProviderBundleVerificationBindingV1 {
    /// Validate exact provider authority, finalized completion, and parsed bundle commitments.
    ///
    /// # Errors
    ///
    /// Returns an error if an account, provider authority, assignment, finalized anchor, archive,
    /// replication order, exact network identity, or required bundle commitment is invalid or
    /// inert.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.completed_by)?;
        validate_musubi_account_id_v1(&self.completion_authority.provider_owner)?;
        if self.network_id.as_bytes()[31] & 1 != 1
            || self.provider_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.completed_by != self.completion_authority.provider_owner
            || !self.completion_authority.is_valid()
            || self
                .replication_order
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.assignment_revision == 0
            || self.completion_epoch == 0
            || !self.finalized_anchor.is_valid()
            || self.archive_id.is_zero()
            || self.bundle_digest.is_zero()
            || self.descriptor_digest.is_zero()
            || self.semantic_release_manifest_digest.is_zero()
            || self.verification_lock_digest.is_zero()
            || self.source_tree_digest.is_zero()
        {
            return Err(ParseError::new(
                "Musubi provider bundle verification binding is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiProviderBundleVerificationPayloadV1 {
    /// Validate the closed schema and every exact attestation binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the attestation version is unsupported or its exact provider binding
    /// is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.version != MUSUBI_REGISTRY_VERSION_V1 {
            return Err(ParseError::new(
                "Musubi provider bundle verification version is invalid",
            ));
        }
        self.binding.validate()
    }
    /// Compute the domain-separated typed hash signed by the provider-owner controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        domain_signing_hash(MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SIGNATURE_DOMAIN_V1, self)
    }
}

impl MusubiProviderBundleVerificationAttestationV1 {
    /// Validate the payload and bounded, strictly ordered controller approval set.
    ///
    /// # Errors
    ///
    /// Returns an error if canonical encoding fails or is oversized, the payload is invalid,
    /// approvals are empty or noncanonical, or an approval signature length is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        let canonical_len = canonical_frame_len(self).map_err(|_| {
            ParseError::new("Musubi provider bundle attestation has no canonical Norito encoding")
        })?;
        if canonical_len == 0
            || canonical_len > MUSUBI_MAX_PROVIDER_BUNDLE_ATTESTATION_CANONICAL_BYTES_V1
        {
            return Err(ParseError::new(
                "Musubi provider bundle attestation exceeds its canonical byte bound",
            ));
        }
        self.payload.validate()?;
        if self.approvals.is_empty()
            || self.approvals.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(ParseError::new(
                "Musubi provider bundle approvals must be bounded, sorted, and unique",
            ));
        }
        self.approvals.iter().try_for_each(|approval| {
            validate_musubi_approval_signature_v1(&approval.public_key, &approval.signature)
        })
    }
    /// Return the deterministic immutable storage identity selected by the signed binding.
    #[must_use]
    pub const fn key(&self) -> MusubiProviderBundleAttestationKeyV1 {
        MusubiProviderBundleAttestationKeyV1 {
            archive_id: self.payload.binding.archive_id,
            replication_order: self.payload.binding.replication_order,
            provider_id: self.payload.binding.provider_id,
        }
    }
    /// Compute the domain-separated digest of the complete canonical attestation.
    #[must_use]
    pub fn digest(&self) -> MusubiProviderBundleAttestationDigestV1 {
        MusubiProviderBundleAttestationDigestV1(domain_hash_value(
            MUSUBI_PROVIDER_BUNDLE_ATTESTATION_DIGEST_DOMAIN_V1,
            self,
        ))
    }
    /// Return the compact provider/digest reference used by an archive-location set commitment.
    #[must_use]
    pub fn reference(&self) -> MusubiProviderBundleAttestationRefV1 {
        MusubiProviderBundleAttestationRefV1 {
            provider_id: self.payload.binding.provider_id,
            digest: self.digest(),
        }
    }
    /// Verify the exact finalized completion binding and provider-owner controller quorum.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the expected binding differs, an approval is not a
    /// provider-owner key, a signature fails, or the provider-owner threshold is not met.
    pub fn verify(
        &self,
        expected_binding: &MusubiProviderBundleVerificationBindingV1,
    ) -> Result<(), ParseError> {
        self.validate()?;
        if &self.payload.binding != expected_binding {
            return Err(ParseError::new(
                "Musubi provider bundle verification binding does not match",
            ));
        }
        let signing_hash = self.payload.signing_hash();
        match self
            .payload
            .binding
            .completion_authority
            .provider_owner
            .controller()
        {
            AccountController::Single(public_key) => {
                let [approval] = self.approvals.as_slice() else {
                    return Err(ParseError::new(
                        "Musubi single-key provider owner requires exactly one approval",
                    ));
                };
                if &approval.public_key != public_key {
                    return Err(ParseError::new(
                        "Musubi provider bundle approval is not a provider-owner key",
                    ));
                }
                approval
                    .signature
                    .verify_hash(public_key, signing_hash)
                    .map_err(|_| ParseError::new("Musubi provider bundle signature failed"))
            }
            AccountController::Multisig(policy) => {
                let mut approved_weight = 0_u32;
                for approval in &self.approvals {
                    let Some(member) = policy
                        .members()
                        .iter()
                        .find(|member| member.public_key() == &approval.public_key)
                    else {
                        return Err(ParseError::new(
                            "Musubi provider bundle approval is not a provider-owner key",
                        ));
                    };
                    approval
                        .signature
                        .verify_hash(&approval.public_key, signing_hash)
                        .map_err(|_| ParseError::new("Musubi provider bundle signature failed"))?;
                    approved_weight = approved_weight
                        .checked_add(u32::from(member.weight()))
                        .ok_or_else(|| {
                            ParseError::new("Musubi provider bundle approval weight overflows")
                        })?;
                }
                if approved_weight < u32::from(policy.threshold()) {
                    return Err(ParseError::new(
                        "Musubi provider bundle approvals do not meet provider-owner threshold",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl MusubiProviderBundleAttestationKeyV1 {
    /// Validate every immutable identity component.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive, replication order, or provider identity is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero()
            || digest_is_zero(self.replication_order.as_bytes())
            || self.provider_id.as_bytes().iter().all(|byte| *byte == 0)
        {
            return Err(ParseError::new(
                "Musubi provider bundle attestation key is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiProviderBundleAttestationRefV1 {
    /// Validate the compact provider and digest binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider identity or attestation digest is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.provider_id.as_bytes().iter().all(|byte| *byte == 0) || self.digest.is_zero() {
            return Err(ParseError::new(
                "Musubi provider bundle attestation reference is invalid",
            ));
        }
        Ok(())
    }
}

impl MusubiProviderBundleAttestationRecordV1 {
    /// Validate the full proof and every redundant immutable identity binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the key, attestation, or registering account is invalid, or if the
    /// stored key, digest, and nonzero registration height do not bind that attestation exactly.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.key.validate()?;
        self.attestation.validate()?;
        validate_musubi_account_id_v1(&self.registered_by)?;
        if self.key != self.attestation.key()
            || self.attestation_digest.is_zero()
            || self.attestation_digest != self.attestation.digest()
            || self.registered_at_height == 0
        {
            return Err(ParseError::new(
                "Musubi provider bundle attestation record is inconsistent",
            ));
        }
        Ok(())
    }
}

fn validate_archive_registration_fields(
    archive_id: ArchiveId,
    commitment: &MusubiArchiveCommitmentV1,
    staging_receipt: &MusubiSeedIngressReceiptV1,
    registered_by: &AccountId,
    registered_at_height: u64,
) -> Result<(), ParseError> {
    commitment.validate()?;
    staging_receipt.validate()?;
    validate_musubi_account_id_v1(registered_by)?;
    if archive_id != commitment.archive_id()
        || staging_receipt.payload.binding.archive_id != archive_id
        || staging_receipt.payload.binding.car_body_digest != commitment.car_digest
        || staging_receipt.payload.binding.car_body_length != commitment.car_size
        || &staging_receipt.payload.binding.publisher != registered_by
        || registered_at_height == 0
    {
        return Err(ParseError::new(
            "Musubi archive registration has an invalid identity or receipt",
        ));
    }
    Ok(())
}
