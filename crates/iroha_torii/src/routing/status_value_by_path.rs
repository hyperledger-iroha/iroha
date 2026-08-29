#[cfg(feature = "telemetry")]
fn status_value_by_path(status: &Status, tail: &str) -> Option<norito::json::Value> {
    let mut segments = tail.split('/').filter(|s| !s.is_empty());
    let field = segments.next()?;
    match field {
        "observed_at_ms" if segments.next().is_none() => Some(status.observed_at_ms.into()),
        "peers" if segments.next().is_none() => Some(status.peers.into()),
        "blocks" if segments.next().is_none() => Some(status.blocks.into()),
        "blocks_non_empty" if segments.next().is_none() => Some(status.blocks_non_empty.into()),
        "commit_time_ms" if segments.next().is_none() => Some(status.commit_time_ms.into()),
        "txs_approved" if segments.next().is_none() => Some(status.txs_approved.into()),
        "txs_rejected" if segments.next().is_none() => Some(status.txs_rejected.into()),
        "last_rejection_at_ms" if segments.next().is_none() => {
            Some(json_value(&status.last_rejection_at_ms))
        }
        "txs_rejected_recent_5m" if segments.next().is_none() => {
            Some(status.txs_rejected_recent_5m.into())
        }
        "uptime" => {
            let duration = status.uptime.0;
            match segments.next() {
                None => Some(crate::json_object(vec![
                    ("secs", duration.as_secs()),
                    ("nanos", u64::from(duration.subsec_nanos())),
                ])),
                Some("secs") if segments.next().is_none() => Some(duration.as_secs().into()),
                Some("nanos") if segments.next().is_none() => {
                    Some(u64::from(duration.subsec_nanos()).into())
                }
                _ => None,
            }
        }
        "da_receipt_cursors" if segments.next().is_none() => Some(
            status
                .da_receipt_cursors
                .iter()
                .map(|cursor| {
                    crate::json_object(vec![
                        ("lane_id", json_value(&cursor.lane_id)),
                        ("epoch", json_value(&cursor.epoch)),
                        ("highest_sequence", json_value(&cursor.highest_sequence)),
                    ])
                })
                .collect::<Vec<_>>()
                .into(),
        ),
        "view_changes" if segments.next().is_none() => Some(status.view_changes.into()),
        "queue_size" if segments.next().is_none() => Some(status.queue_size.into()),
        "queue_queued" if segments.next().is_none() => Some(status.queue_queued.into()),
        "queue_inflight" if segments.next().is_none() => Some(status.queue_inflight.into()),
        "last_block_committed_at_ms" if segments.next().is_none() => {
            Some(status.last_block_committed_at_ms.into())
        }
        "last_non_empty_block_committed_at_ms" if segments.next().is_none() => {
            Some(status.last_non_empty_block_committed_at_ms.into())
        }
        "time_since_last_block_ms" if segments.next().is_none() => {
            Some(status.time_since_last_block_ms.into())
        }
        "time_since_last_non_empty_block_ms" if segments.next().is_none() => {
            Some(status.time_since_last_non_empty_block_ms.into())
        }
        "crypto" => match segments.next() {
            None => norito::json::to_value(&status.crypto).ok(),
            Some("sm_helpers_available") if segments.next().is_none() => {
                Some(status.crypto.sm_helpers_available.into())
            }
            Some("sm_openssl_preview_enabled") if segments.next().is_none() => {
                Some(status.crypto.sm_openssl_preview_enabled.into())
            }
            _ => None,
        },
        "offline" => {
            let offline = status.offline.as_ref()?;
            let value = norito::json::to_value(offline).ok()?;
            json_value_by_segments(value, segments)
        }
        "governance" if segments.next().is_none() => {
            norito::json::to_value(&status.governance).ok()
        }
        "dataspace_catalog" if segments.next().is_none() => {
            norito::json::to_value(&status.dataspace_catalog).ok()
        }
        "nexus" => {
            let nexus = status.nexus.as_ref()?;
            match segments.next() {
                None => norito::json::to_value(nexus).ok(),
                Some("routing_policy") => {
                    let routing = &nexus.routing_policy;
                    match segments.next() {
                        None => norito::json::to_value(routing).ok(),
                        Some("default_lane") if segments.next().is_none() => {
                            Some(routing.default_lane.into())
                        }
                        Some("default_dataspace") if segments.next().is_none() => {
                            Some(routing.default_dataspace.into())
                        }
                        Some("rules") if segments.next().is_none() => {
                            norito::json::to_value(&routing.rules).ok()
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}
