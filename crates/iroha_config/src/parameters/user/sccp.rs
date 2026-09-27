//! User view of the node-local SCCP configuration: `[sccp.attestor]` and
//! `[sccp.light_client_keeper]` (`specs/sccp.md` §4.9, §4.13.4).
//!
//! The tables are file-only (no environment aliases), and every field has a default that works
//! without editing: an empty `key_dir` derives `<kura.store_dir>/sccp/bridge-keys` like the
//! default snapshot store directory, and an empty endpoint list selects the compiled public
//! list. Parsing checks syntax only; owner-only file checks happen at runtime in the attestor,
//! the keeper and the RPC crate.

use std::{
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    time::Duration,
};

use error_stack::Report;
use iroha_config_base::{
    ReadConfig, WithOrigin,
    util::{DurationMs, Emitter},
};
use url::Url;

use super::ParseError;
use crate::parameters::{actual, defaults};

/// Millisecond default wrapper used by the `#[config(default)]` expressions below.
const fn ms(millis: u64) -> DurationMs {
    DurationMs(Duration::from_millis(millis))
}

/// `[sccp]`: node-local SCCP components.
#[derive(Debug, Clone, ReadConfig)]
pub struct SccpNode {
    /// `[sccp.attestor]`: the in-node bridge-key attestor (§4.9).
    #[config(nested)]
    pub attestor: SccpAttestor,
    /// `[sccp.light_client_keeper]`: the in-node inbound light-client keeper (§4.13.4).
    #[config(nested)]
    pub light_client_keeper: SccpLightClientKeeper,
}

impl SccpNode {
    /// Validates the tables and resolves derived defaults against the parsed Kura store
    /// directory. Invalid fields are reported through `emitter` and replaced by defaults.
    pub(super) fn parse(
        self,
        kura_store_dir: &WithOrigin<PathBuf>,
        emitter: &mut Emitter<ParseError>,
    ) -> actual::SccpNode {
        actual::SccpNode {
            attestor: self.attestor.parse(kura_store_dir, emitter),
            light_client_keeper: self.light_client_keeper.parse(emitter),
        }
    }
}

/// `[sccp.attestor]`: the in-node bridge-key attestor (§4.9).
///
/// It is enabled by default and needs no operator input: a validator installs, starts and
/// leaves exactly as it would without SCCP.
#[derive(Debug, Clone, ReadConfig)]
pub struct SccpAttestor {
    /// Run the attestor (default `true`). If the key directory is unusable the attestor is
    /// inert and health reports `sccp_attestor_unconfigured`; the node itself does not abort.
    #[config(default = "defaults::sccp::attestor::ENABLED")]
    pub enabled: bool,
    /// Bridge-key directory. Empty (the default) means `<kura.store_dir>/sccp/bridge-keys`.
    /// A relative path resolves against the configuration file. The directory is created
    /// `0700`; each key is one owner-only `0600` regular file `<address-hex>.key`, and
    /// symlinks and other modes are refused. An operator MAY point it elsewhere or pre-place a
    /// key file there; that override is the only optional step.
    #[config(default = "PathBuf::new()")]
    pub key_dir: WithOrigin<PathBuf>,
    /// Submit `SetSccpBridgeKeyV1` automatically (default `true`) while the node is a
    /// registered, non-barred validator and no active or pending key of its peer equals its
    /// newest local key; retried once per epoch until the key is pending or active.
    #[config(default = "defaults::sccp::attestor::AUTO_REGISTER")]
    pub auto_register: bool,
    /// Entries per `SubmitSccpAttestationsV1` transaction (default `64`, `1..=1024`). The
    /// node also caps it at runtime by the on-chain `max_attestation_entries_per_instruction`.
    #[config(default = "defaults::sccp::attestor::MAX_ENTRIES_PER_TRANSACTION")]
    pub max_entries_per_transaction: u32,
    /// Blocks after which entries that are still unrecorded are resubmitted (default `3`,
    /// at least `1`).
    #[config(default = "defaults::sccp::attestor::RESUBMIT_AFTER_BLOCKS")]
    pub resubmit_after_blocks: u64,
    /// Refuse to sign a rotation subject whose `timestamp_ms` exceeds the local wall clock by
    /// more than this many milliseconds (default `3600000`), because a future-dated rotation
    /// would extend roster validity on destinations.
    #[config(default = "ms(defaults::sccp::attestor::MAX_CLOCK_DRIFT_MS)")]
    pub max_clock_drift_ms: DurationMs,
    /// On a graceful shutdown, sign and submit every pending subject of the node's keys and
    /// delay the exit until they are recorded or this many milliseconds elapse (default
    /// `30000`), logging any handoff that could not complete.
    #[config(default = "ms(defaults::sccp::attestor::SHUTDOWN_GRACE_MS)")]
    pub shutdown_grace_ms: DurationMs,
}

