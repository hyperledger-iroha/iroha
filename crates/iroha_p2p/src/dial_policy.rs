//! Fail-closed outbound peer-dial admission.

use std::{
    collections::BTreeSet,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr as StdSocketAddr},
};

use iroha_primitives::addr::SocketAddr;

/// Maximum concrete addresses accepted from one outbound DNS lookup.
///
/// The policy checks every answer before dialing, but the answer set itself
/// must also be bounded so a hostile resolver cannot turn validation into an
/// unbounded allocation or loop.
pub(crate) const MAX_OUTBOUND_DNS_ANSWERS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IpFamily {
    V4,
    V6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IpNetwork {
    network: u128,
    prefix: u8,
    family: IpFamily,
}

impl IpNetwork {
    fn parse(raw: &str) -> io::Result<Self> {
        let (address, prefix) = raw.trim().split_once('/').ok_or_else(|| {
            invalid_input(format!(
                "outbound dial CIDR `{raw}` must include a prefix length"
            ))
        })?;
        let address = address.parse::<IpAddr>().map_err(|_| {
            invalid_input(format!("outbound dial CIDR `{raw}` has an invalid address"))
        })?;
        let prefix = prefix.parse::<u8>().map_err(|_| {
            invalid_input(format!(
                "outbound dial CIDR `{raw}` has an invalid prefix length"
            ))
        })?;
        match address {
            IpAddr::V4(address) if prefix <= 32 => {
                let mask = v4_mask(prefix);
                Ok(Self {
                    network: u128::from(u32::from(address) & mask),
                    prefix,
                    family: IpFamily::V4,
                })
            }
            IpAddr::V6(address) if prefix <= 128 => {
                if prefix >= 96
                    && let Some(address) = address.to_ipv4_mapped()
                {
                    let prefix = prefix - 96;
                    let mask = v4_mask(prefix);
                    return Ok(Self {
                        network: u128::from(u32::from(address) & mask),
                        prefix,
                        family: IpFamily::V4,
                    });
                }
                let mask = v6_mask(prefix);
                Ok(Self {
                    network: u128::from(address) & mask,
                    prefix,
                    family: IpFamily::V6,
                })
            }
            IpAddr::V4(_) => Err(invalid_input(format!(
                "outbound dial IPv4 CIDR `{raw}` has a prefix larger than 32"
            ))),
            IpAddr::V6(_) => Err(invalid_input(format!(
                "outbound dial IPv6 CIDR `{raw}` has a prefix larger than 128"
            ))),
        }
    }

    fn contains(self, address: IpAddr) -> bool {
        match (self.family, address) {
            (IpFamily::V4, IpAddr::V4(address)) => {
                u128::from(u32::from(address) & v4_mask(self.prefix)) == self.network
            }
            (IpFamily::V6, IpAddr::V6(address)) => {
                u128::from(address) & v6_mask(self.prefix) == self.network
            }
            _ => false,
        }
    }
}

const fn v4_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

const fn v6_mask(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    }
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn permission_denied(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.into())
}

fn canonical_ip(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or(IpAddr::V6(address), IpAddr::V4),
        address => address,
    }
}

fn normalize_dns_name(raw: &str) -> io::Result<String> {
    let name = raw.trim().strip_suffix('.').unwrap_or(raw.trim());
    if name.is_empty() || name.len() > 253 || !name.is_ascii() {
        return Err(invalid_input(
            "outbound dial DNS name is empty, non-ASCII, or too long",
        ));
    }
    if name.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(invalid_input(
            "outbound dial DNS name contains an invalid label",
        ));
    }
    Ok(name.to_ascii_lowercase())
}

fn normalize_dns_suffix(raw: &str) -> io::Result<String> {
    normalize_dns_name(raw.trim().strip_prefix('.').unwrap_or(raw.trim()))
}

