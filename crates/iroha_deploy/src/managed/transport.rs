//! Managed worker requests over the shared owner-authenticated local control transport.

use super::*;
use iroha_fs::PrivateDirectory;
pub(super) use iroha_managed_ipc::transport::{
    IO_TIMEOUT, Listener, RequestFailure, clear_stopped_endpoint, supported,
};
use std::{process::Command, time::Instant};

/// Run the owned supervisor apart from its frontend's terminal and process group.
pub(super) fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // The owned supervisor has a separate process group; its stdio is already retained
        // private files. Lifecycle control never signals a guessed process group or PID.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    #[cfg(not(any(unix, windows)))]
    let _ = command;
}

/// The longest an observer waits for one action's reply.
///
/// A stop reply may await an already-dispatched task, so only it exceeds the ordinary budget.
fn observation_budget(action: ControlAction) -> Duration {
    if action == ControlAction::Down {
        STOP_OBSERVATION_MAXIMUM
    } else {
        IO_TIMEOUT
    }
}

fn request_as<T: JsonDeserialize>(
    directory: &PrivateDirectory,
    request: &ControlRequest,
) -> Result<T> {
    iroha_managed_ipc::transport::request_observed_until(
        directory,
        request,
        Instant::now() + observation_budget(request.action),
    )
    .map_err(|failure| failure.into_error().into())
}

pub fn request(directory: &PrivateDirectory, request: &ControlRequest) -> Result<ManagedStatus> {
    request_as(directory, request)
}

/// Observe one typed foreground reply within both the caller's remainder and the ordinary
/// polling budget. This never renews a startup, attachment or signing deadline.
pub(super) fn request_as_until<T: JsonDeserialize>(
    directory: &PrivateDirectory,
    request: &ControlRequest,
    deadline: Instant,
) -> Result<T> {
    Ok(iroha_managed_ipc::transport::request_as_until(
        directory, request, deadline,
    )?)
}

pub fn request_observed_until(
    directory: &PrivateDirectory,
    request: &ControlRequest,
    deadline: Instant,
) -> std::result::Result<ManagedStatus, RequestFailure> {
    iroha_managed_ipc::transport::request_observed_until(directory, request, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_stop_reply_may_exceed_the_ordinary_observation_budget() {
        assert_eq!(
            observation_budget(ControlAction::Down),
            STOP_OBSERVATION_MAXIMUM
        );
        for action in [
            ControlAction::ListenerAdoption,
            ControlAction::Status,
            ControlAction::StartupExpired,
            ControlAction::AttachmentStart,
            ControlAction::AttachmentStatus,
        ] {
            assert_eq!(observation_budget(action), IO_TIMEOUT);
        }
    }

    #[test]
    fn shared_transport_errors_keep_their_exact_managed_variants() {
        let native = std::io::Error::new(std::io::ErrorKind::TimedOut, "native detail");
        assert!(matches!(
            Error::from(iroha_managed_ipc::Error::from(native)),
            Error::Io(error) if error.kind() == std::io::ErrorKind::TimedOut
        ));
        assert!(matches!(
            Error::from(iroha_managed_ipc::Error::Invalid("refused".into())),
            Error::Invalid(message) if message == "refused"
        ));
        assert!(matches!(
            Error::from(iroha_managed_ipc::Error::NativeDeadline),
            Error::NativeDeadline
        ));
    }

    #[cfg(unix)]
    #[test]
    fn detached_supervisor_leads_its_own_process_group() {
        let mut command = Command::new("sleep");
        command.arg("30");
        detach(&mut command);
        let mut child = command.spawn().unwrap();
        let pid = rustix::process::Pid::from_child(&child);
        let group = rustix::process::getpgid(Some(pid));
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(group.unwrap(), pid);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn native_control_roundtrips_typed_attachment_observations() {
        let _resources = super::super::native_test_guard();
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("network")).unwrap();
        let listener = Listener::bind(&directory).unwrap();
        let expected = ManagedAttachmentStatus {
            network: "fixture".into(),
            stage: ManagedAttachmentPhase::Connecting,
            wallet_status: None,
            local_successor: None,
            parent_confirmed: None,
            failure: None,
        };
        let reply = expected.clone();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(mut connection) = listener.accept().unwrap() {
                    assert_eq!(
                        connection.receive().unwrap().action,
                        ControlAction::AttachmentStatus
                    );
                    connection.reply(&reply).unwrap();
                    return;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let actual: ManagedAttachmentStatus = request_as(
            &directory,
            &ControlRequest {
                token: "c".repeat(64),
                action: ControlAction::AttachmentStatus,
            },
        )
        .unwrap();
        assert_eq!(actual, expected);
        server.join().unwrap();
    }
}