impl SccpAttestor {
    fn parse(
        self,
        kura_store_dir: &WithOrigin<PathBuf>,
        emitter: &mut Emitter<ParseError>,
    ) -> actual::SccpAttestor {
        let defaults = actual::SccpAttestor::defaults_for_kura_store_dir(kura_store_dir);
        let key_dir = if self.key_dir.value().as_os_str().is_empty() {
            defaults.key_dir
        } else {
            self.key_dir
        };
        let ceiling = defaults::sccp::attestor::MAX_ENTRIES_PER_TRANSACTION_CEILING;
        let max_entries_per_transaction = NonZeroU32::new(self.max_entries_per_transaction)
            .filter(|entries| entries.get() <= ceiling)
            .unwrap_or_else(|| {
                emit_sccp_error(
                    emitter,
                    format!("sccp.attestor.max_entries_per_transaction must be in 1..={ceiling}"),
                );
                defaults.max_entries_per_transaction
            });
        let resubmit_after_blocks =
            NonZeroU64::new(self.resubmit_after_blocks).unwrap_or_else(|| {
                emit_sccp_error(
                    emitter,
                    "sccp.attestor.resubmit_after_blocks must be at least 1",
                );
                defaults.resubmit_after_blocks
            });
        actual::SccpAttestor {
            enabled: self.enabled,
            key_dir,
            auto_register: self.auto_register,
            max_entries_per_transaction,
            resubmit_after_blocks,
            max_clock_drift: self.max_clock_drift_ms.get(),
            shutdown_grace: self.shutdown_grace_ms.get(),
        }
    }
}

/// `[sccp.light_client_keeper]`: the in-node inbound light-client keeper (§4.13.4).
///
/// Enabled by default and effective only while the node holds an active or pending registered
/// bridge key; it needs no other setup. When a light client has made no progress for
/// `advance_after_ms`, the keeper builds proof-carrying advances from its RPC endpoints and
/// submits them from that bridge key's account, fee-exempt on success.
#[derive(Debug, Clone, ReadConfig)]
pub struct SccpLightClientKeeper {
    /// Run the keeper (default `true`); effective only while the node holds an active or
    /// pending bridge key.
    #[config(default = "defaults::sccp::light_client_keeper::ENABLED")]
    pub enabled: bool,
    /// Advance a light client once `now - head.last_progress_taira_ms` reaches this many
    /// milliseconds. `0` (the default) means `ws_bound_ms / 4` of each light client.
    #[config(default = "ms(defaults::sccp::light_client_keeper::ADVANCE_AFTER_MS)")]
    pub advance_after_ms: DurationMs,
    /// How often local light-client state is checked, in milliseconds (default `60000`,
    /// nonzero).
    #[config(default = "ms(defaults::sccp::light_client_keeper::POLL_INTERVAL_MS)")]
    pub poll_interval_ms: DurationMs,
    /// Timeout of one RPC request in milliseconds, with failover to the next endpoint (default
    /// `10000`, nonzero).
    #[config(default = "ms(defaults::sccp::light_client_keeper::REQUEST_TIMEOUT_MS)")]
    pub request_timeout_ms: DurationMs,
    /// Largest encoded advance the keeper builds (default `262144`, nonzero); it never exceeds
    /// the on-chain per-instruction bounds, which the node enforces at runtime.
    #[config(default = "defaults::sccp::light_client_keeper::MAX_ADVANCE_BYTES")]
    pub max_advance_bytes: usize,
    /// `[sccp.light_client_keeper.endpoints]`: RPC endpoint lists per chain.
    #[config(nested)]
    pub endpoints: SccpLightClientKeeperEndpoints,
    /// Optional, repeatable `[[sccp.light_client_keeper.secret_headers]]`: a secret header for
    /// one endpoint, read from an owner-only file.
    #[config(default)]
    pub secret_headers: WithOrigin<Vec<SccpSecretHeader>>,
}

