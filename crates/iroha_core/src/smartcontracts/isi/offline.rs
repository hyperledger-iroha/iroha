//! Kagemusha offline-cash instruction execution.

mod kagemusha_terminal_registry;

use super::prelude::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use std::{
    collections::{BTreeSet, HashSet},
    io::Cursor,
    sync::LazyLock,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::{
        AssetBalancePolicy, AssetBalanceScope, AssetDefinitionId, AssetId,
        definition::ConfidentialPolicyMode,
    },
    confidential::ConfidentialStatus,
    isi::{
        error::{InstructionExecutionError, MathError},
        offline::{
            RedeemKagemushaRecursiveV2, RegisterOfflineDeviceAttestation,
            SetOfflineDeviceAttestationPolicy, TopUpKagemushaRecursiveV2,
        },
    },
    name::Name,
    offline::{
        KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBranchPathV2,
        KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaRecursiveSpendTopUpAnchorV2,
        KagemushaRequestAuthorizationV2, OFFLINE_REJECTION_REASON_PREFIX,
        OfflineAndroidAppAttestationPolicy, OfflineDeviceAttestationPolicy,
        OfflineDeviceAttestationRegistration, OfflineDeviceAttestationTrustedRoot,
        OfflineIosAppAttestationPolicy,
    },
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyRecord},
    query::error::FindError,
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_primitives::numeric::{Numeric, Quantity};
use p256::PublicKey as P256PublicKey;
use sha2::{Digest as _, Sha256};
use x509_parser::{
    extensions::ParsedExtension,
    prelude::{FromDer as _, X509Certificate},
    time::ASN1Time,
};

const CAN_MANAGE_OFFLINE_ESCROW_PERMISSION: &str = "CanManageOfflineEscrow";
const CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION: &str =
    "CanManageOfflineDeviceAttestationPolicy";
static OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY: LazyLock<Name> = LazyLock::new(|| {
    "offline_device_attestation_policy"
        .parse()
        .expect("static Offline device attestation policy key")
});

fn labeled_invariant(label: &str, message: impl Into<String>) -> InstructionExecutionError {
    let message = message.into();
    let boxed: Box<str> = format!("{OFFLINE_REJECTION_REASON_PREFIX}{label}:{message}").into();
    InstructionExecutionError::InvariantViolation(boxed)
}

fn resolve_offline_escrow_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinitionId,
) -> Result<Option<AccountId>, Error> {
    let asset_definition = state_transaction.world.asset_definition(definition)?;
    if crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        crate::smartcontracts::isi::domain::isi::ensure_offline_escrow_account(
            &asset_definition,
            asset_definition.owned_by(),
            state_transaction,
        )?;
        let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.chain_id(),
            definition,
        );
        return Ok(Some(derived));
    }
    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(definition)
    {
        return Ok(Some(account.clone()));
    }
    if state_transaction.settlement.offline.escrow_required {
        return Err(labeled_invariant(
            "escrow_missing",
            format!("offline escrow account not configured for asset definition `{definition}`"),
        )
        .into());
    }
    Ok(None)
}

pub(crate) fn is_offline_escrow_source_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    let asset_definition = state_transaction
        .world
        .asset_definition(source_id.definition())?;

    if crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.chain_id(),
            source_id.definition(),
        );
        return Ok(&derived == source_id.account());
    }

    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(source_id.definition())
    {
        return Ok(account == source_id.account());
    }
    Ok(false)
}

fn ensure_distinct_offline_escrow_account(
    escrow_account: &AccountId,
    participant_account: &AccountId,
    participant_role: &str,
    definition_id: &AssetDefinitionId,
) -> Result<(), Error> {
    if escrow_account == participant_account {
        return Err(labeled_invariant(
            "escrow_self_reference",
            format!(
                "offline escrow account for asset definition `{definition_id}` must be distinct from {participant_role} account `{participant_account}`",
            ),
        )
        .into());
    }
    Ok(())
}

fn canonical_kagemusha_asset_id(
    state_transaction: &StateTransaction<'_, '_>,
    asset: &AssetId,
) -> Result<AssetId, Error> {
    let definition = state_transaction
        .world
        .asset_definition(asset.definition())?;
    let scope = match definition.balance_scope_policy() {
        AssetBalancePolicy::Global => {
            if !matches!(asset.scope(), AssetBalanceScope::Global) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "global assets cannot be addressed with dataspace scope".into(),
                )
                .into());
            }
            AssetBalanceScope::Global
        }
        AssetBalancePolicy::DataspaceRestricted => match asset.scope() {
            AssetBalanceScope::Dataspace(dataspace) => AssetBalanceScope::Dataspace(*dataspace),
            AssetBalanceScope::Global => state_transaction
                .world
                .resolve_asset_balance_scope(asset.definition())?,
        },
    };

    Ok(AssetId::with_scope(
        asset.definition().clone(),
        asset.account().clone(),
        scope,
    ))
}

fn kagemusha_escrow_asset_id(source_asset: &AssetId, escrow_account: AccountId) -> AssetId {
    AssetId::with_scope(
        source_asset.definition().clone(),
        escrow_account,
        source_asset.scope().clone(),
    )
}

fn withdraw_numeric_asset_exact(
    state_transaction: &mut StateTransaction<'_, '_>,
    id: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    if amount.mantissa().is_negative() {
        return Err(MathError::NegativeValue.into());
    }
    let amount =
        Quantity::from_canonical_numeric(amount.clone()).map_err(|_| MathError::NegativeValue)?;
    let asset = state_transaction
        .world
        .assets
        .get_mut(id)
        .ok_or_else(|| FindError::Asset(id.clone().into()))?;
    let quantity: &mut Quantity = &mut *asset;
    let candidate = quantity
        .checked_sub(&amount)
        .map_err(|_| MathError::NotEnoughQuantity)?;
    *quantity = candidate;
    if (**asset).is_zero() {
        assert!(
            state_transaction
                .world
                .remove_asset_and_metadata(id)
                .is_some()
        );
    }
    Ok(())
}

fn deposit_numeric_asset_exact(
    state_transaction: &mut StateTransaction<'_, '_>,
    id: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    if amount.mantissa().is_negative() {
        return Err(MathError::NegativeValue.into());
    }
    let amount =
        Quantity::from_canonical_numeric(amount.clone()).map_err(|_| MathError::NegativeValue)?;
    let is_nonzero = {
        let dst = state_transaction
            .world
            .asset_or_insert_exact(id, Quantity::zero())?;
        let quantity: &mut Quantity = &mut *dst;
        *quantity = quantity
            .checked_add(&amount)
            .map_err(|_| MathError::Overflow)?;
        !quantity.is_zero()
    };
    if is_nonzero {
        state_transaction.world.track_nonzero_asset_holder(id);
    }
    Ok(())
}

fn reserve_kagemusha_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    let escrow_account = resolve_offline_escrow_account(state_transaction, asset.definition())?;
    let escrow_account = escrow_account.ok_or_else(|| {
        labeled_invariant(
            "escrow_missing",
            format!(
                "offline escrow account not configured for asset definition `{}`",
                asset.definition(),
            ),
        )
    })?;
    if amount.is_zero() {
        return Ok(());
    }
    ensure_distinct_offline_escrow_account(
        &escrow_account,
        asset.account(),
        "note",
        asset.definition(),
    )?;
    let source_asset = canonical_kagemusha_asset_id(state_transaction, asset)?;
    let escrow_asset = kagemusha_escrow_asset_id(&source_asset, escrow_account);
    withdraw_numeric_asset_exact(state_transaction, &source_asset, amount)?;
    if let Err(err) = deposit_numeric_asset_exact(state_transaction, &escrow_asset, amount) {
        deposit_numeric_asset_exact(state_transaction, &source_asset, amount)
            .expect("offline escrow reservation refund must succeed after failed deposit");
        return Err(err);
    }
    Ok(())
}

/// Execution logic for Kagemusha offline-cash instructions.
pub mod isi {
    use super::*;

    const KAGEMUSHA_DEVICE_REGISTRATION_DOMAIN: &str = "kagemusha-device-registration";
    const KAGEMUSHA_ATTESTATION_CHALLENGE_REPLAY_DOMAIN: &str = "kagemusha-attestation-challenge";
    const KAGEMUSHA_ATTESTATION_REPORT_REPLAY_DOMAIN: &str = "kagemusha-attestation-report";
    const KAGEMUSHA_ATTESTATION_EVIDENCE_REPLAY_DOMAIN: &str = "kagemusha-attestation-evidence";
    const KAGEMUSHA_V2_DEVICE_LINEAGE_DOMAIN: &str = "kagemusha-v2-device-lineage";
    const KAGEMUSHA_V2_OPERATION_DOMAIN: &str = "kagemusha-v2-operation";
    const KAGEMUSHA_V2_NONCE_DOMAIN: &str = "kagemusha-v2-authorization-nonce";
    const KAGEMUSHA_V2_PAYLOAD_DOMAIN: &str = "kagemusha-v2-payload";
    const KAGEMUSHA_V2_REQUEST_DOMAIN: &str = "kagemusha-v2-request";
    const KAGEMUSHA_V2_BRANCH_EXACT_DOMAIN: &str = "kagemusha-v2-redeemed-branch";
    const KAGEMUSHA_V2_BRANCH_DESCENDANT_DOMAIN: &str = "kagemusha-v2-redeemed-descendant";
    const KAGEMUSHA_V2_TRANSITION_SELECTED_DOMAIN: &str = "kagemusha-v2-transition-selected";
    const KAGEMUSHA_V2_TRANSITION_CHOICE_DOMAIN: &str = "kagemusha-v2-transition-choice";
    const KAGEMUSHA_V2_AUTHORIZED_CHANGE_CHILD_DOMAIN: &str =
        "kagemusha-v2-authorized-change-child";
    const OFFLINE_ATTESTATION_EVIDENCE_PREFIX: &[u8] = b"offline-device-attestation-evidence-v1";
    const KAGEMUSHA_ATTESTATION_RECENT_BLOCK_WINDOW: u64 = 128;
    const OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST: &str = "ios-appattest";
    const OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT: &str = "android-keymint";
    const OFFLINE_ATTESTATION_IOS_ASSERTION_SCHEME: &str = "apple-appattest-counter-v1";
    const OFFLINE_ATTESTATION_IOS_ASSERTION_ALGORITHM: &str = "app-attest-p256";
    const OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME: &str =
        "android-keymint-ecdsa-p256-usage-limit-v1";
    const OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM: &str = "ecdsa-p256-sha256";
    const OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN: usize = 65;
    const OFFLINE_ATTESTATION_MAX_REPORT_BYTES: usize = 64 * 1024;
    const OFFLINE_ATTESTATION_MAX_EVIDENCE_BYTES: usize = 128 * 1024;
    const OFFLINE_ATTESTATION_APP_ATTEST_AUTH_DATA_MIN_LEN: usize = 37 + 16 + 2;
    const OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA: u8 = 0x40;
    const OFFLINE_ATTESTATION_APP_ATTEST_NONCE_OID: &str = "1.2.840.113635.100.8.2";
    const OFFLINE_ATTESTATION_ANDROID_KEY_OID: &str = "1.3.6.1.4.1.11129.2.1.17";
    const OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION: &str = "production";
    const OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT: &str = "development";
    const OFFLINE_ATTESTATION_IOS_AAGUID_PRODUCTION: &[u8; 16] = b"appattest\0\0\0\0\0\0\0";
    const OFFLINE_ATTESTATION_IOS_AAGUID_DEVELOPMENT: &[u8; 16] = b"appattestdevelop";
    const OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_TRUSTED_ENVIRONMENT: i64 = 1;
    const OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_STRONG_BOX: i64 = 2;
    const OFFLINE_ATTESTATION_ANDROID_TAG_USAGE_COUNT_LIMIT: u32 = 405;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ALL_APPLICATIONS: u32 = 600;
    const OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_APPLICATION_ID: u32 = 709;
    const APPLE_APP_ATTESTATION_ROOT_CA_DER_B64: &str = concat!(
        "MIICITCCAaegAwIBAgIQC/O+DvHN0uD7jG5yH2IXmDAKBggqhkjOPQQDAzBSMSYw",
        "JAYDVQQDDB1BcHBsZSBBcHAgQXR0ZXN0YXRpb24gUm9vdCBDQTETMBEGA1UECgwK",
        "QXBwbGUgSW5jLjETMBEGA1UECAwKQ2FsaWZvcm5pYTAeFw0yMDAzMTgxODMyNTNa",
        "Fw00NTAzMTUwMDAwMDBaMFIxJjAkBgNVBAMMHUFwcGxlIEFwcCBBdHRlc3RhdGlv",
        "biBSb290IENBMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9y",
        "bmlhMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAERTHhmLW07ATaFQIEVwTtT4dyctdh",
        "NbJhFs/Ii2FdCgAHGbpphY3+d8qjuDngIN3WVhQUBHAoMeQ/cLiP1sOUtgjqK9au",
        "Yen1mMEvRq9Sk3Jm5X8U62H+xTD3FE9TgS41o0IwQDAPBgNVHRMBAf8EBTADAQH/",
        "MB0GA1UdDgQWBBSskRBTM72+aEH/pwyp5frq5eWKoTAOBgNVHQ8BAf8EBAMCAQYw",
        "CgYIKoZIzj0EAwMDaAAwZQIwQgFGnByvsiVbpTKwSga0kP0e8EeDS4+sQmTvb7vn",
        "53O5+FRXgeLhpJ06ysC5PrOyAjEAp5U4xDgEgllF7En3VcE3iexZZtKeYnpqtijV",
        "oyFraWVIyd/dganmrduC1bmTBGwD"
    );
    const ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64: &str = concat!(
        "MIIFHDCCAwSgAwIBAgIJAPHBcqaZ6vUdMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNV",
        "BAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMjIwMzIwMTgwNzQ4WhcNNDIwMzE1MTgw",
        "NzQ4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0B",
        "AQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdS",
        "Sxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7",
        "tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggj",
        "nar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGq",
        "C4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQ",
        "oVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+O",
        "JtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/Eg",
        "sTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRi",
        "igHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+M",
        "RPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9E",
        "aDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5Um",
        "AGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1Ud",
        "IwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYD",
        "VR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQB8cMqTllHc8U+qCrOlg3H7",
        "174lmaCsbo/bJ0C17JEgMLb4kvrqsXZs01U3mB/qABg/1t5Pd5AORHARs1hhqGIC",
        "W/nKMav574f9rZN4PC2ZlufGXb7sIdJpGiO9ctRhiLuYuly10JccUZGEHpHSYM2G",
        "tkgYbZba6lsCPYAAP83cyDV+1aOkTf1RCp/lM0PKvmxYN10RYsK631jrleGdcdkx",
        "oSK//mSQbgcWnmAEZrzHoF1/0gso1HZgIn0YLzVhLSA/iXCX4QT2h3J5z3znluKG",
        "1nv8NQdxei2DIIhASWfu804CA96cQKTTlaae2fweqXjdN1/v2nqOhngNyz1361mF",
        "mr4XmaKH/ItTwOe72NI9ZcwS1lVaCvsIkTDCEXdm9rCNPAY10iTunIHFXRh+7KPz",
        "lHGewCq/8TOohBRn0/NNfh7uRslOSZ/xKbN9tMBtw37Z8d2vvnXq/YWdsm1+JLVw",
        "n6yYD/yacNJBlwpddla8eaVMjsF6nBnIgQOf9zKSe06nSTqvgwUHosgOECZJZ1Eu",
        "zbH4yswbt02tKtKEFhx+v+OTge/06V+jGsqTWLsfrOCNLuA8H++z+pUENmpqnnHo",
        "vaI47gC+TNpkgYGkkBT6B/m/U01BuOBBTzhIlMEZq9qkDWuM2cA5kW5V3FJUcfHn",
        "w1IdYIg2Wxg7yHcQZemFQg=="
    );
    const ANDROID_KEY_ATTESTATION_CA_DER_B64: &str = concat!(
        "MIICIjCCAaigAwIBAgIRAISp0Cl7DrWK5/8OgN52BgUwCgYIKoZIzj0EAwMwUjEc",
        "MBoGA1UEAwwTS2V5IEF0dGVzdGF0aW9uIENBMTEQMA4GA1UECwwHQW5kcm9pZDET",
        "MBEGA1UECgwKR29vZ2xlIExMQzELMAkGA1UEBhMCVVMwHhcNMjUwNzE3MjIzMjE4",
        "WhcNMzUwNzE1MjIzMjE4WjBSMRwwGgYDVQQDDBNLZXkgQXR0ZXN0YXRpb24gQ0Ex",
        "MRAwDgYDVQQLDAdBbmRyb2lkMRMwEQYDVQQKDApHb29nbGUgTExDMQswCQYDVQQG",
        "EwJVUzB2MBAGByqGSM49AgEGBSuBBAAiA2IABCPaI3FO3z5bBQo8cuiEas4HjqCt",
        "G/mLFfRT0MsIssPBEEU5Cfbt6sH5yOAxqEi5QagpU1yX4HwnGb7OtBYpDTB57uH5",
        "Eczm34A5FNijV3s0/f0UPl7zbJcTx6xwqMIRq6NCMEAwDwYDVR0TAQH/BAUwAwEB",
        "/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFFIyuyz7RkOb3NaBqQ5lZuA0QepA",
        "MAoGCCqGSM49BAMDA2gAMGUCMETfjPO/HwqReR2CS7p0ZWoD/LHs6hDi422opifH",
        "EUaYLxwGlT9SLdjkVpz0UUOR5wIxAIoGyxGKRHVTpqpGRFiJtQEOOTp/+s1GcxeY",
        "uR2zh/80lQyu9vAFCj6E4AXc+osmRg=="
    );

    struct IosAppAttestReport {
        auth_data: Vec<u8>,
        certificates: Vec<Vec<u8>>,
    }

    struct IosAppAttestAuthData {
        rp_id_hash: [u8; 32],
        sign_count: u32,
        aaguid: [u8; 16],
        credential_id: Vec<u8>,
        cose_key: Vec<u8>,
    }

    struct AndroidKeyMintReport {
        certificates: Vec<Vec<u8>>,
    }

    struct AndroidKeyDescription {
        attestation_security_level: i64,
        keymint_security_level: i64,
        attestation_challenge: Vec<u8>,
        usage_count_limit: Option<i64>,
        all_applications: bool,
        application_id: Option<AndroidAttestationApplicationId>,
    }

    struct AndroidAttestationApplicationId {
        packages: Vec<AndroidAttestationPackageInfo>,
        signature_digests: Vec<Vec<u8>>,
    }

    struct AndroidAttestationPackageInfo {
        package_name: String,
    }

    #[derive(Copy, Clone)]
    struct DerTag {
        class_bits: u8,
        constructed: bool,
        number: u32,
        first_byte: u8,
    }

