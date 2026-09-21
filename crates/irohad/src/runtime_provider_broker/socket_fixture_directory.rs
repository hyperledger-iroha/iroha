//! One private short-directory owner for broker stream and datagram socket fixtures.

use std::{fs, os::unix::fs::PermissionsExt as _};

/// Own a real, short socket directory independently of checkout depth and `TMPDIR`.
pub(super) fn new_broker_socket_test_directory() -> tempfile::TempDir {
    #[cfg(target_os = "macos")]
    let root = "/private/tmp";
    #[cfg(target_os = "linux")]
    let root = "/tmp";
    let directory = tempfile::Builder::new()
        .prefix(".iroha-rpb-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(root)
        .expect("create short private broker socket directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("retain exact private broker directory mode");
    directory
}
