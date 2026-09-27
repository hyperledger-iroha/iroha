//! Typed scalar values that appear in definition files.
//!
//! Each type parses from its TOML string form with [`FromStr`] and reports the
//! rejected text, so the enclosing reader can attribute the error to a file and
//! key.

use std::{
    ffi::OsStr,
    fmt,
    net::{IpAddr, Ipv6Addr},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use base64::Engine as _;
use iroha_data_model::sns::{DATASPACE_ALIAS_SUFFIX_ID, NameSelectorV1};

/// A definition value that failed to parse or validate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ValueError(String);

impl ValueError {
    /// Construct from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Implement Norito JSON decoding for a string-typed value through [`FromStr`].
macro_rules! json_via_from_str {
    ($ty:ty) => {
        impl norito::json::JsonDeserialize for $ty {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                let text = parser.parse_string()?;
                text.parse()
                    .map_err(|error: ValueError| norito::json::Error::Message(error.to_string()))
            }
        }
    };
}

/// Declare a closed set of string keywords with parsing, display and JSON decoding.
macro_rules! keyword_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant ),+
        }

        impl $name {
            /// Every accepted keyword, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The keyword as written in definition files.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ValueError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|keyword| keyword.as_str() == text)
                    .ok_or_else(|| {
                        let expected: Vec<_> =
                            Self::ALL.iter().map(|keyword| format!("\"{keyword}\"")).collect();
                        ValueError::new(format!(
                            "`{text}` is not one of {}",
                            expected.join(" | ")
                        ))
                    })
            }
        }

        json_via_from_str!($name);
    };
}

keyword_enum! {
    /// A compiled network profile.
    ProfileId {
        /// The public Taira shape.
        SoraNexusV1 => "sora-nexus-v1",
        /// `sora-nexus-v1` with qualification cadence, epoch length and snapshot interval.
        SoraNexusV1Qual => "sora-nexus-v1-qual",
        /// The developer profile.
        IrohaDevV1 => "iroha-dev-v1",
    }
}

impl ProfileId {
    /// The chain discriminant a definition gets when it does not set one.
    pub const fn default_chain_discriminant(self) -> u16 {
        match self {
            Self::SoraNexusV1 | Self::SoraNexusV1Qual => 369,
            Self::IrohaDevV1 => 753,
        }
    }
}

keyword_enum! {
    /// The role of a network node.
    #[derive(Default)]
    Role {
        /// A member of the global validator roster.
        #[default]
        Validator => "validator",
        /// A syncing, non-voting node without Inrou.
        Observer => "observer",
    }
}

keyword_enum! {
    /// How the SSH user gains root on a host.
    Become {
        /// The SSH user is root.
        None => "none",
        /// `sudo -n`.
        Sudo => "sudo",
    }
}

keyword_enum! {
    /// How the edge reaches node Torii endpoints.
    #[derive(Default)]
    Upstream {
        /// A per-node TLS gateway with Torii bound to loopback.
        #[default]
        Mtls => "mtls",
        /// Torii bound to the node's private address.
        Private => "private",
    }
}

keyword_enum! {
    /// Dataspace visibility.
    #[derive(Default)]
    Visibility {
        /// Only permitted accounts read the dataspace.
        #[default]
        Restricted => "restricted",
        /// Anyone reads the dataspace.
        Public => "public",
    }
}

keyword_enum! {
    /// Where a dataspace committee comes from.
    CommitteeSource {
        /// The parent network's validators.
        Network => "network",
        /// Validators the owner brings, listed in `[[committee.node]]`.
        Owner => "owner",
    }
}

keyword_enum! {
    /// A permission a network definition may grant.
    Permission {
        /// The right to register a dataspace.
        CanRegisterDataspace => "CanRegisterDataspace",
    }
}

/// A definition slug, `[a-z0-9-]{1,32}`: network and node names.
///
/// Slugs appear in state directories, unit names and host paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Slug(String);

impl Slug {
    /// Maximum slug length.
    pub const MAX_LEN: usize = 32;

    /// The slug text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Slug {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let valid = (1..=Self::MAX_LEN).contains(&text.len())
            && text
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(ValueError::new(format!(
                "`{text}` must match [a-z0-9-]{{1,{}}}",
                Self::MAX_LEN
            )))
        }
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

json_via_from_str!(Slug);

/// A canonical SNS dataspace name; it determines the `DataSpaceId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DataspaceName(String);

