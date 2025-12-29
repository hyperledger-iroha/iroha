//! Generates reference Norito fixtures for replication orders.

use std::{error::Error, fs, path::PathBuf};

use hex::encode;
use sorafs_manifest::{
    CapacityMetadataEntry,
    capacity::{
        REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
        ReplicationOrderV1,
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let fixture_dir = PathBuf::from("fixtures/sorafs_manifest/replication_order");
    fs::create_dir_all(&fixture_dir)?;

    let order = ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: [0xAB; 32],
        manifest_cid: b"bafyreplicaexamplecidroot".to_vec(),
        manifest_digest: [0x42; 32],
        chunking_profile: "sorafs.sf1@1.0.0".to_string(),
        target_replicas: 2,
        assignments: vec![
            ReplicationAssignmentV1 {
                provider_id: [0x10; 32],
                slice_gib: 512,
                lane: Some("lane-primary".to_string()),
            },
            ReplicationAssignmentV1 {
                provider_id: [0x11; 32],
                slice_gib: 512,
                lane: Some("lane-secondary".to_string()),
            },
        ],
        issued_at: 1_700_000_000,
        deadline_at: 1_700_086_400,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 86_400,
            min_availability_percent_milli: 99_500,
            min_por_success_percent_milli: 98_000,
        },
        metadata: vec![CapacityMetadataEntry {
            key: "governance.ticket".to_string(),
            value: "ticket-sorafs-0001".to_string(),
        }],
    };

    order
        .validate()
        .expect("hard-coded fixture must satisfy validation");

    let bytes = norito::to_bytes(&order)?;
    fs::write(fixture_dir.join("order_v1.to"), &bytes)?;

    let json = format!(
        "{{\n  \"schema_version\": {},\n  \"order_id_hex\": \"{}\",\n  \"manifest_cid\": \"{}\",\n  \"manifest_digest_hex\": \"{}\",\n  \"target_replicas\": {},\n  \"assignments\": [\n    {{ \"provider_id_hex\": \"{}\", \"slice_gib\": {}, \"lane\": \"{}\" }},\n    {{ \"provider_id_hex\": \"{}\", \"slice_gib\": {}, \"lane\": \"{}\" }}\n  ],\n  \"issued_at\": {},\n  \"deadline_at\": {},\n  \"sla\": {{ \"ingest_deadline_secs\": {}, \"min_availability_percent_milli\": {}, \"min_por_success_percent_milli\": {} }},\n  \"metadata\": [ {{ \"key\": \"{}\", \"value\": \"{}\" }} ],\n  \"norito_bytes_hex\": \"{}\"\n}}\n",
        order.version,
        encode(order.order_id),
        String::from_utf8_lossy(&order.manifest_cid),
        encode(order.manifest_digest),
        order.target_replicas,
        encode(order.assignments[0].provider_id),
        order.assignments[0].slice_gib,
        order.assignments[0]
            .lane
            .as_ref()
            .map_or("", String::as_str),
        encode(order.assignments[1].provider_id),
        order.assignments[1].slice_gib,
        order.assignments[1]
            .lane
            .as_ref()
            .map_or("", String::as_str),
        order.issued_at,
        order.deadline_at,
        order.sla.ingest_deadline_secs,
        order.sla.min_availability_percent_milli,
        order.sla.min_por_success_percent_milli,
        order.metadata[0].key,
        order.metadata[0].value,
        encode(&bytes),
    );
    fs::write(fixture_dir.join("order_v1.json"), json)?;

    Ok(())
}
