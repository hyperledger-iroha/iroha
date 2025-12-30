//! Network address types used by Iroha with deterministic serialization and schema support.
//!
//! The standard library's [`std::net`] types carry platform-specific behaviour and serde impls
//! we do not control, so lightweight wrappers are provided here and shared across the
//! Iroha data model. A companion [`socket_addr!`] macro parses address literals at compile
//! time for convenience while keeping the canonical Norito codecs.
#![allow(unexpected_cfgs)]

use std::{borrow::Cow, format, string::String, vec::Vec};

use derive_more::{AsRef, Debug, Display, From, IntoIterator};
use iroha_macro::FromVariant;
/// Parses an IPv4 or IPv6 socket address literal at compile time.
pub use iroha_primitives_derive::socket_addr;
use iroha_schema::IntoSchema;
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize};
use norito::{Decode, Encode, literal};

use crate::{conststr::ConstString, ffi};

/// Error when parsing an address
#[derive(Debug, Clone, Copy, PartialEq, Eq, displaydoc::Display, thiserror::Error)]
pub enum ParseError {
    /// Not enough segments in IP address
    NotEnoughSegments,
    /// Too many segments in IP address
    TooManySegments,
    /// Failed to parse segment in IP address
    InvalidSegment,
    /// Failed to parse port number in socket address
    InvalidPort,
    /// Failed to find port number in socket address
    NoPort,
    /// Ipv6 address contains more than one '::' abbreviation
    UnexpectedAbbreviation,
}

ffi::ffi_item! {
    /// An Iroha-native version of [`std::net::Ipv4Addr`] that integrates with Norito and schema tooling.
        #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        AsRef,
        From,
        IntoIterator,
        Encode,
        Decode,
        IntoSchema,
    )]
    #[display("{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])]
    #[debug("{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])]
    #[repr(transparent)]
pub struct Ipv4Addr([u8; 4]);

    // SAFETY: `Ipv4Addr` has no trap representation in [u8; 4]
    ffi_type(unsafe {robust})
}

impl Ipv4Addr {
    /// Construct new [`Ipv4Addr`] from given octets
    pub const fn new(octets: [u8; 4]) -> Self {
        Self(octets)
    }
}

// Norito slice-based decoding via the derived codec implementation

impl core::ops::Index<usize> for Ipv4Addr {
    type Output = u8;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl core::str::FromStr for Ipv4Addr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 4];
        let mut iter = s.split('.');

        for byte in &mut bytes {
            let octet = iter
                .next()
                .ok_or(Self::Err::NotEnoughSegments)?
                .parse()
                .map_err(|_| ParseError::InvalidSegment)?;
            *byte = octet;
        }

        if iter.next().is_some() {
            Err(ParseError::TooManySegments)
        } else {
            Ok(Self(bytes))
        }
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Ipv4Addr {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Ipv4Addr {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<Ipv4Addr>()
            .map_err(|err| json::Error::InvalidField {
                field: "ipv4_addr".into(),
                message: format!("invalid IPv4 address `{value}`: {err}"),
            })
    }
}

impl Ipv4Addr {
    /// The address normally associated with the local machine.
    pub const LOCALHOST: Self = Self([127, 0, 0, 1]);

    /// An unspecified address. Normally resolves to
    /// [`Self::LOCALHOST`] but might be configured to resolve to
    /// something else.
    pub const UNSPECIFIED: Self = Self([0, 0, 0, 0]);
}

ffi::ffi_item! {
    /// An Iroha-native version of [`std::net::Ipv6Addr`] that integrates with Norito and schema tooling.
        #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        AsRef,
        From,
        IntoIterator,
        Encode,
        Decode,
        IntoSchema,
    )]
    #[repr(transparent)]
pub struct Ipv6Addr([u16; 8]);

    // SAFETY: `Ipv6Addr` has no trap representation in [u16; 8]
    ffi_type(unsafe {robust})
}

impl Ipv6Addr {
    /// The analogue of [`std::net::Ipv4Addr::LOCALHOST`], an address associated
    /// with the local machine.
    pub const LOOPBACK: Self = Self([0, 0, 0, 0_u16, 0, 0, 0, 1]);