impl DataspaceName {
    /// The name text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for DataspaceName {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, text).map_err(|error| {
            ValueError::new(format!("`{text}` is not a dataspace name: {error}"))
        })?;
        let canonical = selector.normalized_label();
        if canonical == text {
            Ok(Self(text.to_owned()))
        } else {
            Err(ValueError::new(format!(
                "dataspace name `{text}` must be written canonically as `{canonical}`"
            )))
        }
    }
}

impl fmt::Display for DataspaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

json_via_from_str!(DataspaceName);

/// The scope of an onboarding credential.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CredentialScope {
    /// Onboarding into any dataspace.
    Universal,
    /// Onboarding into one dataspace.
    Dataspace(DataspaceName),
}

impl FromStr for CredentialScope {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text == "universal" {
            return Ok(Self::Universal);
        }
        text.strip_prefix("dataspace:").map_or_else(
            || {
                Err(ValueError::new(format!(
                    "scope `{text}` must be \"universal\" or \"dataspace:<name>\""
                )))
            },
            |name| name.parse().map(Self::Dataspace),
        )
    }
}

impl fmt::Display for CredentialScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Universal => f.write_str("universal"),
            Self::Dataspace(name) => write!(f, "dataspace:{name}"),
        }
    }
}

json_via_from_str!(CredentialScope);

/// A pinned OpenSSH Ed25519 host key, written `ssh-ed25519 <base64>`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostKey([u8; 32]);

impl HostKey {
    /// The only accepted OpenSSH key type.
    pub const KEY_TYPE: &'static str = "ssh-ed25519";

    /// Wrap raw Ed25519 public key bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw Ed25519 public key bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The OpenSSH wire blob: `string "ssh-ed25519" || string key`.
    pub fn wire_blob(&self) -> Vec<u8> {
        let mut blob = Vec::with_capacity(4 + Self::KEY_TYPE.len() + 4 + self.0.len());
        for field in [Self::KEY_TYPE.as_bytes(), self.0.as_slice()] {
            let len = u32::try_from(field.len()).expect("fixed-size field fits in u32");
            blob.extend_from_slice(&len.to_be_bytes());
            blob.extend_from_slice(field);
        }
        blob
    }

    fn from_wire_blob(blob: &[u8]) -> Option<Self> {
        let (key_type, rest) = take_ssh_string(blob)?;
        let (key, rest) = take_ssh_string(rest)?;
        if key_type != Self::KEY_TYPE.as_bytes() || !rest.is_empty() {
            return None;
        }
        key.try_into().ok().map(Self)
    }
}

/// Split one RFC 4251 `string` (u32 big-endian length, then bytes) off `input`.
fn take_ssh_string(input: &[u8]) -> Option<(&[u8], &[u8])> {
    let (len, rest) = input.split_first_chunk::<4>()?;
    let len = usize::try_from(u32::from_be_bytes(*len)).ok()?;
    (len <= rest.len()).then(|| rest.split_at(len))
}

impl FromStr for HostKey {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = |reason: &str| {
            ValueError::new(format!(
                "host key `{text}` must be \"ssh-ed25519 <base64>\": {reason}"
            ))
        };
        let (key_type, encoded) = text
            .split_once(' ')
            .ok_or_else(|| invalid("expected two space-separated fields"))?;
        if key_type != Self::KEY_TYPE {
            return Err(invalid("unsupported key type"));
        }
        let blob = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| invalid("invalid base64"))?;
        let key = Self::from_wire_blob(&blob).ok_or_else(|| invalid("malformed key blob"))?;
        if key.to_string() == text {
            Ok(key)
        } else {
            Err(invalid("non-canonical encoding"))
        }
    }
}

impl fmt::Display for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let encoded = base64::engine::general_purpose::STANDARD.encode(self.wire_blob());
        write!(f, "{} {encoded}", Self::KEY_TYPE)
    }
}

impl fmt::Debug for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

json_via_from_str!(HostKey);

/// An SSH jump target, `user@host[:port]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SshTarget {
    /// The login user.
    pub user: String,
    /// The host name or IP address.
    pub host: String,
    /// The SSH port, 22 when omitted.
    pub port: u16,
}

