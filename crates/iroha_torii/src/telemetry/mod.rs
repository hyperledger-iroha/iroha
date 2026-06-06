pub mod peers;

/// Record an API hit for Torii endpoints when telemetry is enabled.
#[cfg(all(feature = "app_api", feature = "telemetry"))]
pub fn report_torii_api_hit(
    telemetry: &crate::routing::MaybeTelemetry,
    api_token: &str,
    endpoint: &str,
) {
    telemetry.with_metrics(|metrics| {
        metrics.inc_torii_api_token_hit(endpoint, api_token_state(api_token));
    });
}

#[cfg(not(all(feature = "app_api", feature = "telemetry")))]
pub fn report_torii_api_hit(
    _telemetry: &crate::routing::MaybeTelemetry,
    _api_token: &str,
    _endpoint: &str,
) {
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
fn api_token_state(api_token: &str) -> &'static str {
    if api_token.is_empty() {
        "empty"
    } else {
        "present"
    }
}

#[cfg(all(test, feature = "app_api", feature = "telemetry"))]
mod tests {
    use std::sync::Arc;

    use iroha_config::parameters::actual::TelemetryProfile;
    use iroha_core::telemetry::Telemetry;
    use iroha_telemetry::metrics::Metrics;

    use super::*;

    #[test]
    fn api_hits_increment_without_exporting_token_material() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(Arc::clone(&metrics), true);
        let gate =
            crate::routing::MaybeTelemetry::from_profile(Some(telemetry), TelemetryProfile::Full);
        let token = "top-secret-token";

        report_torii_api_hit(&gate, token, "v1/sccp/capabilities");
        report_torii_api_hit(&gate, "", "v1/sccp/capabilities");

        assert_eq!(
            metrics
                .torii_api_token_hits_total
                .with_label_values(&["v1/sccp/capabilities", "present"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_api_token_hits_total
                .with_label_values(&["v1/sccp/capabilities", "empty"])
                .get(),
            1
        );
        let exported = metrics.try_to_string().expect("metrics should serialize");
        assert!(
            !exported.contains(token),
            "metrics output must not expose raw API token material"
        );
    }
}
