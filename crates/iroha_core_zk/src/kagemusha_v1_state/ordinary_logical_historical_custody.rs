//! Private immutable Bootstrap journal custody for already-captured cash proving.
//! Retained Native times authenticate originals only; this module lends no current authority.
use super::*;

impl KagemushaOrdinaryLogicalApprovalJournalV1 {
    pub(crate) fn recheck_historical_bootstrap_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.wal.check_owned().map_err(storage)?;
        if self.wal.recovery_prefix().map_err(storage)?.sequence > 1_000_000 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let accepted_at = pending
            .accepted_at_ms
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let captured_at = pending
            .captured_at_ms
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let original = pending
            .approved
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let authorization = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
            original.proof_binding_digest(),
            pending
                .approval_integrity_lease
                .as_ref()
                .map(|lease| lease.digest()),
        )
        .map_err(material)?;
        self.require_bootstrap_capture(captured_at, original.digest(), authorization)?;
        let decoded: KagemushaAppOperationApprovalV1 =
            norito::decode_canonical(original.original()).map_err(material)?;
        if norito::encode_canonical(&decoded).map_err(material)? != original.original() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let reverified = if let Some(lease) = &pending.reserve_integrity_lease {
            decoded.authenticate_with_integrity_lease(
                &pending.challenge,
                self.enrollment.app_credential(),
                lease,
                pending.counter_floor_before,
                accepted_at,
            )
        } else {
            decoded.authenticate(
                &pending.challenge,
                self.enrollment.app_credential(),
                pending.counter_floor_before,
                accepted_at,
            )
        }
        .map_err(material)?;
        if reverified.original() != original.original()
            || reverified.digest() != original.digest()
            || reverified.proof_binding_digest() != original.proof_binding_digest()
            || self.counter_floor
                != reverified
                    .app_attest_counter()
                    .or(pending.counter_floor_before)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        // This floor uses the retained genuine original FI admission, never current/latest PI.
        let expected_initialize = initial_record(&self.enrollment, &self.release, &self.bootstrap)?;
        let mut initialized = false;
        let mut reserved = false;
        let mut approved = false;
        let mut captured = false;
        let mut intended = false;
        let mut last_lease: Option<Vec<u8>> = None;
        self.wal
            .scan_complete(|sequence, raw| {
                let record: Record = norito::decode_canonical_with_limits(
                    raw,
                    norito::canonical_decode_limits(raw.len()),
                )
                .map_err(|_| PrivateJournalError::Corrupt)?;
                if norito::encode_canonical(&record).map_err(|_| PrivateJournalError::Corrupt)?
                    != raw
                {
                    return Err(PrivateJournalError::Corrupt);
                }
                match record {
                    Record::Initialize { .. }
                        if sequence == 0 && !initialized && record == expected_initialize =>
                    {
                        initialized = true
                    }
                    Record::IntegrityLease { original } if initialized => {
                        if original.is_empty()
                            || original.len() > FORMAT.maximum_payload_bytes as usize
                        {
                            return Err(PrivateJournalError::Corrupt);
                        }
                        // These are original authenticated rows appended by this sole journal or
                        // independently re-admitted during cold replay. Scan verifies their full
                        // immutable canonical hash chain; no decoder creates a PI capability.
                        last_lease = Some(original);
                    }
                    Record::Reserve {
                        challenge,
                        integrity_lease_digest,
                    } if initialized && !reserved => {
                        if challenge != pending.challenge
                            || integrity_lease_digest
                                != pending
                                    .reserve_integrity_lease
                                    .as_ref()
                                    .map(|lease| lease.digest())
                            || last_lease.as_deref()
                                != pending
                                    .reserve_integrity_lease
                                    .as_ref()
                                    .map(|lease| lease.original())
                        {
                            return Err(PrivateJournalError::Corrupt);
                        }
                        reserved = true;
                    }
                    Record::Approval {
                        accepted_at_ms,
                        integrity_lease_digest,
                        original: raw,
                        counter_floor_before,
                        accepted_counter,
                    } if reserved && !approved => {
                        if accepted_at_ms != accepted_at
                            || raw != original.original()
                            || counter_floor_before != pending.counter_floor_before
                            || accepted_counter != original.app_attest_counter()
                            || integrity_lease_digest
                                != pending
                                    .approval_integrity_lease
                                    .as_ref()
                                    .map(|lease| lease.digest())
                        {
                            return Err(PrivateJournalError::Corrupt);
                        }
                        approved = true;
                    }
                    Record::CaptureBootstrap {
                        captured_at_ms,
                        approval_digest,
                        authorization_binding_digest,
                    } if approved && !captured => {
                        if captured_at_ms != captured_at
                            || approval_digest != original.digest()
                            || authorization_binding_digest != authorization
                        {
                            return Err(PrivateJournalError::Corrupt);
                        }
                        captured = true;
                    }
                    Record::InitialPublicationIntent { original } if captured && !intended => {
                        if self.publication_intent.as_ref() != Some(&original) {
                            return Err(PrivateJournalError::Corrupt);
                        }
                        intended = true;
                    }
                    _ => return Err(PrivateJournalError::Corrupt),
                }
                Ok(())
            })
            .map_err(storage)?;
        if !initialized
            || !reserved
            || !approved
            || !captured
            || intended != self.publication_intent.is_some()
            || last_lease.as_deref() != self.integrity_lease.as_ref().map(|lease| lease.original())
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.wal.check_owned().map_err(storage)
    }
    pub(crate) fn historical_bootstrap_approval(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryHistoricalApprovalV1<'_>, KagemushaStateErrorV1> {
        self.recheck_historical_bootstrap_custody()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        Ok(KagemushaAuthenticatedOrdinaryHistoricalApprovalV1 {
            original: pending
                .approved
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
            journal: self,
            prefix: self.wal.recovery_prefix().map_err(storage)?,
            approval_admission_time_ms: pending
                .captured_at_ms
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
        })
    }
    /// Actual retained publication intent digest and original created/capture times only.
    pub(crate) fn historical_initial_publication_intent(
        &self,
    ) -> Result<(DigestV1, u64, u64), KagemushaStateErrorV1> {
        self.recheck_historical_bootstrap_custody()?;
        let intent = self
            .publication_intent
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let approval = self.historical_bootstrap_approval()?;
        let captured = approval.approval_admission_time_ms;
        if intent.bootstrap_ticket == 0
            || intent.created_at_ms < captured
            || intent.enrollment_id != self.enrollment.certificate().subject.enrollment_id
            || intent.release_id != self.release.release_id()
            || intent.certificate_digest
                != <DigestV1>::from(Sha256::digest(
                    self.enrollment
                        .certificate()
                        .canonical_bytes()
                        .map_err(material)?,
                ))
            || intent.credential_digest != self.enrollment.app_credential().digest()
            || intent.approval_digest != approval.original.digest()
            || intent.authorization_binding_digest != approval.authorization_binding_digest()?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok((
            Sha256::digest(norito::encode_canonical(intent).map_err(material)?).into(),
            intent.created_at_ms,
            captured,
        ))
    }
}
impl KagemushaAuthenticatedOrdinaryHistoricalApprovalV1<'_> {
    pub(crate) fn recheck_retained_capture_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.journal.recheck_historical_bootstrap_custody()?;
        if self.journal.wal.recovery_prefix().map_err(storage)? != self.prefix
            || self
                .journal
                .pending
                .as_ref()
                .and_then(|pending| pending.captured_at_ms)
                != Some(self.approval_admission_time_ms)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.original
            .recheck_at_trusted_time(self.approval_admission_time_ms)
            .map_err(material)
    }
    pub(crate) fn retained_capture_time_ms(&self) -> u64 {
        self.approval_admission_time_ms
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::publication_intent_fixture_journal;
    use super::*;
    #[test]
    fn historical_cash_bootstrap_keeps_actual_capture_and_intent_after_live_expiry() {
        for apple in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().canonicalize().unwrap().join("history");
            let (mut journal, financial) = publication_intent_fixture_journal(&path, apple, true);
            journal
                .begin_initial_publication(17, financial.trusted_time_ms().unwrap())
                .unwrap();
            let original = journal.historical_bootstrap_approval().unwrap();
            let captured_at = original.retained_capture_time_ms();
            let original_bytes = original.original().to_vec();
            let (intent, created_at, actual_capture) =
                journal.historical_initial_publication_intent().unwrap();
            assert_eq!(actual_capture, captured_at);
            assert!(created_at >= captured_at);
            assert_ne!(intent, [0; 32]);
            assert!(journal.recheck_at_trusted_time(90_000).is_err());
            journal.recheck_historical_bootstrap_custody().unwrap();
            assert_eq!(
                journal.historical_bootstrap_approval().unwrap().original(),
                original_bytes
            );
            assert_eq!(
                journal.historical_initial_publication_intent().unwrap(),
                (intent, created_at, actual_capture)
            );
            financial.recheck_historical_proof_custody().unwrap();
            assert_eq!(
                financial
                    .historical_financial_authority_commitment()
                    .unwrap(),
                journal
                    .retained_enrollment()
                    .app_credential()
                    .subject()
                    .financial_authority_commitment
            );
        }
    }
    #[test]
    fn historical_cash_bootstrap_refuses_capture_or_intent_field_substitution() {
        for changed in 0..4 {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().canonicalize().unwrap().join("substituted");
            let (mut journal, financial) = publication_intent_fixture_journal(&path, false, true);
            journal
                .begin_initial_publication(17, financial.trusted_time_ms().unwrap())
                .unwrap();
            journal.recheck_historical_bootstrap_custody().unwrap();
            match changed {
                0 => journal.pending.as_mut().unwrap().captured_at_ms = Some(999),
                1 => journal.pending.as_mut().unwrap().accepted_at_ms = Some(999),
                2 => journal.publication_intent.as_mut().unwrap().created_at_ms += 1,
                3 => journal.bootstrap.state.lane.device_lane_id = [99; 32],
                _ => unreachable!(),
            }
            assert!(journal.historical_initial_publication_intent().is_err());
            assert!(journal.historical_bootstrap_approval().is_err());
        }
    }
    #[test]
    fn historical_cash_bootstrap_refuses_duplicate_capture_even_with_valid_private_row_hash() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap().join("duplicate");
        let (mut journal, financial) = publication_intent_fixture_journal(&path, false, true);
        journal
            .begin_initial_publication(17, financial.trusted_time_ms().unwrap())
            .unwrap();
        let approval = journal.historical_bootstrap_approval().unwrap();
        let row = Record::CaptureBootstrap {
            captured_at_ms: approval.retained_capture_time_ms(),
            approval_digest: approval.original.digest(),
            authorization_binding_digest: approval.authorization_binding_digest().unwrap(),
        };
        drop(approval);
        journal.persist(&row).unwrap();
        assert!(journal.recheck_historical_bootstrap_custody().is_err());
        assert!(journal.historical_initial_publication_intent().is_err());
    }
}
