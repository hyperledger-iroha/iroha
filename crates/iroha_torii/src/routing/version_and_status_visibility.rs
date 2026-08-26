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
    authoritative_block_height: u64,
) -> std::result::Result<(), Error> {
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
#[cfg(all(test, feature = "telemetry"))]
mod status_block_visibility_tests {
    use super::{Error, Status, ensure_status_metrics_match_authoritative_height};
    #[test]
    fn stale_classified_height_is_retriable_instead_of_publishing_a_false_empty_gap() {
        let status = Status {
            blocks: 2,
            blocks_non_empty: 2,
            ..Status::default()
        };
        let error = ensure_status_metrics_match_authoritative_height(&status, 3)
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
        ensure_status_metrics_match_authoritative_height(&status, 3)
            .expect("a single classified frontier is publishable");
    }
}
