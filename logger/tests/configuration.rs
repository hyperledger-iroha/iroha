use std::time::Duration;

use iroha_data_model::Level;
use iroha_logger::{info, init, Configuration, Telemetry, TelemetryFields};
use tokio::time;

#[tokio::test]
async fn telemetry_separation_custom() {
    let config = Configuration {
        max_log_level: Level::TRACE.into(),
        telemetry_capacity: 100,
        compact_mode: true,
        log_file_path: Some("/dev/stdout".into()),
        terminal_colors: true,
        #[cfg(feature = "tokio-console")]
        tokio_console_addr: "127.0.0.1:5555".into(),
    };
    let (mut receiver, _) = init(&config).unwrap().unwrap();
    info!(target: "telemetry::test", a = 2, c = true, d = "this won't be logged");
    info!("This will be logged in bunyan-readable format");
    let telemetry = Telemetry {
        target: "test",
        fields: TelemetryFields(vec![
            ("a", serde_json::json!(2)),
            ("c", serde_json::json!(true)),
            ("d", serde_json::json!("this won't be logged")),
        ]),
    };
    let output = time::timeout(Duration::from_millis(10), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output, telemetry);
}
