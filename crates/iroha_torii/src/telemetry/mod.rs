pub mod peers;

/// Record an API hit for Torii endpoints when telemetry is enabled.
/// This is a no-op placeholder until dedicated metrics are wired.
#[cfg(all(feature = "app_api", feature = "telemetry"))]
pub fn report_torii_api_hit(
    telemetry: &crate::routing::MaybeTelemetry,
    _api_token: &str,
    _endpoint: &str,
) {
    telemetry.with_metrics(|_metrics| {});
}

#[cfg(not(all(feature = "app_api", feature = "telemetry")))]
pub fn report_torii_api_hit(
    _telemetry: &crate::routing::MaybeTelemetry,
    _api_token: &str,
    _endpoint: &str,
) {
}
