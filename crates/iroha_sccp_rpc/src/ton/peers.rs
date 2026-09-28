//! TON liteserver lists (spec §4.13.4, §7.2, §8).
//!
//! A [`LiteServerSet`] is the ordered list of liteservers one
//! [`LiteClient`](super::liteclient::LiteClient) fails over across: the list
//! configured under `[sccp.light_client_keeper.endpoints] ton_liteservers`
//! (entries `<ipv4>:<port>:<base64 ed25519 public key>`), the compiled default
//! public list that `iroha_config` exposes in
//! `defaults::sccp::endpoints::TON_LITESERVERS`, or the `liteservers` array of
//! a `global-config.json` that a wallet user supplies as a file. Every key is
//! checked to be a usable Ed25519 point before anything connects.
//!
//! Liteserver addresses and keys are public; they are logged as `ip:port`.

use std::{
    fmt,
    net::{Ipv4Addr, SocketAddr},
    sync::atomic::{AtomicUsize, Ordering},
};

use iroha_config::parameters::{
    actual::{SccpLightClientKeeper, SccpTonLiteserver, compiled_ton_liteservers},
    defaults,
};
use norito::json::Value;

use super::adnl::{key_id, server_x25519_key};

/// Why a liteserver list or entry was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerError {
    message: String,
}

impl PeerError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PeerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PeerError {}

/// One liteserver: TCP address and ADNL Ed25519 public key.
#[derive(Clone, PartialEq, Eq)]
pub struct LiteServer {
    address: SocketAddr,
    public_key: [u8; 32],
    key_id: [u8; 32],
    label: String,
}

impl fmt::Debug for LiteServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiteServer")
            .field("address", &self.address)
            .field("key_id", &hex::encode(self.key_id))
            .finish_non_exhaustive()
    }
}

impl fmt::Display for LiteServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

impl LiteServer {
    /// A liteserver at `address` with Ed25519 key `public_key`.
    ///
    /// # Errors
    /// If the key is not a usable Ed25519 point.
    pub fn new(address: SocketAddr, public_key: [u8; 32]) -> Result<Self, PeerError> {
        server_x25519_key(&public_key).map_err(|_| {
            PeerError::new(format!(
                "liteserver {address} has a key that is not a usable Ed25519 public key"
            ))
        })?;
        Ok(Self {
            address,
            public_key,
            key_id: key_id(&public_key),
            label: address.to_string(),
        })
    }

    /// A configured liteserver.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn from_config(entry: &SccpTonLiteserver) -> Result<Self, PeerError> {
        Self::new(SocketAddr::V4(entry.address), entry.public_key)
    }

    /// Parses `<ipv4>:<port>:<base64 ed25519 public key>`.
    ///
    /// # Errors
    /// If the entry is not canonical or the key is unusable.
    pub fn parse(entry: &str) -> Result<Self, PeerError> {
        let entry = entry
            .parse::<SccpTonLiteserver>()
            .map_err(|error| PeerError::new(error.to_string()))?;
        Self::from_config(&entry)
    }

    /// TCP address.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// ADNL Ed25519 public key.
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    /// ADNL short id of the key (the first field of the handshake).
    pub fn key_id(&self) -> &[u8; 32] {
        &self.key_id
    }

    /// `ip:port`, for logs and errors.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// An ordered liteserver list with a preferred (last answering) server.
pub struct LiteServerSet {
    servers: Vec<LiteServer>,
    preferred: AtomicUsize,
}

impl Clone for LiteServerSet {
    fn clone(&self) -> Self {
        Self {
            servers: self.servers.clone(),
            preferred: AtomicUsize::new(self.preferred()),
        }
    }
}

impl fmt::Debug for LiteServerSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiteServerSet")
            .field("servers", &self.servers)
            .field("preferred", &self.preferred())
            .finish()
    }
}

impl LiteServerSet {
    /// A list of `servers`.
    ///
    /// # Errors
    /// If the list is empty, longer than the configuration allows, or repeats
    /// an address.
    pub fn new(servers: impl IntoIterator<Item = LiteServer>) -> Result<Self, PeerError> {
        let mut list: Vec<LiteServer> = Vec::new();
        for server in servers {
            if list.iter().any(|known| known.address == server.address) {
                return Err(PeerError::new(format!(
                    "the liteserver list repeats {}",
                    server.address
                )));
            }
            list.push(server);
        }
        if list.is_empty() {
            return Err(PeerError::new("a liteserver list must not be empty"));
        }
        let limit = defaults::sccp::light_client_keeper::MAX_ENDPOINTS_PER_LIST;
        if list.len() > limit {
            return Err(PeerError::new(format!(
                "a liteserver list holds at most {limit} liteservers"
            )));
        }
        Ok(Self {
            servers: list,
            preferred: AtomicUsize::new(0),
        })
    }

