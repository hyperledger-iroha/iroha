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
            (true, false, None) | (false, false, None) => Ok(None),
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
    /// the required KAGEMUSHA V1 CommitQC envelope.
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
    /// Borrow the canonical paired-Pasta CommitQC seal bundle payload when the certificate
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
            (true, false, None) | (false, false, None) => Ok(None),
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

impl TimeoutVote {
    /// Return the domain-separated canonical bytes authenticated by this
    /// timeout vote, excluding the signature itself.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = TimeoutVoteSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            round: self.round,
            highest_prepare_qc: self
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref),
        };
        signature_preimage(b"iroha:sumeragi:v2:timeout-vote", &payload.encode())
    }
    /// Validate context binding, high-QC reference, signer, and signature
    /// presence.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the timeout round or signer
    /// is invalid, the reported high certificate is not a valid non-future
    /// `PrepareQC` for the same context, or the signature is missing or
    /// oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_validator_index(self.signer, context)?;
        if let Some(highest) = &self.highest_prepare_qc {
            if highest.phase != GlobalPhase::Prepare {
                return Err(ValidationError::TimeoutCarriesNonPrepareQc);
            }
            if highest.round.context_id != self.round.context_id
                || highest.round.height != self.round.height
            {
                return Err(ValidationError::WrongHeightContext);
            }
            if highest.round.view > self.round.view {
                return Err(ValidationError::QcFromFutureView);
            }
            highest.validate(context)?;
        }
        require_signature(&self.signature)
    }
}

impl TimeoutCertificate {
    /// Return a stable reference to this timeout certificate.
    #[must_use]
    pub fn as_ref(&self) -> TimeoutCertificateRef {
        TimeoutCertificateRef {
            round: self.round,
            highest_prepare_qc: self.highest_prepare_qc().map(QuorumCertificate::as_ref),
            certificate_hash: HashOf::new(self),
        }
    }
    /// Select the highest reported `PrepareQC` deterministically.
    ///
    /// View is the primary ordering key. The semantic certificate reference
    /// breaks impossible conflicting-subject ties without depending on which
    /// valid quorum subset happened to be aggregated.
    #[must_use]
    pub fn highest_prepare_qc(&self) -> Option<&QuorumCertificate> {
        self.groups
            .iter()
            .filter_map(|group| group.highest_prepare_qc.as_ref())
            .max_by(|left, right| {
                left.round
                    .view
                    .cmp(&right.round.view)
                    .then_with(|| left.as_ref().cmp(&right.as_ref()))
            })
    }
    /// Validate grouping, disjoint signers, context binding, and equal-vote quorum.
    ///
    /// Cryptographic aggregate-signature verification remains the caller's
    /// responsibility.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error if the timeout certificate cannot
    /// be valid under `context`.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        if self.groups.is_empty() {
            return Err(ValidationError::EmptyTimeoutCertificate);
        }
        if self.groups.windows(2).any(|pair| {
            pair[0]
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref)
                >= pair[1]
                    .highest_prepare_qc
                    .as_ref()
                    .map(QuorumCertificate::as_ref)
        }) {
            return Err(ValidationError::TimeoutGroupsNotStrictlySorted);
        }
        let mut all_signers = BTreeSet::new();
        let mut highest_at_view: Option<(View, BlockSubject, ExecutionCommitment)> = None;
        for group in &self.groups {
            if group.signers.is_empty() {
                return Err(ValidationError::EmptyTimeoutGroup);
            }
            require_aggregate_signature(&group.aggregate_signature)?;
            if group.signers.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ValidationError::SignersNotStrictlySorted);
            }
            if let Some(highest) = &group.highest_prepare_qc {
                if highest.phase != GlobalPhase::Prepare {
                    return Err(ValidationError::TimeoutCarriesNonPrepareQc);
                }
                if highest.round.context_id != self.round.context_id
                    || highest.round.height != self.round.height
                {
                    return Err(ValidationError::WrongHeightContext);
                }
                if highest.round.view > self.round.view {
                    return Err(ValidationError::QcFromFutureView);
                }
                highest.validate(context)?;
                match highest_at_view {
                    Some((view, subject, execution_commitment)) if view == highest.round.view => {
                        if subject != highest.subject
                            || execution_commitment != highest.execution_commitment
                        {
                            return Err(ValidationError::ConflictingHighestPrepare);
                        }
                    }
                    Some((view, _, _)) if view > highest.round.view => {
                        return Err(ValidationError::TimeoutGroupsNotStrictlySorted);
                    }
                    _ => {
                        highest_at_view = Some((
                            highest.round.view,
                            highest.subject,
                            highest.execution_commitment,
                        ));
                    }
                }
            }
            for signer in &group.signers {
                if !all_signers.insert(*signer) {
                    return Err(ValidationError::OverlappingTimeoutSigners);
                }
            }
        }
        let all_signers: Vec<_> = all_signers.into_iter().collect();
        context.validate_certificate_signers(&all_signers)
    }
}

