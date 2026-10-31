//! Socket addresses as operators write them in node configuration files.

use iroha_primitives::addr::SocketAddr;
use norito::json;

/// What a configured socket address may look like, quoted in every refusal.
const EXPECTED: &str =
    "expected `host:port`, `ip:port`, `[ipv6]:port` or a checked `addr:host:port#CRC` literal";

/// A socket address read from a node configuration file.
///
/// Operators write `host:port`, `ip:port` or `[ipv6]:port`. Tools may emit the checked
/// `addr:<host:port>#<crc16>` literal instead; its checksum and canonical form are still
/// enforced. Both spellings yield the same canonical [`SocketAddr`] (hostnames lowercased).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSocketAddr(SocketAddr);

impl ConfigSocketAddr {
    /// The canonical address.
    #[must_use]
    pub fn into_inner(self) -> SocketAddr {
        self.0
    }

    fn parse(value: &str) -> Result<Self, ConfigSocketAddrError> {
        let literal = if value.starts_with("addr:") {
            value.to_owned()
        } else {
            value
                .parse::<SocketAddr>()
                .map_err(|_| ConfigSocketAddrError::new(value, EXPECTED))?
                .to_literal()
        };
        let address = json::from_value::<SocketAddr>(json::Value::String(literal))
            .map_err(|error| ConfigSocketAddrError::new(value, &error.to_string()))?;
        if let SocketAddr::Host(host) = &address
            && !is_host_name(host.host.as_ref())
        {
            return Err(ConfigSocketAddrError::new(value, EXPECTED));
        }
        Ok(Self(address))
    }
}

/// A DNS host name: non-empty letters, digits, `-`, `_` and `.`.
fn is_host_name(host: &str) -> bool {
    !host.is_empty()
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// A configured socket address that is neither `host:port` nor a valid checked literal.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid socket address `{value}`: {reason}")]
pub struct ConfigSocketAddrError {
    value: String,
    reason: String,
}

impl ConfigSocketAddrError {
    fn new(value: &str, reason: &str) -> Self {
        Self {
            value: value.to_owned(),
            reason: reason.to_owned(),
        }
    }
}

impl From<ConfigSocketAddrError> for json::Error {
    fn from(error: ConfigSocketAddrError) -> Self {
        json::Error::InvalidField {
            field: "socket_addr".into(),
            message: error.to_string(),
        }
    }
}

impl json::JsonDeserialize for ConfigSocketAddr {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        Self::parse(&value).map_err(Into::into)
    }

    fn json_from_value(value: &json::Value) -> Result<Self, json::Error> {
        let json::Value::String(value) = value else {
            return Err(json::Error::InvalidField {
                field: "socket_addr".into(),
                message: EXPECTED.to_owned(),
            });
        };
        Self::parse(value).map_err(Into::into)
    }
}

/// Environment overlays use the same parser through the blanket `FromEnvStr` implementation.
impl std::str::FromStr for ConfigSocketAddr {
    type Err = ConfigSocketAddrError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config_base::env::FromEnvStr;
    use std::borrow::Cow;

    fn parse(value: &str) -> Result<SocketAddr, ConfigSocketAddrError> {
        ConfigSocketAddr::parse(value).map(ConfigSocketAddr::into_inner)
    }

    #[test]
    fn plain_and_checked_spellings_yield_the_same_address() {
        for (plain, expected) in [
            ("0.0.0.0:1337", "0.0.0.0:1337"),
            ("127.0.0.1:8080", "127.0.0.1:8080"),
            ("[::1]:8080", "[::1]:8080"),
            ("Node-0.Example.COM:1337", "node-0.example.com:1337"),
        ] {
            let address = parse(plain).expect(plain);
            assert_eq!(address.to_string(), expected, "{plain}");
            assert_eq!(parse(&address.to_literal()).expect("literal"), address);
        }
    }

    #[test]
    fn checked_literals_keep_their_checksum_and_canonical_form() {
        assert!(parse("addr:127.0.0.1:8080#0000").is_err());
        let literal = parse("127.0.0.1:8080").expect("plain").to_literal();
        assert!(literal.starts_with("addr:127.0.0.1:8080#"));
        assert_eq!(
            parse(&literal).expect("checked literal").to_string(),
            "127.0.0.1:8080"
        );
    }

    #[test]
    fn malformed_addresses_name_the_accepted_spellings() {
        for invalid in [
            "",
            "127.0.0.1",
            "127.0.0.1:port",
            "addr:",
            ":1337",
            "bad host:1337",
        ] {
            let error = parse(invalid).expect_err(invalid).to_string();
            assert!(error.contains(&format!("`{invalid}`")), "{error}");
        }
        assert!(
            parse("127.0.0.1")
                .expect_err("missing port")
                .to_string()
                .contains("host:port")
        );
    }

    #[test]
    fn toml_strings_and_environment_values_parse_alike() {
        let from_value = <ConfigSocketAddr as json::JsonDeserialize>::json_from_value(
            &json::Value::String("127.0.0.1:1337".to_owned()),
        )
        .expect("TOML value");
        let from_env =
            ConfigSocketAddr::from_env_str(Cow::Borrowed("127.0.0.1:1337")).expect("env value");
        assert_eq!(from_value, from_env);
        assert!(
            <ConfigSocketAddr as json::JsonDeserialize>::json_from_value(&json::Value::Bool(true))
                .is_err()
        );
    }
}
