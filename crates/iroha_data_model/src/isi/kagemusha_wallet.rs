//! Closed KAGEMUSHA ledger instructions. Embedded objects retain their existing canonical
//! G1 frames; Core authenticates them and verifies complete native proofs before monetary effects.
use super::*;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, asset::AssetBalanceScope};

/// Source-finalized load query result. An unsigned body remains distinct from the immutable
/// signed voucher bytes; neither an HTTP success nor this codec alone establishes finality.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema, iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLoadIssuanceV1")]
pub struct KagemushaWalletLoadIssuanceV1 {
    /// Original stable issuance retry identity.
    pub request_id: [u8; 32],
    /// Authenticated payer whose finalized transaction funded the deposit.
    pub payer: AccountId,
    /// Original fixed voucher body, including transaction, height, ordinal and historical signer.
    pub body: crate::kagemusha::KagemushaWalletLoadVoucherBodyV1,
    /// First published canonical voucher bytes, or `None` while publication is pending.
    pub voucher: Option<Vec<u8>>,
}

/// Displayed online load charge, separate from the net offline amount.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLoadChargeV1")]
pub struct KagemushaWalletLoadChargeV1 {
    /// Canonical `KagemushaWalletChargeQuoteV1` frame.
    pub quote: Vec<u8>,
    /// Exact account bound by the signed quote.
    pub beneficiary: AccountId,
}

/// Closed operation set; arbitrary method names and proof verdicts are not accepted.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerActionV1")]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum KagemushaWalletLedgerActionV1 {
    /// Permanently segregate the consenting reserve account's exact balance bucket.
    Register {
        /// Canonical `KagemushaWalletSchemeV1` frame.
        scheme: Vec<u8>,
        /// Canonical `KagemushaWalletAssetScopeV1` frame.
        asset: Vec<u8>,
        /// Account submitting its consent; must hold the dedicated asset permission.
        reserve: AccountId,
        /// Exact immutable balance partition.
        balance_scope: AssetBalanceScope,
        /// Canonical LoadAuthorization-role certificate frame.
        load_authorizer: Vec<u8>,
    },
    /// Complete canonical activation frame; native Bootstrap verification is mandatory.
    Activate(Vec<u8>),
    /// Canonical signed unused-enrollment abandonment frame.
    Abandon(Vec<u8>),
    /// Complete canonical Retiring/later committed package proving the closing ordinal.
    CloseLoads(Vec<u8>),
    /// Debit the authenticated payer and record a unique successive issuance atomically.
    IssueLoad {
        /// Wallet incarnation.
        wallet: [u8; 32],
        /// Stable nonzero retry identity.
        request_id: [u8; 32],
        /// Net offline amount in registered atomic units.
        amount: u128,
        /// Optional exact displayed online charge.
        charge: Option<KagemushaWalletLoadChargeV1>,
    },
    /// Complete canonical Unload claim, paying its bound account at most once.
    Unload(Vec<u8>),
    /// Complete canonical fee claim, paying only its fixed historical beneficiary.
    ClaimFee(Vec<u8>),
    /// Retain a root-authenticated historical signer certificate.
    RetainCertificate {
        /// Registered asset binding.
        asset: [u8; 32],
        /// Canonical signer certificate frame.
        certificate: Vec<u8>,
    },
    /// Retain an issuer-authenticated credential and exact certificate set.
    RetainCredential {
        /// Canonical credential frame.
        credential: Vec<u8>,
        /// Canonical certificate-set frame.
        certificates: Vec<u8>,
    },
    /// Retain the complete signed Request and historical fee terms by their digests.
    RetainRequest(Vec<u8>),
    /// Freeze the first exact voucher bytes for an already recorded issuance. Submission
    /// requires the dedicated permission scoped to its historical signer certificate.
    PublishVoucher {
        /// Original stable issuance request identity.
        request_id: [u8; 32],
        /// Canonical voucher frame signed by the historical `LoadAuthorization` key.
        voucher: Vec<u8>,
    },
    /// Select the root-authenticated load signer used for future issuance. Existing
    /// issuance, certificates and published bytes retain their original identities.
    RotateLoadAuthorizer {
        /// Exact registered asset digest.
        asset: [u8; 32],
        /// Canonical LoadAuthorization-role certificate under this scheme's root.
        certificate: Vec<u8>,
    },
}

