//! Exit-stream framing and first-release routing admission.
//!
//! Token-bearing filesystem route publication is intentionally unavailable in
//! the first release. The relay retains only configured stream labels and the
//! bounded `RouteOpen` decoder so it can reject requests deterministically.
//! There is no compiled route catalog, bearer-token cache, or WebSocket bridge
//! that can be accidentally re-enabled by constructing internal state.

use crate::config::{
    ConfigError, ExitRoutingConfig, KaigiStreamRoutingConfig, NoritoStreamRoutingConfig,
    validate_gar_category_v1, validate_wss_endpoint_v1,
};
use iroha_data_model::soranet::RelayId;
use std::{fmt, ops::Deref, sync::Arc};
use thiserror::Error;

const DEFAULT_GAR_CATEGORY_READ_ONLY: &str = "stream.norito.read_only";
const DEFAULT_GAR_CATEGORY_AUTH: &str = "stream.norito.authenticated";
const DEFAULT_KAIGI_CATEGORY_PUBLIC: &str = "stream.kaigi.public";
const DEFAULT_KAIGI_CATEGORY_AUTH: &str = "stream.kaigi.authenticated";
const ROUTE_OPEN_FRAME_LEN: usize = 34;

/// Owned secret bytes with redacted diagnostics and scrub-on-drop storage.
///
/// The relay uses this for raw admission credential frames so parse failures
/// and early handshake exits do not release an unwiped allocation.
pub(crate) struct SensitiveBytes(Vec<u8>);

impl SensitiveBytes {
    pub(crate) fn from_vec(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    fn scrub(&mut self) {
        let initialized_len = self.0.len();
        // Expose the complete existing allocation without growing it so bytes
        // left behind by an earlier truncation are wiped as well. Preserve the
        // logical length because tests and explicit early clears may still
        // inspect the owner before it is dropped.
        self.0.resize(self.0.capacity(), 0);
        zeroize::Zeroize::zeroize(self.0.as_mut_slice());
        self.0.truncate(initialized_len);
    }
}

impl AsRef<[u8]> for SensitiveBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for SensitiveBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for SensitiveBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "<redacted:{} bytes>", self.0.len())
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        self.scrub();
    }
}

/// Static exit routing configuration derived from user config.
#[derive(Clone, Debug)]
pub struct ExitRouting {
    norito_stream: Option<NoritoStreamRoute>,
    kaigi_stream: Option<KaigiStreamRoute>,
}

impl ExitRouting {
    /// Validate exit routing without constructing a token publication surface.
    pub fn from_config(cfg: &ExitRoutingConfig) -> Result<Self, ConfigError> {
        let norito_stream = cfg
            .norito_stream
            .as_ref()
            .map(NoritoStreamRoute::from_config)
            .transpose()?;
        let kaigi_stream = cfg
            .kaigi_stream
            .as_ref()
            .map(KaigiStreamRoute::from_config)
            .transpose()?;
        Ok(Self {
            norito_stream,
            kaigi_stream,
        })
    }

    /// Bind the non-secret diagnostic routing labels to one relay identity.
    pub fn prepare(&self, _relay_id: RelayId) -> ExitRoutingState {
        ExitRoutingState {
            norito_stream: self
                .norito_stream
                .clone()
                .map(NoritoStreamState::new)
                .map(Arc::new),
            kaigi_stream: self
                .kaigi_stream
                .clone()
                .map(KaigiStreamState::new)
                .map(Arc::new),
        }
    }
}

/// Prepared first-release exit routing state.
#[derive(Clone)]
pub struct ExitRoutingState {
    norito_stream: Option<Arc<NoritoStreamState>>,
    kaigi_stream: Option<Arc<KaigiStreamState>>,
}

impl ExitRoutingState {
    /// Return configured Norito-stream diagnostic state.
    pub fn norito_stream(&self) -> Option<Arc<NoritoStreamState>> {
        self.norito_stream.as_ref().map(Arc::clone)
    }