    /// The configured entries.
    ///
    /// # Errors
    /// As [`Self::new`] and [`LiteServer::new`].
    pub fn from_config(entries: &[SccpTonLiteserver]) -> Result<Self, PeerError> {
        Self::new(
            entries
                .iter()
                .map(LiteServer::from_config)
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    /// Parses `<ipv4>:<port>:<base64 key>` entries.
    ///
    /// # Errors
    /// As [`LiteServer::parse`] and [`Self::new`].
    pub fn parse(entries: &[&str]) -> Result<Self, PeerError> {
        Self::new(
            entries
                .iter()
                .map(|entry| LiteServer::parse(entry))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    /// The compiled default public liteservers.
    pub fn compiled_defaults() -> Self {
        Self::from_config(&compiled_ton_liteservers(
            defaults::sccp::endpoints::TON_LITESERVERS,
        ))
        .expect("compiled TON liteservers are valid")
    }

    /// The keeper's effective list (configured, or the compiled defaults when
    /// the configured list was empty).
    ///
    /// # Errors
    /// As [`Self::from_config`].
    pub fn from_keeper_config(keeper: &SccpLightClientKeeper) -> Result<Self, PeerError> {
        Self::from_config(&keeper.endpoints.ton_liteservers)
    }

    /// The `liteservers` array of a TON `global-config.json`
    /// (`{"ip": <signed 32-bit IPv4>, "port": <u16>, "id": {"@type":
    /// "pub.ed25519", "key": "<base64>"}}`), in file order.
    ///
    /// # Errors
    /// If the JSON is malformed, an entry is invalid, or the list is invalid.
    pub fn from_global_config_json(json: &str) -> Result<Self, PeerError> {
        let config = norito::json::parse_value(json)
            .map_err(|error| PeerError::new(format!("the global config is not JSON: {error}")))?;
        let entries = config
            .get("liteservers")
            .and_then(Value::as_array)
            .ok_or_else(|| PeerError::new("the global config has no `liteservers` array"))?;
        let servers = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                global_config_entry(entry)
                    .map_err(|error| PeerError::new(format!("liteservers[{index}]: {error}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(servers)
    }

    /// Number of liteservers (at least one).
    pub fn len(&self) -> usize {
        self.servers.len()
    }

    /// Always `false`: a list holds at least one liteserver.
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// The liteservers in configured order.
    pub fn servers(&self) -> &[LiteServer] {
        &self.servers
    }

    /// Index of the liteserver the next query starts at.
    pub fn preferred(&self) -> usize {
        self.preferred.load(Ordering::Relaxed) % self.servers.len().max(1)
    }

    /// The liteserver the next query starts at.
    pub fn preferred_server(&self) -> &LiteServer {
        &self.servers[self.preferred()]
    }

    /// Moves the preferred liteserver to the next one, for callers whose
    /// verification rejected the data of the current one.
    pub fn rotate_preferred(&self) {
        let next = (self.preferred() + 1) % self.servers.len().max(1);
        self.preferred.store(next, Ordering::Relaxed);
    }

    pub(crate) fn set_preferred(&self, index: usize) {
        self.preferred.store(index, Ordering::Relaxed);
    }
}

/// One `global-config.json` liteserver entry.
fn global_config_entry(entry: &Value) -> Result<LiteServer, PeerError> {
    let ip = entry
        .get("ip")
        .and_then(Value::as_i64)
        .and_then(|ip| i32::try_from(ip).ok())
        .ok_or_else(|| PeerError::new("`ip` must be a signed 32-bit integer"))?;
    let port = entry
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .ok_or_else(|| PeerError::new("`port` must be a 16-bit integer"))?;
    let id = entry
        .get("id")
        .ok_or_else(|| PeerError::new("`id` is missing"))?;
    if id.get("@type").and_then(Value::as_str) != Some("pub.ed25519") {
        return Err(PeerError::new("`id.@type` must be `pub.ed25519`"));
    }
    let key = id
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| PeerError::new("`id.key` must be a base64 string"))?;
    let ip = Ipv4Addr::from(ip.cast_unsigned());
    LiteServer::parse(&format!("{ip}:{port}:{key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "n4VDnSCUuSpjnCyUk9e3QOOd6o0ItSWYbTnW3Wnn8wk=";
    const KEY_B: &str = "3XO67K/qi+gu3T9v8G2hx1yNmWZhccL3O7SoosFo8G0=";

    #[test]
    fn compiled_defaults_are_valid_and_match_the_keeper_defaults() {
        let set = LiteServerSet::compiled_defaults();
        assert_eq!(set.len(), defaults::sccp::endpoints::TON_LITESERVERS.len());
        assert!(!set.is_empty());
        let keeper = LiteServerSet::from_keeper_config(&SccpLightClientKeeper::default())
            .expect("keeper defaults");
        assert_eq!(keeper.servers(), set.servers());
        for (server, entry) in set
            .servers()
            .iter()
            .zip(defaults::sccp::endpoints::TON_LITESERVERS)
        {
            assert!(entry.starts_with(server.label()));
            assert_eq!(server.key_id(), &key_id(server.public_key()));
        }
    }

    #[test]
    fn entries_parse_and_validate() {
        let server = LiteServer::parse(&format!("5.9.10.47:19949:{KEY_A}")).expect("entry");
        assert_eq!(
            server.address(),
            "5.9.10.47:19949".parse().expect("address")
        );
        assert_eq!(server.label(), "5.9.10.47:19949");
        assert_eq!(server.to_string(), "5.9.10.47:19949");
        assert!(format!("{server:?}").contains("key_id"));
        assert!(LiteServer::parse("5.9.10.47:19949").is_err());
        assert!(LiteServer::parse(&format!("5.9.10.47:0:{KEY_A}")).is_err());
        // The identity point is not a usable key.
        let mut identity = [0_u8; 32];
        identity[0] = 1;
        let error = LiteServer::new("127.0.0.1:1".parse().expect("address"), identity)
            .expect_err("identity key");
        assert!(error.to_string().contains("Ed25519"));
    }

    #[test]
    fn lists_are_validated() {
        assert!(LiteServerSet::parse(&[]).is_err());
        let a = format!("5.9.10.47:19949:{KEY_A}");
        let b = format!("5.9.10.15:48014:{KEY_B}");
        let set = LiteServerSet::parse(&[&a, &b]).expect("list");
        assert_eq!(set.len(), 2);
        let repeated = format!("5.9.10.47:19949:{KEY_B}");
        assert!(LiteServerSet::parse(&[&a, &repeated]).is_err());
        let too_many: Vec<String> = (0
            ..=defaults::sccp::light_client_keeper::MAX_ENDPOINTS_PER_LIST)
            .map(|index| format!("10.0.{}.{}:1000:{KEY_A}", index / 200, index % 200 + 1))
            .collect();
        let too_many: Vec<&str> = too_many.iter().map(String::as_str).collect();
        assert!(LiteServerSet::parse(&too_many).is_err());
    }

    #[test]
    fn preferred_server_rotates() {
        let a = format!("5.9.10.47:19949:{KEY_A}");
        let b = format!("5.9.10.15:48014:{KEY_B}");
        let set = LiteServerSet::parse(&[&a, &b]).expect("list");
        assert_eq!(set.preferred(), 0);
        set.rotate_preferred();
        assert_eq!(set.preferred_server().label(), "5.9.10.15:48014");
        set.rotate_preferred();
        assert_eq!(set.preferred(), 0);
        set.set_preferred(1);
        assert_eq!(set.clone().preferred(), 1);
        assert!(format!("{set:?}").contains("preferred"));
    }

    #[test]
    fn global_config_liteservers_are_read_in_order() {
        let json = format!(
            r#"{{"@type":"config.global","liteservers":[
                {{"ip":84478511,"port":19949,"id":{{"@type":"pub.ed25519","key":"{KEY_A}"}}}},
                {{"ip":-2018135749,"port":53312,"id":{{"@type":"pub.ed25519","key":"{KEY_B}"}}}}
            ]}}"#
        );
        let set = LiteServerSet::from_global_config_json(&json).expect("global config");
        assert_eq!(set.servers()[0].label(), "5.9.10.47:19949");
        // Negative integers are the high half of the IPv4 space.
        assert_eq!(set.servers()[1].label(), "135.181.177.59:53312");

        for broken in [
            "not json",
            r#"{"liteservers":{}}"#,
            r#"{"liteservers":[{"ip":"1.2.3.4","port":1,"id":{"@type":"pub.ed25519","key":"x"}}]}"#,
            r#"{"liteservers":[{"ip":1,"port":70000,"id":{"@type":"pub.ed25519","key":"x"}}]}"#,
            r#"{"liteservers":[{"ip":1,"port":1}]}"#,
            r#"{"liteservers":[{"ip":1,"port":1,"id":{"@type":"pub.aes","key":"x"}}]}"#,
            r#"{"liteservers":[{"ip":1,"port":1,"id":{"@type":"pub.ed25519"}}]}"#,
            r#"{"liteservers":[{"ip":84478511,"port":1,"id":{"@type":"pub.ed25519","key":"AAAA"}}]}"#,
            r#"{"liteservers":[]}"#,
        ] {
            assert!(
                LiteServerSet::from_global_config_json(broken).is_err(),
                "{broken}"
            );
        }
    }
}
