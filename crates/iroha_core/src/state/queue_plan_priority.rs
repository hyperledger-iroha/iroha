//! Canonical first-admission registry policy owned by replicated State.

use super::*;
pub(crate) use iroha_data_model::block::lane_consensus::QueuePlanAdmissionPriorityV1;

impl From<iroha_data_model::block::lane_consensus::LaneAdmissionPriorityError>
    for MergeLedgerCommitError
{
    fn from(error: iroha_data_model::block::lane_consensus::LaneAdmissionPriorityError) -> Self {
        Self::ExecutionBatchInvalid(error.to_string())
    }
}

/// Stored registry owner, including the carrier-assigned order absent from ingress.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::state::QueuePlanAdmissionRegistryRecordV1")]
pub(crate) struct QueuePlanAdmissionRegistryRecordV1 {
    pub(super) version: u16,
    pub(crate) claim: crate::torii_proxy::QueuePlanAdmissionRegistryValueV1,
    pub(crate) priority: QueuePlanAdmissionPriorityV1,
}

/// Exact pending binding and its State-owned first-admission position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RankedQueuePlanPendingBindingV1 {
    /// Immutable position from the authenticated registry owner.
    pub(crate) priority: QueuePlanAdmissionPriorityV1,
    /// Full exact binding whose pending route membership was checked.
    pub(crate) binding: crate::torii_proxy::QueuePlanAdmissionBindingV1,
}

impl State {
    /// Give direct unit-test marker fixtures distinct positions without claiming
    /// a production carrier application. Native priority tests use real staging.
    #[cfg(test)]
    pub(crate) fn queue_plan_fixture_priority_for_binding(
        &self,
        binding: &crate::torii_proxy::QueuePlanAdmissionBindingV1,
    ) -> Result<QueuePlanAdmissionPriorityV1, MergeLedgerCommitError> {
        let storage = self.world.smart_contract_state.view();
        let key = Self::queue_plan_admission_registry_marker_key(&binding.registry_key())?;
        if let Some(payload) = storage.get(&key) {
            return Self::decode_exact_queue_plan_admission_registry_record(&key, payload)
                .map(|record| record.priority);
        }
        let height = binding.admission_context.proposal_height;
        let mut next = 0usize;
        for (key, payload) in storage.iter() {
            if !key
                .as_ref()
                .starts_with(QUEUE_PLAN_ADMISSION_REGISTRY_MARKER_PREFIX)
            {
                continue;
            }
            let record = Self::decode_exact_queue_plan_admission_registry_record(key, payload)?;
            if record.priority.carrier_height == height {
                next = next.max(record.priority.admission_index as usize + 1);
            }
        }
        QueuePlanAdmissionPriorityV1::new(height, next).map_err(Into::into)
    }

