fn transaction_network_id(seed: u8) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([seed; iroha_crypto::Hash::LENGTH]),
    ))
}
#[test]
fn finalized_capacity_projection_has_no_production_local_mutation_entrypoints() {
    let source = include_str!("lib.rs");
    for (prefix, method) in [
        ("pub fn ", "record_capacity_declaration"),
        ("pub fn ", "schedule_replication_order"),
        ("pub fn ", "complete_replication_order"),
        ("fn ", "record_uptime_observation"),
        ("fn ", "record_por_observation"),
        ("fn ", "record_replication_failure"),
    ] {
        let retired = format!("{prefix}{method}(");
        assert!(
            !source.contains(&retired),
            "retired local capacity mutation entrypoint reappeared: {retired}"
        );
    }
    for method in [
        "record_capacity_declaration",
        "schedule_replication_order",
        "complete_replication_order",
    ] {
        let test_only = format!("pub(crate) fn {method}(");
        let offset = source.find(&test_only).expect("test-only capacity helper");
        let prefix = &source[offset.saturating_sub(32)..offset];
        assert!(
            prefix.contains("#[cfg(test)]"),
            "capacity mutation helper must remain cfg(test)-gated: {test_only}"
        );
    }
}
#[test]
fn potr_receipt_admission_requires_explicit_chain_authoritative_handoff() {
    let source = include_str!("lib.rs");
    let retired_default_impl = ["impl potr::PotrLatencyRepairHandoff", " for NodeHandle"].concat();
    let retired_implicit_api = ["pub fn ", "record_potr_receipt", "("].concat();
    let explicit_api = ["pub fn ", "record_potr_receipt_with_handoff", "("].concat();
    assert!(
        !source.contains(&retired_default_impl),
        "NodeHandle must not provide a partial process-local PoTR repair handoff"
    );
    assert!(
        !source.contains(&retired_implicit_api),
        "PoTR receipt admission must not select a fallback handoff implicitly"
    );
    assert!(
        source.contains(&explicit_api),
        "PoTR receipt admission must require an explicit chain-authoritative handoff"
    );
}
#[test]
fn appeal_finance_publication_has_no_unauthenticated_node_handle_entrypoint() {
    let source = include_str!("lib.rs");
    for method in ["appeal_finance_report", "appeal_finance_weekly_rollup"] {
        let retired = format!("pub fn publish_{method}(");
        assert!(
            !source.contains(&retired),
            "unauthenticated finance publication entrypoint reappeared: {retired}"
        );
        let authenticated = format!("pub fn publish_authenticated_{method}(");
        assert!(
            source.contains(&authenticated),
            "authenticated finance publication entrypoint is missing: {authenticated}"
        );
    }
}
#[derive(Debug)]
struct SuccessfulPorRepairHandoff;
impl PorRepairHandoff for SuccessfulPorRepairHandoff {
    fn enqueue_failed_por_repair(
        &self,
        intent: &PorFailedRepairIntentV1,
    ) -> Result<[u8; 32], PorRepairHandoffError> {
        Ok(intent.repair_task_id())
    }
}
#[derive(Debug)]
struct FailingPorRepairHandoff;
impl PorRepairHandoff for FailingPorRepairHandoff {
    fn enqueue_failed_por_repair(
        &self,
        _intent: &PorFailedRepairIntentV1,
    ) -> Result<[u8; 32], PorRepairHandoffError> {
        Err(PorRepairHandoffError(
            "injected repair admission failure".to_owned(),
        ))
    }
}
#[derive(Debug, Default)]
struct RecordingReputationAdmission {
    retained: Mutex<
        Option<(
            ProviderId,
            iroha_data_model::sorafs::reputation::PorTerminalOutcomeV1,
        )>,
    >,
    calls: AtomicU64,
}
impl reputation::runtime::ReputationNativeOutcomeAdmissionApiV1 for RecordingReputationAdmission {
    fn activation_state(
        &self,
    ) -> Result<
        reputation::runtime::ReputationNativeOutcomeAdmissionStateV1,
        reputation::runtime::ReputationRuntimeError,
    > {
        Ok(reputation::runtime::ReputationNativeOutcomeAdmissionStateV1::Active)
    }
    fn record_por_terminal(
        &self,
        provider_id: ProviderId,
        outcome: iroha_data_model::sorafs::reputation::PorTerminalOutcomeV1,
    ) -> Result<
        reputation::runtime::ReputationJournalEnqueueOutcomeV1,
        reputation::runtime::ReputationRuntimeError,
    > {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let mut retained = self
            .retained
            .lock()
            .map_err(|_| reputation::runtime::ReputationRuntimeError::RuntimePoisoned)?;
        let event_id = iroha_data_model::sorafs::reputation::ReputationJournalEventIdV1([0xE1; 32]);
        match retained.as_ref() {
            None => {
                *retained = Some((provider_id, outcome));
                Ok(reputation::runtime::ReputationJournalEnqueueOutcomeV1::Inserted { event_id })
            }
            Some((retained_provider, retained_outcome))
                if *retained_provider == provider_id && *retained_outcome == outcome =>
            {
                Ok(
                    reputation::runtime::ReputationJournalEnqueueOutcomeV1::ExactReplay {
                        event_id,
                    },
                )
            }
            Some(_) => Err(reputation::runtime::ReputationRuntimeError::JournalSourceConflict),
        }
    }
    fn record_authenticated_stream_token_validation(
        &self,
        _provider_id: ProviderId,
        _outcome: iroha_data_model::sorafs::reputation::StreamTokenValidationOutcomeV1,
    ) -> Result<
        reputation::runtime::StreamTokenReputationAdmissionOutcomeV1,
        reputation::runtime::ReputationRuntimeError,
    > {
        Err(reputation::runtime::ReputationRuntimeError::RuntimeBindingMismatch)
    }
}
