//! Small certified runtime originals for downstream lifecycle-consumer tests.

use super::*;

impl State {
    /// Capture typed runtime originals and a selective native witness branch.
    ///
    /// This helper reads no complete World snapshot. Its values remain untrusted
    /// until the consumer matches the native branch to the certified result.
    /// # Errors
    /// The current cut differs, original archive is absent, or bounded proof work refuses.
    pub fn lifecycle_activation_witness_for_testing(
        &self,
        tip: &CommittedBlock,
    ) -> Result<String, WorldStateSnapshotError> {
        let view = self.view();
        if view
            .native_execution_tip()
            .is_none_or(|native| native.height() != tip.height())
        {
            return Err("lifecycle fixture requires its exact current certified cut".into());
        }
        let proof = crate::query::native_receipts::lifecycle_runtime_proof_for_testing(
            &view,
            tip.height(),
        )?;
        let world = view.world();
        let selection = world.runtime_selection();
        let services = world
            .runtime_definitions()
            .get(&selection.services_profile)
            .ok_or("lifecycle fixture services profile is absent")?;
        let implementation_id = selection
            .bindings
            .first()
            .ok_or("lifecycle fixture binding is absent")?
            .implementation;
        let implementation = world
            .runtime_definitions()
            .get(&implementation_id)
            .ok_or("lifecycle fixture implementation is absent")?;
        let runtime_proof = proof.proof();
        let witness = norito::json!({
            "runtime_proof": runtime_proof,
            "selection": selection,
            "services": services,
            "implementation": implementation,
        });
        norito::json::to_json_bounded(&witness, 32 * 1024 * 1024)
            .map_err(|error| WorldStateSnapshotError::from(error.to_string()))
    }
}
