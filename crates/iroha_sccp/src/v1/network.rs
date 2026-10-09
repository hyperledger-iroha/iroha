//! SCCP v1 profiles, identity words and lanes (spec §2).
//!
//! The five first-release profiles, their 1-byte tags, 32-byte identity words,
//! `network_bytes(p) = tag(p) ‖ identity_word(p)` (33 bytes) and
//! `lane_bytes(source, target) = network_bytes(source) ‖ network_bytes(target)` (66 bytes).
//! Taira's identity word is its runtime `NetworkId` (the genesis header hash); nothing
//! Taira-specific is compiled in, so every function takes the Taira network id as input.

use iroha_data_model::bridge::SccpNetworkV1;

use super::{
    constants::{
        BSC_CHAIN_ID, CODEC_EVM_ADDRESS20, CODEC_TAIRA_ACCOUNT, CODEC_TON_ACCOUNT36,
        CODEC_TRON_ADDRESS21, DOMAIN_BSC, DOMAIN_ETHEREUM, DOMAIN_SORA_TAIRA, DOMAIN_TON,
        DOMAIN_TRON, ETHEREUM_CHAIN_ID, MAX_VOID_FROZEN_RANGE_EVM, MAX_VOID_FROZEN_RANGE_TON,
        ROUTE_ID_BSC, ROUTE_ID_ETHEREUM, ROUTE_ID_TON, ROUTE_ID_TRON, TAG_BSC, TAG_ETHEREUM,
        TAG_SORA_TAIRA, TAG_TON, TAG_TRON, TON_GLOBAL_ID, TRON_CHAIN_ID,
    },
    hashes::word_u64,
};

/// Every first-release profile in tag order.
pub const ALL_NETWORKS: [SccpNetworkV1; 5] = [
    SccpNetworkV1::SoraTaira,
    SccpNetworkV1::EthereumMainnet,
    SccpNetworkV1::BscMainnet,
    SccpNetworkV1::TronMainnet,
    SccpNetworkV1::TonMainnet,
];

/// The four external profiles in tag order.
pub const EXTERNAL_NETWORKS: [SccpNetworkV1; 4] = [
    SccpNetworkV1::EthereumMainnet,
    SccpNetworkV1::BscMainnet,
    SccpNetworkV1::TronMainnet,
    SccpNetworkV1::TonMainnet,
];

/// Profile tag `0x40..=0x44` (§2.1).
#[must_use]
pub const fn tag(network: SccpNetworkV1) -> u8 {
    match network {
        SccpNetworkV1::SoraTaira => TAG_SORA_TAIRA,
        SccpNetworkV1::EthereumMainnet => TAG_ETHEREUM,
        SccpNetworkV1::BscMainnet => TAG_BSC,
        SccpNetworkV1::TronMainnet => TAG_TRON,
        SccpNetworkV1::TonMainnet => TAG_TON,
    }
}

/// SCCP domain id of a profile (Taira 0, ETH 1, BSC 2, TON 4, TRON 5).
#[must_use]
pub const fn domain(network: SccpNetworkV1) -> u32 {
    match network {
        SccpNetworkV1::SoraTaira => DOMAIN_SORA_TAIRA,
        SccpNetworkV1::EthereumMainnet => DOMAIN_ETHEREUM,
        SccpNetworkV1::BscMainnet => DOMAIN_BSC,
        SccpNetworkV1::TronMainnet => DOMAIN_TRON,
        SccpNetworkV1::TonMainnet => DOMAIN_TON,
    }
}

/// Profile of an SCCP domain id, if any.
#[must_use]
pub const fn network_from_domain(domain: u32) -> Option<SccpNetworkV1> {
    match domain {
        DOMAIN_SORA_TAIRA => Some(SccpNetworkV1::SoraTaira),
        DOMAIN_ETHEREUM => Some(SccpNetworkV1::EthereumMainnet),
        DOMAIN_BSC => Some(SccpNetworkV1::BscMainnet),
        DOMAIN_TRON => Some(SccpNetworkV1::TronMainnet),
        DOMAIN_TON => Some(SccpNetworkV1::TonMainnet),
        _ => None,
    }
}

/// Route id text of an external profile (§2.3); `None` for Taira.
#[must_use]
pub const fn route_id(network: SccpNetworkV1) -> Option<&'static str> {
    match network {
        SccpNetworkV1::SoraTaira => None,
        SccpNetworkV1::EthereumMainnet => Some(ROUTE_ID_ETHEREUM),
        SccpNetworkV1::BscMainnet => Some(ROUTE_ID_BSC),
        SccpNetworkV1::TronMainnet => Some(ROUTE_ID_TRON),
        SccpNetworkV1::TonMainnet => Some(ROUTE_ID_TON),
    }
}