impl SccpLightClientKeeper {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::SccpLightClientKeeper {
        let defaults = actual::SccpLightClientKeeper::default();
        let advance_after_ms = self.advance_after_ms.get();
        let poll_interval = nonzero_duration(
            self.poll_interval_ms,
            "sccp.light_client_keeper.poll_interval_ms",
            defaults.poll_interval,
            emitter,
        );
        let request_timeout = nonzero_duration(
            self.request_timeout_ms,
            "sccp.light_client_keeper.request_timeout_ms",
            defaults.request_timeout,
            emitter,
        );
        let max_advance_bytes = NonZeroUsize::new(self.max_advance_bytes).unwrap_or_else(|| {
            emit_sccp_error(
                emitter,
                "sccp.light_client_keeper.max_advance_bytes must be nonzero",
            );
            defaults.max_advance_bytes
        });
        actual::SccpLightClientKeeper {
            enabled: self.enabled,
            advance_after: (!advance_after_ms.is_zero()).then_some(advance_after_ms),
            poll_interval,
            request_timeout,
            max_advance_bytes,
            endpoints: self.endpoints.parse(emitter),
            secret_headers: parse_secret_headers(self.secret_headers, emitter),
        }
    }
}

/// `[sccp.light_client_keeper.endpoints]`: RPC endpoint lists per chain, tried in order with
/// failover.
///
/// Each list empty (the default) means the compiled default list of free public endpoints.
/// A configured list replaces the compiled one entirely. HTTP endpoints must use `https`
/// (`http` only for loopback hosts) and must not embed credentials; secrets go into
/// `secret_headers`. Lists hold at most 64 unique entries.
#[derive(Debug, Clone, Default, ReadConfig)]
pub struct SccpLightClientKeeperEndpoints {
    /// Ethereum execution-layer JSON-RPC URLs.
    #[config(default)]
    pub ethereum_execution: Vec<String>,
    /// Ethereum beacon API URLs serving the light-client routes.
    #[config(default)]
    pub ethereum_beacon: Vec<String>,
    /// BNB Smart Chain JSON-RPC URLs.
    #[config(default)]
    pub bsc: Vec<String>,
    /// TRON HTTP API URLs.
    #[config(default)]
    pub tron: Vec<String>,
    /// TON liteservers as `<ipv4>:<port>:<base64 ed25519 public key>`.
    #[config(default)]
    pub ton_liteservers: Vec<String>,
}

impl SccpLightClientKeeperEndpoints {
    fn parse(&self, emitter: &mut Emitter<ParseError>) -> actual::SccpLightClientKeeperEndpoints {
        actual::SccpLightClientKeeperEndpoints {
            ethereum_execution: parse_http_endpoint_list(
                &self.ethereum_execution,
                "ethereum_execution",
                defaults::sccp::endpoints::ETHEREUM_EXECUTION,
                emitter,
            ),
            ethereum_beacon: parse_http_endpoint_list(
                &self.ethereum_beacon,
                "ethereum_beacon",
                defaults::sccp::endpoints::ETHEREUM_BEACON,
                emitter,
            ),
            bsc: parse_http_endpoint_list(
                &self.bsc,
                "bsc",
                defaults::sccp::endpoints::BSC,
                emitter,
            ),
            tron: parse_http_endpoint_list(
                &self.tron,
                "tron",
                defaults::sccp::endpoints::TRON,
                emitter,
            ),
            ton_liteservers: parse_endpoint_list(
                &self.ton_liteservers,
                "ton_liteservers",
                || actual::compiled_ton_liteservers(defaults::sccp::endpoints::TON_LITESERVERS),
                str::parse::<actual::SccpTonLiteserver>,
                |server: &actual::SccpTonLiteserver| server.address,
                emitter,
            ),
        }
    }
}

/// One `[[sccp.light_client_keeper.secret_headers]]` entry.
#[derive(Debug, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SccpSecretHeader {
    /// Endpoint URL the header is sent to; it must follow the endpoint rules and is matched
    /// exactly after URL normalization.
    pub endpoint: String,
    /// HTTP header name, for example `x-api-key`; headers the RPC client sets itself (such as
    /// `host` or `content-type`) are refused.
    pub header: String,
    /// File holding the header value; it must be an owner-only regular file, which is checked
    /// at runtime. A relative path resolves against the configuration file.
    pub value_file: PathBuf,
}

