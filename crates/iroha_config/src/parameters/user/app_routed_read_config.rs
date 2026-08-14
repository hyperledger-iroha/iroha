//! App routed-read request-admission configuration helpers.
use super::{ParseError, Torii, defaults, emit_torii_config_error};
use iroha_config_base::util::{DurationMs, Emitter};
use std::time::Duration;
pub(super) const fn default_body_timeout() -> DurationMs {
    DurationMs(Duration::from_millis(
        defaults::torii::APP_API_ROUTED_READ_BODY_READ_TIMEOUT_MS,
    ))
}
pub(super) const fn default_query_timeout() -> DurationMs {
    DurationMs(Duration::from_millis(
        defaults::torii::QUERY_QUEUE_TIMEOUT_MS,
    ))
}
pub(super) fn body_timeout(config: &Torii) -> Duration {
    config.app_api_routed_read_body_read_timeout_ms.get()
}
pub(super) fn validate(config: &Torii, emitter: &mut Emitter<ParseError>) {
    if body_timeout(config).is_zero() {
        emit_torii_config_error(
            emitter,
            "torii.app_api_routed_read_body_read_timeout_ms must be at least 1 ms",
        );
    }
    let route_body = defaults::torii::app_api_routed_read_route_body_phase_bytes(
        config.query_fanout_max_retained_bytes.get(),
        config.max_content_len.get(),
    );
    if route_body.is_none_or(|phase| defaults::torii::HTTP_READ_CHUNK_BYTES_V1 > phase) {
        emit_torii_config_error(
            emitter,
            "Torii's fixed HTTP read chunk exceeds the App API routed-read transport-frame phase derived from torii.query_fanout_max_retained_bytes and torii.max_content_len",
        );
    }
}
