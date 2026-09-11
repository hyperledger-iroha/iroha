//! Exact reporter-incarnation, ordering and accounting validation.
use super::*;

fn checkpoint() -> SoraServiceLeaseEgressCheckpointV1 {
    SoraServiceLeaseEgressCheckpointV1 {
        reporting_epoch: 1,
        assignment: SoraServiceLeaseReporterAssignmentV1 {
            schema_version: SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1,
            service_version: "1.0.0".to_owned(),
            placement: SoraInrouReplicaPlacementV1 {
                replica_slot: 1,
                economic_clock: SoraServiceLeaseClockV1::CanonicalBlockHeight,
                lease_started_height: 10,
                placement_incarnation: Hash::new(b"checkpoint-placement"),
                host_availability: SoraInrouReplicaHostAvailabilityV1::Available,
                validator_account_id: sample_account_id(202),
                peer_id: sample_peer_id(202),
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
            },
            placement_reconciled_at_ms: 1,
        },
        accounted_egress_bytes: 7,
        last_updated_height: 1,
        finalize_reporter: false,
    }
}

#[test]
fn checkpoint_phase_binds_epoch_replica_and_exact_lease_incarnation() {
    let mut baseline = sample_hosted_service_lease(1);
    baseline.egress_reporter_checkpoints.push(checkpoint());
    baseline.refresh_accounted_egress_bytes().unwrap();
    assert_eq!(baseline.validate(), Ok(()));
    for changed_coordinate in 0..4 {
        let mut lease = baseline.clone();
        let row = &mut lease.egress_reporter_checkpoints[0];
        match changed_coordinate {
            0 => row.reporting_epoch += 1,
            1 => row.assignment.placement.replica_slot += 1,
            2 => row.assignment.placement.lease_started_height += 1,
            3 => row.last_updated_height = 0,
            _ => unreachable!(),
        }
        assert_soracloud_invalid_field(
            lease.validate().unwrap_err(),
            "egress_reporter_checkpoints",
        );
    }
}

#[test]
fn checkpoint_count_and_order_precede_exact_aggregate_accounting() {
    let mut lease = sample_hosted_service_lease(1);
    lease.settled_egress_bytes = 5;
    lease.egress_reporter_checkpoints.push(checkpoint());
    lease.refresh_accounted_egress_bytes().unwrap();
    assert_eq!(lease.accounted_egress_bytes, 12);
    assert_eq!(lease.validate(), Ok(()));
    let mut next = checkpoint();
    next.assignment.service_version = "2.0.0".to_owned();
    lease.egress_reporter_checkpoints.push(next);
    lease.refresh_accounted_egress_bytes().unwrap();
    assert_eq!(lease.validate(), Ok(()));
    lease.accounted_egress_bytes = 0;
    lease.egress_reporter_checkpoints.swap(0, 1);
    assert_soracloud_invalid_field(lease.validate().unwrap_err(), "egress_reporter_checkpoints");
    lease.egress_reporter_checkpoints.swap(0, 1);
    assert_soracloud_invalid_field(lease.validate().unwrap_err(), "accounted_egress_bytes");
    lease.egress_reporter_checkpoints =
        vec![checkpoint(); SORA_SERVICE_LEASE_MAX_EGRESS_REPORTER_CHECKPOINTS_V1 + 1];
    assert_eq!(
        lease.validate(),
        Err(invalid_field(
            "sora service lease state",
            "egress_reporter_checkpoints",
            "exceeds the protocol reporter checkpoint limit"
        ))
    );
    lease.egress_reporter_checkpoints.truncate(1);
    lease.settled_egress_bytes = u128::MAX;
    assert_soracloud_invalid_field(lease.validate().unwrap_err(), "accounted_egress_bytes");
}
