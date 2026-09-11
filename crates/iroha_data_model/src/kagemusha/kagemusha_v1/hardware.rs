//! Hardware profile, credential and durable commit validation.
//!
//! These methods validate canonical wire shapes and bindings. Monetary admission
//! still requires authenticated ledger state and the release-pinned verifier.

use super::{
    COMMIT_CERTIFICATE_DIGEST_DOMAIN, COMMIT_CERTIFICATE_ID_DOMAIN, HARDWARE_CREDENTIAL_ID_DOMAIN,
    HARDWARE_CREDENTIAL_SIGNING_DOMAIN, HARDWARE_PROFILE_DIGEST_DOMAIN,
    HARDWARE_TERMINAL_BODY_COMMITMENT_DOMAIN, HardwareCredentialIdPreimageV1,
    HardwareCredentialSigningPreimageV1, HardwareProfileIdPreimageV1,
    KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1, KAGEMUSHA_HARDWARE_CREDENTIAL_ID_LANE_OFFSET_V1,
    KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1, KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1,
    KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1, KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1,
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KAGEMUSHA_WIRE_VERSION_V1,
    KagemushaCommitCertificateV1, KagemushaCommitEvidenceV1, KagemushaHardwareCredentialV1,
    KagemushaHardwareProfileV1, KagemushaHardwareTerminalBodyV1, KagemushaLifecycleBindingV1,
    KagemushaOperationKindV1, KagemushaOutboxReservationV1, KagemushaValidationErrorV1,
    LIFECYCLE_BINDING_DIGEST_DOMAIN, OUTBOX_RESERVATION_COMMITMENT_DOMAIN, SUITE_COMMITMENT_DOMAIN,
    commit_certificate_circuit_transcript_v1, commit_certificate_id_circuit_transcript_v1,
    decode_bounded_canonical, digest_bytes, digest_encoded, fixed_canonical_preimage_bytes_v1,
    invalid, kagemusha_device_key_reference_v1, kagemusha_liability_pool_id_v1,
    kagemusha_outbox_min_reserved_bytes_v1, outbox_reservation_circuit_transcript_v1,
    require_encoded_size, require_nonzero, require_valid_header,
};

impl KagemushaHardwareProfileV1 {
    fn id_preimage(&self) -> HardwareProfileIdPreimageV1 {
        HardwareProfileIdPreimageV1 {
            version: self.version,
            protocol_version: self.protocol_version,
            provider_id: self.provider_id,
            platform_class: self.platform_class,
            product_class_digest: self.product_class_digest,
            firmware_policy_digest: self.firmware_policy_digest,
            enrollment_attestation_verifier_digest: self.enrollment_attestation_verifier_digest,
            attestation_trust_roots_digest: self.attestation_trust_roots_digest,
            allowed_suite_commitment: self.allowed_suite_commitment,
            policy_epoch: self.policy_epoch,
            governance_credential_public_key: self.governance_credential_public_key,
            capability_mask: self.capability_mask,
            qualification_report_digest: self.qualification_report_digest,
            valid_from_ms: self.valid_from_ms,
            expires_at_ms: self.expires_at_ms,
        }
    }