impl Proposal {
    /// Return the domain-separated canonical bytes authenticated by the
    /// expected leader, excluding the signature itself.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        signature_preimage(b"iroha:sumeragi:v2:proposal", &unsigned.encode())
    }
    /// Validate the complete structural proposal contract against a frozen
    /// height context.
    ///
    /// Cryptographic verification remains the authenticated-ingress adapter's
    /// responsibility and must use [`Self::signature_preimage`].
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the proposal, manifest,
    /// leader, parent/timeout justification, or signature is not valid under
    /// the frozen height context.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        self.manifest.validate(context)?;
        if self.manifest.round != self.round || self.manifest.subject != self.subject {
            return Err(ValidationError::ProposalManifestMismatch);
        }
        validate_validator_index(self.proposer, context)?;
        if self.proposer != context.leader(self.round.view) {
            return Err(ValidationError::WrongProposer);
        }
        match &self.justification {
            ProposalJustification::ParentCommit(parent) => {
                let same_finalized_parent = match (
                    parent.certificate.as_ref(),
                    context.parent_commit_qc.as_ref(),
                ) {
                    (None, None) => true,
                    (Some(carried), Some(frozen)) => {
                        // A subject can acquire valid CommitQCs in more than
                        // one same-round certificate before or after an
                        // unchanged re-proposal. Context identity deliberately
                        // ignores that round and the signer evidence, so
                        // view-zero admission uses the semantic decision key.
                        // The previous roster is unavailable here, but all
                        // context-independent certificate shape checks remain
                        // mandatory before authenticated ingress verifies it.
                        carried.proposal_round == carried.round
                            && carried.round.height.checked_add(1) == Some(context.height)
                            && carried.execution_commitment.validate().is_ok()
                            && !carried.signers.is_empty()
                            && carried.signers.len() <= MAX_VALIDATORS_PER_HEIGHT
                            && carried.signers.windows(2).all(|pair| pair[0] < pair[1])
                            && require_aggregate_signature(&carried.aggregate_signature).is_ok()
                            && carried.as_ref().same_commit_decision(frozen.as_ref())
                    }
                    (None, Some(_)) | (Some(_), None) => false,
                };
                if self.round.view != 0 || !same_finalized_parent {
                    return Err(ValidationError::InvalidProposalJustification);
                }
            }
            ProposalJustification::Timeout(timeout) => {
                if self.round.view == 0
                    || timeout.timeout_certificate.round.context_id != self.round.context_id
                    || timeout.timeout_certificate.round.height != self.round.height
                    || timeout.timeout_certificate.round.view.checked_add(1)
                        != Some(self.round.view)
                {
                    return Err(ValidationError::InvalidProposalJustification);
                }
                timeout.timeout_certificate.validate(context)?;
                let selected_highest = timeout.timeout_certificate.highest_prepare_qc();
                if selected_highest != timeout.highest_prepare_qc.as_ref()
                    || selected_highest.is_some_and(|highest| highest.subject != self.subject)
                {
                    return Err(ValidationError::InvalidProposalJustification);
                }
            }
        }
        require_signature(&self.signature)
    }
}

impl CertifiedBodyRequest {
    /// Return the canonical request bytes authenticated by the requester.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        signature_preimage(
            b"iroha:sumeragi:v2:certified-body-request",
            &unsigned.encode(),
        )
    }
    /// Validate context, certificate, requester, and signature presence.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the requested round or
    /// certificate is invalid under `context`, the certificate identifies a
    /// different proposal, or the requester signature is missing or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        self.certificate.validate(context)?;
        if self.certificate.proposal_round != self.round || self.certificate.subject != self.subject
        {
            return Err(ValidationError::CertifiedBodyCertificateMismatch);
        }
        require_signature(&self.signature)
    }
}

