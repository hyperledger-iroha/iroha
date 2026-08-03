#[derive(Debug)]
struct ReputationRetentionAuthorityForTest {
    qualification: ReputationFinalizedArchiveRetentionAuthorityQualificationV1,
    latest: Mutex<Option<ReputationFinalizedArchiveRetentionApprovalRecordV1>>,
    armed_load_failure_after_cas: Mutex<Option<usize>>,
    load_failure_countdown: Mutex<Option<usize>>,
}

impl ReputationRetentionAuthorityForTest {
    fn new() -> Self {
        Self {
            qualification: ReputationFinalizedArchiveRetentionAuthorityQualificationV1::new(
                7, [0xA7; 32],
            ),
            latest: Mutex::new(None),
            armed_load_failure_after_cas: Mutex::new(None),
            load_failure_countdown: Mutex::new(None),
        }
    }

    fn binding(&self) -> ReputationFinalizedArchiveRetentionAuthorityBindingV1 {
        ReputationFinalizedArchiveRetentionAuthorityBindingV1::try_new(
            self.handle().to_owned(),
            self.qualification.revision(),
            self.qualification.policy_digest(),
        )
        .expect("valid reputation retention authority binding")
    }

    fn fail_nth_load_after_next_cas(&self, load_number: usize) {
        assert!(load_number != 0);
        *self
            .armed_load_failure_after_cas
            .lock()
            .expect("lock armed retention failure") = Some(load_number);
    }
}

impl ReputationFinalizedArchiveRetentionAuthorityV1 for ReputationRetentionAuthorityForTest {
    fn handle(&self) -> &str {
        "sealed.reputation.archive.v2-apply"
    }

    fn qualification(
        &self,
    ) -> Result<
        ReputationFinalizedArchiveRetentionAuthorityQualificationV1,
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        Ok(self.qualification)
    }

    fn load_latest(
        &self,
        _chain_id: &ChainId,
    ) -> Result<
        Option<ReputationFinalizedArchiveRetentionApprovalRecordV1>,
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        let mut countdown = self
            .load_failure_countdown
            .lock()
            .expect("lock retention load failure");
        if let Some(remaining) = *countdown {
            if remaining == 1 {
                *countdown = None;
                return Err(
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Unavailable,
                );
            }
            *countdown = Some(remaining - 1);
        }
        drop(countdown);
        Ok(self.latest.lock().expect("lock retention approval").clone())
    }

    fn compare_and_swap_latest(
        &self,
        _chain_id: &ChainId,
        expected_revision: Option<[u8; 32]>,
        next: &ReputationFinalizedArchiveRetentionApprovalRecordV1,
    ) -> Result<(), ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1> {
        let mut latest = self.latest.lock().expect("lock retention approval");
        if latest
            .as_ref()
            .map(ReputationFinalizedArchiveRetentionApprovalRecordV1::revision)
            != expected_revision
        {
            return Err(ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Rejected);
        }
        *latest = Some(next.clone());
        let armed = self
            .armed_load_failure_after_cas
            .lock()
            .expect("lock armed retention failure")
            .take();
        *self
            .load_failure_countdown
            .lock()
            .expect("lock retention load failure") = armed;
        Ok(())
    }
}
