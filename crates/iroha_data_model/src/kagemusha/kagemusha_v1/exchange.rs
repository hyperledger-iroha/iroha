//! Request, payment, paired-proof and acknowledgement validation.
//!
//! These methods validate canonical wire shapes and bindings. Monetary admission
//! still requires authenticated ledger state and the release-pinned verifier.

use super::{
    KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
    KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
    KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
    KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1,
    KAGEMUSHA_REQUEST_MAX_TTL_MS_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaAcknowledgementV1,
    KagemushaEncryptedCreditEnvelopeV1, KagemushaHardwareProfileV1, KagemushaPairedProofV1,
    KagemushaPaymentOutputV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
    KagemushaPeerCreditContextV1, KagemushaValidationErrorV1, PAYMENT_DIGEST_DOMAIN,
    REQUEST_DIGEST_DOMAIN, REQUEST_SIGNING_DOMAIN, STATEMENT_DIGEST_DOMAIN,
    commit_evidence_circuit_transcript_v1, decode_bounded_canonical, decode_kagemusha_text_v1,
    digest_bytes, digest_encoded, encode_kagemusha_text_v1, invalid,
    kagemusha_acknowledgement_signing_bytes_v1, kagemusha_credit_id_v1,
    kagemusha_liability_pool_id_v1, kagemusha_payment_body_digest_v1,
    kagemusha_payment_request_signing_bytes_v1, kagemusha_prepared_transfer_digest_v1,
    require_encoded_size, require_nonzero, require_valid_header, require_valid_x25519_public_key,
};