    /// Compute the domain-separated profile identity from its unsigned body.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_hardware_profile_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(HARDWARE_PROFILE_DIGEST_DOMAIN, &self.id_preimage())
    }

    /// Return the exact unchanged canonical Norito profile-ID preimage.
    ///
    /// The 40-byte header and fifteen compact-length-prefixed fields are retained,
    /// including the u32-LE platform discriminant, 65-byte governance key, and CRC64.
    /// This encodes the unsigned identity body without validating or authorizing
    /// the profile, so it can also be used before sealing its profile ID.
    ///
    /// # Errors
    ///
    /// Returns an error for encoding failure or an unexpected fixed V1 layout.
    pub fn canonical_id_preimage_bytes(
        &self,
    ) -> Result<[u8; KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1], KagemushaValidationErrorV1>
    {
        fixed_canonical_preimage_bytes_v1(
            &self.id_preimage(),
            "kagemusha.hardware_profile.id_preimage_layout",
        )
    }

    /// Populate the canonical profile identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical profile-body encoding fails.
    pub fn seal_hardware_profile_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.hardware_profile_id = self.expected_hardware_profile_id()?;
        Ok(self)
    }

    /// Validate the exact governed capability set and profile lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved identity, incomplete/unknown capability
    /// set, invalid issuer key, invalid lifetime, or oversized encoding.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
            || self.policy_epoch == 0
            || self.capability_mask != KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1
            || self.valid_from_ms >= self.expires_at_ms
        {
            return Err(invalid("kagemusha.hardware_profile.header"));
        }
        for (field, value) in [
            (
                "kagemusha.hardware_profile.hardware_profile_id",
                self.hardware_profile_id,
            ),
            ("kagemusha.hardware_profile.provider_id", self.provider_id),
            (
                "kagemusha.hardware_profile.firmware_policy_digest",
                self.firmware_policy_digest,
            ),
            (
                "kagemusha.hardware_profile.product_class_digest",
                self.product_class_digest,
            ),
            (
                "kagemusha.hardware_profile.enrollment_attestation_verifier_digest",
                self.enrollment_attestation_verifier_digest,
            ),
            (
                "kagemusha.hardware_profile.attestation_trust_roots_digest",
                self.attestation_trust_roots_digest,
            ),
            (
                "kagemusha.hardware_profile.allowed_suite_commitment",
                self.allowed_suite_commitment,
            ),
            (
                "kagemusha.hardware_profile.qualification_report_digest",
                self.qualification_report_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        self.governance_credential_public_key.validate()?;
        if self.hardware_profile_id != self.expected_hardware_profile_id()? {
            return Err(invalid("kagemusha.hardware_profile.hardware_profile_id"));
        }
        require_encoded_size(self, KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical governed profile digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        Ok(self.hardware_profile_id)
    }
}

impl KagemushaHardwareCredentialV1 {
    fn id_preimage(&self) -> HardwareCredentialIdPreimageV1 {
        HardwareCredentialIdPreimageV1 {
            version: self.version,
            network_id: self.network_id,
            hardware_profile_id: self.hardware_profile_id,
            suite_id: self.suite_id,
            firmware_policy_digest: self.firmware_policy_digest,
            policy_epoch: self.policy_epoch,
            lane_commitment: self.lane_commitment,
            hardware_epoch_id: self.hardware_epoch_id,
            hardware_epoch_generation: self.hardware_epoch_generation,
            device_public_key: self.device_public_key,
            device_key_reference: self.device_key_reference,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
        }
    }

    /// Compute the canonical compact credential identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_credential_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        Ok(digest_bytes(
            HARDWARE_CREDENTIAL_ID_DOMAIN,
            &self.canonical_id_preimage_bytes()?,
        ))
    }

    /// Return the exact unchanged canonical Norito credential-ID preimage.
    ///
    /// This is not a new wire codec. The 40-byte Norito header is followed by
    /// thirteen compact-length-prefixed fixed-width fields, with payload widths
    /// 2,32,32,32,32,8,32,32,8,65,32,8,8. The lane occupies bytes 185..217.
    /// The identity hashes the credential-ID domain, zero separator, u64-LE
    /// byte length, and these complete bytes, including the canonical header.
    /// A proof opening this preimage must bind its digest to the authenticated
    /// credential ID and the lane at this exact offset, not search for a value.
    ///
    /// # Errors
    ///
    /// Returns an error for an encoding failure or unexpected V1 codec layout.
    pub fn canonical_id_preimage_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        let bytes = norito::encode_canonical(&self.id_preimage())?;
        let offset = KAGEMUSHA_HARDWARE_CREDENTIAL_ID_LANE_OFFSET_V1;
        if bytes.len() != KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1
            || bytes.get(offset..offset + 32) != Some(self.lane_commitment.as_slice())
        {
            return Err(invalid("kagemusha.hardware_credential.id_preimage_layout"));
        }
        Ok(bytes)
    }

    /// Populate the canonical credential identity before governance signs it.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical identity encoding fails.
    pub fn seal_credential_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.credential_id = self.expected_credential_id()?;
        Ok(self)
    }

    /// Return the exact bytes signed by the governed profile issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        Ok(norito::encode_canonical(
            &HardwareCredentialSigningPreimageV1 {
                domain: HARDWARE_CREDENTIAL_SIGNING_DOMAIN.to_vec(),
                credential_id: self.credential_id,
                credential: self.id_preimage(),
            },
        )?)
    }

    /// Validate canonical credential fields, identity, embedded key, and size.
    ///
    /// This verifies only shape and self-consistency. Monetary callers must
    /// authenticate the credential with [`Self::validate_against_profile`]
    /// using a profile resolved from an authenticated release.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, inconsistent, or oversized credentials.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.network_id.as_bytes() == &[0; 32]
            || self.policy_epoch == 0
            || self.hardware_epoch_generation == 0
            || self.issued_at_ms >= self.expires_at_ms
        {
            return Err(invalid("kagemusha.hardware_credential.header"));
        }
        for (field, value) in [
            (
                "kagemusha.hardware_credential.credential_id",
                self.credential_id,
            ),
            (
                "kagemusha.hardware_credential.hardware_profile_id",
                self.hardware_profile_id,
            ),
            ("kagemusha.hardware_credential.suite_id", self.suite_id),
            (
                "kagemusha.hardware_credential.firmware_policy_digest",
                self.firmware_policy_digest,
            ),
            (
                "kagemusha.hardware_credential.lane_commitment",
                self.lane_commitment,
            ),
            (
                "kagemusha.hardware_credential.hardware_epoch_id",
                self.hardware_epoch_id,
            ),
            (
                "kagemusha.hardware_credential.device_key_reference",
                self.device_key_reference,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        self.device_public_key.validate()?;
        if self.device_key_reference != kagemusha_device_key_reference_v1(&self.device_public_key)
            || self.credential_id != self.expected_credential_id()?
        {
            return Err(invalid("kagemusha.hardware_credential.identity"));
        }
        require_encoded_size(self, KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Validate this credential against the exact governed profile.
    ///
    /// # Errors
    ///
    /// Returns an error for any profile, firmware, lifetime, identity, or
    /// governance-signature mismatch.
    pub fn validate_against_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        profile.validate()?;
        self.validate_shape()?;
        if self.hardware_profile_id != profile.hardware_profile_id
            || self.firmware_policy_digest != profile.firmware_policy_digest
            || self.policy_epoch != profile.policy_epoch
            || digest_bytes(SUITE_COMMITMENT_DOMAIN, &self.suite_id)
                != profile.allowed_suite_commitment
            || self.issued_at_ms < profile.valid_from_ms
            || self.expires_at_ms > profile.expires_at_ms
        {
            return Err(invalid("kagemusha.hardware_credential.profile_binding"));
        }
        self.governance_signature.verify(
            &profile.governance_credential_public_key,
            &self.canonical_signing_bytes()?,
        )
    }
}

impl KagemushaCommitEvidenceV1 {
    /// Validate the hiding commitment for the selected qualified deadline source.
    ///
    /// This is a structural check only. A release-pinned proof must establish
    /// trusted time or monotonic-lease consumption before the commit deadline.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved zero evidence commitment.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        match self {
            Self::TrustedTime(evidence) => require_nonzero(
                "kagemusha.commit_evidence.time_evidence_commitment",
                evidence.time_evidence_commitment,
            ),
            Self::MonotonicLease(evidence) => require_nonzero(
                "kagemusha.commit_evidence.lease_evidence_commitment",
                evidence.lease_evidence_commitment,
            ),
        }
    }
}