impl CertifiedBodyResponse {
    /// Return the canonical response bytes authenticated by the responder.
    ///
    /// The body is represented by its payload hash in the signed payload so
    /// implementations need not duplicate large bytes during signing.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = CertifiedBodyResponseSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            request_hash: self.request_hash,
            manifest: self.manifest.clone(),
            body_hash: Hash::new(&self.body),
            responder: self.responder.clone(),
        };
        signature_preimage(
            b"iroha:sumeragi:v2:certified-body-response",
            &payload.encode(),
        )
    }
    /// Validate the response against the frozen context and signature
    /// presence. The caller additionally matches `request_hash` to an
    /// outstanding authenticated request.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the manifest is invalid,
    /// the body hash or length differs from the manifest, the responder is
    /// malformed, or the response signature is missing or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        self.manifest.validate(context)?;
        if Hash::new(&self.body) != self.manifest.subject.payload_hash {
            return Err(ValidationError::CertifiedBodyHashMismatch);
        }
        if u64::try_from(self.body.len()).ok() != Some(self.manifest.payload_size_bytes) {
            return Err(ValidationError::PayloadSizeMismatch);
        }
        require_signature(&self.signature)
    }
    /// Validate this response against the exact outstanding request and the
    /// authenticated outer transport sender.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is replayed across requests, changes
    /// round/subject, or the claimed current responder differs from the
    /// authenticated transport sender.
    ///
    /// The responder need not be one of the request QC signers. Historical
    /// archive service is safe because the exact request carries the verified
    /// QC while the response body and manifest are hash-bound to that QC's
    /// subject. The serving path additionally proves that the responder has
    /// the canonical applied block in durable storage.
    pub fn validate_against(
        &self,
        context: &HeightContext,
        request: &CertifiedBodyRequest,
        authenticated_sender: &PeerId,
    ) -> Result<(), ValidationError> {
        request.validate(context)?;
        self.validate(context)?;
        if self.request_hash != HashOf::new(request)
            || self.manifest.round != request.round
            || self.manifest.subject != request.subject
        {
            return Err(ValidationError::CertifiedBodyRequestMismatch);
        }
        if &self.responder != authenticated_sender {
            return Err(ValidationError::ResponderIdentityMismatch);
        }
        Ok(())
    }
}

impl CommitCertificateRequest {
    /// Return the canonical bytes authenticated by the requester.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        signature_preimage(
            b"iroha:sumeragi:v2:commit-certificate-request",
            &unsigned.encode(),
        )
    }
    /// Validate the request against the one active frozen context.
    ///
    /// Cryptographic signature and outer-transport identity verification are
    /// performed by the transport adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when the context itself is invalid, the request uses
    /// another protocol, chain, context, or height, or its signature is missing
    /// or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        context.validate()?;
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.network_id != context.network_id
            || self.context_id != context.id()
            || self.height != context.height
        {
            return Err(ValidationError::WrongHeightContext);
        }
        require_signature(&self.signature)
    }
}

impl CommitCertificateResponse {
    /// Return the canonical bytes authenticated by the responder.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = CommitCertificateResponseSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            request_hash: self.request_hash,
            certificate: self.certificate.clone(),
            responder: self.responder.clone(),
        };
        signature_preimage(
            b"iroha:sumeragi:v2:commit-certificate-response",
            &payload.encode(),
        )
    }
    /// Validate the certificate and response structure against one context.
    ///
    /// Cryptographic aggregate and responder signatures are verified by the
    /// transport and consensus adapters respectively.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the certificate is invalid,
    /// is not a `CommitQC` for `context`, or the response signature is missing
    /// or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        self.certificate.validate(context)?;
        if self.certificate.phase != GlobalPhase::Commit
            || self.certificate.round.context_id != context.id()
            || self.certificate.round.height != context.height
        {
            return Err(ValidationError::CommitCertificateMismatch);
        }
        require_signature(&self.signature)
    }
    /// Validate the response against the exact outstanding request.
    ///
    /// # Errors
    ///
    /// Returns an error when either artifact is invalid under `context` or the
    /// response does not carry the hash of the exact signed request.
    pub fn validate_against(
        &self,
        context: &HeightContext,
        request: &CommitCertificateRequest,
    ) -> Result<(), ValidationError> {
        request.validate(context)?;
        self.validate(context)?;
        if self.request_hash != HashOf::new(request) {
            return Err(ValidationError::CommitCertificateRequestMismatch);
        }
        Ok(())
    }
}

impl GlobalBeaconPartialSignature {
    /// Validate the round and one-based DKG signer seat against a frozen context.
    ///
    /// Cryptographic proof verification is deliberately performed by the
    /// threshold-beacon reducer after it reconstructs the exact pulse payload.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error for another height context or an
    /// out-of-range signer seat.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        let zero_based = self
            .partial
            .signer_index
            .checked_sub(1)
            .ok_or(ValidationError::SignerOutOfRange)?;
        if usize::from(zero_based) >= context.roster.len() {
            return Err(ValidationError::SignerOutOfRange);
        }
        Ok(())
    }
}

impl ConsensusMessageV2 {
    /// Wrap a v2 payload with the canonical protocol version.
    #[must_use]
    pub const fn new(payload: ConsensusMessageV2Payload) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            payload,
        }
    }
    /// Reject envelopes from any other consensus wire version.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::UnsupportedProtocolVersion`] when the
    /// explicit version is not v2.
    pub fn validate_version(&self) -> Result<(), ValidationError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        Ok(())
    }
}