isi! {
    /// One exact KAGEMUSHA ledger operation under a registered scheme.
    #[derive(DeriveJsonSerialize, DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerV1")]
    pub struct KagemushaWalletLedgerV1 {
        /// Exact scheme identity; embedded objects must match.
        pub scheme: [u8; 32],
        /// Closed operation payload.
        pub action: KagemushaWalletLedgerActionV1,
    }
}
impl crate::seal::Instruction for KagemushaWalletLedgerV1 {}
impl KagemushaWalletLedgerV1 {
    /// Canonical first-release instruction wire identity.
    pub const WIRE_ID: &'static str = "iroha.kagemusha.wallet.ledger.v1";
    /// Construct a typed instruction. Core performs bounded object decoding and authorization.
    #[must_use]
    pub const fn new(scheme: [u8; 32], action: KagemushaWalletLedgerActionV1) -> Self {
        Self { scheme, action }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issuance_query_codec_preserves_pending_and_published_forms() {
        let fixtures: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))
        .unwrap();
        let row = fixtures["objects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["type"].as_str() == Some("KagemushaWalletLoadVoucherV1"))
            .unwrap();
        let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
        let voucher: crate::kagemusha::KagemushaWalletLoadVoucherV1 =
            norito::decode_from_bytes(&bytes).unwrap();
        let payer = AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![0x67; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        for published in [None, Some(bytes)] {
            let response = KagemushaWalletLoadIssuanceV1 {
                request_id: [3; 32],
                payer: payer.clone(),
                body: voucher.body,
                voucher: published,
            };
            let encoded = norito::to_bytes(&response).unwrap();
            let decoded: KagemushaWalletLoadIssuanceV1 = norito::decode_canonical_with_limits(
                &encoded,
                norito::canonical_decode_limits(encoded.len()),
            )
            .unwrap();
            assert_eq!(decoded, response);
        }
    }
    #[test]
    #[ignore = "explicit maintenance capture of the current KAGEMUSHA ledger instruction"]
    fn print_ledger_identity_capture() {
        let value = KagemushaWalletLedgerV1::new(
            [1; 32],
            KagemushaWalletLedgerActionV1::IssueLoad {
                wallet: [2; 32],
                request_id: [3; 32],
                amount: 7,
                charge: None,
            },
        );
        let row = super::super::generated_record_identity_tests::capture(value);
        println!(
            "KAGEMUSHA_LEDGER_IDENTITY={}",
            norito::json::to_json(&row).unwrap()
        );
    }
    #[test]
    fn ledger_instruction_roundtrips_through_canonical_registry_and_json() {
        let value = KagemushaWalletLedgerV1::new(
            [1; 32],
            KagemushaWalletLedgerActionV1::IssueLoad {
                wallet: [2; 32],
                request_id: [3; 32],
                amount: 7,
                charge: None,
            },
        );
        let bytes = norito::to_bytes(&value).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<KagemushaWalletLedgerV1>(&bytes).unwrap(),
            value
        );
        let json = norito::json::to_json(&value).unwrap();
        assert_eq!(
            norito::json::from_str::<KagemushaWalletLedgerV1>(&json).unwrap(),
            value
        );
        let registry = super::super::registry::default();
        let boxed: InstructionBox = value.clone().into();
        assert_eq!(
            boxed.as_any().downcast_ref::<KagemushaWalletLedgerV1>(),
            Some(&value)
        );
        assert_eq!(
            registry.wire_id(core::any::type_name::<KagemushaWalletLedgerV1>()),
            Some(KagemushaWalletLedgerV1::WIRE_ID)
        );
        assert_eq!(
            registry
                .decode(KagemushaWalletLedgerV1::WIRE_ID, &bytes)
                .unwrap()
                .unwrap()
                .as_any()
                .downcast_ref::<KagemushaWalletLedgerV1>(),
            Some(&value)
        );
    }
}