impl KagemushaLifecycleBindingV1 {
    /// Validate the complete released-transition lifecycle context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved identity, unsupported protocol, invalid
    /// pooled reserve, or malformed operation-specific binding.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "kagemusha.lifecycle.header",
        )?;
        if self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1 || self.policy_epoch == 0 {
            return Err(invalid("kagemusha.lifecycle.context"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.lifecycle.asset_incarnation"))?;
        for (field, value) in [
            ("kagemusha.lifecycle.suite_id", self.suite_id),
            ("kagemusha.lifecycle.vk_digest", self.vk_digest),
            ("kagemusha.lifecycle.release_id", self.release_id),
            (
                "kagemusha.lifecycle.liability_pool_id",
                self.liability_pool_id,
            ),
            (
                "kagemusha.lifecycle.hardware_profile_id",
                self.hardware_profile_id,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("kagemusha.lifecycle.liability_pool_id"));
        }
        let credit_fields = [self.credit_id, self.ciphertext_digest];
        match self.operation_kind {
            KagemushaOperationKindV1::SendSplit
                if self.request_id != [0; 32]
                    && self.receiver_lane_commitment != [0; 32]
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            KagemushaOperationKindV1::SendSplit => {
                return Err(invalid("kagemusha.lifecycle.payment_binding"));
            }
            KagemushaOperationKindV1::MintFold
                if self.request_id == [0; 32]
                    && self.receiver_lane_commitment == [0; 32]
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            KagemushaOperationKindV1::MintFold => {
                return Err(invalid("kagemusha.lifecycle.mint_binding"));
            }
            _ if [self.request_id, self.receiver_lane_commitment]
                .iter()
                .chain(credit_fields.iter())
                .all(|value| *value == [0; 32]) => {}
            _ => return Err(invalid("kagemusha.lifecycle.non_payment_binding")),
        }
        Ok(())
    }

    /// Return the canonical lifecycle digest bound by proof and certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the lifecycle is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        digest_encoded(LIFECYCLE_BINDING_DIGEST_DOMAIN, self)
    }
}