    /// The analogue of [`std::net::Ipv4Addr::UNSPECIFIED`], an address that
    /// usually resolves to the `LOCALHOST`, but might be configured
    /// to resolve to something else.
    pub const UNSPECIFIED: Self = Self([0, 0, 0, 0_u16, 0, 0, 0, 0]);

    /// Construct new [`Ipv6Addr`] from given segments
    pub const fn new(segments: [u16; 8]) -> Self {
        Self(segments)
    }
}

impl core::ops::Index<usize> for Ipv6Addr {
    type Output = u16;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl core::str::FromStr for Ipv6Addr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut words = [0u16; 8];
        let mut iter = s.split(':');

        let shorthand_pos = s.find("::");

        if s.rfind("::") != shorthand_pos {
            return Err(ParseError::UnexpectedAbbreviation);
        }

        for word in &mut words {
            let group = iter.next().ok_or(Self::Err::NotEnoughSegments)?;

            if group.is_empty() {
                break;
            }

            *word = u16::from_str_radix(group, 16).map_err(|_| ParseError::InvalidSegment)?;
        }

        if shorthand_pos.is_some() {
            let mut rev_iter = s.rsplit(':');

            for word in words.iter_mut().rev() {
                let group = rev_iter.next().unwrap();

                if group.is_empty() {
                    return Ok(Self(words));
                }

                *word = u16::from_str_radix(group, 16).map_err(|_| ParseError::InvalidSegment)?;
            }

            return Err(ParseError::TooManySegments);
        }

        if iter.next().is_some() {
            Err(ParseError::TooManySegments)
        } else {
            Ok(Self(words))
        }
    }
}