    /// Return configured Kaigi-stream diagnostic state.
    pub fn kaigi_stream(&self) -> Option<Arc<KaigiStreamState>> {
        self.kaigi_stream.as_ref().map(Arc::clone)
    }
}

/// Validated Norito-stream labels retained for fail-closed diagnostics.
#[derive(Clone, Debug)]
struct NoritoStreamRoute {
    gar_read_only: String,
    gar_authenticated: String,
}

impl NoritoStreamRoute {
    fn from_config(cfg: &NoritoStreamRoutingConfig) -> Result<Self, ConfigError> {
        validate_wss_endpoint_v1("norito_stream.torii_ws_url", &cfg.torii_ws_url)?;
        if cfg.spool_dir.is_some() {
            return Err(ConfigError::Routing(
                "norito_stream.spool_dir is disabled until RouteOpen proof and durable route revocation are implemented; remove any previously published token files manually"
                    .to_owned(),
            ));
        }
        let gar_read_only = cfg
            .gar_category_read_only
            .as_deref()
            .unwrap_or(DEFAULT_GAR_CATEGORY_READ_ONLY)
            .to_owned();
        let gar_authenticated = cfg
            .gar_category_authenticated
            .as_deref()
            .unwrap_or(DEFAULT_GAR_CATEGORY_AUTH)
            .to_owned();
        validate_gar_category_v1("norito_stream.gar_category_read_only", &gar_read_only)?;
        validate_gar_category_v1(
            "norito_stream.gar_category_authenticated",
            &gar_authenticated,
        )?;
        Ok(Self {
            gar_read_only,
            gar_authenticated,
        })
    }
}

/// Prepared Norito-stream state without a route catalog or bearer cache.
#[derive(Clone)]
pub struct NoritoStreamState {
    config: NoritoStreamRoute,
}

impl NoritoStreamState {
    fn new(config: NoritoStreamRoute) -> Self {
        Self { config }
    }

    /// Return the configured GAR label for diagnostics.
    pub fn gar_category(&self, authenticated: bool) -> &str {
        if authenticated {
            &self.config.gar_authenticated
        } else {
            &self.config.gar_read_only
        }
    }
}

/// Validated Kaigi-stream labels retained for fail-closed diagnostics.
#[derive(Clone, Debug)]
struct KaigiStreamRoute {
    gar_public: String,
    gar_authenticated: String,
}

impl KaigiStreamRoute {
    fn from_config(cfg: &KaigiStreamRoutingConfig) -> Result<Self, ConfigError> {
        validate_wss_endpoint_v1("kaigi_stream.hub_ws_url", &cfg.hub_ws_url)?;
        if cfg.spool_dir.is_some() {
            return Err(ConfigError::Routing(
                "kaigi_stream.spool_dir is disabled until RouteOpen proof and durable route revocation are implemented; remove any previously published token files manually"
                    .to_owned(),
            ));
        }
        let gar_public = cfg
            .gar_category_public
            .as_deref()
            .unwrap_or(DEFAULT_KAIGI_CATEGORY_PUBLIC)
            .to_owned();
        let gar_authenticated = cfg
            .gar_category_authenticated
            .as_deref()
            .unwrap_or(DEFAULT_KAIGI_CATEGORY_AUTH)
            .to_owned();
        validate_gar_category_v1("kaigi_stream.gar_category_public", &gar_public)?;
        validate_gar_category_v1(
            "kaigi_stream.gar_category_authenticated",
            &gar_authenticated,
        )?;
        Ok(Self {
            gar_public,
            gar_authenticated,
        })
    }
}

/// Prepared Kaigi-stream state without a route catalog or bearer cache.
#[derive(Clone)]
pub struct KaigiStreamState {
    config: KaigiStreamRoute,
}

impl KaigiStreamState {
    fn new(config: KaigiStreamRoute) -> Self {
        Self { config }
    }

