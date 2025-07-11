//! Constant values used in Torii that might be re-used by client libraries as well.
use serde::{Deserialize, Serialize};

pub mod uri {
    //! URI that Torii uses to route incoming requests.

    /// Query URI is used to handle incoming Query requests.
    pub const QUERY: &str = "/query";
    /// Transaction URI is used to handle incoming ISI requests.
    pub const TRANSACTION: &str = "/transaction";
    /// Health URI is used to handle incoming Healthcheck requests.
    pub const HEALTH: &str = "/health";
    /// Peers URI is used to find all peers in the network
    pub const PEERS: &str = "/peers";
    /// The web socket uri used to subscribe to block and transactions statuses.
    pub const SUBSCRIPTION: &str = "/events";
    /// The web socket uri used to subscribe to blocks stream.
    pub const BLOCKS_STREAM: &str = "/block/stream";
    /// The URI for local config changing inspecting
    pub const CONFIGURATION: &str = "/configuration";
    /// URI to report status for administration
    pub const STATUS: &str = "/status";
    ///  Metrics URI is used to export metrics according to [Prometheus
    ///  Guidance](https://prometheus.io/docs/instrumenting/writing_exporters/).
    pub const METRICS: &str = "/metrics";
    /// URI for retrieving the schema with which Iroha was built.
    pub const SCHEMA: &str = "/schema";
    /// URI for getting the API version currently used
    pub const API_VERSION: &str = "/api_version";
    /// URI for getting cpu profile
    pub const PROFILE: &str = "/debug/pprof/profile";
    /// URI for getting the server version
    pub const SERVER_VERSION: &str = "/server_version";
}

/// Response body for GET server version request
#[derive(Deserialize, Serialize)]
pub struct Version {
    /// The version string (e.g., from `CARGO_PKG_VERSION`)
    pub version: String,
    /// The git commit SHA
    pub git_sha: String,
}
