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
            transaction_hash,
            reason,
        } => (
            "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN",
            format!(
                "transaction admission outcome is unknown for {transaction_hash}; reconcile that exact signed hash before retrying: {reason}"
            ),
        ),
    }
}