    /// Return the configured GAR label for diagnostics.
    pub fn gar_category(&self, authenticated: bool) -> &str {
        if authenticated {
            &self.config.gar_authenticated
        } else {
            &self.config.gar_public
        }
    }
}

/// Exit-stream kind encoded in a `RouteOpen` frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitStreamTag {
    /// Norito streaming route.
    NoritoStream,
    /// Kaigi streaming route.
    KaigiStream,
}

impl ExitStreamTag {
    /// Parse the stable first-release stream tag.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::NoritoStream),
            0x02 => Some(Self::KaigiStream),
            _ => None,
        }
    }

    /// Encode the stable first-release stream tag.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::NoritoStream => 0x01,
            Self::KaigiStream => 0x02,
        }
    }
}

/// Bounded first-release route-open request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteOpenFrame {
    tag: ExitStreamTag,
    channel_id: [u8; 32],
}

impl RouteOpenFrame {
    /// Fixed length of the on-wire route-open frame.
    pub const fn length() -> usize {
        ROUTE_OPEN_FRAME_LEN
    }

    /// Decode and validate an exact route-open frame.
    pub fn decode(bytes: &[u8]) -> Result<Self, RouteOpenFrameError> {
        if bytes.len() != ROUTE_OPEN_FRAME_LEN {
            return Err(RouteOpenFrameError::InvalidLength(bytes.len()));
        }
        let tag_byte = bytes[0];
        let Some(tag) = ExitStreamTag::from_u8(tag_byte) else {
            return Err(RouteOpenFrameError::UnknownTag(tag_byte));
        };
        if bytes[1] != 0 {
            return Err(RouteOpenFrameError::NonZeroReservedFlags(bytes[1]));
        }
        let mut channel_id = [0u8; 32];
        channel_id.copy_from_slice(&bytes[2..34]);
        if channel_id.iter().all(|byte| *byte == 0) {
            return Err(RouteOpenFrameError::ZeroChannelId);
        }
        Ok(Self { tag, channel_id })
    }

    /// Return the requested exit-stream kind.
    pub const fn tag(&self) -> ExitStreamTag {
        self.tag
    }

    /// Return the non-zero requested channel identifier.
    pub const fn channel_id(&self) -> &[u8; 32] {
        &self.channel_id
    }
}