impl FromStr for SshTarget {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = |reason: String| {
            ValueError::new(format!("`{text}` must be \"user@host[:port]\": {reason}"))
        };
        let (user, endpoint) = text
            .split_once('@')
            .ok_or_else(|| invalid("missing `user@`".to_owned()))?;
        check_user(user).map_err(|error| invalid(error.0))?;
        let (host, port) = split_host_port(endpoint).map_err(|error| invalid(error.0))?;
        check_host(host).map_err(|error| invalid(error.0))?;
        Ok(Self {
            user: user.to_owned(),
            host: host.to_owned(),
            port: port.unwrap_or(22),
        })
    }
}

impl fmt::Display for SshTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "{}@[{}]:{}", self.user, self.host, self.port)
        } else {
            write!(f, "{}@{}:{}", self.user, self.host, self.port)
        }
    }
}

json_via_from_str!(SshTarget);

/// Split `host`, `host:port`, `[v6]` or `[v6]:port`; the port is non-zero.
fn split_host_port(endpoint: &str) -> Result<(&str, Option<u16>), ValueError> {
    let parse_port = |port: &str| {
        port.parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| ValueError::new(format!("invalid port `{port}`")))
    };
    if let Some(bracketed) = endpoint.strip_prefix('[') {
        let (host, after) = bracketed
            .split_once(']')
            .ok_or_else(|| ValueError::new("unterminated `[`"))?;
        if host.parse::<Ipv6Addr>().is_err() {
            return Err(ValueError::new(format!(
                "`[{host}]` must bracket an IPv6 address"
            )));
        }
        let port = match after {
            "" => None,
            _ => Some(parse_port(after.strip_prefix(':').ok_or_else(|| {
                ValueError::new(format!("expected `:<port>` after `[{host}]`"))
            })?)?),
        };
        return Ok((host, port));
    }
    match endpoint.split_once(':') {
        Some((host, port)) => Ok((host, Some(parse_port(port)?))),
        None => Ok((endpoint, None)),
    }
}

/// Check a host name or IP address as passed to SSH and advertised to peers.
///
/// # Errors
///
/// When `host` is neither an IP address nor an RFC 1123 host name.
pub fn check_host(host: &str) -> Result<(), ValueError> {
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let label_ok = |label: &str| {
        (1..=63).contains(&label.len())
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    };
    if host.len() <= 253 && host.split('.').all(label_ok) {
        Ok(())
    } else {
        Err(ValueError::new(format!(
            "`{host}` is not a host name or IP address"
        )))
    }
}

/// Check an SSH login user name, `[a-z_][a-z0-9_-]{0,31}`.
///
/// # Errors
///
/// When `user` is not a portable login name.
pub fn check_user(user: &str) -> Result<(), ValueError> {
    let mut bytes = user.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_');
    let rest_ok = bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
    });
    if first_ok && rest_ok && user.len() <= 32 {
        Ok(())
    } else {
        Err(ValueError::new(format!(
            "`{user}` is not a login name ([a-z_][a-z0-9_-]{{0,31}})"
        )))
    }
}

/// Check a web origin such as `https://explorer.sora.org`, written exactly.
///
/// # Errors
///
/// When `origin` is not the exact serialization of an `http` or `https` origin.
pub fn check_origin(origin: &str) -> Result<(), ValueError> {
    let exact = url::Url::parse(origin).ok().is_some_and(|url| {
        matches!(url.scheme(), "http" | "https") && url.origin().ascii_serialization() == origin
    });
    if exact {
        Ok(())
    } else {
        Err(ValueError::new(format!(
            "`{origin}` is not an exact origin like \"https://host[:port]\""
        )))
    }
}

/// An `https` URL without credentials, query or fragment.
///
/// It is stored canonically, without a trailing slash on an empty path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpsUrl(String);

impl HttpsUrl {
    /// The canonical URL text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for HttpsUrl {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let url = url::Url::parse(text)
            .map_err(|error| ValueError::new(format!("`{text}` is not a URL: {error}")))?;
        let valid = url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none();
        if !valid {
            return Err(ValueError::new(format!(
                "`{text}` must be an https URL without credentials, query or fragment"
            )));
        }
        let canonical = if url.path() == "/" {
            url.as_str().trim_end_matches('/')
        } else {
            url.as_str()
        };
        Ok(Self(canonical.to_owned()))
    }
}

impl fmt::Display for HttpsUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

json_via_from_str!(HttpsUrl);

/// Where release bundles come from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReleaseSource {
    /// A release channel URL.
    Url(HttpsUrl),
    /// A local directory of bundles (resolved against the definition file).
    Directory(PathBuf),
}

