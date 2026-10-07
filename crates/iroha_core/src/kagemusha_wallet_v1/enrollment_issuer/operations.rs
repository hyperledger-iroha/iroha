//! Closed E1, E5, credential-signing and exact E6-delivery operations.

use super::*;

impl<R: EnrollmentIssuerRuntimeV1> EnrollmentIssuerV1<R> {
    fn configuration(
        &mut self,
        session: &EnrollmentIssuerSessionV1,
        provider: &KagemushaEnrollmentProvider,
    ) -> Result<VerifierConfigurationV1> {
        let config = self
            .runtime
            .worker_configuration(provider, &session.dispatch.asset)?;
        config
            .preparation(
                &session.dispatch,
                session.attempt.selection().challenge,
                session.attempt.selection().created_at_ms,
            )
            .map_err(|_| Selection)?;
        if session
            .attempt
            .worker_configuration()
            .is_some_and(|p| p != config.digest())
        {
            return Err(Selection);
        }
        Ok(config)
    }

    fn prepare_worker(
        &mut self,
        session: &mut EnrollmentIssuerSessionV1,
        provider: &KagemushaEnrollmentProvider,
    ) -> Result<()> {
        let configuration = self.configuration(session, provider)?;
        if session.attempt.phase() != Phase::Selected {
            // A consumed attempt never sends Prepare again, even if the worker lost a row.
            self.journal
                .retained_worker_request(&session.attempt, configuration.digest())?;
            return Ok(());
        }
        let packet = configuration
            .journal(random_nonce()?)
            .map_err(|_| Invalid)?;
        let response = self
            .runtime
            .worker_exchange(provider, &configuration, &packet)?;
        let incarnation = packet.journal_response(&response).map_err(|_| Invalid)?;
        self.journal.select_worker_preparation(
            &mut session.attempt,
            &session.dispatch,
            configuration.digest(),
            incarnation,
        )?;
        let exchange = random_nonce()?;
        let packet = self.journal.worker_preparation_exchange(
            &session.attempt,
            configuration.digest(),
            exchange,
        )?;
        let response = self
            .runtime
            .worker_exchange(provider, &configuration, &packet)?;
        self.journal.retain_worker_prepared(
            &mut session.attempt,
            configuration.digest(),
            exchange,
            &response,
        )?;
        Ok(())
    }

    /// Retain the actual worker preparation, consume fresh provider eligibility, then deliver this
    /// dispatch's exact retained permit or retain a newly signed original before delivery.
    /// # Errors
    /// Rejects stale/revoked routing, worker custody/configuration loss, expired E1, denied provider
    /// observations, malformed signatures or uncertain publication. No error permits phone keygen.
    pub fn pre_key_permit(&mut self, session: &mut EnrollmentIssuerSessionV1) -> Result<Vec<u8>> {
        self.require_session(session)?;
        let current = self.current(&session.dispatch)?;
        self.prepare_worker(session, &current.provider)?;
        let operation =
            operation_digest(b"pre-key", &session.dispatch.encode().map_err(|_| Invalid)?)?;
        let (selected, expires) = self.authorize(
            session,
            KagemushaEligibilityPurposeV1::PreKeyPermit,
            operation,
        )?;
        let configuration = self.configuration(session, &selected.provider)?;
        self.journal
            .worker_preparation(&session.attempt, configuration.digest())?;
        if let Some(original) = self.journal.permit(&session.attempt, &session.dispatch)? {
            self.finish_boundary(session, &selected, expires)?;
            return Ok(original);
        }
        let observed_at_ms = self.now()?;
        let dispatch = &session.dispatch;
        let attempt = session.attempt.selection();
        let body = KagemushaEnrollmentPermitBodyV1 {
            version: 1,
            platform: dispatch.platform,
            purpose: dispatch.purpose,
            challenge: attempt.challenge,
            network_id: dispatch.scheme.network_id,
            manifest_digest: dispatch.manifest_digest,
            release_digest: dispatch.release_digest,
            service_origin_digest: dispatch.service_origin_digest,
            fi_digest: dispatch.fi_digest,
            actor_digest: dispatch.actor_digest,
            attempt_id: attempt.attempt_id,
            client_nonce: dispatch.client_nonce,
            native_dispatch_nonce: dispatch.native_dispatch_nonce,
            originals_digest: dispatch
                .originals_digest(&attempt.challenge)
                .map_err(|_| Invalid)?,
            enrollment_certificate: dispatch.enrollment_certificate.certificate_digest(),
            created_at_ms: attempt.created_at_ms,
            expires_at_ms: attempt.expires_at_ms,
            observed_at_ms,
        };
        self.finish_boundary(session, &selected, expires)?;
        let signature = self.runtime.sign_enrollment(
            &selected.provider,
            &body.signing_message().map_err(|_| Invalid)?,
        )?;
        let original = KagemushaEnrollmentPermitV1::from_issuer_der(
            body,
            &dispatch.scheme,
            &dispatch.enrollment_certificate,
            &signature,
        )
        .map_err(|_| Invalid)?
        .encode_canonical()
        .map_err(|_| Invalid)?;
        self.journal
            .retain_permit(&session.attempt, dispatch, original.clone())?;
        self.finish_boundary(session, &selected, expires)?;
        Ok(original)
    }