/// Account codec of an external profile's own accounts (§3.1, §3.2); `None` for Taira.
///
/// This is the recipient codec of Taira → X and the sender codec of X → Taira.
#[must_use]
pub const fn external_account_codec(network: SccpNetworkV1) -> Option<u8> {
    match network {
        SccpNetworkV1::SoraTaira => None,
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet => Some(CODEC_EVM_ADDRESS20),
        SccpNetworkV1::TronMainnet => Some(CODEC_TRON_ADDRESS21),
        SccpNetworkV1::TonMainnet => Some(CODEC_TON_ACCOUNT36),
    }
}

/// Account codec of a profile's own accounts, Taira included (codec 3).
#[must_use]
pub const fn account_codec(network: SccpNetworkV1) -> u8 {
    match external_account_codec(network) {
        Some(codec) => codec,
        None => CODEC_TAIRA_ACCOUNT,
    }
}

/// EVM `block.chainid` of an EVM-family profile (ETH, BSC, TRON); `None` otherwise.
#[must_use]
pub const fn evm_chain_id(network: SccpNetworkV1) -> Option<u64> {
    match network {
        SccpNetworkV1::EthereumMainnet => Some(ETHEREUM_CHAIN_ID),
        SccpNetworkV1::BscMainnet => Some(BSC_CHAIN_ID),
        SccpNetworkV1::TronMainnet => Some(TRON_CHAIN_ID),
        SccpNetworkV1::SoraTaira | SccpNetworkV1::TonMainnet => None,
    }
}

/// Most nonces one frozen void of an external destination may name (§4.16, §5.1.8): the
/// `voidFrozen` range bound on ETH, BSC and TRON, one `sccp_void_frozen` bucket on TON; `None`
/// for Taira, which is never a destination.
#[must_use]
pub const fn max_void_frozen_range(network: SccpNetworkV1) -> Option<u64> {
    match network {
        SccpNetworkV1::SoraTaira => None,
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet | SccpNetworkV1::TronMainnet => {
            Some(MAX_VOID_FROZEN_RANGE_EVM)
        }
        SccpNetworkV1::TonMainnet => Some(MAX_VOID_FROZEN_RANGE_TON),
    }
}

/// Two's-complement `int256` word of a signed 32-bit value.
#[must_use]
pub fn word_i32(value: i32) -> [u8; 32] {
    let fill = if value < 0 { 0xff } else { 0x00 };
    let mut word = [fill; 32];
    word[28..].copy_from_slice(&value.to_be_bytes());
    word
}

/// Identity word of a profile (§2.1).
///
/// Taira: the 32 bytes of its `NetworkId`; ETH `word(1)`; BSC `word(56)`; TRON
/// `word(0x2b6653dc)`; TON two's-complement `int256(-239)`.
#[must_use]
pub fn identity_word(network: SccpNetworkV1, taira_network_id: &[u8; 32]) -> [u8; 32] {
    match network {
        SccpNetworkV1::SoraTaira => *taira_network_id,
        SccpNetworkV1::EthereumMainnet => word_u64(ETHEREUM_CHAIN_ID),
        SccpNetworkV1::BscMainnet => word_u64(BSC_CHAIN_ID),
        SccpNetworkV1::TronMainnet => word_u64(TRON_CHAIN_ID),
        SccpNetworkV1::TonMainnet => word_i32(TON_GLOBAL_ID),
    }
}

/// `network_bytes(p) = tag(p) ‖ identity_word(p)` (33 bytes).
#[must_use]
pub fn network_bytes(network: SccpNetworkV1, taira_network_id: &[u8; 32]) -> [u8; 33] {
    let mut out = [0_u8; 33];
    out[0] = tag(network);
    out[1..].copy_from_slice(&identity_word(network, taira_network_id));
    out
}