impl FromStr for ReleaseSource {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.contains("://") {
            text.parse().map(Self::Url)
        } else if text.is_empty() {
            Err(ValueError::new("release source must not be empty"))
        } else {
            Ok(Self::Directory(PathBuf::from(text)))
        }
    }
}

json_via_from_str!(ReleaseSource);

/// The parent network of a dataspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkRef {
    /// A public root such as `https://taira.sora.org`.
    Url(HttpsUrl),
    /// A network definition file (resolved against the dataspace file).
    Definition(PathBuf),
    /// A card anchor file, `<name>.card.toml` (resolved against the dataspace file).
    CardAnchor(PathBuf),
}

impl NetworkRef {
    /// Suffix that marks a card anchor file.
    pub const CARD_ANCHOR_SUFFIX: &'static str = ".card.toml";
}

impl FromStr for NetworkRef {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.contains("://") {
            return text.parse().map(Self::Url);
        }
        if text.is_empty() {
            return Err(ValueError::new("network must not be empty"));
        }
        let path = PathBuf::from(text);
        let is_anchor = Path::new(text)
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.ends_with(Self::CARD_ANCHOR_SUFFIX));
        Ok(if is_anchor {
            Self::CardAnchor(path)
        } else {
            Self::Definition(path)
        })
    }
}

json_via_from_str!(NetworkRef);

/// A positive duration written like `5m` or `90s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Interval(Duration);

impl Interval {
    /// Wrap a duration.
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// The duration.
    pub const fn get(self) -> Duration {
        self.0
    }
}

impl FromStr for Interval {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match humantime::parse_duration(text) {
            Ok(duration) if !duration.is_zero() => Ok(Self(duration)),
            Ok(_) => Err(ValueError::new(format!(
                "interval `{text}` must be positive"
            ))),
            Err(error) => Err(ValueError::new(format!(
                "`{text}` is not a duration like \"5m\": {error}"
            ))),
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", humantime::format_duration(self.0))
    }
}

json_via_from_str!(Interval);

/// A positive byte count written like systemd sizes: `2G`, `512M`, `4096`.
///
/// Suffixes `K`, `M`, `G` and `T` are powers of 1024.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteSize(u64);

impl ByteSize {
    const UNITS: [(char, u32); 4] = [('T', 40), ('G', 30), ('M', 20), ('K', 10)];

    /// Wrap a byte count.
    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// The byte count.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl FromStr for ByteSize {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ValueError::new(format!("`{text}` is not a positive size like \"2G\""));
        let (digits, shift) = Self::UNITS
            .iter()
            .find_map(|(suffix, shift)| text.strip_suffix(*suffix).map(|digits| (digits, *shift)))
            .unwrap_or((text, 0));
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid());
        }
        digits
            .parse::<u64>()
            .ok()
            .and_then(|value| value.checked_mul(1_u64 << shift))
            .filter(|bytes| *bytes > 0)
            .map(Self)
            .ok_or_else(invalid)
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = Self::UNITS
            .iter()
            .find(|(_, shift)| self.0.trailing_zeros() >= *shift);
        match unit {
            Some((suffix, shift)) => write!(f, "{}{suffix}", self.0 >> shift),
            None => write!(f, "{}", self.0),
        }
    }
}