impl KagemushaOutboxReservationV1 {
    /// Validate operation, capacity, identity, and lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-terminal operation, insufficient capacity,
    /// reserved identity, or empty lifetime.
    pub fn validate(self) -> Result<(), KagemushaValidationErrorV1> {
        let minimum = kagemusha_outbox_min_reserved_bytes_v1(self.operation_kind)
            .ok_or_else(|| invalid("kagemusha.outbox_reservation.operation_kind"))?;
        if self.reservation_id == [0; 32]
            || self.reserved_outbox_bytes < minimum
            || self.issued_at_ms >= self.expires_at_ms
        {
            return Err(invalid("kagemusha.outbox_reservation"));
        }
        Ok(())
    }

    /// Return the hiding commitment proven by the final terminal proof.
    ///
    /// # Errors
    ///
    /// Returns an error when validation fails.
    pub fn canonical_commitment(self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        Ok(digest_bytes(
            OUTBOX_RESERVATION_COMMITMENT_DOMAIN,
            &outbox_reservation_circuit_transcript_v1(self),
        ))
    }
}

impl KagemushaHardwareTerminalBodyV1 {
    /// Return the hiding commitment used by the terminal certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved field, unsupported version, invalid
    /// commit evidence, or canonical encoding failure.
    pub fn canonical_commitment(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.policy_epoch == 0 {
            return Err(invalid("kagemusha.hardware_terminal_body.context"));
        }
        self.commit_evidence.validate()?;
        for (field, value) in [
            (
                "kagemusha.hardware_terminal_body.candidate_envelope_digest",
                self.candidate_envelope_digest,
            ),
            (
                "kagemusha.hardware_terminal_body.lifecycle_binding_digest",
                self.lifecycle_binding_digest,
            ),
            (
                "kagemusha.hardware_terminal_body.transition_nullifier",
                self.transition_nullifier,
            ),
            (
                "kagemusha.hardware_terminal_body.outbox_reservation_commitment",
                self.outbox_reservation_commitment,
            ),
            (
                "kagemusha.hardware_terminal_body.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "kagemusha.hardware_terminal_body.private_successor_commitment",
                self.private_successor_commitment,
            ),
            (
                "kagemusha.hardware_terminal_body.private_journal_commitment",
                self.private_journal_commitment,
            ),
            (
                "kagemusha.hardware_terminal_body.private_recovery_commitment",
                self.private_recovery_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        digest_encoded(HARDWARE_TERMINAL_BODY_COMMITMENT_DOMAIN, self)
    }
}

impl KagemushaCommitCertificateV1 {
    /// Validate public shape and self-identity without authorizing hardware state.
    ///
    /// The native proof must bind the complete sender lifecycle and authenticate
    /// the terminal commitment against a qualified profile.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed evidence, identity, reserved fields or size.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        self.commit_evidence.validate()?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.policy_epoch == 0
            || [
                self.candidate_envelope_digest,
                self.lifecycle_binding_digest,
                self.transition_nullifier,
                self.outbox_reservation_commitment,
                self.hardware_profile_id,
                self.hardware_terminal_commitment,
            ]
            .contains(&[0; 32])
            || self.certificate_id != self.expected_certificate_id()?
        {
            return Err(invalid("kagemusha.commit_certificate.shape"));
        }
        require_encoded_size(self, KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Digest the exact public certificate transcript after structural validation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed certificate shape or self-identity.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_DIGEST_DOMAIN,
            &commit_certificate_circuit_transcript_v1(self),
        ))
    }
    /// Compute the terminal certificate identity without self-reference.
    ///
    /// # Errors
    ///
    /// Returns an error only if its fixed transcript cannot be constructed.
    pub fn expected_certificate_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_ID_DOMAIN,
            &commit_certificate_id_circuit_transcript_v1(self),
        ))
    }

    /// Populate the canonical certificate identity.
    ///
    /// # Errors
    ///
    /// Returns an error if identity derivation fails.
    pub fn seal_certificate_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.certificate_id = self.expected_certificate_id()?;
        Ok(self)
    }

    /// Bind a self-free terminal body and then derive the certificate identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the body is invalid or differs from the public certificate fields.
    pub fn seal_with_terminal_body(
        mut self,
        body: &KagemushaHardwareTerminalBodyV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        if body.version != self.version
            || body.candidate_envelope_digest != self.candidate_envelope_digest
            || body.lifecycle_binding_digest != self.lifecycle_binding_digest
            || body.transition_nullifier != self.transition_nullifier
            || body.outbox_reservation_commitment != self.outbox_reservation_commitment
            || body.commit_evidence != self.commit_evidence
            || body.hardware_profile_id != self.hardware_profile_id
            || body.policy_epoch != self.policy_epoch
        {
            return Err(invalid("kagemusha.hardware_terminal_body.binding"));
        }
        self.hardware_terminal_commitment = body.canonical_commitment()?;
        self.seal_certificate_id()
    }

    /// Validate this recoverable certificate against the exact lifecycle.
    ///
    /// # Errors
    ///
    /// Returns an error for a substituted lifecycle, evidence, nullifier,
    /// terminal body, certificate identity, or size.
    pub fn validate_against(
        &self,
        lifecycle: &KagemushaLifecycleBindingV1,
        expected_evidence: KagemushaCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<(), KagemushaValidationErrorV1> {
        lifecycle.validate()?;
        self.commit_evidence.validate()?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.candidate_envelope_digest == [0; 32]
            || self.lifecycle_binding_digest != lifecycle.canonical_digest()?
            || self.transition_nullifier != expected_nullifier
            || self.transition_nullifier == [0; 32]
            || self.outbox_reservation_commitment == [0; 32]
            || self.commit_evidence != expected_evidence
            || self.hardware_profile_id != lifecycle.hardware_profile_id
            || self.policy_epoch != lifecycle.policy_epoch
            || self.hardware_terminal_commitment == [0; 32]
            || self.certificate_id != self.expected_certificate_id()?
        {
            return Err(invalid("kagemusha.commit_certificate.binding"));
        }
        require_encoded_size(self, KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the fixed-width certificate digest constrained by both final-proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lifecycle, evidence, nullifier, or certificate binding.
    pub fn canonical_digest_against(
        &self,
        lifecycle: &KagemushaLifecycleBindingV1,
        expected_evidence: KagemushaCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_against(lifecycle, expected_evidence, expected_nullifier)?;
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_DIGEST_DOMAIN,
            &commit_certificate_circuit_transcript_v1(self),
        ))
    }

    /// Decode and validate one exact bounded terminal certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, oversized, non-canonical, or substituted input.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        lifecycle: &KagemushaLifecycleBindingV1,
        expected_evidence: KagemushaCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let certificate: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        certificate.validate_against(lifecycle, expected_evidence, expected_nullifier)?;
        Ok(certificate)
    }
}
