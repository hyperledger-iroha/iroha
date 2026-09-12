//! Mint authorization, mint credit and redemption validation.
//!
//! These methods validate canonical wire shapes and bindings. Monetary admission
//! still requires authenticated ledger state and the release-pinned verifier.

use super::{
    KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1, KAGEMUSHA_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
    KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1, KAGEMUSHA_MINT_CREDIT_TEXT_MAX_BYTES_V1,
    KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1, KAGEMUSHA_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaEncryptedCreditAadV1, KagemushaEncryptedCreditEnvelopeV1,
    KagemushaMintAuthorizationContextV1, KagemushaMintAuthorizationStatementV1,
    KagemushaMintAuthorizationV1, KagemushaMintCreditStatementV1, KagemushaMintCreditV1,
    KagemushaOperationKindV1, KagemushaRedemptionStatementV1, KagemushaRedemptionVoucherV1,
    KagemushaValidationErrorV1, MINT_AUTHORIZATION_CONTEXT_DIGEST_DOMAIN,
    MINT_AUTHORIZATION_DIGEST_DOMAIN, MINT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN,
    MINT_CREDIT_ID_DOMAIN, MINT_LIFECYCLE_CONTEXT_DOMAIN, MINT_STATEMENT_DIGEST_DOMAIN,
    MintCreditIdPreimageV1, MintLifecycleContextPreimageV1, REDEMPTION_ID_DOMAIN,
    REDEMPTION_STATEMENT_DIGEST_DOMAIN, RedemptionIdPreimageV1, decode_bounded_canonical,
    decode_kagemusha_text_v1, digest_encoded, encode_kagemusha_text_v1, invalid,
    kagemusha_ciphertext_digest_v1, kagemusha_liability_pool_id_v1, require_encoded_size,
    require_nonzero, require_valid_header, require_valid_x25519_public_key,
};

impl KagemushaMintAuthorizationContextV1 {
    /// Validate the exact pre-ID recipient, asset, release, and commitment context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved value, invalid asset incarnation, or
    /// non-canonical pooled-reserve identity.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            Some(self.amount),
            "kagemusha.mint_authorization_context.header",
        )?;
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.mint_authorization_context.asset_incarnation"))?;
        for (field, value) in [
            (
                "kagemusha.mint_authorization_context.operation_id",
                self.operation_id,
            ),
            (
                "kagemusha.mint_authorization_context.release_id",
                self.release_id,
            ),
            (
                "kagemusha.mint_authorization_context.suite_id",
                self.suite_id,
            ),
            (
                "kagemusha.mint_authorization_context.vk_digest",
                self.vk_digest,
            ),
            (
                "kagemusha.mint_authorization_context.artifact_manifest_digest",
                self.artifact_manifest_digest,
            ),
            (
                "kagemusha.mint_authorization_context.liability_pool_id",
                self.liability_pool_id,
            ),
            (
                "kagemusha.mint_authorization_context.hardware_credential_id",
                self.hardware_credential_id,
            ),
            (
                "kagemusha.mint_authorization_context.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "kagemusha.mint_authorization_context.recipient_credential_commitment",
                self.recipient_credential_commitment,
            ),
            (
                "kagemusha.mint_authorization_context.credit_commitment",
                self.credit_commitment,
            ),
            (
                "kagemusha.mint_authorization_context.recipient_one_time_key",
                self.recipient_one_time_key,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "kagemusha.mint_authorization_context.recipient_one_time_key",
            self.recipient_one_time_key,
        )?;
        if self.policy_epoch == 0
            || self.liability_pool_id
                != kagemusha_liability_pool_id_v1(
                    &self.network_id,
                    &self.asset,
                    self.asset_incarnation,
                )?
        {
            return Err(invalid("kagemusha.mint_authorization_context.binding"));
        }
        Ok(())
    }

    /// Return the pre-ID digest included in issuance and credit identities.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_CONTEXT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaMintAuthorizationStatementV1 {
    /// Validate the complete post-encryption statement without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, version, identifier, or ciphertext binding.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.context.version != self.version {
            return Err(invalid("kagemusha.mint_authorization_statement.version"));
        }
        self.context.validate_shape()?;
        for (field, value) in [
            (
                "kagemusha.mint_authorization_statement.issuance_commitment",
                self.issuance_commitment,
            ),
            (
                "kagemusha.mint_authorization_statement.credit_id",
                self.credit_id,
            ),
            (
                "kagemusha.mint_authorization_statement.ciphertext_digest",
                self.ciphertext_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        Ok(())
    }

    /// Return the semantic digest constrained by both authorization proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN, self)
    }

    /// Return the exact mint associated data authenticated by the credit envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when this authorization statement is invalid.
    pub fn encrypted_credit_aad(
        &self,
    ) -> Result<KagemushaEncryptedCreditAadV1, KagemushaValidationErrorV1> {
        KagemushaEncryptedCreditAadV1::for_mint(self)
    }
}