/// `lane_bytes(source, target)` (66 bytes); `None` unless exactly one endpoint is Taira.
#[must_use]
pub fn lane_bytes(
    source: SccpNetworkV1,
    target: SccpNetworkV1,
    taira_network_id: &[u8; 32],
) -> Option<[u8; 66]> {
    if source.is_sora() == target.is_sora() {
        return None;
    }
    let mut out = [0_u8; 66];
    out[..33].copy_from_slice(&network_bytes(source, taira_network_id));
    out[33..].copy_from_slice(&network_bytes(target, taira_network_id));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIRA: [u8; 32] = [0x11; 32];

    #[test]
    fn domains_roundtrip_and_taira_has_no_route() {
        for network in ALL_NETWORKS {
            assert_eq!(network_from_domain(domain(network)), Some(network));
            assert_eq!(domain(network), network.domain_id());
        }
        assert_eq!(network_from_domain(3), None);
        assert_eq!(network_from_domain(6), None);
        assert_eq!(route_id(SccpNetworkV1::SoraTaira), None);
    }

    #[test]
    fn identity_words_match_the_spec_table() {
        assert_eq!(identity_word(SccpNetworkV1::SoraTaira, &TAIRA), TAIRA);
        assert_eq!(
            identity_word(SccpNetworkV1::EthereumMainnet, &TAIRA),
            word_u64(1)
        );
        assert_eq!(
            identity_word(SccpNetworkV1::BscMainnet, &TAIRA),
            word_u64(56)
        );
        let tvm_word = identity_word(SccpNetworkV1::TronMainnet, &TAIRA);
        assert_eq!(tvm_word[..28], [0; 28]);
        assert_eq!(tvm_word[28..], [0x2b, 0x66, 0x53, 0xdc]);
        let ton_word = identity_word(SccpNetworkV1::TonMainnet, &TAIRA);
        assert_eq!(ton_word[..31], [0xff; 31]);
        assert_eq!(ton_word[31], 0x11);
    }

    #[test]
    fn word_i32_is_twos_complement() {
        assert_eq!(word_i32(0), [0; 32]);
        assert_eq!(word_i32(-1), [0xff; 32]);
        let mut expected = [0_u8; 32];
        expected[31] = 5;
        assert_eq!(word_i32(5), expected);
    }

    #[test]
    fn network_and_lane_bytes_layout() {
        let bytes = network_bytes(SccpNetworkV1::BscMainnet, &TAIRA);
        assert_eq!(bytes[0], 0x42);
        assert_eq!(bytes[32], 56);
        let lane = lane_bytes(SccpNetworkV1::SoraTaira, SccpNetworkV1::TonMainnet, &TAIRA)
            .expect("taira to ton");
        assert_eq!(lane[0], 0x40);
        assert_eq!(lane[1..33], TAIRA);
        assert_eq!(lane[33], 0x44);
        assert_eq!(lane[65], 0x11);
        let inbound = lane_bytes(SccpNetworkV1::TronMainnet, SccpNetworkV1::SoraTaira, &TAIRA)
            .expect("tron to taira");
        assert_eq!(inbound[0], 0x43);
        assert_eq!(inbound[33], 0x40);
        assert!(lane_bytes(SccpNetworkV1::SoraTaira, SccpNetworkV1::SoraTaira, &TAIRA).is_none());
        assert!(
            lane_bytes(
                SccpNetworkV1::EthereumMainnet,
                SccpNetworkV1::BscMainnet,
                &TAIRA
            )
            .is_none()
        );
    }

    #[test]
    fn codecs_and_chain_ids() {
        assert_eq!(account_codec(SccpNetworkV1::SoraTaira), 3);
        assert_eq!(account_codec(SccpNetworkV1::EthereumMainnet), 2);
        assert_eq!(account_codec(SccpNetworkV1::BscMainnet), 2);
        assert_eq!(account_codec(SccpNetworkV1::TronMainnet), 5);
        assert_eq!(account_codec(SccpNetworkV1::TonMainnet), 7);
        assert_eq!(external_account_codec(SccpNetworkV1::SoraTaira), None);
        assert_eq!(evm_chain_id(SccpNetworkV1::EthereumMainnet), Some(1));
        assert_eq!(evm_chain_id(SccpNetworkV1::BscMainnet), Some(56));
        assert_eq!(evm_chain_id(SccpNetworkV1::TronMainnet), Some(0x2b66_53dc));
        assert_eq!(evm_chain_id(SccpNetworkV1::TonMainnet), None);
    }

    #[test]
    fn frozen_void_ranges_follow_each_destination() {
        assert_eq!(max_void_frozen_range(SccpNetworkV1::SoraTaira), None);
        for network in [
            SccpNetworkV1::EthereumMainnet,
            SccpNetworkV1::BscMainnet,
            SccpNetworkV1::TronMainnet,
        ] {
            assert_eq!(max_void_frozen_range(network), Some(256));
        }
        assert_eq!(max_void_frozen_range(SccpNetworkV1::TonMainnet), Some(512));
    }
}
