/// Geo lookup configuration for peer telemetry.
#[derive(ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiPeerGeo {
    /// Enable geo lookups for peer telemetry.
    #[config(default = "defaults::torii::peer_geo::ENABLED")]
    pub enabled: bool,
    /// Optional geo endpoint; required and HTTPS-only when lookups are enabled.
    pub endpoint: Option<Url>,
}
impl core::fmt::Debug for ToriiPeerGeo {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ToriiPeerGeo")
            .field("enabled", &self.enabled)
            .field(
                "endpoint",
                &RedactedConfigSecret::present(self.endpoint.is_some()),
            )
            .finish()
    }
}
impl Default for ToriiPeerGeo {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::peer_geo::ENABLED,
            endpoint: defaults::torii::peer_geo::endpoint(),
        }
    }
}
impl ToriiPeerGeo {
    fn parse(self) -> actual::ToriiPeerGeo {
        actual::ToriiPeerGeo {
            enabled: self.enabled,
            endpoint: self.endpoint,
        }
    }
}
#[cfg(test)]
mod torii_peer_geo_tests {
    use super::*;
    #[test]
    fn torii_peer_geo_parse_copies_enabled_and_endpoint() {
        let endpoint = Url::parse("https://geo.example").expect("valid endpoint");
        let parsed = ToriiPeerGeo {
            enabled: true,
            endpoint: Some(endpoint.clone()),
        }
        .parse();
        assert!(parsed.enabled);
        assert_eq!(
            parsed.endpoint.as_ref().map(Url::as_str),
            Some(endpoint.as_str())
        );
    }
    #[test]
    fn torii_peer_geo_parse_preserves_missing_endpoint() {
        let parsed = ToriiPeerGeo {
            enabled: true,
            endpoint: None,
        }
        .parse();
        assert_eq!(parsed.endpoint, None);
    }
}
