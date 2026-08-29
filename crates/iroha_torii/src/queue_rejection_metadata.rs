impl Error {
    fn status_code_for_queue_error(err: &queue::Error) -> StatusCode {
        match err {
            queue::Error::Full
            | queue::Error::LatencySaturated
            | queue::Error::MaximumTransactionsPerUser => StatusCode::TOO_MANY_REQUESTS,
            queue::Error::Expired => StatusCode::BAD_REQUEST,
            queue::Error::UnresolvedRoute { .. } => StatusCode::BAD_REQUEST,
            queue::Error::InBlockchain => StatusCode::CONFLICT,
            queue::Error::IsInQueue => StatusCode::CONFLICT,
            queue::Error::UnregisteredAuthority { .. } => StatusCode::FORBIDDEN,
            queue::Error::Governance(_) => StatusCode::INTERNAL_SERVER_ERROR,
            queue::Error::GovernanceNotPermitted { .. } => StatusCode::FORBIDDEN,
            queue::Error::LaneComplianceDenied { .. } => StatusCode::FORBIDDEN,
            queue::Error::LanePrivacyProofRejected { .. } => StatusCode::FORBIDDEN,
            queue::Error::NexusFeeAdmissionRejected { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            queue::Error::ConfidentialPolicyAdmissionRejected { .. } => StatusCode::FORBIDDEN,
            queue::Error::NexusFeeAdmissionConfigInvalid { .. } => StatusCode::SERVICE_UNAVAILABLE,
            queue::Error::PlanJournalDurabilityRejected { .. }
            | queue::Error::PlanJournalDurabilityIndeterminate { .. } => {
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
    }
    fn queue_error_summary(err: &queue::Error) -> (&'static str, &'static str) {
        match err {
            queue::Error::Full => ("queue_full", "transaction queue is at capacity"),
            queue::Error::LatencySaturated => (
                "queue_latency_saturated",
                "transaction queue latency budget is saturated",
            ),
            queue::Error::MaximumTransactionsPerUser => (
                "per_user_queue_limit",
                "authority reached its per-user queue capacity",
            ),
            queue::Error::Expired => (
                "transaction_expired",
                "transaction expired before admission",
            ),
            queue::Error::UnresolvedRoute { .. } => (
                "queue_unresolved_route",
                "transaction route could not be resolved",
            ),
            queue::Error::InBlockchain => (
                "already_committed",
                "transaction already committed to the blockchain",
            ),
            queue::Error::IsInQueue => (
                "already_enqueued",
                "transaction already present in the queue",
            ),
            queue::Error::UnregisteredAuthority { .. } => (
                "unregistered_authority",
                "transaction authority is not registered",
            ),
            queue::Error::Governance(_) => (
                "queue_governance_invalid",
                "lane governance manifest is missing or invalid",
            ),
            queue::Error::GovernanceNotPermitted { .. } => (
                "queue_governance_rejected",
                "lane governance manifest rejected the transaction",
            ),
            queue::Error::LaneComplianceDenied { .. } => (
                "queue_lane_compliance_denied",
                "lane compliance policy rejected the transaction",
            ),
            queue::Error::LanePrivacyProofRejected { .. } => (
                "queue_lane_privacy_proof_rejected",
                "lane privacy proof rejected the transaction",
            ),
            queue::Error::NexusFeeAdmissionRejected { .. } => (
                "queue_nexus_fee_rejected",
                "transaction cannot cover the Nexus fee admission bound",
            ),
            queue::Error::ConfidentialPolicyAdmissionRejected { .. } => (
                "queue_confidential_policy_rejected",
                "confidential policy rejected the transaction",
            ),
            queue::Error::NexusFeeAdmissionConfigInvalid { .. } => (
                "queue_nexus_fee_config_invalid",
                "node Nexus fee configuration is invalid",
            ),
            queue::Error::PlanJournalDurabilityRejected { .. } => (
                "queue_plan_journal_unavailable",
                "transaction queue could not establish the required durability boundary",
            ),
            queue::Error::PlanJournalDurabilityIndeterminate { .. } => (
                "queue_plan_journal_outcome_unknown",
                "transaction admission outcome is unknown; reconcile by exact entrypoint hash before retrying",
            ),
        }
    }
    fn queue_error_envelope(
        err: &queue::Error,
        backpressure: Option<queue::BackpressureState>,
    ) -> ErrorEnvelope {
        let (code, message) = Self::queue_error_summary(err);
        let retry_after_seconds = match err {
            queue::Error::Full
            | queue::Error::LatencySaturated
            | queue::Error::MaximumTransactionsPerUser => Some(1),
            _ => None,
        };
        let (reject_code, _detail) = queue_rejection_metadata(err);
        let fee = match err {
            queue::Error::NexusFeeAdmissionRejected { code, .. }
            | queue::Error::NexusFeeAdmissionConfigInvalid { code, .. } => Some(FeeErrorDetails {
                code: code.as_str().to_owned(),
                retryable: fee_quote_rejection_retryable(*code),
                remediation: Some(fee_quote_remediation(*code).to_owned()),
                ..FeeErrorDetails::default()
            }),
            _ => None,
        };
        let (entrypoint_hash, tx_hash, hint) = match err {
            queue::Error::PlanJournalDurabilityIndeterminate {
                entrypoint_hash,
                signed_transaction_hash,
                ..
            } => {
                let hint = if signed_transaction_hash.is_some() {
                    "Reconcile this exact entrypoint hash, then query status by the signed transaction hash or resubmit byte-identical signed bytes; do not create a replacement transaction until the outcome is known."
                } else {
                    "Reconcile this exact entrypoint hash; do not create a replacement transaction until the outcome is known."
                };
                (
                    Some(entrypoint_hash.to_string()),
                    signed_transaction_hash.as_ref().map(ToString::to_string),
                    Some(hint.to_owned()),
                )
            }
            queue::Error::PlanJournalDurabilityRejected { .. } => (
                None,
                None,
                Some(
                    "The transaction was not admitted; restore queue-plan journal health before retrying."
                        .to_owned(),
                ),
            ),
            _ => (None, None, None),
        };
        ErrorEnvelope::new(code, message).with_details(ErrorDetails {
            reject_code: Some(reject_code.to_owned()),
            queue: backpressure.map(|backpressure| {
                let saturated = backpressure.is_saturated();
                QueueErrorSnapshot {
                    state: if saturated {
                        "saturated".to_owned()
                    } else {
                        "healthy".to_owned()
                    },
                    queued: backpressure.queued() as u64,
                    capacity: backpressure.capacity().get() as u64,
                    saturated,
                }
            }),
            retry_after_seconds,
            fee,
            entrypoint_hash,
            tx_hash,
            hint,
            ..Default::default()
        })
    }
}
fn queue_rejection_metadata(err: &queue::Error) -> (&'static str, String) {
    match err {
        queue::Error::Full => (
            "PRTRY:QUEUE_FULL",
            "transaction queue is at capacity".to_owned(),
        ),
        queue::Error::LatencySaturated => (
            "PRTRY:QUEUE_LATENCY",
            "transaction queue latency budget is saturated".to_owned(),
        ),
        queue::Error::MaximumTransactionsPerUser => (
            "PRTRY:QUEUE_RATE",
            "authority reached per-user queue capacity".to_owned(),
        ),
        queue::Error::Expired => ("ED07", "transaction expired before admission".to_owned()),
        queue::Error::UnresolvedRoute { reason } => (
            "PRTRY:ROUTE_UNRESOLVED",
            format!("transaction route could not be resolved: {reason}"),
        ),
        queue::Error::InBlockchain => (
            "PRTRY:ALREADY_COMMITTED",
            "transaction already committed to the blockchain".to_owned(),
        ),
        queue::Error::IsInQueue => (
            "PRTRY:ALREADY_ENQUEUED",
            "transaction already present in the queue".to_owned(),
        ),
        queue::Error::UnregisteredAuthority { authority } => (
            "PRTRY:UNREGISTERED_AUTHORITY",
            format!("transaction authority is not registered: {authority}"),
        ),
        queue::Error::Governance(err) => (
            "PRTRY:QUEUE_GOVERNANCE_INVALID",
            format!("lane governance manifest invalid: {err}"),
        ),
        queue::Error::GovernanceNotPermitted { alias, reason } => (
            "PRTRY:QUEUE_GOVERNANCE_REJECTED",
            format!("lane governance rejected transaction for alias '{alias}': {reason}"),
        ),
        queue::Error::LaneComplianceDenied { alias, reason } => (
            "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED",
            format!("lane compliance policy rejected transaction for alias '{alias}': {reason}"),
        ),
        queue::Error::LanePrivacyProofRejected { alias, reason } => (
            "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED",
            format!("lane privacy proof rejected transaction for alias '{alias}': {reason}"),
        ),
        queue::Error::NexusFeeAdmissionRejected { code, .. } => (
            "PRTRY:NEXUS_FEE_ADMISSION_REJECTED",
            format!(
                "transaction rejected by Nexus fee admission: {}",
                code.as_str()
            ),
        ),
        queue::Error::ConfidentialPolicyAdmissionRejected { detail, .. } => (
            "PRTRY:CONFIDENTIAL_POLICY_REJECTED",
            format!("transaction rejected by confidential policy admission: {detail}"),
        ),
        queue::Error::NexusFeeAdmissionConfigInvalid { code, .. } => (
            "PRTRY:NEXUS_FEE_ADMISSION_CONFIG_INVALID",
            format!(
                "invalid Nexus fee admission configuration: {}",
                code.as_str()
            ),
        ),
        queue::Error::PlanJournalDurabilityRejected { reason } => (
            "PRTRY:QUEUE_PLAN_JOURNAL_UNAVAILABLE",
            format!("transaction queue did not durably admit the transaction: {reason}"),
        ),
        queue::Error::PlanJournalDurabilityIndeterminate {
            entrypoint_hash,
            signed_transaction_hash,
            reason,
        } => (
            "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN",
            format!(
                "transaction admission outcome is unknown for entrypoint {entrypoint_hash}{}; reconcile that exact entrypoint before retrying: {reason}",
                signed_transaction_hash
                    .as_ref()
                    .map(|hash| format!(" (signed transaction {hash})"))
                    .unwrap_or_default()
            ),
        ),
    }
}
