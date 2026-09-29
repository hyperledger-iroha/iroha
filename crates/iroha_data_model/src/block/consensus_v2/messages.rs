//! Consensus message validation and canonical signature preimages.

use super::*;

impl Vote {
    /// Return the domain-separated canonical bytes authenticated by this vote.
    ///
    /// The signature and signer fields are excluded so every signer of the
    /// same certificate signs the same BLS message. The authenticated-ingress
    /// adapter still selects the public key by `signer`, and the certificate
    /// binds its strictly ordered signer set, so a share cannot be reassigned
    /// to another key.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = VoteSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            round: self.round,
            proposal_round: self.proposal_round,
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
        };
        signature_preimage(b"iroha:sumeragi:v2:vote", &payload.encode())
    }
    /// Borrow the ordinary BLS signature from either its raw representation or
    /// the required KAGEMUSHA V1 Commit-vote envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when the signature framing is malformed or disagrees
    /// with the vote phase/top-up commitment.
    pub fn bls_signature(&self) -> Result<&[u8], ValidationError> {
        match self.kagemusha_finality_seal_payload()? {
            Some(_) => decode_kagemusha_consensus_signature_envelope_v1(&self.signature)?
                .map(|parts| parts.bls_signature)
                .ok_or(ValidationError::InvalidKagemushaSignatureEnvelope),
            None => Ok(&self.signature),
        }
    }
    /// Borrow the canonical paired-Pasta Commit-vote seal payload when this vote certifies a
    /// non-empty KAGEMUSHA V1 top-up root or carries an epoch-boundary roster rotation.
    ///
    /// # Errors
    ///
    /// Returns an error when a required envelope is absent, an envelope occurs
    /// on another vote kind, or its framing is malformed.
    pub fn kagemusha_finality_seal_payload(&self) -> Result<Option<&[u8]>, ValidationError> {
        let envelope = decode_kagemusha_consensus_signature_envelope_v1(&self.signature)?;
        let commit = self.phase == GlobalPhase::Commit;
        let required = commit && self.execution_commitment.kagemusha_top_up_count != 0;
        match (commit, required, envelope) {
            (true, _, Some(parts))
                if parts.kind == KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1 =>
            {
                Ok(Some(parts.auxiliary_payload))
            }
            (_, false, None) => Ok(None),
            _ => Err(ValidationError::InvalidKagemushaSignatureEnvelope),
        }
    }
    /// Validate the vote's context, signer, and signature presence.
    ///
    /// Cryptographic verification remains the authenticated-ingress adapter's
    /// responsibility and must use [`Self::signature_preimage`].
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the vote or proposal origin
    /// belongs to another context, its signer is outside the frozen roster,
    /// its execution commitment is invalid, or its signature is missing or
    /// oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_proposal_round(self.proposal_round, self.round, context)?;
        validate_validator_index(self.signer, context)?;
        self.execution_commitment.validate()?;
        require_signature(&self.signature)?;
        self.bls_signature().map(|_| ())
    }
}

impl QuorumCertificateRef {
    /// Return whether both references certify the same committed decision.
    ///
    /// `CommitQC`s for one immutable body may be assembled before or after an
    /// unchanged re-proposal. Their stable decision identity excludes the
    /// round and signer evidence while retaining context, height, subject, and
    /// deterministic execution.
    #[must_use]
    pub fn same_commit_decision(self, other: Self) -> bool {
        self.phase == GlobalPhase::Commit
            && other.phase == GlobalPhase::Commit
            && self.round.context_id == other.round.context_id
            && self.round.height == other.round.height
            && self.subject == other.subject
            && self.execution_commitment == other.execution_commitment
    }
}

impl QuorumCertificate {
    /// Return a stable reference to this certificate.
    #[must_use]
    pub fn as_ref(&self) -> QuorumCertificateRef {
        QuorumCertificateRef {
            round: self.round,
            proposal_round: self.proposal_round,
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
        }
    }
    /// Borrow the ordinary BLS aggregate from either its raw representation or
    /// the required KAGEMUSHA V1 `CommitQC` envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when the signature framing is malformed or disagrees
    /// with the certificate phase/top-up commitment.
    pub fn bls_aggregate_signature(&self) -> Result<&[u8], ValidationError> {
        match self.kagemusha_finality_seal_payload()? {
            Some(_) => decode_kagemusha_consensus_signature_envelope_v1(&self.aggregate_signature)?
                .map(|parts| parts.bls_signature)
                .ok_or(ValidationError::InvalidKagemushaSignatureEnvelope),
            None => Ok(&self.aggregate_signature),
        }
    }
    /// Borrow the canonical paired-Pasta `CommitQC` seal bundle payload when the certificate
    /// commits a non-empty KAGEMUSHA V1 top-up root or an epoch-boundary roster rotation.
    ///
    /// # Errors
    ///
    /// Returns an error when a required envelope is absent, an envelope occurs
    /// on another certificate kind, or its framing is malformed.
    pub fn kagemusha_finality_seal_payload(&self) -> Result<Option<&[u8]>, ValidationError> {
        let envelope = decode_kagemusha_consensus_signature_envelope_v1(&self.aggregate_signature)?;
        let commit = self.phase == GlobalPhase::Commit;
        let required = commit && self.execution_commitment.kagemusha_top_up_count != 0;
        match (commit, required, envelope) {
            (true, _, Some(parts))
                if parts.kind == KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1 =>
            {
                Ok(Some(parts.auxiliary_payload))
            }
            (_, false, None) => Ok(None),
            _ => Err(ValidationError::InvalidKagemushaSignatureEnvelope),
        }
    }
    /// Validate the certificate's context binding and equal-vote quorum.
    ///
    /// Cryptographic aggregate-signature verification remains the caller's
    /// responsibility.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error if the certificate cannot be
    /// valid under `context`.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_proposal_round(self.proposal_round, self.round, context)?;
        self.execution_commitment.validate()?;
        context.validate_certificate_signers(&self.signers)?;
        require_aggregate_signature(&self.aggregate_signature)?;
        self.bls_aggregate_signature().map(|_| ())
    }
    /// Reconstruct the canonical vote preimage for one certified signer.
    ///
    /// # Errors
    ///
    /// Returns an error when `signer` is not part of this certificate or is
    /// outside the frozen roster.
    pub fn signer_preimage(
        &self,
        context: &HeightContext,
        signer: ValidatorIndex,
    ) -> Result<Vec<u8>, ValidationError> {
        self.validate(context)?;
        if self.signers.binary_search(&signer).is_err() {
            return Err(ValidationError::SignerNotInCertificate);
        }
        Ok(Vote {
            round: self.round,
            proposal_round: self.proposal_round,
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
            signer,
            signature: Vec::new(),
        }
        .signature_preimage())
    }
}
