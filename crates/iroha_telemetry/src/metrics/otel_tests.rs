#[cfg(test)]
mod otel_tests {
    use std::sync::Arc;
    use super::*;
    #[test]
    fn global_fetch_otel_is_singleton() {
        let first = global_sorafs_fetch_otel();
        let second = global_sorafs_fetch_otel();
        assert!(
            Arc::ptr_eq(&first, &second),
            "expected OTEL handle to be singleton"
        );
    }
    #[test]
    fn global_gateway_otel_is_singleton() {
        let first = global_sorafs_gateway_otel();
        let second = global_sorafs_gateway_otel();
        assert!(
            Arc::ptr_eq(&first, &second),
            "expected gateway OTEL handle to be singleton"
        );
    }
    #[cfg(not(feature = "otel-exporter"))]
    #[test]
    fn installing_exporter_without_feature_fails() {
        let result = install_sorafs_fetch_otlp_exporter(
            "http://127.0.0.1:4317",
            "sorafs-orchestrator",
            &[],
            Duration::from_secs(5),
        );
        assert!(
            result.is_err(),
            "expected exporter installation to fail without otel-exporter feature"
        );
    }
    #[cfg(feature = "otel-exporter")]
    #[tokio::test]
    async fn installing_exporter_with_valid_configuration_succeeds() {
        let result = install_sorafs_fetch_otlp_exporter(
            "http://127.0.0.1:4317",
            "sorafs-orchestrator-test",
            &[("deployment.environment", "test")],
            Duration::from_secs(3_600),
        );
        assert!(result.is_ok(), "OTLP exporter should install: {result:?}");
    }
}