impl KagemushaPaymentRequestV1 {
    /// Return the fixed signed semantic transcript, distinct from Norito wire encoding.
    ///
    /// Layout: version:u16 LE | release:32 | network:32 | normalized asset:32 |
    /// incarnation:32 | scale:u32 LE | pool:32 | normalized account:32 |
    /// amount:u128 LE | receiver encryption key:32 | credential ID:32 |
    /// request ID:32 | issued:u64 LE |
    /// expires:u64 LE | signature:64. Identity normalization uses the existing
    /// typed Norito asset/account digest domains; no typed identity is discarded on wire.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid amount/key or unencodable normalized identity.
    pub fn circuit_transcript_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        let signed = self.canonical_signing_bytes()?;
        let mut bytes = signed[REQUEST_SIGNING_DOMAIN.len() + 1..].to_vec();
        bytes.extend_from_slice(self.signature.as_raw_bytes());
        Ok(bytes)
    }

    /// Return the exact bytes signed by the recipient device.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        kagemusha_payment_request_signing_bytes_v1(
            self.version,
            self.release_id,
            &self.network_id,
            &self.asset,
            self.asset_incarnation,
            self.scale,
            self.liability_pool_id,
            &self.recipient,
            self.amount,
            self.recipient_encryption_key,
            self.hardware_credential.credential_id,
            self.request_id,
            self.issued_at_ms,
            self.expires_at_ms,
        )
    }

    /// Encode this validated request as canonical unpadded `kgm1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid, cannot be encoded, or exceeds its cap.
    pub fn encode_text(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` request.
    ///
    /// Text syntax and size are checked before base64 decoding, and the raw cap
    /// is checked before Norito decoding. The decoded request must re-encode to
    /// the exact original text.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, oversized, padded, non-canonical, or legacy text.
    pub fn decode_text_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_exact,
        )
    }

    /// Decode and validate one exact bounded recipient request.
    ///
    /// The byte cap is enforced before Norito reads a header or declared
    /// sequence length, and decoding rejects non-canonical byte forms.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid request.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let request: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        request.validate_shape()?;
        Ok(request)
    }

    /// Validate the request's shape and its signature under its embedded key.
    ///
    /// The expiry is an exclusive deadline for the sender's trusted commit. It
    /// is not a validity horizon for an already committed payment.
    /// This deliberately does **not** authenticate the embedded governance
    /// credential. Monetary callers must additionally call
    /// [`Self::validate_against_profile`] with a profile resolved from an
    /// authenticated release catalog.
    ///
    /// # Errors
    ///
    /// Returns an error when any request invariant fails.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "kagemusha.request.header",
        )?;
        for (field, value) in [
            ("kagemusha.request.release_id", self.release_id),
            (
                "kagemusha.request.liability_pool_id",
                self.liability_pool_id,
            ),
            ("kagemusha.request.request_id", self.request_id),
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
            return Err(invalid("kagemusha.request.liability_pool_id"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.request.asset_incarnation"))?;
        if self.amount == 0 {
            return Err(invalid("kagemusha.request.amount"));
        }
        require_valid_x25519_public_key(
            "kagemusha.request.recipient_encryption_key",
            self.recipient_encryption_key,
        )?;
        self.hardware_credential.validate_shape()?;
        if self.hardware_credential.network_id != self.network_id
            || self.issued_at_ms < self.hardware_credential.issued_at_ms
            || self.expires_at_ms > self.hardware_credential.expires_at_ms
        {
            return Err(invalid("kagemusha.request.hardware_credential"));
        }
        let ttl = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .ok_or_else(|| invalid("kagemusha.request.expires_at_ms"))?;
        if ttl == 0 || ttl > KAGEMUSHA_REQUEST_MAX_TTL_MS_V1 {
            return Err(invalid("kagemusha.request.expires_at_ms"));
        }
        self.signature.verify(
            &self.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Authenticate the compact credential against one release-resolved
    /// hardware profile after validating the complete request shape.
    ///
    /// This authenticates the request and receiver hardware identity only. It
    /// does not verify a later payment's recursive proof; Core must still use
    /// the release-pinned native proof verifier before monetary admission.
    ///
    /// # Errors
    ///
    /// Returns an error for any structural, signature, profile, suite, policy,
    /// firmware, capability, or lifetime mismatch.
    pub fn validate_against_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        self.hardware_credential.validate_against_profile(profile)
    }

    /// Return the canonical request identity consumed by a sender split.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        Ok(digest_bytes(
            REQUEST_DIGEST_DOMAIN,
            &self.circuit_transcript_bytes()?,
        ))
    }
}

impl KagemushaPaymentOutputV1 {
    /// Return the exact fixed semantic transcript, not Norito wire bytes.
    #[must_use]
    pub fn circuit_transcript_bytes(&self) -> [u8; 254] {
        let mut bytes = [0; 254];
        bytes[..2].copy_from_slice(&self.version.to_le_bytes());
        bytes[2..34].copy_from_slice(&self.request_digest);
        bytes[34..50].copy_from_slice(&self.amount.to_le_bytes());
        for (index, digest) in [
            self.sender_before_commitment,
            self.sender_after_commitment,
            self.transition_nullifier,
            self.credit_id,
            self.ciphertext_commitment,
        ]
        .iter()
        .enumerate()
        {
            bytes[50 + index * 32..82 + index * 32].copy_from_slice(digest);
        }
        bytes[210..246]
            .copy_from_slice(&commit_evidence_circuit_transcript_v1(self.commit_evidence));
        bytes[246..].copy_from_slice(&self.committed_at_ms.to_le_bytes());
        bytes
    }

    /// Digest the fixed output transcript independently of proof and certificate bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for reserved fields, an unchanged state, or invalid evidence.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.amount == 0
            || self.committed_at_ms == 0
            || self.sender_before_commitment == self.sender_after_commitment
            || [
                self.request_digest,
                self.sender_before_commitment,
                self.sender_after_commitment,
                self.transition_nullifier,
                self.credit_id,
                self.ciphertext_commitment,
            ]
            .contains(&[0; 32])
        {
            return Err(invalid("kagemusha.payment_output.shape"));
        }
        self.commit_evidence.validate()?;
        Ok(digest_bytes(
            STATEMENT_DIGEST_DOMAIN,
            &self.circuit_transcript_bytes(),
        ))
    }

    /// Compute the request-bound credit ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request or zero nullifier.
    pub fn expected_credit_id_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        kagemusha_credit_id_v1(self.transition_nullifier, request.canonical_digest()?)
    }

    /// Populate the stable credit ID before encryption.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request context.
    pub fn seal_credit_id_against(
        mut self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        self.credit_id = self.expected_credit_id_against(request)?;
        Ok(self)
    }

    /// Build the pre-encryption context directly from the signed request.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request, output, or receiver binding.
    pub fn peer_credit_context_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<KagemushaPeerCreditContextV1, KagemushaValidationErrorV1> {
        request.validate_shape()?;
        self.canonical_digest()?;
        let request_digest = request.canonical_digest()?;
        if self.request_digest != request_digest
            || self.amount != request.amount
            || self.credit_id != self.expected_credit_id_against(request)?
            || self.committed_at_ms < request.issued_at_ms
            || self.committed_at_ms >= request.expires_at_ms
        {
            return Err(invalid("kagemusha.peer_credit_context.binding"));
        }
        let context = KagemushaPeerCreditContextV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest,
            amount: self.amount,
            sender_before_commitment: self.sender_before_commitment,
            sender_after_commitment: self.sender_after_commitment,
            prepared_transfer_digest: kagemusha_prepared_transfer_digest_v1(
                request,
                self.sender_before_commitment,
                self.sender_after_commitment,
                self.transition_nullifier,
                self.ciphertext_commitment,
            )?,
            recipient_encryption_key: request.recipient_encryption_key,
        };
        context.validate_shape()?;
        Ok(context)
    }

    /// Validate direct request, receiver, amount, deadline, and output bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or substituted context.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.peer_credit_context_against(request)?;
        Ok(())
    }

    /// Digest the output after exact request validation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or substituted context.
    pub fn canonical_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        self.canonical_digest()
    }
}

impl KagemushaPairedProofV1 {
    /// Validate fixed parity roles, proof caps, and exact history sizes.
    ///
    /// # Errors
    ///
    /// Returns an error when the proof is empty, oversized, aliased, or mis-bound.
    pub fn validate_shape_for_semantic_digest(
        &self,
        expected_semantic_digest: [u8; 32],
    ) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 {
            return Err(invalid("kagemusha.proof.version"));
        }
        require_nonzero(
            "kagemusha.proof.eq_protocol_digest",
            self.eq_protocol_digest,
        )?;
        require_nonzero(
            "kagemusha.proof.ep_protocol_digest",
            self.ep_protocol_digest,
        )?;
        require_nonzero("kagemusha.proof.semantic_digest", self.semantic_digest)?;
        require_nonzero(
            "kagemusha.proof.guard_eq_credential_audit",
            self.guard_eq_credential_audit,
        )?;
        require_nonzero(
            "kagemusha.proof.guard_ep_credential_audit",
            self.guard_ep_credential_audit,
        )?;
        require_nonzero("kagemusha.proof.eq_deferred_audit", self.eq_deferred_audit)?;
        require_nonzero("kagemusha.proof.ep_deferred_audit", self.ep_deferred_audit)?;
        if self.eq_protocol_digest == self.ep_protocol_digest
            || self.semantic_digest != expected_semantic_digest
            || self.guard_eq_credential_audit == self.guard_ep_credential_audit
            || self.eq_deferred_audit == self.ep_deferred_audit
        {
            return Err(invalid("kagemusha.proof.role_binding"));
        }
        if self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1
        {
            return Err(invalid("kagemusha.proof.current"));
        }
        if self.eq_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(invalid("kagemusha.proof.history"));
        }
        require_encoded_size(self, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1)?;
        Ok(())
    }
}

impl KagemushaPaymentV1 {
    /// Encode a shape-validated committed payment as canonical text.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, bindings, framing, or size.
    pub fn encode_text_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode bounded canonical text against the exact request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, bindings, framing, or size.
    pub fn decode_text_exact_against(
        text: &str,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request),
        )
    }

    /// Decode the complete bounded canonical committed-payment envelope.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, bindings, framing, or size.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let payment: Self = decode_bounded_canonical(bytes, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
        payment.validate_shape_against(request)?;
        Ok(payment)
    }

    /// Return the proof-independent body digest after exact output validation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or substituted request, key, or output.
    pub fn body_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.output.version != self.version {
            return Err(invalid("kagemusha.payment.version"));
        }
        self.output.validate_shape_against(request)?;
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            request.recipient_encryption_key,
        )?;
        kagemusha_payment_body_digest_v1(&self.output, &self.encrypted_credit)
    }

    /// Validate exact wire bindings without claiming cryptographic proof validity.
    ///
    /// Native verification must authenticate the release/profile and recursively
    /// prove the hidden sender transition and exact hardware certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid certificate, proof, evidence, output, or size.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        let body_digest = self.body_digest_against(request)?;
        self.commit_certificate.validate_shape()?;
        if self.commit_certificate.transition_nullifier != self.output.transition_nullifier
            || self.commit_certificate.commit_evidence != self.output.commit_evidence
        {
            return Err(invalid("kagemusha.payment.commit_certificate"));
        }
        self.proof.validate_shape_against(
            body_digest,
            self.commit_certificate.candidate_envelope_digest,
            self.commit_certificate.canonical_digest()?,
        )?;
        require_encoded_size(self, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the complete proof-bearing envelope digest after finalization.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid envelope or contextual binding.
    pub fn canonical_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        digest_encoded(PAYMENT_DIGEST_DOMAIN, self)
    }

    /// Return the unlinkable transition nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error for the reserved zero nullifier.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        require_nonzero(
            "kagemusha.payment.transition_nullifier",
            self.output.transition_nullifier,
        )?;
        Ok(self.output.transition_nullifier)
    }
}

impl KagemushaAcknowledgementV1 {
    /// Encode this acknowledgement as canonical unpadded `kgm1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when context validation, encoding, or a size bound fails.
    pub fn encode_text_against(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape_against(request, payment)?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, prefix, or canonical bytes.
    pub fn decode_text_exact_against(
        text: &str,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request, payment),
        )
    }

    /// Decode and validate one bounded durable-inbox acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid input.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let acknowledgement: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        acknowledgement.validate_shape_against(request, payment)?;
        Ok(acknowledgement)
    }

    /// Return the exact bytes signed after persisting the inbox receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        kagemusha_acknowledgement_signing_bytes_v1(
            self.version,
            self.request_digest,
            self.payment_digest,
            self.inbox_receipt,
        )
    }

    /// Validate request, payment, credit, durable receipt, and receiver signature bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when any receipt, identity, signature, or size binding fails.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        payment.validate_shape_against(request)?;
        let request_digest = request.canonical_digest()?;
        let payment_digest = payment.canonical_digest_against(request)?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.request_digest != request_digest
            || self.payment_digest != payment_digest
            || self.inbox_receipt.version != self.version
            || self.inbox_receipt.credit_id != payment.output.credit_id
            || self.inbox_receipt.receipt_commitment == [0; 32]
        {
            return Err(invalid("kagemusha.acknowledgement.binding"));
        }
        self.signature.verify(
            &request.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        Ok(())
    }
}
