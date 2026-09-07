//! Synchronous SDK reads isolated from the asynchronous test-network runtime.

use color_eyre::eyre::{self, Result, WrapErr};

/// Run a synchronous read on a dedicated thread without an inherited Tokio runtime.
///
/// Tokio blocking-pool workers retain a runtime handle, which the SDK correctly
/// rejects for synchronous HTTP reads. This thread has no runtime context.
///
/// # Errors
/// Returns the read error, a thread-spawn failure, or a reported worker panic.
pub async fn read_on_dedicated_thread<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("iroha-test-network-read".to_owned())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
                .unwrap_or_else(|panic| {
                    let message = panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap_or("non-string panic payload");
                    Err(eyre::eyre!("dedicated read task panicked: {message}"))
                });
            let _ = sender.send(result);
        })
        .wrap_err("failed to spawn dedicated read task")?;
    receiver
        .await
        .wrap_err("dedicated read task ended without a result")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dedicated_read_excludes_runtime_and_preserves_errors_and_panics() -> Result<()> {
        let value = read_on_dedicated_thread(|| {
            assert!(tokio::runtime::Handle::try_current().is_err());
            Ok(7_u8)
        })
        .await?;
        assert_eq!(value, 7);
        let error = read_on_dedicated_thread(|| Err::<(), _>(eyre::eyre!("read failed")))
            .await
            .expect_err("read error must propagate");
        assert_eq!(error.to_string(), "read failed");
        let panic = read_on_dedicated_thread(|| -> Result<()> { panic!("read panicked") })
            .await
            .expect_err("read panic must be reported");
        assert!(panic.to_string().contains("read panicked"));
        Ok(())
    }
}
