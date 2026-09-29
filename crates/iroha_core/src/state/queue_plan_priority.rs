//! Canonical first-admission registry policy owned by replicated State.

use super::*;

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
}
