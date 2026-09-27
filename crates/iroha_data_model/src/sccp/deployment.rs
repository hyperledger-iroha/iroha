//! Destination deployments bound by SCCP v1 route revisions (`specs/sccp.md` §4.14.1, §3.4).
//!
//! Each route revision binds exactly one deployed destination contract. The deployment's
//! [`destination_word`](SccpDeploymentV1::destination_word) is the contract-visible identity
//! committed by transfer and control leaves; it is unique across all routes and revisions.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, bridge::SccpNetworkV1};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// First byte of every canonical TRON address (`0x41`).
pub const SCCP_TRON_ADDRESS_PREFIX_V1: u8 = 0x41;

/// Cell reference of TON contract code: representation hash and depth.
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::deployment::SccpTonCodeRefV1")]
pub struct SccpTonCodeRefV1 {
    /// Representation hash of the code cell.
    pub hash: [u8; 32],
    /// Depth of the code cell.
    pub depth: u16,
}

/// EVM deployment (Ethereum or BSC).
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::deployment::SccpEvmDeploymentV1")]
pub struct SccpEvmDeploymentV1 {
    /// Bridge-token contract address.
    pub address: [u8; 20],
    /// Keccak-256 of the deployed runtime bytecode (reviewed off-chain, §4.14.4).
    pub runtime_code_hash: [u8; 32],
}

/// TRON deployment.
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::deployment::SccpTronDeploymentV1")]
pub struct SccpTronDeploymentV1 {
    /// Bridge-token contract address, `0x41`-prefixed.
    pub address: [u8; 21],
    /// Keccak-256 of the deployed runtime bytecode (reviewed off-chain, §4.14.4).
    pub runtime_code_hash: [u8; 32],
}

/// TON deployment: the Jetton master (bridge) and its code references.
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::deployment::SccpTonDeploymentV1")]
pub struct SccpTonDeploymentV1 {
    /// Basechain account id of the Jetton master; Taira recomputes it at registration.
    pub master_account: [u8; 32],
    /// Minter (Jetton master) code.
    pub minter_code: SccpTonCodeRefV1,
    /// Jetton wallet code.
    pub wallet_code: SccpTonCodeRefV1,
    /// Replay bucket code.
    pub bucket_code: SccpTonCodeRefV1,
}

/// Destination contract bound by one route revision (`SccpDeploymentV1`, §4.14.1).
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
#[norito(tag = "family", content = "deployment")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::deployment::SccpDeploymentV1")]
pub enum SccpDeploymentV1 {
    /// Ethereum or BSC contract.
    #[codec(index = 0)]
    #[norito(rename = "evm")]
    Evm(SccpEvmDeploymentV1),
    /// TRON contract.
    #[codec(index = 1)]
    #[norito(rename = "tron")]
    Tron(SccpTronDeploymentV1),
    /// TON Jetton master.
    #[codec(index = 2)]
    #[norito(rename = "ton")]
    Ton(SccpTonDeploymentV1),
}

impl SccpDeploymentV1 {
    /// Return the contract-visible `destination_word` of §3.4.
    ///
    /// EVM: `word(address20)`, twelve zero bytes then the address. TRON: `word(address20)` of
    /// the address without its `0x41` prefix. TON: the Jetton master's account id.
    #[must_use]
    pub fn destination_word(&self) -> [u8; 32] {
        match self {
            Self::Evm(deployment) => evm_word(&deployment.address),
            Self::Tron(deployment) => {
                let mut address = [0_u8; 20];
                address.copy_from_slice(&deployment.address[1..]);
                evm_word(&address)
            }
            Self::Ton(deployment) => deployment.master_account,
        }
    }

    /// Return whether this deployment family and address shape fit `network`.
    ///
    /// EVM fits Ethereum and BSC with a nonzero address; TRON fits TRON with the `0x41`
    /// prefix and a nonzero remainder; TON fits TON with a nonzero account id. Nothing fits
    /// `sora-taira`.
    #[must_use]
    pub fn fits_network(&self, network: SccpNetworkV1) -> bool {
        match (self, network) {
            (Self::Evm(deployment), SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet) => {
                deployment.address != [0; 20]
            }
            (Self::Tron(deployment), SccpNetworkV1::TronMainnet) => {
                deployment.address[0] == SCCP_TRON_ADDRESS_PREFIX_V1
                    && deployment.address[1..].iter().any(|byte| *byte != 0)
            }
            (Self::Ton(deployment), SccpNetworkV1::TonMainnet) => {
                deployment.master_account != [0; 32]
            }
            _ => false,
        }
    }
}

