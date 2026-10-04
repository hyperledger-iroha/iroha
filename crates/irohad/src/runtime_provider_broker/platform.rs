use super::*;
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
#[path = "absolute_deadline.rs"]
mod absolute_deadline;
use absolute_deadline::{BrokerDeadlineV1, DeadlineUnixStreamV1};
#[path = "protocol/platform/endpoint_recovery.rs"]
mod endpoint_recovery;
#[cfg(test)]
#[path = "protocol/platform/process_admission_fixture.rs"]
mod process_admission_fixture;
#[cfg(test)]
use std::io;
const STOCK_BROKER_SOCKET_MODE_V1: u32 = 0o660;
const BROKER_IO_TIMEOUT_V1: Duration = Duration::from_secs(15);
const MAX_BROKER_SESSIONS_V1: usize = 8;
include!("platform_server_qualification.rs");
include!("platform_operation_dispatch.rs");
#[path = "server_observation.rs"]
mod server_observation;
include!("platform_server_transport.rs");
#[path = "received_exchange.rs"]
mod received_exchange;
use received_exchange::{OutboundExchangeV1, ReceivedExchangeV1};
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
mod tests {
    use super::process_admission_fixture::*;
    fn validated_production_endpoint()
    -> iroha_config::parameters::actual::RuntimeProviderBrokerEndpointPath {
        iroha_config::parameters::actual::RuntimeProviderBrokerEndpointPath::try_new(
            iroha_config::parameters::defaults::runtime_provider_broker::endpoint_path(),
        )
        .expect("validated default broker endpoint")
    }
    include!("server_tests_01.rs");
    include!("server_tests_02.rs");
    include!("server_tests_03.rs");
    include!("server_tests_04.rs");
    include!("runtime_operation_tests.rs");
    include!("absolute_deadline_exchange_tests.rs");
}
