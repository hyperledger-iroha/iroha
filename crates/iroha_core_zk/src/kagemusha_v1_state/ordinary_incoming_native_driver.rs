//! Reachable closed Native incoming proof/reservation/approval/commit lifecycle.
//! The platform signs only the actual retained W. Proof profiles/storage are public data;
//! every capability is created by the real Main/source/verifier and globally acknowledged CAS.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaArtifactByteResolverV1, KagemushaProductionProverV1,
    KagemushaRecursiveVerifierProfileV1, assemble_ordinary_incoming_commit_v1,
    generate_ordinary_incoming_candidate_v1, verify_ordinary_incoming_preparation_guard_v1,
    verify_ordinary_incoming_terminal_guard_v1,
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaHardwarePlatformClassV1,
};
use zeroize::Zeroize as _;

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Select actual finalized Mint source under the already retained genuine pre-debit owner.
    /// Offered finality/credit bytes are independently authenticated; they never create funds.
    /// # Errors
    /// Rejects absent prior Mint custody, stale FI, forged source or incompatible current head.
    pub fn prepare_finalized_incoming_mint_platform(
        &mut self,
        finalized_original: &[u8],
        credit_original: &[u8],
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.reserve_incoming_mint()?;
        self.prepare_incoming_mint_approval(finalized_original, credit_original)?;
        self.incoming_platform_fields(false)
    }

    /// Produce actual Guard, source folds and State, then fsync the full pre-Reserve row.
    /// A retained row is independently reverified and reused without regenerating proof/nonce.
    /// # Errors
    /// Rejects unavailable qualified proof artifacts, changed source/custody or insufficient quota.
    pub fn prove_retained_incoming_mint_reservation<R: KagemushaArtifactByteResolverV1 + Clone>(
        &mut self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if self.incoming_reservation_candidate.is_some() {
            return Ok(Sha256::digest(self.incoming_reservation_proof_original()?).into());
        }
        let selection = self.captured_incoming_approval()?;
        if selection.transition_statement()?.kind != KagemushaTransitionKindV1::MintFold {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let prover = KagemushaProductionProverV1::load_ordinary_incoming(
            &selection,
            profile.clone(),
            resolver.clone(),
        )
        .map_err(material)?;
        let guard_raw = prover
            .prove_ordinary_incoming_preparation_guard(&selection)
            .map_err(material)?;
        let guard = verify_ordinary_incoming_preparation_guard_v1(&selection, &guard_raw)?;
        let auxiliaries = prover
            .prepare_ordinary_incoming_auxiliaries(&selection, &guard)
            .map_err(material)?;
        let generated = generate_ordinary_incoming_candidate_v1(
            &selection,
            &guard,
            profile,
            resolver,
            &auxiliaries,
        )
        .map_err(material)?;
        let (candidate, mut duplicate, mut checkpoint) = generated.into_parts();
        // The candidate retains its authenticated canonical private checkpoint. Do not leave
        // the duplicate generated private carrier columns/proof bytes in disposable memory.
        duplicate.eq_public_instances.fill(Default::default());
        duplicate.ep_public_instances.fill(Default::default());
        duplicate
            .eq_transport_public_instances
            .fill(Default::default());
        duplicate
            .ep_transport_public_instances
            .fill(Default::default());
        duplicate.eq_inner_proof.zeroize();
        duplicate.ep_inner_proof.zeroize();
        checkpoint.zeroize();
        self.retain_incoming_reservation_candidate(candidate, guard)
    }

    /// Actual Native W/S and enrolled credential projections; no constructor is exposed.
    /// # Errors
    /// Rejects unknown stage, expired uninvoked W or changed actual owner/custody.
    pub fn incoming_platform_fields(
        &self,
        terminal: bool,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (challenge, _, _, _) = self.incoming_platform_state(terminal)?;
        let fields = vec![
            challenge.operation_id.to_vec(),
            challenge.canonical_signing_bytes().map_err(material)?,
            challenge
                .canonical_subject_signing_bytes()
                .map_err(material)?,
            self.publication
                .cash_financial()
                .enrollment()
                .certificate()
                .canonical_bytes()
                .map_err(material)?,
        ];
        self.incoming_platform_state(terminal)?;
        Ok(fields)
    }
    /// Fsync actual one-use platform fence before authorizing exactly one OS call.
    /// # Errors
    /// An uncertain fence never authorizes another call; exact retained evidence is recovered.
    pub fn fence_incoming_platform(
        &mut self,
        terminal: bool,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (challenge, fenced, original, _) = self.incoming_platform_state(terminal)?;
        if original.is_some() {
            return self.recover_incoming_platform(terminal);
        }
        if fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if terminal {
            self.fence_incoming_terminal_platform(challenge.operation_id)?;
        } else {
            self.fence_incoming_approval_platform()?;
        }
        Ok(vec![vec![0], vec![], vec![]])
    }
    /// Wrap DER/CBOR only in Native-held W and authenticate under actual C/PI before fsync.
    /// # Errors
    /// Rejects a different retry original, unknown fence or substituted/expired evidence.
    pub fn retain_incoming_platform_original(
        &mut self,
        terminal: bool,
        raw: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        if raw.is_empty() || raw.len() > 4096 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let (challenge, fenced, old, captured) = self.incoming_platform_state(terminal)?;
        if !fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let evidence = match self
            .publication
            .cash_financial()
            .enrollment()
            .app_credential()
            .subject()
            .platform_class
        {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: raw.to_vec(),
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                    raw_assertion: raw.to_vec(),
                }
            }
            _ => return Err(KagemushaStateErrorV1::InvalidHardwareProfile),
        };
        let original = norito::encode_canonical(&KagemushaAppOperationApprovalV1 {
            challenge,
            evidence,
        })
        .map_err(material)?;
        if let Some(old) = old {
            if old != original {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if !captured {
                if terminal {
                    self.acknowledge_incoming_terminal_capture(challenge.operation_id)?;
                } else {
                    self.acknowledge_incoming_approval_capture()?;
                }
            }
        } else if terminal {
            self.capture_incoming_terminal_original(challenge.operation_id, &original)?;
        } else {
            self.capture_incoming_approval_original(&original)?;
        }
        self.incoming_platform_state(terminal)?;
        Ok(Sha256::digest(raw).into())
    }
    /// Recover exact signed evidence/full canonical original; unknown invocation remains closed.
    /// # Errors
    /// Refuses unretained fenced outcomes instead of selecting a new operation or signature.
    pub fn recover_incoming_platform(
        &self,
        terminal: bool,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (challenge, fenced, original, captured) = self.incoming_platform_state(terminal)?;
        let Some(original) = original else {
            if fenced {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            return Ok(vec![vec![0], vec![], vec![]]);
        };
        let value: KagemushaAppOperationApprovalV1 = norito::decode_canonical_with_limits(
            &original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(material)?;
        if norito::encode_canonical(&value).map_err(material)? != original
            || value.challenge != challenge
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let raw = match value.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                signature_der
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                raw_assertion
            }
        };
        Ok(vec![vec![if captured { 2 } else { 1 }], raw, original])
    }
    fn incoming_platform_state(
        &self,
        terminal: bool,
    ) -> Result<
        (
            KagemushaAppOperationApprovalChallengeV1,
            bool,
            Option<Vec<u8>>,
            bool,
        ),
        KagemushaStateErrorV1,
    > {
        if terminal {
            self.incoming_terminal_platform_state()
        } else {
            self.incoming_preparation_platform_state()
        }
    }

    /// Prove actual fresh W1 Guard and full Commit from the same acknowledged reservation.
    /// Full generated cap/checkpoint/service original is fsynced before Commit account dispatch.
    /// # Errors
    /// Rejects unavailable qualified keys, changed clocks/FI/source/reservation or insufficient quota.
    pub fn prove_retained_incoming_mint_commit<R: KagemushaArtifactByteResolverV1>(
        &mut self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(digest) = self.retained_incoming_commit_digest()? {
            return Ok(digest);
        }
        let selection = self.captured_incoming_terminal()?;
        if selection
            .preparation_selection()?
            .transition_statement()?
            .kind
            != KagemushaTransitionKindV1::MintFold
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let prover = KagemushaProductionProverV1::load_ordinary_incoming_terminal(
            &selection, profile, resolver,
        )
        .map_err(material)?;
        let raw = prover
            .prove_ordinary_incoming_terminal_guard(&selection)
            .map_err(material)?;
        let guard = verify_ordinary_incoming_terminal_guard_v1(&selection, &raw)?;
        let receipt = self
            .lineage_cas
            .incoming_reservation_receipt(
                selection.terminal_intent()?.reserve_request_original_sha256,
                self.publication.cash_financial(),
                selection.preparation_selection()?.reservation()?,
            )
            .map_err(material)?;
        let generated =
            assemble_ordinary_incoming_commit_v1(&self.verifier, &selection, &guard, &receipt)
                .map_err(material)?;
        self.retain_generated_incoming_commit(generated)
    }
    /// Return exact retained Commit request and full portable proof for the protected transport.
    /// # Errors
    /// Refuses absent/changed proof custody or failure of genuine Native request retention.
    pub fn incoming_mint_commit_transport_originals(
        &mut self,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.retained_incoming_commit_transport_originals()
    }
    /// Apply only authentic durable globally acknowledged Commit; post-State fsync Ack stays separate.
    /// # Errors
    /// Refuses unknown/substituted global result, failed suffix durability or source retirement.
    pub fn advance_incoming_mint_commit(
        &mut self,
        request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.advance_acknowledged_incoming_commit(request_original_sha256)
    }
    /// Retry only the real post-State fsync Ack; no balance/key mutation is repeated.
    /// # Errors
    /// Refuses absent StateAdvance or unavailable authentic current FI/full signed clock.
    pub fn acknowledge_incoming_mint_state_advance(
        &mut self,
        request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.acknowledge_incoming_state_advance(request_original_sha256)
    }
}
