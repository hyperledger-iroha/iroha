//! Borrowed lifecycle evidence from the same immutable World source.
//!
//! Validation preserves archive, binding and identity error order. A retired
//! provider set remains with the original row; no authority payload is copied.

use super::*;

/// Check the complete binding before borrowing its existing lifecycle.
pub(super) fn validate_replication_order_archive_binding<'world>(
    archive: &MusubiArchiveRecordV1,
    replication_order: &iroha_data_model::sorafs::pin_registry::ReplicationOrderId,
    world: &'world impl WorldReadOnly,
) -> Result<&'world MusubiReplicationOrderLocationLifecycleV1, Error> {
    archive
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    let reference = world
        .musubi_locations_by_replication_order()
        .get(replication_order)
        .ok_or_else(|| invariant("Musubi replication order has no consensus archive binding"))?;
    reference
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if reference.binding.replication_order != *replication_order
        || reference.binding.archive_id != archive.archive_id
        || reference.binding.commitment != archive.commitment
    {
        return Err(invariant(
            "Musubi replication-order binding does not match the authoritative archive commitment",
        ));
    }
    Ok(&reference.lifecycle)
}
