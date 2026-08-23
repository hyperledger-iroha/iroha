use super::*;
use iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1 as _;
use std::{
    fmt, fs,
    os::unix::{
        fs::{FileTypeExt as _, MetadataExt as _},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
#[path = "protocol/platform/endpoint_recovery.rs"]
mod endpoint_recovery;
#[path = "protocol/platform/stream_token_gateway_client.rs"]
mod stream_token_gateway_client;
#[cfg(test)]
use std::io;
use stream_token_gateway_client::StreamTokenGatewayAdmissionBrokerProvider;
#[cfg(target_os = "linux")]
const STOCK_BROKER_ENDPOINT_V1: &str =
    "/run/iroha-runtime-provider-broker-v1/runtime-provider-broker-v1.sock";
#[cfg(target_os = "macos")]
const STOCK_BROKER_ENDPOINT_V1: &str = "/private/var/iroha/run/runtime-provider-broker-v1.sock";
const STOCK_BROKER_SOCKET_MODE_V1: u32 = 0o660;
const BROKER_IO_TIMEOUT_V1: Duration = Duration::from_secs(15);
const MAX_BROKER_SESSIONS_V1: usize = 8;
include!("platform_server_qualification.rs");
include!("platform_operation_dispatch.rs");
include!("platform_server_transport.rs");
include!("platform_provider_clients_01.rs");
include!("pop_recipient_client.rs");
include!("platform_provider_clients_02.rs");
include!("platform_provider_clients_03.rs");
#[cfg(test)]
fn set_socket_mode(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(STOCK_BROKER_SOCKET_MODE_V1),
    )
}
#[cfg(test)]
#[expect(
    clippy::too_many_lines,
    reason = "broker scenario tests keep each ordered protocol transcript together"
)]
mod tests {
    include!("server_tests_01.rs");
    include!("server_tests_02.rs");
    include!("server_tests_03.rs");
    include!("server_tests_04.rs");
    include!("runtime_operation_tests.rs");
}
