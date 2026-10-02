//! Closed Native outgoing proof/Reserve/W1/Commit/StateAdvance/outbox lifecycle.
//! Public method inputs are artifact data and exact transport originals only. Managed code cannot
//! offer a financial key, prover, clock, W, candidate, approval verdict or committed-State callback.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaArtifactByteResolverV1, KagemushaOrdinaryLineageOutgoingOriginalsV1,
    KagemushaProductionProverV1, KagemushaRecursiveVerifierProfileV1,
    generate_ordinary_outgoing_candidate_v1, verify_ordinary_preparation_guard_v1,
    verify_ordinary_terminal_guard_v1,
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaHardwarePlatformClassV1,
};
impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Actual W2 Guard and paired State/checkpoint are durably retained before Reserve dispatch.
    /// An exact retained candidate is independently readmitted and reused without generating proof.
    /// # Errors
    /// Refuses missing actual custody, qualified artifacts, genuine parent checkpoint or capacity.
    pub fn prove_retained_outgoing_reservation<R: KagemushaArtifactByteResolverV1 + Clone>(
        &mut self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if self.outgoing_reservation_candidate.is_some() {
            let (_, reservation) = self.retained_outgoing_reservation(profile, resolver)?;
            return Ok(Sha256::digest(reservation.proof_bundle_original()).into());
        }
        let guard = if self.outgoing_proof_operands.is_some() {
            self.retained_outgoing_guard()?
        } else {
            let selection = self.captured_preparation()?;
            let prover = KagemushaProductionProverV1::load_ordinary_cash(
                &selection,
                profile.clone(),
                resolver.clone(),
            )
            .map_err(material)?;
            let original = prover
                .prove_ordinary_preparation_guard(&selection)
                .map_err(material)?;
            let guard = verify_ordinary_preparation_guard_v1(&selection, &original)?;
            self.retain_outgoing_proof_operands(&guard)?;
            guard
        };
        let selection = self.captured_preparation()?;
        let prover = KagemushaProductionProverV1::load_ordinary_cash(
            &selection,
            profile.clone(),
            resolver.clone(),
        )
        .map_err(material)?;
        let auxiliaries = prover
            .prepare_ordinary_outgoing_auxiliaries(&selection, &guard)
            .map_err(material)?;
        let generated = generate_ordinary_outgoing_candidate_v1(
            &selection,
            &guard,
            profile.clone(),
            resolver.clone(),
            &auxiliaries,
        )
        .map_err(material)?;
        self.retain_outgoing_reservation_candidate(generated.into_candidate(), guard)?;
        let (_, reservation) = self.retained_outgoing_reservation(profile, resolver)?;
        Ok(Sha256::digest(reservation.proof_bundle_original()).into())
    }
    /// Native account signing is durably fenced, exact retries reuse same account signature/WAL.
    /// Returned full public originals are data, not independently reconstructed money authority.
    /// # Errors
    /// Refuses uncertain account invocation, stale FI, substituted proof or ambiguous acknowledgment.
    pub fn sign_outgoing_reservation_transport<R: KagemushaArtifactByteResolverV1>(
        &mut self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (_, reservation) = self.retained_outgoing_reservation(profile, resolver)?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let acknowledged = self
            .lineage_cas
            .acknowledged_outgoing_reservation_request(financial, &current, reservation.proof())
            .map_err(material)?;
        let (status, request, signature) = if let Some((request, signature)) = acknowledged {
            (2, request, signature.to_vec())
        } else {
            self.lineage_cas
                .reserve_transition(financial, &current, reservation.proof())
                .map_err(material)?;
            let fields = self.sign_retained_lineage_request(sign)?;
            match fields.as_slice() {
                [request, signature] => (0, request.clone(), signature.clone()),
                _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            }
        };
        let key: DigestV1 = Sha256::digest(&request).into();
        // The service independently verifies all four complete signed samples. A projected
        // context is not their carrier and cannot recreate its historical finality originals.
        let financial = self.publication.cash_financial();
        let clock = financial
            .retained_cash_clock_originals(reservation.preparation_clock_context())
            .map_err(material)?;
        financial
            .recheck_retained_cash_clock_originals(&clock)
            .map_err(material)?;
        let signed_clock_original = clock.canonical_original().to_vec();
        financial
            .recheck_retained_cash_clock_originals(&clock)
            .map_err(material)?;
        let fields = vec![
            vec![status],
            request,
            signature,
            reservation.proof_bundle_original().to_vec(),
            reservation.predecessor_public_state_original().to_vec(),
            reservation.neutral_reservation_original().to_vec(),
            signed_clock_original,
            key.to_vec(),
        ];
        self.require_current_financial_control()?;
        Ok(fields)
    }
    /// Select W1 only through authentic acknowledged Reserve of the same retained W2/candidate.
    /// A selected W1 is reused; this method cannot reset an uncertain platform fence.
    /// # Errors
    /// Refuses unknown/mismatched Reserve or any changed candidate, receiver or sealed original.
    pub fn select_retained_outgoing_terminal<R: KagemushaArtifactByteResolverV1>(
        &mut self,
        reserve_request_original_sha256: DigestV1,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (admitted, reservation) = self.retained_outgoing_reservation(profile, resolver)?;
        self.lineage_cas
            .reservation_receipt(
                reserve_request_original_sha256,
                self.publication.cash_financial(),
                reservation.reservation(),
            )
            .map_err(material)?
            .recheck_historical(self.publication.cash_financial())
            .map_err(material)?;
        if self
            .retained_outgoing_terminal_challenge(&admitted.candidate, &admitted.guard)?
            .is_some()
        {
            return self.outgoing_terminal_platform_fields();
        }
        let selection = self.captured_preparation()?;
        let outgoing = selection.outgoing_transport_originals()?;
        let (prepared, transition, recovery) =
            selection.retained_outgoing_proof_operands(&admitted.guard)?;
        if prepared != admitted.candidate.prepared_record() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let transition = transition.to_vec();
        let recovery = recovery.to_vec();
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let (transport, receiver, receiver_lease) = match outgoing {
            KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
                request,
                output,
                encrypted_credit,
                ..
            } => {
                let send = pending
                    .send_credit
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                (
                    terminal::TransportOriginals::Send {
                        request: *request,
                        output,
                        encrypted_credit,
                        receiver_counter_floor: send.originals.receiver_counter_floor(),
                        receiver_lease_original: send
                            .originals
                            .receiver_lease_original()
                            .map(<[u8]>::to_vec),
                    },
                    Some(Arc::clone(&send.receiver)),
                    send.receiver_lease.as_ref().map(Arc::clone),
                )
            }
            KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
                output,
                beneficiary,
                manifest_original,
                ..
            } => (
                terminal::TransportOriginals::Redeem {
                    output,
                    beneficiary,
                    manifest_original,
                },
                None,
                None,
            ),
        };
        self.require_outgoing_rows(4 + 3)?;
        self.select_terminal(
            admitted.candidate,
            admitted.guard,
            transport,
            receiver,
            receiver_lease,
            transition,
            recovery,
        )?;
        self.outgoing_terminal_platform_fields()
    }
    /// Fixed purpose1 original selected by Native; it cannot represent cash PrepareTransition.
    /// # Errors
    /// Refuses absent/changed current custody or expired uninvoked W1.
    pub fn outgoing_terminal_platform_fields(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (challenge, _, _, _) = self.outgoing_terminal_platform_state()?;
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
        self.outgoing_terminal_platform_state()?;
        Ok(fields)
    }
    /// Fsync before exactly one platform call; retained original recovery never invokes OS again.
    /// # Errors
    /// Refuses an uncertain fence without complete retained evidence.
    pub fn fence_outgoing_terminal_platform(
        &mut self,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (challenge, fenced, original, _) = self.outgoing_terminal_platform_state()?;
        if original.is_some() {
            return self.recover_outgoing_terminal_platform();
        }
        if fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.fence_terminal_platform(challenge.operation_id)?;
        Ok(vec![vec![0], vec![], vec![]])
    }
    /// Construct and authenticate full raw OS signature wrapper around the exact held W1.
    /// # Errors
    /// Rejects unfenced, substituted, unqualified, expired or differently retried raw evidence.
    pub fn retain_outgoing_terminal_platform_original(
        &mut self,
        raw: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        require_platform_original_length(raw)?;
        let (challenge, fenced, old, captured) = self.outgoing_terminal_platform_state()?;
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
                self.acknowledge_terminal_capture(challenge.operation_id)?;
            }
        } else {
            self.capture_terminal_original(challenge.operation_id, &original)?;
        }
        self.outgoing_terminal_platform_state()?;
        Ok(Sha256::digest(raw).into())
    }
    /// Return complete held originals only; a fenced unknown outcome remains frozen.
    /// # Errors
    /// Refuses changed challenge, partial bytes or an unretained invoked platform result.
    pub fn recover_outgoing_terminal_platform(
        &self,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let (challenge, fenced, original, captured) = self.outgoing_terminal_platform_state()?;
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
        require_platform_original_length(&raw)?;
        Ok(vec![vec![if captured { 2 } else { 1 }], raw, original])
    }
    /// Genuine W1 Guard + paired whole Terminal/Wrapper + opaque matching Reserve assemble Commit.
    /// The complete cap is fsynced before any account/HTTP Commit invocation.
    /// # Errors
    /// Refuses unavailable qualified keys, stale actual custody or changed global reservation.
    pub fn prove_retained_outgoing_commit<R: KagemushaArtifactByteResolverV1 + Clone>(
        &mut self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(digest) = self.retained_outgoing_commit_digest()? {
            return Ok(digest);
        }
        let (_, reservation) =
            self.retained_outgoing_reservation(profile.clone(), resolver.clone())?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let (request, _) = self
            .lineage_cas
            .acknowledged_outgoing_reservation_request(financial, &current, reservation.proof())
            .map_err(material)?
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let reserve_key = Sha256::digest(&request).into();
        let receipt = self
            .lineage_cas
            .reservation_receipt(reserve_key, financial, reservation.reservation())
            .map_err(material)?;
        let selection = self.captured_terminal()?;
        selection.recheck_lineage_reservation(&receipt)?;
        let prover =
            KagemushaProductionProverV1::load_ordinary_terminal(&selection, profile, resolver)
                .map_err(material)?;
        let original = prover
            .prove_ordinary_terminal_guard(&selection)
            .map_err(material)?;
        let guard = verify_ordinary_terminal_guard_v1(&selection, &original)?;
        let whole = prover
            .prove_ordinary_cash_terminal(&selection, &guard)
            .map_err(material)?;
        let generated = prover
            .assemble_ordinary_cash_commit(&selection, &guard, &whole, &receipt)
            .map_err(material)?;
        self.require_outgoing_rows(3)?;
        self.retain_generated_commit(generated, reserve_key)
    }
    /// Account signature is from the same Native session; full proof is the held generated cap.
    /// # Errors
    /// Refuses uncertain signing, changed proof or unavailable current FI.
    pub fn sign_outgoing_commit_transport(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.sign_retained_outgoing_commit_transport(sign)
    }
    /// Applies only actual acknowledged global Commit; a reservation does not publish State.
    /// # Errors
    /// Refuses mismatched/unknown Commit or any failed physical suffix/clock acknowledgment.
    pub fn advance_outgoing_commit(
        &mut self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.advance_acknowledged_commit(commit_request_original_sha256)
    }
    /// Retry only the separate post-State-fsync actual FI/current-clock acknowledgment.
    /// # Errors
    /// Refuses unavailable original/current custody and never creates another proof/State effect.
    pub fn acknowledge_outgoing_state_advance(
        &mut self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.acknowledge_state_advance(commit_request_original_sha256)
    }
    /// Delivery bytes are borrowable only after global Commit and separate actual StateAdvance Ack.
    /// # Errors
    /// Refuses missing/substituted acknowledgment or stale actual monetary custody.
    pub fn outgoing_delivery_original(
        &self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.acknowledged_outgoing_original(commit_request_original_sha256)
            .map(<[u8]>::to_vec)
    }
}
fn require_platform_original_length(raw: &[u8]) -> Result<(), KagemushaStateErrorV1> {
    if raw.is_empty() || raw.len() > 4096 {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outgoing_platform_requires_one_complete_bounded_os_original() {
        assert!(require_platform_original_length(&[]).is_err());
        assert!(require_platform_original_length(&[1]).is_ok());
        assert!(require_platform_original_length(&vec![1; 4096]).is_ok());
        assert!(require_platform_original_length(&vec![1; 4097]).is_err());
    }
}