fn dns_suffix_matches(name: &str, suffix: &str) -> bool {
    name == suffix
        || name
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn parse_cidrs(values: Vec<String>) -> io::Result<Vec<IpNetwork>> {
    values
        .into_iter()
        .map(|value| IpNetwork::parse(&value))
        .collect()
}

fn parse_dns_suffixes(values: Vec<String>) -> io::Result<Vec<String>> {
    let suffixes = values
        .into_iter()
        .map(|value| normalize_dns_suffix(&value))
        .collect::<io::Result<BTreeSet<_>>>()?;
    Ok(suffixes.into_iter().collect())
}

/// Operator policy applied before DNS and again to every resolved dial target.
///
/// Deny entries always take precedence. An empty allow-list for one dimension
/// (IP or DNS) leaves that dimension unrestricted, so operators can constrain
/// names, address ranges, or both independently without changing defaults.
#[derive(Clone, Debug, Default)]
pub(crate) struct OutboundDialPolicy {
    allow_cidrs: Vec<IpNetwork>,
    deny_cidrs: Vec<IpNetwork>,
    allow_dns_suffixes: Vec<String>,
    deny_dns_suffixes: Vec<String>,
}

impl OutboundDialPolicy {
    pub(crate) fn from_config(
        allow_cidrs: Vec<String>,
        deny_cidrs: Vec<String>,
        allow_dns_suffixes: Vec<String>,
        deny_dns_suffixes: Vec<String>,
    ) -> io::Result<Self> {
        Ok(Self {
            allow_cidrs: parse_cidrs(allow_cidrs)?,
            deny_cidrs: parse_cidrs(deny_cidrs)?,
            allow_dns_suffixes: parse_dns_suffixes(allow_dns_suffixes)?,
            deny_dns_suffixes: parse_dns_suffixes(deny_dns_suffixes)?,
        })
    }

    pub(crate) fn check_target(&self, target: &SocketAddr) -> io::Result<()> {
        match target {
            SocketAddr::Ipv4(address) => self.check_ip(IpAddr::V4(Ipv4Addr::from(address.ip))),
            SocketAddr::Ipv6(address) => self.check_ip(IpAddr::V6(Ipv6Addr::from(address.ip))),
            SocketAddr::Host(address) => self.check_dns_name(address.host.as_ref()),
        }
    }

    pub(crate) fn has_ip_constraints(&self) -> bool {
        !self.allow_cidrs.is_empty() || !self.deny_cidrs.is_empty()
    }

    pub(crate) fn check_resolved(&self, target: StdSocketAddr) -> io::Result<()> {
        self.check_ip(target.ip())
    }

    pub(crate) fn check_resolved_targets(
        &self,
        targets: impl IntoIterator<Item = StdSocketAddr>,
    ) -> io::Result<Vec<StdSocketAddr>> {
        let mut admitted = Vec::new();
        let mut policy_error = None;
        for (index, target) in targets.into_iter().enumerate() {
            if index >= MAX_OUTBOUND_DNS_ANSWERS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "outbound dial resolution exceeds the {MAX_OUTBOUND_DNS_ANSWERS}-address limit"
                    ),
                ));
            }
            match self.check_resolved(target) {
                Ok(()) if !admitted.contains(&target) => admitted.push(target),
                Ok(()) => {}
                Err(error) => policy_error = Some(error),
            }
        }
        if admitted.is_empty() {
            return Err(policy_error.unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "outbound dial target resolved to no addresses",
                )
            }));
        }
        Ok(admitted)
    }

    pub(crate) fn check_dns_name(&self, name: &str) -> io::Result<()> {
        let name = normalize_dns_name(name)?;
        if self
            .deny_dns_suffixes
            .iter()
            .any(|suffix| dns_suffix_matches(&name, suffix))
        {
            return Err(permission_denied(format!(
                "outbound dial DNS name `{name}` is denied by policy"
            )));
        }
        if !self.allow_dns_suffixes.is_empty()
            && !self
                .allow_dns_suffixes
                .iter()
                .any(|suffix| dns_suffix_matches(&name, suffix))
        {
            return Err(permission_denied(format!(
                "outbound dial DNS name `{name}` is outside the configured allow-list"
            )));
        }
        Ok(())
    }

    fn check_ip(&self, address: IpAddr) -> io::Result<()> {
        let original_address = address;
        let address = canonical_ip(original_address);
        if self.deny_cidrs.iter().any(|network| {
            network.contains(address)
                || (original_address != address && network.contains(original_address))
        }) {
            return Err(permission_denied(format!(
                "outbound dial address `{address}` is denied by policy"
            )));
        }
        if !self.allow_cidrs.is_empty()
            && !self
                .allow_cidrs
                .iter()
                .any(|network| network.contains(address))
        {
            return Err(permission_denied(format!(
                "outbound dial address `{address}` is outside the configured allow-list"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_primitives::addr::SocketAddrHost;

    fn policy(
        allow_cidrs: &[&str],
        deny_cidrs: &[&str],
        allow_dns: &[&str],
        deny_dns: &[&str],
    ) -> OutboundDialPolicy {
        OutboundDialPolicy::from_config(
            allow_cidrs.iter().map(ToString::to_string).collect(),
            deny_cidrs.iter().map(ToString::to_string).collect(),
            allow_dns.iter().map(ToString::to_string).collect(),
            deny_dns.iter().map(ToString::to_string).collect(),
        )
        .expect("valid policy")
    }

    fn host(name: &str) -> SocketAddr {
        SocketAddr::Host(SocketAddrHost {
            host: name.into(),
            port: 1337,
        })
    }

    #[test]
    fn empty_policy_allows_literal_and_named_targets() {
        let policy = policy(&[], &[], &[], &[]);
        policy
            .check_target(&host("peer.example"))
            .expect("name allowed");
        policy
            .check_resolved("203.0.113.7:1337".parse().expect("address"))
            .expect("address allowed");
    }

    #[test]
    fn deny_cidr_takes_precedence_over_allow_cidr() {
        let policy = policy(&["10.0.0.0/8"], &["10.9.0.0/16"], &[], &[]);
        policy
            .check_resolved("10.8.1.2:1337".parse().expect("address"))
            .expect("allowed subnet");
        let error = policy
            .check_resolved("10.9.1.2:1337".parse().expect("address"))
            .expect_err("deny must win");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            policy
                .check_resolved("192.0.2.1:1337".parse().expect("address"))
                .is_err(),
            "address outside allow-list must fail closed"
        );
    }

    #[test]
    fn ipv6_prefixes_are_enforced() {
        let policy = policy(&["2001:db8::/32"], &["2001:db8:dead::/48"], &[], &[]);
        policy
            .check_resolved("[2001:db8:1::1]:1337".parse().expect("address"))
            .expect("allowed IPv6 range");
        assert!(
            policy
                .check_resolved("[2001:db8:dead::1]:1337".parse().expect("address"))
                .is_err()
        );
    }

    #[test]
    fn ipv4_mapped_ipv6_cannot_bypass_an_ipv4_deny_rule() {
        let policy = policy(&[], &["127.0.0.0/8"], &[], &[]);
        let error = policy
            .check_resolved("[::ffff:127.0.0.1]:1337".parse().expect("mapped address"))
            .expect_err("mapped loopback must be treated as IPv4 loopback");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn ipv4_mapped_cidrs_are_canonicalized_without_weakening_ipv6_denies() {
        let mapped_deny = policy(&[], &["::ffff:127.0.0.0/104"], &[], &[]);
        for target in ["127.0.0.1:1337", "[::ffff:127.0.0.1]:1337"] {
            let error = mapped_deny
                .check_resolved(target.parse().expect("address"))
                .expect_err("mapped IPv4 CIDR must deny both address representations");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }

        let mapped_allow = policy(&["::ffff:192.0.2.0/120"], &[], &[], &[]);
        for target in ["192.0.2.7:1337", "[::ffff:192.0.2.7]:1337"] {
            mapped_allow
                .check_resolved(target.parse().expect("address"))
                .expect("mapped IPv4 CIDR must allow both address representations");
        }
        assert!(
            mapped_allow
                .check_resolved("192.0.3.7:1337".parse().expect("address"))
                .is_err(),
            "mapped IPv4 allow CIDR must retain its prefix"
        );

        let broad_ipv6_deny = policy(&[], &["::/0"], &[], &[]);
        assert!(
            broad_ipv6_deny
                .check_resolved("[::ffff:198.51.100.7]:1337".parse().expect("address"))
                .is_err(),
            "a mapped address must not lose its original IPv6 deny match"
        );

        let ipv6_only_allow = policy(&["::/0"], &[], &[], &[]);
        assert!(
            ipv6_only_allow
                .check_resolved("[::ffff:198.51.100.7]:1337".parse().expect("address"))
                .is_err(),
            "a broad IPv6 allow must not admit a semantically IPv4 target"
        );
    }

    #[test]
    fn zero_length_prefixes_are_well_defined() {
        let policy = policy(&["0.0.0.0/0", "::/0"], &[], &[], &[]);
        policy
            .check_resolved("198.51.100.7:1337".parse().expect("IPv4 address"))
            .expect("IPv4 /0 covers every IPv4 address");
        policy
            .check_resolved("[2001:db8::7]:1337".parse().expect("IPv6 address"))
            .expect("IPv6 /0 covers every IPv6 address");
    }

    #[test]
    fn dns_suffixes_match_only_at_label_boundaries() {
        let policy = policy(&[], &[], &[".example.com"], &["blocked.example.com"]);
        policy
            .check_target(&host("validator.example.com."))
            .expect("subdomain allowed");
        assert!(policy.check_target(&host("notexample.com")).is_err());
        assert!(policy.check_target(&host("blocked.example.com")).is_err());
        assert!(policy.check_target(&host("x.blocked.example.com")).is_err());
    }

    #[test]
    fn host_and_resolved_ip_are_independent_admission_steps() {
        let policy = policy(&["203.0.113.0/24"], &["127.0.0.0/8"], &["example.com"], &[]);
        policy
            .check_target(&host("peer.example.com"))
            .expect("hostname admitted before lookup");
        policy
            .check_resolved("203.0.113.9:1337".parse().expect("address"))
            .expect("public resolution admitted");
        assert!(
            policy
                .check_resolved("127.0.0.1:1337".parse().expect("address"))
                .is_err(),
            "a rebinding result must be checked after lookup"
        );
    }

    #[test]
    fn malformed_policy_entries_fail_at_startup() {
        assert!(
            OutboundDialPolicy::from_config(
                vec!["192.0.2.0/33".to_owned()],
                vec![],
                vec![],
                vec![],
            )
            .is_err()
        );
        assert!(
            OutboundDialPolicy::from_config(
                vec![],
                vec![],
                vec!["bad..example".to_owned()],
                vec![],
            )
            .is_err()
        );
    }

    #[test]
    fn resolved_answer_set_is_bounded_before_retention() {
        let policy = policy(&[], &[], &[], &[]);
        let answers = (0..=MAX_OUTBOUND_DNS_ANSWERS).map(|index| {
            StdSocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(192, 0, 2, (index % 255) as u8)),
                1337,
            )
        });
        let error = policy
            .check_resolved_targets(answers)
            .expect_err("the 65th DNS answer must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