    /// Durably select and dispatch one account-signed E5, or recover the exact consumed E5.
    /// Returning success means genuine worker evidence was retained, not credential issuance.
    /// # Errors
    /// Refuses changed requests, fresh verification outside E1, current selection failure,
    /// worker rejection or unavailable/unknown outcomes. Recovery never sends Prepare or a new E5.
    pub fn verify_evidence(
        &mut self,
        session: &mut EnrollmentIssuerSessionV1,
        request_original: &[u8],
    ) -> Result<()> {
        self.require_session(session)?;
        let request = RequestV1::decode(request_original).map_err(|_| Invalid)?;
        let mut selected = self.current(&session.dispatch)?;
        let mut configuration = self.configuration(session, &selected.provider)?;
        let mut authorization_expires = None;
        let action = if session.attempt.phase() == Phase::Selected {
            self.journal
                .permit(&session.attempt, &session.dispatch)?
                .ok_or(Selection)?;
            let operation = operation_digest(b"verify-evidence", request_original)?;
            let (fresh, expires) = self.authorize(
                session,
                KagemushaEligibilityPurposeV1::VerifyEvidence,
                operation,
            )?;
            selected = fresh;
            configuration = self.configuration(session, &selected.provider)?;
            authorization_expires = Some(expires);
            let verification_time = self.now()?;
            if verification_time >= expires {
                return Err(Selection);
            }
            let preparation = self
                .journal
                .worker_preparation(&session.attempt, configuration.digest())?;
            let expected = configuration
                .request(request.clone(), &preparation, verification_time)
                .map_err(|_| Invalid)?;
            let permit = self.journal.select_verification(
                &mut session.attempt,
                request,
                configuration.digest(),
                verification_time,
            )?;
            if permit.into_original() != expected.original() {
                return Err(Invalid);
            }
            ActionV1::Complete
        } else {
            let (retained, _, _) = session.attempt.verification().ok_or(Selection)?;
            if retained != request_original {
                return Err(Selection);
            }
            match session.attempt.phase() {
                Phase::Evidence | Phase::Signing | Phase::Issued => {
                    self.journal
                        .retained_worker_evidence(&session.attempt, configuration.digest())?;
                    return Ok(());
                }
                Phase::Rejected => return Err(Rejected),
                Phase::Verifying => {
                    if self.now()? >= session.attempt.selection().expires_at_ms {
                        // The passive action cannot claim an unprocessed row. It may only
                        // read the exact retained result produced under the original E5.
                        ActionV1::Inspect
                    } else {
                        let operation = operation_digest(b"verify-evidence", request_original)?;
                        let (fresh, expires) = self.authorize(
                            session,
                            KagemushaEligibilityPurposeV1::VerifyEvidence,
                            operation,
                        )?;
                        selected = fresh;
                        configuration = self.configuration(session, &selected.provider)?;
                        authorization_expires = Some(expires);
                        ActionV1::Recover
                    }
                }
                Phase::Selected => return Err(Invalid),
            }
        };
        let now = self.now()?;
        let exchange = self.journal.worker_exchange(
            &session.attempt,
            configuration.digest(),
            action,
            random_nonce()?,
            now,
        )?;
        if let Some(expires) = authorization_expires {
            self.finish_boundary(session, &selected, expires)?;
        }
        let response =
            self.runtime
                .worker_exchange(&selected.provider, &configuration, &exchange)?;
        let outcome = self.journal.retain_worker_response(
            &mut session.attempt,
            configuration.digest(),
            &exchange,
            &response,
        )?;
        self.unchanged(&session.dispatch, &selected)?;
        match outcome {
            OutcomeV1::Evidence(_) => Ok(()),
            OutcomeV1::Rejected => Err(Rejected),
            OutcomeV1::OutcomeUnknown | OutcomeV1::Unavailable => Err(Pending),
        }
    }

