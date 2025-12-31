//! Telemetry for development rather than production purposes

use std::path::PathBuf;

use chrono::Utc;
use eyre::{Result, WrapErr, eyre};
use iroha_config::parameters::actual::TelemetryIntegrity;
use iroha_futures::FuturePollTelemetry;
use iroha_logger::telemetry::Event as Telemetry;
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    sync::broadcast::Receiver,
    task::{self, JoinHandle},
};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::integrity::ChainState;
/// Starts telemetry writing to a file. Will create all parent directories.
///
/// # Errors
/// Fails if unable to open the file
pub async fn start_file_output(
    path: PathBuf,
    integrity: TelemetryIntegrity,
    telemetry: Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    let mut stream = crate::futures::get_stream(BroadcastStream::new(telemetry).fuse());

    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| eyre!("the dev telemetry output file should have a parent directory"))?,
    )
    .wrap_err_with(|| {
        eyre!(
            "failed to recursively create directories for the dev telemetry output file: {}",
            path.display()
        )
    })?;

    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(&path)
        .await
        .wrap_err_with(|| {
            eyre!(
                "failed to open the dev telemetry output file: {}",
                path.display()
            )
        })?;

    let mut chain = ChainState::new_with_kind(integrity, "dev");
    let join_handle = task::spawn(async move {
        while let Some(item) = stream.next().await {
            if let Err(error) = write_telemetry(&mut file, &item, &mut chain).await {
                iroha_logger::error!(%error, "failed to write telemetry")
            }
        }
    });

    Ok(join_handle)
}

async fn write_telemetry(
    file: &mut File,
    item: &FuturePollTelemetry,
    integrity: &mut ChainState,
) -> Result<()> {
    // Serde doesn't support async Read Write traits.
    // So let synchronous code be here.
    let mut record = match norito::json::to_value(&item)
        .wrap_err("failed to serialize telemetry to JSON value")?
    {
        norito::json::Value::Object(map) => map,
        _ => return Err(eyre!("dev telemetry must serialize to an object")),
    };
    record.insert("ts".into(), Utc::now().to_rfc3339().into());
    integrity.attach_chain(&mut record)?;

    let mut json =
        norito::json::to_json(&record).wrap_err("failed to serialize telemetry to JSON")?;

    json.push('\n');
    file.write_all(json.as_bytes())
        .await
        .wrap_err("failed to write data to the file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use norito::json::Value;
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn dev_output_includes_chain() {
        let filename = format!(
            "iroha-dev-telemetry-{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(filename);
        let mut file = File::create(&path).await.expect("create file");
        let mut chain = ChainState::new_with_state_path(
            TelemetryIntegrity {
                enabled: true,
                state_dir: None,
                signing_key: None,
                signing_key_id: None,
            },
            None,
        );
        let item = FuturePollTelemetry {
            id: 1,
            name: "test".to_string(),
            duration: 42,
        };

        write_telemetry(&mut file, &item, &mut chain)
            .await
            .expect("write telemetry");
        file.flush().await.expect("flush file");

        let contents = tokio::fs::read_to_string(&path).await.expect("read file");
        let line = contents.lines().next().expect("telemetry line");
        let value: Value = norito::json::from_str(line).expect("parse JSON");
        let map = value.as_object().expect("object");
        assert!(map.contains_key("chain"));
        assert!(map.contains_key("ts"));
        assert_eq!(map.get("name").and_then(Value::as_str), Some("test"));

        let _ = std::fs::remove_file(&path);
    }
}
