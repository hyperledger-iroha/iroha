//! Daemon stdin lifetime shutdown through the ordinary supervisor signal.

use iroha_futures::supervisor::ShutdownSignal;

/// Shut the daemon down once `input` reaches end-of-file (`lifecycle.exit_on_stdin_close`).
///
/// A supervising parent holds the write end of the child's stdin pipe. When the parent exits for
/// any reason, including SIGKILL, the pipe closes and the node shuts down cleanly through the same
/// signal a panic uses. Bytes written to the pipe are discarded; a read error also shuts down.
pub(super) fn spawn_input_close_shutdown(
    input: impl std::io::Read + Send + 'static,
    signal: ShutdownSignal,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("stdin-close-shutdown".to_owned())
        .spawn(move || {
            drain_until_eof(input);
            iroha_logger::info!("standard input closed, shutting down...");
            signal.send();
        })
        .map(drop)
}
/// Read and discard `input` until end-of-file or a read error.
fn drain_until_eof(mut input: impl std::io::Read) {
    let mut buffer = [0_u8; 256];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => return,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

#[cfg(test)]
#[path = "input_close_tests.rs"]
mod tests;
