//! Exact reconstruction of the selected private worker request during recovery.
use super::*;
use iroha_core_zk::kagemusha_wallet_enrollment_v1::{
    PreKeyDispatchV1, RequestV1, ResultV1,
    issuer_worker::{EvidenceProjectionV1, OutcomeV1, VerifierRequestV1},
};
use iroha_data_model::kagemusha::KagemushaWalletCredentialBodyV1;

impl EnrollmentJournalV1 {
    /// Reconstruct the exact private request from retained account-signed E5 and issuer time.
    /// The independently admitted process must match the original configuration pin. This
    /// returns codec state only: a reread `Verifying` attempt permits `Recover`, never another
    /// `Verify`. Later terminal phases already have their original outcome in this journal.
    /// # Errors
    /// Refuses an unselected or stale attempt, configuration substitution, invalid E5, changed
    /// issuer bindings or any mismatch with the exact previously published worker request.
    pub fn retained_worker_request(
        &self,
        attempt: &EnrollmentAttemptV1,
        configuration: [u8; 32],
    ) -> Result<VerifierRequestV1> {
        self.require_current(attempt)?;
        if attempt.worker_configuration() != Some(configuration) || configuration == [0; 32] {
            return Err(Conflict);
        }
        let request = RequestV1::decode(&attempt.record.request).map_err(|_| Invalid)?;
        if request.body.challenge != attempt.selection().challenge
            || attempt
                .selection()
                .created_at_ms
                .checked_add(request.body.policy.challenge_lifetime_ms)
                != Some(attempt.selection().expires_at_ms)
        {
            return Err(Conflict);
        }
        let app = request.body.app.clone();
        let policy = request.body.policy;
        let worker = VerifierRequestV1::from_retained(
            request,
            &app,
            &policy,
            attempt.selection().created_at_ms,
            attempt.record.verification_time_ms,
            configuration,
        )
        .map_err(|_| Invalid)?;
        if worker.original() != attempt.record.worker_request {
            return Err(Invalid);
        }
        Ok(worker)
    }

    /// Validate and retain an authenticated worker's exact reply before permitting signing.
    /// Worker custody remains the calling service's responsibility. Unknown/unavailable
    /// outcomes never clear the consumed attempt or authorize another Verify dispatch.
    /// # Errors
    /// Refuses a stale/configuration-substituted request, malformed response, changed result
    /// or failure to durably retain a definitive outcome.
    pub fn retain_worker_response(
        &mut self,
        attempt: &mut EnrollmentAttemptV1,
        configuration: [u8; 32],
        exchange: [u8; 32],
        frame: &[u8],
    ) -> Result<OutcomeV1> {
        let worker = self.retained_worker_request(attempt, configuration)?;
        let outcome = worker.response(exchange, frame).map_err(|_| Invalid)?;
        match &outcome {
            OutcomeV1::Evidence(projection) => {
                self.retain_worker_result(attempt, projection.original_result.clone(), false)?;
            }
            // A fresh Recover exchange changes its envelope but cannot replace the first
            // definitive rejection original or restore this terminal attempt to Verifying.
            OutcomeV1::Rejected if attempt.phase() == EnrollmentJournalPhaseV1::Rejected => {}
            OutcomeV1::Rejected => self.retain_worker_result(attempt, frame.to_vec(), true)?,
            OutcomeV1::OutcomeUnknown | OutcomeV1::Unavailable => {}
        }
        Ok(outcome)
    }

    /// Recheck the exact retained evidence against the original request and worker selection.
    /// This is consistency evidence, not proof of worker custody, KYC or current eligibility.
    /// # Errors
    /// Refuses an unfinished/rejected attempt, corrupt evidence, changed configuration or custody.
    pub fn retained_worker_evidence(
        &self,
        attempt: &EnrollmentAttemptV1,
        configuration: [u8; 32],
    ) -> Result<EvidenceProjectionV1> {
        use EnrollmentJournalPhaseV1::{Evidence, Issued, Signing};
        if !matches!(attempt.phase(), Evidence | Signing | Issued) {
            return Err(Conflict);
        }
        self.retained_worker_request(attempt, configuration)?
            .retained_evidence(&attempt.record.worker_result)
            .map_err(|_| Invalid)
    }

