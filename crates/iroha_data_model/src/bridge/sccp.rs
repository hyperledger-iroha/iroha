//! SCCP network profile wire type.
//!
//! `SccpNetworkV1` models only the first-release network inventory (`specs/sccp.md` preamble and
//! §2). There is no catch-all network or arbitrary network identifier: unsupported profiles must
//! fail decoding instead of being interpreted by node-local policy. Every other SCCP v1 type lives
//! in [`crate::sccp`].

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// A supported SCCP network profile for the V1 wire format.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "network", content = "profile")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp::SccpNetworkV1")]
pub enum SccpNetworkV1 {
    /// The sole production SORA endpoint admitted by SCCP V1.
    #[codec(index = 64)]
    #[norito(rename = "sora_taira")]
    SoraTaira,
    /// Ethereum mainnet.
    #[codec(index = 65)]
    #[norito(rename = "ethereum_mainnet")]
    EthereumMainnet,
    /// BNB Smart Chain mainnet.
    #[codec(index = 66)]
    #[norito(rename = "bsc_mainnet")]
    BscMainnet,
    /// TRON mainnet.
    #[codec(index = 67)]
    #[norito(rename = "tron_mainnet")]
    TronMainnet,
    /// TON mainnet (global id −239).
    #[codec(index = 68)]
    #[norito(rename = "ton_mainnet")]
    TonMainnet,
}
impl SccpNetworkV1 {
    /// Return the SCCP protocol domain carried by messages for this profile.
    #[must_use]
    pub const fn domain_id(self) -> u32 {
        match self {
            Self::SoraTaira => 0,
            Self::EthereumMainnet => 1,
            Self::BscMainnet => 2,
            Self::TonMainnet => 4,
            Self::TronMainnet => 5,
        }
    }
    /// Return the canonical, stable textual key for this exact profile.
    #[must_use]
    pub const fn profile_key(self) -> &'static str {
        match self {
            Self::SoraTaira => "sora-taira",
            Self::EthereumMainnet => "ethereum-mainnet",
            Self::BscMainnet => "bsc-mainnet",
            Self::TronMainnet => "tron-mainnet",
            Self::TonMainnet => "ton-mainnet",
        }
    }
    /// Parse an exact canonical profile key.
    ///
    /// Parsing is deliberately case-sensitive and accepts neither domain-wide
    /// aliases nor abbreviated chain names. This makes textual storage keys a
    /// one-to-one representation of the closed V1 network inventory.
    #[must_use]
    pub fn from_profile_key(profile: &str) -> Option<Self> {
        match profile {
            "sora-taira" => Some(Self::SoraTaira),
            "ethereum-mainnet" => Some(Self::EthereumMainnet),
            "bsc-mainnet" => Some(Self::BscMainnet),
            "tron-mainnet" => Some(Self::TronMainnet),
            "ton-mainnet" => Some(Self::TonMainnet),
            _ => None,
        }
    }
    /// Return whether this profile belongs to the SORA domain.
    #[must_use]
    pub const fn is_sora(self) -> bool {
        matches!(self, Self::SoraTaira)
    }
    /// Return whether this profile is a supported external SCCP endpoint.
    #[must_use]
    pub const fn is_external(self) -> bool {
        !self.is_sora()
    }
    /// Return whether V1 can safely admit this external network as a message source.
    ///
    /// Only families with exact value-moving source and destination implementations
    /// are representable in the first-release registry.
    #[must_use]
    pub const fn supports_native_inbound_source(self) -> bool {
        matches!(
            self,
            Self::EthereumMainnet | Self::BscMainnet | Self::TronMainnet | Self::TonMainnet
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::DecodeAll as _;
    const NETWORKS: [SccpNetworkV1; 5] = [
        SccpNetworkV1::SoraTaira,
        SccpNetworkV1::EthereumMainnet,
        SccpNetworkV1::BscMainnet,
        SccpNetworkV1::TronMainnet,
        SccpNetworkV1::TonMainnet,
    ];
    #[test]
    fn network_inventory_and_profile_keys_are_exact() {
        assert_eq!(NETWORKS.len(), 5);
        for network in NETWORKS {
            assert_eq!(
                SccpNetworkV1::from_profile_key(network.profile_key()),
                Some(network)
            );
        }
        for unsupported in [
            "",
            "sora-nexus",
            "sora_nexus",
            "ethereum",
            "ethereum-sepolia",
            "ETHEREUM-MAINNET",
            "bsc-testnet",
            "tron-nile",
            "tron-shasta",
            "solana-mainnet-beta",
            "solana-testnet",
            "solana_testnet",
            "ton",
            "ton-testnet",
            "TON-MAINNET",
            "ton_mainnet",
        ] {
            assert_eq!(SccpNetworkV1::from_profile_key(unsupported), None);
        }
        assert_eq!(SccpNetworkV1::SoraTaira.domain_id(), 0);
        assert_eq!(SccpNetworkV1::EthereumMainnet.domain_id(), 1);
        assert_eq!(SccpNetworkV1::BscMainnet.domain_id(), 2);
        assert_eq!(SccpNetworkV1::TronMainnet.domain_id(), 5);
        assert_eq!(SccpNetworkV1::TonMainnet.domain_id(), 4);
    }
    #[test]
    fn network_binary_roundtrips_cover_the_closed_inventory() {
        for network in NETWORKS {
            let encoded = network.encode();
            assert_eq!(
                SccpNetworkV1::decode_all(&mut encoded.as_slice()).expect("network decodes"),
                network
            );
        }
    }
    #[test]
    fn unknown_binary_network_tags_are_rejected() {
        for unsupported_tag in [
            0_u32,
            1,
            2,
            3,
            4,
            5,
            10,
            11,
            12,
            13,
            14,
            15,
            63,
            69,
            u32::MAX,
        ] {
            let encoded = unsupported_tag.encode();
            assert!(
                SccpNetworkV1::decode_all(&mut encoded.as_slice()).is_err(),
                "network tag {unsupported_tag} unexpectedly decoded"
            );
        }
    }
    #[test]
    fn binary_tags_use_the_fresh_final_v1_block() {
        let expected = [
            (SccpNetworkV1::SoraTaira, 0x40_u32),
            (SccpNetworkV1::EthereumMainnet, 0x41),
            (SccpNetworkV1::BscMainnet, 0x42),
            (SccpNetworkV1::TronMainnet, 0x43),
            (SccpNetworkV1::TonMainnet, 0x44),
        ];
        for (network, tag) in expected {
            assert_eq!(
                network.encode().get(..4),
                Some(tag.to_le_bytes().as_slice()),
                "wrong Norito tag for {network:?}"
            );
        }
    }
    #[test]
    fn unsupported_networks_are_not_json_decodable() {
        for profile in [
            "sora_nexus",
            "ethereum_sepolia",
            "bsc_testnet",
            "tron_nile",
            "tron_shasta",
            "solana_testnet",
            "ton_testnet",
            "unknown_network",
        ] {
            let json = format!(r#"{{"network":"{profile}","profile":null}}"#);
            assert!(
                norito::json::from_json::<SccpNetworkV1>(&json).is_err(),
                "unsupported profile {profile} unexpectedly decoded"
            );
        }
    }
    #[test]
    fn json_roundtrips_advertise_only_first_release_profiles() {
        for network in NETWORKS {
            let json = norito::json::to_json(&network).expect("network serializes");
            assert_eq!(
                norito::json::from_json::<SccpNetworkV1>(&json).expect("network decodes"),
                network
            );
        }
    }
    #[test]
    fn native_source_support_is_closed_to_exact_external_inventory() {
        for network in NETWORKS {
            assert_eq!(
                network.supports_native_inbound_source(),
                network.is_external()
            );
            assert_eq!(network.is_sora(), network == SccpNetworkV1::SoraTaira);
        }
    }
}
#[cfg(test)]
mod captured_sccp_schema_tests;