fn evm_word(address: &[u8; 20]) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[12..].copy_from_slice(address);
    word
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

    fn code(seed: u8) -> SccpTonCodeRefV1 {
        SccpTonCodeRefV1 {
            hash: [seed; 32],
            depth: u16::from(seed),
        }
    }

    fn evm(address: [u8; 20]) -> SccpDeploymentV1 {
        SccpDeploymentV1::Evm(SccpEvmDeploymentV1 {
            address,
            runtime_code_hash: [0xc0; 32],
        })
    }

    fn tron(address: [u8; 21]) -> SccpDeploymentV1 {
        SccpDeploymentV1::Tron(SccpTronDeploymentV1 {
            address,
            runtime_code_hash: [0xc1; 32],
        })
    }

    fn ton(master_account: [u8; 32]) -> SccpDeploymentV1 {
        SccpDeploymentV1::Ton(SccpTonDeploymentV1 {
            master_account,
            minter_code: code(1),
            wallet_code: code(2),
            bucket_code: code(3),
        })
    }

    fn tron_address(body: [u8; 20]) -> [u8; 21] {
        let mut address = [0_u8; 21];
        address[0] = SCCP_TRON_ADDRESS_PREFIX_V1;
        address[1..].copy_from_slice(&body);
        address
    }

    #[test]
    fn destination_word_vectors() {
        // EVM: the §3.4 control-leaf example destination word, 12 zero bytes then 0x22 × 20.
        let mut expected = [0_u8; 32];
        expected[12..].copy_from_slice(&[0x22; 20]);
        assert_eq!(evm([0x22; 20]).destination_word(), expected);

        let address: [u8; 20] = core::array::from_fn(|index| u8::try_from(index + 1).unwrap());
        let mut expected = [0_u8; 32];
        expected[12..].copy_from_slice(&address);
        assert_eq!(evm(address).destination_word(), expected);

        // TRON: the 0x41 prefix is dropped, the remaining 20 bytes are left-padded.
        assert_eq!(tron(tron_address(address)).destination_word(), expected);
        assert_eq!(
            tron(tron_address([0x22; 20])).destination_word(),
            evm([0x22; 20]).destination_word()
        );

        // TON: the Jetton master account id is taken verbatim.
        let master: [u8; 32] = core::array::from_fn(|index| u8::try_from(0xa0 + index).unwrap());
        assert_eq!(ton(master).destination_word(), master);
    }

    #[test]
    fn fits_network_matrix() {
        let cases = [
            (evm([0x22; 20]), [false, true, true, false, false]),
            (
                tron(tron_address([0x22; 20])),
                [false, false, false, true, false],
            ),
            (ton([0x33; 32]), [false, false, false, false, true]),
        ];
        for (deployment, fits) in cases {
            for (network, expected) in NETWORKS.into_iter().zip(fits) {
                assert_eq!(
                    deployment.fits_network(network),
                    expected,
                    "{deployment:?} on {network:?}"
                );
            }
        }
    }

    #[test]
    fn fits_network_rejects_degenerate_addresses() {
        assert!(!evm([0; 20]).fits_network(SccpNetworkV1::EthereumMainnet));
        assert!(!evm([0; 20]).fits_network(SccpNetworkV1::BscMainnet));
        let mut one = [0_u8; 20];
        one[19] = 1;
        assert!(evm(one).fits_network(SccpNetworkV1::EthereumMainnet));

        assert!(!tron(tron_address([0; 20])).fits_network(SccpNetworkV1::TronMainnet));
        let mut wrong_prefix = tron_address([0x22; 20]);
        wrong_prefix[0] = 0x42;
        assert!(!tron(wrong_prefix).fits_network(SccpNetworkV1::TronMainnet));
        assert!(!tron([0; 21]).fits_network(SccpNetworkV1::TronMainnet));
        assert!(tron(tron_address(one)).fits_network(SccpNetworkV1::TronMainnet));

        assert!(!ton([0; 32]).fits_network(SccpNetworkV1::TonMainnet));
        let mut last = [0_u8; 32];
        last[31] = 1;
        assert!(ton(last).fits_network(SccpNetworkV1::TonMainnet));
    }

    #[test]
    fn binary_and_json_roundtrip() {
        for deployment in [
            evm([0x22; 20]),
            tron(tron_address([0x23; 20])),
            ton([0x24; 32]),
        ] {
            let encoded = deployment.encode();
            assert_eq!(
                SccpDeploymentV1::decode_all(&mut encoded.as_slice()).expect("decode"),
                deployment
            );
            let framed = norito::to_bytes(&deployment).expect("frame");
            assert_eq!(
                norito::decode_from_bytes::<SccpDeploymentV1>(&framed).expect("decode frame"),
                deployment
            );
            let json = norito::json::to_json(&deployment).expect("serialize");
            assert_eq!(
                norito::json::from_json::<SccpDeploymentV1>(&json).expect("deserialize"),
                deployment
            );
        }
        let reference = code(9);
        let json = norito::json::to_json(&reference).expect("serialize");
        assert_eq!(
            norito::json::from_json::<SccpTonCodeRefV1>(&json).expect("deserialize"),
            reference
        );
    }

    #[test]
    fn unknown_family_and_fields_are_rejected() {
        let mut value = norito::json::to_value(&evm([0x22; 20])).expect("to value");
        let norito::json::Value::Object(object) = &mut value else {
            panic!("deployment JSON is an object");
        };
        let tag = object
            .keys()
            .find(|key| key.as_str() != "deployment")
            .cloned()
            .expect("tag key");
        object.insert(tag, norito::json::Value::String("solana".to_owned()));
        let hostile = norito::json::to_json(&value).expect("serialize");
        assert!(norito::json::from_json::<SccpDeploymentV1>(&hostile).is_err());

        let mut value = norito::json::to_value(&SccpEvmDeploymentV1 {
            address: [1; 20],
            runtime_code_hash: [2; 32],
        })
        .expect("to value");
        value
            .as_object_mut()
            .expect("object")
            .insert("verifier_key_hash".to_owned(), norito::json::Value::Null);
        let hostile = norito::json::to_json(&value).expect("serialize");
        assert!(norito::json::from_json::<SccpEvmDeploymentV1>(&hostile).is_err());

        for unsupported_tag in [3_u32, 4, u32::MAX] {
            let mut encoded = unsupported_tag.encode();
            encoded.extend_from_slice(&[0x11; 160]);
            assert!(
                SccpDeploymentV1::decode_all(&mut encoded.as_slice()).is_err(),
                "deployment tag {unsupported_tag} unexpectedly decoded"
            );
        }
    }
}