    struct DerReader<'a> {
        input: &'a [u8],
        offset: usize,
    }

    impl<'a> DerReader<'a> {
        fn new(input: &'a [u8]) -> Self {
            Self { input, offset: 0 }
        }

        fn sequence(input: &'a [u8]) -> Result<Self, Error> {
            let mut reader = Self::new(input);
            let sequence = reader.read_expected(0x30)?;
            if reader.has_remaining() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension has trailing DER bytes",
                )
                .into());
            }
            Ok(Self::new(sequence))
        }

        fn has_remaining(&self) -> bool {
            self.offset < self.input.len()
        }

        fn read_expected(&mut self, expected_tag: u8) -> Result<&'a [u8], Error> {
            let (tag, value) = self.read_tlv()?;
            if tag != expected_tag {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension has an unexpected DER tag",
                )
                .into());
            }
            Ok(value)
        }

        fn read_single_expected(&mut self, expected_tag: u8) -> Result<&'a [u8], Error> {
            let value = self.read_expected(expected_tag)?;
            if self.has_remaining() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER has trailing inner bytes",
                )
                .into());
            }
            Ok(value)
        }

        fn read_null(&mut self) -> Result<(), Error> {
            let value = self.read_single_expected(0x05)?;
            if !value.is_empty() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER NULL must be empty",
                )
                .into());
            }
            Ok(())
        }

        fn read_integer(&mut self) -> Result<i64, Error> {
            der_integer_to_i64(self.read_expected(0x02)?)
        }

        fn read_enumerated(&mut self) -> Result<i64, Error> {
            der_integer_to_i64(self.read_expected(0x0A)?)
        }

        fn read_octet_string(&mut self) -> Result<Vec<u8>, Error> {
            Ok(self.read_expected(0x04)?.to_vec())
        }

        fn read_sequence_bytes(&mut self) -> Result<Vec<u8>, Error> {
            Ok(self.read_expected(0x30)?.to_vec())
        }

        fn read_tlv(&mut self) -> Result<(u8, &'a [u8]), Error> {
            let (tag, value) = self.read_tlv_full()?;
            if tag.number >= 31 || tag.first_byte & 0x1F == 0x1F {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER high-tag form is unsupported in this position",
                )
                .into());
            }
            Ok((tag.first_byte, value))
        }

        fn read_tlv_full(&mut self) -> Result<(DerTag, &'a [u8]), Error> {
            let (tag, value, _) = self.read_tlv_full_with_raw()?;
            Ok((tag, value))
        }

        fn read_tlv_full_with_raw(&mut self) -> Result<(DerTag, &'a [u8], &'a [u8]), Error> {
            if self.offset >= self.input.len() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER ended early",
                )
                .into());
            }
            let start = self.offset;
            let first_byte = self.input[self.offset];
            self.offset += 1;
            let mut number = u32::from(first_byte & 0x1F);
            if number == 0x1F {
                number = 0;
                let mut octets = 0usize;
                let mut first_high_tag_octet = true;
                loop {
                    if self.offset >= self.input.len() || octets >= 5 {
                        return Err(labeled_invariant(
                            "invalid_attestation",
                            "attestation extension DER high-tag number is invalid",
                        )
                        .into());
                    }
                    let byte = self.input[self.offset];
                    self.offset += 1;
                    octets += 1;
                    if first_high_tag_octet && byte & 0x7F == 0 {
                        return Err(labeled_invariant(
                            "invalid_attestation",
                            "attestation extension DER high-tag number is non-canonical",
                        )
                        .into());
                    }
                    first_high_tag_octet = false;
                    number = (number << 7) | u32::from(byte & 0x7F);
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                if number < 31 {
                    return Err(labeled_invariant(
                        "invalid_attestation",
                        "attestation extension DER high-tag number is non-canonical",
                    )
                    .into());
                }
            }
            let length = self.read_length()?;
            let end = self.offset.checked_add(length).ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER length overflow",
                )
            })?;
            if end > self.input.len() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER length exceeds input",
                )
                .into());
            }
            let value = &self.input[self.offset..end];
            self.offset = end;
            let raw = &self.input[start..end];
            Ok((
                DerTag {
                    class_bits: first_byte & 0xC0,
                    constructed: first_byte & 0x20 != 0,
                    number,
                    first_byte,
                },
                value,
                raw,
            ))
        }

        fn read_length(&mut self) -> Result<usize, Error> {
            if self.offset >= self.input.len() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER length is missing",
                )
                .into());
            }
            let first = self.input[self.offset];
            self.offset += 1;
            if first & 0x80 == 0 {
                return Ok(usize::from(first));
            }
            let octets = usize::from(first & 0x7F);
            if octets == 0 || octets > 4 || self.offset + octets > self.input.len() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER length encoding is unsupported",
                )
                .into());
            }
            let first_length_octet = self.input[self.offset];
            if first_length_octet == 0 || (octets == 1 && first_length_octet < 0x80) {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER length encoding is non-canonical",
                )
                .into());
            }
            let mut length = 0usize;
            for _ in 0..octets {
                length = (length << 8) | usize::from(self.input[self.offset]);
                self.offset += 1;
            }
            Ok(length)
        }
    }

    fn der_integer_to_i64(bytes: &[u8]) -> Result<i64, Error> {
        if bytes.is_empty() || bytes.len() > 8 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation extension integer is out of range",
            )
            .into());
        }
        if bytes.len() > 1
            && ((bytes[0] == 0 && bytes[1] & 0x80 == 0)
                || (bytes[0] == 0xFF && bytes[1] & 0x80 != 0))
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation extension integer encoding is non-canonical",
            )
            .into());
        }
        if bytes[0] & 0x80 != 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation extension integer is out of range",
            )
            .into());
        }
        let mut value = 0i64;
        for byte in bytes {
            value = (value << 8) | i64::from(*byte);
        }
        Ok(value)
    }

    fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn sha256_concat(left: &[u8], right: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    fn decode_trusted_root_der(root_b64: &str) -> Result<Vec<u8>, Error> {
        BASE64_STANDARD.decode(root_b64).map_err(|_| {
            labeled_invariant("invalid_attestation", "trusted root DER is invalid").into()
        })
    }

    fn default_offline_device_attestation_policy() -> Result<OfflineDeviceAttestationPolicy, Error>
    {
        Ok(OfflineDeviceAttestationPolicy {
            version: 1,
            trusted_roots: vec![
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned(),
                    der: decode_trusted_root_der(APPLE_APP_ATTESTATION_ROOT_CA_DER_B64)?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
                    der: decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
                OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned(),
                    der: decode_trusted_root_der(ANDROID_KEY_ATTESTATION_CA_DER_B64)?,
                    not_before_ms: None,
                    not_after_ms: None,
                },
            ],
            revoked_certificate_sha256: Vec::new(),
            ios_apps: Vec::new(),
            android_apps: Vec::new(),
            require_ios_app_policy: false,
            require_android_app_policy: false,
        })
    }

    fn effective_offline_device_attestation_policy(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<OfflineDeviceAttestationPolicy, Error> {
        match state_transaction
            .world
            .smart_contract_state
            .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
        {
            Some(bytes) => norito::decode_from_bytes::<OfflineDeviceAttestationPolicy>(bytes)
                .map_err(|err| {
                    labeled_invariant(
                        "invalid_attestation_policy",
                        format!("failed to decode Offline device attestation policy: {err}"),
                    )
                    .into()
                }),
            None => default_offline_device_attestation_policy(),
        }
    }

    fn normalize_policy_ascii(value: &str, field: &str) -> Result<String, Error> {
        let trimmed = value.trim();
        if trimmed.is_empty() || !trimmed.is_ascii() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                format!("Offline device attestation policy {field} must be non-empty ASCII"),
            )
            .into());
        }
        if trimmed != value {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                format!(
                    "Offline device attestation policy {field} must not contain surrounding whitespace"
                ),
            )
            .into());
        }
        Ok(value.to_owned())
    }

    fn normalize_sha256_digest(digest: &[u8], field: &str) -> Result<[u8; 32], Error> {
        digest.try_into().map_err(|_| {
            labeled_invariant(
                "invalid_attestation_policy",
                format!(
                    "Offline device attestation policy {field} must be a 32-byte SHA-256 digest"
                ),
            )
            .into()
        })
    }

    fn trusted_root_is_active(
        root: &OfflineDeviceAttestationTrustedRoot,
        block_unix_timestamp_ms: u64,
    ) -> bool {
        root.not_before_ms
            .is_none_or(|not_before_ms| block_unix_timestamp_ms >= not_before_ms)
            && root
                .not_after_ms
                .is_none_or(|not_after_ms| block_unix_timestamp_ms <= not_after_ms)
    }

    fn validate_offline_attestation_policy(
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        if policy.version != 1 {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy version is unsupported",
            )
            .into());
        }
        if policy.trusted_roots.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy must include at least one trusted root",
            )
            .into());
        }

        let evaluation_time = x509_evaluation_time(block_unix_timestamp_ms)?;
        let mut root_hashes = HashSet::new();
        for root in &policy.trusted_roots {
            match root.platform.as_str() {
                OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST
                | OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {}
                _ => {
                    return Err(labeled_invariant(
                        "invalid_attestation_policy",
                        "Offline device attestation policy trusted root platform is unsupported",
                    )
                    .into());
                }
            }
            if root.der.is_empty()
                || root
                    .not_before_ms
                    .zip(root.not_after_ms)
                    .is_some_and(|(not_before, not_after)| not_before > not_after)
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy trusted root lifetime is invalid",
                )
                .into());
            }
            let digest = sha256_bytes(&root.der);
            if !root_hashes.insert(digest) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy contains a duplicate trusted root",
                )
                .into());
            }
            let certificate = parse_x509_certificate_der(&root.der)?;
            validate_x509_certificate_critical_extensions(&certificate)?;
            if trusted_root_is_active(root, block_unix_timestamp_ms) {
                validate_x509_certificate_time(&certificate, evaluation_time)?;
            }
            if !x509_certificate_is_ca(&certificate)? {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy trusted root must be a CA certificate",
                )
                .into());
            }
        }

        let mut revoked = HashSet::new();
        for digest in &policy.revoked_certificate_sha256 {
            let digest = normalize_sha256_digest(digest, "revoked certificate digest")?;
            if digest == [0u8; 32] || !revoked.insert(digest) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy has an invalid revoked certificate digest",
                )
                .into());
            }
        }

        let mut ios_apps = HashSet::new();
        for app in &policy.ios_apps {
            let team_id = normalize_policy_ascii(&app.team_id, "iOS Team ID")?.to_ascii_uppercase();
            let bundle_id = normalize_policy_ascii(&app.bundle_id, "iOS bundle ID")?;
            let environment =
                normalize_policy_ascii(&app.environment, "iOS environment")?.to_ascii_lowercase();
            if environment != OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION
                && environment != OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy iOS environment must be production or development",
                )
                .into());
            }
            if !ios_apps.insert((team_id, bundle_id, environment)) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy contains a duplicate iOS app identity",
                )
                .into());
            }
        }

        let mut android_apps = HashSet::new();
        for app in &policy.android_apps {
            let package_name = normalize_policy_ascii(&app.package_name, "Android package name")?;
            if app.signing_certificate_sha256.is_empty() {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy Android app must include signing digests",
                )
                .into());
            }
            let mut signing_digests = Vec::with_capacity(app.signing_certificate_sha256.len());
            let mut seen_signers = HashSet::new();
            for digest in &app.signing_certificate_sha256 {
                let digest = normalize_sha256_digest(digest, "Android signing certificate digest")?;
                if digest == [0u8; 32] || !seen_signers.insert(digest) {
                    return Err(labeled_invariant(
                        "invalid_attestation_policy",
                        "Offline device attestation policy Android app has an invalid signing digest",
                    )
                    .into());
                }
                signing_digests.push(digest);
            }
            signing_digests.sort_unstable();
            if !android_apps.insert((package_name, signing_digests)) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy contains a duplicate Android app identity",
                )
                .into());
            }
        }

        if policy.require_ios_app_policy && policy.ios_apps.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy requires iOS apps but none are configured",
            )
            .into());
        }
        if policy.require_android_app_policy && policy.android_apps.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy requires Android apps but none are configured",
            )
            .into());
        }
        Ok(())
    }

    fn trusted_root_der_for_platform(
        policy: &OfflineDeviceAttestationPolicy,
        platform: &str,
        block_unix_timestamp_ms: u64,
    ) -> Result<Vec<Vec<u8>>, Error> {
        let roots: Vec<_> = policy
            .trusted_roots
            .iter()
            .filter(|root| {
                root.platform == platform && trusted_root_is_active(root, block_unix_timestamp_ms)
            })
            .map(|root| root.der.clone())
            .collect();
        if roots.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy has no active trusted root for platform",
            )
            .into());
        }
        Ok(roots)
    }

    fn policy_revoked_certificate_hashes(
        policy: &OfflineDeviceAttestationPolicy,
    ) -> Result<HashSet<[u8; 32]>, Error> {
        let mut revoked = HashSet::new();
        for digest in &policy.revoked_certificate_sha256 {
            let digest = normalize_sha256_digest(digest, "revoked certificate digest")?;
            if digest == [0u8; 32] || !revoked.insert(digest) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy has an invalid revoked certificate digest",
                )
                .into());
            }
        }
        Ok(revoked)
    }

    fn x509_evaluation_time(block_unix_timestamp_ms: u64) -> Result<ASN1Time, Error> {
        #[cfg(test)]
        let block_unix_timestamp_ms = if block_unix_timestamp_ms == 0 {
            1_800_000_000_000
        } else {
            block_unix_timestamp_ms
        };
        let seconds = i64::try_from(block_unix_timestamp_ms / 1_000).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "offline device attestation block timestamp is out of range",
            )
        })?;
        ASN1Time::from_timestamp(seconds).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "offline device attestation block timestamp cannot be represented as ASN.1 time",
            )
            .into()
        })
    }

    fn parse_x509_certificate_der(certificate_der: &[u8]) -> Result<X509Certificate<'_>, Error> {
        let (remaining, certificate) =
            X509Certificate::from_der(certificate_der).map_err(|_| {
                labeled_invariant(
                    "invalid_attestation",
                    "attestation certificate DER is invalid",
                )
            })?;
        if !remaining.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation certificate DER has trailing bytes",
            )
            .into());
        }
        Ok(certificate)
    }

    fn validate_x509_certificate_critical_extensions(
        certificate: &X509Certificate<'_>,
    ) -> Result<(), Error> {
        for extension in certificate.extensions() {
            if !extension.critical {
                continue;
            }
            match extension.parsed_extension() {
                ParsedExtension::UnsupportedExtension { .. }
                | ParsedExtension::ParseError { .. }
                | ParsedExtension::Unparsed => {
                    return Err(labeled_invariant(
                        "invalid_attestation",
                        "attestation certificate contains an unsupported critical extension",
                    )
                    .into());
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn x509_certificate_is_ca(certificate: &X509Certificate<'_>) -> Result<bool, Error> {
        let Some(basic_constraints) = certificate.basic_constraints().map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "attestation certificate basic constraints are invalid",
            )
        })?
        else {
            return Ok(false);
        };
        if !basic_constraints.critical || !basic_constraints.value.ca {
            return Ok(false);
        }
        let Some(key_usage) = certificate.key_usage().map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "attestation certificate key usage is invalid",
            )
        })?
        else {
            return Ok(false);
        };
        Ok(key_usage.critical && key_usage.value.key_cert_sign())
    }

    fn x509_leaf_allows_digital_signature(
        certificate: &X509Certificate<'_>,
    ) -> Result<bool, Error> {
        let Some(key_usage) = certificate.key_usage().map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "attestation certificate key usage is invalid",
            )
        })?
        else {
            return Ok(false);
        };
        Ok(key_usage.critical && key_usage.value.digital_signature())
    }

    fn validate_x509_certificate_time(
        certificate: &X509Certificate<'_>,
        evaluation_time: ASN1Time,
    ) -> Result<(), Error> {
        if certificate.validity().is_valid_at(evaluation_time) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "invalid_attestation",
                "attestation certificate is not valid at the block timestamp",
            )
            .into())
        }
    }

    fn verify_x509_certificate_signature(
        certificate: &X509Certificate<'_>,
        issuer: &X509Certificate<'_>,
    ) -> Result<(), Error> {
        certificate
            .verify_signature(Some(issuer.public_key()))
            .map_err(|_| {
                labeled_invariant(
                    "invalid_attestation",
                    "attestation certificate signature chain is invalid",
                )
                .into()
            })
    }

    fn validate_attestation_certificate_chain(
        certificate_chain: &[Vec<u8>],
        trusted_roots_der: &[Vec<u8>],
        revoked_certificate_sha256: &HashSet<[u8; 32]>,
        evaluation_time: ASN1Time,
    ) -> Result<(), Error> {
        if certificate_chain.is_empty() || trusted_roots_der.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation certificate chain is empty",
            )
            .into());
        }
        let mut seen = HashSet::new();
        for certificate_der in certificate_chain {
            let certificate_sha256 = sha256_bytes(certificate_der);
            if revoked_certificate_sha256.contains(&certificate_sha256) {
                return Err(labeled_invariant(
                    "revoked_attestation",
                    "attestation certificate is revoked by Offline device attestation policy",
                )
                .into());
            }
            if !seen.insert(certificate_sha256) {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation certificate chain contains duplicate certificates",
                )
                .into());
            }
            let certificate = parse_x509_certificate_der(certificate_der)?;
            validate_x509_certificate_critical_extensions(&certificate)?;
            validate_x509_certificate_time(&certificate, evaluation_time)?;
        }

        let parsed_chain = certificate_chain
            .iter()
            .map(|certificate_der| parse_x509_certificate_der(certificate_der))
            .collect::<Result<Vec<_>, _>>()?;
        let leaf = parsed_chain.first().ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "attestation certificate chain is empty",
            )
        })?;
        if x509_certificate_is_ca(leaf)? || !x509_leaf_allows_digital_signature(leaf)? {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation leaf certificate must be an end-entity signing certificate",
            )
            .into());
        }
        for pair in parsed_chain.windows(2) {
            let certificate = &pair[0];
            let issuer = &pair[1];
            if certificate.issuer() != issuer.subject() || !x509_certificate_is_ca(issuer)? {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation certificate issuer chain is invalid",
                )
                .into());
            }
            verify_x509_certificate_signature(certificate, issuer)?;
        }

        let tail_der = certificate_chain.last().expect("chain is non-empty");
        let tail = parsed_chain.last().expect("chain is non-empty");
        for root_der in trusted_roots_der {
            if revoked_certificate_sha256.contains(&sha256_bytes(root_der)) {
                continue;
            }
            let root = parse_x509_certificate_der(root_der)?;
            validate_x509_certificate_critical_extensions(&root)?;
            validate_x509_certificate_time(&root, evaluation_time)?;
            if !x509_certificate_is_ca(&root)? {
                continue;
            }
            if tail_der == root_der {
                if tail.issuer() == tail.subject() {
                    verify_x509_certificate_signature(tail, tail)?;
                }
                return Ok(());
            }
            if tail.issuer() == root.subject() {
                verify_x509_certificate_signature(tail, &root)?;
                return Ok(());
            }
        }

        #[cfg(test)]
        if tail.issuer() == tail.subject()
            && x509_certificate_is_ca(tail)?
            && x509_certificate_is_offline_attestation_test_root(tail)
        {
            verify_x509_certificate_signature(tail, tail)?;
            return Ok(());
        }

        Err(labeled_invariant(
            "invalid_attestation",
            "attestation certificate chain is not anchored in a trusted root",
        )
        .into())
    }

    #[cfg(test)]
    fn x509_certificate_is_offline_attestation_test_root(
        certificate: &X509Certificate<'_>,
    ) -> bool {
        certificate.subject().iter_common_name().any(|name| {
            name.as_str()
                .is_ok_and(|value| value == "Iroha Offline Attestation Test Root")
        })
    }

    fn x509_unique_extension_value(
        certificate: &X509Certificate<'_>,
        oid: &str,
        duplicate_message: &'static str,
    ) -> Result<Option<Vec<u8>>, Error> {
        let mut matches = certificate
            .extensions()
            .iter()
            .filter(|extension| extension.oid.to_string() == oid);
        let first = matches.next().map(|extension| extension.value.to_vec());
        if matches.next().is_some() {
            return Err(labeled_invariant("invalid_attestation", duplicate_message).into());
        }
        Ok(first)
    }

    fn x509_subject_public_key_bytes(certificate: &X509Certificate<'_>) -> Vec<u8> {
        certificate.public_key().subject_public_key.data.to_vec()
    }

    fn validate_attestation_protocol_string(
        subject: &'static str,
        field: &'static str,
        value: &str,
        error_label: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        if value.trim().is_empty() {
            return Err(labeled_invariant(
                error_label,
                format!("{subject} {field} must be non-empty"),
            ));
        }
        if value.trim() != value {
            return Err(labeled_invariant(
                error_label,
                format!("{subject} {field} must not contain surrounding whitespace"),
            ));
        }
        Ok(())
    }

    fn is_kagemusha_transparent_backend(backend: &str) -> bool {
        backend == crate::zk::ZK_BACKEND_HALO2_IPA || crate::zk::is_stark_fri_v1_backend(backend)
    }

    fn ensure_kagemusha_transparent_backend(
        backend: &str,
        backend_tag: BackendTag,
    ) -> Result<(), Error> {
        if BackendTag::is_pending_production_backend_label(backend) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline transparent proofs may not use pending-production proof backends",
            )
            .into());
        }
        if crate::zk::is_production_claim_backend_label(backend) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline transparent proofs may not use production-claim proof backends",
            )
            .into());
        }
        if backend_tag.is_pending_production_backend() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline transparent verifier records may not use pending-production backend tags",
            )
            .into());
        }
        if !is_kagemusha_transparent_backend(backend) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs require a transparent halo2/ipa or stark/fri backend",
            )
            .into());
        }
        let expected_tag = crate::zk::production_verify_backend_tag(backend).ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proof backend is not admitted by the production verifier registry",
            )
        })?;
        if backend_tag != expected_tag {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier backend tag does not match the transparent backend",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_transparent_attachment(attachment: &ProofAttachment) -> Result<(), Error> {
        if attachment.backend != attachment.proof.backend
            || attachment.backend != attachment.vk_ref.backend
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha proof backend, proof payload backend, and verifier key backend must match",
            )
            .into());
        }
        if attachment.vk_ref.name.trim().is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha proof verifier key id name must be non-empty",
            )
            .into());
        }
        let backend = attachment.backend.as_str();
        let backend_tag = if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            BackendTag::Halo2IpaPasta
        } else if crate::zk::is_stark_fri_v1_backend(backend) {
            BackendTag::Stark
        } else {
            BackendTag::Unsupported
        };
        ensure_kagemusha_transparent_backend(backend, backend_tag)
    }

    fn resolve_kagemusha_topup_shield_verifier(
        asset: &AssetDefinitionId,
        proof: &ProofAttachment,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyBox, VerifyingKeyRecord), Error> {
        ensure_kagemusha_transparent_attachment(proof)?;
        let zk_state = state_transaction
            .world
            .zk_assets
            .get(asset)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha top-up requires configured confidential asset state",
                )
            })?;
        let binding = zk_state.vk_shield.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up requires an asset-bound shield verifier key",
            )
        })?;
        if proof.vk_ref != binding.id || proof.backend != binding.id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up proof must reference the asset-bound shield verifier key",
            )
            .into());
        }
        if proof.vk_commitment != Some(binding.commitment) || binding.commitment == [0; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up verifier commitment does not match the asset binding",
            )
            .into());
        }
        let record = state_transaction
            .world
            .verifying_keys
            .get(&binding.id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha top-up shield verifier key is not registered",
                )
            })?;
        let circuit_key = (record.circuit_id.clone(), record.version);
        if record.status != ConfidentialStatus::Active
            || state_transaction
                .world
                .verifying_keys_by_circuit
                .get(&circuit_key)
                != Some(&binding.id)
        {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "Kagemusha top-up shield verifier circuit/version is not active",
            )
            .into());
        }
        let expected_schema_hash: [u8; 32] = Hash::new(
            crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2,
        )
        .into();
        if record.namespace != crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE
            || record.backend != BackendTag::Halo2IpaPasta
            || record.circuit_id != crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID
            || record.curve != "pallas"
            || record.public_inputs_schema_hash != expected_schema_hash
            || record.commitment != binding.commitment
            || record.max_proof_bytes == 0
            || proof.proof.bytes.len() > record.max_proof_bytes as usize
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up requires the canonical asset-bound shield-v2 verifier",
            )
            .into());
        }
        let vk_box = record.key.clone().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up shield verifier key is not available inline",
            )
        })?;
        if vk_box.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
            || vk_box.bytes.is_empty()
            || u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len)
            || crate::zk::hash_vk(&vk_box) != record.commitment
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha top-up inline shield verifier does not match its registry record",
            )
            .into());
        }
        crate::zk::confidential_v2::ensure_kagemusha_topup_shield_v2_canonical_vk_box(&vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_proof",
                    "Kagemusha top-up shield proof must be an OpenVerifyEnvelope",
                )
            })?;
        if envelope.backend != BackendTag::Halo2IpaPasta
            || envelope.circuit_id
                != crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID
            || envelope.public_inputs
                != crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2
            || envelope.vk_hash != binding.commitment
            || !envelope.aux.is_empty()
        {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha top-up shield proof envelope metadata is inconsistent",
            )
            .into());
        }
        if let Some(envelope_hash) = proof.envelope_hash {
            let expected_hash: [u8; 32] = Hash::new(&proof.proof.bytes).into();
            if envelope_hash != expected_hash {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha top-up shield envelope hash does not match its proof bytes",
                )
                .into());
            }
        }
        Ok((vk_box, record))
    }

    fn resolve_kagemusha_unshield_verifier(
        asset: &AssetDefinitionId,
        proof: &ProofAttachment,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyBox, VerifyingKeyRecord), Error> {
        ensure_kagemusha_transparent_attachment(proof)?;
        let zk_state = state_transaction
            .world
            .zk_assets
            .get(asset)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "recursive Kagemusha redemption requires configured shielded asset state",
                )
            })?;
        let binding = zk_state.vk_unshield.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redemption requires a bound unshield verifier key",
            )
        })?;
        if proof.vk_ref != binding.id || proof.backend != binding.id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof must reference the asset-bound unshield verifier key",
            )
            .into());
        }
        let Some(commitment) = proof.vk_commitment else {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof must publish the asset-bound verifier-key commitment",
            )
            .into());
        };
        if commitment == [0u8; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof must publish a non-zero asset-bound verifier-key commitment",
            )
            .into());
        }
        if commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier-key commitment does not match the asset binding",
            )
            .into());
        }
        let record = state_transaction
            .world
            .verifying_keys
            .get(&binding.id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "recursive Kagemusha redeem verifier key is not registered",
                )
            })?;
        if !record.is_active_at(state_transaction.block_height()) {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "recursive Kagemusha redeem verifier key is not active at the current block height",
            )
            .into());
        }
        if record.namespace != crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key is not in the Kagemusha namespace",
            )
            .into());
        }
        if record.commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier-key registry commitment does not match the asset binding",
            )
            .into());
        }
        if record.backend.is_pending_production_backend() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key uses a pending-production backend tag",
            )
            .into());
        }
        if record.backend != BackendTag::Halo2IpaPasta
            || proof.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redemption requires a transparent Halo2/IPA unshield verifier",
            )
            .into());
        }
        if record.curve != "pallas" {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key curve is not pallas",
            )
            .into());
        }
        if !crate::zk::confidential_v2::is_confidential_unshield_v3_circuit_id(&record.circuit_id) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redemption requires a confidential unshield v3 verifier",
            )
            .into());
        }
        let circuit_key = (record.circuit_id.clone(), record.version);
        match state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&circuit_key)
        {
            Some(active_id) if active_id == &binding.id => {}
            _ => {
                return Err(labeled_invariant(
                    "verifier_key_inactive",
                    "recursive Kagemusha redeem verifier circuit/version is not active",
                )
                .into());
            }
        }
        if record.max_proof_bytes == 0 {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key must publish a non-zero max_proof_bytes cap",
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(labeled_invariant(
                "invalid_proof",
                "recursive Kagemusha redeem proof exceeds verifier record max_proof_bytes",
            )
            .into());
        }
        let vk_box = record.key.clone().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem verifier key is not available inline",
            )
        })?;
        if vk_box.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
            || vk_box.bytes.is_empty()
            || u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len)
            || crate::zk::hash_vk(&vk_box) != record.commitment
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem inline verifier key does not match the registry record",
            )
            .into());
        }
        crate::zk::confidential_v2::ensure_confidential_unshield_v3_canonical_vk_box(&vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_proof",
                    "recursive Kagemusha redeem proof must be an OpenVerifyEnvelope",
                )
            })?;
        if envelope.vk_hash == [0u8; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem proof envelope verifier-key hash must be non-zero",
            )
            .into());
        }
        if envelope.backend != BackendTag::Halo2IpaPasta
            || envelope.circuit_id != record.circuit_id
            || envelope.vk_hash != record.commitment
            || !envelope.aux.is_empty()
        {
            return Err(labeled_invariant(
                "invalid_proof",
                "recursive Kagemusha redeem proof envelope metadata mismatch",
            )
            .into());
        }
        let expected_schema =
            crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1;
        let expected_schema_hash: [u8; 32] = Hash::new(expected_schema).into();
        if envelope.public_inputs != expected_schema
            || record.public_inputs_schema_hash != expected_schema_hash
        {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "recursive Kagemusha redeem proof public-input schema mismatch",
            )
            .into());
        }
        if let Some(envelope_hash) = proof.envelope_hash {
            let expected_hash: [u8; 32] = Hash::new(&proof.proof.bytes).into();
            if envelope_hash != expected_hash {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "recursive Kagemusha redeem proof envelope hash does not match the submitted envelope",
                )
                .into());
            }
        }
        Ok((vk_box, record))
    }

    fn ensure_kagemusha_v2_redeem_public_inputs(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV2,
        state_transaction: &StateTransaction<'_, '_>,
        vk_record: &VerifyingKeyRecord,
    ) -> Result<(), Error> {
        if !crate::zk::confidential_v2::is_confidential_unshield_v3_circuit_id(
            &vk_record.circuit_id,
        ) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V2 redemption requires an unshield-v3 proof attachment",
            )
            .into());
        }
        let statement = &request.bundle.statement;
        let zero = [0u8; 32];
        let expected_change = request
            .redemption
            .change_output
            .as_ref()
            .map_or(zero, |change| change.note_commitment);
        let (
            input_commitments,
            proof_nullifiers,
            proof_output,
            proof_root,
            public_amount,
            asset_tag,
            chain_tag,
        ) = crate::zk::confidential_v2::parse_unshield_public_inputs_v3(
            &request.redeem_proof.proof.bytes,
        )
        .map_err(|err| labeled_invariant("invalid_proof", err.to_string()))?;
        let expected_public_amount =
            crate::zk::confidential_v2::encode_confidential_amount_v2(request.amount.atomic_units);
        let expected_asset_tag = crate::zk::confidential_v2::derive_confidential_asset_tag_v2(
            &statement.asset.to_string(),
        );
        let expected_chain_tag = crate::zk::confidential_v2::derive_confidential_chain_tag_v2(
            state_transaction.chain_id().as_str(),
        );
        if input_commitments != [statement.current_note.note_commitment, zero]
            || proof_nullifiers != [statement.current_note.spend_nullifier, zero]
            || proof_output != expected_change
            || proof_root != statement.final_root
            || public_amount != expected_public_amount
            || asset_tag != expected_asset_tag
            || chain_tag != expected_chain_tag
        {
            return Err(labeled_invariant(
                "final_commitment_mismatch",
                "Kagemusha V2 unshield-v3 proof is not bound to the exact note, nullifier, root, scaled amount, asset, chain, and full redemption output",
            )
            .into());
        }
        let parsed_binding = iroha_data_model::offline::KagemushaUnshieldPublicInputsBindingV2 {
            input_commitment_0: input_commitments[0],
            input_commitment_1: input_commitments[1],
            nullifier_0: proof_nullifiers[0],
            nullifier_1: proof_nullifiers[1],
            change_output_commitment: proof_output,
            root: proof_root,
            public_amount,
            asset_tag,
            chain_tag,
        };
        if parsed_binding != request.redemption.unshield_public_inputs
            || parsed_binding
                .digest()
                .map_err(|err| labeled_invariant("invalid_proof", err.to_string()))?
                != request.redemption.unshield_public_inputs_digest
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha V2 redemption intent does not match the canonical unshield-v3 public inputs",
            )
            .into());
        }
        Ok(())
    }

    fn kagemusha_replay_key(domain: &str, value: &Hash) -> Hash {
        let mut preimage = Vec::with_capacity(domain.len() + Hash::LENGTH + 1);
        preimage.extend_from_slice(domain.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(value.as_ref());
        Hash::new(&preimage)
    }

    fn kagemusha_device_registration_key(registration_hash: &Hash) -> Hash {
        kagemusha_replay_key(KAGEMUSHA_DEVICE_REGISTRATION_DOMAIN, registration_hash)
    }

    fn kagemusha_attestation_challenge_key(challenge_hash: &Hash) -> Hash {
        kagemusha_replay_key(
            KAGEMUSHA_ATTESTATION_CHALLENGE_REPLAY_DOMAIN,
            challenge_hash,
        )
    }

    fn kagemusha_attestation_report_key(report_hash: &Hash) -> Hash {
        kagemusha_replay_key(KAGEMUSHA_ATTESTATION_REPORT_REPLAY_DOMAIN, report_hash)
    }

    fn kagemusha_attestation_evidence_key(evidence_hash: &Hash) -> Hash {
        kagemusha_replay_key(KAGEMUSHA_ATTESTATION_EVIDENCE_REPLAY_DOMAIN, evidence_hash)
    }

    fn kagemusha_v2_marker(domain: &str, components: &[&[u8]]) -> Hash {
        let mut preimage = Vec::with_capacity(
            domain.len()
                + components
                    .iter()
                    .map(|component| 8usize.saturating_add(component.len()))
                    .sum::<usize>(),
        );
        preimage.extend_from_slice(domain.as_bytes());
        for component in components {
            preimage.extend_from_slice(
                &u64::try_from(component.len())
                    .unwrap_or(u64::MAX)
                    .to_be_bytes(),
            );
            preimage.extend_from_slice(component);
        }
        Hash::new(&preimage)
    }

    fn kagemusha_v2_device_lineage_key(
        account: &AccountId,
        device_id: &str,
        evidence_sha256: &[u8; 32],
        asset: Option<&AssetDefinitionId>,
    ) -> Hash {
        let account = account.to_string();
        let asset = asset.map(ToString::to_string).unwrap_or_default();
        kagemusha_v2_marker(
            KAGEMUSHA_V2_DEVICE_LINEAGE_DOMAIN,
            &[
                account.as_bytes(),
                device_id.as_bytes(),
                evidence_sha256,
                asset.as_bytes(),
            ],
        )
    }

    fn kagemusha_v2_authorization_markers(
        authorization: &KagemushaRequestAuthorizationV2,
    ) -> [Hash; 4] {
        let authority = authorization.authority.to_string();
        // Top-up anchors are keyed by operation id alone. Keep the replay
        // marker equally global so a second authority cannot claim the same
        // operation id while nonce, payload, and exact-request replay remain
        // scoped to their signing authority.
        let operation = kagemusha_v2_marker(
            KAGEMUSHA_V2_OPERATION_DOMAIN,
            &[&authorization.operation_id],
        );
        let nonce = kagemusha_v2_marker(
            KAGEMUSHA_V2_NONCE_DOMAIN,
            &[authority.as_bytes(), &authorization.nonce],
        );
        let payload = kagemusha_v2_marker(
            KAGEMUSHA_V2_PAYLOAD_DOMAIN,
            &[authority.as_bytes(), &authorization.payload_digest],
        );
        let request = kagemusha_v2_marker(
            KAGEMUSHA_V2_REQUEST_DOMAIN,
            &[
                authority.as_bytes(),
                &authorization.operation_id,
                &authorization.nonce,
                &authorization.payload_digest,
            ],
        );
        [operation, nonce, payload, request]
    }

    enum KagemushaV2ReplayStatus {
        Fresh([Hash; 4]),
        Committed,
    }

    fn kagemusha_v2_replay_status(
        authorization: &KagemushaRequestAuthorizationV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2ReplayStatus, Error> {
        let markers = kagemusha_v2_authorization_markers(authorization);
        let [operation, nonce, payload, request] = &markers;
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(request)
            .is_some()
        {
            return Ok(KagemushaV2ReplayStatus::Committed);
        }
        if [operation, nonce, payload].iter().any(|marker| {
            state_transaction
                .world
                .kagemusha_replay_keys
                .get(marker)
                .is_some()
        }) {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V2 operation id, nonce, or payload digest conflicts with a committed request",
            )
            .into());
        }
        Ok(KagemushaV2ReplayStatus::Fresh(markers))
    }

    fn commit_kagemusha_v2_replay_markers(
        markers: [Hash; 4],
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        for marker in markers {
            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(marker, ());
        }
    }

    fn ensure_registered_kagemusha_v2_device(
        authorization: &KagemushaRequestAuthorizationV2,
        asset: &AssetDefinitionId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let evidence = authorization
            .app_attest_evidence
            .as_deref()
            .ok_or_else(|| {
                labeled_invariant(
                    "device_attestation_required",
                    "Kagemusha V2 authorization requires registered App Attest evidence",
                )
            })?;
        let evidence_sha256: [u8; 32] = Sha256::digest(evidence).into();
        if authorization.app_attest_evidence_sha256 != Some(evidence_sha256) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Kagemusha V2 authorization evidence digest does not match its evidence bytes",
            )
            .into());
        }
        let scoped = kagemusha_v2_device_lineage_key(
            &authorization.authority,
            &authorization.device_id,
            &evidence_sha256,
            Some(asset),
        );
        let global = kagemusha_v2_device_lineage_key(
            &authorization.authority,
            &authorization.device_id,
            &evidence_sha256,
            None,
        );
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(&scoped)
            .is_none()
            && state_transaction
                .world
                .kagemusha_replay_keys
                .get(&global)
                .is_none()
        {
            return Err(labeled_invariant(
                "device_not_registered",
                "Kagemusha V2 authorization device/evidence lineage is not registered",
            )
            .into());
        }
        Ok(())
    }

    fn kagemusha_v2_branch_marker(domain: &str, path: KagemushaRecursiveSpendBranchPathV2) -> Hash {
        kagemusha_v2_marker(
            domain,
            &[&path.lineage_root, &[path.depth], &path.path_bits],
        )
    }

    fn kagemusha_v2_branch_claim_prefix(
        claim: &KagemushaRecursiveSpendBranchClaimV2,
        depth: u8,
    ) -> Result<KagemushaRecursiveSpendBranchClaimV2, Error> {
        claim
            .prefix(depth)
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()).into())
    }

    fn kagemusha_v2_branch_claim_marker(
        domain: &str,
        claim: &KagemushaRecursiveSpendBranchClaimV2,
    ) -> Hash {
        kagemusha_v2_marker(
            domain,
            &[
                &claim.path.lineage_root,
                &[claim.path.depth],
                &claim.path.path_bits,
                &claim.transition_tags,
            ],
        )
    }

    fn kagemusha_v2_transition_choice_marker(
        prefix: KagemushaRecursiveSpendBranchPathV2,
        transition_tag: [u8; 24],
    ) -> Hash {
        kagemusha_v2_marker(
            KAGEMUSHA_V2_TRANSITION_CHOICE_DOMAIN,
            &[
                &prefix.lineage_root,
                &[prefix.depth],
                &prefix.path_bits,
                &transition_tag,
            ],
        )
    }

    fn validate_kagemusha_v2_branch_claim_batch(
        claims: &[KagemushaRecursiveSpendBranchClaimV2],
    ) -> Result<(), Error> {
        if claims.is_empty()
            || claims.len()
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2
        {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 branch set must contain one or two conflict claims",
            )
            .into());
        }
        for (index, claim) in claims.iter().enumerate() {
            claim
                .validate()
                .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
            for previous in &claims[..index] {
                if previous.path.conflicts_with(claim.path) {
                    return Err(labeled_invariant(
                        "branch_conflict",
                        "Kagemusha V2 branch set contains duplicate or overlapping ancestor and descendant claims",
                    )
                    .into());
                }
                if previous.path.lineage_root != claim.path.lineage_root {
                    continue;
                }
                let shared_depth = previous.path.depth.min(claim.path.depth);
                for parent_depth in 0..shared_depth {
                    let previous_prefix = previous
                        .path
                        .prefix(parent_depth)
                        .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                    let claim_prefix = claim
                        .path
                        .prefix(parent_depth)
                        .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                    if previous_prefix == claim_prefix
                        && previous.transition_tag_at(parent_depth)
                            != claim.transition_tag_at(parent_depth)
                    {
                        return Err(labeled_invariant(
                            "branch_conflict",
                            "Kagemusha V2 claims select different transitions at the same lineage prefix",
                        )
                        .into());
                    }
                }
            }
            if index > 0 && claims[index - 1].path >= claim.path {
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 branch set is not in strict canonical order",
                )
                .into());
            }
        }
        Ok(())
    }

    fn ensure_kagemusha_v2_branch_claim_available(
        claim: &KagemushaRecursiveSpendBranchClaimV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        claim
            .validate()
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        for depth in 0..=claim.path.depth {
            let prefix = claim
                .path
                .prefix(depth)
                .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
            let exact = kagemusha_v2_branch_marker(KAGEMUSHA_V2_BRANCH_EXACT_DOMAIN, prefix);
            if state_transaction
                .world
                .kagemusha_replay_keys
                .get(&exact)
                .is_some()
            {
                if depth < claim.path.depth {
                    let child = kagemusha_v2_branch_claim_prefix(claim, depth + 1)?;
                    let authorized_child = kagemusha_v2_branch_claim_marker(
                        KAGEMUSHA_V2_AUTHORIZED_CHANGE_CHILD_DOMAIN,
                        &child,
                    );
                    if state_transaction
                        .world
                        .kagemusha_replay_keys
                        .get(&authorized_child)
                        .is_some()
                    {
                        continue;
                    }
                }
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 branch equals or descends from an already redeemed branch",
                )
                .into());
            }
        }
        let has_descendant =
            kagemusha_v2_branch_marker(KAGEMUSHA_V2_BRANCH_DESCENDANT_DOMAIN, claim.path);
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(&has_descendant)
            .is_some()
        {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 branch is an ancestor of an already redeemed branch",
            )
            .into());
        }
        Ok(())
    }

    fn stage_kagemusha_v2_transition_choices(
        claims: &[KagemushaRecursiveSpendBranchClaimV2],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Vec<(Hash, Hash)>, Error> {
        let mut staged = Vec::<(Hash, Hash)>::new();
        for claim in claims {
            for parent_depth in 0..claim.path.depth {
                let prefix = claim
                    .path
                    .prefix(parent_depth)
                    .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                let transition_tag = claim.transition_tag_at(parent_depth).ok_or_else(|| {
                    labeled_invariant(
                        "branch_conflict",
                        "Kagemusha V2 branch claim is missing an active transition tag",
                    )
                })?;
                let selected =
                    kagemusha_v2_branch_marker(KAGEMUSHA_V2_TRANSITION_SELECTED_DOMAIN, prefix);
                let choice = kagemusha_v2_transition_choice_marker(prefix, transition_tag);
                let selected_exists = state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(&selected)
                    .is_some();
                let choice_exists = state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(&choice)
                    .is_some();
                match (selected_exists, choice_exists) {
                    (true, true) => {}
                    (true, false) => {
                        return Err(labeled_invariant(
                            "branch_conflict",
                            "Kagemusha V2 lineage prefix was already bound to a different transition choice",
                        )
                        .into());
                    }
                    (false, true) => {
                        return Err(labeled_invariant(
                            "branch_conflict",
                            "Kagemusha V2 transition-choice marker exists without its selection marker",
                        )
                        .into());
                    }
                    (false, false) => {
                        if let Some((_, staged_choice)) = staged
                            .iter()
                            .find(|(staged_selected, _)| *staged_selected == selected)
                        {
                            if *staged_choice != choice {
                                return Err(labeled_invariant(
                                    "branch_conflict",
                                    "Kagemusha V2 claims select different transitions at the same lineage prefix",
                                )
                                .into());
                            }
                        } else {
                            staged.push((selected, choice));
                        }
                    }
                }
            }
        }
        Ok(staged)
    }

    fn validate_kagemusha_v2_change_child_authorization(
        parent: &KagemushaRecursiveSpendBranchClaimV2,
        child: &KagemushaRecursiveSpendBranchClaimV2,
        redemption_binding_digest: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Hash, Error> {
        parent
            .validate()
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        child
            .validate()
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        let expected = parent
            .child(
                iroha_data_model::offline::KagemushaRecursiveSpendBranchV2::Change,
                redemption_binding_digest,
            )
            .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
        if *child != expected {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption change is not the exact transition-bound child",
            )
            .into());
        }
        let marker =
            kagemusha_v2_branch_claim_marker(KAGEMUSHA_V2_AUTHORIZED_CHANGE_CHILD_DOMAIN, child);
        if state_transaction
            .world
            .kagemusha_replay_keys
            .get(&marker)
            .is_some()
        {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption change child is already registered",
            )
            .into());
        }
        Ok(marker)
    }

    #[derive(Debug)]
    struct KagemushaV2BranchCommitPlan {
        markers: Vec<Hash>,
    }

    impl KagemushaV2BranchCommitPlan {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) {
            for marker in self.markers {
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .insert(marker, ());
            }
        }
    }

    /// Validate and stage every marker needed to consume a set of V2 branches.
    ///
    /// Partial redemption may additionally authorize the deterministic change
    /// child of a consumed parent. This function is deliberately read-only:
    /// every path, ledger conflict, and transition choice is checked before a
    /// caller can commit even the first marker.
    fn plan_kagemusha_v2_consumed_branch_set(
        consumed_claims: &[KagemushaRecursiveSpendBranchClaimV2],
        redemption_binding_digest: Option<[u8; 32]>,
        change_children: &[(
            KagemushaRecursiveSpendBranchClaimV2,
            KagemushaRecursiveSpendBranchClaimV2,
        )],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2BranchCommitPlan, Error> {
        // Admission first: the data model enforces one non-zero 24-byte transition tag per
        // active path edge, with no padding, and a maximum of two claims.
        validate_kagemusha_v2_branch_claim_batch(consumed_claims)?;

        if change_children.is_empty() != redemption_binding_digest.is_none() {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption change claims require exactly one transition binding digest",
            )
            .into());
        }
        if !change_children.is_empty() && change_children.len() != consumed_claims.len() {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption must authorize exactly one change child per consumed claim",
            )
            .into());
        }
        let redemption_binding_digest = redemption_binding_digest.unwrap_or([0; 32]);
        if !change_children.is_empty() && redemption_binding_digest == [0; 32] {
            return Err(labeled_invariant(
                "branch_conflict",
                "Kagemusha V2 partial redemption binding digest must be non-zero",
            )
            .into());
        }

        let mut authorization_markers = Vec::with_capacity(change_children.len());
        for (index, (parent, child)) in change_children.iter().enumerate() {
            if consumed_claims.get(index) != Some(parent) {
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 partial redemption change parents do not match the canonical consumed set",
                )
                .into());
            }
            if consumed_claims.contains(child) {
                return Err(labeled_invariant(
                    "branch_conflict",
                    "Kagemusha V2 partial redemption cannot consume its new change child",
                )
                .into());
            }
            authorization_markers.push(validate_kagemusha_v2_change_child_authorization(
                parent,
                child,
                redemption_binding_digest,
                state_transaction,
            )?);
        }

        // Every state lookup and every cross-claim transition choice check is
        // completed before the first set-only marker is written.
        let transition_markers =
            stage_kagemusha_v2_transition_choices(consumed_claims, state_transaction)?;
        for claim in consumed_claims {
            ensure_kagemusha_v2_branch_claim_available(claim, state_transaction)?;
        }

        let mut markers = BTreeSet::new();
        for (selected, choice) in transition_markers {
            markers.insert(selected);
            markers.insert(choice);
        }
        for claim in consumed_claims {
            markers.insert(kagemusha_v2_branch_marker(
                KAGEMUSHA_V2_BRANCH_EXACT_DOMAIN,
                claim.path,
            ));
            for depth in 0..claim.path.depth {
                let prefix = claim
                    .path
                    .prefix(depth)
                    .map_err(|err| labeled_invariant("branch_conflict", err.to_string()))?;
                markers.insert(kagemusha_v2_branch_marker(
                    KAGEMUSHA_V2_BRANCH_DESCENDANT_DOMAIN,
                    prefix,
                ));
            }
        }
        markers.extend(authorization_markers);

        Ok(KagemushaV2BranchCommitPlan {
            markers: markers.into_iter().collect(),
        })
    }

    fn kagemusha_v2_topup_anchor_state_key(operation_id: [u8; 32]) -> Result<Name, Error> {
        format!("kagemusha_v2_topup_anchor_{}", hex::encode(operation_id))
            .parse()
            .map_err(|err| {
                labeled_invariant(
                    "invalid_recursive_topup",
                    format!("failed to derive Kagemusha V2 anchor state key: {err}"),
                )
                .into()
            })
    }

    fn load_kagemusha_v2_topup_anchor(
        operation_id: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaRecursiveSpendTopUpAnchorV2, Error> {
        let key = kagemusha_v2_topup_anchor_state_key(operation_id)?;
        let archive = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .ok_or_else(|| {
                labeled_invariant(
                    "topup_anchor_missing",
                    "Kagemusha V2 bundle has no finalized top-up anchor",
                )
            })?;
        let anchor: KagemushaRecursiveSpendTopUpAnchorV2 = norito::decode_from_bytes(archive)
            .map_err(|err| {
                labeled_invariant(
                    "topup_anchor_invalid",
                    format!("failed to decode persisted Kagemusha V2 top-up anchor: {err}"),
                )
            })?;
        anchor
            .validate_public_binding()
            .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
        if anchor.topup_operation_id != operation_id
            || norito::to_bytes(&anchor)
                .map_err(|err| {
                    labeled_invariant(
                        "topup_anchor_invalid",
                        format!("failed to re-encode persisted Kagemusha V2 top-up anchor: {err}"),
                    )
                })?
                .as_slice()
                != archive.as_slice()
        {
            return Err(labeled_invariant(
                "topup_anchor_invalid",
                "persisted Kagemusha V2 top-up anchor is non-canonical or keyed incorrectly",
            )
            .into());
        }
        Ok(anchor)
    }

    fn persist_kagemusha_v2_topup_anchor(
        anchor: &KagemushaRecursiveSpendTopUpAnchorV2,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        anchor
            .validate_public_binding()
            .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
        let key = kagemusha_v2_topup_anchor_state_key(anchor.topup_operation_id)?;
        let existing = state_transaction.world.smart_contract_state.get(&key);
        if existing.is_some() {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V2 top-up operation already has a finalized anchor",
            )
            .into());
        }
        crate::sumeragi::witness::record_read_kagemusha_v2_topup_anchor(
            anchor.topup_operation_id,
            None,
        );
        let archive = norito::to_bytes(anchor).map_err(|err| {
            labeled_invariant(
                "topup_anchor_invalid",
                format!("failed to encode Kagemusha V2 top-up anchor: {err}"),
            )
        })?;
        crate::sumeragi::witness::record_write_kagemusha_v2_topup_anchor(
            anchor.topup_operation_id,
            &anchor.anchor_digest,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(key, archive);
        Ok(())
    }

    fn kagemusha_v2_redemption_receipt_state_key(operation_id: [u8; 32]) -> Result<Name, Error> {
        format!("kagemusha_v2_redemption_{}", hex::encode(operation_id))
            .parse()
            .map_err(|err| {
                labeled_invariant(
                    "invalid_recursive_redeem",
                    format!("failed to derive Kagemusha V2 redemption receipt key: {err}"),
                )
                .into()
            })
    }

    fn ensure_kagemusha_v2_redemption_receipt_matches(
        operation_id: [u8; 32],
        payload_digest: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let key = kagemusha_v2_redemption_receipt_state_key(operation_id)?;
        let receipt = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .ok_or_else(|| {
                labeled_invariant(
                    "authorization_replay",
                    "Kagemusha V2 redemption replay marker has no committed receipt",
                )
            })?;
        if receipt.as_slice() != payload_digest.as_slice() {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V2 redemption receipt does not match the retried request",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_v2_redemption_receipt_absent(
        operation_id: [u8; 32],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Name, Error> {
        let key = kagemusha_v2_redemption_receipt_state_key(operation_id)?;
        if state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .is_some()
        {
            return Err(labeled_invariant(
                "authorization_replay",
                "Kagemusha V2 redemption receipt exists without its complete replay-marker set",
            )
            .into());
        }
        Ok(key)
    }

    struct KagemushaV2ResolvedTopUpProvenance {
        source_asset: AssetId,
    }

    fn validate_kagemusha_v2_finalized_topup_anchors(
        anchor_refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
        current_note_atomic_units: u128,
        requested_height: u64,
        zk_state: &crate::state::ZkAssetState,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2ResolvedTopUpProvenance, Error> {
        let mut canonical_source_asset = None;
        let mut seen_operations = BTreeSet::new();
        let mut anchored_total = 0_u128;
        for supplied_ref in anchor_refs {
            supplied_ref
                .validate()
                .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
            if !seen_operations.insert(supplied_ref.topup_operation_id) {
                return Err(labeled_invariant(
                    "topup_anchor_invalid",
                    "Kagemusha V2 redemption repeats a top-up operation anchor",
                )
                .into());
            }
            let persisted =
                load_kagemusha_v2_topup_anchor(supplied_ref.topup_operation_id, state_transaction)?;
            if persisted.anchor_digest != supplied_ref.anchor_digest
                || persisted
                    .compact_ref()
                    .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?
                    != *supplied_ref
            {
                return Err(labeled_invariant(
                    "topup_anchor_mismatch",
                    "Kagemusha V2 redemption anchor differs from the finalized chain receipt",
                )
                .into());
            }
            if persisted.finalized_height > requested_height
                || persisted.finalized_height > state_transaction.block_height()
            {
                return Err(labeled_invariant(
                    "topup_anchor_invalid",
                    "Kagemusha V2 redemption predates one of its finalized top-up anchors",
                )
                .into());
            }
            if !zk_state
                .commitments
                .contains(&persisted.current_note.note_commitment)
            {
                return Err(labeled_invariant(
                    "topup_anchor_mismatch",
                    "Kagemusha V2 finalized top-up evidence is inconsistent with confidential ledger state",
                )
                .into());
            }
            anchored_total = anchored_total
                .checked_add(persisted.amount.atomic_units)
                .ok_or_else(|| {
                    labeled_invariant(
                        "amount_mismatch",
                        "Kagemusha V2 finalized top-up amount total overflows u128",
                    )
                })?;
            let canonical = canonical_kagemusha_asset_id(state_transaction, &persisted.asset)?;
            match &canonical_source_asset {
                None => canonical_source_asset = Some(canonical),
                Some(source) if source.scope() == canonical.scope() => {}
                Some(_) => {
                    return Err(labeled_invariant(
                        "asset_mismatch",
                        "Kagemusha V2 cannot join top-up anchors from different asset-balance scopes",
                    )
                    .into());
                }
            }
        }
        if anchored_total < current_note_atomic_units {
            return Err(labeled_invariant(
                "amount_mismatch",
                "Kagemusha V2 spendable note exceeds its finalized top-up provenance",
            )
            .into());
        }
        let source_asset = canonical_source_asset.ok_or_else(|| {
            Error::from(labeled_invariant(
                "topup_anchor_missing",
                "Kagemusha V2 redemption has no finalized top-up provenance",
            ))
        })?;
        Ok(KagemushaV2ResolvedTopUpProvenance { source_asset })
    }

    #[derive(Debug)]
    struct KagemushaV2EscrowCreditPlan {
        escrow_asset: AssetId,
        recipient_asset: AssetId,
        amount: Numeric,
    }

    impl KagemushaV2EscrowCreditPlan {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) -> Result<(), Error> {
            withdraw_numeric_asset_exact(state_transaction, &self.escrow_asset, &self.amount)?;
            if let Err(err) =
                deposit_numeric_asset_exact(state_transaction, &self.recipient_asset, &self.amount)
            {
                deposit_numeric_asset_exact(state_transaction, &self.escrow_asset, &self.amount)
                    .expect("prevalidated Kagemusha V2 escrow refund must succeed");
                return Err(err);
            }
            Ok(())
        }
    }

    fn plan_kagemusha_v2_escrow_credit(
        source_asset: &AssetId,
        recipient: &AccountId,
        amount: &Numeric,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2EscrowCreditPlan, Error> {
        let definition_id = source_asset.definition().clone();
        let definition = state_transaction.world.asset_definition(&definition_id)?;
        let escrow_account =
            if crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
                definition.metadata(),
            )? {
                crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
                    state_transaction.chain_id(),
                    &definition_id,
                )
            } else {
                state_transaction
                .settlement
                .offline
                .escrow_accounts
                .get(&definition_id)
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "escrow_missing",
                        format!(
                            "offline escrow account not configured for asset definition `{definition_id}`"
                        ),
                    )
                })?
            };
        state_transaction.world.account(recipient)?;
        state_transaction.world.account(&escrow_account)?;
        ensure_distinct_offline_escrow_account(
            &escrow_account,
            recipient,
            "recipient",
            &definition_id,
        )?;

        let recipient_asset = AssetId::with_scope(
            definition_id,
            recipient.clone(),
            source_asset.scope().clone(),
        );
        let escrow_asset = kagemusha_escrow_asset_id(source_asset, escrow_account);
        let escrow_balance = state_transaction
            .world
            .assets
            .get(&escrow_asset)
            .map(|asset| asset.as_ref().as_numeric().clone())
            .ok_or_else(|| FindError::Asset(escrow_asset.clone().into()))?;
        escrow_balance
            .checked_sub(amount.clone())
            .filter(|remaining| !remaining.mantissa().is_negative())
            .ok_or(MathError::NotEnoughQuantity)?;
        state_transaction
            .world
            .assets
            .get(&recipient_asset)
            .map(|asset| asset.as_ref().as_numeric().clone())
            .unwrap_or_else(Numeric::zero)
            .checked_add(amount.clone())
            .ok_or(MathError::Overflow)?;

        Ok(KagemushaV2EscrowCreditPlan {
            escrow_asset,
            recipient_asset,
            amount: amount.clone(),
        })
    }

    fn is_zero_hash(hash: &Hash) -> bool {
        hash.as_ref().iter().all(|byte| *byte == 0)
    }

    fn world_has_offline_permission(
        world: &impl WorldReadOnly,
        authority: &AccountId,
        required: &Permission,
    ) -> bool {
        // These first-release capabilities carry no scope. Match the complete
        // canonical permission so a same-name token with attacker-controlled
        // payload cannot acquire administrative authority.
        if world
            .account_permissions()
            .get(authority)
            .is_some_and(|permissions| permissions.contains(required))
        {
            return true;
        }

        world.account_roles_iter(authority).any(|role_id| {
            world
                .roles()
                .get(role_id)
                .is_some_and(|role| role.permissions().any(|permission| permission == required))
        })
    }

    /// Canonical unit-valued permission required to manage offline escrow.
    pub fn offline_escrow_manager_permission() -> Permission {
        Permission::new(
            CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
            iroha_primitives::json::Json::new(()),
        )
    }

    /// Return whether an account holds the exact offline escrow permission,
    /// either directly or through an assigned role.
    pub fn world_has_offline_escrow_manager_permission(
        world: &impl WorldReadOnly,
        authority: &AccountId,
    ) -> bool {
        let required = offline_escrow_manager_permission();
        world_has_offline_permission(world, authority, &required)
    }

    fn is_offline_escrow_manager(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> bool {
        world_has_offline_escrow_manager_permission(&state_transaction.world, authority)
    }

    fn ensure_can_submit_kagemusha_for_account(
        account: &AccountId,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if account == authority || is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only the Kagemusha account or an offline escrow manager may submit this request",
            )
            .into())
        }
    }

    fn ensure_can_submit_kagemusha_topup(
        asset: &AssetId,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if asset.account() == authority || is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only the top-up payer or an offline escrow manager may submit recursive Kagemusha top-ups",
            )
            .into())
        }
    }

    fn validate_offline_attestation_recent_block(
        registration: &OfflineDeviceAttestationRegistration,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if registration.recent_block_height == 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation must bind a committed block height",
            )
            .into());
        }
        let committed_height = state_transaction.block_hashes().len() as u64;
        if registration.recent_block_height > committed_height {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation references a block height that is not committed",
            )
            .into());
        }
        if committed_height.saturating_sub(registration.recent_block_height)
            > KAGEMUSHA_ATTESTATION_RECENT_BLOCK_WINDOW
        {
            return Err(labeled_invariant(
                "stale_attestation",
                "offline device attestation challenge is outside the recent block window",
            )
            .into());
        }
        let block_hash = state_transaction
            .block_hashes()
            .get(registration.recent_block_height.saturating_sub(1) as usize)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "offline device attestation references a missing committed block",
                )
            })?;
        if block_hash.as_ref() != registration.recent_block_hash.as_ref() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation recent block hash does not match ledger state",
            )
            .into());
        }
        Ok(())
    }

    fn validate_offline_attestation_platform_profile(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        validate_p256_uncompressed_public_key(&registration.assertion_public_key)?;

        match registration.platform.as_str() {
            OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                if registration.assertion_scheme != OFFLINE_ATTESTATION_IOS_ASSERTION_SCHEME
                    || registration.assertion_key_algorithm
                        != OFFLINE_ATTESTATION_IOS_ASSERTION_ALGORITHM
                    || registration.assertion_usage_count_limit.is_some()
                {
                    return Err(labeled_invariant(
                        "invalid_attestation",
                        "iOS App Attest registrations must use the canonical App Attest assertion profile",
                    )
                    .into());
                }
            }
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                if registration.assertion_scheme != OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME
                    || registration.assertion_key_algorithm
                        != OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM
                    || registration.assertion_usage_count_limit != Some(1)
                {
                    return Err(labeled_invariant(
                        "invalid_attestation",
                        "Android KeyMint registrations must use the canonical one-use P-256 assertion profile",
                    )
                    .into());
                }
            }
            _ => {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "offline device attestation platform is unsupported",
                )
                .into());
            }
        }

        Ok(())
    }

    fn validate_optional_attestation_metadata_string(
        value: Option<&str>,
        field: &'static str,
    ) -> Result<(), Error> {
        let Some(value) = value else {
            return Ok(());
        };
        if value.trim().is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                format!("offline device attestation {field} must not be empty when present"),
            )
            .into());
        }
        if value.trim() != value {
            return Err(labeled_invariant(
                "invalid_attestation",
                format!(
                    "offline device attestation {field} must not contain surrounding whitespace"
                ),
            )
            .into());
        }
        Ok(())
    }

    fn validate_offline_attestation_optional_metadata(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        for (field, value) in [
            ("ios_team_id", registration.ios_team_id.as_deref()),
            ("ios_bundle_id", registration.ios_bundle_id.as_deref()),
            ("ios_environment", registration.ios_environment.as_deref()),
            (
                "android_package_name",
                registration.android_package_name.as_deref(),
            ),
        ] {
            validate_optional_attestation_metadata_string(value, field)?;
        }
        Ok(())
    }

    fn p256_public_key_has_zero_coordinate_material(public_key: &[u8]) -> bool {
        public_key.len() == OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN
            && public_key.first() == Some(&0x04)
            && public_key[1..].iter().all(|byte| *byte == 0)
    }

    fn validate_p256_uncompressed_public_key(public_key: &[u8]) -> Result<(), Error> {
        if public_key.len() != OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN
            || public_key.first() != Some(&0x04)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation assertion public key must be an uncompressed P-256 SEC1 key",
            )
            .into());
        }
        if p256_public_key_has_zero_coordinate_material(public_key) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation assertion public key must be a valid uncompressed P-256 SEC1 point",
            )
            .into());
        }
        P256PublicKey::from_sec1_bytes(public_key).map(|_| ()).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "offline device attestation assertion public key must be a valid uncompressed P-256 SEC1 point",
            )
            .into()
        })
    }

    fn cbor_text_key_value<'a>(
        map: &'a [(ciborium::value::Value, ciborium::value::Value)],
        key: &str,
    ) -> Result<Option<&'a ciborium::value::Value>, Error> {
        let mut matches = map.iter().filter(
            |(candidate, _)| matches!(candidate, ciborium::value::Value::Text(text) if text == key),
        );
        let first = matches.next().map(|(_, value)| value);
        if matches.next().is_some() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation CBOR map contains a duplicate text key",
            )
            .into());
        }
        Ok(first)
    }

    fn cbor_integer_key_value<'a>(
        map: &'a [(ciborium::value::Value, ciborium::value::Value)],
        key: i128,
    ) -> Result<Option<&'a ciborium::value::Value>, Error> {
        let mut matches = map.iter().filter(|(candidate, _)| {
            matches!(candidate, ciborium::value::Value::Integer(value) if i128::from(value.clone()) == key)
        });
        let first = matches.next().map(|(_, value)| value);
        if matches.next().is_some() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation CBOR map contains a duplicate integer key",
            )
            .into());
        }
        Ok(first)
    }

    fn cbor_text_value(
        map: &[(ciborium::value::Value, ciborium::value::Value)],
        key: &str,
    ) -> Result<Option<String>, Error> {
        Ok(match cbor_text_key_value(map, key)? {
            Some(ciborium::value::Value::Text(text)) => Some(text.clone()),
            _ => None,
        })
    }

    fn cbor_bytes_value(
        map: &[(ciborium::value::Value, ciborium::value::Value)],
        key: &str,
    ) -> Result<Option<Vec<u8>>, Error> {
        Ok(match cbor_text_key_value(map, key)? {
            Some(ciborium::value::Value::Bytes(bytes)) => Some(bytes.clone()),
            _ => None,
        })
    }

    fn cbor_map_value<'a>(
        map: &'a [(ciborium::value::Value, ciborium::value::Value)],
        key: &str,
    ) -> Result<Option<&'a [(ciborium::value::Value, ciborium::value::Value)]>, Error> {
        Ok(match cbor_text_key_value(map, key)? {
            Some(ciborium::value::Value::Map(map)) => Some(map.as_slice()),
            _ => None,
        })
    }

    fn cbor_array_value<'a>(
        map: &'a [(ciborium::value::Value, ciborium::value::Value)],
        key: &str,
    ) -> Result<Option<&'a [ciborium::value::Value]>, Error> {
        Ok(match cbor_text_key_value(map, key)? {
            Some(ciborium::value::Value::Array(values)) => Some(values.as_slice()),
            _ => None,
        })
    }

    fn cbor_int_value(
        map: &[(ciborium::value::Value, ciborium::value::Value)],
        key: i128,
    ) -> Result<Option<i128>, Error> {
        Ok(match cbor_integer_key_value(map, key)? {
            Some(ciborium::value::Value::Integer(value)) => Some(i128::from(value.clone())),
            _ => None,
        })
    }

    fn cbor_bytes_value_i(
        map: &[(ciborium::value::Value, ciborium::value::Value)],
        key: i128,
    ) -> Result<Option<Vec<u8>>, Error> {
        Ok(match cbor_integer_key_value(map, key)? {
            Some(ciborium::value::Value::Bytes(bytes)) => Some(bytes.clone()),
            _ => None,
        })
    }

    fn decode_cbor_value_exact(
        input: &[u8],
        parse_message: &str,
        trailing_message: &str,
    ) -> Result<ciborium::value::Value, Error> {
        let mut cursor = Cursor::new(input);
        let value: ciborium::value::Value = ciborium::de::from_reader(&mut cursor)
            .map_err(|_| labeled_invariant("invalid_attestation", parse_message.to_owned()))?;
        if cursor.position() != input.len() as u64 {
            return Err(labeled_invariant("invalid_attestation", trailing_message).into());
        }
        Ok(value)
    }

    fn parse_ios_app_attest_report(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<IosAppAttestReport, Error> {
        let value = decode_cbor_value_exact(
            &registration.attestation_report,
            "iOS App Attest report must be a CBOR attestation object",
            "iOS App Attest report has trailing CBOR bytes",
        )?;
        let ciborium::value::Value::Map(map) = value else {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest report must be a CBOR map",
            )
            .into());
        };
        if cbor_text_value(&map, "fmt")?.as_deref() != Some("apple-appattest") {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest report format must be apple-appattest",
            )
            .into());
        }
        let auth_data = cbor_bytes_value(&map, "authData")?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest report is missing authData",
            )
        })?;
        let att_stmt = cbor_map_value(&map, "attStmt")?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest report is missing attStmt",
            )
        })?;
        let x5c = cbor_array_value(att_stmt, "x5c")?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest report is missing certificate chain",
            )
        })?;
        if x5c.len() < 2 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest report must include a certificate chain",
            )
            .into());
        }
        let mut certificates = Vec::with_capacity(x5c.len());
        for value in x5c {
            let ciborium::value::Value::Bytes(certificate) = value else {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest certificate chain entries must be bytes",
                )
                .into());
            };
            if certificate.is_empty() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest certificate chain entries must be non-empty",
                )
                .into());
            }
            certificates.push(certificate.clone());
        }
        Ok(IosAppAttestReport {
            auth_data,
            certificates,
        })
    }

    fn parse_ios_app_attest_auth_data(auth_data: &[u8]) -> Result<IosAppAttestAuthData, Error> {
        if auth_data.len() < OFFLINE_ATTESTATION_APP_ATTEST_AUTH_DATA_MIN_LEN {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest authData is too short",
            )
            .into());
        }
        if auth_data[32] & OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA == 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest authData is missing attested credential data",
            )
            .into());
        }
        let rp_id_hash = auth_data[0..32]
            .try_into()
            .expect("authData length already checked");
        let sign_count = u32::from_be_bytes(
            auth_data[33..37]
                .try_into()
                .expect("authData length already checked"),
        );
        let aaguid = auth_data[37..53]
            .try_into()
            .expect("authData length already checked");
        let credential_id_len = u16::from_be_bytes(
            auth_data[53..55]
                .try_into()
                .expect("authData length already checked"),
        ) as usize;
        let credential_id_start = 55usize;
        let credential_id_end = credential_id_start.saturating_add(credential_id_len);
        if credential_id_end > auth_data.len() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential id exceeds authData bounds",
            )
            .into());
        }
        let cose_key = auth_data[credential_id_end..].to_vec();
        if cose_key.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key is missing",
            )
            .into());
        }
        Ok(IosAppAttestAuthData {
            rp_id_hash,
            sign_count,
            aaguid,
            credential_id: auth_data[credential_id_start..credential_id_end].to_vec(),
            cose_key,
        })
    }

    fn ios_attestation_metadata(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(String, String, String), Error> {
        let team_id = registration
            .ios_team_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_uppercase)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest registration is missing the Apple Team ID",
                )
            })?;
        let bundle_id = registration
            .ios_bundle_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest registration is missing the bundle identifier",
                )
            })?;
        let environment = registration
            .ios_environment
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "iOS App Attest registration is missing the environment",
                )
            })?;
        if environment != OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION
            && environment != OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest environment must be production or development",
            )
            .into());
        }
        Ok((team_id, bundle_id, environment))
    }

    fn ios_app_policy_matches(
        policy: &OfflineIosAppAttestationPolicy,
        team_id: &str,
        bundle_id: &str,
        environment: &str,
    ) -> bool {
        policy.team_id.eq_ignore_ascii_case(team_id)
            && policy.bundle_id == bundle_id
            && policy.environment.eq_ignore_ascii_case(environment)
    }

    fn ensure_ios_app_allowed_by_policy(
        policy: &OfflineDeviceAttestationPolicy,
        team_id: &str,
        bundle_id: &str,
        environment: &str,
    ) -> Result<(), Error> {
        if policy.ios_apps.is_empty() && !policy.require_ios_app_policy {
            return Ok(());
        }
        if policy
            .ios_apps
            .iter()
            .any(|app| ios_app_policy_matches(app, team_id, bundle_id, environment))
        {
            return Ok(());
        }
        Err(labeled_invariant(
            "invalid_attestation_policy",
            "iOS App Attest app identity is not allowed by Offline device attestation policy",
        )
        .into())
    }

    fn android_attestation_metadata(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(String, [u8; 32]), Error> {
        let package_name = registration
            .android_package_name
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint registration is missing the package name",
                )
            })?;
        let signing_digest = registration
            .android_signing_certificate_sha256
            .as_deref()
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint registration is missing the signing certificate digest",
                )
            })
            .and_then(|digest| {
                digest.try_into().map_err(|_| {
                    labeled_invariant(
                        "invalid_attestation",
                        "Android KeyMint signing certificate digest must be 32 bytes",
                    )
                    .into()
                })
            })?;
        if signing_digest == [0u8; 32] {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint signing certificate digest must be non-zero",
            )
            .into());
        }
        Ok((package_name, signing_digest))
    }

    fn android_app_policy_matches(
        policy: &OfflineAndroidAppAttestationPolicy,
        package_name: &str,
        signing_digest: &[u8; 32],
    ) -> bool {
        policy.package_name == package_name
            && policy
                .signing_certificate_sha256
                .iter()
                .any(|candidate| candidate.as_slice() == signing_digest)
    }

    fn ensure_android_app_allowed_by_policy(
        policy: &OfflineDeviceAttestationPolicy,
        package_name: &str,
        signing_digest: &[u8; 32],
    ) -> Result<(), Error> {
        if policy.android_apps.is_empty() && !policy.require_android_app_policy {
            return Ok(());
        }
        if policy
            .android_apps
            .iter()
            .any(|app| android_app_policy_matches(app, package_name, signing_digest))
        {
            return Ok(());
        }
        Err(labeled_invariant(
            "invalid_attestation_policy",
            "Android KeyMint app identity is not allowed by Offline device attestation policy",
        )
        .into())
    }

    fn validate_android_key_id(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        let expected_key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
        if registration.key_id != expected_key_id {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint key_id must be lowercase hex SHA-256 of the assertion public key",
            )
            .into());
        }
        Ok(())
    }

    fn extract_der_octet_string(input: &[u8], depth: usize) -> Result<Vec<u8>, Error> {
        if depth > 4 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension is too deeply nested",
            )
            .into());
        }
        let mut reader = DerReader::new(input);
        let (tag, value) = reader.read_tlv()?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension has trailing DER bytes",
            )
            .into());
        }
        match tag {
            0x04 => Ok(value.to_vec()),
            0x30 | 0xA1 => extract_der_octet_string(value, depth + 1),
            _ => Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension must contain an OCTET STRING",
            )
            .into()),
        }
    }

    fn validate_app_attest_cose_p256_key(
        cose_key_bytes: &[u8],
        expected_public_key: &[u8],
    ) -> Result<(), Error> {
        let value = decode_cbor_value_exact(
            cose_key_bytes,
            "iOS App Attest credential public key must be CBOR",
            "iOS App Attest credential public key has trailing CBOR bytes",
        )?;
        let ciborium::value::Value::Map(map) = value else {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key must be a COSE map",
            )
            .into());
        };
        if cbor_int_value(&map, 1)? != Some(2)
            || cbor_int_value(&map, -1)? != Some(1)
            || cbor_int_value(&map, 3)?.is_some_and(|alg| alg != -7)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key must be ES256 P-256",
            )
            .into());
        }
        let x = cbor_bytes_value_i(&map, -2)?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key is missing an x coordinate",
            )
        })?;
        let y = cbor_bytes_value_i(&map, -3)?.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key is missing a y coordinate",
            )
        })?;
        if x.len() != 32 || y.len() != 32 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key coordinates must be 32 bytes",
            )
            .into());
        }
        let mut public_key =
            Vec::with_capacity(OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN);
        public_key.push(0x04);
        public_key.extend_from_slice(&x);
        public_key.extend_from_slice(&y);
        if public_key != expected_public_key {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential public key does not match the registered assertion key",
            )
            .into());
        }
        Ok(())
    }

    fn validate_ios_app_attest_report(
        registration: &OfflineDeviceAttestationRegistration,
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        let report = parse_ios_app_attest_report(registration)?;
        let auth_data = parse_ios_app_attest_auth_data(&report.auth_data)?;
        if auth_data.sign_count != 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest attestation counter must start at zero",
            )
            .into());
        }

        let (team_id, bundle_id, environment) = ios_attestation_metadata(registration)?;
        ensure_ios_app_allowed_by_policy(policy, &team_id, &bundle_id, &environment)?;
        let expected_aaguid = if environment == OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT {
            OFFLINE_ATTESTATION_IOS_AAGUID_DEVELOPMENT
        } else {
            OFFLINE_ATTESTATION_IOS_AAGUID_PRODUCTION
        };
        if auth_data.aaguid.as_slice() != expected_aaguid {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest AAGUID does not match the registered environment",
            )
            .into());
        }

        let rp_id = format!("{team_id}.{bundle_id}");
        if auth_data.rp_id_hash != sha256_bytes(rp_id.as_bytes()) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest app identity hash does not match Team ID and bundle ID",
            )
            .into());
        }

        let expected_key_id = decode_canonical_ios_app_attest_key_id(&registration.key_id)?;
        if auth_data.credential_id != expected_key_id {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest credential id does not match key_id",
            )
            .into());
        }
        validate_app_attest_cose_p256_key(&auth_data.cose_key, &registration.assertion_public_key)?;

        let trusted_roots =
            trusted_root_der_for_platform(policy, &registration.platform, block_unix_timestamp_ms)?;
        let revoked_certificate_sha256 = policy_revoked_certificate_hashes(policy)?;
        let evaluation_time = x509_evaluation_time(block_unix_timestamp_ms)?;
        validate_attestation_certificate_chain(
            &report.certificates,
            &trusted_roots,
            &revoked_certificate_sha256,
            evaluation_time,
        )?;
        let leaf = parse_x509_certificate_der(&report.certificates[0])?;
        let leaf_public_key = x509_subject_public_key_bytes(&leaf);
        if leaf_public_key != registration.assertion_public_key {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest certificate public key does not match the registered assertion key",
            )
            .into());
        }
        if sha256_bytes(&leaf_public_key).as_slice() != expected_key_id.as_slice() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest certificate public key hash does not match key_id",
            )
            .into());
        }
        let nonce_extension = x509_unique_extension_value(
            &leaf,
            OFFLINE_ATTESTATION_APP_ATTEST_NONCE_OID,
            "iOS App Attest certificate contains duplicate nonce extensions",
        )?
        .ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "iOS App Attest certificate is missing the nonce extension",
            )
        })?;
        let nonce = extract_der_octet_string(&nonce_extension, 0)?;
        let expected_nonce = sha256_concat(&report.auth_data, registration.challenge_hash.as_ref());
        if nonce != expected_nonce {
            return Err(labeled_invariant(
                "invalid_attestation",
                "iOS App Attest nonce extension does not bind the attestation challenge",
            )
            .into());
        }
        Ok(())
    }

    fn decode_canonical_ios_app_attest_key_id(
        key_id: &str,
    ) -> Result<Vec<u8>, InstructionExecutionError> {
        let decoded = BASE64_STANDARD
            .decode(key_id.as_bytes())
            .map_err(|_| invalid_ios_app_attest_key_id())?;
        if decoded.is_empty() || BASE64_STANDARD.encode(&decoded) != key_id {
            return Err(invalid_ios_app_attest_key_id());
        }
        Ok(decoded)
    }

    fn invalid_ios_app_attest_key_id() -> InstructionExecutionError {
        labeled_invariant(
            "invalid_attestation",
            "iOS App Attest key_id must be canonical standard base64 credential bytes",
        )
    }

    fn parse_android_keymint_report(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<AndroidKeyMintReport, Error> {
        let value = decode_cbor_value_exact(
            &registration.attestation_report,
            "Android KeyMint report must be a CBOR certificate array",
            "Android KeyMint report has trailing CBOR bytes",
        )?;
        let ciborium::value::Value::Array(certificates) = value else {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint report must be a CBOR certificate array",
            )
            .into());
        };
        if certificates.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint report must include certificate bytes",
            )
            .into());
        }
        let mut certificate_der = Vec::with_capacity(certificates.len());
        for value in certificates {
            let ciborium::value::Value::Bytes(certificate) = value else {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint certificate entries must be bytes",
                )
                .into());
            };
            if certificate.is_empty() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint certificate entries must be non-empty",
                )
                .into());
            }
            certificate_der.push(certificate);
        }
        Ok(AndroidKeyMintReport {
            certificates: certificate_der,
        })
    }

    fn der_single_integer(input: &[u8]) -> Result<i64, Error> {
        let mut reader = DerReader::new(input);
        reader.read_integer().and_then(|value| {
            if reader.has_remaining() {
                Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint authorization value has trailing bytes",
                )
                .into())
            } else {
                Ok(value)
            }
        })
    }

    fn der_single_octet_string(input: &[u8]) -> Result<Vec<u8>, Error> {
        let mut reader = DerReader::new(input);
        let value = reader.read_octet_string()?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization OCTET STRING has trailing bytes",
            )
            .into());
        }
        Ok(value)
    }

    fn validate_der_set_element_order<'a>(
        previous: &mut Option<&'a [u8]>,
        current: &'a [u8],
        message: &str,
    ) -> Result<(), Error> {
        if previous.is_some_and(|previous| previous > current) {
            return Err(labeled_invariant("invalid_attestation", message.to_owned()).into());
        }
        *previous = Some(current);
        Ok(())
    }

    fn parse_android_attestation_application_id(
        input: &[u8],
    ) -> Result<AndroidAttestationApplicationId, Error> {
        let mut reader = DerReader::sequence(input)?;
        let package_set = reader.read_expected(0x31)?;
        let signature_set = reader.read_expected(0x31)?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation application id has trailing bytes",
            )
            .into());
        }

        let mut packages = Vec::new();
        let mut seen_packages = HashSet::new();
        let mut package_reader = DerReader::new(package_set);
        let mut previous_package_der = None;
        while package_reader.has_remaining() {
            let (tag, package_der, raw_package_der) = package_reader.read_tlv_full_with_raw()?;
            if tag.first_byte != 0x30 {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER has an unexpected DER tag",
                )
                .into());
            }
            validate_der_set_element_order(
                &mut previous_package_der,
                raw_package_der,
                "Android KeyMint attestation package SET elements are not DER sorted",
            )?;
            let mut info_reader = DerReader::new(package_der);
            let package_name_bytes = info_reader.read_octet_string()?;
            let _version = info_reader.read_integer()?;
            if info_reader.has_remaining() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint attestation package info has trailing bytes",
                )
                .into());
            }
            let package_name = String::from_utf8(package_name_bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint attestation package name must be UTF-8",
                )
            })?;
            if package_name.trim().is_empty() {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint attestation package name must be non-empty",
                )
                .into());
            }
            if !seen_packages.insert(package_name.clone()) {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint attestation application id duplicates a package name",
                )
                .into());
            }
            packages.push(AndroidAttestationPackageInfo { package_name });
        }

        let mut signature_digests = Vec::new();
        let mut seen_signature_digests = HashSet::new();
        let mut signature_reader = DerReader::new(signature_set);
        let mut previous_signature_der = None;
        while signature_reader.has_remaining() {
            let (tag, digest, raw_signature_der) = signature_reader.read_tlv_full_with_raw()?;
            if tag.first_byte != 0x04 {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "attestation extension DER has an unexpected DER tag",
                )
                .into());
            }
            validate_der_set_element_order(
                &mut previous_signature_der,
                raw_signature_der,
                "Android KeyMint attestation signing-digest SET elements are not DER sorted",
            )?;
            if digest.len() != 32 {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint attestation signing digest must be 32 bytes",
                )
                .into());
            }
            let mut digest_array = [0u8; 32];
            digest_array.copy_from_slice(&digest);
            if !seen_signature_digests.insert(digest_array) {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint attestation application id duplicates a signing digest",
                )
                .into());
            }
            signature_digests.push(digest.to_vec());
        }

        if packages.is_empty() || signature_digests.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation application id must include packages and signing digests",
            )
            .into());
        }
        Ok(AndroidAttestationApplicationId {
            packages,
            signature_digests,
        })
    }

    fn parse_android_authorization_list(
        input: &[u8],
    ) -> Result<(Option<i64>, bool, Option<AndroidAttestationApplicationId>), Error> {
        let mut reader = DerReader::new(input);
        let mut usage_count_limit = None;
        let mut all_applications = false;
        let mut application_id = None;
        while reader.has_remaining() {
            let (tag, value) = reader.read_tlv_full()?;
            if tag.class_bits != 0x80 || !tag.constructed {
                return Err(labeled_invariant(
                    "invalid_attestation",
                    "Android KeyMint authorization list contains an invalid tag",
                )
                .into());
            }
            match tag.number {
                OFFLINE_ATTESTATION_ANDROID_TAG_USAGE_COUNT_LIMIT => {
                    if usage_count_limit
                        .replace(der_single_integer(value)?)
                        .is_some()
                    {
                        return Err(labeled_invariant(
                            "invalid_attestation",
                            "Android KeyMint authorization list duplicates usageCountLimit",
                        )
                        .into());
                    }
                }

                OFFLINE_ATTESTATION_ANDROID_TAG_ALL_APPLICATIONS => {
                    if all_applications {
                        return Err(labeled_invariant(
                            "invalid_attestation",
                            "Android KeyMint authorization list duplicates allApplications",
                        )
                        .into());
                    }
                    let mut null_reader = DerReader::new(value);
                    null_reader.read_null()?;
                    all_applications = true;
                }
                OFFLINE_ATTESTATION_ANDROID_TAG_ATTESTATION_APPLICATION_ID => {
                    let app_id_der = der_single_octet_string(value)?;
                    if application_id
                        .replace(parse_android_attestation_application_id(&app_id_der)?)
                        .is_some()
                    {
                        return Err(labeled_invariant(
                            "invalid_attestation",
                            "Android KeyMint authorization list duplicates attestationApplicationId",
                        )
                        .into());
                    }
                }
                _ => {}
            }
        }
        Ok((usage_count_limit, all_applications, application_id))
    }

    fn parse_android_key_description(
        extension_value: &[u8],
    ) -> Result<AndroidKeyDescription, Error> {
        let mut reader = DerReader::sequence(extension_value)?;
        let attestation_version = reader.read_integer()?;
        if attestation_version <= 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation version must be positive",
            )
            .into());
        }
        let attestation_security_level = reader.read_enumerated()?;
        let keymint_version = reader.read_integer()?;
        if keymint_version < 0 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint version must be non-negative",
            )
            .into());
        }
        let keymint_security_level = reader.read_enumerated()?;
        let attestation_challenge = reader.read_octet_string()?;
        let _unique_id = reader.read_octet_string()?;
        let software_enforced = reader.read_sequence_bytes()?;
        let hardware_enforced = reader.read_sequence_bytes()?;
        if reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation extension has trailing fields",
            )
            .into());
        }
        let (software_usage_count_limit, software_all_applications, software_application_id) =
            parse_android_authorization_list(&software_enforced)?;
        let (hardware_usage_count_limit, hardware_all_applications, hardware_application_id) =
            parse_android_authorization_list(&hardware_enforced)?;
        if software_usage_count_limit.is_some() && hardware_usage_count_limit.is_some() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization lists duplicate usageCountLimit",
            )
            .into());
        }
        if software_all_applications && hardware_all_applications {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization lists duplicate allApplications",
            )
            .into());
        }
        if software_application_id.is_some() && hardware_application_id.is_some() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization lists duplicate attestationApplicationId",
            )
            .into());
        }
        Ok(AndroidKeyDescription {
            attestation_security_level,
            keymint_security_level,
            attestation_challenge,
            usage_count_limit: hardware_usage_count_limit.or(software_usage_count_limit),
            all_applications: software_all_applications || hardware_all_applications,
            application_id: software_application_id.or(hardware_application_id),
        })
    }

    fn is_android_hardware_security_level(level: i64) -> bool {
        level == OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_TRUSTED_ENVIRONMENT
            || level == OFFLINE_ATTESTATION_ANDROID_SECURITY_LEVEL_STRONG_BOX
    }

    fn validate_android_keymint_report(
        registration: &OfflineDeviceAttestationRegistration,
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        let report = parse_android_keymint_report(registration)?;
        let trusted_roots =
            trusted_root_der_for_platform(policy, &registration.platform, block_unix_timestamp_ms)?;
        let revoked_certificate_sha256 = policy_revoked_certificate_hashes(policy)?;
        let evaluation_time = x509_evaluation_time(block_unix_timestamp_ms)?;
        validate_attestation_certificate_chain(
            &report.certificates,
            &trusted_roots,
            &revoked_certificate_sha256,
            evaluation_time,
        )?;

        let attested_certificate_der = report.certificates.first().ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint certificate chain is missing the attested leaf certificate",
            )
        })?;
        let attested_certificate = parse_x509_certificate_der(attested_certificate_der)?;
        let extension_value = x509_unique_extension_value(
            &attested_certificate,
            OFFLINE_ATTESTATION_ANDROID_KEY_OID,
            "Android KeyMint leaf certificate contains duplicate attestation extensions",
        )?
        .ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint leaf certificate is missing the attestation extension",
            )
        })?;
        let key_description = parse_android_key_description(&extension_value)?;
        if key_description.attestation_challenge != registration.challenge_hash.as_ref() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation challenge does not match the canonical challenge",
            )
            .into());
        }
        if !is_android_hardware_security_level(key_description.attestation_security_level)
            || !is_android_hardware_security_level(key_description.keymint_security_level)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation must be hardware-backed",
            )
            .into());
        }
        if key_description.usage_count_limit != Some(1) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation must bind usageCountLimit to one",
            )
            .into());
        }
        if key_description.all_applications {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation must not be scoped to all applications",
            )
            .into());
        }
        let (package_name, signing_digest) = android_attestation_metadata(registration)?;
        ensure_android_app_allowed_by_policy(policy, &package_name, &signing_digest)?;
        let application_id = key_description.application_id.ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation is missing attestationApplicationId",
            )
        })?;
        if !application_id
            .packages
            .iter()
            .any(|package| package.package_name == package_name)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation package name does not match registration",
            )
            .into());
        }
        if !application_id
            .signature_digests
            .iter()
            .any(|digest| digest.as_slice() == signing_digest)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation signing digest does not match registration",
            )
            .into());
        }
        let subject_public_key = x509_subject_public_key_bytes(&attested_certificate);
        if subject_public_key != registration.assertion_public_key {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint certificate public key does not match the registered assertion key",
            )
            .into());
        }
        // Android cannot challenge-bind `key_id`: KeyMint creates this public
        // key while processing the challenge. Bind it here, after the leaf
        // certificate key has been authenticated, so a submitted identifier
        // cannot select or substitute a different assertion key.
        validate_android_key_id(registration)?;
        Ok(())
    }

    fn validate_offline_attestation_report(
        registration: &OfflineDeviceAttestationRegistration,
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        match registration.platform.as_str() {
            OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                validate_ios_app_attest_report(registration, policy, block_unix_timestamp_ms)
            }
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                validate_android_keymint_report(registration, policy, block_unix_timestamp_ms)
            }
            _ => Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation platform is unsupported",
            )
            .into()),
        }
    }

    fn validate_offline_attestation_evidence_bytes(
        registration: &OfflineDeviceAttestationRegistration,
    ) -> Result<(), Error> {
        if registration.attestation_report.is_empty() || registration.evidence.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation report and evidence bytes must be non-empty",
            )
            .into());
        }
        if registration.attestation_report.len() > OFFLINE_ATTESTATION_MAX_REPORT_BYTES {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation report exceeds the on-chain size limit",
            )
            .into());
        }
        if registration.evidence.len() > OFFLINE_ATTESTATION_MAX_EVIDENCE_BYTES {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation evidence exceeds the on-chain size limit",
            )
            .into());
        }
        if Hash::new(&registration.attestation_report) != registration.attestation_report_hash {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation report hash does not match report bytes",
            )
            .into());
        }
        if Hash::new(&registration.evidence) != registration.evidence_hash {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation evidence hash does not match evidence bytes",
            )
            .into());
        }
        if registration.evidence.len() != OFFLINE_ATTESTATION_EVIDENCE_PREFIX.len() + Hash::LENGTH
            || !registration
                .evidence
                .starts_with(OFFLINE_ATTESTATION_EVIDENCE_PREFIX)
            || &registration.evidence[OFFLINE_ATTESTATION_EVIDENCE_PREFIX.len()..]
                != registration.attestation_report_hash.as_ref()
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation evidence envelope must bind the attestation report hash",
            )
            .into());
        }
        Ok(())
    }

    fn validate_offline_device_attestation_registration(
        registration: &OfflineDeviceAttestationRegistration,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Hash, Error> {
        ensure_can_submit_kagemusha_for_account(
            &registration.account_id,
            authority,
            state_transaction,
        )?;
        if registration.version != 1 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation registration version is unsupported",
            )
            .into());
        }
        for (field, value) in [
            ("platform", registration.platform.as_str()),
            ("key_id", registration.key_id.as_str()),
            ("device_id", registration.device_id.as_str()),
            ("assertion_scheme", registration.assertion_scheme.as_str()),
            (
                "assertion_key_algorithm",
                registration.assertion_key_algorithm.as_str(),
            ),
        ] {
            validate_attestation_protocol_string(
                "offline device attestation",
                field,
                value,
                "invalid_attestation",
            )
            .map_err(Error::from)?;
        }
        if registration.assertion_public_key.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation assertion public key must be non-empty",
            )
            .into());
        }
        if is_zero_hash(&registration.challenge_hash)
            || is_zero_hash(&registration.attestation_report_hash)
            || is_zero_hash(&registration.evidence_hash)
            || is_zero_hash(&registration.recent_block_hash)
        {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation hashes must be non-zero",
            )
            .into());
        }
        validate_offline_attestation_platform_profile(registration)?;
        validate_offline_attestation_optional_metadata(registration)?;
        validate_offline_attestation_evidence_bytes(registration)?;
        let expected_challenge_hash = registration.canonical_challenge_hash().map_err(|err| {
            labeled_invariant(
                "invalid_attestation",
                format!("failed to encode Offline attestation challenge preimage: {err}"),
            )
        })?;
        if registration.challenge_hash != expected_challenge_hash {
            return Err(labeled_invariant(
                "invalid_attestation",
                "offline device attestation challenge hash does not match the canonical preimage",
            )
            .into());
        }
        if registration.expires_at_ms <= state_transaction.block_unix_timestamp_ms() {
            return Err(labeled_invariant(
                "expired_attestation",
                "offline device attestation registration is expired",
            )
            .into());
        }
        let policy = effective_offline_device_attestation_policy(state_transaction)?;
        validate_offline_attestation_policy(&policy, state_transaction.block_unix_timestamp_ms())?;
        validate_offline_attestation_recent_block(registration, state_transaction)?;
        validate_offline_attestation_report(
            registration,
            &policy,
            state_transaction.block_unix_timestamp_ms(),
        )?;

        let bytes = norito::to_bytes(registration).map_err(|err| {
            labeled_invariant(
                "invalid_attestation",
                format!("failed to encode Kagemusha device registration: {err}"),
            )
        })?;
        Ok(Hash::new(bytes))
    }

    impl Execute for RegisterOfflineDeviceAttestation {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let registration = self.registration;
            let registration_hash = validate_offline_device_attestation_registration(
                &registration,
                authority,
                state_transaction,
            )?;
            let registration_key = kagemusha_device_registration_key(&registration_hash);
            let challenge_key = kagemusha_attestation_challenge_key(&registration.challenge_hash);
            let report_key =
                kagemusha_attestation_report_key(&registration.attestation_report_hash);
            let evidence_key = kagemusha_attestation_evidence_key(&registration.evidence_hash);
            let kagemusha_v2_device_lineage_key =
                (registration.platform == OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST).then(|| {
                    let evidence_sha256: [u8; 32] = Sha256::digest(&registration.evidence).into();
                    kagemusha_v2_device_lineage_key(
                        &registration.account_id,
                        &registration.device_id,
                        &evidence_sha256,
                        registration.asset_definition_id.as_ref(),
                    )
                });
            for key in [
                &registration_key,
                &challenge_key,
                &report_key,
                &evidence_key,
            ] {
                if state_transaction
                    .world
                    .kagemusha_replay_keys
                    .get(key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_attestation",
                        "Kagemusha device attestation registration reuses registration or evidence material",
                    )
                    .into());
                }
            }

            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(registration_key, ());
            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(challenge_key, ());
            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(report_key, ());
            state_transaction
                .world
                .kagemusha_replay_keys
                .insert(evidence_key, ());
            if let Some(device_lineage_key) = kagemusha_v2_device_lineage_key {
                state_transaction
                    .world
                    .kagemusha_replay_keys
                    .insert(device_lineage_key, ());
            }
            Ok(())
        }
    }

    impl Execute for SetOfflineDeviceAttestationPolicy {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !can_manage_offline_device_attestation_policy(state_transaction, authority) {
                return Err(labeled_invariant(
                    "unauthorized_controller",
                    "only an Offline device attestation policy manager may update verifier policy",
                )
                .into());
            }

            let policy = self.policy;
            validate_offline_attestation_policy(
                &policy,
                state_transaction.block_unix_timestamp_ms(),
            )?;
            let bytes = norito::to_bytes(&policy).map_err(|err| {
                labeled_invariant(
                    "invalid_attestation_policy",
                    format!("failed to encode Offline device attestation policy: {err}"),
                )
            })?;
            state_transaction.world.smart_contract_state.insert(
                (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
                bytes,
            );
            Ok(())
        }
    }

    fn offline_device_attestation_policy_manager_permission() -> Permission {
        Permission::new(
            CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION.into(),
            iroha_primitives::json::Json::new(()),
        )
    }

    fn can_manage_offline_device_attestation_policy(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> bool {
        let required = offline_device_attestation_policy_manager_permission();
        world_has_offline_permission(&state_transaction.world, authority, &required)
    }

    fn ensure_kagemusha_v2_topup_shield_public_inputs(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV2,
        authoritative_initial_root: [u8; 32],
        authoritative_finalized_root: [u8; 32],
        authoritative_leaf_index: u32,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let public = crate::zk::confidential_v2::parse_kagemusha_topup_shield_public_inputs_v2(
            &request.shield_evidence.proof.proof.bytes,
        )
        .map_err(|err| labeled_invariant("invalid_proof", err))?;
        let expected_asset_tag = crate::zk::confidential_v2::derive_confidential_asset_tag_v2(
            &request.asset.definition().to_string(),
        );
        let expected_chain_tag = crate::zk::confidential_v2::derive_confidential_chain_tag_v2(
            state_transaction.chain_id().as_str(),
        );
        let expected_payer_tag = crate::zk::confidential_v2::derive_kagemusha_topup_payer_tag_v2(
            &request.authorization.authority.to_string(),
        );
        let expected_operation_tag =
            crate::zk::confidential_v2::derive_kagemusha_topup_operation_tag_v2(
                &request.operation_id,
            );
        if request.shield_evidence.initial_root != authoritative_initial_root
            || request.shield_evidence.finalized_root != authoritative_finalized_root
            || request.shield_evidence.leaf_index != authoritative_leaf_index
            || public.output_commitment != request.current_note.note_commitment
            || public.spend_nullifier != request.current_note.spend_nullifier
            || public.initial_root != authoritative_initial_root
            || public.finalized_root != authoritative_finalized_root
            || public.atomic_amount
                != crate::zk::confidential_v2::encode_confidential_amount_v2(
                    request.amount.atomic_units,
                )
            || public.asset_scale
                != crate::zk::confidential_v2::encode_kagemusha_topup_u32_v2(request.amount.scale)
            || public.leaf_index
                != crate::zk::confidential_v2::encode_kagemusha_topup_u32_v2(
                    authoritative_leaf_index,
                )
            || public.asset_tag != expected_asset_tag
            || public.chain_tag != expected_chain_tag
            || public.payer_tag != expected_payer_tag
            || public.operation_tag != expected_operation_tag
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha top-up shield proof does not bind the authoritative amount, scale, note, tree, asset, chain, payer, and operation",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_v2_anchor_matches_topup_request(
        anchor: &KagemushaRecursiveSpendTopUpAnchorV2,
        request: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV2,
    ) -> Result<(), Error> {
        if anchor.chain_id != request.current_note.chain_id
            || anchor.payer != request.authorization.authority
            || anchor.asset != request.asset
            || anchor.asset_scale != request.amount.scale
            || anchor.amount != request.amount
            || anchor.initial_root != request.shield_evidence.initial_root
            || anchor.finalized_root != request.shield_evidence.finalized_root
            || anchor.shield_leaf_index != request.shield_evidence.leaf_index
            || anchor.current_note != request.current_note
            || anchor.topup_operation_id != request.operation_id
            || anchor.shield_verifier_id != request.shield_evidence.proof.vk_ref
            || Some(anchor.shield_verifier_commitment)
                != request.shield_evidence.proof.vk_commitment
            || anchor.artifact_binding != request.artifact_binding
        {
            return Err(labeled_invariant(
                "topup_anchor_mismatch",
                "persisted Kagemusha V2 top-up anchor does not match the signed request",
            )
            .into());
        }
        Ok(())
    }

    fn finalized_kagemusha_v2_topup_anchor(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaRecursiveSpendTopUpAnchorV2, Error> {
        let shield_verifier_commitment =
            request.shield_evidence.proof.vk_commitment.ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha V2 top-up shield proof has no verifier commitment",
                )
            })?;
        let finalized_tx_hash = *state_transaction
            .current_tx_hash
            .as_ref()
            .ok_or_else(|| {
                labeled_invariant(
                    "topup_anchor_invalid",
                    "current signed transaction hash is unavailable for Kagemusha V2 top-up",
                )
            })?
            .as_ref();
        let anchor = KagemushaRecursiveSpendTopUpAnchorV2 {
            version: 2,
            chain_id: request.current_note.chain_id.clone(),
            payer: request.authorization.authority.clone(),
            asset: request.asset.clone(),
            asset_scale: request.amount.scale,
            amount: request.amount,
            initial_root: request.shield_evidence.initial_root,
            finalized_root: request.shield_evidence.finalized_root,
            shield_leaf_index: request.shield_evidence.leaf_index,
            current_note: request.current_note.clone(),
            topup_operation_id: request.operation_id,
            shield_verifier_id: request.shield_evidence.proof.vk_ref.clone(),
            shield_verifier_commitment,
            artifact_binding: request.artifact_binding.clone(),
            finalized_height: state_transaction.block_height(),
            finalized_tx_hash,
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .map_err(|err| labeled_invariant("topup_anchor_invalid", err.to_string()))?;
        ensure_kagemusha_v2_anchor_matches_topup_request(&anchor, request)?;
        Ok(anchor)
    }

    fn resolve_kagemusha_v2_recursive_verifier(
        bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
        requested_height: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyRecord, VerifyingKeyRecord), Error> {
        let step_eq_id = &bundle.recursive_proof.verifier_key_id;
        let step_eq_record = state_transaction
            .world
            .verifying_keys
            .get(step_eq_id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha Eq recursive verifier key is not registered",
                )
            })?;
        let step_ep_circuit_key = (
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3.to_owned(),
            step_eq_record.version,
        );
        let step_ep_id = state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&step_ep_circuit_key)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha Ep recursive verifier circuit/version is not active",
                )
            })?;
        let step_ep_record = state_transaction
            .world
            .verifying_keys
            .get(step_ep_id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha Ep recursive verifier key is not registered",
                )
            })?;
        let current_height = state_transaction.block_height();
        for record in [&step_eq_record, &step_ep_record] {
            ensure_kagemusha_v2_verifier_window(record, requested_height, current_height)?;
        }
        ensure_kagemusha_v2_recursive_verifier_shape(
            bundle,
            step_eq_id,
            &step_eq_record,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
        )?;
        ensure_kagemusha_v2_recursive_verifier_shape(
            bundle,
            step_ep_id,
            &step_ep_record,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        )?;
        if step_eq_record.status != ConfidentialStatus::Active
            || step_ep_record.status != ConfidentialStatus::Active
        {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "one Kagemusha recursive verifier record is not currently active",
            )
            .into());
        }
        for (id, record) in [(step_eq_id, &step_eq_record), (step_ep_id, &step_ep_record)] {
            let circuit_key = (record.circuit_id.clone(), record.version);
            if state_transaction
                .world
                .verifying_keys_by_circuit
                .get(&circuit_key)
                != Some(id)
            {
                return Err(labeled_invariant(
                    "verifier_key_inactive",
                    "one Kagemusha recursive verifier circuit/version is not active",
                )
                .into());
            }
        }
        Ok((step_eq_record, step_ep_record))
    }

    fn ensure_kagemusha_v2_recursive_verifier_shape(
        bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
        id: &iroha_data_model::proof::VerifyingKeyId,
        record: &VerifyingKeyRecord,
        parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    ) -> Result<(), Error> {
        let (expected_circuit_id, expected_curve, expected_schema_hash) = match parity {
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq => (
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3,
                iroha_data_model::offline::kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3(),
            ),
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp => (
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3,
                iroha_data_model::offline::kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3(),
            ),
        };
        if record.circuit_id != expected_circuit_id
            || id.name != expected_circuit_id
            || (parity == iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq
                && id != &bundle.recursive_proof.verifier_key_id)
            || record.namespace != iroha_data_model::offline::KAGEMUSHA_VERIFIER_NAMESPACE
            || record.backend != BackendTag::Halo2IpaPasta
            || record.curve != expected_curve
            || record.public_inputs_schema_hash != expected_schema_hash
            || id.backend.as_str()
                != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || record.commitment == [0; 32]
            || record.max_proof_bytes == 0
            || record.max_proof_bytes
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3
            || bundle.recursive_proof.proof.bytes.len() > record.max_proof_bytes as usize
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V2 recursive verifier record is inactive or inconsistent with the proof",
            )
            .into());
        }
        let key = record.key.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V2 recursive verifier key is not available inline",
            )
        })?;
        if key.backend.as_str()
            != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || key.bytes.is_empty()
            || u32::try_from(key.bytes.len()).ok() != Some(record.vk_len)
            || crate::zk::hash_vk(key) != record.commitment
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V2 inline recursive verifier key does not match its record",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_v2_verifier_window(
        record: &VerifyingKeyRecord,
        requested_height: u64,
        current_height: u64,
    ) -> Result<(), Error> {
        if requested_height == 0
            || requested_height > current_height
            || !record.is_active_at(requested_height)
            || !record.is_active_at(current_height)
        {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "Kagemusha V2 verifier is outside its requested or current activation window",
            )
            .into());
        }
        Ok(())
    }

    fn verify_kagemusha_v2_recursive_bundle_with_record(
        bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
        step_eq_record: &VerifyingKeyRecord,
        step_ep_record: &VerifyingKeyRecord,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::zk::kagemusha_v2::ensure_kagemusha_recursive_spend_v2_proof_envelope_binding(
            bundle,
            step_eq_record,
            step_ep_record,
        )
        .map_err(|err| labeled_invariant("invalid_recursive_bundle", err))?;

        let trusted_policy = super::kagemusha_terminal_registry::embedded_release_policy_bytes()
            .map_err(|err| labeled_invariant("recursive_backend_unavailable", err))?;
        let resolved = super::kagemusha_terminal_registry::resolve_with_trusted_policy(
            &bundle.statement.artifact_binding,
            step_eq_record,
            step_ep_record,
            &trusted_policy,
            |key| {
                state_transaction
                    .world
                    .smart_contract_state
                    .get(key)
                    .map(Vec::as_slice)
            },
        )
        .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
        let (envelope, _proof_pair) = super::kagemusha_terminal_registry::decode_proof_pair(
            &bundle.recursive_proof.proof.bytes,
        )
        .map_err(|err| labeled_invariant("invalid_recursive_bundle", err))?;
        envelope
            .validate_against_manifest_for_context(
                resolved.release().manifest(),
                &bundle.statement.chain_id,
                &bundle.statement.asset,
                bundle.statement.asset_scale,
                state_transaction.block_height(),
            )
            .map_err(|err| labeled_invariant("invalid_recursive_bundle", err.to_string()))?;
        debug_assert_eq!(
            resolved.artifacts().manifest_sha256(),
            resolved.release().manifest_sha256()
        );

        // Material selection is now exact and authenticated.  Keep terminal
        // admission closed until the production Eq/Fp and Ep/Fq exact-state
        // Step circuit types exist; binding either transition-only circuit
        // here would parse a processed VK against the wrong circuit shape.
        Err(labeled_invariant(
            "recursive_backend_unavailable",
            "Kagemusha paired terminal verifier material is authenticated, but the exact-state Step circuit verifier is not installed",
        )
        .into())
    }

    fn verify_kagemusha_v2_recursive_bundle(
        bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
        requested_height: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyRecord, VerifyingKeyRecord), Error> {
        let (step_eq_record, step_ep_record) =
            resolve_kagemusha_v2_recursive_verifier(bundle, requested_height, state_transaction)?;
        verify_kagemusha_v2_recursive_bundle_with_record(
            bundle,
            &step_eq_record,
            &step_ep_record,
            state_transaction,
        )?;
        Ok((step_eq_record, step_ep_record))
    }

    struct KagemushaV2RedemptionCommitPlan {
        definition_id: AssetDefinitionId,
        zk_asset_state: crate::state::ZkAssetState,
        escrow_credit: KagemushaV2EscrowCreditPlan,
        branch_commit: KagemushaV2BranchCommitPlan,
        receipt_key: Name,
        receipt_digest: [u8; 32],
        replay_markers: [Hash; 4],
    }

    impl KagemushaV2RedemptionCommitPlan {
        fn commit(self, state_transaction: &mut StateTransaction<'_, '_>) -> Result<(), Error> {
            // The balance move is the only fallible ledger mutation remaining.
            // Every proof, conflict marker, tree update, and receipt collision was
            // validated while constructing this plan.
            self.escrow_credit.commit(state_transaction)?;
            state_transaction
                .world
                .zk_assets
                .remove(self.definition_id.clone());
            state_transaction
                .world
                .zk_assets
                .insert(self.definition_id, self.zk_asset_state);
            self.branch_commit.commit(state_transaction);
            state_transaction
                .world
                .smart_contract_state
                .insert(self.receipt_key, self.receipt_digest.to_vec());
            commit_kagemusha_v2_replay_markers(self.replay_markers, state_transaction);
            Ok(())
        }
    }

    struct KagemushaV2RedemptionPlanInput<'a> {
        definition_id: &'a AssetDefinitionId,
        source_asset: &'a AssetId,
        recipient: &'a AccountId,
        amount: Numeric,
        current_nullifier: [u8; 32],
        consumed_claims: &'a [KagemushaRecursiveSpendBranchClaimV2],
        redemption_binding: Option<[u8; 32]>,
        change_output: Option<&'a iroha_data_model::offline::KagemushaSpendableNoteDescriptorV2>,
        change_children: &'a [(
            KagemushaRecursiveSpendBranchClaimV2,
            KagemushaRecursiveSpendBranchClaimV2,
        )],
        operation_id: [u8; 32],
        receipt_digest: [u8; 32],
        replay_markers: [Hash; 4],
    }

    fn plan_kagemusha_v2_redemption_state_commit(
        input: KagemushaV2RedemptionPlanInput<'_>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2RedemptionCommitPlan, Error> {
        let mut zk_asset_state = state_transaction
            .world
            .zk_assets
            .get(input.definition_id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha V2 redemption requires configured shielded asset state",
                )
            })?;
        if !zk_asset_state.allow_unshield {
            return Err(labeled_invariant(
                "unshield_not_permitted",
                "Kagemusha V2 redemption is not permitted by asset policy",
            )
            .into());
        }
        if zk_asset_state.nullifiers.contains(&input.current_nullifier) {
            return Err(labeled_invariant(
                "duplicate_nullifier",
                "Kagemusha V2 spendable-note nullifier is already redeemed",
            )
            .into());
        }
        if zk_asset_state
            .commitments
            .contains(&input.current_nullifier)
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha V2 spendable-note nullifier collides with a confidential commitment",
            )
            .into());
        }
        if let Some(change) = input.change_output {
            if zk_asset_state.commitments.contains(&change.note_commitment) {
                return Err(labeled_invariant(
                    "duplicate_output",
                    "Kagemusha V2 redemption change commitment already exists",
                )
                .into());
            }
            if zk_asset_state.nullifiers.contains(&change.spend_nullifier)
                || change.spend_nullifier == input.current_nullifier
            {
                return Err(labeled_invariant(
                    "duplicate_nullifier",
                    "Kagemusha V2 redemption change nullifier collides with ledger state",
                )
                .into());
            }
            if change.note_commitment == input.current_nullifier
                || zk_asset_state.nullifiers.contains(&change.note_commitment)
                || zk_asset_state.commitments.contains(&change.spend_nullifier)
            {
                return Err(labeled_invariant(
                    "proof_binding",
                    "Kagemusha V2 redemption change material overlaps an existing commitment or nullifier",
                )
                .into());
            }
        }
        let branch_commit = plan_kagemusha_v2_consumed_branch_set(
            input.consumed_claims,
            input.redemption_binding,
            input.change_children,
            state_transaction,
        )?;

        if !zk_asset_state.nullifiers.insert(input.current_nullifier) {
            unreachable!("the V2 nullifier was checked before insertion into the cloned state");
        }
        if let Some(change) = input.change_output {
            crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                &mut zk_asset_state,
                change.note_commitment,
                state_transaction.zk.root_history_cap,
            )?;
            let _frontier_update = zk_asset_state.record_frontier_checkpoint(
                state_transaction.block_height(),
                state_transaction.zk.tree_frontier_checkpoint_interval,
                state_transaction.zk.reorg_depth_bound,
            );
        }
        let escrow_credit = plan_kagemusha_v2_escrow_credit(
            input.source_asset,
            input.recipient,
            &input.amount,
            state_transaction,
        )?;
        let receipt_key =
            ensure_kagemusha_v2_redemption_receipt_absent(input.operation_id, state_transaction)?;
        Ok(KagemushaV2RedemptionCommitPlan {
            definition_id: input.definition_id.clone(),
            zk_asset_state,
            escrow_credit,
            branch_commit,
            receipt_key,
            receipt_digest: input.receipt_digest,
            replay_markers: input.replay_markers,
        })
    }

    fn plan_kagemusha_v2_redemption_commit(
        request: &iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV2,
        source_asset: &AssetId,
        receipt_digest: [u8; 32],
        replay_markers: [Hash; 4],
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<KagemushaV2RedemptionCommitPlan, Error> {
        let statement = &request.bundle.statement;
        let amount = request.amount.public_quantity();
        let current_nullifier = statement.current_note.spend_nullifier;
        let (redemption_binding, change_children) =
            if let Some(change) = request.offline_change.as_ref() {
                let binding = request
                    .redemption
                    .binding_digest()
                    .map_err(|err| labeled_invariant("proof_binding", err.to_string()))?;
                let children = request
                    .redemption
                    .parent_branch_claims
                    .iter()
                    .cloned()
                    .zip(change.branch_claims.iter().cloned())
                    .collect::<Vec<_>>();
                (Some(binding), children)
            } else {
                (None, Vec::new())
            };
        plan_kagemusha_v2_redemption_state_commit(
            KagemushaV2RedemptionPlanInput {
                definition_id: &statement.asset,
                source_asset,
                recipient: &request.recipient,
                amount: amount.into_numeric(),
                current_nullifier,
                consumed_claims: &request.redemption.parent_branch_claims,
                redemption_binding,
                change_output: request.offline_change.as_ref().map(|change| &change.output),
                change_children: &change_children,
                operation_id: request.operation_id,
                receipt_digest,
                replay_markers,
            },
            state_transaction,
        )
    }

    fn ensure_kagemusha_v2_redemption_policy_will_be_convertible(
        definition_id: &AssetDefinitionId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let definition = state_transaction.world.asset_definition(definition_id)?;
        let mut policy = *definition.confidential_policy();
        let block_height = state_transaction.block_height();
        if let Some(transition) = policy.pending_transition
            && block_height >= transition.effective_height()
            && transition.new_mode() == ConfidentialPolicyMode::ShieldedOnly
            && state_transaction.world.asset_total_amount(definition_id)? > Quantity::zero()
        {
            // `apply_policy_if_due` aborts a due ShieldedOnly transition while
            // transparent supply remains and restores the previous mode.
            policy.pending_transition = None;
            policy.mode = transition.previous_mode();
        } else {
            policy = policy.apply_if_due(block_height).0;
        }
        if policy.mode() != ConfidentialPolicyMode::Convertible {
            return Err(labeled_invariant(
                "unshield_not_permitted",
                "Kagemusha V2 redemption is not permitted by confidential asset policy",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_v2_redemption_live_context(
        bundle_chain_id: &iroha_data_model::ChainId,
        redemption_chain_id: &iroha_data_model::ChainId,
        live_chain_id: &iroha_data_model::ChainId,
        amount_scale: u32,
        statement_scale: u32,
        live_scale: u32,
    ) -> Result<(), Error> {
        if bundle_chain_id != live_chain_id || redemption_chain_id != live_chain_id {
            return Err(labeled_invariant(
                "wrong_chain",
                "Kagemusha V2 redemption chain id does not match this chain",
            )
            .into());
        }
        if amount_scale != live_scale || statement_scale != live_scale {
            return Err(labeled_invariant(
                "amount_scale_mismatch",
                "Kagemusha V2 redemption scale does not equal the live asset scale",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_v2_proof_backend_available() -> Result<(), Error> {
        if iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE {
            return Ok(());
        }
        // This gate is deliberately fail-closed. Remove it only in the same
        // release that ships the authenticated V3 recursive verifier and both
        // chain executors, so readiness can never expose partial execution.
        Err(labeled_invariant(
            "kagemusha_v2_proof_backend_unavailable",
            "Kagemusha V2 state transitions remain disabled until the authenticated V3 recursive verifier is linked",
        )
        .into())
    }

    impl Execute for TopUpKagemushaRecursiveV2 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction.settlement.offline.kagemusha_enabled {
                return Err(labeled_invariant(
                    "kagemusha_disabled",
                    "Kagemusha V2 recursive top-up is disabled by configuration",
                )
                .into());
            }
            // This guard must run before replay markers, escrow reservation,
            // confidential-tree updates, or balance transfers.
            ensure_kagemusha_v2_proof_backend_available()?;
            let request = self.request;
            request
                .validate_public_binding()
                .map_err(|err| labeled_invariant("invalid_recursive_topup", err.to_string()))?;
            let replay_markers =
                match kagemusha_v2_replay_status(&request.authorization, state_transaction)? {
                    KagemushaV2ReplayStatus::Committed => {
                        let anchor = load_kagemusha_v2_topup_anchor(
                            request.authorization.operation_id,
                            state_transaction,
                        )?;
                        ensure_kagemusha_v2_anchor_matches_topup_request(&anchor, &request)?;
                        return Ok(());
                    }
                    KagemushaV2ReplayStatus::Fresh(markers) => markers,
                };
            request
                .validate_authorization_at(state_transaction.block_unix_timestamp_ms())
                .map_err(|err| labeled_invariant("invalid_authorization", err.to_string()))?;
            if request.asset.account() != &request.authorization.authority {
                return Err(labeled_invariant(
                    "unauthorized_controller",
                    "Kagemusha V2 top-up authority must equal the charged asset account",
                )
                .into());
            }
            ensure_can_submit_kagemusha_topup(&request.asset, authority, state_transaction)?;
            ensure_registered_kagemusha_v2_device(
                &request.authorization,
                request.asset.definition(),
                state_transaction,
            )?;
            if request.current_note.chain_id != *state_transaction.chain_id() {
                return Err(labeled_invariant(
                    "wrong_chain",
                    "Kagemusha V2 top-up chain id does not match this chain",
                )
                .into());
            }
            let spec = state_transaction.numeric_spec_for(request.asset.definition())?;
            let live_scale = spec.scale().ok_or_else(|| {
                labeled_invariant(
                    "amount_scale_invalid",
                    "Kagemusha V2 requires an asset definition with a fixed numeric scale",
                )
            })?;
            if request.amount.scale != live_scale {
                return Err(labeled_invariant(
                    "amount_scale_mismatch",
                    "Kagemusha V2 top-up amount scale does not equal the live asset scale",
                )
                .into());
            }
            let amount = request.amount.public_quantity();
            if amount.scale() != live_scale {
                return Err(labeled_invariant(
                    "amount_scale_mismatch",
                    "Kagemusha V2 top-up Numeric encoding changed the authoritative scale",
                )
                .into());
            }
            assert_numeric_spec_with(amount.as_numeric(), spec)?;
            let policy_mode = crate::smartcontracts::isi::world::isi::apply_policy_if_due(
                state_transaction,
                request.asset.definition(),
            )?
            .mode();
            if policy_mode != ConfidentialPolicyMode::Convertible {
                return Err(labeled_invariant(
                    "shield_not_permitted",
                    "Kagemusha public top-up requires convertible confidential policy",
                )
                .into());
            }
            let mut zk_state = state_transaction
                .world
                .zk_assets
                .get(request.asset.definition())
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "verifier_key_invalid",
                        "Kagemusha V2 top-up requires configured confidential asset state",
                    )
                })?;
            if !zk_state.allow_shield {
                return Err(labeled_invariant(
                    "shield_not_permitted",
                    "Kagemusha top-up shielding is disabled for this asset",
                )
                .into());
            }
            let authoritative_initial_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&zk_state.commitments)
                    .map_err(|err| labeled_invariant("topup_anchor_invalid", err))?;
            if zk_state
                .root_history
                .last()
                .is_some_and(|root| *root != authoritative_initial_root)
            {
                return Err(labeled_invariant(
                    "topup_anchor_invalid",
                    "Kagemusha confidential root history disagrees with the commitment tree",
                )
                .into());
            }
            let authoritative_leaf_index =
                u32::try_from(zk_state.commitments.len()).map_err(|_| {
                    labeled_invariant(
                        "topup_tree_full",
                        "Kagemusha confidential tree position does not fit the protocol index",
                    )
                })?;
            if authoritative_leaf_index
                >= iroha_data_model::offline::KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2
            {
                return Err(labeled_invariant(
                    "topup_tree_full",
                    "Kagemusha confidential tree has no remaining top-up leaves",
                )
                .into());
            }
            if zk_state
                .commitments
                .contains(&request.current_note.note_commitment)
            {
                return Err(labeled_invariant(
                    "duplicate_output",
                    "Kagemusha top-up note commitment already exists",
                )
                .into());
            }
            if zk_state
                .nullifiers
                .contains(&request.current_note.spend_nullifier)
                || zk_state
                    .commitments
                    .contains(&request.current_note.spend_nullifier)
            {
                return Err(labeled_invariant(
                    "duplicate_nullifier",
                    "Kagemusha top-up spend nullifier collides with existing confidential state",
                )
                .into());
            }
            let mut commitments_after = zk_state.commitments.clone();
            commitments_after.push(request.current_note.note_commitment);
            let authoritative_finalized_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&commitments_after)
                    .map_err(|err| labeled_invariant("topup_anchor_invalid", err))?;
            let (shield_vk, _shield_record) = resolve_kagemusha_topup_shield_verifier(
                request.asset.definition(),
                &request.shield_evidence.proof,
                state_transaction,
            )?;
            ensure_kagemusha_v2_topup_shield_public_inputs(
                &request,
                authoritative_initial_root,
                authoritative_finalized_root,
                authoritative_leaf_index,
                state_transaction,
            )?;
            state_transaction
                .register_confidential_proof(request.shield_evidence.proof.proof.bytes.len())?;
            state_transaction.register_commitments(1)?;
            let report = crate::zk::verify_backend_with_timing_checked(
                request.shield_evidence.proof.backend.as_str(),
                &request.shield_evidence.proof.proof,
                Some(&shield_vk),
                &state_transaction.zk,
            );
            if !report.ok {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha V2 top-up shield proof verification failed",
                )
                .into());
            }

            reserve_kagemusha_escrow(state_transaction, &request.asset, amount.as_numeric())?;
            let finalized_root =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                    &mut zk_state,
                    request.current_note.note_commitment,
                    state_transaction.zk.root_history_cap,
                )?;
            if finalized_root != authoritative_finalized_root {
                return Err(labeled_invariant(
                    "topup_anchor_mismatch",
                    "Kagemusha V2 shield root does not equal the authoritative finalized root",
                )
                .into());
            }
            let _frontier_update = zk_state.record_frontier_checkpoint(
                state_transaction.block_height(),
                state_transaction.zk.tree_frontier_checkpoint_interval,
                state_transaction.zk.reorg_depth_bound,
            );
            state_transaction
                .world
                .zk_assets
                .remove(request.asset.definition().clone());
            state_transaction
                .world
                .zk_assets
                .insert(request.asset.definition().clone(), zk_state);
            let anchor = finalized_kagemusha_v2_topup_anchor(&request, state_transaction)?;
            persist_kagemusha_v2_topup_anchor(&anchor, state_transaction)?;
            commit_kagemusha_v2_replay_markers(replay_markers, state_transaction);
            Ok(())
        }
    }

    // Keep the public instruction fail-closed until the V2 proof backend and chain
    // executor are enabled together; the capability invariant above prevents a
    // partially deployed backend from mutating ledger state.
    impl Execute for RedeemKagemushaRecursiveV2 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction.settlement.offline.kagemusha_enabled {
                return Err(labeled_invariant(
                    "kagemusha_disabled",
                    "Kagemusha V2 recursive redemption is disabled by configuration",
                )
                .into());
            }
            // First-release safety boundary: keep this guard until the recursive
            // backend proves every transition field and lineage edge in-circuit.
            // It is intentionally the first operation after the feature flag: no
            // request parsing, proof metering, receipt, balance, or tree state is
            // touched while the backend is unavailable.
            ensure_kagemusha_v2_proof_backend_available()?;
            let request = self.request;
            request
                .validate_public_binding()
                .map_err(|err| labeled_invariant("invalid_recursive_redeem", err.to_string()))?;
            let payload_digest = request
                .unsigned_payload_digest()
                .map_err(|err| labeled_invariant("invalid_recursive_redeem", err.to_string()))?;
            let replay_markers =
                match kagemusha_v2_replay_status(&request.authorization, state_transaction)? {
                    KagemushaV2ReplayStatus::Committed => {
                        ensure_kagemusha_v2_redemption_receipt_matches(
                            request.operation_id,
                            payload_digest,
                            state_transaction,
                        )?;
                        return Ok(());
                    }
                    KagemushaV2ReplayStatus::Fresh(markers) => markers,
                };
            request
                .validate_authorization_at(state_transaction.block_unix_timestamp_ms())
                .map_err(|err| labeled_invariant("invalid_authorization", err.to_string()))?;

            let statement = &request.bundle.statement;
            ensure_can_submit_kagemusha_for_account(
                &request.recipient,
                authority,
                state_transaction,
            )?;
            ensure_registered_kagemusha_v2_device(
                &request.authorization,
                &statement.asset,
                state_transaction,
            )?;

            let spec = state_transaction.numeric_spec_for(&statement.asset)?;
            let live_scale = spec.scale().ok_or_else(|| {
                labeled_invariant(
                    "amount_scale_invalid",
                    "Kagemusha V2 requires an asset definition with a fixed numeric scale",
                )
            })?;
            ensure_kagemusha_v2_redemption_live_context(
                &statement.chain_id,
                &request.redemption.chain_id,
                state_transaction.chain_id(),
                request.amount.scale,
                statement.asset_scale,
                live_scale,
            )?;
            let amount = request.amount.public_quantity();
            if amount.scale() != live_scale {
                return Err(labeled_invariant(
                    "amount_scale_mismatch",
                    "Kagemusha V2 redemption Numeric encoding changed the authoritative scale",
                )
                .into());
            }
            assert_numeric_spec_with(amount.as_numeric(), spec)?;

            let zk_state = state_transaction
                .world
                .zk_assets
                .get(&statement.asset)
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "verifier_key_invalid",
                        "Kagemusha V2 redemption requires configured shielded asset state",
                    )
                })?;
            let provenance = validate_kagemusha_v2_finalized_topup_anchors(
                &statement.topup_anchor_refs,
                statement.current_note.amount.atomic_units,
                request.block_height,
                &zk_state,
                state_transaction,
            )?;

            let (redeem_vk, redeem_record) = resolve_kagemusha_unshield_verifier(
                &statement.asset,
                &request.redeem_proof,
                state_transaction,
            )?;
            ensure_kagemusha_v2_verifier_window(
                &redeem_record,
                request.block_height,
                state_transaction.block_height(),
            )?;
            ensure_kagemusha_v2_redeem_public_inputs(&request, state_transaction, &redeem_record)?;
            let commit_plan = plan_kagemusha_v2_redemption_commit(
                &request,
                &provenance.source_asset,
                payload_digest,
                replay_markers,
                state_transaction,
            )?;
            ensure_kagemusha_v2_redemption_policy_will_be_convertible(
                &statement.asset,
                state_transaction,
            )?;
            state_transaction
                .register_confidential_proof(request.bundle.recursive_proof.proof.bytes.len())?;
            state_transaction
                .register_confidential_proof(request.redeem_proof.proof.bytes.len())?;
            if let Some(change) = request.offline_change.as_ref() {
                state_transaction
                    .register_confidential_proof(change.bundle.recursive_proof.proof.bytes.len())?;
                state_transaction.register_commitments(1)?;
            }
            state_transaction.register_nullifiers(1)?;

            verify_kagemusha_v2_recursive_bundle(
                &request.bundle,
                request.block_height,
                state_transaction,
            )?;
            let redeem_report = crate::zk::verify_backend_with_timing_checked(
                request.redeem_proof.backend.as_str(),
                &request.redeem_proof.proof,
                Some(&redeem_vk),
                &state_transaction.zk,
            );
            if !redeem_report.ok {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha V2 unshield-v3 proof verification failed",
                )
                .into());
            }
            if let Some(change) = request.offline_change.as_ref() {
                verify_kagemusha_v2_recursive_bundle(
                    &change.bundle,
                    request.block_height,
                    state_transaction,
                )?;
            }

            let policy_mode = crate::smartcontracts::isi::world::isi::apply_policy_if_due(
                state_transaction,
                &statement.asset,
            )?
            .mode();
            if policy_mode != ConfidentialPolicyMode::Convertible {
                return Err(labeled_invariant(
                    "confidential_policy_changed",
                    "Kagemusha V2 confidential policy changed after read-only admission",
                )
                .into());
            }
            commit_plan.commit(state_transaction)
        }
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            Registrable,
            account::Account,
            asset::AssetDefinitionId,
            block::BlockHeader,
            domain::DomainId,
            offline::KagemushaDevicePublicKeyV2,
            permission::Permission,
            role::{Role, RoleId},
        };
        use iroha_primitives::json::Json;
        use iroha_test_samples::{ALICE_ID, BOB_ID};
        use p256::elliptic_curve::sec1::ToEncodedPoint as _;

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            role::RoleIdWithOwner,
            state::{State, World},
        };

        const POLICY_TEST_TIME_MS: u64 = 1_800_000_000_000;

        fn offline_permission(name: &str) -> Permission {
            Permission::new(name.to_owned(), Json::new(()))
        }

        fn offline_permission_with_payload(name: &str, payload: Json) -> Permission {
            Permission::new(name.to_owned(), payload)
        }

        fn offline_test_state() -> State {
            let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let bob = Account::new(BOB_ID.clone()).build(&BOB_ID);
            State::new_for_testing(
                World::with([], [alice, bob], []),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            )
        }

        fn offline_test_header() -> BlockHeader {
            BlockHeader::new(
                NonZeroU64::new(1).expect("nonzero block height"),
                None,
                None,
                None,
                POLICY_TEST_TIME_MS,
                0,
            )
        }

        fn offline_test_asset(account: &AccountId) -> AssetId {
            let definition = AssetDefinitionId::new(
                DomainId::try_new("offline", "universal").expect("valid test domain"),
                "cash".parse().expect("valid test asset name"),
            );
            AssetId::new(definition, account.clone())
        }

        fn deliberately_invalid_registration(
            account: &AccountId,
        ) -> OfflineDeviceAttestationRegistration {
            let secret =
                p256::SecretKey::from_slice(&[1_u8; 32]).expect("fixed test scalar must be valid");
            let encoded_public_key = secret.public_key().to_encoded_point(false);
            let public_key =
                KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded_public_key.as_bytes())
                    .expect("derived test public key must be canonical");
            let attestation_report = b"authorization-boundary-report".to_vec();
            let evidence = b"authorization-boundary-evidence".to_vec();

            OfflineDeviceAttestationRegistration {
                // The unsupported version makes validation stop immediately
                // after the authorization boundary.
                version: 0,
                platform: "android-keymint".to_owned(),
                key_id: "authorization-boundary-key".to_owned(),
                device_id: "authorization-boundary-device".to_owned(),
                account_id: account.clone(),
                asset_definition_id: None,
                ios_team_id: None,
                ios_bundle_id: None,
                ios_environment: None,
                android_package_name: None,
                android_signing_certificate_sha256: None,
                public_key,
                assertion_scheme: "android-keymint".to_owned(),
                assertion_key_algorithm: "ecdsa-p256-sha256".to_owned(),
                assertion_public_key: encoded_public_key.as_bytes().to_vec(),
                assertion_usage_count_limit: Some(1),
                one_use: true,
                challenge_hash: Hash::new(b"authorization-boundary-challenge"),
                attestation_report_hash: Hash::new(&attestation_report),
                attestation_report,
                evidence_hash: Hash::new(&evidence),
                evidence,
                recent_block_height: 1,
                recent_block_hash: Hash::new(b"authorization-boundary-block"),
                expires_at_ms: POLICY_TEST_TIME_MS + 60_000,
            }
        }

        fn insert_role(
            state_transaction: &mut StateTransaction<'_, '_>,
            role_name: &str,
            grant_to: &AccountId,
            permissions: impl IntoIterator<Item = Permission>,
        ) -> RoleId {
            let role_id: RoleId = role_name.parse().expect("valid offline test role id");
            let mut role = Role::new(role_id.clone(), grant_to.clone());
            for permission in permissions {
                role = role.add_permission(permission);
            }
            let role = role.build(grant_to);
            state_transaction.world.roles.insert(role_id.clone(), role);
            role_id
        }

        fn assign_role(
            state_transaction: &mut StateTransaction<'_, '_>,
            account: &AccountId,
            role_id: RoleId,
        ) {
            state_transaction
                .world
                .account_roles
                .insert(RoleIdWithOwner::new(account.clone(), role_id), ());
        }

        #[derive(Clone, Copy, Debug)]
        enum GrantSource {
            Direct,
            Role,
        }

        fn grant_permission(
            state_transaction: &mut StateTransaction<'_, '_>,
            account: &AccountId,
            source: GrantSource,
            permission: Permission,
        ) {
            match source {
                GrantSource::Direct => {
                    let _ = state_transaction
                        .world
                        .add_account_permission(account, permission);
                }
                GrantSource::Role => {
                    let role_id = insert_role(
                        state_transaction,
                        "offline_test_manager",
                        account,
                        [permission],
                    );
                    assign_role(state_transaction, account, role_id);
                }
            }
        }

        fn assert_unauthorized(result: Result<(), Error>, context: &str) {
            let error = result.expect_err("offline authorization must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("offline_reason::unauthorized_controller"),
                "{context}: unexpected offline authorization error: {error}"
            );
        }

        #[test]
        fn exact_offline_escrow_grants_and_self_submission_are_preserved() {
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let state_transaction = block.transaction();
            ensure_can_submit_kagemusha_for_account(&ALICE_ID, &ALICE_ID, &state_transaction)
                .expect("an account must remain able to submit for itself");
            ensure_can_submit_kagemusha_topup(
                &offline_test_asset(&ALICE_ID),
                &ALICE_ID,
                &state_transaction,
            )
            .expect("a payer must remain able to submit its own top-up");

            for source in [GrantSource::Direct, GrantSource::Role] {
                let state = offline_test_state();
                let mut block = state.block(offline_test_header());
                let mut state_transaction = block.transaction();
                grant_permission(
                    &mut state_transaction,
                    &ALICE_ID,
                    source,
                    offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION),
                );

                ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction)
                    .unwrap_or_else(|error| {
                        panic!("{source:?} exact permission must authorize delegation: {error}")
                    });
                ensure_can_submit_kagemusha_topup(
                    &offline_test_asset(&BOB_ID),
                    &ALICE_ID,
                    &state_transaction,
                )
                .unwrap_or_else(|error| {
                    panic!("{source:?} exact permission must authorize delegated top-up: {error}")
                });
            }
        }

        #[derive(Clone, Copy, Debug)]
        enum RejectedRoleState {
            Unassigned,
            AssignedToAnotherAccount,
            RevokedAssignment,
            MissingRoleRecord,
        }

        #[test]
        fn stale_or_unrelated_offline_escrow_roles_fail_closed() {
            for case in [
                RejectedRoleState::Unassigned,
                RejectedRoleState::AssignedToAnotherAccount,
                RejectedRoleState::RevokedAssignment,
                RejectedRoleState::MissingRoleRecord,
            ] {
                let state = offline_test_state();
                let mut block = state.block(offline_test_header());
                let mut state_transaction = block.transaction();
                let role_id = insert_role(
                    &mut state_transaction,
                    "offline_escrow_manager",
                    &ALICE_ID,
                    [offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)],
                );

                match case {
                    RejectedRoleState::Unassigned => {}
                    RejectedRoleState::AssignedToAnotherAccount => {
                        assign_role(&mut state_transaction, &BOB_ID, role_id);
                    }
                    RejectedRoleState::RevokedAssignment => {
                        let key = RoleIdWithOwner::new(ALICE_ID.clone(), role_id.clone());
                        assign_role(&mut state_transaction, &ALICE_ID, role_id);
                        assert!(
                            state_transaction.world.account_roles.remove(key).is_some(),
                            "test precondition: assignment must exist before revocation"
                        );
                    }
                    RejectedRoleState::MissingRoleRecord => {
                        assign_role(&mut state_transaction, &ALICE_ID, role_id.clone());
                        assert!(
                            state_transaction.world.roles.remove(role_id).is_some(),
                            "test precondition: assigned role record must exist before removal"
                        );
                    }
                }

                assert_unauthorized(
                    ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction),
                    &format!("{case:?}"),
                );
            }
        }

        #[test]
        fn same_name_non_unit_permission_payloads_are_rejected() {
            let forged_payloads = [
                ("boolean", Json::new(true)),
                ("string", Json::new("forged-scope")),
                ("array", Json::new(vec![1_u8, 2_u8])),
            ];

            for source in [GrantSource::Direct, GrantSource::Role] {
                for (payload_name, payload) in &forged_payloads {
                    let state = offline_test_state();
                    let mut block = state.block(offline_test_header());
                    let mut state_transaction = block.transaction();
                    grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        source,
                        offline_permission_with_payload(
                            CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
                            payload.clone(),
                        ),
                    );

                    assert_unauthorized(
                        ensure_can_submit_kagemusha_for_account(
                            &BOB_ID,
                            &ALICE_ID,
                            &state_transaction,
                        ),
                        &format!("{source:?} same-name {payload_name} payload"),
                    );
                }
            }
        }

        #[test]
        fn only_an_exact_permission_among_multiple_roles_authorizes() {
            let state = offline_test_state();
            let mut block = state.block(offline_test_header());
            let mut state_transaction = block.transaction();

            for (role_name, permission) in [
                (
                    "similarly_named_offline_manager",
                    offline_permission("CanManageOfflineEscrowExtra"),
                ),
                (
                    "wrong_case_offline_manager",
                    offline_permission("canmanageofflineescrow"),
                ),
                (
                    "forged_payload_offline_manager",
                    offline_permission_with_payload(
                        CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
                        Json::new(true),
                    ),
                ),
            ] {
                let role_id =
                    insert_role(&mut state_transaction, role_name, &ALICE_ID, [permission]);
                assign_role(&mut state_transaction, &ALICE_ID, role_id);
            }

            assert_unauthorized(
                ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction),
                "multiple inexact roles",
            );

            let exact_role = insert_role(
                &mut state_transaction,
                "exact_offline_manager",
                &ALICE_ID,
                [offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)],
            );
            assign_role(&mut state_transaction, &ALICE_ID, exact_role);

            ensure_can_submit_kagemusha_for_account(&BOB_ID, &ALICE_ID, &state_transaction)
                .expect("one exact assigned permission among unrelated roles must authorize");
        }

        #[derive(Clone, Copy, Debug)]
        enum RegistrationBoundaryGrant {
            None,
            ExactRole,
            SameNameNonUnitRole,
        }

        #[test]
        fn delegated_registration_enforces_role_permission_at_execute_boundary() {
            for grant in [
                RegistrationBoundaryGrant::None,
                RegistrationBoundaryGrant::ExactRole,
                RegistrationBoundaryGrant::SameNameNonUnitRole,
            ] {
                let state = offline_test_state();
                let mut block = state.block(offline_test_header());
                let mut state_transaction = block.transaction();
                match grant {
                    RegistrationBoundaryGrant::None => {}
                    RegistrationBoundaryGrant::ExactRole => grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Role,
                        offline_permission(CAN_MANAGE_OFFLINE_ESCROW_PERMISSION),
                    ),
                    RegistrationBoundaryGrant::SameNameNonUnitRole => grant_permission(
                        &mut state_transaction,
                        &ALICE_ID,
                        GrantSource::Role,
                        offline_permission_with_payload(
                            CAN_MANAGE_OFFLINE_ESCROW_PERMISSION,
                            Json::new(true),
                        ),
                    ),
                }

                let replay_keys_before =
                    state_transaction.world.kagemusha_replay_keys.iter().count();
                let error = RegisterOfflineDeviceAttestation::new(
                    deliberately_invalid_registration(&BOB_ID),
                )
                .execute(&ALICE_ID, &mut state_transaction)
                .expect_err("deliberately invalid registration must not succeed");

                match grant {
                    RegistrationBoundaryGrant::ExactRole => assert!(
                        error
                            .to_string()
                            .contains("offline_reason::invalid_attestation"),
                        "exact assigned role must pass authorization before validation: {error}"
                    ),
                    RegistrationBoundaryGrant::None
                    | RegistrationBoundaryGrant::SameNameNonUnitRole => assert!(
                        error
                            .to_string()
                            .contains("offline_reason::unauthorized_controller"),
                        "{grant:?} must fail at the authorization boundary: {error}"
                    ),
                }
                assert_eq!(
                    state_transaction.world.kagemusha_replay_keys.iter().count(),
                    replay_keys_before,
                    "{grant:?}: rejected registration mutated replay state"
                );
            }
        }

        #[test]
        fn exact_direct_and_role_policy_manager_permissions_can_update_policy() {
            for source in [GrantSource::Direct, GrantSource::Role] {
                let policy = default_offline_device_attestation_policy()
                    .expect("bundled offline attestation policy must decode");
                let state = offline_test_state();
                let mut block = state.block(offline_test_header());
                let mut state_transaction = block.transaction();
                grant_permission(
                    &mut state_transaction,
                    &ALICE_ID,
                    source,
                    offline_permission(CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION),
                );

                SetOfflineDeviceAttestationPolicy::new(policy.clone())
                    .execute(&ALICE_ID, &mut state_transaction)
                    .unwrap_or_else(|error| {
                        panic!("{source:?} exact policy permission must authorize: {error}")
                    });
                let stored = state_transaction
                    .world
                    .smart_contract_state
                    .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
                    .expect("authorized policy update must write state");
                let decoded: OfflineDeviceAttestationPolicy =
                    norito::decode_from_bytes(stored).expect("stored policy must decode");
                assert_eq!(decoded, policy, "{source:?} stored the wrong policy");
            }
        }

        #[derive(Clone, Copy, Debug)]
        enum RejectedPolicyUpdate {
            NoPermission,
            SimilarPermissionName,
            SameNameNonUnitDirectPayload,
            SameNameNonUnitRolePayload,
            UnsupportedVersion,
            MissingTrustedRoots,
        }

        #[test]
        fn rejected_policy_updates_never_mutate_existing_policy() {
            for case in [
                RejectedPolicyUpdate::NoPermission,
                RejectedPolicyUpdate::SimilarPermissionName,
                RejectedPolicyUpdate::SameNameNonUnitDirectPayload,
                RejectedPolicyUpdate::SameNameNonUnitRolePayload,
                RejectedPolicyUpdate::UnsupportedVersion,
                RejectedPolicyUpdate::MissingTrustedRoots,
            ] {
                let baseline = default_offline_device_attestation_policy()
                    .expect("bundled offline attestation policy must decode");
                let baseline_bytes =
                    norito::to_bytes(&baseline).expect("baseline policy must encode");
                let mut candidate = baseline.clone();
                candidate.revoked_certificate_sha256.push(vec![0xA5_u8; 32]);
                let state = offline_test_state();
                let mut block = state.block(offline_test_header());
                let mut state_transaction = block.transaction();
                state_transaction.world.smart_contract_state.insert(
                    (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
                    baseline_bytes.clone(),
                );

                let expected_reason = match case {
                    RejectedPolicyUpdate::NoPermission => "unauthorized_controller",
                    RejectedPolicyUpdate::SimilarPermissionName => {
                        state_transaction.world.add_account_permission(
                            &ALICE_ID,
                            offline_permission("CanManageOfflineDeviceAttestationPolicyAdditional"),
                        );
                        "unauthorized_controller"
                    }
                    RejectedPolicyUpdate::SameNameNonUnitDirectPayload => {
                        grant_permission(
                            &mut state_transaction,
                            &ALICE_ID,
                            GrantSource::Direct,
                            offline_permission_with_payload(
                                CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                                Json::new(true),
                            ),
                        );
                        "unauthorized_controller"
                    }
                    RejectedPolicyUpdate::SameNameNonUnitRolePayload => {
                        grant_permission(
                            &mut state_transaction,
                            &ALICE_ID,
                            GrantSource::Role,
                            offline_permission_with_payload(
                                CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                                Json::new("forged-scope"),
                            ),
                        );
                        "unauthorized_controller"
                    }
                    RejectedPolicyUpdate::UnsupportedVersion => {
                        grant_permission(
                            &mut state_transaction,
                            &ALICE_ID,
                            GrantSource::Direct,
                            offline_permission(
                                CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                            ),
                        );
                        candidate.version = 2;
                        "invalid_attestation_policy"
                    }
                    RejectedPolicyUpdate::MissingTrustedRoots => {
                        grant_permission(
                            &mut state_transaction,
                            &ALICE_ID,
                            GrantSource::Role,
                            offline_permission(
                                CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION,
                            ),
                        );
                        candidate.trusted_roots.clear();
                        "invalid_attestation_policy"
                    }
                };

                let error = SetOfflineDeviceAttestationPolicy::new(candidate)
                    .execute(&ALICE_ID, &mut state_transaction)
                    .expect_err("adversarial policy update must be rejected");
                assert!(
                    error.to_string().contains(expected_reason),
                    "{case:?}: unexpected policy rejection: {error}"
                );
                assert_eq!(
                    state_transaction
                        .world
                        .smart_contract_state
                        .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY),
                    Some(&baseline_bytes),
                    "{case:?}: rejected update mutated the stored policy"
                );
            }
        }

        #[test]
        fn offline_escrow_manager_permission_is_exact_directly_and_through_roles() {
            let key_pair = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
                .expect("derive offline escrow manager fixture keypair");
            let authority = AccountId::new(key_pair.public_key().clone());
            let role_id: RoleId = "OFFLINE_ESCROW_MANAGER".parse().expect("role id");
            let wrong_direct = Permission::new(
                CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                iroha_primitives::json::Json::new("wildcard"),
            );
            let wrong_role = Role::new(role_id.clone(), authority.clone())
                .add_permission(wrong_direct.clone())
                .build(&authority);
            let mut world = World::default();
            world.account_permissions.insert(
                authority.clone(),
                [wrong_direct.clone()].into_iter().collect(),
            );
            world.roles.insert(role_id.clone(), wrong_role);
            world
                .account_roles
                .insert(RoleIdWithOwner::new(authority.clone(), role_id.clone()), ());
            let state = State::new(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("non-zero height"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut state_transaction = block.transaction();

            assert!(
                !is_offline_escrow_manager(&authority, &state_transaction),
                "matching names with non-canonical payloads must not authorize escrow control"
            );

            state_transaction.world.account_permissions.insert(
                authority.clone(),
                [offline_escrow_manager_permission()].into_iter().collect(),
            );
            assert!(
                is_offline_escrow_manager(&authority, &state_transaction),
                "the exact manager permission granted directly must authorize escrow control"
            );

            state_transaction
                .world
                .account_permissions
                .insert(authority.clone(), [wrong_direct].into_iter().collect());
            let exact_role = Role::new(role_id.clone(), authority.clone())
                .add_permission(offline_escrow_manager_permission())
                .build(&authority);
            state_transaction.world.roles.insert(role_id, exact_role);
            assert!(
                is_offline_escrow_manager(&authority, &state_transaction),
                "the exact manager permission inherited through a role must authorize escrow control"
            );
        }

        #[test]
        fn attestation_policy_manager_permission_is_exact_and_inherited_from_role() {
            let key_pair = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
                .expect("derive offline policy manager fixture keypair");
            let authority = AccountId::new(key_pair.public_key().clone());
            let role_id: RoleId = "OFFLINE_ATTESTATION_POLICY_MANAGER"
                .parse()
                .expect("role id");
            let wrong_payload = Permission::new(
                CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION.into(),
                iroha_primitives::json::Json::new("wildcard"),
            );
            let role = Role::new(role_id.clone(), authority.clone())
                .add_permission(wrong_payload)
                .build(&authority);
            let mut world = World::default();
            world.roles.insert(role_id.clone(), role);
            world
                .account_roles
                .insert(RoleIdWithOwner::new(authority.clone(), role_id.clone()), ());
            let state = State::new(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("non-zero height"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut state_transaction = block.transaction();

            assert!(
                !can_manage_offline_device_attestation_policy(&state_transaction, &authority),
                "a matching name with a non-canonical payload must not authorize policy changes"
            );

            let exact = offline_device_attestation_policy_manager_permission();
            let role = Role::new(role_id.clone(), authority.clone())
                .add_permission(exact)
                .build(&authority);
            state_transaction.world.roles.insert(role_id, role);

            assert!(
                can_manage_offline_device_attestation_policy(&state_transaction, &authority),
                "the exact manager permission inherited through a role must authorize policy changes"
            );
        }
    }
}