fn emit_sccp_error(emitter: &mut Emitter<ParseError>, message: impl Into<String>) {
    emitter.emit(Report::new(ParseError::InvalidSccpConfig).attach(message.into()));
}

fn nonzero_duration(
    value: DurationMs,
    name: &str,
    fallback: Duration,
    emitter: &mut Emitter<ParseError>,
) -> Duration {
    let value = value.get();
    if value.is_zero() {
        emit_sccp_error(emitter, format!("{name} must be nonzero"));
        fallback
    } else {
        value
    }
}

/// Validates one HTTP endpoint list; empty selects the `compiled` list.
fn parse_http_endpoint_list(
    entries: &[String],
    name: &str,
    compiled: &[&str],
    emitter: &mut Emitter<ParseError>,
) -> Vec<Url> {
    parse_endpoint_list(
        entries,
        name,
        || actual::compiled_http_endpoints(compiled),
        actual::parse_sccp_http_endpoint,
        |url: &Url| url.as_str().to_owned(),
        emitter,
    )
}

/// Validates one endpoint list: empty selects the compiled list, otherwise every entry must
/// parse, entries must be unique by `key`, and the list is bounded.
fn parse_endpoint_list<T, K: Ord>(
    entries: &[String],
    name: &str,
    compiled: impl FnOnce() -> Vec<T>,
    parse: impl Fn(&str) -> Result<T, actual::SccpEndpointError>,
    key: impl Fn(&T) -> K,
    emitter: &mut Emitter<ParseError>,
) -> Vec<T> {
    if entries.is_empty() {
        return compiled();
    }
    let max = defaults::sccp::light_client_keeper::MAX_ENDPOINTS_PER_LIST;
    if entries.len() > max {
        emit_sccp_error(
            emitter,
            format!("sccp.light_client_keeper.endpoints.{name} must have at most {max} entries"),
        );
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        match parse(entry) {
            Ok(value) if seen.insert(key(&value)) => parsed.push(value),
            Ok(_) => emit_sccp_error(
                emitter,
                format!(
                    "sccp.light_client_keeper.endpoints.{name}[{index}] duplicates an earlier entry"
                ),
            ),
            Err(error) => emit_sccp_error(
                emitter,
                format!("sccp.light_client_keeper.endpoints.{name}[{index}]: {error}"),
            ),
        }
    }
    parsed
}

