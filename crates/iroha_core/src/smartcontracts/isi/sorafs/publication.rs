//! Purpose-owned native publication assertion, consumed only through signed execution proofs.

use super::*;
use iroha_data_model::isi::sorafs::AssertSorafsPublicationV1;

impl Execute for AssertSorafsPublicationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let rejected = || {
            invalid_parameter("SoraFS publication assertion does not match current native state")
        };
        let index = usize::try_from(self.minimum_height)
            .ok()
            .and_then(|height| height.checked_sub(1))
            .ok_or_else(rejected)?;
        if self.challenge == [0; 32]
            || self.minimum_block_hash == [0; 32]
            || self.assignment_revision == 0
            || self.canonical_order_digest == [0; 32]
            || state_transaction
                .block_hashes()
                .get(index)
                .map(|hash| *hash.as_ref())
                != Some(self.minimum_block_hash)
            || derive_sorafs_auto_replication_order_id_v1(&self.manifest_digest) != self.order_id
        {
            return Err(rejected());
        }
        let now = pin_consensus_epoch(state_transaction);
        let pin = state_transaction
            .world
            .pin_manifests
            .get(&self.manifest_digest)
            .ok_or_else(rejected)?;
        if &pin.submitted_by != authority
            || !matches!(pin.status, PinStatus::Approved(_))
            || pin.policy.retention_epoch <= now
            || pin.pin_fee_payment.is_none()
        {
            return Err(rejected());
        }
        let order = state_transaction
            .world
            .replication_orders
            .get(&self.order_id)
            .ok_or_else(rejected)?;
        let canonical = validate_stored_automatic_replication_order(
            pin,
            order,
            &hex::encode(self.order_id.as_bytes()),
        )?;
        if order.assignment_revision != self.assignment_revision
            || blake3_hash(&order.canonical_order).as_bytes() != &self.canonical_order_digest
            || order.manifest_digest != self.manifest_digest
            || canonical.target_replicas != pin.policy.min_replicas
            || canonical.assignments.len() != usize::from(pin.policy.min_replicas)
            || match (self.require_complete, order.status) {
                (true, ReplicationOrderStatus::Completed(_)) => {
                    order.provider_completions.len() != canonical.assignments.len()
                }
                (false, ReplicationOrderStatus::Pending) => order.deadline_epoch < now,
                (false, ReplicationOrderStatus::Completed(_)) => false,
                _ => true,
            }
        {
            return Err(rejected());
        }
        Ok(())
    }
}
