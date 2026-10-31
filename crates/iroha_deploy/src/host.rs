//! Host and endpoint literals shared by generated network bundles and Kagami commands.

use color_eyre::eyre::{Result, ensure, eyre};
use iroha_primitives::addr::{SocketAddr, SocketAddrHost};
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone)]
enum HostKind {
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Name(String),
}
#[derive(Debug, Clone)]
pub(crate) struct CanonicalHost {
    kind: HostKind,
}
impl CanonicalHost {
    pub(crate) fn parse(raw: &str, field: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("`{field}` must not be empty"));
        }
        if trimmed != raw {
            return Err(eyre!("`{field}` must not contain surrounding whitespace"));
        }
        let has_prefix = trimmed.starts_with('[');
        let has_suffix = trimmed.ends_with(']');
        if has_prefix != has_suffix {
            return Err(eyre!("`{field}` has unmatched '[' or ']': `{raw}`"));
        }
        let unbracketed = if has_prefix && trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };
        if unbracketed.is_empty() {
            return Err(eyre!("`{field}` must not be empty"));
        }
        if has_prefix {
            return unbracketed.parse::<Ipv6Addr>().map_or_else(
                |_| {
                    Err(eyre!(
                        "`{field}` brackets are only valid around an IPv6 literal"
                    ))
                },
                |ipv6| {
                    Ok(Self {
                        kind: HostKind::Ipv6(ipv6),
                    })
                },
            );
        }
        if let Ok(ipv4) = unbracketed.parse::<Ipv4Addr>() {
            return Ok(Self {
                kind: HostKind::Ipv4(ipv4),
            });
        }
        if let Ok(ipv6) = unbracketed.parse::<Ipv6Addr>() {
            return Ok(Self {
                kind: HostKind::Ipv6(ipv6),
            });
        }
        if unbracketed.contains(':') {
            return Err(eyre!(
                "`{field}` must be a host name or IP literal without a port: `{raw}`"
            ));
        }
        if unbracketed.len() > 253
            || !unbracketed.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_alphanumeric)
                    && label
                        .as_bytes()
                        .last()
                        .is_some_and(u8::is_ascii_alphanumeric)
                    && label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
        {
            return Err(eyre!(
                "`{field}` must be an ASCII DNS name or IP literal without a port: `{raw}`"
            ));
        }
        Ok(Self {
            kind: HostKind::Name(unbracketed.to_ascii_lowercase()),
        })
    }
    pub(crate) fn addr_literal(&self, port: u16) -> String {
        let addr = match &self.kind {
            HostKind::Ipv4(ipv4) => SocketAddr::from((ipv4.octets(), port)),
            HostKind::Ipv6(ipv6) => SocketAddr::from((ipv6.segments(), port)),
            HostKind::Name(host) => SocketAddr::Host(SocketAddrHost {
                host: host.clone().into(),
                port,
            }),
        };
        addr.to_literal()
    }
    pub(crate) fn url_host(&self) -> String {
        match &self.kind {
            HostKind::Ipv4(ipv4) => ipv4.to_string(),
            HostKind::Ipv6(ipv6) => format!("[{ipv6}]"),
            HostKind::Name(host) => host.clone(),
        }
    }
    pub(crate) fn torii_url(&self, port: u16) -> String {
        format!("http://{}:{port}/", self.url_host())
    }
}

/// Validate and canonicalize a host name or IP literal for another Kagami command.
///
/// # Errors
/// Returns an error when the input is not an accepted host name or IP literal.
pub fn canonical_host(raw: &str, field: &str) -> Result<String> {
    Ok(CanonicalHost::parse(raw, field)?.url_host())
}

/// Validate a non-zero endpoint and render its canonical socket-address literal.
///
/// # Errors
/// Returns an error for a zero port or an invalid host name or IP literal.
pub fn canonical_endpoint_literal(raw: &str, field: &str, port: u16) -> Result<String> {
    ensure!(port != 0, "`{field}` port must be greater than zero");
    Ok(CanonicalHost::parse(raw, field)?.addr_literal(port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::literal;

    #[test]
    fn canonical_host_formats_ipv6_literals_and_urls() {
        let host = CanonicalHost::parse("::1", "--public-host").expect("ipv6 host");
        let literal = host.addr_literal(8080);
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "[::1]:8080");
        assert_eq!(host.url_host(), "[::1]");
    }
    #[test]
    fn canonical_host_lowercases_names() {
        let host = CanonicalHost::parse("LOCALHOST", "--public-host").expect("host");
        let literal = host.addr_literal(1337);
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "localhost:1337");
        assert_eq!(host.url_host(), "localhost");
    }
    #[test]
    fn canonical_host_rejects_host_with_port() {
        let err = CanonicalHost::parse("127.0.0.1:8080", "--public-host")
            .expect_err("host with port should fail");
        assert!(err.to_string().contains("without a port"));
    }
    #[test]
    fn canonical_host_rejects_unbalanced_brackets() {
        let err = CanonicalHost::parse("[::1", "--public-host")
            .expect_err("missing closing bracket should fail");
        assert!(err.to_string().contains("unmatched"));
        let err = CanonicalHost::parse("::1]", "--public-host")
            .expect_err("missing opening bracket should fail");
        assert!(err.to_string().contains("unmatched"));
    }
    #[test]
    fn canonical_host_rejects_non_dns_names_and_injection_characters() {
        for raw in [
            " example.test",
            "example.test ",
            "example.test/path",
            "example.test\nnext",
            "example\".test",
            "$(command)",
            "-leading.example",
            "trailing-.example",
            "two..labels",
            "[localhost]",
        ] {
            let _error = CanonicalHost::parse(raw, "--public-host")
                .expect_err("noncanonical or injectable host must fail");
        }
        let host = CanonicalHost::parse("Node-1.Example.Test", "--public-host")
            .expect("canonical DNS host");
        assert_eq!(host.url_host(), "node-1.example.test");
    }
}