fn parse_secret_headers(
    entries: WithOrigin<Vec<SccpSecretHeader>>,
    emitter: &mut Emitter<ParseError>,
) -> Vec<actual::SccpSecretHeader> {
    let (entries, origin) = entries.into_tuple();
    let max = defaults::sccp::light_client_keeper::MAX_SECRET_HEADERS;
    if entries.len() > max {
        emit_sccp_error(
            emitter,
            format!("sccp.light_client_keeper.secret_headers must have at most {max} entries"),
        );
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let context = format!("sccp.light_client_keeper.secret_headers[{index}]");
        let endpoint = actual::parse_sccp_http_endpoint(&entry.endpoint)
            .map_err(|error| emit_sccp_error(emitter, format!("{context}.endpoint: {error}")))
            .ok();
        let header = actual::parse_sccp_secret_header_name(&entry.header)
            .map_err(|error| emit_sccp_error(emitter, format!("{context}.header: {error}")))
            .ok();
        let value_file = if entry.value_file.as_os_str().is_empty() {
            emit_sccp_error(emitter, format!("{context}.value_file must name a file"));
            None
        } else {
            Some(WithOrigin::new(entry.value_file, origin.clone()).resolve_relative_path())
        };
        let (Some(endpoint), Some(header), Some(value_file)) = (endpoint, header, value_file)
        else {
            continue;
        };
        if seen.insert((endpoint.as_str().to_owned(), header.clone())) {
            parsed.push(actual::SccpSecretHeader {
                endpoint,
                header,
                value_file,
            });
        } else {
            emit_sccp_error(
                emitter,
                format!("{context} repeats the endpoint and header of an earlier entry"),
            );
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config_base::{ParameterOrigin, read::ConfigReader, toml::TomlSource};

    fn kura_store_dir() -> WithOrigin<PathBuf> {
        WithOrigin::new(
            PathBuf::from("/srv/iroha/storage"),
            ParameterOrigin::custom("test kura.store_dir".to_owned()),
        )
    }

    fn read(toml_text: &str) -> SccpNode {
        let table: toml::Table = toml::from_str(toml_text).expect("test TOML parses");
        ConfigReader::new()
            .without_env()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<SccpNode>()
            .expect("SCCP tables read")
    }

    fn parse(toml_text: &str) -> Result<actual::SccpNode, String> {
        let mut emitter = Emitter::new();
        let parsed = read(toml_text).parse(&kura_store_dir(), &mut emitter);
        emitter
            .into_result()
            .map(|()| parsed)
            .map_err(|report| format!("{report:?}"))
    }

    #[test]
    fn sccp_empty_tables_yield_defaults() {
        let node = parse("").expect("empty SCCP config is valid");
        assert!(node.attestor.enabled);
        assert!(node.attestor.auto_register);
        assert_eq!(
            node.attestor.key_dir_path(),
            PathBuf::from("/srv/iroha/storage/sccp/bridge-keys")
        );
        assert_eq!(node.attestor.max_entries_per_transaction.get(), 64);
        assert_eq!(node.attestor.resubmit_after_blocks.get(), 3);
        assert_eq!(
            node.attestor.max_clock_drift,
            Duration::from_millis(3_600_000)
        );
        assert_eq!(node.attestor.shutdown_grace, Duration::from_millis(30_000));
        assert_eq!(
            node.light_client_keeper,
            actual::SccpLightClientKeeper::default()
        );
    }

    #[test]
    fn sccp_explicit_empty_key_dir_derives_from_kura() {
        let node = parse("[attestor]\nkey_dir = \"\"\n").expect("empty key_dir is the default");
        assert_eq!(
            node.attestor.key_dir_path(),
            PathBuf::from("/srv/iroha/storage/sccp/bridge-keys")
        );
    }

    #[test]
    fn sccp_attestor_bounds() {
        for (text, needle) in [
            (
                "[attestor]\nmax_entries_per_transaction = 0\n",
                "max_entries_per_transaction must be in 1..=1024",
            ),
            (
                "[attestor]\nmax_entries_per_transaction = 1025\n",
                "max_entries_per_transaction must be in 1..=1024",
            ),
            (
                "[attestor]\nresubmit_after_blocks = 0\n",
                "resubmit_after_blocks must be at least 1",
            ),
        ] {
            let error = parse(text).expect_err(text);
            assert!(error.contains(needle), "{error}");
        }
        for entries in [1, 1024] {
            let node = parse(&format!(
                "[attestor]\nmax_entries_per_transaction = {entries}\n"
            ))
            .expect("bound is inclusive");
            assert_eq!(node.attestor.max_entries_per_transaction.get(), entries);
        }
    }

    #[test]
    fn sccp_keeper_zero_values() {
        for (text, needle) in [
            (
                "[light_client_keeper]\npoll_interval_ms = 0\n",
                "poll_interval_ms must be nonzero",
            ),
            (
                "[light_client_keeper]\nrequest_timeout_ms = 0\n",
                "request_timeout_ms must be nonzero",
            ),
            (
                "[light_client_keeper]\nmax_advance_bytes = 0\n",
                "max_advance_bytes must be nonzero",
            ),
        ] {
            let error = parse(text).expect_err(text);
            assert!(error.contains(needle), "{error}");
        }
        let node = parse("[light_client_keeper]\nadvance_after_ms = 1500\n").expect("explicit");
        assert_eq!(
            node.light_client_keeper.advance_after,
            Some(Duration::from_millis(1_500))
        );
    }

    #[test]
    fn sccp_endpoint_lists_replace_defaults_and_reject_bad_entries() {
        let node = parse(
            "[light_client_keeper.endpoints]\n\
             bsc = [\"https://bsc.example.org\", \"http://127.0.0.1:8545\"]\n",
        )
        .expect("override");
        let bsc: Vec<_> = node
            .light_client_keeper
            .endpoints
            .bsc
            .iter()
            .map(url::Url::as_str)
            .collect();
        assert_eq!(bsc, ["https://bsc.example.org/", "http://127.0.0.1:8545/"]);
        assert_eq!(
            node.light_client_keeper.endpoints.tron,
            actual::compiled_http_endpoints(defaults::sccp::endpoints::TRON)
        );
        for (text, needle) in [
            (
                "tron = [\"http://tron.example.org\"]",
                "endpoints.tron[0]: an endpoint URL must use https",
            ),
            (
                "bsc = [\"https://a.example.org\", \"https://a.example.org/\"]",
                "endpoints.bsc[1] duplicates an earlier entry",
            ),
            (
                "ton_liteservers = [\"1.2.3.4:5\"]",
                "endpoints.ton_liteservers[0]: a TON liteserver must be",
            ),
        ] {
            let error =
                parse(&format!("[light_client_keeper.endpoints]\n{text}\n")).expect_err(text);
            assert!(error.contains(needle), "{error}");
        }
        let too_many: Vec<String> = (0
            ..=defaults::sccp::light_client_keeper::MAX_ENDPOINTS_PER_LIST)
            .map(|index| format!("\"https://rpc{index}.example.org\""))
            .collect();
        let error = parse(&format!(
            "[light_client_keeper.endpoints]\nethereum_execution = [{}]\n",
            too_many.join(", ")
        ))
        .expect_err("list is bounded");
        assert!(error.contains("must have at most 64 entries"), "{error}");
    }

    #[test]
    fn sccp_secret_headers_parse_and_validate() {
        let node = parse(
            "[[light_client_keeper.secret_headers]]\n\
             endpoint = \"https://rpc.example.org\"\n\
             header = \"X-Api-Key\"\n\
             value_file = \"/etc/iroha/rpc-key\"\n",
        )
        .expect("secret header");
        let headers = &node.light_client_keeper.secret_headers;
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].endpoint.as_str(), "https://rpc.example.org/");
        assert_eq!(headers[0].header, "x-api-key");
        assert_eq!(headers[0].value_file, PathBuf::from("/etc/iroha/rpc-key"));
        for (entry, needle) in [
            (
                "endpoint = \"http://rpc.example.org\"\nheader = \"x-api-key\"\nvalue_file = \"/k\"",
                "secret_headers[0].endpoint",
            ),
            (
                "endpoint = \"https://rpc.example.org\"\nheader = \"host\"\nvalue_file = \"/k\"",
                "secret_headers[0].header",
            ),
            (
                "endpoint = \"https://rpc.example.org\"\nheader = \"x-api-key\"\nvalue_file = \"\"",
                "secret_headers[0].value_file must name a file",
            ),
        ] {
            let text = format!("[[light_client_keeper.secret_headers]]\n{entry}\n");
            let error = parse(&text).expect_err(&text);
            assert!(error.contains(needle), "{error}");
        }
        let duplicate = "[[light_client_keeper.secret_headers]]\n\
                         endpoint = \"https://rpc.example.org\"\n\
                         header = \"x-api-key\"\nvalue_file = \"/a\"\n\
                         [[light_client_keeper.secret_headers]]\n\
                         endpoint = \"https://rpc.example.org/\"\n\
                         header = \"X-API-KEY\"\nvalue_file = \"/b\"\n";
        let error = parse(duplicate).expect_err("duplicate header");
        assert!(error.contains("secret_headers[1] repeats"), "{error}");
    }

    #[test]
    fn sccp_unknown_keys_are_rejected() {
        for text in [
            "[attestor]\nkey_directory = \"/x\"\n",
            "[light_client_keeper.endpoints]\nethereum = []\n",
        ] {
            let table: toml::Table = toml::from_str(text).expect("test TOML parses");
            let result = ConfigReader::new()
                .without_env()
                .with_toml_source(TomlSource::inline(table))
                .read_and_complete::<SccpNode>();
            assert!(result.is_err(), "{text} must be rejected");
        }
        let table: toml::Table = toml::from_str(
            "[[light_client_keeper.secret_headers]]\nendpoint = \"https://a.example.org\"\n\
             header = \"x-api-key\"\nvalue_file = \"/k\"\nvalue = \"inline secret\"\n",
        )
        .expect("test TOML parses");
        let result = ConfigReader::new()
            .without_env()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<SccpNode>();
        assert!(result.is_err(), "inline secret values are refused");
    }
}
