//! Status-path visibility helpers for telemetry routes.
pub(super) fn is_nexus_status_segment(tail: &str) -> bool {
    let mut segments = tail.split('/').filter(|segment| !segment.is_empty());
    matches!(
        segments.next(),
        Some(
            "teu_lane_commit"
                | "teu_dataspace_backlog"
                | "dataspace_catalog"
                | "nexus"
                | "tx_gossip"
                | "da_receipt_cursors"
        )
    )
}