impl core::fmt::Display for Ipv6Addr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let segments = self.0;

        // Find the longest run of zero segments. Only runs of length >= 2 are
        // eligible for compression. In case of a tie the left-most run wins.
        let mut best_start = None;
        let mut best_len = 0;
        let mut cur_start = None;

        for (i, &seg) in segments.iter().enumerate() {
            if seg == 0 {
                if cur_start.is_none() {
                    cur_start = Some(i);
                }
            } else if let Some(s) = cur_start {
                let len = i - s;
                if len > best_len {
                    best_start = Some(s);
                    best_len = len;
                }
                cur_start = None;
            }
        }

        if let Some(s) = cur_start {
            let len = 8 - s;
            if len > best_len {
                best_start = Some(s);
                best_len = len;
            }
        }

        if best_len < 2 {
            best_start = None;
        }

        let mut i = 0_usize;
        let mut need_colon = false;
        while i < 8 {
            if Some(i) == best_start {
                write!(f, "::")?;
                need_colon = false;
                i += best_len;
                if i == 8 {
                    break;
                }
            } else {
                if need_colon {
                    write!(f, ":")?;
                }
                write!(f, "{:x}", segments[i])?;
                need_colon = true;
                i += 1;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Ipv6Addr {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Ipv6Addr {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<Ipv6Addr>()
            .map_err(|err| json::Error::InvalidField {
                field: "ipv6_addr".into(),
                message: format!("invalid IPv6 address `{value}`: {err}"),
            })
    }
}

ffi::ffi_item! {
    /// An Iroha-native version of [`std::net::IpAddr`] used for deterministic serialization.
        #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Encode,
        Decode,
        IntoSchema,
        FromVariant,
        Hash,
    )]
    #[allow(variant_size_differences)] // Boxing 16 bytes probably doesn't make sense
    pub enum IpAddr {
        /// Ipv4 variant
        V4(Ipv4Addr),
        /// Ipv6 variant
        V6(Ipv6Addr),
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for IpAddr {
    fn write_json(&self, out: &mut String) {
        match self {
            IpAddr::V4(addr) => addr.write_json(out),
            IpAddr::V6(addr) => addr.write_json(out),
        }
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for IpAddr {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        let v4 = value.parse::<Ipv4Addr>();
        if let Ok(addr) = v4 {
            return Ok(IpAddr::V4(addr));
        }
        let v4_err = v4.err().unwrap();
        let v6 = value.parse::<Ipv6Addr>();
        if let Ok(addr) = v6 {
            return Ok(IpAddr::V6(addr));
        }
        let v6_err = v6.err().unwrap();
        Err(json::Error::InvalidField {
            field: "ip_addr".into(),
            message: format!(
                "invalid IP address `{value}` (IPv4 error: {v4_err}; IPv6 error: {v6_err})"
            ),
        })
    }
}

ffi::ffi_item! {
    /// This struct provides an Iroha-native version of [`std::net::SocketAddrV4`] used for deterministic serialization.
        #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Encode,
        Decode,
        IntoSchema,
    )]
    #[display("{}:{}", self.ip, self.port)]
    #[debug("{}:{}", self.ip, self.port)]
pub struct SocketAddrV4 {
        /// The Ipv4 address.
        pub ip: Ipv4Addr,
        /// The port number.
        pub port: u16,
    }
}

impl From<([u8; 4], u16)> for SocketAddrV4 {
    fn from(value: ([u8; 4], u16)) -> Self {
        Self {
            ip: value.0.into(),
            port: value.1,
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for Ipv4Addr {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        if bytes.len() < 4 {
            return Err(norito::core::Error::LengthMismatch);
        }
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes[..4]);
        Ok((Self::from(buf), 4))
    }
}

// `DecodeFromSlice` is provided by derives for this type.

impl core::str::FromStr for SocketAddrV4 {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (ip, port) = value.split_once(':').ok_or(ParseError::NoPort)?;
        Ok(Self {
            ip: ip.parse()?,
            port: port.parse().map_err(|_| ParseError::InvalidPort)?,
        })
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for SocketAddrV4 {
    fn write_json(&self, out: &mut String) {
        let literal = format_addr_literal(&SocketAddr::Ipv4(*self));
        json::write_json_string(&literal, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SocketAddrV4 {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match parse_addr_literal(&value, "socket_addr_v4")? {
            SocketAddr::Ipv4(addr) => Ok(addr),
            _ => Err(json::Error::InvalidField {
                field: "socket_addr_v4".into(),
                message: format!("expected IPv4 socket address literal, got `{value}`"),
            }),
        }
    }
}

ffi::ffi_item! {
    /// This struct provides an Iroha-native version of [`std::net::SocketAddrV6`] used for deterministic serialization.
        #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Encode,
        Decode,
        IntoSchema,
    )]
    #[display("[{}]:{}", self.ip, self.port)]
    #[debug("[{}]:{}", self.ip, self.port)]
pub struct SocketAddrV6 {
        /// The Ipv6 address.
        pub ip: Ipv6Addr,
        /// The port number.
        pub port: u16,
    }
}

impl From<([u16; 8], u16)> for SocketAddrV6 {
    fn from(value: ([u16; 8], u16)) -> Self {
        Self {
            ip: value.0.into(),
            port: value.1,
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for Ipv6Addr {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        if bytes.len() < 16 {
            return Err(norito::core::Error::LengthMismatch);
        }
        let mut segments = [0u16; 8];
        for (i, seg) in segments.iter_mut().enumerate() {
            let start = i * 2;
            let mut buf = [0u8; 2];
            buf.copy_from_slice(&bytes[start..start + 2]);
            *seg = u16::from_le_bytes(buf);
        }
        Ok((Self::from(segments), 16))
    }
}

// `DecodeFromSlice` is provided by derives for this type.

impl core::str::FromStr for SocketAddrV6 {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim_start_matches('[');
        let (ip, port) = value.split_once("]:").ok_or(ParseError::NoPort)?;
        Ok(Self {
            ip: ip.parse()?,
            port: port.parse().map_err(|_| ParseError::InvalidPort)?,
        })
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for SocketAddrV6 {
    fn write_json(&self, out: &mut String) {
        let literal = format_addr_literal(&SocketAddr::Ipv6(*self));
        json::write_json_string(&literal, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SocketAddrV6 {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match parse_addr_literal(&value, "socket_addr_v6")? {
            SocketAddr::Ipv6(addr) => Ok(addr),
            _ => Err(json::Error::InvalidField {
                field: "socket_addr_v6".into(),
                message: format!("expected IPv6 socket address literal, got `{value}`"),
            }),
        }
    }
}

ffi::ffi_item! {
    /// Socket address defined by hostname and port
        #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Encode,
        Decode,
        IntoSchema,
    )]
pub struct SocketAddrHost {
        /// The hostname
        pub host: ConstString,
        /// The port number
        pub port: u16,
    }
}

impl core::fmt::Display for SocketAddrHost {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl core::str::FromStr for SocketAddrHost {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (host, port) = s.split_once(':').ok_or(ParseError::NoPort)?;
        let port = port.parse().map_err(|_| ParseError::InvalidPort)?;
        Ok(Self {
            host: host.into(),
            port,
        })
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for SocketAddrHost {
    fn write_json(&self, out: &mut String) {
        let literal = format_addr_literal(&SocketAddr::Host(self.clone()));
        json::write_json_string(&literal, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SocketAddrHost {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match parse_addr_literal(&value, "socket_addr_host")? {
            SocketAddr::Host(addr) => Ok(addr),
            _ => Err(json::Error::InvalidField {
                field: "socket_addr_host".into(),
                message: format!("expected hostname socket address literal, got `{value}`"),
            }),
        }
    }
}

// Norito derives now implement DecodeFromSlice for these enums and structs.

ffi::ffi_item! {
    /// This enum provides an Iroha-native version of [`std::net::SocketAddr`] used for deterministic serialization.
        #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Encode,
        Decode,
        IntoSchema,
        FromVariant,
    )]
pub enum SocketAddr {
        /// An Ipv4 socket address.
        Ipv4(SocketAddrV4),
        /// An Ipv6 socket address.
        Ipv6(SocketAddrV6),
        /// A socket address identified by hostname
        Host(SocketAddrHost),
    }
}

#[cfg(feature = "json")]
fn parse_socket_addr(input: &str) -> Option<SocketAddr> {
    if let Ok(addr) = input.parse::<SocketAddrV4>() {
        return Some(SocketAddr::Ipv4(addr));
    }
    if let Ok(addr) = input.parse::<SocketAddrV6>() {
        return Some(SocketAddr::Ipv6(addr));
    }
    if let Ok(addr) = input.parse::<SocketAddrHost>() {
        return Some(SocketAddr::Host(addr));
    }
    None
}

fn canonicalize_socket_addr(mut addr: SocketAddr) -> SocketAddr {
    if let SocketAddr::Host(ref mut host) = addr {
        let lower = host.host.as_ref().to_ascii_lowercase();
        if host.host.as_ref() != lower {
            host.host = lower.into();
        }
    }
    addr
}

fn canonical_addr_body(addr: &SocketAddr) -> String {
    match addr {
        SocketAddr::Ipv4(inner) => format!("{}:{}", inner.ip, inner.port),
        SocketAddr::Ipv6(inner) => format!("[{}]:{}", inner.ip, inner.port),
        SocketAddr::Host(inner) => format!("{}:{}", inner.host.as_ref(), inner.port),
    }
}

fn format_addr_literal(addr: &SocketAddr) -> String {
    let canonical = canonicalize_socket_addr(addr.clone());
    literal::format("addr", &canonical_addr_body(&canonical))
}

#[cfg(feature = "json")]
fn parse_addr_literal(value: &str, field: &str) -> Result<SocketAddr, json::Error> {
    let body = literal::parse("addr", value)?;
    let addr = parse_socket_addr(body).ok_or_else(|| json::Error::InvalidField {
        field: field.into(),
        message: format!("invalid {field} literal body `{body}`"),
    })?;
    let canonical = canonicalize_socket_addr(addr);
    let expected_body = canonical_addr_body(&canonical);
    if expected_body != body {
        let expected_literal = literal::format("addr", &expected_body);
        return Err(json::Error::InvalidField {
            field: field.into(),
            message: format!(
                "addr literal `{value}` is not canonical (expected `{expected_literal}`)"
            ),
        });
    }
    Ok(canonical)
}

#[cfg(feature = "json")]
impl FastJsonWrite for SocketAddr {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&format_addr_literal(self), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SocketAddr {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        parse_addr_literal(&value, "socket_addr")
    }
}

// NOTE: `DecodeFromSlice` is now provided by Norito derives for enums as well,
// so the explicit impls for `IpAddr` and `SocketAddr` are redundant and have
// been removed to avoid conflicts.

impl SocketAddr {
    /// Extracts [`IpAddr`] from [`Self::Ipv4`] and [`Self::Ipv6`] variants
    pub fn ip(&self) -> Option<IpAddr> {
        match self {
            SocketAddr::Ipv4(addr) => Some(addr.ip.into()),
            SocketAddr::Ipv6(addr) => Some(addr.ip.into()),
            SocketAddr::Host(_) => None,
        }
    }

    /// Extracts port from [`Self`]
    pub fn port(&self) -> u16 {
        match self {
            SocketAddr::Ipv4(addr) => addr.port,
            SocketAddr::Ipv6(addr) => addr.port,
            SocketAddr::Host(addr) => addr.port,
        }
    }

    /// Serialize the data contained in this [`SocketAddr`] for use in hashing.
    pub fn payload(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            SocketAddr::Ipv4(addr) => {
                bytes.extend(addr.ip);
                bytes.extend(addr.port.to_le_bytes());
            }
            SocketAddr::Ipv6(addr) => {
                bytes.extend(addr.ip.0.iter().copied().flat_map(u16::to_be_bytes));
                bytes.extend(addr.port.to_le_bytes());
            }
            SocketAddr::Host(addr) => {
                bytes.extend(addr.host.bytes());
                bytes.extend(addr.port.to_le_bytes());
            }
        }
        bytes
    }

    /// Returns the canonical Norito literal form of this socket address (`addr:<body>#<crc16>`).
    #[must_use]
    pub fn to_literal(&self) -> String {
        format_addr_literal(self)
    }

    /// Returns the host portion of the socket address.
    ///
    /// Hostname-based addresses borrow their host string, while IP variants return
    /// an owned textual representation of the address.
    #[must_use]
    pub fn host_str(&self) -> Cow<'_, str> {
        match self {
            SocketAddr::Ipv4(addr) => Cow::Owned(addr.ip.to_string()),
            SocketAddr::Ipv6(addr) => Cow::Owned(addr.ip.to_string()),
            SocketAddr::Host(addr) => Cow::Borrowed(addr.host.as_ref()),
        }
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use norito::json::{self, FastJsonWrite};

    use super::*;

    #[test]
    fn socket_addr_json_roundtrip() {
        let addr: SocketAddr = "127.0.0.1:8080".parse::<SocketAddrV4>().unwrap().into();
        let mut json_repr = String::new();
        addr.write_json(&mut json_repr);
        let expected_literal = format_addr_literal(&addr);
        assert_eq!(json_repr, format!("\"{expected_literal}\""));

        let decoded: SocketAddr = json::from_json(&json_repr).expect("roundtrip socket addr");
        assert_eq!(decoded, addr);
    }

    #[test]
    fn socket_addr_literal_rejects_bad_checksum() {
        let json_literal = "\"addr:127.0.0.1:8080#0000\"";
        let err = json::from_json::<SocketAddr>(json_literal).expect_err("checksum mismatch");
        match err {
            json::Error::InvalidField { .. } | json::Error::Message(_) => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn socket_addr_literal_requires_lowercase_host() {
        let body = "Example.COM:8080";
        let literal = literal::format("addr", body);
        let json_literal = format!("\"{literal}\"");
        let err =
            json::from_json::<SocketAddr>(&json_literal).expect_err("non-canonical host must fail");
        match err {
            json::Error::InvalidField { .. } | json::Error::Message(_) => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn socket_addr_raw_literal_is_rejected() {
        let err =
            json::from_json::<SocketAddr>("\"127.0.0.1:8080\"").expect_err("raw literal must fail");
        match err {
            json::Error::InvalidField { .. } | json::Error::Message(_) => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}

#[cfg(test)]
mod literal_tests {
    use super::*;

    #[test]
    fn socket_addr_literal_roundtrip_ipv4() {
        let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
        let literal = addr.to_literal();
        assert!(
            literal.starts_with("addr:127.0.0.1:8080#"),
            "unexpected literal: {literal}"
        );
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "127.0.0.1:8080");
    }

    #[test]
    fn socket_addr_literal_canonicalizes_host_case() {
        let addr: SocketAddr = "Example.COM:3030".parse().expect("host socket addr");
        let literal = addr.to_literal();
        assert!(
            literal.starts_with("addr:example.com:3030#"),
            "unexpected literal: {literal}"
        );
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "example.com:3030");
    }

    #[test]
    fn socket_addr_literal_formats_ipv6() {
        let addr = SocketAddr::from(([0x2001, 0x0db8, 0, 0, 0, 0, 0, 1], 4040));
        let literal = addr.to_literal();
        assert!(
            literal.starts_with("addr:[2001:db8::1]:4040#"),
            "unexpected literal: {literal}"
        );
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "[2001:db8::1]:4040");
    }

    #[test]
    fn host_str_reports_host_component() {
        let v4 = SocketAddr::from(([10, 0, 0, 1], 3030));
        assert_eq!(v4.host_str(), "10.0.0.1");

        let v6 = SocketAddr::from(([0u16; 8], 4040));
        assert_eq!(v6.host_str(), "::");

        let host = SocketAddr::Host(SocketAddrHost {
            host: "example.com".into(),
            port: 5050,
        });
        assert_eq!(host.host_str(), "example.com");
    }
}

impl From<([u8; 4], u16)> for SocketAddr {
    fn from(value: ([u8; 4], u16)) -> Self {
        Self::Ipv4(value.into())
    }
}

impl From<([u16; 8], u16)> for SocketAddr {
    fn from(value: ([u16; 8], u16)) -> Self {
        Self::Ipv6(value.into())
    }
}

impl core::str::FromStr for SocketAddr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(addr) = SocketAddrV4::from_str(s) {
            Ok(Self::Ipv4(addr))
        } else if let Ok(addr) = SocketAddrV6::from_str(s) {
            Ok(Self::Ipv6(addr))
        } else {
            Ok(Self::Host(SocketAddrHost::from_str(s)?))
        }
    }
}

mod std_compat {
    use std::net::ToSocketAddrs;

    use super::*;

    impl From<Ipv4Addr> for std::net::Ipv4Addr {
        #[inline]
        fn from(other: Ipv4Addr) -> Self {
            let Ipv4Addr(octets) = other;
            std::net::Ipv4Addr::from_octets(octets)
        }
    }

    impl From<std::net::Ipv4Addr> for Ipv4Addr {
        #[inline]
        fn from(other: std::net::Ipv4Addr) -> Self {
            Self(other.octets())
        }
    }

    impl From<Ipv6Addr> for std::net::Ipv6Addr {
        #[allow(clippy::many_single_char_names)]
        #[inline]
        fn from(other: Ipv6Addr) -> Self {
            let Ipv6Addr(segments) = other;
            std::net::Ipv6Addr::from_segments(segments)
        }
    }

    impl From<std::net::Ipv6Addr> for Ipv6Addr {
        #[inline]
        fn from(other: std::net::Ipv6Addr) -> Self {
            Self(other.segments())
        }
    }

    impl From<std::net::IpAddr> for IpAddr {
        fn from(value: std::net::IpAddr) -> Self {
            match value {
                std::net::IpAddr::V4(addr) => Self::V4(addr.into()),
                std::net::IpAddr::V6(addr) => Self::V6(addr.into()),
            }
        }
    }

    impl From<IpAddr> for std::net::IpAddr {
        fn from(value: IpAddr) -> Self {
            match value {
                IpAddr::V4(addr) => Self::V4(addr.into()),
                IpAddr::V6(addr) => Self::V6(addr.into()),
            }
        }
    }

    impl From<std::net::SocketAddrV4> for SocketAddrV4 {
        #[inline]
        fn from(other: std::net::SocketAddrV4) -> Self {
            Self {
                ip: (*other.ip()).into(),
                port: other.port(),
            }
        }
    }

    impl From<std::net::SocketAddrV6> for SocketAddrV6 {
        #[inline]
        fn from(other: std::net::SocketAddrV6) -> Self {
            Self {
                ip: (*other.ip()).into(),
                port: other.port(),
            }
        }
    }

    impl From<std::net::SocketAddr> for SocketAddr {
        #[inline]
        fn from(other: std::net::SocketAddr) -> Self {
            match other {
                std::net::SocketAddr::V4(addr) => Self::Ipv4(addr.into()),
                std::net::SocketAddr::V6(addr) => Self::Ipv6(addr.into()),
            }
        }
    }

    impl From<SocketAddrV4> for std::net::SocketAddrV4 {
        #[inline]
        fn from(other: SocketAddrV4) -> Self {
            Self::new(other.ip.into(), other.port)
        }
    }

    impl From<SocketAddrV6> for std::net::SocketAddrV6 {
        #[inline]
        fn from(other: SocketAddrV6) -> Self {
            Self::new(other.ip.into(), other.port, 0, 0)
        }
    }

    impl ToSocketAddrs for SocketAddr {
        type Iter = std::vec::IntoIter<std::net::SocketAddr>;

        fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
            match self {
                SocketAddr::Ipv4(addr) => {
                    Ok(vec![std::net::SocketAddr::V4((*addr).into())].into_iter())
                }
                SocketAddr::Ipv6(addr) => {
                    Ok(vec![std::net::SocketAddr::V6((*addr).into())].into_iter())
                }
                SocketAddr::Host(addr) => (addr.host.as_ref(), addr.port).to_socket_addrs(),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Parsing IPv4 strings should yield the correct address or errors for invalid input.
    #[test]
    fn ipv4() {
        assert_eq!(
            "0.0.0.0".parse::<Ipv4Addr>().unwrap(),
            Ipv4Addr([0, 0, 0, 0])
        );

        assert_eq!(
            "127.0.0.1".parse::<Ipv4Addr>().unwrap(),
            Ipv4Addr([127, 0, 0, 1])
        );

        assert_eq!(
            "192.168.1.256".parse::<Ipv4Addr>().unwrap_err(),
            ParseError::InvalidSegment
        );

        assert_eq!(
            "192.168.1".parse::<Ipv4Addr>().unwrap_err(),
            ParseError::NotEnoughSegments
        );

        assert_eq!(
            "192.168.1.2.3".parse::<Ipv4Addr>().unwrap_err(),
            ParseError::TooManySegments
        );
    }

    // Parsing IPv6 strings should handle compressed and full forms as well as error cases.
    #[test]
    fn ipv6() {
        assert_eq!(
            "::1".parse::<Ipv6Addr>().unwrap(),
            Ipv6Addr([0, 0, 0, 0, 0, 0, 0, 1])
        );

        assert_eq!(
            "ff02::1".parse::<Ipv6Addr>().unwrap(),
            Ipv6Addr([0xff02, 0, 0, 0, 0, 0, 0, 1])
        );

        assert_eq!(
            "2001:0db8::".parse::<Ipv6Addr>().unwrap(),
            Ipv6Addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0])
        );

        assert_eq!(
            "2001:0db8:0000:0000:0000:0000:0000:0001"
                .parse::<Ipv6Addr>()
                .unwrap(),
            Ipv6Addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1])
        );

        assert_eq!(
            "2001:0db8::0001".parse::<Ipv6Addr>().unwrap(),
            Ipv6Addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1])
        );

        assert_eq!(
            "2001:db8:0:1:2:3:4".parse::<Ipv6Addr>().unwrap_err(),
            ParseError::NotEnoughSegments
        );

        assert_eq!(
            "2001:db8:0:1:2:3:4:5:6".parse::<Ipv6Addr>().unwrap_err(),
            ParseError::TooManySegments
        );
    }

    // Formatting should compress leading, trailing and internal zero groups.
    #[test]
    fn ipv6_display_leading_zero_compression() {
        let addr = Ipv6Addr([0, 0, 0, 1, 2, 3, 4, 5]);
        assert_eq!(addr.to_string(), "::1:2:3:4:5");
    }

    #[test]
    fn ipv6_display_trailing_zero_compression() {
        let addr = Ipv6Addr([1, 2, 3, 4, 0, 0, 0, 0]);
        assert_eq!(addr.to_string(), "1:2:3:4::");
    }

    #[test]
    fn ipv6_display_internal_zero_compression() {
        let addr = Ipv6Addr([1, 2, 0, 0, 0, 3, 4, 5]);
        assert_eq!(addr.to_string(), "1:2::3:4:5");
    }

    // Ensure `SocketAddrV4` parsing succeeds with a port and reports errors otherwise.
    #[test]
    fn socket_v4() {
        assert_eq!(
            "192.168.1.0:9019".parse::<SocketAddrV4>().unwrap(),
            SocketAddrV4 {
                ip: Ipv4Addr([192, 168, 1, 0]),
                port: 9019
            }
        );

        assert_eq!(
            "192.168.1.1".parse::<SocketAddrV4>().unwrap_err(),
            ParseError::NoPort
        );

        assert_eq!(
            "192.168.1.1:FOO".parse::<SocketAddrV4>().unwrap_err(),
            ParseError::InvalidPort
        );
    }

    // Ensure `SocketAddrV6` parsing succeeds with a port and reports errors otherwise.
    #[test]
    fn socket_v6() {
        assert_eq!(
            "[2001:0db8::]:9019".parse::<SocketAddrV6>().unwrap(),
            SocketAddrV6 {
                ip: Ipv6Addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0]),
                port: 9019
            }
        );

        assert_eq!(
            "[2001:0db8::]".parse::<SocketAddrV6>().unwrap_err(),
            ParseError::NoPort
        );

        assert_eq!(
            "[2001:0db8::]:FOO".parse::<SocketAddrV6>().unwrap_err(),
            ParseError::InvalidPort
        );
    }

    // Parsing into the enum variant should cover IPv4, IPv6 and hostname forms.
    #[test]
    fn full_socket() {
        let v4 = SocketAddr::Ipv4(SocketAddrV4 {
            ip: Ipv4Addr([192, 168, 1, 0]),
            port: 9019,
        });
        let v4_literal = format!("\"{}\"", format_addr_literal(&v4));
        assert_eq!(
            norito::json::from_json::<SocketAddr>(&v4_literal).unwrap(),
            v4
        );

        let v6 = SocketAddr::Ipv6(SocketAddrV6 {
            ip: Ipv6Addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0]),
            port: 9019,
        });
        let v6_literal = format!("\"{}\"", format_addr_literal(&v6));
        assert_eq!(
            norito::json::from_json::<SocketAddr>(&v6_literal).unwrap(),
            v6
        );

        let host = SocketAddr::Host(SocketAddrHost {
            host: "localhost".into(),
            port: 9019,
        });
        let host_literal = format!("\"{}\"", format_addr_literal(&host));
        assert_eq!(
            norito::json::from_json::<SocketAddr>(&host_literal).unwrap(),
            host
        );
    }

    // Serialising and deserialising addresses should round-trip without loss.
    #[test]
    fn json_roundtrip() {
        let v4 = SocketAddr::Ipv4(SocketAddrV4 {
            ip: Ipv4Addr([192, 168, 1, 0]),
            port: 9019,
        });

        let serialized_v4 = norito::json::to_json(&v4).unwrap();
        assert_eq!(
            norito::json::from_json::<SocketAddr>(&serialized_v4).unwrap(),
            v4
        );

        let v6 = SocketAddr::Ipv6(SocketAddrV6 {
            ip: Ipv6Addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0]),
            port: 9019,
        });

        let addr_str = norito::json::to_json(&v6).unwrap();
        let addr = norito::json::from_json::<SocketAddr>(&addr_str).unwrap();
        assert_eq!(addr, v6);

        let host = SocketAddr::Host(SocketAddrHost {
            host: "localhost".into(),
            port: 9019,
        });

        let host_json = norito::json::to_json(&host).unwrap();
        assert_eq!(
            norito::json::from_json::<SocketAddr>(&host_json).unwrap(),
            host
        );
    }

    // Host-style addresses should parse into `SocketAddrHost` correctly.
    #[test]
    fn host() {
        assert_eq!(
            "localhost:9019".parse::<SocketAddrHost>().unwrap(),
            SocketAddrHost {
                host: "localhost".into(),
                port: 9019
            }
        );
    }
}
