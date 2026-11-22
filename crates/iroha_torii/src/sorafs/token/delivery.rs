//! Recover an exact native receipt if the account request stopped waiting for its worker.
use super::*;
use sorafs_manifest::provider_advert::account_read::RegisteredAccountReadV1;

#[derive(PartialEq, Eq)]
pub(in crate::sorafs) struct AccountDelivery {
    subject: StreamTokenQuotaSubject,
    manifest_id: String,
    manifest_cid: Vec<u8>,
    provider: [u8; 32],
    profile: String,
    overrides: TokenOverrides,
    policy: RegisteredAccountReadV1,
}
impl AccountDelivery {
    pub(in crate::sorafs) fn new(
        subject: StreamTokenQuotaSubject,
        manifest_id: String,
        manifest_cid: Vec<u8>,
        provider: [u8; 32],
        profile: String,
        overrides: TokenOverrides,
        policy: RegisteredAccountReadV1,
    ) -> Self {
        Self {
            subject,
            manifest_id,
            manifest_cid,
            provider,
            profile,
            overrides,
            policy,
        }
    }
}

pub(in crate::sorafs) struct UndeliveredIssue {
    request: AccountDelivery,
    body: StreamTokenBodyV1,
    remaining_quota: u32,
}

impl StreamTokenIssuer {
    pub(in crate::sorafs) fn take_undelivered(
        &self,
        request: &AccountDelivery,
    ) -> Option<UndeliveredIssue> {
        let mut slot = self.undelivered.lock().ok()?;
        if slot.as_ref().is_some_and(|old| &old.request == request) {
            return slot.take();
        }
        None
    }

    pub(in crate::sorafs) fn recover_undelivered(
        &self,
        old: UndeliveredIssue,
    ) -> Result<TokenIssue, StreamTokenIssuerError> {
        let now = self.signer.now_unix_ms()? / 1000;
        self.observe_epoch(now)?;
        if now >= old.body.ttl_epoch {
            let r = old.request;
            return self.issue_token(
                r.subject,
                r.manifest_cid,
                r.provider,
                r.profile,
                r.overrides,
                Some(&r.policy),
            );
        }
        let token = self.signer.recover_undelivered(old.body)?;
        // The original issuance spent quota. Account for any later issuances in the same window.
        let now = Instant::now();
        let budgets = self
            .issuance_budgets
            .lock()
            .map_err(|_| StreamTokenIssuerError::IssuanceQuotaStateUnavailable)?;
        let limit = self
            .defaults
            .requests_per_minute
            .min(old.request.policy.requests_per_minute);
        let remaining = budgets
            .get(&old.request.subject)
            .filter(|budget| {
                now.saturating_duration_since(budget.window_start) < ISSUANCE_QUOTA_WINDOW
            })
            .map_or(limit, |budget| limit.saturating_sub(budget.used));
        Ok(TokenIssue {
            token,
            remaining_quota: old.remaining_quota.min(remaining),
        })
    }
}

/// Normal delivery disarms the guard. A cancelled physical worker keeps only the public
/// operation body for a fresh receipt proof. No signature is stored in the issuer.
pub(in crate::sorafs) struct WorkerDelivery {
    issuer: Arc<StreamTokenIssuer>,
    request: Option<AccountDelivery>,
    issue: Option<TokenIssue>,
}
impl WorkerDelivery {
    pub(in crate::sorafs) fn new(
        issuer: Arc<StreamTokenIssuer>,
        request: Option<AccountDelivery>,
        issue: TokenIssue,
    ) -> Self {
        Self {
            issuer,
            request,
            issue: Some(issue),
        }
    }
    pub(in crate::sorafs) fn receive(mut self) -> TokenIssue {
        self.issue.take().expect("worker owns its completed issue")
    }
}
impl Drop for WorkerDelivery {
    fn drop(&mut self) {
        if let Some(mut issue) = self.issue.take() {
            iroha_crypto::zeroize_value_for_confidential_discard(&mut issue.token.signature);
            if let Some(request) = self.request.take()
                && let Ok(mut slot) = self.issuer.undelivered.lock()
            {
                *slot = Some(UndeliveredIssue {
                    request,
                    body: issue.token.body,
                    remaining_quota: issue.remaining_quota,
                });
            }
        }
    }
}