    /// Retain one initial credential body and actual rooted signature under a fresh current
    /// provider observation. The original evidence/issue time is never refreshed on recovery.
    /// # Errors
    /// Refuses missing evidence, revoked current selection, denied eligibility, substituted
    /// worker/signer or uncertain publication. Verified recovery is not expired by the old E1.
    pub fn issue_credential(&mut self, session: &mut EnrollmentIssuerSessionV1) -> Result<()> {
        self.require_session(session)?;
        if session.attempt.phase() == Phase::Issued {
            // Do not re-sign or consume another signing authorization. Delivery has its own
            // fresh current-provider boundary and checks the retained original again.
            return Ok(());
        }
        let current = self.current(&session.dispatch)?;
        let configuration = self.configuration(session, &current.provider)?;
        let evidence = self
            .journal
            .retained_worker_evidence(&session.attempt, configuration.digest())?;
        let now = self.now()?;
        let body = self.journal.select_credential_body(
            &mut session.attempt,
            &session.dispatch,
            configuration.digest(),
            now,
        )?;
        let operation = operation_digest(
            b"issue-credential",
            &norito::encode_canonical(&body).map_err(|_| Invalid)?,
        )?;
        let (selected, expires) = self.authorize(
            session,
            KagemushaEligibilityPurposeV1::IssueCredential,
            operation,
        )?;
        let current_configuration = self.configuration(session, &selected.provider)?;
        if current_configuration.digest() != configuration.digest() {
            return Err(Selection);
        }
        self.finish_boundary(session, &selected, expires)?;
        let signature = self
            .runtime
            .sign_enrollment(&selected.provider, &body.signing_message())?;
        let credential = KagemushaWalletCredentialV1::sign(
            body,
            &selected.provider.certificate,
            KagemushaWalletSignerOutputV1::Der(&signature),
        )
        .map_err(|_| Invalid)?;
        let certificates =
            KagemushaWalletCertificateSetV1::new(vec![selected.provider.certificate])
                .map_err(|_| Invalid)?;
        let original = ResultV1 {
            version: 1,
            credential: credential.to_canonical_bytes().map_err(|_| Invalid)?,
            certificates: norito::encode_canonical(&certificates).map_err(|_| Invalid)?,
            evidence: evidence.originals,
        }
        .encode()
        .map_err(|_| Invalid)?;
        self.journal.retain_credential(
            &mut session.attempt,
            &session.dispatch,
            configuration.digest(),
            original,
        )?;
        self.finish_boundary(session, &selected, expires)
    }

    /// Return exact durable E6 bytes only after a separate fresh delivery observation.
    /// # Errors
    /// Refuses incomplete issuance, current revocation, foreign routing or signer/evidence
    /// originals and all custody failures. No old E1 deadline is applied to retained delivery.
    pub fn deliver_credential(
        &mut self,
        session: &mut EnrollmentIssuerSessionV1,
    ) -> Result<Vec<u8>> {
        self.require_session(session)?;
        let original = session.attempt.issued().ok_or(Pending)?.to_vec();
        let operation = operation_digest(b"deliver-credential", &original)?;
        let (selected, expires) = self.authorize(
            session,
            KagemushaEligibilityPurposeV1::DeliverCredential,
            operation,
        )?;
        let configuration = self.configuration(session, &selected.provider)?;
        self.journal.retain_credential(
            &mut session.attempt,
            &session.dispatch,
            configuration.digest(),
            original.clone(),
        )?;
        self.finish_boundary(session, &selected, expires)?;
        Ok(original)
    }
}
