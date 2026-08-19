/// Get running Iroha version (block header version).
#[iroha_futures::telemetry_future]
pub async fn handle_version(state: Arc<CoreState>) -> Response {
    use iroha_version::Version;
    let latest_block = std::num::NonZeroUsize::new(state.committed_height())
        .and_then(|height| state.block_by_height(height));
    let mut resp = match latest_block {
        Some(block) => Response::new(Body::from(block.version().to_string())),
        None => {
            let mut resp = Response::new(Body::from("genesis not applied"));
            *resp.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
            resp
        }
    };
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    resp
}
// Version and status visibility helpers and regressions.
#[cfg(test)]
mod version_tests {
    use super::*;
    use http_body_util::BodyExt as _;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    #[tokio::test]
    async fn handle_version_reports_unavailable_without_genesis() {
        let state = Arc::new(CoreState::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let response = handle_version(state).await;
        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/plain; charset=utf-8"),
        );
        let body_bytes = body.collect().await.expect("collect body").to_bytes();
        assert_eq!(
            std::str::from_utf8(&body_bytes).expect("utf8"),
            "genesis not applied"
        );
    }
}
#[cfg(feature = "telemetry")]
fn ensure_status_metrics_match_authoritative_height(
    status: &Status,
    authoritative_block_height: Option<u64>,
) -> std::result::Result<(), Error> {
    let Some(authoritative_block_height) = authoritative_block_height else {
        return Ok(());
    };
    if status.blocks != authoritative_block_height {
        return Err(Error::AppServiceUnavailable {
            code: "status_metrics_stale",
            message: format!(
                "status metrics classified height {} while applied state is at height {authoritative_block_height}; retry",
                status.blocks
            ),
        });
    }
    Ok(())
}
#[cfg(feature = "telemetry")]
/// Anchor the public chain-height field to applied state.
///
/// The Prometheus block counter is populated by a lazy Kura scan and can trail
/// while a peer applies a catch-up batch. Kura is also persisted before the WSV
/// commit boundary, so that counter can briefly lead query-visible state. The
/// state block-hash journal publishes query-visible committed height on the
/// apply path, so `/status.blocks` must use that height exactly whenever the
/// handler provides it. Direct callers without a state anchor retain the legacy
/// monotonic CommitQC fallback.
fn normalize_status_block_visibility(status: &mut Status, authoritative_block_height: Option<u64>) {
    let telemetry_commit_height = status
        .sumeragi
        .as_ref()
        .map_or(0, |sumeragi| sumeragi.commit_qc_height);
    status.blocks =
        authoritative_block_height.unwrap_or_else(|| status.blocks.max(telemetry_commit_height));
}
#[cfg(all(test, feature = "telemetry"))]
mod status_block_visibility_tests {
    use super::{
        Error, Status, ensure_status_metrics_match_authoritative_height,
        normalize_status_block_visibility,
    };
    use iroha_telemetry::metrics::SumeragiConsensusStatus;
    #[test]
    fn stale_classified_height_is_retriable_instead_of_publishing_a_false_empty_gap() {
        let status = Status {
            blocks: 2,
            blocks_non_empty: 2,
            ..Status::default()
        };
        let error = ensure_status_metrics_match_authoritative_height(&status, Some(3))
            .expect_err("an authoritative height ahead of classification must be retriable");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "status_metrics_stale",
                ..
            }
        ));
    }
    #[test]
    fn matching_classified_and_authoritative_heights_are_publishable() {
        let status = Status {
            blocks: 3,
            blocks_non_empty: 3,
            ..Status::default()
        };
        ensure_status_metrics_match_authoritative_height(&status, Some(3))
            .expect("a single classified frontier is publishable");
    }
    #[test]
    fn authoritative_state_height_replaces_lagging_and_leading_counters() {
        for telemetry_height in [3, 19] {
            let mut sumeragi = SumeragiConsensusStatus::default();
            sumeragi.commit_qc_height = telemetry_height;
            let mut status = Status {
                blocks: telemetry_height,
                sumeragi: Some(sumeragi),
                ..Status::default()
            };
            normalize_status_block_visibility(&mut status, Some(11));
            assert_eq!(status.blocks, 11);
        }
    }
    #[test]
    fn missing_state_anchor_keeps_monotonic_commit_qc_fallback() {
        let mut sumeragi = SumeragiConsensusStatus::default();
        sumeragi.commit_qc_height = 8;
        let mut status = Status {
            blocks: 5,
            sumeragi: Some(sumeragi),
            ..Status::default()
        };
        normalize_status_block_visibility(&mut status, None);
        assert_eq!(status.blocks, 8);
    }
}