json_via_from_str!(ByteSize);

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJx61jSMsPGwubMm7CiE1KkCd7Awvkl/Cgr7EvdSzhH9";

    #[test]
    fn keywords_parse_display_and_reject_unknown() {
        assert_eq!(
            "sora-nexus-v1-qual".parse::<ProfileId>(),
            Ok(ProfileId::SoraNexusV1Qual)
        );
        assert_eq!(ProfileId::IrohaDevV1.to_string(), "iroha-dev-v1");
        let error = "sora-nexus-v2".parse::<ProfileId>().unwrap_err();
        assert!(error.to_string().contains("\"sora-nexus-v1\""));
        assert_eq!("observer".parse::<Role>(), Ok(Role::Observer));
        assert_eq!(Role::default(), Role::Validator);
        assert_eq!("sudo".parse::<Become>(), Ok(Become::Sudo));
        assert_eq!(Upstream::default(), Upstream::Mtls);
        assert_eq!(Visibility::default(), Visibility::Restricted);
        assert_eq!(
            "owner".parse::<CommitteeSource>(),
            Ok(CommitteeSource::Owner)
        );
        assert!("CanManagePeers".parse::<Permission>().is_err());
    }

    #[test]
    fn profile_default_chain_discriminants() {
        assert_eq!(ProfileId::SoraNexusV1.default_chain_discriminant(), 369);
        assert_eq!(ProfileId::SoraNexusV1Qual.default_chain_discriminant(), 369);
        assert_eq!(ProfileId::IrohaDevV1.default_chain_discriminant(), 753);
    }

    #[test]
    fn slug_rule() {
        assert_eq!("taira-v1".parse::<Slug>().unwrap().as_str(), "taira-v1");
        assert!("a".repeat(32).parse::<Slug>().is_ok());
        for bad in ["", "Taira", "a_b", "a.b", &"a".repeat(33)] {
            assert!(bad.parse::<Slug>().is_err(), "{bad}");
        }
    }

    #[test]
    fn dataspace_names_must_be_canonical() {
        assert_eq!("acme".parse::<DataspaceName>().unwrap().to_string(), "acme");
        assert!("Acme".parse::<DataspaceName>().is_err());
        assert!("".parse::<DataspaceName>().is_err());
        assert!("a b".parse::<DataspaceName>().is_err());
    }

    #[test]
    fn credential_scopes() {
        assert_eq!("universal".parse(), Ok(CredentialScope::Universal));
        let scoped: CredentialScope = "dataspace:acme".parse().unwrap();
        assert_eq!(scoped.to_string(), "dataspace:acme");
        assert!("dataspace:".parse::<CredentialScope>().is_err());
        assert!("domain:acme".parse::<CredentialScope>().is_err());
    }

    #[test]
    fn host_key_round_trips_and_decodes_the_wire_blob() {
        let key: HostKey = KEY.parse().unwrap();
        assert_eq!(key.to_string(), KEY);
        assert_eq!(format!("{key:?}"), KEY);
        assert_eq!(HostKey::from_bytes(*key.as_bytes()), key);
        assert_eq!(key.wire_blob().len(), 51);
        assert_eq!(HostKey::from_wire_blob(&key.wire_blob()), Some(key));
    }

    #[test]
    fn host_key_rejects_malformed_input() {
        let blob = HostKey::from_bytes([7; 32]).wire_blob();
        let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let mut rsa = blob.clone();
        rsa[4..15].copy_from_slice(b"ssh-rsa-xyz");
        let cases = [
            "ssh-ed25519".to_owned(),
            format!("ssh-rsa {}", encode(&blob)),
            format!("{KEY} comment"),
            "ssh-ed25519 AAAA...v1".to_owned(),
            format!("ssh-ed25519 {}", encode(&blob[..50])),
            format!("ssh-ed25519 {}", encode(&[blob.as_slice(), &[0]].concat())),
            format!("ssh-ed25519 {}", encode(&rsa)),
        ];
        for case in cases {
            assert!(case.parse::<HostKey>().is_err(), "{case}");
        }
    }

    #[test]
    fn ssh_string_reader_bounds_lengths() {
        assert_eq!(
            take_ssh_string(&[0, 0, 0, 2, b'a', b'b', b'c']),
            Some((&b"ab"[..], &b"c"[..]))
        );
        assert_eq!(take_ssh_string(&[0, 0, 0, 5, b'a']), None);
        assert_eq!(take_ssh_string(&[0, 0]), None);
    }

    #[test]
    fn ssh_targets() {
        let target: SshTarget = "ops@bastion.example:2222".parse().unwrap();
        assert_eq!((target.user.as_str(), target.port), ("ops", 2222));
        assert_eq!(target.to_string(), "ops@bastion.example:2222");
        let v6: SshTarget = "ops@[2001:db8::1]".parse().unwrap();
        assert_eq!((v6.host.as_str(), v6.port), ("2001:db8::1", 22));
        assert_eq!(v6.to_string(), "ops@[2001:db8::1]:22");
        for bad in [
            "bastion",
            "@bastion",
            "ops@",
            "ops@-oProxy",
            "ops@h:99999",
            "ops@h:0",
            "ops@[::1]2222",
            "ops@[::1]:0",
            "ops@[bastion]",
            "ops@[10.0.0.1]:22",
            "Ops@h",
        ] {
            assert!(bad.parse::<SshTarget>().is_err(), "{bad}");
        }
    }

    #[test]
    fn host_port_splitting() {
        assert_eq!(split_host_port("h").unwrap(), ("h", None));
        assert_eq!(split_host_port("h:1").unwrap(), ("h", Some(1)));
        assert_eq!(split_host_port("[::1]:2").unwrap(), ("::1", Some(2)));
        assert_eq!(split_host_port("[::1]").unwrap(), ("::1", None));
        assert!(split_host_port("[::1").is_err());
        assert!(split_host_port("[::1]2").is_err());
        assert!(split_host_port("[host]").is_err());
        assert!(split_host_port("h:0").is_err());
    }

    #[test]
    fn hosts_users_and_origins() {
        for good in ["taira-v1.sora.org", "127.0.0.1", "::1", "localhost"] {
            assert!(check_host(good).is_ok(), "{good}");
        }
        for bad in ["", "-oProxyCommand=x", "a..b", "a b", "a_b.example"] {
            assert!(check_host(bad).is_err(), "{bad}");
        }
        assert!(check_user("root").is_ok());
        assert!(check_user("deploy-1").is_ok());
        assert!(check_user("Root").is_err());
        assert!(check_user("").is_err());
        assert!(check_origin("https://explorer.sora.org").is_ok());
        assert!(check_origin("http://localhost:3000").is_ok());
        assert!(check_origin("https://explorer.sora.org/").is_err());
        assert!(check_origin("ftp://x.example").is_err());
    }

    #[test]
    fn https_urls_are_canonical() {
        let url: HttpsUrl = "https://taira.sora.org/".parse().unwrap();
        assert_eq!(url.as_str(), "https://taira.sora.org");
        let with_path: HttpsUrl = "https://example.org/iroha".parse().unwrap();
        assert_eq!(with_path.to_string(), "https://example.org/iroha");
        for bad in [
            "http://taira.sora.org",
            "https://u:p@x.org",
            "https://x.org/?q",
            "x",
        ] {
            assert!(bad.parse::<HttpsUrl>().is_err(), "{bad}");
        }
    }

    #[test]
    fn release_sources_and_network_refs() {
        assert!(matches!(
            "https://releases.example.org".parse(),
            Ok(ReleaseSource::Url(_))
        ));
        assert_eq!(
            "dist".parse(),
            Ok(ReleaseSource::Directory(PathBuf::from("dist")))
        );
        assert!("http://insecure".parse::<ReleaseSource>().is_err());
        assert!("".parse::<ReleaseSource>().is_err());
        assert!(matches!(
            "https://taira.sora.org".parse(),
            Ok(NetworkRef::Url(_))
        ));
        assert_eq!(
            "../networks/dev.toml".parse(),
            Ok(NetworkRef::Definition(PathBuf::from(
                "../networks/dev.toml"
            )))
        );
        assert_eq!(
            "../networks/taira.card.toml".parse(),
            Ok(NetworkRef::CardAnchor(PathBuf::from(
                "../networks/taira.card.toml"
            )))
        );
        assert!("".parse::<NetworkRef>().is_err());
    }

    #[test]
    fn intervals() {
        let interval: Interval = "5m".parse().unwrap();
        assert_eq!(interval.get(), Duration::from_secs(300));
        assert_eq!(interval.to_string(), "5m");
        assert_eq!(Interval::new(Duration::from_secs(1)).get().as_secs(), 1);
        assert!("0s".parse::<Interval>().is_err());
        assert!("soon".parse::<Interval>().is_err());
    }

    #[test]
    fn byte_sizes() {
        let size: ByteSize = "2G".parse().unwrap();
        assert_eq!(size.get(), 2 << 30);
        assert_eq!(size.to_string(), "2G");
        assert_eq!("4096".parse::<ByteSize>().unwrap().to_string(), "4K");
        assert_eq!(ByteSize::new(1000).to_string(), "1000");
        for bad in ["", "G", "0", "-1G", "1.5G", "2g", "2GG", "99999999999T"] {
            assert!(bad.parse::<ByteSize>().is_err(), "{bad}");
        }
    }

    #[test]
    fn json_decoding_goes_through_from_str() {
        let key: HostKey = norito::json::from_json(&format!("\"{KEY}\"")).unwrap();
        assert_eq!(key.to_string(), KEY);
        let error = norito::json::from_json::<Slug>("\"Bad\"").unwrap_err();
        assert!(error.to_string().contains("[a-z0-9-]"));
    }
}