/// Route-open frame validation error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteOpenFrameError {
    /// Frame length does not match the expected 34-byte payload.
    #[error("route open frame length must be {ROUTE_OPEN_FRAME_LEN} bytes (got {0})")]
    InvalidLength(usize),
    /// Unknown stream tag detected in the first byte.
    #[error("unknown stream tag: {0:#04x}")]
    UnknownTag(u8),
    /// The reserved flags byte was non-zero.
    #[error("route open reserved flags must be zero (got {0:#04x})")]
    NonZeroReservedFlags(u8),
    /// Channel ID was all zeroes, which is invalid.
    #[error("channel identifier must not be all zeros")]
    ZeroChannelId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_route() -> NoritoStreamRoutingConfig {
        NoritoStreamRoutingConfig {
            torii_ws_url: "wss://torii.test/norito/stream".into(),
            connect_timeout_millis: 750,
            padding_target_millis: 45,
            gar_category_read_only: None,
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 5,
        }
    }

    fn sample_kaigi_route() -> KaigiStreamRoutingConfig {
        KaigiStreamRoutingConfig {
            hub_ws_url: "wss://kaigi.test/hub".into(),
            connect_timeout_millis: 900,
            gar_category_public: None,
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 6,
        }
    }

    #[test]
    fn route_defaults_apply_without_constructing_catalogs() {
        let norito = NoritoStreamRoute::from_config(&sample_route()).expect("Norito route config");
        assert_eq!(norito.gar_read_only, DEFAULT_GAR_CATEGORY_READ_ONLY);
        assert_eq!(norito.gar_authenticated, DEFAULT_GAR_CATEGORY_AUTH);
        let kaigi =
            KaigiStreamRoute::from_config(&sample_kaigi_route()).expect("Kaigi route config");
        assert_eq!(kaigi.gar_public, DEFAULT_KAIGI_CATEGORY_PUBLIC);
        assert_eq!(kaigi.gar_authenticated, DEFAULT_KAIGI_CATEGORY_AUTH);

        let routing = ExitRouting::from_config(&ExitRoutingConfig {
            norito_stream: Some(sample_route()),
            kaigi_stream: Some(sample_kaigi_route()),
        })
        .expect("routing config");
        let state = routing.prepare([0xAA; 32]);
        assert!(state.norito_stream().is_some());
        assert!(state.kaigi_stream().is_some());
    }

    #[test]
    fn filesystem_route_catalog_configuration_is_rejected() {
        let mut norito = sample_route();
        norito.spool_dir = Some(PathBuf::from("/disabled/norito"));
        let error = ExitRouting::from_config(&ExitRoutingConfig {
            norito_stream: Some(norito),
            kaigi_stream: None,
        })
        .expect_err("Norito token catalog must remain disabled");
        assert!(error.to_string().contains("durable route revocation"));

        let mut kaigi = sample_kaigi_route();
        kaigi.spool_dir = Some(PathBuf::from("/disabled/kaigi"));
        let error = ExitRouting::from_config(&ExitRoutingConfig {
            norito_stream: None,
            kaigi_stream: Some(kaigi),
        })
        .expect_err("Kaigi token catalog must remain disabled");
        assert!(error.to_string().contains("durable route revocation"));
    }

    #[test]
    fn route_open_frames_are_strictly_decoded() {
        for (tag, byte, channel) in [
            (ExitStreamTag::NoritoStream, 0x01, [0xAA; 32]),
            (ExitStreamTag::KaigiStream, 0x02, [0xBB; 32]),
        ] {
            let mut bytes = [0u8; ROUTE_OPEN_FRAME_LEN];
            bytes[0] = byte;
            bytes[2..].copy_from_slice(&channel);
            let frame = RouteOpenFrame::decode(&bytes).expect("decode route-open frame");
            assert_eq!(frame.tag(), tag);
            assert_eq!(frame.channel_id(), &channel);
        }

        let mut unknown = [0u8; ROUTE_OPEN_FRAME_LEN];
        unknown[0] = 0xFF;
        unknown[2] = 1;
        assert_eq!(
            RouteOpenFrame::decode(&unknown),
            Err(RouteOpenFrameError::UnknownTag(0xFF))
        );
        let mut reserved = [0u8; ROUTE_OPEN_FRAME_LEN];
        reserved[0] = ExitStreamTag::NoritoStream.as_u8();
        reserved[1] = 1;
        reserved[2] = 1;
        assert_eq!(
            RouteOpenFrame::decode(&reserved),
            Err(RouteOpenFrameError::NonZeroReservedFlags(1))
        );
        let mut zero_channel = [0u8; ROUTE_OPEN_FRAME_LEN];
        zero_channel[0] = ExitStreamTag::KaigiStream.as_u8();
        assert_eq!(
            RouteOpenFrame::decode(&zero_channel),
            Err(RouteOpenFrameError::ZeroChannelId)
        );
    }

    #[test]
    fn sensitive_bytes_debug_is_redacted_and_scrub_is_explicit() {
        let mut allocation = Vec::with_capacity(64);
        allocation.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let mut bytes = SensitiveBytes::from_vec(allocation);
        let debug = format!("{bytes:?}");
        assert!(debug.contains("redacted:4 bytes"));
        assert!(!debug.contains("222, 173, 190, 239"));
        bytes.scrub();
        assert_eq!(bytes.as_ref(), &[0_u8; 4]);
        assert_eq!(bytes.0.capacity(), 64);
    }
}
