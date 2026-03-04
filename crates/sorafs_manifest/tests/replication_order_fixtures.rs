#![allow(unexpected_cfgs)]

use std::{fs, str};

use sorafs_manifest::{REPLICATION_ORDER_VERSION_V1, ReplicationOrderV1};

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest"
);

fn read_fixture_bytes(path: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn read_fixture_string(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

#[test]
fn replication_order_fixture_roundtrip() {
    let bytes = read_fixture_bytes(&format!("{FIXTURES_ROOT}/replication_order/order_v1.to"));
    let order: ReplicationOrderV1 =
        norito::decode_from_bytes(&bytes).expect("fixture should decode via Norito");

    order.validate().expect("fixture order must validate");

    assert_eq!(
        order.version, REPLICATION_ORDER_VERSION_V1,
        "schema version drifted"
    );
    assert_eq!(
        hex::encode(order.order_id),
        "abababababababababababababababababababababababababababababababab",
        "fixture order id changed"
    );
    let manifest_cid = str::from_utf8(&order.manifest_cid).expect("manifest CID should be UTF-8");
    assert_eq!(
        manifest_cid, "bafyreplicaexamplecidroot",
        "fixture manifest CID changed"
    );
    assert_eq!(
        hex::encode(order.manifest_digest),
        "4242424242424242424242424242424242424242424242424242424242424242",
        "fixture manifest digest changed"
    );
    assert_eq!(
        order.chunking_profile, "sorafs.sf1@1.0.0",
        "fixture chunker handle changed"
    );
    assert_eq!(order.target_replicas, 2, "replica target drifted");
    assert_eq!(order.assignments.len(), 2, "assignment count changed");

    let first = &order.assignments[0];
    assert_eq!(
        hex::encode(first.provider_id),
        "1010101010101010101010101010101010101010101010101010101010101010",
        "first provider id changed"
    );
    assert_eq!(first.slice_gib, 512, "first slice gib changed");
    assert_eq!(
        first.lane.as_deref(),
        Some("lane-primary"),
        "first lane hint changed"
    );

    let second = &order.assignments[1];
    assert_eq!(
        hex::encode(second.provider_id),
        "1111111111111111111111111111111111111111111111111111111111111111",
        "second provider id changed"
    );
    assert_eq!(second.slice_gib, 512, "second slice gib changed");
    assert_eq!(
        second.lane.as_deref(),
        Some("lane-secondary"),
        "second lane hint changed"
    );

    assert_eq!(order.issued_at, 1_700_000_000, "fixture issued_at changed");
    assert_eq!(
        order.deadline_at, 1_700_086_400,
        "fixture deadline_at changed"
    );
    assert_eq!(
        order.sla.ingest_deadline_secs, 86_400,
        "fixture SLA ingest deadline changed"
    );
    assert_eq!(
        order.sla.min_availability_percent_milli, 99_500,
        "fixture SLA availability changed"
    );
    assert_eq!(
        order.sla.min_por_success_percent_milli, 98_000,
        "fixture SLA PoR threshold changed"
    );

    assert_eq!(order.metadata.len(), 1, "metadata entry count changed");
    let meta = &order.metadata[0];
    assert_eq!(meta.key, "governance.ticket", "metadata key changed");
    assert_eq!(meta.value, "ticket-sorafs-0001", "metadata value changed");

    let reencoded = norito::to_bytes(&order).expect("fixture should round-trip via Norito");
    assert_eq!(
        reencoded, bytes,
        "re-encoded bytes differ from fixture payload"
    );

    let json_text =
        read_fixture_string(&format!("{FIXTURES_ROOT}/replication_order/order_v1.json"));
    let json_value =
        norito::json::parse_value(&json_text).expect("fixture commentary must be valid JSON");
    let norito_hex = json_value
        .get("norito_bytes_hex")
        .and_then(|value| value.as_str())
        .expect("fixture commentary must contain `norito_bytes_hex` string");
    let norito_bytes =
        hex::decode(norito_hex).expect("fixture commentary must contain valid hex payload");
    assert_eq!(
        norito_bytes, reencoded,
        "`norito_bytes_hex` comment drifted from fixture bytes"
    );
}