    /// Encode the single first-release State registry layout.
    pub(crate) fn queue_plan_admission_registry_marker_payload(
        registry_value: &crate::torii_proxy::QueuePlanAdmissionRegistryValueV1,
        priority: QueuePlanAdmissionPriorityV1,
    ) -> Result<Vec<u8>, MergeLedgerCommitError> {
        priority.validate()?;
        if registry_value.version != crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1
            || registry_value
                .binding_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "queue-plan admission registry value is malformed".to_owned(),
            ));
        }
        let record = QueuePlanAdmissionRegistryRecordV1 {
            version: 1,
            claim: *registry_value,
            priority,
        };
        let payload = norito::encode_canonical(&record).map_err(|error| {
            MergeLedgerCommitError::ExecutionBatchInvalid(format!(
                "queue-plan admission registry record cannot be encoded: {error}"
            ))
        })?;
        if payload.is_empty() || payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "queue-plan admission registry record is empty or oversized".to_owned(),
            ));
        }
        Ok(payload)
    }

    pub(crate) fn decode_exact_queue_plan_admission_registry_record(
        key: &StatePath,
        payload: &[u8],
    ) -> Result<QueuePlanAdmissionRegistryRecordV1, MergeLedgerCommitError> {
        if payload.is_empty() || payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES {
            return Err(MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                "queue-plan admission registry marker `{key}` is empty or oversized"
            )));
        }
        let record = norito::decode_canonical::<QueuePlanAdmissionRegistryRecordV1>(payload)
            .map_err(|_| {
                MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                    "queue-plan admission registry marker `{key}` is not exact canonical Norito"
                ))
            })?;
        if record.version != 1
            || Self::queue_plan_admission_registry_marker_payload(&record.claim, record.priority)?
                .as_slice()
                != payload
        {
            return Err(MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                "queue-plan admission registry marker `{key}` is not canonical"
            )));
        }
        Ok(record)
    }

    // Existing identity/application readers use this projection of the one
    // validated record. There is no alternate rankless storage or decode path.
    pub(super) fn decode_exact_queue_plan_admission_registry_marker(
        key: &StatePath,
        payload: &[u8],
    ) -> Result<crate::torii_proxy::QueuePlanAdmissionRegistryValueV1, MergeLedgerCommitError> {
        Self::decode_exact_queue_plan_admission_registry_record(key, payload)
            .map(|record| record.claim)
    }

    /// Read exact pending route bindings in global order under a caller-authenticated cut.
    ///
    /// `carrier_height` is the completed carrier being validated, including when
    /// its StateBlock hash journal is still at H-1. The caller authenticates that
    /// height through block validation or snapshot recovery. This lower boundary
    /// checks storage ownership; it does not claim transaction-membership finality.
    pub(super) fn queue_plan_pending_route_at_admission_cut_from_storage(
        storage: &impl StorageReadOnly<StatePath, Vec<u8>>,
        network_id: &iroha_data_model::NetworkId,
        route: QueuePlanPendingObligationRouteV1,
        opening_global_height: u64,
        carrier_height: u64,
    ) -> Result<Vec<RankedQueuePlanPendingBindingV1>, String> {
        if opening_global_height == 0 || opening_global_height > carrier_height {
            return Err("QueuePlan opening admission cut exceeds its validated carrier".to_owned());
        }
        Self::validate_queue_plan_pending_obligation_route(&route)
            .map_err(|error| error.to_string())?;
        let network = crate::torii_proxy::queue_plan_admission_network_id_digest(network_id);
        let members = Self::queue_plan_pending_route_members_from_storage(storage, route)
            .map_err(|error| error.to_string())?;
        let mut ranked = Vec::with_capacity(members.len());
        let mut positions = BTreeSet::new();
        for (_, member) in members {
            let obligation_key = Self::queue_plan_pending_obligation_marker_key(
                member.network_id_digest,
                member.entrypoint_hash,
            )
            .map_err(|error| error.to_string())?;
            let obligation_payload = storage
                .get(&obligation_key)
                .ok_or_else(|| "QueuePlan ranked route member lost its obligation".to_owned())?;
            let obligation = Self::decode_exact_queue_plan_pending_obligation_marker(
                &obligation_key,
                obligation_payload,
            )
            .map_err(|error| error.to_string())?;
            let binding = obligation.binding.clone();
            if binding.network_id_digest != network
                || member.network_id_digest != network
                || binding.canonical_hash() != member.binding_hash
                || Self::queue_plan_pending_exact_route_member_state_in_storage(
                    storage,
                    &obligation,
                )
                .map_err(|error| error.to_string())?
                    != QueuePlanPendingRouteMemberState::AllPresent
            {
                return Err(
                    "QueuePlan ranked route member differs from its exact pending owner".to_owned(),
                );
            }
            Self::require_queue_plan_pending_signed_alias_member_marker(storage, &obligation)
                .map_err(|error| error.to_string())?;
            let terminal = Self::queue_plan_signed_alias_terminal_marker_key_from_claim(
                binding.network_id_digest,
                binding.entrypoint_hash,
            )
            .map_err(|error| error.to_string())?;
            if storage.get(&terminal).is_some() {
                return Err(
                    "QueuePlan ranked pending owner retains terminal resolution evidence"
                        .to_owned(),
                );
            }
            let key = Self::queue_plan_admission_registry_marker_key(&binding.registry_key())
                .map_err(|error| error.to_string())?;
            let payload = storage.get(&key).ok_or_else(|| {
                "QueuePlan ranked route member lost its registry owner".to_owned()
            })?;
            let record = Self::decode_exact_queue_plan_admission_registry_record(&key, payload)
                .map_err(|error| error.to_string())?;
            if record.claim != binding.registry_value()
                || record.priority.carrier_height < binding.admission_context.proposal_height
                || record.priority.carrier_height > carrier_height
                || !positions.insert(record.priority)
            {
                return Err(
                    "QueuePlan admission priority differs from its exact owner/carrier or repeats"
                        .to_owned(),
                );
            }
            if record.priority.carrier_height <= opening_global_height {
                ranked.push(RankedQueuePlanPendingBindingV1 {
                    priority: record.priority,
                    binding,
                });
            }
        }
        ranked.sort_by_key(|entry| entry.priority);
        Ok(ranked)
    }

    /// Add complete transaction-membership/lifecycle checks to the storage projection.
    ///
    /// This is a fresh read, not a scheduler or a vote authorization. A future
    /// native validator must require the selected group to be the pinned head on
    /// every affected route before an irreversible proposal or Prepare.
    pub(crate) fn queue_plan_pending_route_at_admission_cut(
        state: &impl StateReadOnlyWithTransactions,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        incarnation: Hash,
        opening_global_height: u64,
        carrier_height: u64,
    ) -> Result<Vec<RankedQueuePlanPendingBindingV1>, String> {
        let ranked = Self::queue_plan_pending_route_at_admission_cut_from_storage(
            state.world().smart_contract_state(),
            state.network_id(),
            QueuePlanPendingObligationRouteV1 {
                version: QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1,
                lane_id,
                dataspace_id,
                lane_incarnation: incarnation,
            },
            opening_global_height,
            carrier_height,
        )?;
        for entry in &ranked {
            if Self::queue_plan_binding_application_evidence_in_view(state, &entry.binding)?
                != QueuePlanBindingApplicationEvidence::Pending
            {
                return Err(
                    "QueuePlan ranked route member is not pending in the exact State".to_owned(),
                );
            }
        }
        Ok(ranked)
    }

    /// Return the first exact unresolved binding in a frozen opening's admission cut.
    pub(crate) fn queue_plan_pending_route_head_at_admission_cut(
        state: &impl StateReadOnlyWithTransactions,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        incarnation: Hash,
        opening_global_height: u64,
        carrier_height: u64,
    ) -> Result<Option<RankedQueuePlanPendingBindingV1>, String> {
        Self::queue_plan_pending_route_at_admission_cut(
            state,
            lane_id,
            dataspace_id,
            incarnation,
            opening_global_height,
            carrier_height,
        )
        .map(|ranked| ranked.into_iter().next())
    }
}