impl KagemushaMintAuthorizationV1 {
    /// Validate proof framing and exact statement binding without granting authority.
    ///
    /// Core must resolve the named release, profile, suite, verifying keys, and
    /// artifact manifest from authenticated state and cryptographically verify
    /// both proof parities before mutating payer balance or pooled reserve.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid statement/proof binding or encoded size.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("kagemusha.mint_authorization.version"));
        }
        let semantic_digest = self.statement.canonical_digest()?;
        self.proof
            .validate_shape_for_semantic_digest(semantic_digest)?;
        require_encoded_size(self, KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the digest recursively bound by the finalized mint helper.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_DIGEST_DOMAIN, self)
    }

    /// Encode one shape-validated mint authorization as `kgm1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            KAGEMUSHA_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical mint authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, text framing, canonical bytes, or shape.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            KAGEMUSHA_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode one exact bounded mint authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let authorization: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1)?;
        authorization.validate_shape()?;
        Ok(authorization)
    }
}

impl KagemushaMintCreditStatementV1 {
    fn lifecycle_context_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(
            MINT_LIFECYCLE_CONTEXT_DOMAIN,
            &MintLifecycleContextPreimageV1 {
                version: self.lifecycle.version,
                network_id: self.lifecycle.network_id,
                protocol_version: self.lifecycle.protocol_version,
                suite_id: self.lifecycle.suite_id,
                vk_digest: self.lifecycle.vk_digest,
                release_id: self.lifecycle.release_id,
                asset: self.lifecycle.asset.clone(),
                asset_incarnation: self.lifecycle.asset_incarnation,
                scale: self.lifecycle.scale,
                liability_pool_id: self.lifecycle.liability_pool_id,
                hardware_profile_id: self.lifecycle.hardware_profile_id,
                policy_epoch: self.lifecycle.policy_epoch,
                operation_kind: self.lifecycle.operation_kind,
            },
        )
    }

    fn credit_id_preimage(&self) -> Result<MintCreditIdPreimageV1, KagemushaValidationErrorV1> {
        Ok(MintCreditIdPreimageV1 {
            lifecycle_context_digest: self.lifecycle_context_digest()?,
            recipient_credential_commitment: self.recipient_credential_commitment,
            authorization_context_digest: self.authorization_context_digest,
            amount: self.amount,
            issuance_commitment: self.issuance_commitment,
            recipient: self.recipient.clone(),
            credit_commitment: self.credit_commitment,
        })
    }

    /// Compute the unique mint-credit identity from committed issuance and output bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_credit_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(MINT_CREDIT_ID_DOMAIN, &self.credit_id_preimage()?)
    }

    /// Populate the canonical mint-credit identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_credit_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.lifecycle.credit_id = self.expected_credit_id()?;
        Ok(self)
    }

    /// Validate a public committed-liability mint statement's complete shape.
    ///
    /// This does not authenticate the committed credential, release, or proof.
    /// Core must match the statement to the finalized authenticated top-up and
    /// verify the release-pinned helper proof before monetary admission.
    ///
    /// # Errors
    ///
    /// Returns an error when any issuance, recipient, or output binding is invalid.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.lifecycle.version != self.version
            || self.amount == 0
            || self.minted_at_ms == 0
        {
            return Err(invalid("kagemusha.mint_statement.header"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "kagemusha.mint_statement.recipient_credential_commitment",
                self.recipient_credential_commitment,
            ),
            (
                "kagemusha.mint_statement.authorization_context_digest",
                self.authorization_context_digest,
            ),
            (
                "kagemusha.mint_statement.mint_authorization_digest",
                self.mint_authorization_digest,
            ),
            (
                "kagemusha.mint_statement.issuance_commitment",
                self.issuance_commitment,
            ),
            (
                "kagemusha.mint_statement.credit_commitment",
                self.credit_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.lifecycle.operation_kind != KagemushaOperationKindV1::MintFold
            || self.lifecycle.credit_id != self.expected_credit_id()?
        {
            return Err(invalid("kagemusha.mint_statement.credit_id"));
        }
        Ok(())
    }

    /// Return the mint statement digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaMintCreditV1 {
    /// Encode this shape-validated mint credit as canonical `kgm1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
            KAGEMUSHA_MINT_CREDIT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` mint credit.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, prefix, padding, base64url, Norito, or credit data.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
            KAGEMUSHA_MINT_CREDIT_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode and validate one exact bounded top-up mint credit.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid credit.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let credit: Self = decode_bounded_canonical(bytes, KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1)?;
        credit.validate_shape()?;
        Ok(credit)
    }

    /// Validate committed-liability, proof framing, recipient opening, and
    /// release-binding shape.
    ///
    /// This does not cryptographically verify either proof parity or
    /// authenticate the release/profile. Core must do both before folding.
    ///
    /// # Errors
    ///
    /// Returns an error when any mint-credit invariant fails.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("kagemusha.mint_credit.version"));
        }
        self.statement.validate_shape()?;
        self.proof
            .validate_shape_for_semantic_digest(self.statement.canonical_digest()?)?;
        for (field, value) in [
            (
                "kagemusha.mint_credit.finality_certificate_binding",
                self.finality_certificate_binding,
            ),
            (
                "kagemusha.mint_credit.finality_authority_head",
                self.finality_authority_head,
            ),
            (
                "kagemusha.mint_credit.finality_genesis_roster_id",
                self.finality_genesis_roster_id,
            ),
            (
                "kagemusha.mint_credit.finality_proof_binding_digest",
                self.finality_proof_binding_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact(&self.encrypted_credit)?;
        if self.statement.lifecycle.ciphertext_digest
            != kagemusha_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("kagemusha.mint_credit.encrypted_credit"));
        }
        require_nonzero(
            "kagemusha.mint_credit.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Validate a mint credit against the exact pre-debit recipient authorization.
    ///
    /// This remains a shape and digest-binding check. Core must authenticate the
    /// release and cryptographically verify the authorization and mint helper
    /// proofs before debit, reserve mutation, or folding.
    ///
    /// # Errors
    ///
    /// Returns an error for any substituted authorization, recipient key,
    /// ciphertext, release, asset, credential, or amount binding.
    pub fn validate_shape_against_authorization(
        &self,
        authorization: &KagemushaMintAuthorizationV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        authorization.validate_shape()?;
        let context = &authorization.statement.context;
        if self.statement.authorization_context_digest != context.canonical_digest()?
            || self.statement.mint_authorization_digest != authorization.canonical_digest()?
            || self.statement.issuance_commitment != authorization.statement.issuance_commitment
            || self.statement.lifecycle.credit_id != authorization.statement.credit_id
            || self.statement.lifecycle.ciphertext_digest
                != authorization.statement.ciphertext_digest
            || self.statement.amount != context.amount
            || self.statement.recipient != context.recipient
            || self.statement.recipient_credential_commitment
                != context.recipient_credential_commitment
            || self.statement.credit_commitment != context.credit_commitment
            || self.statement.lifecycle.release_id != context.release_id
            || self.statement.lifecycle.suite_id != context.suite_id
            || self.statement.lifecycle.vk_digest != context.vk_digest
            || self.statement.lifecycle.network_id != context.network_id
            || self.statement.lifecycle.asset != context.asset
            || self.statement.lifecycle.asset_incarnation != context.asset_incarnation
            || self.statement.lifecycle.scale != context.scale
            || self.statement.lifecycle.liability_pool_id != context.liability_pool_id
            || self.statement.lifecycle.hardware_profile_id != context.hardware_profile_id
            || self.statement.lifecycle.policy_epoch != context.policy_epoch
            || self.artifact_manifest_digest != context.artifact_manifest_digest
            || authorization.statement.ciphertext_digest
                != kagemusha_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("kagemusha.mint_credit.authorization_binding"));
        }
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            context.recipient_one_time_key,
        )?;
        authorization.statement.encrypted_credit_aad()?;
        Ok(())
    }
}

impl KagemushaRedemptionStatementV1 {
    fn redemption_id_preimage(&self) -> Result<RedemptionIdPreimageV1, KagemushaValidationErrorV1> {
        Ok(RedemptionIdPreimageV1 {
            lifecycle_binding_digest: self.lifecycle.canonical_digest()?,
            terminal_nullifier: self.terminal_nullifier,
            amount: self.amount,
            beneficiary: self.beneficiary.clone(),
            redemption_commitment: self.redemption_commitment,
        })
    }

    /// Compute the identity of this exact redemption output.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_redemption_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(REDEMPTION_ID_DOMAIN, &self.redemption_id_preimage()?)
    }

    /// Populate the canonical redemption identity.
    ///
    /// # Errors
    ///
    /// Returns an error when identity hashing fails.
    pub fn seal_redemption_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.redemption_id = self.expected_redemption_id()?;
        Ok(self)
    }

    /// Return the unlinkable circuit nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error only when the reserved all-zero value is present.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        require_nonzero(
            "kagemusha.redemption_statement.terminal_nullifier",
            self.terminal_nullifier,
        )?;
        Ok(self.terminal_nullifier)
    }

    /// Validate an unlinkable terminal aggregate-state transition.
    ///
    /// # Errors
    ///
    /// Returns an error when any lifecycle, nullifier, output, or commit binding is invalid.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.lifecycle.version != self.version {
            return Err(invalid("kagemusha.redemption_statement.version"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "kagemusha.redemption_statement.terminal_nullifier",
                self.terminal_nullifier,
            ),
            (
                "kagemusha.redemption_statement.redemption_commitment",
                self.redemption_commitment,
            ),
            (
                "kagemusha.redemption_statement.redemption_id",
                self.redemption_id,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.amount == 0
            || self.lifecycle.operation_kind != KagemushaOperationKindV1::RedeemSplit
            || self.terminal_nullifier == self.redemption_commitment
            || self.terminal_nullifier == self.redemption_id
            || self.redemption_commitment == self.redemption_id
        {
            return Err(invalid("kagemusha.redemption_statement.operation"));
        }
        self.commit_evidence.validate()?;
        if self.redemption_id != self.expected_redemption_id()? {
            return Err(invalid("kagemusha.redemption_statement.redemption_id"));
        }
        Ok(())
    }

    /// Return the redemption semantic digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(REDEMPTION_STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaRedemptionVoucherV1 {
    /// Encode this shape-validated voucher as canonical `kgm1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1,
            KAGEMUSHA_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` redemption voucher.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, prefix, padding, base64url, Norito, or voucher data.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1,
            KAGEMUSHA_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode and validate one exact bounded redemption voucher.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid voucher.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let voucher: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        voucher.validate_shape()?;
        Ok(voucher)
    }

    /// Validate terminal state consumption, certificate, and redemption-proof binding.
    ///
    /// Global redemption admission must additionally reject a previously seen
    /// `terminal_nullifier`.
    ///
    /// # Errors
    ///
    /// Returns an error when any voucher invariant fails.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("kagemusha.redemption_voucher.version"));
        }
        self.statement.validate_shape()?;
        self.commit_certificate.validate_against(
            &self.statement.lifecycle,
            self.statement.commit_evidence,
            self.statement.terminal_nullifier,
        )?;
        let certificate_digest = self.commit_certificate.canonical_digest_against(
            &self.statement.lifecycle,
            self.statement.commit_evidence,
            self.statement.terminal_nullifier,
        )?;
        self.proof.validate_shape_against(
            self.statement.canonical_digest()?,
            self.commit_certificate.candidate_envelope_digest,
            certificate_digest,
        )?;
        require_nonzero(
            "kagemusha.redemption_voucher.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        Ok(())
    }
}