    /// Freeze the exact initial credential body and original issue time before invoking the
    /// independently admitted Enrollment signer. Retries recover this body without refreshing
    /// its time. The service must recheck current eligibility before signing or delivery.
    /// # Errors
    /// Refuses missing permits/evidence, changed selections, invalid trusted time, overflow,
    /// corrupt retained bodies or uncertain publication. No error grants signing authority.
    pub fn select_credential_body(
        &mut self,
        attempt: &mut EnrollmentAttemptV1,
        dispatch: &PreKeyDispatchV1,
        configuration: [u8; 32],
        issued_at_ms: u64,
    ) -> Result<KagemushaWalletCredentialBodyV1> {
        self.permit(attempt, dispatch)?.ok_or(Conflict)?;
        let evidence = self.retained_worker_evidence(attempt, configuration)?;
        let request = RequestV1::decode(&attempt.record.request).map_err(|_| Invalid)?;
        let retained = if matches!(
            attempt.phase(),
            EnrollmentJournalPhaseV1::Signing | EnrollmentJournalPhaseV1::Issued
        ) {
            Some(
                norito::decode_canonical_with_limits::<KagemushaWalletCredentialBodyV1>(
                    &attempt.record.credential_body,
                    norito::canonical_decode_limits(ORIGINAL_MAX),
                )
                .map_err(|_| Invalid)?,
            )
        } else {
            None
        };
        let time = retained
            .as_ref()
            .map_or(issued_at_ms, |body| body.issued_at_ms);
        if time == 0 || time < attempt.record.verification_time_ms {
            return Err(Invalid);
        }
        let request = &request.body;
        let body = KagemushaWalletCredentialBodyV1 {
            version: 1,
            scheme_id: request.challenge.scheme_id,
            asset_digest: request.challenge.asset_digest,
            wallet_id: request.marker.wallet_id,
            account_digest: request.challenge.account_digest,
            payment_key: request.marker.payment_key,
            provider_contract: dispatch.scheme.provider_contract,
            evidence_kind: evidence.kind,
            enrollment_evidence: evidence.evidence,
            fresh_evidence: evidence.evidence,
            app_policy: request.challenge.app_policy,
            regulatory_policy: request.policy.regulatory_policy,
            enrollment_id: request.challenge.enrollment_id(&request.marker.payment_key),
            issued_at_ms: time,
            renewal_sequence: 0,
            lease_expires_at_ms: request.policy.lease_expires_at(time).map_err(|_| Invalid)?,
            issuer_certificate: dispatch.enrollment_certificate.certificate_digest(),
        };
        body.validate().map_err(|_| Invalid)?;
        if let Some(retained) = retained {
            return if retained == body {
                Ok(retained)
            } else {
                Err(Conflict)
            };
        }
        let mut record = attempt.record.clone();
        record.phase = EnrollmentJournalPhaseV1::Signing;
        record.credential_body = norito::encode_canonical(&body).map_err(|_| Invalid)?;
        self.advance(attempt, record)?;
        Ok(body)
    }

    /// Verify the actual signed E6 against the frozen body, selected issuer and worker originals,
    /// then retain its exact bytes before delivery. A retry can never replace a signed original.
    /// # Errors
    /// Refuses missing signing selection, foreign signatures/evidence, stale custody or failed
    /// publication. The caller still owns current eligibility, signing custody and ledger claims.
    pub fn retain_credential(
        &mut self,
        attempt: &mut EnrollmentAttemptV1,
        dispatch: &PreKeyDispatchV1,
        configuration: [u8; 32],
        original: Vec<u8>,
    ) -> Result<()> {
        if !matches!(
            attempt.phase(),
            EnrollmentJournalPhaseV1::Signing | EnrollmentJournalPhaseV1::Issued
        ) {
            return Err(Conflict);
        }
        let body = self.select_credential_body(attempt, dispatch, configuration, 0)?;
        let request = RequestV1::decode(&attempt.record.request).map_err(|_| Invalid)?;
        let evidence = self.retained_worker_evidence(attempt, configuration)?;
        let result = ResultV1::decode(&original).map_err(|_| Invalid)?;
        let credential = result
            .verify_for(&dispatch.scheme, &dispatch.enrollment_certificate, &request)
            .map_err(|_| Invalid)?;
        if credential.body != body || result.evidence != evidence.originals {
            return Err(Conflict);
        }
        self.retain_issued(attempt, original)
    }
}
