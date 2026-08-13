impl TransactionBuilder {
    fn validate_payload(
        payload: &TransactionPayload,
        construction: TransactionConstruction,
    ) -> Result<(), TransactionSignatureError> {
        match (construction, payload.domain) {
            (TransactionConstruction::Ordinary, TransactionDomain::Genesis) => {
                return Err(TransactionSignatureError::GenesisDomainNotAllowed);
            }
            (TransactionConstruction::Genesis, TransactionDomain::Network(_)) => {
                return Err(TransactionSignatureError::GenesisDomainRequired);
            }
            _ => {}
        }
        if payload.time_to_live_ms.is_none() {
            return Err(TransactionSignatureError::MissingTimeToLive);
        }
        payload
            .validate_fee_payment_intent()
            .map_err(|err| TransactionSignatureError::InvalidFeePaymentIntent(err.to_string()))
    }
    /// Reconstruct a transaction builder from one exact unsigned payload.
    ///
    /// The payload retains its signature-bound proof attachments. Only the
    /// authorization-proof bundle starts empty.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload's fee intent or metadata violates the
    /// canonical signature-bound fee policy, or when an ordinary payload uses
    /// the genesis-only transaction domain.
    pub fn from_payload(payload: TransactionPayload) -> Result<Self, TransactionSignatureError> {
        Self::validate_payload(&payload, TransactionConstruction::Ordinary)?;
        Ok(Self {
            payload,
            multisig_signatures: None,
            construction: TransactionConstruction::Ordinary,
        })
    }
    /// Reconstruct an explicit genesis-only builder from one exact unsigned payload.
    ///
    /// This entry point is intentionally separate from [`Self::from_payload`]
    /// so ordinary quote, external-signing, and relay workflows cannot accept
    /// the genesis marker.
    ///
    /// # Errors
    ///
    /// Returns an error unless the payload carries the genesis transaction
    /// domain and satisfies the common signature-bound payload policy.
    pub fn from_genesis_payload(
        payload: TransactionPayload,
    ) -> Result<Self, TransactionSignatureError> {
        Self::validate_payload(&payload, TransactionConstruction::Genesis)?;
        Ok(Self {
            payload,
            multisig_signatures: None,
            construction: TransactionConstruction::Genesis,
        })
    }
    /// Consume the builder and return its exact unsigned payload.
    ///
    /// Proof attachments are part of the returned signature preimage.
    /// Multisig authorization proofs remain outside it.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload's fee intent or metadata violates the
    /// canonical signature-bound fee policy, or when an ordinary builder was
    /// changed to use the genesis-only transaction domain.
    pub fn into_payload(self) -> Result<TransactionPayload, TransactionSignatureError> {
        self.validate_payload_state()?;
        Ok(self.payload)
    }
    fn decode_payload_for_construction(
        bytes: &[u8],
        construction: TransactionConstruction,
    ) -> Result<Self, norito::core::Error> {
        let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (payload, used) = TransactionPayload::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        let builder = Self {
            payload,
            multisig_signatures: None,
            construction,
        };
        builder
            .validate_payload_state()
            .map_err(|error| norito::core::Error::Message(error.to_string()))?;
        if builder.encode_payload() != bytes {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(builder)
    }
    /// Reconstruct a transaction builder from an exact canonical payload archive.
    ///
    /// This is the inverse of [`Self::encode_payload`] for external-signature
    /// workflows. Trailing bytes are rejected so callers cannot sign one payload
    /// while later submitting a different envelope suffix.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when `bytes` is malformed, non-canonical for the
    /// default V1 layout, contains trailing bytes, or carries the genesis-only
    /// transaction domain.
    pub fn decode_payload(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        Self::decode_payload_for_construction(bytes, TransactionConstruction::Ordinary)
    }
    /// Reconstruct an explicit genesis-only builder from an exact canonical payload archive.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when `bytes` is malformed, non-canonical for the
    /// default V1 layout, contains trailing bytes, or does not carry the
    /// genesis transaction domain.
    pub fn decode_genesis_payload(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        Self::decode_payload_for_construction(bytes, TransactionConstruction::Genesis)
    }
}
