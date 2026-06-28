//! Offline note instruction execution.

use super::prelude::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use std::{
    collections::{BTreeSet, HashSet},
    io::Cursor,
    sync::LazyLock,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::{Algorithm, Hash, PublicKey};
use iroha_data_model::{
    account::AccountId,
    asset::{
        AssetBalancePolicy, AssetBalanceScope, AssetDefinitionId, AssetId,
        definition::ConfidentialPolicyMode,
    },
    confidential::ConfidentialStatus,
    events::data::prelude::{
        OfflineNoteAuditRecorded, OfflineNoteEvent, OfflineNoteIssued, OfflineNoteRedeemed,
    },
    isi::{
        error::{InstructionExecutionError, MathError},
        offline::{
            AuditOfflineNote, IssueOfflineNote, KagemushaTransfer, RedeemKagemushaRecursive,
            RedeemOfflineNote, RegisterOfflineDeviceAttestation, SetOfflineDeviceAttestationPolicy,
        },
    },
    name::Name,
    offline::{
        OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA, OFFLINE_REJECTION_REASON_PREFIX,
        OfflineAndroidAppAttestationPolicy, OfflineDeviceAttestationPolicy,
        OfflineDeviceAttestationRegistration, OfflineDeviceAttestationTrustedRoot,
        OfflineIosAppAttestationPolicy, OfflineNoteAuditOutputClaim, OfflineNoteIssuedClaim,
        OfflineNoteKeyCertificate, OfflineNoteRecursiveProof,
        offline_note_recursive_public_inputs_schema_hash,
    },
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    query::error::FindError,
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
use iroha_primitives::numeric::Numeric;
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

fn canonical_offline_note_asset_id(
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

fn offline_note_escrow_asset_id(source_asset: &AssetId, escrow_account: AccountId) -> AssetId {
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
    let asset = state_transaction
        .world
        .assets
        .get_mut(id)
        .ok_or_else(|| FindError::Asset(id.clone().into()))?;
    let quantity: &mut Numeric = &mut *asset;
    if quantity.mantissa().is_negative() {
        return Err(MathError::NegativeValue.into());
    }
    let candidate = quantity
        .clone()
        .checked_sub(amount.clone())
        .ok_or(MathError::NotEnoughQuantity)?;
    if candidate.mantissa().is_negative() {
        return Err(MathError::NotEnoughQuantity.into());
    }
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
    let is_nonzero = {
        let dst = state_transaction
            .world
            .asset_or_insert_exact(id, Numeric::zero())?;
        let quantity: &mut Numeric = &mut *dst;
        if quantity.mantissa().is_negative() {
            return Err(MathError::NegativeValue.into());
        }
        *quantity = quantity
            .clone()
            .checked_add(amount.clone())
            .ok_or(MathError::Overflow)?;
        !quantity.is_zero()
    };
    if is_nonzero {
        state_transaction.world.track_nonzero_asset_holder(id);
    }
    Ok(())
}

fn reserve_offline_note_escrow(
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
    let source_asset = canonical_offline_note_asset_id(state_transaction, asset)?;
    let escrow_asset = offline_note_escrow_asset_id(&source_asset, escrow_account);
    withdraw_numeric_asset_exact(state_transaction, &source_asset, amount)?;
    if let Err(err) = deposit_numeric_asset_exact(state_transaction, &escrow_asset, amount) {
        deposit_numeric_asset_exact(state_transaction, &source_asset, amount)
            .expect("offline escrow reservation refund must succeed after failed deposit");
        return Err(err);
    }
    Ok(())
}

fn credit_from_offline_note_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset: &AssetId,
    recipient: &AccountId,
    amount: &Numeric,
) -> Result<(), Error> {
    let definition_id = asset.definition().clone();
    let claim_asset = canonical_offline_note_asset_id(state_transaction, asset)?;
    let recipient_asset = AssetId::with_scope(
        definition_id.clone(),
        recipient.clone(),
        claim_asset.scope().clone(),
    );
    let spec = state_transaction.numeric_spec_for(&definition_id)?;
    assert_numeric_spec_with(amount, spec)?;
    state_transaction.world.account(recipient)?;
    if !amount.is_zero() {
        let current_balance = state_transaction
            .world
            .assets
            .get(&recipient_asset)
            .map(|asset| asset.as_ref().clone())
            .unwrap_or_else(Numeric::zero);
        current_balance
            .checked_add(amount.clone())
            .ok_or(MathError::Overflow)?;
    }
    let escrow_account = resolve_offline_escrow_account(state_transaction, &definition_id)?;
    let escrow_account = escrow_account.ok_or_else(|| {
        labeled_invariant(
            "escrow_missing",
            format!("offline escrow account not configured for asset definition `{definition_id}`"),
        )
    })?;
    if amount.is_zero() {
        return state_transaction
            .world
            .deposit_numeric_asset(&recipient_asset, amount);
    }
    ensure_distinct_offline_escrow_account(
        &escrow_account,
        recipient,
        "recipient",
        &definition_id,
    )?;
    let escrow_asset = offline_note_escrow_asset_id(&recipient_asset, escrow_account);
    withdraw_numeric_asset_exact(state_transaction, &escrow_asset, amount)?;
    if let Err(err) = deposit_numeric_asset_exact(state_transaction, &recipient_asset, amount) {
        deposit_numeric_asset_exact(state_transaction, &escrow_asset, amount)
            .expect("escrow refund must succeed after failed deposit credit");
        return Err(err);
    }
    Ok(())
}

/// Execution logic for Offline note instructions.
pub mod isi {
    use super::*;

    const OFFLINE_NOTE_VERIFIER_NAMESPACE: &str = "offline_note";
    const OFFLINE_NOTE_REPLAY_ISSUE_DOMAIN: &str = "offline-note-issued-note";
    const OFFLINE_NOTE_REPLAY_KEY_CERTIFICATE_DOMAIN: &str = "offline-note-issued-key-certificate";
    const OFFLINE_NOTE_REPLAY_ISSUED_CLAIM_DOMAIN: &str = "offline-note-issued-claim";
    const OFFLINE_NOTE_REPLAY_SPENT_CLAIM_DOMAIN: &str = "offline-note-spent-claim";
    const OFFLINE_NOTE_REPLAY_NULLIFIER_DOMAIN: &str = "offline-note-spent-nullifier";
    const OFFLINE_NOTE_REPLAY_AUDIT_TOKEN_DOMAIN: &str = "offline-note-audit-token";
    const OFFLINE_NOTE_REPLAY_AUDIT_RECORD_DOMAIN: &str = "offline-note-audit-record";
    const OFFLINE_NOTE_REPLAY_AUDIT_NULLIFIER_DOMAIN: &str = "offline-note-audit-nullifier";
    const OFFLINE_NOTE_REPLAY_AUDIT_OUTPUT_DOMAIN: &str = "offline-note-audit-output";
    const OFFLINE_NOTE_ATTESTED_CERTIFICATE_DOMAIN: &str = "offline-note-attested-key-certificate";
    const OFFLINE_NOTE_ATTESTATION_CHALLENGE_DOMAIN: &str = "offline-note-attestation-challenge";
    const OFFLINE_NOTE_ATTESTATION_REPORT_DOMAIN: &str = "offline-note-attestation-report";
    const OFFLINE_NOTE_ATTESTATION_EVIDENCE_DOMAIN: &str = "offline-note-attestation-evidence";
    const OFFLINE_ATTESTATION_EVIDENCE_PREFIX: &[u8] = b"offline-device-attestation-evidence-v1";
    const OFFLINE_NOTE_ATTESTATION_RECENT_BLOCK_WINDOW: u64 = 128;
    const OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST_LEGACY: &str = "ios-app-attest";
    const OFFLINE_ATTESTATION_IOS_LEGACY_ASSERTION_SCHEME: &str = "apple-app-attest-v1";
    const OFFLINE_ATTESTATION_IOS_LEGACY_ASSERTION_ALGORITHM: &str = "ecdsa-p256-sha256";
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

    fn validate_offline_note_key_certificate(
        certificate: &OfflineNoteKeyCertificate,
    ) -> Result<(), InstructionExecutionError> {
        if certificate.version != iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION
            || !certificate.one_use
            || certificate
                .assertion_usage_count_limit
                .is_some_and(|limit| limit != 1)
        {
            return Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note operation requires a compact one-use key certificate with a matching hardware usage limit",
            ));
        }
        for (field, value) in [
            ("platform", certificate.platform.as_str()),
            ("key_id", certificate.key_id.as_str()),
            ("device_id", certificate.device_id.as_str()),
            ("assertion_scheme", certificate.assertion_scheme.as_str()),
            (
                "assertion_key_algorithm",
                certificate.assertion_key_algorithm.as_str(),
            ),
        ] {
            validate_attestation_protocol_string(
                "offline note certificate",
                field,
                value,
                "invalid_issuer_cert",
            )?;
        }
        if certificate.assertion_public_key.is_empty() {
            return Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate assertion_public_key must be non-empty",
            ));
        }
        validate_offline_note_key_certificate_profile(certificate)?;
        validate_offline_note_key_certificate_assertion_public_key(certificate)?;
        if certificate.public_key.is_empty() {
            return Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate public key must be non-empty",
            ));
        }
        PublicKey::from_bytes(Algorithm::Ed25519, &certificate.public_key).map_err(|_| {
            labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate public key must be an Ed25519 public key",
            )
        })?;
        Ok(())
    }

    fn validate_offline_note_key_certificate_profile(
        certificate: &OfflineNoteKeyCertificate,
    ) -> Result<(), InstructionExecutionError> {
        let valid = match certificate.platform.as_str() {
            OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST_LEGACY => {
                certificate.assertion_scheme == OFFLINE_ATTESTATION_IOS_LEGACY_ASSERTION_SCHEME
                    && certificate.assertion_key_algorithm
                        == OFFLINE_ATTESTATION_IOS_LEGACY_ASSERTION_ALGORITHM
                    && certificate.assertion_usage_count_limit.is_none()
            }
            OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
                certificate.assertion_scheme == OFFLINE_ATTESTATION_IOS_ASSERTION_SCHEME
                    && certificate.assertion_key_algorithm
                        == OFFLINE_ATTESTATION_IOS_ASSERTION_ALGORITHM
                    && certificate.assertion_usage_count_limit.is_none()
            }
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
                certificate.assertion_scheme == OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME
                    && certificate.assertion_key_algorithm
                        == OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM
                    && certificate.assertion_usage_count_limit == Some(1)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate hardware assertion profile is unsupported",
            ))
        }
    }

    fn validate_offline_note_key_certificate_assertion_public_key(
        certificate: &OfflineNoteKeyCertificate,
    ) -> Result<(), InstructionExecutionError> {
        if certificate.assertion_public_key.len()
            != OFFLINE_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN
            || certificate.assertion_public_key.first() != Some(&0x04)
        {
            return Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate assertion_public_key must be an uncompressed P-256 SEC1 key",
            ));
        }
        P256PublicKey::from_sec1_bytes(&certificate.assertion_public_key)
            .map(|_| ())
            .map_err(|_| {
                labeled_invariant(
                    "invalid_issuer_cert",
                    "offline note certificate assertion_public_key must be a valid uncompressed P-256 SEC1 point",
                )
            })
    }

    fn is_offline_note_transparent_backend(backend: &str) -> bool {
        backend == crate::zk::ZK_BACKEND_HALO2_IPA || crate::zk::is_stark_fri_v1_backend(backend)
    }

    fn ensure_offline_note_transparent_backend(
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
        if !is_offline_note_transparent_backend(backend) {
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
        ensure_offline_note_transparent_backend(backend, backend_tag)
    }

    fn ensure_kagemusha_transfer_verifier_binding(
        asset: &AssetDefinitionId,
        proof: &ProofAttachment,
        root_hint: Option<[u8; 32]>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let zk_state = state_transaction
            .world
            .zk_assets
            .get(asset)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha transfers require a configured shielded asset verifier binding",
                )
            })?;
        let binding = zk_state.vk_transfer.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfers require a bound confidential transfer verifier key",
            )
        })?;
        if proof.vk_ref != binding.id || proof.backend != binding.id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof must reference the asset-bound verifier key",
            )
            .into());
        }
        let Some(commitment) = proof.vk_commitment else {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof must publish the asset-bound verifier-key commitment",
            )
            .into());
        };
        if commitment == [0u8; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof must publish a non-zero asset-bound verifier-key commitment",
            )
            .into());
        }
        if commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier-key commitment does not match the asset binding",
            )
            .into());
        }

        let record = state_transaction
            .world
            .verifying_keys
            .get(&binding.id)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha transfer verifier key is not registered",
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "Kagemusha transfer verifier key is not active",
            )
            .into());
        }
        if record.namespace != crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key is not in the Kagemusha namespace",
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
                    "Kagemusha transfer verifier circuit/version is not active",
                )
                .into());
            }
        }
        if record.commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier-key registry commitment does not match the asset binding",
            )
            .into());
        }
        if record.backend.is_pending_production_backend() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key uses a pending-production backend tag",
            )
            .into());
        }
        if record.backend != BackendTag::Halo2IpaPasta
            || proof.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
            || record.circuit_id != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfers require the canonical transparent confidential-transfer-v2 Halo2/IPA verifier",
            )
            .into());
        }
        if record.curve != "pallas" {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key curve is not pallas",
            )
            .into());
        }
        let expected_schema_hash: [u8; 32] =
            Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1)
                .into();
        if record.public_inputs_schema_hash != expected_schema_hash {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "Kagemusha transfer verifier key uses an unexpected public-input schema",
            )
            .into());
        }
        if record.max_proof_bytes == 0 {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key must publish a non-zero max_proof_bytes cap",
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha transfer proof exceeds verifier record max_proof_bytes",
            )
            .into());
        }
        let vk_box = record.key.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key is not available inline",
            )
        })?;
        if vk_box.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer inline verifier key backend does not match Halo2/IPA",
            )
            .into());
        }
        if vk_box.bytes.is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key bytes must be non-empty",
            )
            .into());
        }
        if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key length mismatch",
            )
            .into());
        }
        if crate::zk::hash_vk(vk_box) != record.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer inline verifier-key commitment mismatch",
            )
            .into());
        }
        crate::zk::confidential_v2::ensure_confidential_transfer_v2_canonical_vk_box(vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_proof",
                    "Kagemusha transfer proof must be an OpenVerifyEnvelope",
                )
            })?;
        if envelope.backend != BackendTag::Halo2IpaPasta {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope backend does not match Halo2/IPA Pasta",
            )
            .into());
        }
        if envelope.circuit_id != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
            || envelope.circuit_id != record.circuit_id
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope must use the canonical asset-bound confidential-transfer-v2 circuit",
            )
            .into());
        }
        if envelope.public_inputs
            != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
        {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "Kagemusha transfer proof envelope public-input schema mismatch",
            )
            .into());
        }
        if !envelope.aux.is_empty() {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha transfer proof envelope must have empty auxiliary bytes",
            )
            .into());
        }
        if envelope.vk_hash == [0u8; 32] {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope verifier-key hash must be non-zero",
            )
            .into());
        }
        if envelope.vk_hash != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope verifier-key hash does not match the asset binding",
            )
            .into());
        }
        if let Some(envelope_hash) = proof.envelope_hash {
            let expected_hash: [u8; 32] = Hash::new(&proof.proof.bytes).into();
            if envelope_hash != expected_hash {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha transfer proof envelope hash does not match the submitted envelope",
                )
                .into());
            }
        }
        if root_hint.is_none() {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha confidential transfers require a root hint",
            )
            .into());
        }
        Ok(())
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
        if record.status != ConfidentialStatus::Active {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "recursive Kagemusha redeem verifier key is not active",
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

    fn expected_kagemusha_recursive_redeem_change_output(
        public_amount: u128,
        current_amount: u128,
        current_note_commitment: [u8; 32],
        change_output: Option<[u8; 32]>,
        redeem_nullifiers: &[[u8; 32]],
    ) -> Result<[u8; 32], Error> {
        let zero = [0u8; 32];
        if public_amount == 0 {
            return Err(labeled_invariant(
                "amount_mismatch",
                "recursive Kagemusha redeem amount must be non-zero",
            )
            .into());
        }
        match change_output {
            None if public_amount == current_amount => Ok(zero),
            None => Err(labeled_invariant(
                "amount_mismatch",
                "recursive Kagemusha partial redeem requires a private change output",
            )
            .into()),
            Some(_) if public_amount >= current_amount => Err(labeled_invariant(
                "amount_mismatch",
                "recursive Kagemusha redeem with change must redeem less than the spendable note amount",
            )
            .into()),
            Some(change_output)
                if change_output == zero
                    || change_output == current_note_commitment
                    || redeem_nullifiers.contains(&change_output) =>
            {
                Err(labeled_invariant(
                    "final_commitment_mismatch",
                    "recursive Kagemusha redeem change commitment is invalid",
                )
                .into())
            }
            Some(change_output) => Ok(change_output),
        }
    }

    fn ensure_kagemusha_recursive_redeem_change_output_available(
        st: &crate::state::ZkAssetState,
        change_output: Option<[u8; 32]>,
    ) -> Result<(), Error> {
        if let Some(change_output) = change_output
            && st.commitments.contains(&change_output)
        {
            return Err(labeled_invariant(
                "duplicate_output",
                "recursive Kagemusha redeem change commitment is already in the shielded tree",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_recursive_redeem_public_inputs(
        instruction: &RedeemKagemushaRecursive,
        state_transaction: &StateTransaction<'_, '_>,
        vk_record: &VerifyingKeyRecord,
        redeem_nullifiers: &[[u8; 32]],
    ) -> Result<(), Error> {
        let current_note = &instruction.bundle.accumulator.current_note;
        if current_note.amount.scale() != 0 {
            return Err(labeled_invariant(
                "amount_mismatch",
                "recursive Kagemusha redeem amount does not match the spendable note descriptor",
            )
            .into());
        }
        let Some(current_amount) = current_note.amount.try_mantissa_u128() else {
            return Err(labeled_invariant(
                "amount_mismatch",
                "recursive Kagemusha redeem amount does not match the spendable note descriptor",
            )
            .into());
        };
        let zero = [0u8; 32];
        let expected_change = expected_kagemusha_recursive_redeem_change_output(
            instruction.public_amount,
            current_amount,
            current_note.note_commitment,
            instruction.change_output,
            redeem_nullifiers,
        )?;
        let expected_public_amount =
            crate::zk::confidential_v2::encode_confidential_amount_v2(instruction.public_amount);
        let expected_asset_tag = crate::zk::confidential_v2::derive_confidential_asset_tag_v2(
            &instruction.bundle.accumulator.asset.to_string(),
        );
        let expected_chain_tag = crate::zk::confidential_v2::derive_confidential_chain_tag_v2(
            state_transaction.chain_id().as_str(),
        );
        if !crate::zk::confidential_v2::is_confidential_unshield_v3_circuit_id(
            &vk_record.circuit_id,
        ) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "recursive Kagemusha redeem requires an unshield v3 final proof",
            )
            .into());
        }
        let (
            input_commitments,
            proof_nullifiers,
            proof_output,
            proof_root,
            public_amount,
            asset_tag,
            chain_tag,
        ) = crate::zk::confidential_v2::parse_unshield_public_inputs_v3(
            &instruction.redeem_proof.proof.bytes,
        )
        .map_err(|err| labeled_invariant("invalid_proof", err.to_string()))?;
        if input_commitments[0] != current_note.note_commitment
            || input_commitments[1] != zero
            || proof_nullifiers[0] != current_note.spend_nullifier
            || proof_nullifiers[1] != zero
            || proof_output != expected_change
            || proof_root != instruction.bundle.accumulator.final_root
            || public_amount != expected_public_amount
            || asset_tag != expected_asset_tag
            || chain_tag != expected_chain_tag
        {
            return Err(labeled_invariant(
                "final_commitment_mismatch",
                "recursive Kagemusha final redeem proof is not bound to the final spendable note and change output",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_recursive_lineage_verifier_records_match_registered<'record>(
        witness: &iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1,
        mut registered_record: impl FnMut(&VerifyingKeyId) -> Option<&'record VerifyingKeyRecord>,
    ) -> Result<(), String> {
        let mut required = BTreeSet::new();
        for step in &witness.record_bundle.bundle.steps {
            required.insert(step.attachment.vk_ref.clone());
        }
        let mut supplied = BTreeSet::new();
        for entry in &witness.record_bundle.verifier_records {
            if !required.contains(&entry.id) {
                return Err(format!(
                    "recursive Kagemusha lineage verifier record `{}`/`{}` is not referenced by any hop",
                    entry.id.backend, entry.id.name
                ));
            }
            if !supplied.insert(entry.id.clone()) {
                return Err(format!(
                    "recursive Kagemusha lineage verifier record `{}`/`{}` is duplicated",
                    entry.id.backend, entry.id.name
                ));
            }
            let Some(registered) = registered_record(&entry.id) else {
                return Err(format!(
                    "recursive Kagemusha lineage verifier record `{}`/`{}` is not registered",
                    entry.id.backend, entry.id.name
                ));
            };
            if registered != &entry.record {
                return Err(format!(
                    "recursive Kagemusha lineage verifier record `{}`/`{}` does not match the registered record",
                    entry.id.backend, entry.id.name
                ));
            }
        }
        for id in required {
            if !supplied.contains(&id) {
                return Err(format!(
                    "recursive Kagemusha lineage verifier record `{}`/`{}` is missing",
                    id.backend, id.name
                ));
            }
        }
        Ok(())
    }

    fn ensure_kagemusha_recursive_lineage_verifier_records_registered(
        witness: &iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), String> {
        ensure_kagemusha_recursive_lineage_verifier_records_match_registered(witness, |id| {
            state_transaction.world.verifying_keys.get(id)
        })
    }

    fn offline_note_public_instances_from_envelope(
        proof: &ProofBox,
        envelope: &OpenVerifyEnvelope,
    ) -> Result<Vec<Vec<[u8; 32]>>, Error> {
        match envelope.backend {
            BackendTag::Halo2IpaPasta => crate::zk::extract_pasta_instance_columns_bytes(
                &envelope.proof_bytes,
            )
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_proof",
                    "offline recursive proof does not expose Halo2 public instances",
                )
                .into()
            }),
            BackendTag::Stark => {
                if !crate::zk::is_stark_fri_v1_backend(proof.backend.as_str()) {
                    return Err(labeled_invariant(
                        "invalid_proof",
                        "offline recursive proof Stark backend is unsupported",
                    )
                    .into());
                }
                let open: StarkFriOpenProofV1 = norito::decode_from_bytes(&envelope.proof_bytes)
                    .map_err(|_| {
                        labeled_invariant(
                            "invalid_proof",
                            "offline recursive proof has invalid STARK public inputs",
                        )
                    })?;
                Ok(open.public_inputs)
            }
            _ => Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof backend is unsupported",
            )
            .into()),
        }
    }

    fn offline_note_resolve_verifier(
        proof: &OfflineNoteRecursiveProof,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyRecord, VerifyingKeyBox, OpenVerifyEnvelope), Error> {
        let verifier_id: &VerifyingKeyId = &proof.verifier_key_id;
        if proof.proof.backend != verifier_id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proof backend does not match verifier key id",
            )
            .into());
        }
        if verifier_id.name.trim().is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key id name must be non-empty",
            )
            .into());
        }
        let backend = verifier_id.backend.as_str();
        if BackendTag::is_pending_production_backend_label(backend) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs may not use pending-production proof backends",
            )
            .into());
        }
        if crate::zk::is_production_claim_backend_label(backend) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs may not use production-claim proof backends",
            )
            .into());
        }
        if crate::zk::is_trusted_setup_backend_label(backend)
            || crate::zk::is_developer_only_backend_label(backend)
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs may not use trusted-setup or developer-only proof backends",
            )
            .into());
        }
        let backend_tag = if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            BackendTag::Halo2IpaPasta
        } else if crate::zk::is_stark_fri_v1_backend(backend) {
            BackendTag::Stark
        } else {
            BackendTag::Unsupported
        };
        ensure_offline_note_transparent_backend(backend, backend_tag)?;
        if proof.proof.bytes.is_empty() {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof must not be empty",
            )
            .into());
        }

        let record = state_transaction
            .world
            .verifying_keys
            .get(verifier_id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "offline recursive verifier key is not registered",
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "offline recursive verifier key is not active",
            )
            .into());
        }
        ensure_offline_note_transparent_backend(backend, record.backend)?;
        if record.namespace != OFFLINE_NOTE_VERIFIER_NAMESPACE {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "offline recursive verifier key is not in the Offline namespace",
            )
            .into());
        }
        if record.public_inputs_schema_hash != offline_note_recursive_public_inputs_schema_hash() {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "offline recursive verifier key uses an unexpected public-input schema",
            )
            .into());
        }
        if record.circuit_id != crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier must use the canonical offline-note-recursive circuit",
            )
            .into());
        }
        if record.max_proof_bytes == 0 {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key must set max_proof_bytes",
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof exceeds verifier max_proof_bytes",
            )
            .into());
        }
        let circuit_key = (record.circuit_id.clone(), record.version);
        match state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&circuit_key)
        {
            Some(active_id) if active_id == verifier_id => {}
            _ => {
                return Err(labeled_invariant(
                    "verifier_key_inactive",
                    "offline recursive verifier circuit/version is not active",
                )
                .into());
            }
        }

        let vk_box = record.key.clone().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key bytes are not available inline",
            )
        })?;
        if vk_box.bytes.is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key bytes must be non-empty",
            )
            .into());
        }
        if vk_box.backend != verifier_id.backend || vk_box.backend != proof.proof.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier backend mismatch",
            )
            .into());
        }
        if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key length mismatch",
            )
            .into());
        }
        if crate::zk::hash_vk(&vk_box) != record.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier commitment mismatch",
            )
            .into());
        }
        crate::zk::ensure_offline_note_recursive_canonical_vk_box(&vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;

        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_proof",
                    "offline recursive proof must be an OpenVerifyEnvelope",
                )
            })?;
        if envelope.backend != record.backend {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof envelope backend mismatch",
            )
            .into());
        }
        if envelope.circuit_id != record.circuit_id {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof circuit id mismatch",
            )
            .into());
        }
        if envelope.vk_hash == [0u8; 32] {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof verifier-key hash must be non-zero",
            )
            .into());
        }
        if envelope.vk_hash != record.commitment {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof verifier commitment mismatch",
            )
            .into());
        }
        if !envelope.aux.is_empty() {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof envelope must have empty auxiliary bytes",
            )
            .into());
        }
        if envelope.public_inputs != OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "offline recursive proof public-input schema mismatch",
            )
            .into());
        }

        Ok((record, vk_box, envelope))
    }

    fn verify_offline_note_recursive_proof(
        proof: &OfflineNoteRecursiveProof,
        expected_public_inputs_hash: &Hash,
        expected_public_instances: Vec<Vec<[u8; 32]>>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &proof.public_inputs_hash != expected_public_inputs_hash {
            return Err(labeled_invariant(
                "proof_binding",
                "offline recursive proof is not bound to expected public inputs",
            )
            .into());
        }

        let (_record, vk_box, envelope) = offline_note_resolve_verifier(proof, state_transaction)?;
        let actual_instances =
            offline_note_public_instances_from_envelope(&proof.proof, &envelope)?;
        if actual_instances != expected_public_instances {
            return Err(labeled_invariant(
                "proof_binding",
                "offline recursive proof public instances do not match expected public inputs",
            )
            .into());
        }

        state_transaction.register_confidential_proof(proof.proof.bytes.len())?;
        let report = crate::zk::verify_backend_with_timing_checked(
            proof.proof.backend.as_str(),
            &proof.proof,
            Some(&vk_box),
            &state_transaction.zk,
        );
        if !report.ok {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof verification failed",
            )
            .into());
        }
        Ok(())
    }

    fn offline_note_replay_key(domain: &str, value: &Hash) -> Hash {
        let mut preimage = Vec::with_capacity(domain.len() + Hash::LENGTH + 1);
        preimage.extend_from_slice(domain.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(value.as_ref());
        Hash::new(&preimage)
    }

    fn offline_note_issue_key(note_commitment: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUE_DOMAIN, note_commitment)
    }

    fn offline_note_key_certificate_key(certificate_payload_hash: &Hash) -> Hash {
        offline_note_replay_key(
            OFFLINE_NOTE_REPLAY_KEY_CERTIFICATE_DOMAIN,
            certificate_payload_hash,
        )
    }

    fn offline_note_issued_claim_key(claim_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUED_CLAIM_DOMAIN, claim_hash)
    }

    fn offline_note_spent_claim_key(claim_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_SPENT_CLAIM_DOMAIN, claim_hash)
    }

    fn offline_note_nullifier_key(nullifier: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_NULLIFIER_DOMAIN, nullifier)
    }

    fn offline_note_audit_token_key(token_id: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_TOKEN_DOMAIN, token_id)
    }

    fn offline_note_audit_record_key(public_inputs_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_RECORD_DOMAIN, public_inputs_hash)
    }

    fn offline_note_audit_nullifier_key(nullifier: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_NULLIFIER_DOMAIN, nullifier)
    }

    fn offline_note_audit_output_key(output_commitment: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_OUTPUT_DOMAIN, output_commitment)
    }

    fn offline_note_attested_certificate_key(certificate_payload_hash: &Hash) -> Hash {
        offline_note_replay_key(
            OFFLINE_NOTE_ATTESTED_CERTIFICATE_DOMAIN,
            certificate_payload_hash,
        )
    }

    fn offline_note_attestation_challenge_key(challenge_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_ATTESTATION_CHALLENGE_DOMAIN, challenge_hash)
    }

    fn offline_note_attestation_report_key(report_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_ATTESTATION_REPORT_DOMAIN, report_hash)
    }

    fn offline_note_attestation_evidence_key(evidence_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_ATTESTATION_EVIDENCE_DOMAIN, evidence_hash)
    }

    fn is_zero_hash(hash: &Hash) -> bool {
        hash.as_ref().iter().all(|byte| *byte == 0)
    }

    fn ensure_unique_hashes(
        hashes: &[Hash],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let mut seen = BTreeSet::new();
        for hash in hashes {
            if !seen.insert(*hash) {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_disjoint_hashes(
        left: &[Hash],
        right: &[Hash],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let left = left.iter().copied().collect::<BTreeSet<_>>();
        for hash in right {
            if left.contains(hash) {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_unique_bytes32(
        values: &[[u8; 32]],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let mut seen = BTreeSet::new();
        for value in values {
            if !seen.insert(*value) {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_disjoint_bytes32(
        left: &[[u8; 32]],
        right: &[[u8; 32]],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let left = left.iter().copied().collect::<BTreeSet<_>>();
        for value in right {
            if left.contains(value) {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_non_zero_bytes32(
        values: &[[u8; 32]],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        for value in values {
            if *value == [0u8; 32] {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_offline_audit_output_claim_count(
        output_commitments_len: usize,
        output_claims_len: usize,
    ) -> Result<(), InstructionExecutionError> {
        if output_claims_len == 0 || output_claims_len != output_commitments_len {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline audit requires output claims to match output commitments one-to-one",
            ));
        }
        Ok(())
    }

    fn ensure_offline_audit_output_claim_binding(
        output_commitments: &[Hash],
        output_claims: &[OfflineNoteAuditOutputClaim],
    ) -> Result<(), InstructionExecutionError> {
        ensure_offline_audit_output_claim_count(output_commitments.len(), output_claims.len())?;
        for (commitment, claim) in output_commitments.iter().zip(output_claims) {
            if commitment != &claim.note_commitment {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline audit output claims must be ordered one-to-one with output commitments",
                ));
            }
        }
        Ok(())
    }

    fn ensure_offline_audit_input_claim_anchor(
        input_claims: &[OfflineNoteIssuedClaim],
        certificate_payload_hash: &Hash,
    ) -> Result<(), InstructionExecutionError> {
        for claim in input_claims {
            if &claim.key_certificate_payload_hash != certificate_payload_hash {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline audit input claim is not anchored to the sender key certificate",
                ));
            }
        }
        Ok(())
    }

    fn ensure_offline_audit_conserves_asset_amounts(
        input_claims: &[OfflineNoteIssuedClaim],
        output_claims: &[OfflineNoteAuditOutputClaim],
    ) -> Result<(), Error> {
        let input_definition = input_claims
            .first()
            .ok_or_else(|| labeled_invariant("invalid_proof", "offline audit requires inputs"))?
            .asset
            .definition();

        let mut input_total = Numeric::zero();
        for claim in input_claims {
            if claim.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline audit input claim amount must be positive",
                )
                .into());
            }
            if claim.asset.definition() != input_definition {
                return Err(labeled_invariant(
                    "asset_mismatch",
                    "offline audit input claims must use one asset definition",
                )
                .into());
            }
            input_total = input_total
                .checked_add(claim.amount.clone())
                .ok_or(MathError::Overflow)?;
        }

        let mut output_total = Numeric::zero();
        for claim in output_claims {
            if claim.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline audit output claim amount must be positive",
                )
                .into());
            }
            if claim.asset.definition() != input_definition {
                return Err(labeled_invariant(
                    "asset_mismatch",
                    "offline audit output claims must use the input asset definition",
                )
                .into());
            }
            output_total = output_total
                .checked_add(claim.amount.clone())
                .ok_or(MathError::Overflow)?;
        }

        if input_total != output_total {
            return Err(labeled_invariant(
                "amount_conservation",
                "offline audit input amount total must equal output amount total",
            )
            .into());
        }
        Ok(())
    }

    fn is_offline_escrow_manager(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> bool {
        state_transaction
            .world
            .account_permissions
            .get(authority)
            .is_some_and(|perms| {
                perms
                    .iter()
                    .any(|permission| permission.name() == CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)
            })
    }

    fn ensure_can_submit_offline_note_for_account(
        account: &AccountId,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if account == authority || is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only the note account or an offline escrow manager may submit Offline notes",
            )
            .into())
        }
    }

    fn ensure_can_issue_offline_note(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only an offline escrow manager may issue Offline notes",
            )
            .into())
        }
    }

    fn ensure_offline_note_certificate_signature(
        certificate: &OfflineNoteKeyCertificate,
        issuer: &AccountId,
    ) -> Result<(), Error> {
        let payload = certificate.signing_bytes().map_err(|err| {
            labeled_invariant(
                "invalid_issuer_cert",
                format!("failed to encode Offline key certificate payload: {err}"),
            )
        })?;
        let issuer_key = issuer.try_signatory().ok_or_else(|| {
            labeled_invariant(
                "invalid_issuer_cert",
                "offline note issuer account must be single-signature",
            )
        })?;
        certificate
            .issuer_signature
            .verify(issuer_key, &payload)
            .map_err(|_| {
                labeled_invariant(
                    "invalid_issuer_cert",
                    "offline key certificate signature does not match issuer account",
                )
                .into()
            })
    }

    fn ensure_offline_note_certificate_authorized(
        certificate: &OfflineNoteKeyCertificate,
        issuer: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Hash, Error> {
        let certificate_payload_hash = certificate.payload_hash().map_err(|err| {
            labeled_invariant(
                "invalid_issuer_cert",
                format!("failed to encode Offline key certificate payload: {err}"),
            )
        })?;
        let attested_certificate_key =
            offline_note_attested_certificate_key(&certificate_payload_hash);
        if state_transaction
            .world
            .offline_note_replay_keys
            .get(&attested_certificate_key)
            .is_some()
        {
            return Ok(certificate_payload_hash);
        }

        ensure_offline_note_certificate_signature(certificate, issuer)?;
        Ok(certificate_payload_hash)
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
            > OFFLINE_NOTE_ATTESTATION_RECENT_BLOCK_WINDOW
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
        ensure_can_submit_offline_note_for_account(
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

        let certificate = registration.key_certificate();
        validate_offline_note_key_certificate(&certificate)?;
        registration.key_certificate_payload_hash().map_err(|err| {
            labeled_invariant(
                "invalid_attestation",
                format!("failed to encode Offline attested key certificate payload: {err}"),
            )
            .into()
        })
    }

    fn offline_note_issued_claim_hash(claim: OfflineNoteIssuedClaim) -> Result<Hash, Error> {
        claim.claim_hash().map_err(|err| {
            labeled_invariant(
                "invalid_proof",
                format!("failed to encode Offline issued-note claim: {err}"),
            )
            .into()
        })
    }

    impl Execute for RegisterOfflineDeviceAttestation {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let registration = self.registration;
            let certificate_payload_hash = validate_offline_device_attestation_registration(
                &registration,
                authority,
                state_transaction,
            )?;
            let attested_certificate_key =
                offline_note_attested_certificate_key(&certificate_payload_hash);
            let challenge_key =
                offline_note_attestation_challenge_key(&registration.challenge_hash);
            let report_key =
                offline_note_attestation_report_key(&registration.attestation_report_hash);
            let evidence_key = offline_note_attestation_evidence_key(&registration.evidence_hash);
            for key in [
                &attested_certificate_key,
                &challenge_key,
                &report_key,
                &evidence_key,
            ] {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_attestation",
                        "offline device attestation registration reuses certificate or evidence material",
                    )
                    .into());
                }
            }

            state_transaction
                .world
                .offline_note_replay_keys
                .insert(attested_certificate_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(challenge_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(report_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(evidence_key, ());
            Ok(())
        }
    }

    impl Execute for SetOfflineDeviceAttestationPolicy {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction
                .world
                .account_permissions
                .get(authority)
                .is_some_and(|perms| {
                    perms.iter().any(|permission| {
                        permission.name() == CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION
                    })
                })
            {
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

    impl Execute for IssueOfflineNote {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let issue = self.issue;
            validate_offline_note_key_certificate(&issue.key_certificate)?;
            if issue.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline note issue amount must be positive",
                )
                .into());
            }
            if issue.key_certificate.account_id != *issue.asset.account() {
                return Err(labeled_invariant(
                    "invalid_issuer_cert",
                    "offline note issue certificate account must match the debited asset owner",
                )
                .into());
            }
            ensure_can_issue_offline_note(authority, state_transaction)?;
            let certificate_payload_hash = ensure_offline_note_certificate_authorized(
                &issue.key_certificate,
                authority,
                state_transaction,
            )?;
            let spec = state_transaction.numeric_spec_for(issue.asset.definition())?;
            assert_numeric_spec_with(&issue.amount, spec)?;
            let issue_key = offline_note_issue_key(&issue.note_commitment);
            let audit_output_key = offline_note_audit_output_key(&issue.note_commitment);
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim_hash = offline_note_issued_claim_hash(
                OfflineNoteIssuedClaim::from_issue(&issue).map_err(|err| {
                    labeled_invariant(
                        "invalid_proof",
                        format!("failed to encode Offline issued-note claim: {err}"),
                    )
                })?,
            )?;
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&issue_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_issue",
                    "offline note commitment is already issued",
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&audit_output_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_issue",
                    "offline note commitment is already issued",
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&certificate_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_key_certificate",
                    "offline key certificate is already issued",
                )
                .into());
            }
            reserve_offline_note_escrow(state_transaction, &issue.asset, &issue.amount)?;
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(issue_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());
            let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
            state_transaction
                .world
                .emit_events(Some(OfflineNoteEvent::NoteIssued(OfflineNoteIssued {
                    note_commitment: issue.note_commitment,
                    account: issue.key_certificate.account_id,
                    asset: issue.asset,
                    amount: issue.amount,
                    recorded_at_ms,
                })));
            Ok(())
        }
    }

    impl Execute for RedeemOfflineNote {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let redemption = self.redemption;
            if redemption.input_nullifiers.is_empty() || redemption.input_nullifiers.len() > 4 {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline redemption requires 1 to 4 input nullifiers",
                )
                .into());
            }
            if redemption.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline redemption amount must be positive",
                )
                .into());
            }
            if redemption.asset.account() != &redemption.recipient {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline redemption asset owner must match recipient",
                )
                .into());
            }
            ensure_unique_hashes(
                &redemption.input_nullifiers,
                "duplicate_nullifier",
                "offline redemption input nullifiers must be unique",
            )?;
            validate_offline_note_key_certificate(&redemption.sender_key_certificate)?;
            ensure_can_submit_offline_note_for_account(
                &redemption.recipient,
                authority,
                state_transaction,
            )?;
            let spec = state_transaction.numeric_spec_for(redemption.asset.definition())?;
            assert_numeric_spec_with(&redemption.amount, spec)?;
            let expected_public_inputs_hash = redemption.public_inputs_hash().map_err(|err| {
                labeled_invariant(
                    "invalid_proof",
                    format!("failed to encode Offline redemption public inputs: {err}"),
                )
            })?;
            if redemption.recursive_proof.public_inputs_hash != expected_public_inputs_hash {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline recursive proof is not bound to redemption public inputs",
                )
                .into());
            }
            let expected_public_instances =
                crate::zk::offline_note_redeem_instance_values(&redemption)
                    .map_err(|err| labeled_invariant("invalid_proof", err))?
                    .public_instance_columns();
            let issued_claim_hash = offline_note_issued_claim_hash(
                OfflineNoteIssuedClaim::from_redemption(&redemption).map_err(|err| {
                    labeled_invariant(
                        "invalid_proof",
                        format!("failed to encode Offline issued-note claim: {err}"),
                    )
                })?,
            )?;
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let issued_commitment_key = offline_note_issue_key(&redemption.source_note_commitment);
            let spent_claim_key = offline_note_spent_claim_key(&issued_claim_hash);
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&issued_claim_key)
                .is_none()
            {
                return Err(labeled_invariant(
                    "note_not_issued",
                    "offline note was not issued for this source commitment, recipient, asset, and amount",
                )
                    .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&issued_commitment_key)
                .is_none()
            {
                return Err(labeled_invariant(
                    "note_not_issued",
                    "offline redemption source note commitment was not issued by top-up or prior audit",
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&spent_claim_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_redeem",
                    "offline issued note is already redeemed",
                )
                .into());
            }
            let consumed_keys = redemption
                .input_nullifiers
                .iter()
                .map(offline_note_nullifier_key)
                .collect::<Vec<_>>();
            for consumed_key in &consumed_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(consumed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_nullifier",
                        "offline nullifier is already redeemed",
                    )
                    .into());
                }
            }
            verify_offline_note_recursive_proof(
                &redemption.recursive_proof,
                &expected_public_inputs_hash,
                expected_public_instances,
                state_transaction,
            )?;
            credit_from_offline_note_escrow(
                state_transaction,
                &redemption.asset,
                &redemption.recipient,
                &redemption.amount,
            )?;
            for consumed_key in consumed_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(consumed_key, ());
            }
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(spent_claim_key, ());
            let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
            state_transaction
                .world
                .emit_events(Some(OfflineNoteEvent::NoteRedeemed(OfflineNoteRedeemed {
                    source_note_commitment: redemption.source_note_commitment,
                    recipient: redemption.recipient,
                    asset: redemption.asset,
                    amount: redemption.amount,
                    recorded_at_ms,
                })));
            Ok(())
        }
    }

    impl Execute for AuditOfflineNote {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let audit = self.audit;
            if audit.input_nullifiers.is_empty() || audit.input_nullifiers.len() > 4 {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 4 input nullifiers",
                )
                .into());
            }
            if audit.input_claims.is_empty()
                || audit.input_claims.len() > 4
                || audit.input_claims.len() != audit.input_nullifiers.len()
            {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 4 input claims matching input nullifiers",
                )
                .into());
            }
            if audit.output_commitments.is_empty() || audit.output_commitments.len() > 2 {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 2 output commitments",
                )
                .into());
            }
            ensure_offline_audit_output_claim_binding(
                &audit.output_commitments,
                &audit.output_claims,
            )?;
            ensure_unique_hashes(
                &audit.input_nullifiers,
                "audit_duplicate_nullifier",
                "offline audit input nullifiers must be unique",
            )?;
            ensure_unique_hashes(
                &audit.output_commitments,
                "audit_duplicate_output",
                "offline audit output commitments must be unique",
            )?;
            ensure_disjoint_hashes(
                &audit.input_nullifiers,
                &audit.output_commitments,
                "proof_binding",
                "offline audit output commitments must be disjoint from input nullifiers",
            )?;
            let output_commitment_set = audit
                .output_commitments
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut output_claim_commitments = BTreeSet::new();
            for output_claim in &audit.output_claims {
                if !output_commitment_set.contains(&output_claim.note_commitment) {
                    return Err(labeled_invariant(
                        "proof_binding",
                        "offline audit output claim is not bound to an output commitment",
                    )
                    .into());
                }
                if !output_claim_commitments.insert(output_claim.note_commitment) {
                    return Err(labeled_invariant(
                        "audit_duplicate_output",
                        "offline audit output claims must be unique",
                    )
                    .into());
                }
                validate_offline_note_key_certificate(&output_claim.key_certificate)?;
                if output_claim.amount <= Numeric::zero() {
                    return Err(labeled_invariant(
                        "invalid_amount",
                        "offline audit output claim amount must be positive",
                    )
                    .into());
                }
                if output_claim.key_certificate.account_id != *output_claim.asset.account() {
                    return Err(labeled_invariant(
                        "invalid_issuer_cert",
                        "offline audit output claim certificate account must match the note asset owner",
                    )
                    .into());
                }
                ensure_offline_note_certificate_authorized(
                    &output_claim.key_certificate,
                    &output_claim.key_certificate.account_id,
                    state_transaction,
                )?;
                let spec = state_transaction.numeric_spec_for(output_claim.asset.definition())?;
                assert_numeric_spec_with(&output_claim.amount, spec)?;
            }
            validate_offline_note_key_certificate(&audit.sender_key_certificate)?;
            let certificate_payload_hash =
                audit.sender_key_certificate.payload_hash().map_err(|err| {
                    labeled_invariant(
                        "invalid_issuer_cert",
                        format!("failed to encode Offline key certificate payload: {err}"),
                    )
                })?;
            ensure_offline_audit_input_claim_anchor(
                &audit.input_claims,
                &certificate_payload_hash,
            )?;
            ensure_offline_audit_conserves_asset_amounts(
                &audit.input_claims,
                &audit.output_claims,
            )?;
            for input_claim in &audit.input_claims {
                let spec = state_transaction.numeric_spec_for(input_claim.asset.definition())?;
                assert_numeric_spec_with(&input_claim.amount, spec)?;
            }
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&certificate_key)
                .is_none()
            {
                return Err(labeled_invariant(
                    "invalid_issuer_cert",
                    "offline audit key certificate was not issued",
                )
                .into());
            }
            let expected_public_inputs_hash = audit.public_inputs_hash().map_err(|err| {
                labeled_invariant(
                    "invalid_proof",
                    format!("failed to encode Offline audit public inputs: {err}"),
                )
            })?;
            if audit.recursive_proof.public_inputs_hash != expected_public_inputs_hash {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline recursive proof is not bound to audit public inputs",
                )
                .into());
            }
            let expected_public_instances = crate::zk::offline_note_audit_instance_values(&audit)
                .map_err(|err| labeled_invariant("invalid_proof", err))?
                .public_instance_columns();
            let audit_token_key = offline_note_audit_token_key(&audit.token_id);
            let audit_record_key = offline_note_audit_record_key(&expected_public_inputs_hash);
            let issued_output_claim_keys = audit
                .output_claims
                .iter()
                .map(|output_claim: &OfflineNoteAuditOutputClaim| {
                    let claim =
                        OfflineNoteIssuedClaim::from_audit_output(output_claim).map_err(|err| {
                            labeled_invariant(
                                "invalid_proof",
                                format!("failed to encode Offline audited output claim: {err}"),
                            )
                        })?;
                    Ok(offline_note_issued_claim_key(
                        &offline_note_issued_claim_hash(claim)?,
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let output_certificate_payload_hashes = audit
                .output_claims
                .iter()
                .map(|output_claim| {
                    output_claim.key_certificate.payload_hash().map_err(|err| {
                        labeled_invariant(
                            "invalid_issuer_cert",
                            format!(
                                "failed to encode Offline output key certificate payload: {err}"
                            ),
                        )
                        .into()
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            ensure_unique_hashes(
                &output_certificate_payload_hashes,
                "duplicate_key_certificate",
                "offline audit output key certificates must be unique",
            )?;
            let issued_output_certificate_keys = output_certificate_payload_hashes
                .iter()
                .map(offline_note_key_certificate_key)
                .collect::<Vec<_>>();
            let input_claim_hashes = audit
                .input_claims
                .iter()
                .cloned()
                .map(offline_note_issued_claim_hash)
                .collect::<Result<Vec<_>, Error>>()?;
            ensure_unique_hashes(
                &input_claim_hashes,
                "duplicate_redeem",
                "offline audit input claims must be unique",
            )?;
            let issued_input_claim_keys = input_claim_hashes
                .iter()
                .map(offline_note_issued_claim_key)
                .collect::<Vec<_>>();
            let issued_input_commitment_keys = audit
                .input_claims
                .iter()
                .map(|claim| offline_note_issue_key(&claim.note_commitment))
                .collect::<Vec<_>>();
            let spent_input_claim_keys = input_claim_hashes
                .iter()
                .map(offline_note_spent_claim_key)
                .collect::<Vec<_>>();
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&audit_token_key)
                .is_some()
            {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(&audit_record_key)
                    .is_some()
                {
                    return Ok(());
                }
                return Err(labeled_invariant(
                    "audit_conflict",
                    "offline audit token already records different public inputs",
                )
                .into());
            }
            let consumed_nullifier_keys = audit
                .input_nullifiers
                .iter()
                .map(offline_note_nullifier_key)
                .collect::<Vec<_>>();
            let observed_nullifier_keys = audit
                .input_nullifiers
                .iter()
                .map(offline_note_audit_nullifier_key)
                .collect::<Vec<_>>();
            let observed_output_keys = audit
                .output_commitments
                .iter()
                .map(offline_note_audit_output_key)
                .collect::<Vec<_>>();
            let issued_output_commitment_keys = audit
                .output_commitments
                .iter()
                .map(offline_note_issue_key)
                .collect::<Vec<_>>();
            for issued_claim_key in &issued_input_claim_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_claim_key)
                    .is_none()
                {
                    return Err(labeled_invariant(
                        "note_not_issued",
                        "offline audit input claim was not issued",
                    )
                    .into());
                }
            }
            for issued_commitment_key in &issued_input_commitment_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_commitment_key)
                    .is_none()
                {
                    return Err(labeled_invariant(
                        "note_not_issued",
                        "offline audit input note commitment was not issued by top-up or prior audit",
                    )
                    .into());
                }
            }
            for spent_claim_key in &spent_input_claim_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(spent_claim_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_redeem",
                        "offline audit input claim is already redeemed",
                    )
                    .into());
                }
            }
            for consumed_key in &consumed_nullifier_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(consumed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_nullifier",
                        "offline audit nullifier is already redeemed",
                    )
                    .into());
                }
            }
            for observed_key in &observed_nullifier_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(observed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "audit_duplicate_nullifier",
                        "offline audit observed a duplicate nullifier",
                    )
                    .into());
                }
            }
            for observed_key in &observed_output_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(observed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "audit_duplicate_output",
                        "offline audit observed a duplicate output commitment",
                    )
                    .into());
                }
            }
            for issued_output_commitment_key in &issued_output_commitment_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_output_commitment_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_issue",
                        "offline audit output commitment is already issued",
                    )
                    .into());
                }
            }
            for issued_claim_key in &issued_output_claim_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_claim_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_issue",
                        "offline audit output claim is already issued",
                    )
                    .into());
                }
            }
            for issued_certificate_key in &issued_output_certificate_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_certificate_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_key_certificate",
                        "offline audit output key certificate is already issued",
                    )
                    .into());
                }
            }
            verify_offline_note_recursive_proof(
                &audit.recursive_proof,
                &expected_public_inputs_hash,
                expected_public_instances,
                state_transaction,
            )?;
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(audit_token_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(audit_record_key, ());
            for consumed_key in consumed_nullifier_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(consumed_key, ());
            }
            for spent_claim_key in spent_input_claim_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(spent_claim_key, ());
            }
            for observed_key in observed_nullifier_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(observed_key, ());
            }
            for observed_key in observed_output_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(observed_key, ());
            }
            for issued_output_commitment_key in issued_output_commitment_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(issued_output_commitment_key, ());
            }
            for issued_claim_key in issued_output_claim_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(issued_claim_key, ());
            }
            for issued_certificate_key in issued_output_certificate_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(issued_certificate_key, ());
            }
            let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
            state_transaction
                .world
                .emit_events(Some(OfflineNoteEvent::AuditRecorded(
                    OfflineNoteAuditRecorded {
                        token_id: audit.token_id,
                        account: audit.sender_key_certificate.account_id,
                        public_inputs_hash: expected_public_inputs_hash,
                        recorded_at_ms,
                    },
                )));
            Ok(())
        }
    }

    impl Execute for KagemushaTransfer {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction.settlement.offline.kagemusha_enabled {
                return Err(labeled_invariant(
                    "kagemusha_disabled",
                    "Kagemusha offline-offline settlement is disabled by configuration",
                )
                .into());
            }
            if self.inputs.is_empty()
                || self.inputs.len() > 2
                || self.outputs.is_empty()
                || self.outputs.len() > 2
            {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha transfers require 1 to 2 input nullifiers and 1 to 2 output commitments",
                )
                .into());
            }
            ensure_non_zero_bytes32(
                &self.inputs,
                "invalid_proof",
                "Kagemusha transfer input nullifiers must be non-zero",
            )?;
            ensure_non_zero_bytes32(
                &self.outputs,
                "invalid_proof",
                "Kagemusha transfer output commitments must be non-zero",
            )?;
            ensure_unique_bytes32(
                &self.inputs,
                "duplicate_nullifier",
                "Kagemusha transfer input nullifiers must be unique",
            )?;
            ensure_unique_bytes32(
                &self.outputs,
                "duplicate_output",
                "Kagemusha transfer output commitments must be unique",
            )?;
            ensure_disjoint_bytes32(
                &self.inputs,
                &self.outputs,
                "proof_binding",
                "Kagemusha transfer output commitments must be disjoint from input nullifiers",
            )?;
            ensure_kagemusha_transparent_attachment(&self.proof)?;
            ensure_kagemusha_transfer_verifier_binding(
                &self.asset,
                &self.proof,
                self.root_hint,
                state_transaction,
            )?;
            let transfer = iroha_data_model::isi::zk::ZkTransfer::new(
                self.asset,
                self.inputs,
                self.outputs,
                self.proof,
                self.root_hint,
            );
            transfer.execute(authority, state_transaction)
        }
    }

    impl Execute for RedeemKagemushaRecursive {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction.settlement.offline.kagemusha_enabled {
                return Err(labeled_invariant(
                    "kagemusha_disabled",
                    "Kagemusha recursive redemption is disabled by configuration",
                )
                .into());
            }
            if self.public_amount == 0 {
                return Err(labeled_invariant(
                    "amount_mismatch",
                    "recursive Kagemusha redemption amount must be non-zero",
                )
                .into());
            }
            self.bundle
                .validate_public_input_binding()
                .map_err(|err| labeled_invariant("invalid_recursive_bundle", err.to_string()))?;
            let redeem_nullifiers =
                self.bundle.accumulator.redeem_nullifiers().map_err(|err| {
                    labeled_invariant("invalid_recursive_bundle", err.to_string())
                })?;
            let current_note_nullifier = self.bundle.accumulator.current_note.spend_nullifier;
            if self.bundle.accumulator.chain_id != *state_transaction.chain_id() {
                return Err(labeled_invariant(
                    "wrong_chain",
                    "recursive Kagemusha bundle chain id does not match this chain",
                )
                .into());
            }

            let def_id = self.bundle.accumulator.asset.clone();
            let policy_mode = crate::smartcontracts::isi::world::isi::apply_policy_if_due(
                state_transaction,
                &def_id,
            )?
            .mode();
            match policy_mode {
                ConfidentialPolicyMode::TransparentOnly | ConfidentialPolicyMode::ShieldedOnly => {
                    return Err(labeled_invariant(
                        "unshield_not_permitted",
                        "recursive Kagemusha redemption is not permitted by policy",
                    )
                    .into());
                }
                ConfidentialPolicyMode::Convertible => {}
            }

            let mut st = state_transaction
                .world
                .zk_assets
                .get(&def_id)
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "verifier_key_invalid",
                        "recursive Kagemusha redemption requires configured shielded asset state",
                    )
                })?;
            if !st.allow_unshield {
                return Err(labeled_invariant(
                    "unshield_not_permitted",
                    "recursive Kagemusha redemption is not permitted by asset policy",
                )
                .into());
            }
            if !st
                .root_history
                .iter()
                .any(|root| root == &self.bundle.accumulator.initial_root)
            {
                return Err(labeled_invariant(
                    "stale_root",
                    "recursive Kagemusha initial root is stale or unknown",
                )
                .into());
            }
            for nullifier in &redeem_nullifiers {
                if st.nullifiers.contains(nullifier) {
                    let kind = if *nullifier == current_note_nullifier {
                        "current note"
                    } else {
                        "top-up anchor"
                    };
                    return Err(labeled_invariant(
                        "duplicate_nullifier",
                        format!("recursive Kagemusha {kind} nullifier is already spent"),
                    )
                    .into());
                }
            }
            ensure_kagemusha_recursive_redeem_change_output_available(&st, self.change_output)?;

            let (redeem_vk, redeem_record) = resolve_kagemusha_unshield_verifier(
                &def_id,
                &self.redeem_proof,
                state_transaction,
            )?;
            ensure_kagemusha_recursive_redeem_public_inputs(
                &self,
                state_transaction,
                &redeem_record,
                &redeem_nullifiers,
            )?;
            let recursive_record = state_transaction
                .world
                .verifying_keys
                .get(&self.bundle.recursive_proof.verifier_key_id)
                .cloned()
                .ok_or_else(|| {
                    labeled_invariant(
                        "verifier_key_invalid",
                        "recursive Kagemusha spend verifier key is not registered",
                    )
                })?;
            let block_height = state_transaction.block_height();
            crate::zk::preverify_kagemusha_recursive_spend_bundle_with_record_at_height(
                &self.bundle,
                &recursive_record,
                block_height,
            )
            .map_err(|err| labeled_invariant("invalid_recursive_bundle", err))?;
            state_transaction
                .register_confidential_proof(self.bundle.recursive_proof.proof.bytes.len())?;

            if let Some(lineage_witness) = &self.lineage_witness {
                ensure_kagemusha_recursive_lineage_verifier_records_registered(
                    lineage_witness,
                    state_transaction,
                )
                .map_err(|err| labeled_invariant("invalid_recursive_lineage", err))?;
                crate::zk::verify_kagemusha_recursive_spend_lineage_witness_with_record_resolver_at_height(
                    &self.bundle,
                    lineage_witness,
                    block_height,
                    |id| state_transaction.world.verifying_keys.get(id),
                )
                .map_err(|err| labeled_invariant("invalid_recursive_lineage", err))?;
            } else {
                crate::zk::ensure_kagemusha_recursive_spend_chain_admission_proves_lineage(
                    &self.bundle,
                )
                .map_err(|err| labeled_invariant("invalid_recursive_bundle", err))?;
            }
            if !crate::zk::verify_kagemusha_recursive_spend_bundle_with_record_at_height(
                &self.bundle,
                &recursive_record,
                block_height,
            ) {
                return Err(labeled_invariant(
                    "invalid_recursive_bundle",
                    "recursive Kagemusha spend proof verification failed",
                )
                .into());
            }
            if self.change_output.is_some() {
                state_transaction.register_commitments(1)?;
            }
            state_transaction.register_nullifiers(redeem_nullifiers.len())?;
            state_transaction.register_confidential_proof(self.redeem_proof.proof.bytes.len())?;
            let report = crate::zk::verify_backend_with_timing_checked(
                self.redeem_proof.backend.as_str(),
                &self.redeem_proof.proof,
                Some(&redeem_vk),
                &state_transaction.zk,
            );
            if !report.ok {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "recursive Kagemusha final redeem proof verification failed",
                )
                .into());
            }
            for nullifier in redeem_nullifiers {
                if !st.nullifiers.insert(nullifier) {
                    let kind = if nullifier == current_note_nullifier {
                        "current note"
                    } else {
                        "top-up anchor"
                    };
                    return Err(labeled_invariant(
                        "duplicate_nullifier",
                        format!("recursive Kagemusha {kind} nullifier is already spent"),
                    )
                    .into());
                }
            }
            if let Some(change_output) = self.change_output {
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                    &mut st,
                    change_output,
                    state_transaction.zk.root_history_cap,
                )?;
                let _frontier_update = st.record_frontier_checkpoint(
                    state_transaction.block_height(),
                    state_transaction.zk.tree_frontier_checkpoint_interval,
                    state_transaction.zk.reorg_depth_bound,
                );
            }
            state_transaction.world.zk_assets.remove(def_id.clone());
            state_transaction.world.zk_assets.insert(def_id.clone(), st);
            let mint = Mint::asset_numeric(
                Numeric::new(self.public_amount, 0),
                AssetId::of(def_id, self.recipient),
            );
            mint.execute(authority, state_transaction)
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{
            collections::{BTreeMap, BTreeSet},
            sync::Arc,
        };

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, StateTransaction, World, ZkAssetState, ZkAssetVerifierBinding},
        };
        use iroha_crypto::{KeyPair, Signature};
        use iroha_data_model::{
            Registrable,
            account::Account,
            asset::{Asset, AssetDefinition, definition::AssetConfidentialPolicy},
            block::BlockHeader,
            domain::{Domain, DomainId},
            offline::{
                KagemushaFoldStep, KagemushaRecursiveAggregationProof,
                KagemushaRecursiveSpendBundleV1, KagemushaRecursiveSpendLineageWitnessV1,
                KagemushaVerifiedFoldBundle, KagemushaVerifiedFoldRecordBundle,
                KagemushaVerifiedFoldStep, KagemushaVerifiedFoldVerifierRecord,
                OfflineNoteAuditBundle,
            },
            permission::Permission,
            proof::ProofAttachment,
        };
        use iroha_primitives::json::Json;
        use iroha_primitives::numeric::NumericSpec;
        use nonzero_ext::nonzero;
        use rcgen::{
            BasicConstraints, CertificateParams, CertifiedIssuer, CustomExtension,
            DistinguishedName, DnType, IsCa, KeyPair as RcgenKeyPair, KeyUsagePurpose,
            PKCS_ECDSA_P256_SHA256, date_time_ymd,
        };

        fn fixture_key_pair(seed: u8) -> KeyPair {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed must derive a valid keypair")
        }

        fn sample_account(seed: u8) -> AccountId {
            let keypair = fixture_key_pair(seed);
            AccountId::new(keypair.public_key().clone())
        }

        fn sample_signature(seed: u8) -> Signature {
            let mut payload = [0u8; 64];
            for (idx, byte) in payload.iter_mut().enumerate() {
                let offset = u8::try_from(idx).expect("index fits into u8");
                *byte = seed.wrapping_add(offset);
            }
            Signature::from_bytes(&payload)
        }

        fn checked_signature(private_key: &iroha_crypto::PrivateKey, payload: &[u8]) -> Signature {
            Signature::try_new(private_key, payload).expect("test fixture signing should succeed")
        }

        #[test]
        fn fixture_key_pair_uses_checked_seed_derivation() {
            assert_eq!(fixture_key_pair(0x01).algorithm(), Algorithm::Ed25519);
            assert!(
                KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
                "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
            );
        }

        #[test]
        fn attestation_der_reader_rejects_non_canonical_length() {
            let mut reader = DerReader::new(&[0x04, 0x81, 0x01, 0x00]);

            let err = reader
                .read_octet_string()
                .expect_err("DER long-form length for a short value must reject");
            assert_offline_rejection(err, "invalid_attestation", "non-canonical");
        }

        #[test]
        fn attestation_der_reader_rejects_non_canonical_high_tag_number() {
            let cases: &[&[u8]] = &[&[0xBF, 0x80, 0x1F, 0x00], &[0xBF, 0x1E, 0x00]];

            for input in cases {
                let mut reader = DerReader::new(input);
                match reader.read_tlv_full() {
                    Ok(_) => panic!("DER must reject non-canonical high-tag numbers"),
                    Err(err) => {
                        assert_offline_rejection(err, "invalid_attestation", "non-canonical")
                    }
                }
            }
        }

        #[test]
        fn attestation_der_integer_rejects_non_canonical_positive_encoding() {
            let err = der_integer_to_i64(&[0x00, 0x7F])
                .expect_err("DER INTEGER must reject unnecessary leading zero");
            assert_offline_rejection(err, "invalid_attestation", "non-canonical");
        }

        #[test]
        fn attestation_der_integer_rejects_non_canonical_negative_encoding() {
            let err = der_integer_to_i64(&[0xFF, 0x80])
                .expect_err("DER INTEGER must reject unnecessary leading 0xff");
            assert_offline_rejection(err, "invalid_attestation", "non-canonical");
        }

        fn test_der_wrap(tag: u8, content: Vec<u8>) -> Vec<u8> {
            let len = u8::try_from(content.len()).expect("test DER content length fits short form");
            let mut der = Vec::with_capacity(content.len() + 2);
            der.push(tag);
            der.push(len);
            der.extend_from_slice(&content);
            der
        }

        fn test_der_octet_string(content: &[u8]) -> Vec<u8> {
            test_der_wrap(0x04, content.to_vec())
        }

        fn test_der_android_package_info(package_name: &str) -> Vec<u8> {
            let mut content = test_der_octet_string(package_name.as_bytes());
            content.extend_from_slice(&[0x02, 0x01, 0x01]);
            test_der_wrap(0x30, content)
        }

        fn test_android_application_id_der(
            package_set: Vec<u8>,
            signature_set: Vec<u8>,
        ) -> Vec<u8> {
            let mut content = package_set;
            content.extend_from_slice(&signature_set);
            test_der_wrap(0x30, content)
        }

        #[test]
        fn android_attestation_application_id_rejects_unsorted_der_sets() {
            let mut packages = test_der_android_package_info("z");
            packages.extend_from_slice(&test_der_android_package_info("a"));
            let package_set = test_der_wrap(0x31, packages);
            let signature_set = test_der_wrap(0x31, test_der_octet_string(&[0xC3; 32]));

            let err = match parse_android_attestation_application_id(
                &test_android_application_id_der(package_set, signature_set),
            ) {
                Ok(_) => panic!("unsorted Android package SET must reject"),
                Err(err) => err,
            };
            assert_offline_rejection(err, "invalid_attestation", "package SET");

            let package_set = test_der_wrap(
                0x31,
                test_der_android_package_info(ANDROID_TEST_PACKAGE_NAME),
            );
            let mut signatures = test_der_octet_string(&[0xFF; 32]);
            signatures.extend_from_slice(&test_der_octet_string(&[0x00; 32]));
            let signature_set = test_der_wrap(0x31, signatures);

            let err = match parse_android_attestation_application_id(
                &test_android_application_id_der(package_set, signature_set),
            ) {
                Ok(_) => panic!("unsorted Android signing-digest SET must reject"),
                Err(err) => err,
            };
            assert_offline_rejection(err, "invalid_attestation", "signing-digest SET");
        }

        fn fixed_bytes(label: &[u8]) -> [u8; 32] {
            Hash::new(label).into()
        }

        fn sample_p256_assertion_public_key() -> Vec<u8> {
            hex::decode(concat!(
                "04",
                "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
                "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            ))
            .expect("sample P-256 base point decodes")
        }

        fn sample_app_attest_cose_key(assertion_public_key: &[u8]) -> Vec<u8> {
            assert_eq!(assertion_public_key.len(), 65);
            assert_eq!(assertion_public_key.first(), Some(&0x04));
            let map = vec![
                (
                    ciborium::value::Value::Integer(1.into()),
                    ciborium::value::Value::Integer(2.into()),
                ),
                (
                    ciborium::value::Value::Integer(3.into()),
                    ciborium::value::Value::Integer((-7).into()),
                ),
                (
                    ciborium::value::Value::Integer((-1).into()),
                    ciborium::value::Value::Integer(1.into()),
                ),
                (
                    ciborium::value::Value::Integer((-2).into()),
                    ciborium::value::Value::Bytes(assertion_public_key[1..33].to_vec()),
                ),
                (
                    ciborium::value::Value::Integer((-3).into()),
                    ciborium::value::Value::Bytes(assertion_public_key[33..65].to_vec()),
                ),
            ];
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&ciborium::value::Value::Map(map), &mut bytes)
                .expect("sample COSE key encodes");
            bytes
        }

        const TRUSTED_TEST_ATTESTATION_ROOT_CN: &str = "Iroha Offline Attestation Test Root";

        fn test_attestation_root_with_common_name(
            common_name: &str,
        ) -> CertifiedIssuer<'static, RcgenKeyPair> {
            let root_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test root key generation");
            let mut root_name = DistinguishedName::new();
            root_name.push(DnType::CommonName, common_name);
            let mut root_params = CertificateParams::default();
            root_params.distinguished_name = root_name;
            root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            root_params.key_usages = vec![
                KeyUsagePurpose::KeyCertSign,
                KeyUsagePurpose::DigitalSignature,
                KeyUsagePurpose::CrlSign,
            ];
            CertifiedIssuer::self_signed(root_params, root_key).expect("test root certificate")
        }

        fn test_attestation_root() -> CertifiedIssuer<'static, RcgenKeyPair> {
            test_attestation_root_with_common_name(TRUSTED_TEST_ATTESTATION_ROOT_CN)
        }

        fn test_attestation_root_der_with_validity(
            common_name: &str,
            not_before_year: i32,
            not_after_year: i32,
        ) -> Vec<u8> {
            let root_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test root key generation");
            let mut root_name = DistinguishedName::new();
            root_name.push(DnType::CommonName, common_name);
            let mut root_params = CertificateParams::default();
            root_params.distinguished_name = root_name;
            root_params.not_before = date_time_ymd(not_before_year, 1, 1);
            root_params.not_after = date_time_ymd(not_after_year, 1, 1);
            root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            root_params.key_usages = vec![
                KeyUsagePurpose::KeyCertSign,
                KeyUsagePurpose::DigitalSignature,
                KeyUsagePurpose::CrlSign,
            ];
            CertifiedIssuer::self_signed(root_params, root_key)
                .expect("test root certificate")
                .der()
                .as_ref()
                .to_vec()
        }

        fn test_leaf_params(
            common_name: &str,
            extensions: Vec<CustomExtension>,
        ) -> CertificateParams {
            let mut leaf_name = DistinguishedName::new();
            leaf_name.push(DnType::CommonName, common_name);
            let mut leaf_params = CertificateParams::default();
            leaf_params.distinguished_name = leaf_name;
            leaf_params.is_ca = IsCa::NoCa;
            leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
            leaf_params.custom_extensions = extensions;
            leaf_params
        }

        fn app_attest_nonce_extension(auth_data: &[u8], challenge_hash: &Hash) -> CustomExtension {
            let nonce = sha256_concat(auth_data, challenge_hash.as_ref());
            let extension_der = yasna::construct_der(|writer| {
                writer.write_sequence(|writer| {
                    writer
                        .next()
                        .write_tagged(yasna::Tag::context(1), |writer| writer.write_bytes(&nonce));
                });
            });
            CustomExtension::from_oid_content(&[1, 2, 840, 113635, 100, 8, 2], extension_der)
        }

        const ANDROID_TEST_PACKAGE_NAME: &str = "jp.co.soramitsu.iroha.offline";
        const ANDROID_TEST_SIGNING_CERT_SHA256: [u8; 32] = [0xC3; 32];

        fn android_attestation_application_id_der_with_entries(
            package_names: &[&str],
            signing_digests: &[[u8; 32]],
        ) -> Vec<u8> {
            yasna::construct_der(|writer| {
                writer.write_sequence(|writer| {
                    writer.next().write_set_of(|writer| {
                        for package_name in package_names {
                            writer.next().write_sequence(|writer| {
                                writer.next().write_bytes(package_name.as_bytes());
                                writer.next().write_i64(1);
                            });
                        }
                    });
                    writer.next().write_set_of(|writer| {
                        for signing_digest in signing_digests {
                            writer.next().write_bytes(signing_digest);
                        }
                    });
                });
            })
        }

        fn android_attestation_application_id_der(
            package_name: &str,
            signing_digest: &[u8; 32],
        ) -> Vec<u8> {
            android_attestation_application_id_der_with_entries(&[package_name], &[*signing_digest])
        }

        fn write_android_keymint_authorization_list(
            writer: &mut yasna::DERWriterSeq<'_>,
            application_id: Option<&[u8]>,
            usage_count_limit: Option<i64>,
            all_applications_count: usize,
        ) {
            for _ in 0..all_applications_count {
                writer
                    .next()
                    .write_tagged(yasna::Tag::context(600), |writer| {
                        writer.write_null();
                    });
            }
            if let Some(application_id) = application_id {
                writer
                    .next()
                    .write_tagged(yasna::Tag::context(709), |writer| {
                        writer.write_bytes(application_id);
                    });
            }
            if let Some(limit) = usage_count_limit {
                writer
                    .next()
                    .write_tagged(yasna::Tag::context(405), |writer| {
                        writer.write_i64(limit);
                    });
            }
        }

        fn android_keymint_extension(
            challenge_hash: &Hash,
            hardware_backed: bool,
            package_name: &str,
            signing_digest: &[u8; 32],
            usage_count_limit: Option<i64>,
            all_applications: bool,
        ) -> CustomExtension {
            let security_level = if hardware_backed { 1 } else { 0 };
            let application_id =
                android_attestation_application_id_der(package_name, signing_digest);
            let extension_der = yasna::construct_der(|writer| {
                writer.write_sequence(|writer| {
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_bytes(challenge_hash.as_ref());
                    writer.next().write_bytes(&[]);
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            Some(&application_id),
                            None,
                            usize::from(all_applications),
                        );
                    });
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            None,
                            usage_count_limit,
                            0,
                        );
                    });
                });
            });
            CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17], extension_der)
        }

        fn android_keymint_extension_with_application_entries(
            challenge_hash: &Hash,
            hardware_backed: bool,
            package_names: &[&str],
            signing_digests: &[[u8; 32]],
            usage_count_limit: Option<i64>,
            all_applications_count: usize,
        ) -> CustomExtension {
            let security_level = if hardware_backed { 1 } else { 0 };
            let application_id =
                android_attestation_application_id_der_with_entries(package_names, signing_digests);
            let extension_der = yasna::construct_der(|writer| {
                writer.write_sequence(|writer| {
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_bytes(challenge_hash.as_ref());
                    writer.next().write_bytes(&[]);
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            Some(&application_id),
                            None,
                            all_applications_count,
                        );
                    });
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            None,
                            usage_count_limit,
                            0,
                        );
                    });
                });
            });
            CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17], extension_der)
        }

        #[allow(clippy::too_many_arguments)]
        fn android_keymint_extension_with_split_authorizations(
            challenge_hash: &Hash,
            software_usage_count_limit: Option<i64>,
            hardware_usage_count_limit: Option<i64>,
            software_application_id: bool,
            hardware_application_id: bool,
            software_all_applications_count: usize,
            hardware_all_applications_count: usize,
            extra_authorization_list_count: usize,
        ) -> CustomExtension {
            let security_level = 1;
            let application_id = android_attestation_application_id_der(
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
            );
            let extension_der = yasna::construct_der(|writer| {
                writer.write_sequence(|writer| {
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_bytes(challenge_hash.as_ref());
                    writer.next().write_bytes(&[]);
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            software_application_id.then_some(application_id.as_slice()),
                            software_usage_count_limit,
                            software_all_applications_count,
                        );
                    });
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            hardware_application_id.then_some(application_id.as_slice()),
                            hardware_usage_count_limit,
                            hardware_all_applications_count,
                        );
                    });
                    for _ in 0..extra_authorization_list_count {
                        writer.next().write_sequence(|_| {});
                    }
                });
            });
            CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17], extension_der)
        }

        fn sample_ios_app_attest_report(
            leaf_key: &RcgenKeyPair,
            key_id: &[u8],
            challenge_hash: &Hash,
            ios_team_id: &str,
            ios_bundle_id: &str,
            ios_environment: &str,
            root_common_name: &str,
        ) -> Vec<u8> {
            sample_ios_app_attest_report_with_nonce_extension_count(
                leaf_key,
                key_id,
                challenge_hash,
                ios_team_id,
                ios_bundle_id,
                ios_environment,
                root_common_name,
                1,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn sample_ios_app_attest_report_with_nonce_extension_count(
            leaf_key: &RcgenKeyPair,
            key_id: &[u8],
            challenge_hash: &Hash,
            ios_team_id: &str,
            ios_bundle_id: &str,
            ios_environment: &str,
            root_common_name: &str,
            nonce_extension_count: usize,
        ) -> Vec<u8> {
            let assertion_public_key = leaf_key.public_key_raw();
            let cose_key = sample_app_attest_cose_key(assertion_public_key);
            let mut auth_data = Vec::new();
            let rp_id = format!("{}.{}", ios_team_id.to_ascii_uppercase(), ios_bundle_id);
            auth_data.extend_from_slice(&sha256_bytes(rp_id.as_bytes()));
            auth_data.push(OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA);
            auth_data.extend_from_slice(&0_u32.to_be_bytes());
            if ios_environment == OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT {
                auth_data.extend_from_slice(OFFLINE_ATTESTATION_IOS_AAGUID_DEVELOPMENT);
            } else {
                auth_data.extend_from_slice(OFFLINE_ATTESTATION_IOS_AAGUID_PRODUCTION);
            }
            auth_data.extend_from_slice(
                &u16::try_from(key_id.len())
                    .expect("credential id length fits")
                    .to_be_bytes(),
            );
            auth_data.extend_from_slice(key_id);
            auth_data.extend_from_slice(&cose_key);
            let root = test_attestation_root_with_common_name(root_common_name);
            let nonce_extensions = (0..nonce_extension_count)
                .map(|_| app_attest_nonce_extension(&auth_data, challenge_hash))
                .collect();
            let leaf = test_leaf_params("Iroha Offline App Attest Test Leaf", nonce_extensions)
                .signed_by(leaf_key, &root)
                .expect("test App Attest leaf certificate");
            let att_stmt = vec![(
                ciborium::value::Value::Text("x5c".to_owned()),
                ciborium::value::Value::Array(vec![
                    ciborium::value::Value::Bytes(leaf.der().as_ref().to_vec()),
                    ciborium::value::Value::Bytes(root.der().as_ref().to_vec()),
                ]),
            )];
            let report = vec![
                (
                    ciborium::value::Value::Text("fmt".to_owned()),
                    ciborium::value::Value::Text("apple-appattest".to_owned()),
                ),
                (
                    ciborium::value::Value::Text("authData".to_owned()),
                    ciborium::value::Value::Bytes(auth_data),
                ),
                (
                    ciborium::value::Value::Text("attStmt".to_owned()),
                    ciborium::value::Value::Map(att_stmt),
                ),
            ];
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&ciborium::value::Value::Map(report), &mut bytes)
                .expect("sample App Attest report encodes");
            bytes
        }

        fn sample_android_keymint_report(
            leaf_key: &RcgenKeyPair,
            challenge_hash: &Hash,
            hardware_backed: bool,
        ) -> Vec<u8> {
            sample_android_keymint_report_with_options(
                leaf_key,
                challenge_hash,
                hardware_backed,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                false,
                true,
            )
        }

        fn sample_android_keymint_report_with_extension(
            leaf_key: &RcgenKeyPair,
            keymint_extension: CustomExtension,
        ) -> Vec<u8> {
            sample_android_keymint_report_with_extensions(leaf_key, vec![keymint_extension])
        }

        fn sample_android_keymint_report_with_extensions(
            leaf_key: &RcgenKeyPair,
            keymint_extensions: Vec<CustomExtension>,
        ) -> Vec<u8> {
            let root = test_attestation_root();
            let leaf = test_leaf_params(
                "Iroha Offline Android KeyMint Test Leaf",
                keymint_extensions,
            )
            .signed_by(leaf_key, &root)
            .expect("test Android KeyMint leaf certificate");
            let report = ciborium::value::Value::Array(vec![
                ciborium::value::Value::Bytes(leaf.der().as_ref().to_vec()),
                ciborium::value::Value::Bytes(root.der().as_ref().to_vec()),
            ]);
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&report, &mut bytes)
                .expect("sample Android KeyMint report encodes");
            bytes
        }

        #[allow(clippy::too_many_arguments)]
        fn sample_android_keymint_report_with_options(
            leaf_key: &RcgenKeyPair,
            challenge_hash: &Hash,
            hardware_backed: bool,
            package_name: &str,
            signing_digest: &[u8; 32],
            usage_count_limit: Option<i64>,
            all_applications: bool,
            include_application_id: bool,
        ) -> Vec<u8> {
            let keymint_extension = if include_application_id {
                android_keymint_extension(
                    challenge_hash,
                    hardware_backed,
                    package_name,
                    signing_digest,
                    usage_count_limit,
                    all_applications,
                )
            } else {
                android_keymint_extension_without_application_id(
                    challenge_hash,
                    hardware_backed,
                    usage_count_limit,
                )
            };
            sample_android_keymint_report_with_extension(leaf_key, keymint_extension)
        }

        fn sample_android_keymint_report_with_application_entries(
            leaf_key: &RcgenKeyPair,
            challenge_hash: &Hash,
            package_names: &[&str],
            signing_digests: &[[u8; 32]],
            all_applications_count: usize,
        ) -> Vec<u8> {
            let keymint_extension = android_keymint_extension_with_application_entries(
                challenge_hash,
                true,
                package_names,
                signing_digests,
                Some(1),
                all_applications_count,
            );
            sample_android_keymint_report_with_extension(leaf_key, keymint_extension)
        }

        fn sample_android_report_without_leaf_extension(leaf_key: &RcgenKeyPair) -> Vec<u8> {
            let root = test_attestation_root();
            let leaf = test_leaf_params("Iroha Offline Android Missing Extension Leaf", Vec::new())
                .signed_by(leaf_key, &root)
                .expect("test Android leaf certificate without extension");
            let report = ciborium::value::Value::Array(vec![
                ciborium::value::Value::Bytes(leaf.der().as_ref().to_vec()),
                ciborium::value::Value::Bytes(root.der().as_ref().to_vec()),
            ]);
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&report, &mut bytes)
                .expect("sample Android missing-extension report encodes");
            bytes
        }

        fn android_keymint_extension_without_application_id(
            challenge_hash: &Hash,
            hardware_backed: bool,
            usage_count_limit: Option<i64>,
        ) -> CustomExtension {
            let security_level = if hardware_backed { 1 } else { 0 };
            let extension_der = yasna::construct_der(|writer| {
                writer.write_sequence(|writer| {
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_i64(400);
                    writer.next().write_enum(security_level);
                    writer.next().write_bytes(challenge_hash.as_ref());
                    writer.next().write_bytes(&[]);
                    writer.next().write_sequence(|_| {});
                    writer.next().write_sequence(|writer| {
                        write_android_keymint_authorization_list(
                            writer,
                            None,
                            usage_count_limit,
                            0,
                        );
                    });
                });
            });
            CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17], extension_der)
        }

        fn refresh_attestation_material(registration: &mut OfflineDeviceAttestationRegistration) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            let key_id = sha256_bytes(&registration.assertion_public_key);
            registration.key_id =
                if registration.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT {
                    registration.android_package_name = Some(ANDROID_TEST_PACKAGE_NAME.to_owned());
                    registration.android_signing_certificate_sha256 =
                        Some(ANDROID_TEST_SIGNING_CERT_SHA256.to_vec());
                    hex::encode(key_id)
                } else {
                    registration.android_package_name = None;
                    registration.android_signing_certificate_sha256 = None;
                    BASE64_STANDARD.encode(key_id)
                };
            refresh_attestation_challenge(registration);
            registration.attestation_report =
                if registration.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT {
                    sample_android_keymint_report(&leaf_key, &registration.challenge_hash, true)
                } else {
                    sample_ios_app_attest_report(
                        &leaf_key,
                        &key_id,
                        &registration.challenge_hash,
                        registration.ios_team_id.as_deref().unwrap_or("TEAMID1234"),
                        registration
                            .ios_bundle_id
                            .as_deref()
                            .unwrap_or("jp.co.soramitsu.iroha.offline"),
                        registration
                            .ios_environment
                            .as_deref()
                            .unwrap_or(OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION),
                        TRUSTED_TEST_ATTESTATION_ROOT_CN,
                    )
                };
            refresh_attestation_hashes(registration);
        }

        fn refresh_ios_attestation_material_with_root_name(
            registration: &mut OfflineDeviceAttestationRegistration,
            root_common_name: &str,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test iOS attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            let key_id = sha256_bytes(&registration.assertion_public_key);
            registration.key_id = BASE64_STANDARD.encode(key_id);
            refresh_attestation_challenge(registration);
            registration.attestation_report = sample_ios_app_attest_report(
                &leaf_key,
                &key_id,
                &registration.challenge_hash,
                registration.ios_team_id.as_deref().unwrap_or("TEAMID1234"),
                registration
                    .ios_bundle_id
                    .as_deref()
                    .unwrap_or("jp.co.soramitsu.iroha.offline"),
                registration
                    .ios_environment
                    .as_deref()
                    .unwrap_or(OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION),
                root_common_name,
            );
            refresh_attestation_hashes(registration);
        }

        fn refresh_ios_attestation_material_with_key_id_override(
            registration: &mut OfflineDeviceAttestationRegistration,
            key_id: &[u8],
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test iOS attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = BASE64_STANDARD.encode(key_id);
            refresh_attestation_challenge(registration);
            registration.attestation_report = sample_ios_app_attest_report(
                &leaf_key,
                key_id,
                &registration.challenge_hash,
                registration.ios_team_id.as_deref().unwrap_or("TEAMID1234"),
                registration
                    .ios_bundle_id
                    .as_deref()
                    .unwrap_or("jp.co.soramitsu.iroha.offline"),
                registration
                    .ios_environment
                    .as_deref()
                    .unwrap_or(OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION),
                TRUSTED_TEST_ATTESTATION_ROOT_CN,
            );
            refresh_attestation_hashes(registration);
        }

        fn refresh_ios_attestation_material_with_noncanonical_key_id(
            registration: &mut OfflineDeviceAttestationRegistration,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test iOS attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            let key_id = sha256_bytes(&registration.assertion_public_key);
            registration.key_id =
                noncanonical_single_padding_base64_alias(&BASE64_STANDARD.encode(key_id));
            refresh_attestation_challenge(registration);
            registration.attestation_report = sample_ios_app_attest_report(
                &leaf_key,
                &key_id,
                &registration.challenge_hash,
                registration.ios_team_id.as_deref().unwrap_or("TEAMID1234"),
                registration
                    .ios_bundle_id
                    .as_deref()
                    .unwrap_or("jp.co.soramitsu.iroha.offline"),
                registration
                    .ios_environment
                    .as_deref()
                    .unwrap_or(OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION),
                TRUSTED_TEST_ATTESTATION_ROOT_CN,
            );
            refresh_attestation_hashes(registration);
        }

        fn noncanonical_single_padding_base64_alias(canonical: &str) -> String {
            const ALPHABET: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut alias = canonical.as_bytes().to_vec();
            assert!(
                alias.ends_with(b"=") && !alias.ends_with(b"=="),
                "expected single-padding base64 text"
            );
            let target = alias
                .len()
                .checked_sub(2)
                .expect("single-padding base64 has a symbol before padding");
            let index = ALPHABET
                .iter()
                .position(|symbol| *symbol == alias[target])
                .expect("canonical base64 symbol is in the standard alphabet");
            alias[target] = ALPHABET[(index & !0b11) | ((index + 1) & 0b11)];
            let alias = String::from_utf8(alias).expect("base64 alphabet is UTF-8");
            assert_ne!(alias, canonical);
            alias
        }

        fn refresh_ios_attestation_material_with_duplicate_nonce_extension(
            registration: &mut OfflineDeviceAttestationRegistration,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test iOS attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            let key_id = sha256_bytes(&registration.assertion_public_key);
            registration.key_id = BASE64_STANDARD.encode(key_id);
            refresh_attestation_challenge(registration);
            registration.attestation_report =
                sample_ios_app_attest_report_with_nonce_extension_count(
                    &leaf_key,
                    &key_id,
                    &registration.challenge_hash,
                    registration.ios_team_id.as_deref().unwrap_or("TEAMID1234"),
                    registration
                        .ios_bundle_id
                        .as_deref()
                        .unwrap_or("jp.co.soramitsu.iroha.offline"),
                    registration
                        .ios_environment
                        .as_deref()
                        .unwrap_or(OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION),
                    TRUSTED_TEST_ATTESTATION_ROOT_CN,
                    2,
                );
            refresh_attestation_hashes(registration);
        }

        fn duplicate_ios_attestation_report_text_key(
            registration: &mut OfflineDeviceAttestationRegistration,
            key: &str,
        ) {
            let value: ciborium::value::Value =
                ciborium::de::from_reader(registration.attestation_report.as_slice())
                    .expect("sample iOS report decodes");
            let ciborium::value::Value::Map(mut map) = value else {
                panic!("sample iOS report must be a CBOR map");
            };
            let duplicate = map
                .iter()
                .find(|(candidate, _)| {
                    matches!(candidate, ciborium::value::Value::Text(text) if text == key)
                })
                .cloned()
                .expect("sample iOS report contains the requested key");
            map.push(duplicate);
            registration.attestation_report.clear();
            ciborium::ser::into_writer(
                &ciborium::value::Value::Map(map),
                &mut registration.attestation_report,
            )
            .expect("duplicated iOS report encodes");
            refresh_attestation_hashes(registration);
        }

        fn refresh_android_attestation_material(
            registration: &mut OfflineDeviceAttestationRegistration,
            hardware_backed: bool,
        ) {
            registration.android_package_name = Some(ANDROID_TEST_PACKAGE_NAME.to_owned());
            registration.android_signing_certificate_sha256 =
                Some(ANDROID_TEST_SIGNING_CERT_SHA256.to_vec());
            refresh_android_attestation_material_with_options(
                registration,
                hardware_backed,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                false,
                true,
            );
        }

        fn refresh_android_attestation_material_with_key_id_override(
            registration: &mut OfflineDeviceAttestationRegistration,
            key_id: String,
        ) {
            registration.android_package_name = Some(ANDROID_TEST_PACKAGE_NAME.to_owned());
            registration.android_signing_certificate_sha256 =
                Some(ANDROID_TEST_SIGNING_CERT_SHA256.to_vec());
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = key_id;
            refresh_attestation_challenge(registration);
            registration.attestation_report = sample_android_keymint_report_with_options(
                &leaf_key,
                &registration.challenge_hash,
                true,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                false,
                true,
            );
            refresh_attestation_hashes(registration);
        }

        fn refresh_android_attestation_material_with_uppercase_key_id(
            registration: &mut OfflineDeviceAttestationRegistration,
        ) {
            registration.android_package_name = Some(ANDROID_TEST_PACKAGE_NAME.to_owned());
            registration.android_signing_certificate_sha256 =
                Some(ANDROID_TEST_SIGNING_CERT_SHA256.to_vec());
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id =
                hex::encode_upper(sha256_bytes(&registration.assertion_public_key));
            refresh_attestation_challenge(registration);
            registration.attestation_report = sample_android_keymint_report_with_options(
                &leaf_key,
                &registration.challenge_hash,
                true,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                false,
                true,
            );
            refresh_attestation_hashes(registration);
        }

        #[allow(clippy::too_many_arguments)]
        fn refresh_android_attestation_material_with_options(
            registration: &mut OfflineDeviceAttestationRegistration,
            hardware_backed: bool,
            report_package_name: &str,
            report_signing_digest: &[u8; 32],
            usage_count_limit: Option<i64>,
            all_applications: bool,
            include_application_id: bool,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
            refresh_attestation_challenge(registration);
            registration.attestation_report = sample_android_keymint_report_with_options(
                &leaf_key,
                &registration.challenge_hash,
                hardware_backed,
                report_package_name,
                report_signing_digest,
                usage_count_limit,
                all_applications,
                include_application_id,
            );
            refresh_attestation_hashes(registration);
        }

        fn refresh_android_attestation_material_with_application_entries(
            registration: &mut OfflineDeviceAttestationRegistration,
            package_names: &[&str],
            signing_digests: &[[u8; 32]],
            all_applications_count: usize,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
            refresh_attestation_challenge(registration);
            registration.attestation_report =
                sample_android_keymint_report_with_application_entries(
                    &leaf_key,
                    &registration.challenge_hash,
                    package_names,
                    signing_digests,
                    all_applications_count,
                );
            refresh_attestation_hashes(registration);
        }

        #[allow(clippy::too_many_arguments)]
        fn refresh_android_attestation_material_with_split_authorizations(
            registration: &mut OfflineDeviceAttestationRegistration,
            software_usage_count_limit: Option<i64>,
            hardware_usage_count_limit: Option<i64>,
            software_application_id: bool,
            hardware_application_id: bool,
            software_all_applications_count: usize,
            hardware_all_applications_count: usize,
            extra_authorization_list_count: usize,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
            refresh_attestation_challenge(registration);
            let keymint_extension = android_keymint_extension_with_split_authorizations(
                &registration.challenge_hash,
                software_usage_count_limit,
                hardware_usage_count_limit,
                software_application_id,
                hardware_application_id,
                software_all_applications_count,
                hardware_all_applications_count,
                extra_authorization_list_count,
            );
            registration.attestation_report =
                sample_android_keymint_report_with_extension(&leaf_key, keymint_extension);
            refresh_attestation_hashes(registration);
        }

        fn refresh_android_attestation_material_without_leaf_extension(
            registration: &mut OfflineDeviceAttestationRegistration,
        ) {
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
            refresh_attestation_challenge(registration);
            registration.attestation_report =
                sample_android_report_without_leaf_extension(&leaf_key);
            refresh_attestation_hashes(registration);
        }

        fn refresh_android_attestation_material_with_duplicate_leaf_extension(
            registration: &mut OfflineDeviceAttestationRegistration,
        ) {
            registration.android_package_name = Some(ANDROID_TEST_PACKAGE_NAME.to_owned());
            registration.android_signing_certificate_sha256 =
                Some(ANDROID_TEST_SIGNING_CERT_SHA256.to_vec());
            let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test Android attestation key generation");
            registration.assertion_public_key = leaf_key.public_key_raw().to_vec();
            registration.key_id = hex::encode(sha256_bytes(&registration.assertion_public_key));
            refresh_attestation_challenge(registration);
            let keymint_extensions = vec![
                android_keymint_extension(
                    &registration.challenge_hash,
                    true,
                    ANDROID_TEST_PACKAGE_NAME,
                    &ANDROID_TEST_SIGNING_CERT_SHA256,
                    Some(1),
                    false,
                ),
                android_keymint_extension(
                    &registration.challenge_hash,
                    true,
                    ANDROID_TEST_PACKAGE_NAME,
                    &ANDROID_TEST_SIGNING_CERT_SHA256,
                    Some(1),
                    false,
                ),
            ];
            registration.attestation_report =
                sample_android_keymint_report_with_extensions(&leaf_key, keymint_extensions);
            refresh_attestation_hashes(registration);
        }

        fn refresh_attestation_hashes(registration: &mut OfflineDeviceAttestationRegistration) {
            registration.attestation_report_hash = Hash::new(&registration.attestation_report);
            let mut evidence = OFFLINE_ATTESTATION_EVIDENCE_PREFIX.to_vec();
            evidence.extend_from_slice(registration.attestation_report_hash.as_ref());
            registration.evidence = evidence;
            registration.evidence_hash = Hash::new(&registration.evidence);
        }

        fn android_report_certificate_der(report: &[u8], index: usize) -> Vec<u8> {
            let value: ciborium::value::Value =
                ciborium::de::from_reader(report).expect("Android report decodes");
            let ciborium::value::Value::Array(certificates) = value else {
                panic!("Android report must be an array");
            };
            let ciborium::value::Value::Bytes(certificate) = &certificates[index] else {
                panic!("Android certificate must be bytes");
            };
            certificate.clone()
        }

        fn ios_report_certificate_der(report: &[u8], index: usize) -> Vec<u8> {
            let value: ciborium::value::Value =
                ciborium::de::from_reader(report).expect("iOS report decodes");
            let ciborium::value::Value::Map(map) = value else {
                panic!("iOS report must be a map");
            };
            let att_stmt = cbor_map_value(&map, "attStmt")
                .expect("attStmt lookup succeeds")
                .expect("attStmt exists");
            let x5c = cbor_array_value(att_stmt, "x5c")
                .expect("x5c lookup succeeds")
                .expect("x5c exists");
            let ciborium::value::Value::Bytes(certificate) = &x5c[index] else {
                panic!("iOS certificate must be bytes");
            };
            certificate.clone()
        }

        fn default_policy_for_tests() -> OfflineDeviceAttestationPolicy {
            default_offline_device_attestation_policy().expect("default policy builds")
        }

        fn store_attestation_policy_for_tests(
            transaction: &mut StateTransaction<'_, '_>,
            policy: OfflineDeviceAttestationPolicy,
        ) {
            validate_offline_attestation_policy(&policy, transaction.block_unix_timestamp_ms())
                .expect("test policy validates");
            let bytes = norito::to_bytes(&policy).expect("test policy encodes");
            transaction.world.smart_contract_state.insert(
                (*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone(),
                bytes,
            );
        }

        const PENDING_PRODUCTION_BACKEND_TAGS: [BackendTag; 15] = [
            BackendTag::Halo2IpaOrchard,
            BackendTag::Groth16Bls12377,
            BackendTag::FcmpPlusPlusCurveTree,
            BackendTag::LatticePcsSis,
            BackendTag::MidenStark,
            BackendTag::AztecPlonkishPrivateKernel,
            BackendTag::PqMaspStarkFri,
            BackendTag::AnonymousPgc,
            BackendTag::VeRange,
            BackendTag::ZkAt,
            BackendTag::RecursiveAnonymousAdmission,
            BackendTag::VegaExistingCredentialZk,
            BackendTag::SilentThresholdAnoncred,
            BackendTag::ZkX509,
            BackendTag::SisWithHints,
        ];

        const PENDING_PRODUCTION_BACKEND_LABELS: [&str; 17] = [
            "halo2/ipa/orchard",
            "groth16/bls12-377",
            "penumbra-masp",
            "monero-fcmp++",
            "jindo-lattice-pcs-zk",
            "stark/fri/miden",
            "aztec-plonkish-private-kernel",
            "stark/fri/pq-masp-stark-fri",
            "anonymous-pgc",
            "verange",
            "zkat",
            "zk-ams-recursive-admission-v0",
            "vega-existing-credential-zk",
            "silent-threshold-anoncred",
            "zk-x509-onchain-identity-v0",
            "sis-with-hints",
            "lattice-anonymous-credentials",
        ];

        fn tamper_open_verify_envelope_inner_proof_byte(bytes: &mut Vec<u8>) {
            let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
                norito::decode_from_bytes(bytes).expect("decode OpenVerifyEnvelope");
            assert!(
                envelope.proof_bytes.len() > 12,
                "fixture proof must carry a non-empty ZK1 PROF payload"
            );
            assert_eq!(&envelope.proof_bytes[..4], b"ZK1\0");
            assert_eq!(&envelope.proof_bytes[4..8], b"PROF");
            let prof_len = u32::from_le_bytes(
                envelope.proof_bytes[8..12]
                    .try_into()
                    .expect("PROF length bytes"),
            );
            assert!(prof_len > 0, "fixture PROF payload must not be empty");
            envelope.proof_bytes[12] ^= 0x01;
            *bytes = norito::to_bytes(&envelope).expect("re-encode tampered OpenVerifyEnvelope");
        }

        #[cfg(feature = "zk-halo2-ipa")]
        struct RecursiveKagemushaRedeemBundleFixture {
            bundle: KagemushaRecursiveSpendBundleV1,
            lineage_witness: KagemushaRecursiveSpendLineageWitnessV1,
            lineage_record_id: VerifyingKeyId,
            lineage_record: VerifyingKeyRecord,
        }

        #[cfg(feature = "zk-halo2-ipa")]
        struct RecursiveKagemushaRedeemTestFixture {
            state: State,
            authority: AccountId,
            recipient: AccountId,
            definition_id: AssetDefinitionId,
            instruction: RedeemKagemushaRecursive,
            lineage_witness: KagemushaRecursiveSpendLineageWitnessV1,
            lineage_record_id: VerifyingKeyId,
            lineage_record: VerifyingKeyRecord,
        }

        fn lineage_record_match_test_witness(
            vk_id: VerifyingKeyId,
            record: VerifyingKeyRecord,
        ) -> iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 {
            let chain_id: iroha_data_model::ChainId =
                "lineage-record-match-chain".parse().expect("chain id");
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let asset = AssetDefinitionId::new(
                domain_id,
                "lineage".parse().expect("asset definition name"),
            );
            let step = iroha_data_model::offline::KagemushaVerifiedFoldStep {
                root_before: fixed_bytes(b"lineage-record-root-before"),
                input_nullifiers: vec![fixed_bytes(b"lineage-record-input")],
                output_commitments: vec![fixed_bytes(b"lineage-record-output")],
                root_after: fixed_bytes(b"lineage-record-root-after"),
                attachment: ProofAttachment::new_ref(
                    crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                    ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xA1]),
                    vk_id.clone(),
                ),
                verifier_key: VerifyingKeyBox::new(
                    crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                    vec![0xA2],
                ),
            };
            iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 {
                record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                    bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                        chain_id,
                        asset,
                        steps: vec![step],
                    },
                    verifier_records: vec![
                        iroha_data_model::offline::KagemushaVerifiedFoldVerifierRecord {
                            id: vk_id,
                            record,
                        },
                    ],
                },
                pallas_open_envelopes_archive: vec![0xA3],
                current_notes: vec![
                    iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                        note_commitment: fixed_bytes(b"lineage-record-output"),
                        spend_nullifier: fixed_bytes(b"lineage-record-current-nullifier"),
                        amount: Numeric::new(7, 0),
                    },
                ],
                previous_recursive_proofs: Vec::new(),
            }
        }

        #[test]
        fn kagemusha_recursive_lineage_records_must_match_registered_records() {
            let vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "kagemusha-lineage-hop-record-match",
            );
            let mut record = VerifyingKeyRecord::new(
                1,
                "lineage-record-match-circuit",
                BackendTag::Halo2IpaPasta,
                "pallas",
                fixed_bytes(b"lineage-record-match-schema"),
                fixed_bytes(b"lineage-record-match-commitment"),
            );
            record.namespace = crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
            record.status = ConfidentialStatus::Active;
            let witness = lineage_record_match_test_witness(vk_id.clone(), record.clone());

            let registered = BTreeMap::from([(vk_id.clone(), record.clone())]);
            ensure_kagemusha_recursive_lineage_verifier_records_match_registered(&witness, |id| {
                registered.get(id)
            })
            .expect("matching registered lineage records are accepted");

            let missing_records = BTreeMap::<VerifyingKeyId, VerifyingKeyRecord>::new();
            let err = ensure_kagemusha_recursive_lineage_verifier_records_match_registered(
                &witness,
                |id| missing_records.get(id),
            )
            .expect_err("missing registered lineage record must reject");
            assert!(err.contains("not registered"), "unexpected error: {err}");

            let mut changed_record = record.clone();
            changed_record.version = 2;
            let changed_records = BTreeMap::from([(vk_id.clone(), changed_record)]);
            let err = ensure_kagemusha_recursive_lineage_verifier_records_match_registered(
                &witness,
                |id| changed_records.get(id),
            )
            .expect_err("stale lineage record snapshot must reject");
            assert!(
                err.contains("does not match the registered record"),
                "unexpected error: {err}"
            );

            let mut missing_witness = witness.clone();
            missing_witness.record_bundle.verifier_records.clear();
            let err = ensure_kagemusha_recursive_lineage_verifier_records_match_registered(
                &missing_witness,
                |id| registered.get(id),
            )
            .expect_err("missing lineage witness record must reject");
            assert!(err.contains("is missing"), "unexpected error: {err}");

            let mut duplicate_witness = witness.clone();
            let duplicate = duplicate_witness.record_bundle.verifier_records[0].clone();
            duplicate_witness
                .record_bundle
                .verifier_records
                .push(duplicate);
            let err = ensure_kagemusha_recursive_lineage_verifier_records_match_registered(
                &duplicate_witness,
                |id| registered.get(id),
            )
            .expect_err("duplicate lineage witness record must reject");
            assert!(err.contains("is duplicated"), "unexpected error: {err}");

            let mut extra_witness = witness;
            let mut extra = extra_witness.record_bundle.verifier_records[0].clone();
            extra.id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "unused-kagemusha-lineage-hop-record",
            );
            extra_witness.record_bundle.verifier_records.push(extra);
            let err = ensure_kagemusha_recursive_lineage_verifier_records_match_registered(
                &extra_witness,
                |id| registered.get(id),
            )
            .expect_err("unreferenced lineage witness record must reject");
            assert!(err.contains("not referenced"), "unexpected error: {err}");
        }

        fn sample_certificate() -> OfflineNoteKeyCertificate {
            let keypair = fixture_key_pair(0xAA);
            let (_algorithm, public_key) = keypair
                .public_key()
                .try_to_bytes()
                .expect("fixture public key must be valid");
            OfflineNoteKeyCertificate {
                version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
                platform: "ios-appattest".to_owned(),
                key_id: "one-use-key".to_owned(),
                device_id: "device-1".to_owned(),
                account_id: sample_account(0x01),
                public_key: public_key.to_vec(),
                assertion_scheme: "apple-appattest-counter-v1".to_owned(),
                assertion_key_algorithm: "app-attest-p256".to_owned(),
                assertion_public_key: sample_p256_assertion_public_key(),
                assertion_usage_count_limit: None,
                one_use: true,
                issuer_signature: sample_signature(0x44),
            }
        }

        fn signed_sample_certificate(
            issuer: &KeyPair,
            account_id: AccountId,
            note_seed: u8,
            key_id: &str,
        ) -> OfflineNoteKeyCertificate {
            let note_key = fixture_key_pair(note_seed);
            let (_algorithm, public_key) = note_key
                .public_key()
                .try_to_bytes()
                .expect("fixture public key must be valid");
            let mut certificate = OfflineNoteKeyCertificate {
                version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
                platform: OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned(),
                key_id: key_id.to_owned(),
                device_id: "offline-unit-device".to_owned(),
                account_id,
                public_key: public_key.to_vec(),
                assertion_scheme: OFFLINE_ATTESTATION_IOS_ASSERTION_SCHEME.to_owned(),
                assertion_key_algorithm: OFFLINE_ATTESTATION_IOS_ASSERTION_ALGORITHM.to_owned(),
                assertion_public_key: sample_p256_assertion_public_key(),
                assertion_usage_count_limit: None,
                one_use: true,
                issuer_signature: checked_signature(issuer.private_key(), b"placeholder"),
            };
            let payload = certificate
                .signing_bytes()
                .expect("certificate signing payload encodes");
            certificate.issuer_signature = checked_signature(issuer.private_key(), &payload);
            certificate
        }

        fn attestation_registration(
            certificate: &OfflineNoteKeyCertificate,
            recent_block_height: u64,
            recent_block_hash: Hash,
        ) -> OfflineDeviceAttestationRegistration {
            let mut registration = OfflineDeviceAttestationRegistration {
                version: 1,
                platform: certificate.platform.clone(),
                key_id: certificate.key_id.clone(),
                device_id: certificate.device_id.clone(),
                account_id: certificate.account_id.clone(),
                asset_definition_id: Some(sample_issued_claim().asset.definition().clone()),
                ios_team_id: Some("TEAMID1234".to_owned()),
                ios_bundle_id: Some("jp.co.soramitsu.iroha.offline".to_owned()),
                ios_environment: Some("production".to_owned()),
                android_package_name: None,
                android_signing_certificate_sha256: None,
                public_key: certificate.public_key.clone(),
                assertion_scheme: certificate.assertion_scheme.clone(),
                assertion_key_algorithm: certificate.assertion_key_algorithm.clone(),
                assertion_public_key: certificate.assertion_public_key.clone(),
                assertion_usage_count_limit: certificate.assertion_usage_count_limit,
                one_use: certificate.one_use,
                challenge_hash: Hash::new(b"offline-attestation-challenge-placeholder"),
                attestation_report_hash: Hash::new(b"offline-attestation-report-placeholder"),
                attestation_report: Vec::new(),
                evidence_hash: Hash::new(b"offline-attestation-evidence-placeholder"),
                evidence: Vec::new(),
                recent_block_height,
                recent_block_hash,
                expires_at_ms: 10_000,
            };
            refresh_attestation_material(&mut registration);
            registration.challenge_hash = registration
                .canonical_challenge_hash()
                .expect("canonical attestation challenge hash");
            registration
        }

        fn refresh_attestation_challenge(registration: &mut OfflineDeviceAttestationRegistration) {
            registration.challenge_hash = registration
                .canonical_challenge_hash()
                .expect("canonical attestation challenge hash");
        }

        fn state_with_attestation_anchor() -> (State, u64, Hash) {
            state_with_attestation_anchor_count(1)
        }

        fn state_with_attestation_anchor_count(committed_height: u64) -> (State, u64, Hash) {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let mut last_hash = None;
            {
                let mut block_hashes = state.block_hashes.block();
                for height in 1..=committed_height {
                    let anchor_header = BlockHeader::new(
                        std::num::NonZeroU64::new(height).expect("height is non-zero"),
                        None,
                        None,
                        None,
                        0,
                        0,
                    );
                    let anchor_hash = anchor_header.hash();
                    block_hashes.push_for_tests(anchor_hash);
                    last_hash = Some(anchor_hash);
                }
                block_hashes.commit_for_tests();
            }
            (
                state,
                committed_height,
                Hash::from(last_hash.expect("at least one anchor")),
            )
        }

        fn grant_attestation_policy_manager(
            transaction: &mut StateTransaction<'_, '_>,
            authority: &AccountId,
        ) {
            transaction.world.account_permissions.insert(
                authority.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_DEVICE_ATTESTATION_POLICY_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
        }

        fn execute_policy_update_for_tests(
            policy: OfflineDeviceAttestationPolicy,
        ) -> Result<(), Error> {
            let (state, _recent_block_height, _recent_block_hash) = state_with_attestation_anchor();
            let authority = sample_account(0x72);
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            grant_attestation_policy_manager(&mut transaction, &authority);
            SetOfflineDeviceAttestationPolicy::new(policy).execute(&authority, &mut transaction)
        }

        fn assert_policy_update_rejected(policy: OfflineDeviceAttestationPolicy, detail: &str) {
            let err = execute_policy_update_for_tests(policy)
                .expect_err("invalid attestation policy must reject");
            assert_offline_rejection(err, "invalid_attestation_policy", detail);
        }

        fn assert_attestation_registration_rejects_preseeded_replay_marker(
            marker_key: impl FnOnce(&OfflineDeviceAttestationRegistration) -> Hash,
        ) {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let replay_key = marker_key(&registration);
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(replay_key, ());

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("preseeded replay marker must reject registration");
            assert_offline_rejection(
                err,
                "duplicate_attestation",
                "certificate or evidence material",
            );
        }

        fn android_attestation_registration(
            recent_block_height: u64,
            recent_block_hash: Hash,
        ) -> OfflineDeviceAttestationRegistration {
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = Some(1);
            refresh_android_attestation_material(&mut registration, true);
            registration
        }

        fn sample_issued_claim() -> OfflineNoteIssuedClaim {
            let account_id = sample_account(0x01);
            let definition_id = AssetDefinitionId::new(
                DomainId::try_new("offline", "universal").expect("domain id"),
                "xor".parse().expect("asset definition name"),
            );
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-note-source-note"),
                key_certificate: sample_certificate(),
                asset: AssetId::new(definition_id, account_id),
                amount: Numeric::new(10, 0),
            };
            OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim")
        }

        fn placeholder_recursive_proof(public_inputs_hash: Hash) -> OfflineNoteRecursiveProof {
            OfflineNoteRecursiveProof {
                verifier_key_id: VerifyingKeyId::new(
                    crate::zk::ZK_BACKEND_HALO2_IPA,
                    crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                ),
                public_inputs_hash,
                proof: ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new()),
            }
        }

        fn sample_audit_bundle_for_issue(
            issue: &iroha_data_model::offline::OfflineNoteIssue,
            output_certificate: OfflineNoteKeyCertificate,
        ) -> OfflineNoteAuditBundle {
            let input_claim = OfflineNoteIssuedClaim::from_issue(issue).expect("issued claim");
            let output_commitment = Hash::new(b"offline-audit-output-note");
            let mut audit = OfflineNoteAuditBundle {
                token_id: Hash::new(b"offline-audit-token"),
                sender_key_certificate: issue.key_certificate.clone(),
                input_nullifiers: vec![Hash::new(b"offline-audit-input-nullifier")],
                input_claims: vec![input_claim],
                output_commitments: vec![output_commitment],
                output_claims: vec![OfflineNoteAuditOutputClaim {
                    note_commitment: output_commitment,
                    key_certificate: output_certificate,
                    asset: issue.asset.clone(),
                    amount: issue.amount.clone(),
                }],
                recursive_proof: placeholder_recursive_proof(Hash::new(
                    b"offline-placeholder-public-inputs",
                )),
            };
            let public_inputs_hash = audit.public_inputs_hash().expect("audit hash");
            audit.recursive_proof = placeholder_recursive_proof(public_inputs_hash);
            audit
        }

        fn sample_redemption_for_issue(
            issue: &iroha_data_model::offline::OfflineNoteIssue,
            recipient: AccountId,
        ) -> iroha_data_model::offline::OfflineNoteRedeem {
            let mut redemption = iroha_data_model::offline::OfflineNoteRedeem {
                source_note_commitment: issue.note_commitment,
                input_nullifiers: vec![Hash::new(b"offline-redeem-input-nullifier")],
                sender_key_certificate: issue.key_certificate.clone(),
                recipient,
                asset: issue.asset.clone(),
                amount: issue.amount.clone(),
                recursive_proof: placeholder_recursive_proof(Hash::new(
                    b"offline-redeem-placeholder-public-inputs",
                )),
            };
            let public_inputs_hash = redemption
                .public_inputs_hash()
                .expect("redemption public-input hash");
            redemption.recursive_proof = placeholder_recursive_proof(public_inputs_hash);
            redemption
        }

        fn self_escrow_test_state(
            balance: Numeric,
        ) -> (State, AssetId, AccountId, AssetDefinitionId) {
            let account_id = sample_account(0x01);
            let domain_id: DomainId = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "xor".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), account_id.clone());
            let domain = Domain::new(domain_id).build(&account_id);
            let account = Account::new(account_id.clone()).build(&account_id);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("xor".to_owned())
                    .build(&account_id);
            let asset = Asset::new(asset_id.clone(), balance);
            let world = World::with_assets([domain], [account], [asset_definition], [asset], []);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), account_id.clone());

            (state, asset_id, account_id, definition_id)
        }

        fn distinct_escrow_test_state(
            balance: Numeric,
            escrow_seed: u8,
        ) -> (State, AssetId, AccountId, AssetDefinitionId) {
            let account_id = sample_account(0x01);
            let escrow_account_id = sample_account(escrow_seed);
            let domain_id: DomainId = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "xor".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), account_id.clone());
            let escrow_asset_id = AssetId::new(definition_id.clone(), escrow_account_id.clone());
            let domain = Domain::new(domain_id).build(&account_id);
            let account = Account::new(account_id.clone()).build(&account_id);
            let escrow_account = Account::new(escrow_account_id.clone()).build(&account_id);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("xor".to_owned())
                    .build(&account_id);
            let asset = Asset::new(asset_id.clone(), balance);
            let escrow_asset = Asset::new(escrow_asset_id, Numeric::zero());
            let world = World::with_assets(
                [domain],
                [account, escrow_account],
                [asset_definition],
                [asset, escrow_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow_account_id);

            (state, asset_id, account_id, definition_id)
        }

        fn scoped_restricted_escrow_test_state(
            balance: Numeric,
            escrow_balance: Numeric,
            balance_dataspace: DataSpaceId,
            escrow_seed: u8,
        ) -> (
            State,
            AssetId,
            AssetId,
            AccountId,
            AccountId,
            AssetDefinitionId,
        ) {
            let account_id = sample_account(0x01);
            let escrow_account_id = sample_account(escrow_seed);
            let domain_id: DomainId = DomainId::try_new("offline", "cbsi").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "sbd".parse().expect("asset definition name"),
            );
            let scope = AssetBalanceScope::Dataspace(balance_dataspace);
            let asset_id =
                AssetId::with_scope(definition_id.clone(), account_id.clone(), scope.clone());
            let escrow_asset_id =
                AssetId::with_scope(definition_id.clone(), escrow_account_id.clone(), scope);
            let domain = Domain::new(domain_id).build(&account_id);
            let account = Account::new(account_id.clone()).build(&account_id);
            let escrow_account = Account::new(escrow_account_id.clone()).build(&account_id);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("sbd".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&account_id);
            let asset = Asset::new(asset_id.clone(), balance);
            let escrow_asset = Asset::new(escrow_asset_id.clone(), escrow_balance);
            let world = World::with_assets(
                [domain],
                [account, escrow_account],
                [asset_definition],
                [asset, escrow_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow_account_id.clone());

            (
                state,
                asset_id,
                escrow_asset_id,
                account_id,
                escrow_account_id,
                definition_id,
            )
        }

        fn offline_note_verifier_test_state(
            status: ConfidentialStatus,
        ) -> (State, OfflineNoteRecursiveProof, Hash) {
            let verifier_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            );
            #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
            let mut record =
                crate::zk::offline_note_recursive_vk_record(OFFLINE_NOTE_VERIFIER_NAMESPACE, 1)
                    .expect("offline recursive verifier record");
            #[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
            let mut record = {
                let vk_box = VerifyingKeyBox::new(
                    crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                    b"offline-note-test-verifying-key".to_vec(),
                );
                let commitment = crate::zk::hash_vk(&vk_box);
                let mut record = VerifyingKeyRecord::new_with_owner(
                    1,
                    crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                    None,
                    OFFLINE_NOTE_VERIFIER_NAMESPACE,
                    BackendTag::Halo2IpaPasta,
                    "pasta",
                    offline_note_recursive_public_inputs_schema_hash(),
                    commitment,
                );
                record.key = Some(vk_box);
                record.max_proof_bytes = 4096;
                record.vk_len = b"offline-note-test-verifying-key".len() as u32;
                record
            };
            record.status = status;
            let commitment = record.commitment;

            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                commitment,
                OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA.to_vec(),
                b"offline-note-test-proof".to_vec(),
            );
            let proof_bytes = norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope");
            let public_inputs_hash = Hash::new(b"offline-note-public-inputs");

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), Arc::clone(&kura), query);
            state
                .world
                .verifying_keys
                .insert(verifier_id.clone(), record);
            state.world.verifying_keys_by_circuit.insert(
                (crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID.to_owned(), 1),
                verifier_id.clone(),
            );

            let proof = OfflineNoteRecursiveProof {
                verifier_key_id: verifier_id,
                public_inputs_hash: public_inputs_hash.clone(),
                proof: ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), proof_bytes),
            };

            (state, proof, public_inputs_hash)
        }

        fn mutate_offline_note_recursive_envelope(
            proof: &mut OfflineNoteRecursiveProof,
            mutate: impl FnOnce(&mut OpenVerifyEnvelope),
        ) {
            let mut envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&proof.proof.bytes)
                .expect("offline recursive proof should be an OpenVerifyEnvelope");
            mutate(&mut envelope);
            proof.proof.bytes =
                norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
        }

        fn mutate_verifier_record(
            transaction: &mut StateTransaction<'_, '_>,
            verifier_id: &VerifyingKeyId,
            mutate: impl FnOnce(&mut VerifyingKeyRecord),
        ) {
            let mut record = transaction
                .world
                .verifying_keys
                .get(verifier_id)
                .expect("verifier record")
                .clone();
            mutate(&mut record);
            transaction
                .world
                .verifying_keys
                .insert(verifier_id.clone(), record);
        }

        fn assert_offline_note_record_mutation_rejects(
            mutate: impl for<'block, 'state> FnOnce(
                &mut StateTransaction<'block, 'state>,
                &VerifyingKeyId,
                &OfflineNoteRecursiveProof,
            ),
            label: &str,
            detail: &str,
        ) {
            let (state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            let verifier_id = proof.verifier_key_id.clone();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation_transaction = block.transaction();
                mutate(&mut mutation_transaction, &verifier_id, &proof);
                mutation_transaction.apply();
            }
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("mutated offline recursive verifier metadata must reject");
            assert_offline_rejection(err, label, detail);
        }

        fn sample_kagemusha_transfer(backend: &str) -> KagemushaTransfer {
            KagemushaTransfer::new(
                sample_issued_claim().asset.definition().clone(),
                vec![[0x11; 32]],
                vec![[0x22; 32]],
                ProofAttachment::new_ref(
                    backend.into(),
                    ProofBox::new(backend.into(), vec![0xCA, 0xFE]),
                    VerifyingKeyId::new(backend, "offline-kagemusha-transfer"),
                ),
                Some([0x33; 32]),
            )
        }

        fn real_kagemusha_test_state() -> (
            State,
            AccountId,
            AssetDefinitionId,
            KagemushaTransfer,
            Vec<[u8; 32]>,
            Vec<[u8; 32]>,
        ) {
            let authority = sample_account(0x46);
            let chain_id: iroha_data_model::ChainId =
                "kagemusha-transfer-chain".parse().expect("chain id");
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "kgm".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), authority.clone());
            let domain = Domain::new(domain_id).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("kgm".to_owned())
                    .confidential_policy(AssetConfidentialPolicy::convertible())
                    .build(&authority);
            let asset = Asset::new(asset_id, Numeric::zero());

            let mut vk_record = crate::zk::confidential_v2::confidential_transfer_v2_vk_record(
                crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                1,
            )
            .expect("confidential transfer v2 verifier record");
            let vk_box = vk_record
                .key
                .clone()
                .expect("confidential transfer v2 verifier key");
            let vk_commitment = vk_record.commitment;
            let vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "offline-kagemusha-confidential-transfer-v2",
            );
            vk_record.status = ConfidentialStatus::Active;

            let spend_key = [0x11_u8; 32];
            let input_rho = [0x21_u8; 32];
            let input_diversifier =
                crate::zk::confidential_v2::derive_confidential_diversifier_v2(b"kagemusha-input");
            let input_owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &spend_key,
                    input_diversifier,
                )
                .expect("input owner tag");
            let input_commitment = crate::zk::confidential_v2::derive_confidential_note_v2(
                &definition_id.to_string(),
                7,
                input_rho,
                input_owner_tag,
            )
            .expect("input commitment");
            let initial_commitments = vec![input_commitment];
            let root_hint =
                crate::zk::confidential_v2::compute_confidential_root_v2(&initial_commitments)
                    .expect("initial confidential root");

            let output_rho = [0x31_u8; 32];
            let recipient_diversifier =
                crate::zk::confidential_v2::derive_confidential_diversifier_v2(
                    b"kagemusha-recipient",
                );
            let output_owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &[0x41_u8; 32],
                    recipient_diversifier,
                )
                .expect("output owner tag");
            let proof = crate::zk::confidential_v2::build_confidential_transfer_proof_v2(
                &chain_id,
                &definition_id.to_string(),
                &spend_key,
                &initial_commitments,
                &[crate::zk::confidential_v2::ConfidentialTransferInputV2 {
                    amount: 7,
                    rho: input_rho,
                    diversifier: input_diversifier,
                    leaf_index: 0,
                }],
                &[crate::zk::confidential_v2::ConfidentialTransferOutputV2 {
                    amount: 7,
                    rho: output_rho,
                    owner_tag: output_owner_tag,
                }],
                root_hint,
                &vk_record.circuit_id,
                &vk_box,
            )
            .expect("real confidential transfer v2 proof");

            let mut expected_commitments = initial_commitments.clone();
            expected_commitments.extend(proof.output_commitments.iter().copied());
            let expected_final_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&expected_commitments)
                    .expect("final confidential root");

            let mut world =
                World::with_assets([domain], [account], [asset_definition], [asset], []);
            world.verifying_keys_by_circuit.insert(
                (vk_record.circuit_id.clone(), vk_record.version),
                vk_id.clone(),
            );
            world.verifying_keys.insert(vk_id.clone(), vk_record);
            world.zk_assets.insert(definition_id.clone(), {
                let mut zk_state = ZkAssetState::default();
                zk_state.commitments = initial_commitments;
                zk_state.root_history = vec![root_hint];
                zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                    id: vk_id.clone(),
                    commitment: vk_commitment,
                });
                zk_state
            });

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id);
            assert!(
                state.settlement.offline.kagemusha_enabled,
                "Kagemusha must remain enabled by default"
            );
            let mut zk = state.zk.clone();
            zk.halo2.enabled = true;
            zk.halo2.max_envelope_bytes = usize::MAX;
            zk.halo2.max_proof_bytes = usize::MAX;
            state.set_zk(zk);

            let mut proof_attachment = ProofAttachment::new_ref(
                crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                proof.proof,
                vk_id,
            );
            proof_attachment.vk_commitment = Some(vk_commitment);
            proof_attachment.envelope_hash = Some(Hash::new(&proof_attachment.proof.bytes).into());
            let transfer = KagemushaTransfer::new(
                definition_id.clone(),
                proof.nullifiers.clone(),
                proof.output_commitments.clone(),
                proof_attachment,
                Some(root_hint),
            );

            (
                state,
                authority,
                definition_id,
                transfer,
                expected_commitments,
                vec![expected_final_root],
            )
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn recursive_redeem_lineage_pallas_open_envelopes_archive(hop_count: usize) -> Vec<u8> {
            let envelopes = (0..hop_count)
                .map(|hop_index| {
                    let label =
                        0x80_u8.wrapping_add(u8::try_from(hop_index).expect("hop index fits u8"));
                    iroha_zkp_halo2::OpenVerifyEnvelope {
                        params: iroha_zkp_halo2::IpaParams {
                            version: 1,
                            curve_id: 1,
                            n: 2,
                            g: vec![[label; Hash::LENGTH], [label.wrapping_add(1); Hash::LENGTH]],
                            h: vec![
                                [label.wrapping_add(2); Hash::LENGTH],
                                [label.wrapping_add(3); Hash::LENGTH],
                            ],
                            u: [label.wrapping_add(4); Hash::LENGTH],
                        },
                        public: iroha_zkp_halo2::PolyOpenPublic {
                            version: 1,
                            curve_id: 1,
                            n: 2,
                            z: [label.wrapping_add(5); Hash::LENGTH],
                            t: [label.wrapping_add(6); Hash::LENGTH],
                            p_g: [label.wrapping_add(7); Hash::LENGTH],
                        },
                        proof: iroha_zkp_halo2::IpaProofData {
                            version: 1,
                            l: vec![[label.wrapping_add(8); Hash::LENGTH]],
                            r: vec![[label.wrapping_add(9); Hash::LENGTH]],
                            a_final: [label.wrapping_add(10); Hash::LENGTH],
                            b_final: [label.wrapping_add(11); Hash::LENGTH],
                        },
                        transcript_label: format!(
                            "recursive-redeem-lineage-open-envelope-{hop_index}"
                        ),
                        vk_commitment: Some([label.wrapping_add(12); Hash::LENGTH]),
                        public_inputs_schema_hash: Some([label.wrapping_add(13); Hash::LENGTH]),
                        domain_tag: Some([label.wrapping_add(14); Hash::LENGTH]),
                    }
                })
                .collect::<Vec<_>>();
            norito::to_bytes(&envelopes)
                .expect("encode recursive Kagemusha lineage Pallas envelopes")
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn recursive_redeem_real_pallas_open_envelope(
            label: &str,
            metadata: iroha_zkp_halo2::PolyOpenTranscriptMetadata,
        ) -> iroha_zkp_halo2::OpenVerifyEnvelope {
            let n = 4usize;
            let params = iroha_zkp_halo2::pallas::Params::new(n).expect("Pallas params");
            let coeffs = (0..n)
                .map(|index| iroha_zkp_halo2::pallas::Scalar::from((index + 1) as u64))
                .collect::<Vec<_>>();
            let poly = iroha_zkp_halo2::pallas::Polynomial::from_coeffs(coeffs);
            let commitment = poly.commit(&params).expect("Pallas commitment");
            let z = iroha_zkp_halo2::pallas::Scalar::from(5u64);
            let mut transcript = iroha_zkp_halo2::Transcript::new(label);
            let (proof, t) = poly
                .open_with_metadata(&params, &mut transcript, z, commitment, metadata)
                .expect("Pallas opening proof");
            iroha_zkp_halo2::OpenVerifyEnvelope {
                params: iroha_zkp_halo2::norito_helpers::params_to_wire(&params),
                public: iroha_zkp_halo2::norito_helpers::poly_open_public::<
                    iroha_zkp_halo2::pallas::PallasBackend,
                >(params.n(), z, t, commitment),
                proof: iroha_zkp_halo2::norito_helpers::proof_to_wire(&proof),
                transcript_label: label.to_owned(),
                vk_commitment: metadata.vk_commitment,
                public_inputs_schema_hash: metadata.public_inputs_schema_hash,
                domain_tag: metadata.domain_tag,
            }
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn recursive_redeem_real_lineage_pallas_open_envelopes_archive(
            chain_id: &iroha_data_model::ChainId,
            asset: &AssetDefinitionId,
            steps: &[KagemushaVerifiedFoldStep],
        ) -> Vec<u8> {
            let envelopes = steps
                .iter()
                .enumerate()
                .map(|(stored_hop_index, step)| {
                    let metadata =
                        crate::zk::kagemusha_pallas_open_envelope_metadata_for_verified_hop(
                            chain_id, asset, 0, step,
                        )
                        .expect("record-backed one-hop Pallas metadata");
                    recursive_redeem_real_pallas_open_envelope(
                        &format!("recursive-redeem-real-lineage-open-envelope-{stored_hop_index}"),
                        metadata,
                    )
                })
                .collect::<Vec<_>>();
            norito::to_bytes(&envelopes)
                .expect("encode real recursive Kagemusha lineage Pallas envelopes")
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[allow(clippy::too_many_arguments)]
        fn recursive_redeem_real_lineage_transfer_step(
            chain_id: &iroha_data_model::ChainId,
            asset: &AssetDefinitionId,
            vk_record: &VerifyingKeyRecord,
            vk_box: &VerifyingKeyBox,
            vk_id: &VerifyingKeyId,
            tree_commitments: &[[u8; 32]],
            spend_key: [u8; 32],
            amount: u128,
            input_rho: [u8; 32],
            input_diversifier: [u8; 32],
            leaf_index: usize,
            output_spend_key: [u8; 32],
            output_rho: [u8; 32],
            output_diversifier: [u8; 32],
        ) -> (
            KagemushaVerifiedFoldStep,
            iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1,
        ) {
            let root_before =
                crate::zk::confidential_v2::compute_confidential_root_v2(tree_commitments)
                    .expect("real lineage root before");
            let output_owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &output_spend_key,
                    output_diversifier,
                )
                .expect("real lineage output owner tag");
            let proof = crate::zk::confidential_v2::build_confidential_transfer_proof_v2(
                chain_id,
                &asset.to_string(),
                &spend_key,
                tree_commitments,
                &[crate::zk::confidential_v2::ConfidentialTransferInputV2 {
                    amount,
                    rho: input_rho,
                    diversifier: input_diversifier,
                    leaf_index,
                }],
                &[crate::zk::confidential_v2::ConfidentialTransferOutputV2 {
                    amount,
                    rho: output_rho,
                    owner_tag: output_owner_tag,
                }],
                root_before,
                &vk_record.circuit_id,
                vk_box,
            )
            .expect("real lineage confidential transfer v2 proof");
            let mut next_tree = tree_commitments.to_vec();
            next_tree.extend(proof.output_commitments.iter().copied());
            let root_after = crate::zk::confidential_v2::compute_confidential_root_v2(&next_tree)
                .expect("real lineage root after");
            let vk_commitment = crate::zk::hash_vk(vk_box);
            let mut attachment = ProofAttachment::new_ref(
                crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                proof.proof,
                vk_id.clone(),
            );
            attachment.vk_commitment = Some(vk_commitment);
            attachment.envelope_hash = Some(Hash::new(&attachment.proof.bytes).into());
            let spend_nullifier = crate::zk::confidential_v2::derive_confidential_nullifier_v2(
                chain_id.as_str(),
                &asset.to_string(),
                &output_spend_key,
                output_rho,
            );
            let current_note = iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: proof.output_commitments[0],
                spend_nullifier,
                amount: Numeric::new(amount, 0),
            };
            (
                KagemushaVerifiedFoldStep {
                    root_before,
                    input_nullifiers: proof.nullifiers,
                    output_commitments: proof.output_commitments,
                    root_after,
                    attachment,
                    verifier_key: vk_box.clone(),
                },
                current_note,
            )
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn recursive_spend_bundle_fixture_for_note(
            chain_id: &iroha_data_model::ChainId,
            asset: &AssetDefinitionId,
            initial_root: [u8; 32],
            final_root: [u8; 32],
            note_commitment: [u8; 32],
            spend_nullifier: [u8; 32],
            amount: u128,
        ) -> RecursiveKagemushaRedeemBundleFixture {
            let mut previous =
                None::<iroha_data_model::offline::KagemushaRecursiveSpendAccumulatorV1>;
            let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;
            let lineage_record_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "kagemusha-recursive-redeem-lineage-hop",
            );
            let lineage_verifier_key = VerifyingKeyBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                b"recursive-redeem-lineage-hop-vk".to_vec(),
            );
            let lineage_vk_commitment = crate::zk::hash_vk(&lineage_verifier_key);
            let lineage_vk_poseidon_digest =
                iroha_data_model::offline::kagemusha_verifier_key_poseidon_digest(
                    lineage_verifier_key.backend.as_str(),
                    &lineage_verifier_key.bytes,
                )
                .expect("recursive redeem lineage verifier-key digest");
            let mut lineage_record = VerifyingKeyRecord::new(
                1,
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                BackendTag::Halo2IpaPasta,
                "pallas",
                Hash::new(
                    crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1,
                )
                .into(),
                lineage_vk_commitment,
            );
            lineage_record.namespace = crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
            lineage_record.status = ConfidentialStatus::Active;
            lineage_record.max_proof_bytes = 4096;
            lineage_record.vk_len = u32::try_from(lineage_verifier_key.bytes.len())
                .expect("recursive redeem lineage verifier-key length fits u32");
            lineage_record.key = Some(lineage_verifier_key.clone());

            let mut lineage_steps = Vec::new();
            let mut current_notes = Vec::new();
            let mut previous_recursive_proofs = Vec::new();
            let roots = [
                initial_root,
                fixed_bytes(b"recursive-redeem-root-1"),
                fixed_bytes(b"recursive-redeem-root-2"),
                final_root,
            ];
            for hop_index in 0..3usize {
                let proof_label = format!("recursive-redeem-hop-{hop_index}");
                let mut step = KagemushaFoldStep {
                    root_before: roots[hop_index],
                    input_nullifiers: vec![
                        [0x21_u8.wrapping_add(u8::try_from(hop_index).expect("hop fits")); 32],
                        [0x31_u8.wrapping_add(u8::try_from(hop_index).expect("hop fits")); 32],
                    ],
                    output_commitments: vec![
                        [0x41_u8.wrapping_add(u8::try_from(hop_index).expect("hop fits")); 32],
                        [0x51_u8.wrapping_add(u8::try_from(hop_index).expect("hop fits")); 32],
                    ],
                    root_after: roots[hop_index + 1],
                    proof_hash: Hash::new(proof_label.as_bytes()),
                    proof_public_inputs_digest: fixed_bytes(
                        format!("{proof_label}:public").as_bytes(),
                    ),
                    verifier_key_id: lineage_record_id.clone(),
                    verifier_key_commitment: lineage_vk_commitment,
                    verifier_key_poseidon_digest: lineage_vk_poseidon_digest,
                };
                if let Some(previous) = previous.as_ref() {
                    step.input_nullifiers = vec![previous.current_note.spend_nullifier];
                }
                let current_note = iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                    note_commitment: if hop_index == 2 {
                        note_commitment
                    } else {
                        step.output_commitments[0]
                    },
                    spend_nullifier: if hop_index == 2 {
                        spend_nullifier
                    } else {
                        fixed_bytes(format!("recursive-redeem-nullifier-{hop_index}").as_bytes())
                    },
                    amount: Numeric::new(amount, 0),
                };
                step.output_commitments[0] = current_note.note_commitment;
                let mut attachment = ProofAttachment::new_ref(
                    crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                    ProofBox::new(
                        crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                        proof_label.into_bytes(),
                    ),
                    lineage_record_id.clone(),
                );
                attachment.vk_commitment = Some(lineage_vk_commitment);
                lineage_steps.push(KagemushaVerifiedFoldStep {
                    root_before: step.root_before,
                    input_nullifiers: step.input_nullifiers.clone(),
                    output_commitments: step.output_commitments.clone(),
                    root_after: step.root_after,
                    attachment,
                    verifier_key: lineage_verifier_key.clone(),
                });
                current_notes.push(current_note.clone());
                let evidence =
                    iroha_data_model::offline::kagemusha_recursive_aggregation_evidence_from_steps(
                        chain_id,
                        asset,
                        &[step],
                        4,
                        fixed_bytes(b"recursive-redeem-pallas-params"),
                        crate::zk::kagemusha_recursive_fixed_window_table_schedule_digest(4)
                            .expect("canonical recursive schedule digest"),
                        crate::zk::kagemusha_recursive_fixed_window_shared_table_manifest_digest(4)
                            .expect("canonical recursive shared-table manifest digest"),
                        fixed_bytes(b"recursive-redeem-table-bases"),
                        fixed_bytes(format!("recursive-redeem-batch-{hop_index}").as_bytes()),
                    )
                    .expect("recursive redeem evidence");
                let accumulator = match previous.as_ref() {
                    Some(previous) => {
                        iroha_data_model::offline::kagemusha_recursive_spend_accumulator_append_evidence(
                            previous,
                            previous_proof
                                .as_ref()
                                .expect("previous recursive redeem proof"),
                            &evidence,
                            &current_note,
                        )
                        .expect("append recursive redeem accumulator")
                    }
                    None => {
                        iroha_data_model::offline::kagemusha_recursive_spend_accumulator_from_initial_evidence(
                            &evidence,
                            &current_note,
                        )
                        .expect("initial recursive redeem accumulator")
                    }
                };
                let public_inputs = accumulator
                    .recursive_public_inputs()
                    .expect("recursive redeem public inputs");
                let public_inputs_hash = public_inputs
                    .public_inputs_hash()
                    .expect("recursive redeem public-input hash");
                let recursive_proof = KagemushaRecursiveAggregationProof {
                    verifier_key_id: VerifyingKeyId::new(
                        crate::zk::ZK_BACKEND_HALO2_IPA,
                        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                    ),
                    public_inputs,
                    public_inputs_hash,
                    proof: ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.into(), vec![0xC7; 96]),
                };
                if hop_index < 2 {
                    previous_recursive_proofs.push(recursive_proof.clone());
                }
                previous_proof = Some(recursive_proof);
                previous = Some(accumulator);
            }
            let vk_box = crate::zk::kagemusha_recursive_aggregation_proof_vk_box()
                .expect("recursive aggregation vk");
            let bundle = crate::zk::prove_kagemusha_recursive_spend_accumulator(
                crate::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
                &vk_box,
                previous.expect("three-hop recursive redeem accumulator"),
                None,
            )
            .expect("recursive spend proof");
            let lineage_witness = KagemushaRecursiveSpendLineageWitnessV1 {
                record_bundle: KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id: chain_id.clone(),
                        asset: asset.clone(),
                        steps: lineage_steps,
                    },
                    verifier_records: vec![KagemushaVerifiedFoldVerifierRecord {
                        id: lineage_record_id.clone(),
                        record: lineage_record.clone(),
                    }],
                },
                pallas_open_envelopes_archive:
                    recursive_redeem_lineage_pallas_open_envelopes_archive(current_notes.len()),
                current_notes,
                previous_recursive_proofs,
            };
            RecursiveKagemushaRedeemBundleFixture {
                bundle,
                lineage_witness,
                lineage_record_id,
                lineage_record,
            }
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn real_recursive_kagemusha_redeem_record_backed_multi_hop_fixture()
        -> RecursiveKagemushaRedeemTestFixture {
            let authority = sample_account(0x66);
            let recipient = sample_account(0x67);
            let chain_id: iroha_data_model::ChainId = "kagemusha-recursive-redeem-real-lineage"
                .parse()
                .expect("chain id");
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "kgmrl".parse().expect("asset definition name"),
            );
            let recipient_asset_id = AssetId::new(definition_id.clone(), recipient.clone());
            let domain = Domain::new(domain_id).build(&authority);
            let authority_account = Account::new(authority.clone()).build(&authority);
            let recipient_account = Account::new(recipient.clone()).build(&authority);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("kgmrl".to_owned())
                    .confidential_policy(AssetConfidentialPolicy::convertible())
                    .build(&authority);
            let recipient_asset = Asset::new(recipient_asset_id, Numeric::zero());

            let mut lineage_record =
                crate::zk::confidential_v2::confidential_transfer_v2_vk_record(
                    crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                    3,
                )
                .expect("real lineage transfer verifier record");
            lineage_record.status = ConfidentialStatus::Active;
            let lineage_record_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "recursive-kagemusha-real-lineage-transfer-v2",
            );
            let lineage_vk_box = lineage_record
                .key
                .clone()
                .expect("real lineage inline transfer verifier key");

            let amount = 42;
            let initial_spend_key = [0x10_u8; 32];
            let initial_rho = [0x20_u8; 32];
            let initial_diversifier =
                crate::zk::confidential_v2::derive_confidential_diversifier_v2(
                    b"recursive-real-lineage-hop-0-input",
                );
            let initial_owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &initial_spend_key,
                    initial_diversifier,
                )
                .expect("real lineage initial owner tag");
            let initial_commitment = crate::zk::confidential_v2::derive_confidential_note_v2(
                &definition_id.to_string(),
                amount,
                initial_rho,
                initial_owner_tag,
            )
            .expect("real lineage initial commitment");
            let mut tree_commitments = vec![initial_commitment];
            let initial_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&tree_commitments)
                    .expect("real lineage initial root");

            let hop1_spend_key = [0x30_u8; 32];
            let hop1_rho = [0x31_u8; 32];
            let hop1_diversifier = crate::zk::confidential_v2::derive_confidential_diversifier_v2(
                b"recursive-real-lineage-hop-0-output",
            );
            let (hop0, note0) = recursive_redeem_real_lineage_transfer_step(
                &chain_id,
                &definition_id,
                &lineage_record,
                &lineage_vk_box,
                &lineage_record_id,
                &tree_commitments,
                initial_spend_key,
                amount,
                initial_rho,
                initial_diversifier,
                0,
                hop1_spend_key,
                hop1_rho,
                hop1_diversifier,
            );
            tree_commitments.extend(hop0.output_commitments.iter().copied());

            let final_spend_key = [0x50_u8; 32];
            let final_rho = [0x51_u8; 32];
            let final_diversifier = crate::zk::confidential_v2::derive_confidential_diversifier_v2(
                b"recursive-real-lineage-hop-1-output",
            );
            let (hop1, final_note) = recursive_redeem_real_lineage_transfer_step(
                &chain_id,
                &definition_id,
                &lineage_record,
                &lineage_vk_box,
                &lineage_record_id,
                &tree_commitments,
                hop1_spend_key,
                amount,
                hop1_rho,
                hop1_diversifier,
                1,
                final_spend_key,
                final_rho,
                final_diversifier,
            );
            tree_commitments.extend(hop1.output_commitments.iter().copied());
            let final_leaf_index = tree_commitments
                .len()
                .checked_sub(1)
                .expect("real lineage final note exists");
            let final_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&tree_commitments)
                    .expect("real lineage final root");
            assert_eq!(hop1.root_after, final_root);

            let lineage_steps = vec![hop0, hop1];
            let current_notes = vec![note0, final_note.clone()];
            let pallas_open_envelopes_archive =
                recursive_redeem_real_lineage_pallas_open_envelopes_archive(
                    &chain_id,
                    &definition_id,
                    &lineage_steps,
                );
            let pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(&pallas_open_envelopes_archive)
                    .expect("decode real lineage Pallas opening archive");
            let recursive_vk_box = crate::zk::kagemusha_recursive_aggregation_proof_vk_box()
                .expect("recursive aggregation vk");
            let mut previous_bundle = None::<KagemushaRecursiveSpendBundleV1>;
            let mut previous_recursive_proofs = Vec::new();
            for (hop_index, ((step, current_note), envelope)) in lineage_steps
                .iter()
                .zip(current_notes.iter())
                .zip(pallas_open_envelopes.iter())
                .enumerate()
            {
                let one_hop_record_bundle = KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id: chain_id.clone(),
                        asset: definition_id.clone(),
                        steps: vec![step.clone()],
                    },
                    verifier_records: vec![KagemushaVerifiedFoldVerifierRecord {
                        id: lineage_record_id.clone(),
                        record: lineage_record.clone(),
                    }],
                };
                let evidence =
                    crate::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes(
                        &one_hop_record_bundle,
                        std::slice::from_ref(envelope),
                    )
                    .expect("real lineage recursive aggregation evidence");
                let accumulator = if let Some(previous_bundle) = previous_bundle.as_ref() {
                    iroha_data_model::offline::kagemusha_recursive_spend_accumulator_append_evidence(
                        &previous_bundle.accumulator,
                        &previous_bundle.recursive_proof,
                        &evidence,
                        current_note,
                    )
                    .expect("real lineage append accumulator")
                } else {
                    iroha_data_model::offline::kagemusha_recursive_spend_accumulator_from_initial_evidence(
                        &evidence,
                        current_note,
                    )
                    .expect("real lineage initial accumulator")
                };
                let bundle = crate::zk::prove_kagemusha_recursive_spend_accumulator(
                    crate::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
                    &recursive_vk_box,
                    accumulator,
                    None,
                )
                .expect("real lineage recursive spend proof");
                if hop_index + 1 < lineage_steps.len() {
                    previous_recursive_proofs.push(bundle.recursive_proof.clone());
                }
                previous_bundle = Some(bundle);
            }
            let bundle = previous_bundle.expect("real lineage final recursive spend bundle");
            assert_eq!(bundle.accumulator.initial_root, initial_root);
            assert_eq!(bundle.accumulator.final_root, final_root);
            assert_eq!(bundle.accumulator.current_note, final_note);

            let lineage_witness = KagemushaRecursiveSpendLineageWitnessV1 {
                record_bundle: KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id: chain_id.clone(),
                        asset: definition_id.clone(),
                        steps: lineage_steps,
                    },
                    verifier_records: vec![KagemushaVerifiedFoldVerifierRecord {
                        id: lineage_record_id.clone(),
                        record: lineage_record.clone(),
                    }],
                },
                pallas_open_envelopes_archive,
                current_notes,
                previous_recursive_proofs,
            };
            crate::zk::verify_kagemusha_recursive_spend_lineage_witness_with_record_resolver(
                &bundle,
                &lineage_witness,
                |_| None,
            )
            .expect("real record-backed lineage witness replays");

            let recursive_vk_record = crate::zk::kagemusha_recursive_aggregation_proof_vk_record(
                crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                1,
            )
            .expect("recursive spend verifier record");
            let recursive_vk_id = bundle.recursive_proof.verifier_key_id.clone();
            let mut unshield_record =
                crate::zk::confidential_v2::confidential_unshield_v3_vk_record(
                    crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                    1,
                )
                .expect("confidential unshield v3 verifier record");
            let unshield_vk_box = unshield_record.key.clone().expect("unshield verifier key");
            let unshield_vk_commitment = unshield_record.commitment;
            let unshield_vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "recursive-kagemusha-real-lineage-unshield-v3",
            );
            unshield_record.status = ConfidentialStatus::Active;
            let unshield_proof = crate::zk::confidential_v2::build_confidential_unshield_proof_v3(
                &chain_id,
                &definition_id.to_string(),
                &final_spend_key,
                &tree_commitments,
                &[crate::zk::confidential_v2::ConfidentialUnshieldInputV2 {
                    amount,
                    rho: final_rho,
                    diversifier: final_diversifier,
                    leaf_index: final_leaf_index,
                }],
                &[],
                amount,
                final_root,
                &unshield_record.circuit_id,
                &unshield_vk_box,
            )
            .expect("real lineage final unshield proof");
            assert_eq!(unshield_proof.nullifiers, vec![final_note.spend_nullifier]);
            let mut redeem_attachment = ProofAttachment::new_ref(
                crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                unshield_proof.proof,
                unshield_vk_id.clone(),
            );
            redeem_attachment.vk_commitment = Some(unshield_vk_commitment);
            redeem_attachment.envelope_hash =
                Some(Hash::new(&redeem_attachment.proof.bytes).into());
            let instruction = RedeemKagemushaRecursive::new_with_lineage_witness(
                bundle,
                recipient.clone(),
                amount,
                redeem_attachment,
                Some(lineage_witness.clone()),
            );

            let mut world = World::with_assets(
                [domain],
                [authority_account, recipient_account],
                [asset_definition],
                [recipient_asset],
                [],
            );
            world
                .verifying_keys
                .insert(recursive_vk_id, recursive_vk_record);
            world
                .verifying_keys
                .insert(unshield_vk_id.clone(), unshield_record);
            world
                .verifying_keys
                .insert(lineage_record_id.clone(), lineage_record.clone());
            world.verifying_keys_by_circuit.insert(
                (
                    crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID.to_owned(),
                    1,
                ),
                unshield_vk_id.clone(),
            );
            world.zk_assets.insert(definition_id.clone(), {
                let mut zk_state = ZkAssetState::default();
                zk_state.allow_unshield = true;
                zk_state.commitments = tree_commitments;
                zk_state.root_history = vec![initial_root];
                zk_state.vk_unshield = Some(ZkAssetVerifierBinding {
                    id: unshield_vk_id,
                    commitment: unshield_vk_commitment,
                });
                zk_state
            });

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id);
            let mut zk = state.zk.clone();
            zk.halo2.enabled = true;
            zk.halo2.max_envelope_bytes = usize::MAX;
            zk.halo2.max_proof_bytes = usize::MAX;
            state.set_zk(zk);

            RecursiveKagemushaRedeemTestFixture {
                state,
                authority,
                recipient,
                definition_id,
                instruction,
                lineage_witness,
                lineage_record_id,
                lineage_record,
            }
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn real_recursive_kagemusha_redeem_test_fixture() -> RecursiveKagemushaRedeemTestFixture {
            let authority = sample_account(0x56);
            let recipient = sample_account(0x57);
            let chain_id: iroha_data_model::ChainId = "kagemusha-recursive-redeem-chain"
                .parse()
                .expect("chain id");
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "kgmr".parse().expect("asset definition name"),
            );
            let recipient_asset_id = AssetId::new(definition_id.clone(), recipient.clone());
            let domain = Domain::new(domain_id).build(&authority);
            let authority_account = Account::new(authority.clone()).build(&authority);
            let recipient_account = Account::new(recipient.clone()).build(&authority);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("kgmr".to_owned())
                    .confidential_policy(AssetConfidentialPolicy::convertible())
                    .build(&authority);
            let recipient_asset = Asset::new(recipient_asset_id, Numeric::zero());

            let spend_key = [0x77_u8; 32];
            let input_rho = [0x78_u8; 32];
            let input_diversifier = crate::zk::confidential_v2::derive_confidential_diversifier_v2(
                b"recursive-redeem-input",
            );
            let owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &spend_key,
                    input_diversifier,
                )
                .expect("recursive redeem owner tag");
            let note_commitment = crate::zk::confidential_v2::derive_confidential_note_v2(
                &definition_id.to_string(),
                42,
                input_rho,
                owner_tag,
            )
            .expect("recursive redeem note commitment");
            let tree_commitments = vec![note_commitment];
            let final_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&tree_commitments)
                    .expect("recursive redeem final root");
            let spend_nullifier = crate::zk::confidential_v2::derive_confidential_nullifier_v2(
                chain_id.as_str(),
                &definition_id.to_string(),
                &spend_key,
                input_rho,
            );
            let initial_root = fixed_bytes(b"recursive-redeem-initial-root");
            let recursive_fixture = recursive_spend_bundle_fixture_for_note(
                &chain_id,
                &definition_id,
                initial_root,
                final_root,
                note_commitment,
                spend_nullifier,
                42,
            );
            let bundle = recursive_fixture.bundle.clone();

            let recursive_vk_record = crate::zk::kagemusha_recursive_aggregation_proof_vk_record(
                crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                1,
            )
            .expect("recursive spend verifier record");
            let recursive_vk_id = bundle.recursive_proof.verifier_key_id.clone();
            let mut unshield_record =
                crate::zk::confidential_v2::confidential_unshield_v3_vk_record(
                    crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                    1,
                )
                .expect("confidential unshield v3 verifier record");
            let unshield_vk_box = unshield_record.key.clone().expect("unshield verifier key");
            let unshield_vk_commitment = unshield_record.commitment;
            let unshield_vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "recursive-kagemusha-unshield-v3",
            );
            unshield_record.status = ConfidentialStatus::Active;
            let unshield_proof = crate::zk::confidential_v2::build_confidential_unshield_proof_v3(
                &chain_id,
                &definition_id.to_string(),
                &spend_key,
                &tree_commitments,
                &[crate::zk::confidential_v2::ConfidentialUnshieldInputV2 {
                    amount: 42,
                    rho: input_rho,
                    diversifier: input_diversifier,
                    leaf_index: 0,
                }],
                &[],
                42,
                final_root,
                &unshield_record.circuit_id,
                &unshield_vk_box,
            )
            .expect("recursive Kagemusha final unshield proof");
            assert_eq!(unshield_proof.nullifiers, vec![spend_nullifier]);
            let mut redeem_attachment = ProofAttachment::new_ref(
                crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                unshield_proof.proof,
                unshield_vk_id.clone(),
            );
            redeem_attachment.vk_commitment = Some(unshield_vk_commitment);
            redeem_attachment.envelope_hash =
                Some(Hash::new(&redeem_attachment.proof.bytes).into());
            let instruction =
                RedeemKagemushaRecursive::new(bundle, recipient.clone(), 42, redeem_attachment);

            let mut world = World::with_assets(
                [domain],
                [authority_account, recipient_account],
                [asset_definition],
                [recipient_asset],
                [],
            );
            world
                .verifying_keys
                .insert(recursive_vk_id, recursive_vk_record);
            world
                .verifying_keys
                .insert(unshield_vk_id.clone(), unshield_record);
            world.verifying_keys_by_circuit.insert(
                (
                    crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID.to_owned(),
                    1,
                ),
                unshield_vk_id.clone(),
            );
            world.zk_assets.insert(definition_id.clone(), {
                let mut zk_state = ZkAssetState::default();
                zk_state.allow_unshield = true;
                zk_state.commitments = tree_commitments;
                zk_state.root_history = vec![initial_root];
                zk_state.vk_unshield = Some(ZkAssetVerifierBinding {
                    id: unshield_vk_id,
                    commitment: unshield_vk_commitment,
                });
                zk_state
            });

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id);
            let mut zk = state.zk.clone();
            zk.halo2.enabled = true;
            zk.halo2.max_envelope_bytes = usize::MAX;
            zk.halo2.max_proof_bytes = usize::MAX;
            state.set_zk(zk);

            RecursiveKagemushaRedeemTestFixture {
                state,
                authority,
                recipient,
                definition_id,
                instruction,
                lineage_witness: recursive_fixture.lineage_witness,
                lineage_record_id: recursive_fixture.lineage_record_id,
                lineage_record: recursive_fixture.lineage_record,
            }
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn real_recursive_kagemusha_redeem_test_state() -> (
            State,
            AccountId,
            AccountId,
            AssetDefinitionId,
            RedeemKagemushaRecursive,
        ) {
            let fixture = real_recursive_kagemusha_redeem_test_fixture();
            (
                fixture.state,
                fixture.authority,
                fixture.recipient,
                fixture.definition_id,
                fixture.instruction,
            )
        }

        fn mutate_kagemusha_transfer_envelope(
            transfer: &mut KagemushaTransfer,
            mutate: impl FnOnce(&mut OpenVerifyEnvelope),
        ) {
            let mut envelope: OpenVerifyEnvelope =
                norito::decode_from_bytes(&transfer.proof.proof.bytes)
                    .expect("Kagemusha transfer proof should be an OpenVerifyEnvelope");
            mutate(&mut envelope);
            transfer.proof.proof.bytes =
                norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
        }

        fn mutate_proof_attachment_envelope(
            proof: &mut ProofAttachment,
            mutate: impl FnOnce(&mut OpenVerifyEnvelope),
        ) {
            let mut envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&proof.proof.bytes)
                .expect("proof attachment should be an OpenVerifyEnvelope");
            mutate(&mut envelope);
            let encoded = norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
            proof.envelope_hash = Some(Hash::new(&encoded).into());
            proof.proof.bytes = encoded;
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn append_test_zk1_tlv(buf: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
            buf.extend_from_slice(tag);
            buf.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test TLV payload length fits u32")
                    .to_le_bytes(),
            );
            buf.extend_from_slice(payload);
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn reserved_lineage_spend_vk_box_for_tests() -> VerifyingKeyBox {
            let semantic_vk = crate::zk::kagemusha_recursive_aggregation_proof_vk_box()
                .expect("recursive aggregation verifier key");
            let mut bytes = b"ZK1\0".to_vec();
            let mut cursor = 4usize;
            let mut replaced_cid = false;
            while cursor < semantic_vk.bytes.len() {
                assert!(
                    semantic_vk.bytes.len() - cursor >= 8,
                    "verifier-key TLV header must be complete"
                );
                let tag = [
                    semantic_vk.bytes[cursor],
                    semantic_vk.bytes[cursor + 1],
                    semantic_vk.bytes[cursor + 2],
                    semantic_vk.bytes[cursor + 3],
                ];
                cursor += 4;
                let len = usize::try_from(u32::from_le_bytes([
                    semantic_vk.bytes[cursor],
                    semantic_vk.bytes[cursor + 1],
                    semantic_vk.bytes[cursor + 2],
                    semantic_vk.bytes[cursor + 3],
                ]))
                .expect("verifier-key TLV length fits usize");
                cursor += 4;
                let end = cursor
                    .checked_add(len)
                    .expect("verifier-key TLV end does not overflow");
                assert!(
                    end <= semantic_vk.bytes.len(),
                    "verifier-key TLV payload must be complete"
                );
                let payload = &semantic_vk.bytes[cursor..end];
                cursor = end;
                if &tag == b"CID1" {
                    append_test_zk1_tlv(
                        &mut bytes,
                        b"CID1",
                        crate::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_CIRCUIT_ID.as_bytes(),
                    );
                    replaced_cid = true;
                } else {
                    append_test_zk1_tlv(&mut bytes, &tag, payload);
                }
            }
            assert!(
                replaced_cid,
                "recursive aggregation verifier key must carry CID1"
            );
            VerifyingKeyBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), bytes)
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn reserved_lineage_spend_vk_record_for_tests(
            vk_box: &VerifyingKeyBox,
        ) -> VerifyingKeyRecord {
            let commitment = crate::zk::hash_vk(vk_box);
            let mut record = VerifyingKeyRecord::new_with_owner(
                1,
                crate::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_CIRCUIT_ID,
                None,
                crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                BackendTag::Halo2IpaPasta,
                "pallas",
                iroha_data_model::offline::kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
                commitment,
            );
            record.status = ConfidentialStatus::Active;
            record.key = Some(vk_box.clone());
            record.max_proof_bytes = 64 * 1024;
            record.vk_len =
                u32::try_from(vk_box.bytes.len()).expect("test verifier-key length fits u32");
            record
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn recursive_spend_public_inputs_for_tests(
            bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV1,
        ) -> Vec<[u8; 32]> {
            crate::zk::kagemusha_recursive_spend_bundle_instance_values(bundle)
                .expect("recursive spend instance values")
                .public_instance_columns()
                .into_iter()
                .map(|column| {
                    let [value]: [[u8; 32]; 1] =
                        column.try_into().expect("single-row public instance");
                    value
                })
                .collect()
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn install_reserved_lineage_spend_profile_for_tests(
            state: &mut State,
            instruction: &mut RedeemKagemushaRecursive,
        ) {
            let vk_box = reserved_lineage_spend_vk_box_for_tests();
            let vk_hash = crate::zk::hash_vk(&vk_box);
            let verifier_key_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                crate::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_CIRCUIT_ID,
            );
            instruction.bundle.recursive_proof.verifier_key_id = verifier_key_id.clone();
            instruction
                .bundle
                .recursive_proof
                .public_inputs
                .recursive_verifier_scalar_projection_digest =
                fixed_bytes(b"recursive-redeem-lineage-scalar-projection");
            instruction.bundle.recursive_proof.public_inputs_hash = instruction
                .bundle
                .recursive_proof
                .public_inputs
                .public_inputs_hash()
                .expect("reserved lineage public-input hash");

            let public_inputs = recursive_spend_public_inputs_for_tests(&instruction.bundle);
            let mut proof_bytes = b"ZK1\0".to_vec();
            append_test_zk1_tlv(&mut proof_bytes, b"PROF", &[0xB1; 64]);
            let mut instance_payload = Vec::with_capacity(8 + public_inputs.len() * 32);
            instance_payload.extend_from_slice(
                &u32::try_from(public_inputs.len())
                    .expect("reserved lineage instance column count fits u32")
                    .to_le_bytes(),
            );
            instance_payload.extend_from_slice(&1u32.to_le_bytes());
            for value in public_inputs {
                instance_payload.extend_from_slice(&value);
            }
            append_test_zk1_tlv(&mut proof_bytes, b"I10P", &instance_payload);
            let envelope = OpenVerifyEnvelope {
                backend: BackendTag::Halo2IpaPasta,
                circuit_id: crate::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_CIRCUIT_ID.to_owned(),
                vk_hash,
                public_inputs:
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
                        .to_vec(),
                proof_bytes,
                aux: Vec::new(),
            };
            instruction.bundle.recursive_proof.proof = ProofBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                norito::to_bytes(&envelope).expect("reserved lineage OpenVerifyEnvelope encode"),
            );
            state.world.verifying_keys.insert(
                verifier_key_id,
                reserved_lineage_spend_vk_record_for_tests(&vk_box),
            );
        }

        fn assert_kagemusha_transfer_record_mutation_rejects(
            mutate: impl for<'block, 'state> FnOnce(
                &mut StateTransaction<'block, 'state>,
                &VerifyingKeyId,
            ),
            label: &str,
            detail: &str,
        ) {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            let verifier_id = transfer.proof.vk_ref.clone();
            transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation_transaction = block.transaction();
                mutate(&mut mutation_transaction, &verifier_id);
                mutation_transaction.apply();
            }
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("mutated Kagemusha verifier metadata must reject before proof decode");
            assert_offline_rejection(err, label, detail);
        }

        fn assert_offline_rejection(err: Error, label: &str, detail: &str) {
            let message = err.to_string();
            assert!(
                message.contains(label),
                "expected error label `{label}`, got: {message}"
            );
            assert!(
                message.contains(detail),
                "expected error detail `{detail}`, got: {message}"
            );
        }

        #[test]
        fn kagemusha_recursive_redeem_change_output_rejects_existing_commitment() {
            let existing = fixed_bytes(b"recursive-redeem-existing-change");
            let fresh = fixed_bytes(b"recursive-redeem-fresh-change");
            let mut st = ZkAssetState::default();
            st.commitments.push(existing);

            ensure_kagemusha_recursive_redeem_change_output_available(&st, None)
                .expect("whole-note redeem has no change output to de-duplicate");
            ensure_kagemusha_recursive_redeem_change_output_available(&st, Some(fresh))
                .expect("fresh partial-redeem change output is available");
            let err =
                ensure_kagemusha_recursive_redeem_change_output_available(&st, Some(existing))
                    .expect_err("partial redeem change output must not reuse a tree commitment");
            assert_offline_rejection(err, "duplicate_output", "already in the shielded tree");
        }

        #[test]
        fn kagemusha_recursive_redeem_change_output_uses_v2_root_without_transfer_binding() {
            let initial = fixed_bytes(b"recursive-redeem-v2-root-initial");
            let change = fixed_bytes(b"recursive-redeem-v2-root-change");
            let mut st = ZkAssetState::default();
            st.commitments = vec![initial];
            st.root_history = vec![
                crate::zk::confidential_v2::compute_confidential_root_v2(&st.commitments)
                    .expect("initial confidential v2 root"),
            ];
            assert!(
                st.vk_transfer.is_none(),
                "fixture must cover assets without a transfer-v2 binding"
            );

            let root =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                    &mut st, change, 1,
                )
                .expect("append recursive redeem change output with v2 root");
            let expected_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&st.commitments)
                    .expect("updated confidential v2 root");
            assert_eq!(root, expected_root);
            assert_eq!(st.commitments, vec![initial, change]);
            assert_eq!(st.root_history, vec![expected_root]);

            let mut legacy = ZkAssetState::default();
            legacy.push_commitment(initial, 8);
            let legacy_root = legacy.push_commitment(change, 8);
            assert_ne!(
                root, legacy_root,
                "partial recursive redeem change output must not record the legacy tree root"
            );
        }

        #[test]
        fn kagemusha_recursive_redeem_change_output_v2_root_append_is_transactional_at_capacity() {
            let full_len = crate::zk::confidential_v2::CONFIDENTIAL_TREE_CAPACITY_V2;
            let mut st = ZkAssetState::default();
            st.commitments = vec![[0xA7; 32]; full_len];
            st.root_history = vec![[0xB8; 32], [0xC9; 32]];
            let original_commitments = st.commitments.clone();
            let original_roots = st.root_history.clone();

            let err =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                    &mut st,
                    fixed_bytes(b"recursive-redeem-over-cap-change"),
                    8,
                )
                .expect_err("over-capacity confidential v2 append must reject");
            let message = err.to_string();
            assert!(
                message.contains("supports at most"),
                "unexpected capacity error: {message}"
            );
            assert_eq!(
                st.commitments, original_commitments,
                "failed v2-root append must not mutate commitments"
            );
            assert_eq!(
                st.root_history, original_roots,
                "failed v2-root append must not mutate root history"
            );
        }

        #[test]
        fn kagemusha_recursive_redeem_change_output_v2_root_append_enforces_root_history_cap() {
            let initial = fixed_bytes(b"recursive-redeem-v2-cap-initial");
            let first = fixed_bytes(b"recursive-redeem-v2-cap-first");
            let second = fixed_bytes(b"recursive-redeem-v2-cap-second");
            let mut st = ZkAssetState::default();
            st.commitments = vec![initial];
            st.root_history = vec![
                crate::zk::confidential_v2::compute_confidential_root_v2(&st.commitments)
                    .expect("initial confidential v2 root"),
            ];

            let first_root =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                    &mut st, first, 2,
                )
                .expect("first capped v2-root append");
            let second_root =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_with_v2_root(
                    &mut st, second, 2,
                )
                .expect("second capped v2-root append");
            assert_eq!(st.commitments, vec![initial, first, second]);
            assert_eq!(
                st.root_history,
                vec![first_root, second_root],
                "v2-root append must evict old roots according to root_history_cap"
            );
        }

        #[test]
        fn confidential_transfer_v2_binding_path_uses_v2_root_helper() {
            let initial = fixed_bytes(b"transfer-v2-helper-initial");
            let output = fixed_bytes(b"transfer-v2-helper-output");
            let vk_id =
                VerifyingKeyId::new(crate::zk::ZK_BACKEND_HALO2_IPA, "transfer-v2-helper-vk");
            let mut vk_record = crate::zk::confidential_v2::confidential_transfer_v2_vk_record(
                crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                1,
            )
            .expect("confidential transfer v2 verifier record");
            let commitment = vk_record.commitment;
            vk_record.status = ConfidentialStatus::Active;

            let mut world = World::default();
            world.verifying_keys.insert(vk_id.clone(), vk_record);
            let kura = Kura::blank_kura_for_testing();
            let state = State::new(world, Arc::clone(&kura), LiveQueryStore::start_test());
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let mut st = ZkAssetState::default();
            st.commitments = vec![initial];
            st.root_history = vec![
                crate::zk::confidential_v2::compute_confidential_root_v2(&st.commitments)
                    .expect("initial confidential v2 root"),
            ];
            st.vk_transfer = Some(ZkAssetVerifierBinding {
                id: vk_id,
                commitment,
            });

            let root =
                crate::smartcontracts::isi::world::isi::push_confidential_commitment_for_asset(
                    &mut st,
                    output,
                    &transaction,
                )
                .expect("transfer-v2 binding path appends with v2 root");
            assert_eq!(
                root,
                crate::zk::confidential_v2::compute_confidential_root_v2(&st.commitments)
                    .expect("updated confidential v2 root")
            );
        }

        #[test]
        fn kagemusha_recursive_redeem_change_policy_accepts_exact_and_partial_only() {
            let current_amount = 50;
            let current_note_commitment = fixed_bytes(b"recursive-redeem-change-policy-note");
            let current_note_nullifier =
                fixed_bytes(b"recursive-redeem-change-policy-current-nullifier");
            let top_up_nullifier = fixed_bytes(b"recursive-redeem-change-policy-top-up-nullifier");
            let redeem_nullifiers = [current_note_nullifier, top_up_nullifier];
            let change_output = fixed_bytes(b"recursive-redeem-change-policy-change");

            assert_eq!(
                expected_kagemusha_recursive_redeem_change_output(
                    current_amount,
                    current_amount,
                    current_note_commitment,
                    None,
                    &redeem_nullifiers,
                )
                .expect("whole-note redeem uses the zero output marker"),
                [0u8; 32]
            );
            assert_eq!(
                expected_kagemusha_recursive_redeem_change_output(
                    45,
                    current_amount,
                    current_note_commitment,
                    Some(change_output),
                    &redeem_nullifiers,
                )
                .expect("partial redeem accepts an explicit private change output"),
                change_output
            );

            let cases = [
                (0, Some(change_output), "amount_mismatch", "non-zero"),
                (45, None, "amount_mismatch", "partial redeem"),
                (
                    current_amount,
                    Some(change_output),
                    "amount_mismatch",
                    "less than",
                ),
                (
                    current_amount + 1,
                    Some(change_output),
                    "amount_mismatch",
                    "less than",
                ),
                (
                    45,
                    Some([0u8; 32]),
                    "final_commitment_mismatch",
                    "change commitment",
                ),
                (
                    45,
                    Some(current_note_commitment),
                    "final_commitment_mismatch",
                    "change commitment",
                ),
                (
                    45,
                    Some(current_note_nullifier),
                    "final_commitment_mismatch",
                    "change commitment",
                ),
                (
                    45,
                    Some(top_up_nullifier),
                    "final_commitment_mismatch",
                    "change commitment",
                ),
            ];
            for (public_amount, change_output, label, detail) in cases {
                let err = expected_kagemusha_recursive_redeem_change_output(
                    public_amount,
                    current_amount,
                    current_note_commitment,
                    change_output,
                    &redeem_nullifiers,
                )
                .expect_err("invalid recursive Kagemusha redeem change policy must reject");
                assert_offline_rejection(err, label, detail);
            }
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_confidential_v2_envelope_mismatches() {
            let cases: [(&str, &str, fn(&mut OpenVerifyEnvelope)); 7] = [
                ("verifier_key_invalid", "backend", |envelope| {
                    envelope.backend = BackendTag::Stark;
                }),
                (
                    "verifier_key_invalid",
                    "confidential-transfer-v2",
                    |envelope| {
                        envelope.circuit_id = "halo2/pasta/ipa/tiny-add".to_owned();
                    },
                ),
                ("verifier_key_invalid", "canonical", |envelope| {
                    envelope.circuit_id =
                        "anon-transfer-2x2-merkle16-poseidon-diversified".to_owned();
                }),
                ("verifier_schema_mismatch", "schema", |envelope| {
                    envelope.public_inputs = b"not-confidential-transfer-v2".to_vec();
                }),
                ("verifier_key_invalid", "verifier-key hash", |envelope| {
                    envelope.vk_hash = [0xA5; 32];
                }),
                ("verifier_key_invalid", "non-zero", |envelope| {
                    envelope.vk_hash = [0u8; 32];
                }),
                ("invalid_proof", "auxiliary bytes", |envelope| {
                    envelope.aux = b"kagemusha-forged-chain-aux".to_vec();
                }),
            ];

            for (label, detail, mutate) in cases {
                let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate_kagemusha_transfer_envelope(&mut transfer, mutate);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("mutated Kagemusha proof envelope must reject");
                assert_offline_rejection(err, label, detail);
            }
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_malformed_envelope_and_missing_root_hint() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("malformed Kagemusha proof envelope must reject");
            assert_offline_rejection(err, "invalid_proof", "OpenVerifyEnvelope");

            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.root_hint = None;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha confidential transfer must require root_hint");
            assert_offline_rejection(err, "invalid_proof", "root hint");
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_forged_envelope_hash_metadata() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.envelope_hash = Some([0xA7; 32]);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("forged Kagemusha envelope hash must reject");
            assert_offline_rejection(err, "invalid_proof", "envelope hash");
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_missing_verifier_key_commitment_metadata() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.vk_commitment = None;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("missing Kagemusha verifier-key commitment must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "must publish the asset-bound verifier-key commitment",
            );
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_zero_verifier_key_commitment_metadata() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.vk_commitment = Some([0u8; 32]);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("zero Kagemusha verifier-key commitment must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "non-zero asset-bound verifier-key commitment",
            );
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_empty_verifier_key_id_name() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.vk_ref.name = "   ".to_owned();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("empty Kagemusha verifier-key id name must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "verifier key id name must be non-empty",
            );
        }

        #[test]
        fn reserve_offline_note_escrow_rejects_escrow_self_reference() {
            let (state, asset_id, _account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err =
                reserve_offline_note_escrow(&mut transaction, &asset_id, &Numeric::new(25, 0))
                    .expect_err("self-referenced escrow must reject note reservation");
            assert!(
                err.to_string().contains("escrow_self_reference"),
                "unexpected error: {err}"
            );

            let balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::new(100, 0));
        }

        #[test]
        fn reserve_offline_note_escrow_preserves_explicit_universal_partition_on_asset_route() {
            let (state, asset_id, escrow_asset_id, account_id, _escrow_account_id, definition_id) =
                scoped_restricted_escrow_test_state(
                    Numeric::new(100, 0),
                    Numeric::zero(),
                    DataSpaceId::UNIVERSAL,
                    0x7B,
                );
            let route_dataspace = DataSpaceId::new(20);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.current_dataspace_id = Some(route_dataspace);
            transaction.world.current_dataspace_id = Some(route_dataspace);

            reserve_offline_note_escrow(&mut transaction, &asset_id, &Numeric::new(25, 0))
                .expect("offline issue should reserve the explicitly requested partition");

            let source_balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            let route_scoped_asset = AssetId::with_scope(
                definition_id,
                account_id,
                AssetBalanceScope::Dataspace(route_dataspace),
            );

            assert_eq!(source_balance, Numeric::new(75, 0));
            assert_eq!(escrow_balance, Numeric::new(25, 0));
            assert!(
                transaction.world.assets.get(&route_scoped_asset).is_none(),
                "offline escrow must not retarget an explicit universal balance to the route dataspace"
            );
        }

        #[test]
        fn credit_from_offline_note_escrow_preserves_explicit_universal_partition_on_asset_route() {
            let (state, asset_id, escrow_asset_id, account_id, _escrow_account_id, _definition_id) =
                scoped_restricted_escrow_test_state(
                    Numeric::new(75, 0),
                    Numeric::new(25, 0),
                    DataSpaceId::UNIVERSAL,
                    0x7C,
                );
            let route_dataspace = DataSpaceId::new(20);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.current_dataspace_id = Some(route_dataspace);
            transaction.world.current_dataspace_id = Some(route_dataspace);

            credit_from_offline_note_escrow(
                &mut transaction,
                &asset_id,
                &account_id,
                &Numeric::new(10, 0),
            )
            .expect("offline redeem should credit the explicitly requested partition");

            let source_balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            let escrow_balance = transaction
                .world
                .assets
                .get(&escrow_asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);

            assert_eq!(source_balance, Numeric::new(85, 0));
            assert_eq!(escrow_balance, Numeric::new(15, 0));
        }

        #[test]
        fn credit_from_offline_note_escrow_rejects_escrow_self_reference() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = credit_from_offline_note_escrow(
                &mut transaction,
                &asset_id,
                &account_id,
                &Numeric::new(25, 0),
            )
            .expect_err("self-referenced escrow must reject note credit");
            assert!(
                err.to_string().contains("escrow_self_reference"),
                "unexpected error: {err}"
            );

            let balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::new(100, 0));
        }

        #[test]
        fn expected_public_instances_encode_semantic_columns() {
            let values = crate::zk::OfflineNoteInstanceValues {
                public_values: [11, 22, 33, 44, 1, 1, 1, 10, 10, 55, 0, 66, 77, 88, 0, 0],
                input_amounts: [10, 0, 0, 0],
                output_amounts: [10, 0],
            };
            let instances = values.public_instance_columns();

            assert_eq!(instances.len(), crate::zk::OFFLINE_NOTE_INSTANCE_COLUMNS);
            for (index, value) in values.public_values.iter().copied().enumerate() {
                let mut expected = [0u8; 32];
                expected[..8].copy_from_slice(&value.to_le_bytes());
                assert_eq!(instances[index], vec![expected]);
            }
        }

        #[test]
        fn key_certificate_requires_one_use_ed25519_key() {
            let mut certificate = sample_certificate();
            assert!(validate_offline_note_key_certificate(&certificate).is_ok());

            certificate.one_use = false;
            assert!(validate_offline_note_key_certificate(&certificate).is_err());

            certificate.one_use = true;
            certificate.assertion_usage_count_limit = Some(2);
            assert!(validate_offline_note_key_certificate(&certificate).is_err());

            certificate.assertion_usage_count_limit = Some(0);
            assert!(validate_offline_note_key_certificate(&certificate).is_err());

            certificate.assertion_usage_count_limit = Some(1);
            assert!(validate_offline_note_key_certificate(&certificate).is_err());

            certificate.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            certificate.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            certificate.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            assert!(validate_offline_note_key_certificate(&certificate).is_ok());

            certificate.public_key.clear();
            assert!(validate_offline_note_key_certificate(&certificate).is_err());
        }

        #[test]
        fn key_certificate_rejects_blank_hardware_attestation_identity() {
            let cases: [(&str, fn(&mut OfflineNoteKeyCertificate)); 6] = [
                ("platform", |certificate: &mut OfflineNoteKeyCertificate| {
                    certificate.platform = "   ".to_owned();
                }),
                ("key_id", |certificate: &mut OfflineNoteKeyCertificate| {
                    certificate.key_id.clear();
                }),
                (
                    "device_id",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.device_id = "\t".to_owned();
                    },
                ),
                (
                    "assertion_scheme",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.assertion_scheme.clear();
                    },
                ),
                (
                    "assertion_key_algorithm",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.assertion_key_algorithm = " ".to_owned();
                    },
                ),
                (
                    "assertion_public_key",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.assertion_public_key.clear();
                    },
                ),
            ];

            for (field, mutate) in cases {
                let mut certificate = sample_certificate();
                mutate(&mut certificate);
                let err = validate_offline_note_key_certificate(&certificate)
                    .expect_err("blank hardware attestation identity must reject");
                assert_offline_rejection(err.into(), "invalid_issuer_cert", field);
            }
        }

        #[test]
        fn key_certificate_rejects_padded_hardware_attestation_identity() {
            let cases: [(&str, fn(&mut OfflineNoteKeyCertificate)); 5] = [
                ("platform", |certificate: &mut OfflineNoteKeyCertificate| {
                    certificate.platform =
                        format!(" {OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST}");
                }),
                ("key_id", |certificate: &mut OfflineNoteKeyCertificate| {
                    certificate.key_id = format!("{} ", certificate.key_id);
                }),
                (
                    "device_id",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.device_id = format!("\t{}", certificate.device_id);
                    },
                ),
                (
                    "assertion_scheme",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.assertion_scheme = format!("{} ", certificate.assertion_scheme);
                    },
                ),
                (
                    "assertion_key_algorithm",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.assertion_key_algorithm =
                            format!("\n{}", certificate.assertion_key_algorithm);
                    },
                ),
            ];

            for (field, mutate) in cases {
                let mut certificate = sample_certificate();
                mutate(&mut certificate);
                let err = validate_offline_note_key_certificate(&certificate)
                    .expect_err("padded hardware attestation identity must reject");
                assert_offline_rejection(err.into(), "invalid_issuer_cert", field);
            }
        }

        #[test]
        fn key_certificate_rejects_unsupported_hardware_assertion_profiles() {
            let cases: [(&str, fn(&mut OfflineNoteKeyCertificate)); 4] = [
                (
                    "unsupported",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.platform = "offline-unit-test".to_owned();
                        certificate.assertion_scheme = "unit-test-one-use".to_owned();
                        certificate.assertion_key_algorithm = "ed25519-test".to_owned();
                        certificate.assertion_usage_count_limit = Some(1);
                    },
                ),
                ("splice", |certificate: &mut OfflineNoteKeyCertificate| {
                    certificate.platform = OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned();
                    certificate.assertion_scheme =
                        OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
                    certificate.assertion_key_algorithm =
                        OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
                    certificate.assertion_usage_count_limit = Some(1);
                }),
                (
                    "ios usage",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.assertion_usage_count_limit = Some(1);
                    },
                ),
                (
                    "android missing usage",
                    |certificate: &mut OfflineNoteKeyCertificate| {
                        certificate.platform =
                            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
                        certificate.assertion_scheme =
                            OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
                        certificate.assertion_key_algorithm =
                            OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
                        certificate.assertion_usage_count_limit = None;
                    },
                ),
            ];

            for (_case, mutate) in cases {
                let mut certificate = sample_certificate();
                mutate(&mut certificate);
                let err = validate_offline_note_key_certificate(&certificate)
                    .expect_err("unsupported hardware assertion profile must reject");
                assert_offline_rejection(err.into(), "invalid_issuer_cert", "profile");
            }
        }

        #[test]
        fn key_certificate_rejects_off_curve_assertion_public_key() {
            let mut certificate = sample_certificate();
            certificate.assertion_public_key = vec![0; 65];
            certificate.assertion_public_key[0] = 0x04;

            let err = validate_offline_note_key_certificate(&certificate)
                .expect_err("off-curve certificate assertion key must reject");
            assert_offline_rejection(
                err.into(),
                "invalid_issuer_cert",
                "valid uncompressed P-256",
            );
        }

        #[test]
        fn offline_device_attestation_registration_rejects_blank_identity_fields() {
            let cases: [(&str, fn(&mut OfflineDeviceAttestationRegistration)); 5] = [
                (
                    "platform",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.platform = "   ".to_owned();
                    },
                ),
                (
                    "key_id",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.key_id = "\n".to_owned();
                    },
                ),
                (
                    "device_id",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.device_id.clear();
                    },
                ),
                (
                    "assertion_scheme",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.assertion_scheme = "\t".to_owned();
                    },
                ),
                (
                    "assertion_key_algorithm",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.assertion_key_algorithm = " ".to_owned();
                    },
                ),
            ];

            for (field, mutate) in cases {
                let (state, recent_block_height, recent_block_hash) =
                    state_with_attestation_anchor();
                let certificate = sample_certificate();
                let mut registration =
                    attestation_registration(&certificate, recent_block_height, recent_block_hash);
                mutate(&mut registration);
                let authority = registration.account_id.clone();
                let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = RegisterOfflineDeviceAttestation::new(registration)
                    .execute(&authority, &mut transaction)
                    .expect_err("blank device attestation identity field must reject");
                assert_offline_rejection(err, "invalid_attestation", field);
            }
        }

        #[test]
        fn offline_device_attestation_registration_rejects_padded_identity_fields() {
            let cases: [(&str, fn(&mut OfflineDeviceAttestationRegistration)); 5] = [
                (
                    "platform",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.platform =
                            format!("{OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST} ");
                    },
                ),
                (
                    "key_id",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.key_id = format!(" {}", registration.key_id);
                    },
                ),
                (
                    "device_id",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.device_id = format!("{}\n", registration.device_id);
                    },
                ),
                (
                    "assertion_scheme",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.assertion_scheme =
                            format!("\t{}", registration.assertion_scheme);
                    },
                ),
                (
                    "assertion_key_algorithm",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.assertion_key_algorithm =
                            format!("{} ", registration.assertion_key_algorithm);
                    },
                ),
            ];

            for (field, mutate) in cases {
                let (state, recent_block_height, recent_block_hash) =
                    state_with_attestation_anchor();
                let certificate = sample_certificate();
                let mut registration =
                    attestation_registration(&certificate, recent_block_height, recent_block_hash);
                mutate(&mut registration);
                let authority = registration.account_id.clone();
                let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = RegisterOfflineDeviceAttestation::new(registration)
                    .execute(&authority, &mut transaction)
                    .expect_err("padded device attestation identity field must reject");
                assert_offline_rejection(err, "invalid_attestation", field);
            }
        }

        #[test]
        fn offline_device_attestation_registration_rejects_padded_app_metadata() {
            let cases: [(&str, fn(&mut OfflineDeviceAttestationRegistration)); 4] = [
                (
                    "ios_team_id",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.ios_team_id = Some(" TEAMID1234 ".to_owned());
                    },
                ),
                (
                    "ios_bundle_id",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.ios_bundle_id =
                            Some("\tjp.co.soramitsu.iroha.offline".to_owned());
                    },
                ),
                (
                    "ios_environment",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.ios_environment = Some("production\n".to_owned());
                    },
                ),
                (
                    "android_package_name",
                    |registration: &mut OfflineDeviceAttestationRegistration| {
                        registration.android_package_name =
                            Some(" jp.co.soramitsu.iroha.offline ".to_owned());
                    },
                ),
            ];

            for (field, mutate) in cases {
                let (state, recent_block_height, recent_block_hash) =
                    state_with_attestation_anchor();
                let mut registration = if field == "android_package_name" {
                    android_attestation_registration(recent_block_height, recent_block_hash)
                } else {
                    let certificate = sample_certificate();
                    attestation_registration(&certificate, recent_block_height, recent_block_hash)
                };
                mutate(&mut registration);
                let authority = registration.account_id.clone();
                let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = RegisterOfflineDeviceAttestation::new(registration)
                    .execute(&authority, &mut transaction)
                    .expect_err("padded attestation app metadata field must reject");
                assert_offline_rejection(err, "invalid_attestation", field);
            }
        }

        #[test]
        fn offline_device_attestation_registration_records_replay_markers() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let certificate_payload_hash = registration
                .key_certificate_payload_hash()
                .expect("certificate hash");
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            RegisterOfflineDeviceAttestation::new(registration.clone())
                .execute(&authority, &mut transaction)
                .expect("valid attestation registration");

            for key in [
                offline_note_attested_certificate_key(&certificate_payload_hash),
                offline_note_attestation_challenge_key(&registration.challenge_hash),
                offline_note_attestation_report_key(&registration.attestation_report_hash),
                offline_note_attestation_evidence_key(&registration.evidence_hash),
            ] {
                assert!(
                    transaction
                        .world
                        .offline_note_replay_keys
                        .get(&key)
                        .is_some(),
                    "attestation marker must be recorded"
                );
            }
        }

        #[test]
        fn offline_device_attestation_registration_rejects_duplicate_evidence() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            RegisterOfflineDeviceAttestation::new(registration.clone())
                .execute(&authority, &mut transaction)
                .expect("first registration records replay markers");
            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("duplicate registration must reject");

            assert!(
                err.to_string().contains("duplicate_attestation"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn offline_device_attestation_registration_rejects_preseeded_certificate_marker() {
            assert_attestation_registration_rejects_preseeded_replay_marker(|registration| {
                offline_note_attested_certificate_key(
                    &registration
                        .key_certificate_payload_hash()
                        .expect("certificate hash"),
                )
            });
        }

        #[test]
        fn offline_device_attestation_registration_rejects_preseeded_challenge_marker() {
            assert_attestation_registration_rejects_preseeded_replay_marker(|registration| {
                offline_note_attestation_challenge_key(&registration.challenge_hash)
            });
        }

        #[test]
        fn offline_device_attestation_registration_rejects_preseeded_report_marker() {
            assert_attestation_registration_rejects_preseeded_replay_marker(|registration| {
                offline_note_attestation_report_key(&registration.attestation_report_hash)
            });
        }

        #[test]
        fn offline_device_attestation_registration_rejects_preseeded_evidence_marker() {
            assert_attestation_registration_rejects_preseeded_replay_marker(|registration| {
                offline_note_attestation_evidence_key(&registration.evidence_hash)
            });
        }

        #[test]
        fn offline_device_attestation_registration_invalid_attempt_does_not_consume_replay_markers()
        {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let mut invalid_registration = registration.clone();
            invalid_registration.attestation_report.push(0xFF);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(invalid_registration)
                .execute(&authority, &mut transaction)
                .expect_err("invalid registration must reject");
            assert_offline_rejection(err, "invalid_attestation", "report hash");

            RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect("valid registration must still succeed after a failed attempt");
        }

        #[test]
        fn on_chain_attested_certificate_authorizes_without_issuer_signature() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let unsigned_certificate = registration.key_certificate();
            assert!(
                ensure_offline_note_certificate_signature(&unsigned_certificate, &authority)
                    .is_err(),
                "zero issuer signature should not satisfy middleware flow"
            );

            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            RegisterOfflineDeviceAttestation::new(registration.clone())
                .execute(&authority, &mut transaction)
                .expect("valid attestation registration");

            let authorized_hash = ensure_offline_note_certificate_authorized(
                &unsigned_certificate,
                &authority,
                &transaction,
            )
            .expect("on-chain attestation marker authorizes certificate");
            assert_eq!(
                authorized_hash,
                registration
                    .key_certificate_payload_hash()
                    .expect("certificate hash")
            );
        }

        #[test]
        fn offline_device_attestation_registration_requires_recent_committed_block() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height + 1, recent_block_hash);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("uncommitted block anchor must reject");
            assert!(
                err.to_string().contains("invalid_attestation"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn offline_device_attestation_registration_rejects_tampered_challenge_preimage() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.key_id.push_str("-tampered-after-challenge");
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("tampered challenge preimage must reject");
            assert_offline_rejection(err, "invalid_attestation", "challenge hash");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_wrong_recent_block_hash() {
            let (state, recent_block_height, _recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration = attestation_registration(
                &certificate,
                recent_block_height,
                Hash::new(b"wrong-attestation-anchor"),
            );
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("wrong block hash must reject");
            assert_offline_rejection(err, "invalid_attestation", "recent block hash");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_stale_anchor() {
            let (state, _committed_height, _latest_hash) = state_with_attestation_anchor_count(
                OFFLINE_NOTE_ATTESTATION_RECENT_BLOCK_WINDOW + 2,
            );
            let stale_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, 1, Hash::from(stale_header.hash()));
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(200_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("stale block anchor must reject");
            assert_offline_rejection(err, "stale_attestation", "recent block window");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_expired_registration() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.expires_at_ms = 0;
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("expired attestation must reject");
            assert_offline_rejection(err, "expired_attestation", "expired");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_unauthorized_authority() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let unauthorized = sample_account(0x77);
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&unauthorized, &mut transaction)
                .expect_err("unrelated authority must reject");
            assert_offline_rejection(err, "unauthorized_controller", "note account");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_unsupported_platform() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = "browser-webauthn".to_owned();
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("unsupported platform must reject");
            assert_offline_rejection(err, "invalid_attestation", "unsupported");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_profile_splice() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = Some(1);
            refresh_attestation_material(&mut registration);
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("spliced Android assertion profile must reject for iOS");
            assert_offline_rejection(err, "invalid_attestation", "App Attest assertion profile");
        }

        #[test]
        fn offline_device_attestation_registration_accepts_android_keymint_profile() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = Some(1);
            refresh_attestation_material(&mut registration);
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect("canonical Android KeyMint profile should register");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_without_usage_limit() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = None;
            refresh_attestation_material(&mut registration);
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint registration without one-use limit must reject");
            assert_offline_rejection(err, "invalid_attestation", "one-use P-256");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_malformed_assertion_key() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.assertion_public_key = vec![0x02; 33];
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("compressed or malformed assertion key must reject");
            assert_offline_rejection(err, "invalid_attestation", "uncompressed P-256");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_off_curve_assertion_key() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.assertion_public_key = vec![0; 65];
            registration.assertion_public_key[0] = 0x04;
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("off-curve assertion key must reject");
            assert_offline_rejection(err, "invalid_attestation", "valid uncompressed P-256");
        }

        #[test]
        fn app_attest_cose_key_rejects_trailing_cbor_bytes() {
            let public_key = sample_p256_assertion_public_key();
            let mut cose_key = sample_app_attest_cose_key(&public_key);
            cose_key.push(0xF6);

            let err = validate_app_attest_cose_p256_key(&cose_key, &public_key)
                .expect_err("COSE key with trailing CBOR data must reject");
            assert_offline_rejection(err, "invalid_attestation", "trailing CBOR bytes");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_report_hash_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.attestation_report.push(0xFF);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("report bytes that do not match the hash must reject");
            assert_offline_rejection(err, "invalid_attestation", "report hash");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_forged_evidence_envelope() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.evidence = OFFLINE_ATTESTATION_EVIDENCE_PREFIX.to_vec();
            registration
                .evidence
                .extend_from_slice(&[0xA5; Hash::LENGTH]);
            registration.evidence_hash = Hash::new(&registration.evidence);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("evidence envelope that does not bind report hash must reject");
            assert_offline_rejection(err, "invalid_attestation", "evidence envelope");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_malformed_ios_report() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.attestation_report = b"not-cbor".to_vec();
            refresh_attestation_hashes(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("malformed iOS App Attest report must reject");
            assert_offline_rejection(err, "invalid_attestation", "CBOR attestation object");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_report_trailing_cbor_bytes() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.attestation_report.push(0xF6);
            refresh_attestation_hashes(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("iOS App Attest report with trailing CBOR data must reject");
            assert_offline_rejection(err, "invalid_attestation", "trailing CBOR bytes");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_duplicate_cbor_key() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            duplicate_ios_attestation_report_text_key(&mut registration, "fmt");
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest report with duplicate CBOR keys must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicate text key");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_report_key_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let wrong_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("test App Attest wrong key generation");
            registration.assertion_public_key = wrong_key.public_key_raw().to_vec();
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest credential key mismatch must reject");
            assert_offline_rejection(err, "invalid_attestation", "credential public key");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_malformed_key_id() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.key_id = "not standard base64!".to_owned();
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("malformed App Attest key_id must reject");
            assert_offline_rejection(err, "invalid_attestation", "standard base64");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_noncanonical_key_id() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            refresh_ios_attestation_material_with_noncanonical_key_id(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("non-canonical App Attest key_id must reject");
            assert_offline_rejection(err, "invalid_attestation", "canonical standard base64");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_credential_id_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.key_id = BASE64_STANDARD.encode([0x77; 32]);
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest credential id mismatch must reject");
            assert_offline_rejection(err, "invalid_attestation", "credential id");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_duplicate_nonce_extension() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            refresh_ios_attestation_material_with_duplicate_nonce_extension(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest duplicate nonce extensions must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicate nonce extensions");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_key_id_public_key_hash_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            refresh_ios_attestation_material_with_key_id_override(&mut registration, &[0x77; 32]);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest key_id public key hash mismatch must reject");
            assert_offline_rejection(err, "invalid_attestation", "public key hash");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_missing_metadata() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.ios_team_id = None;
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("missing iOS App Attest metadata must reject");
            assert_offline_rejection(err, "invalid_attestation", "Apple Team ID");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_app_identity_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.ios_bundle_id = Some("jp.co.soramitsu.iroha.other".to_owned());
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest rpIdHash mismatch must reject");
            assert_offline_rejection(err, "invalid_attestation", "app identity hash");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_nonce_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.expires_at_ms += 1;
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest nonce that binds an older challenge must reject");
            assert_offline_rejection(err, "invalid_attestation", "nonce extension");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_untrusted_ios_root() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            refresh_ios_attestation_material_with_root_name(
                &mut registration,
                "Iroha Offline Attestation Wrong Root",
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("App Attest report under an untrusted root must reject");
            assert_offline_rejection(err, "invalid_attestation", "trusted root");
        }

        #[test]
        fn offline_device_attestation_policy_update_requires_manager_permission() {
            let (state, _recent_block_height, _recent_block_hash) = state_with_attestation_anchor();
            let authority = sample_account(0x71);
            let policy = default_policy_for_tests();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = SetOfflineDeviceAttestationPolicy::new(policy.clone())
                .execute(&authority, &mut transaction)
                .expect_err("unauthorized policy update must reject");
            assert_offline_rejection(err, "unauthorized_controller", "policy manager");

            grant_attestation_policy_manager(&mut transaction, &authority);
            SetOfflineDeviceAttestationPolicy::new(policy)
                .execute(&authority, &mut transaction)
                .expect("authorized policy manager can store attestation policy");
            assert!(
                transaction
                    .world
                    .smart_contract_state
                    .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
                    .is_some(),
                "policy update must write the governed policy state"
            );
        }

        #[test]
        fn offline_device_attestation_policy_rejects_invalid_revocation_digest() {
            let (state, _recent_block_height, _recent_block_hash) = state_with_attestation_anchor();
            let authority = sample_account(0x72);
            let mut policy = default_policy_for_tests();
            policy.revoked_certificate_sha256.push(vec![0xAA; 31]);
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            grant_attestation_policy_manager(&mut transaction, &authority);

            let err = SetOfflineDeviceAttestationPolicy::new(policy)
                .execute(&authority, &mut transaction)
                .expect_err("invalid revocation digest must reject");
            assert_offline_rejection(err, "invalid_attestation_policy", "32-byte SHA-256");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_unsupported_version() {
            let mut policy = default_policy_for_tests();
            policy.version = 2;

            assert_policy_update_rejected(policy, "version is unsupported");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_duplicate_trusted_root() {
            let mut policy = default_policy_for_tests();
            policy.trusted_roots.push(
                policy
                    .trusted_roots
                    .first()
                    .expect("default policy has a trusted root")
                    .clone(),
            );

            assert_policy_update_rejected(policy, "duplicate trusted root");
        }

        #[test]
        fn offline_device_attestation_policy_ignores_x509_time_for_inactive_root() {
            let mut policy = default_policy_for_tests();
            policy
                .trusted_roots
                .push(OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned(),
                    der: test_attestation_root_der_with_validity(
                        "Iroha Offline Attestation Inactive Expired Root",
                        1999,
                        2000,
                    ),
                    not_before_ms: Some(1),
                    not_after_ms: Some(2),
                });

            execute_policy_update_for_tests(policy)
                .expect("inactive out-of-time root must not invalidate active policy roots");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_active_root_outside_x509_time() {
            let mut policy = default_policy_for_tests();
            policy
                .trusted_roots
                .push(OfflineDeviceAttestationTrustedRoot {
                    platform: OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST.to_owned(),
                    der: test_attestation_root_der_with_validity(
                        "Iroha Offline Attestation Active Expired Root",
                        1999,
                        2000,
                    ),
                    not_before_ms: None,
                    not_after_ms: None,
                });

            let err = execute_policy_update_for_tests(policy)
                .expect_err("active out-of-time root must reject");
            assert_offline_rejection(err, "invalid_attestation", "block timestamp");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_zero_revocation_digest() {
            let mut policy = default_policy_for_tests();
            policy.revoked_certificate_sha256.push(vec![0; 32]);

            assert_policy_update_rejected(policy, "invalid revoked certificate digest");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_duplicate_revocation_digest() {
            let mut policy = default_policy_for_tests();
            policy.revoked_certificate_sha256.push(vec![0xAA; 32]);
            policy.revoked_certificate_sha256.push(vec![0xAA; 32]);

            assert_policy_update_rejected(policy, "invalid revoked certificate digest");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_required_empty_ios_app_policy() {
            let mut policy = default_policy_for_tests();
            policy.require_ios_app_policy = true;
            policy.ios_apps.clear();

            assert_policy_update_rejected(policy, "requires iOS apps");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_required_empty_android_app_policy() {
            let mut policy = default_policy_for_tests();
            policy.require_android_app_policy = true;
            policy.android_apps.clear();

            assert_policy_update_rejected(policy, "requires Android apps");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_duplicate_android_signing_digest() {
            let mut policy = default_policy_for_tests();
            policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
                package_name: ANDROID_TEST_PACKAGE_NAME.to_owned(),
                signing_certificate_sha256: vec![
                    ANDROID_TEST_SIGNING_CERT_SHA256.to_vec(),
                    ANDROID_TEST_SIGNING_CERT_SHA256.to_vec(),
                ],
            }];

            assert_policy_update_rejected(policy, "invalid signing digest");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_zero_android_signing_digest() {
            let mut policy = default_policy_for_tests();
            policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
                package_name: ANDROID_TEST_PACKAGE_NAME.to_owned(),
                signing_certificate_sha256: vec![vec![0; 32]],
            }];

            assert_policy_update_rejected(policy, "invalid signing digest");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_duplicate_ios_app_identity() {
            let mut policy = default_policy_for_tests();
            policy.ios_apps = vec![
                OfflineIosAppAttestationPolicy {
                    team_id: "teamid1234".to_owned(),
                    bundle_id: "jp.co.soramitsu.iroha.offline".to_owned(),
                    environment: "production".to_owned(),
                },
                OfflineIosAppAttestationPolicy {
                    team_id: "TEAMID1234".to_owned(),
                    bundle_id: "jp.co.soramitsu.iroha.offline".to_owned(),
                    environment: "PRODUCTION".to_owned(),
                },
            ];

            assert_policy_update_rejected(policy, "duplicate iOS app identity");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_padded_ios_app_identity() {
            let mut policy = default_policy_for_tests();
            policy.ios_apps = vec![OfflineIosAppAttestationPolicy {
                team_id: " TEAMID1234 ".to_owned(),
                bundle_id: "jp.co.soramitsu.iroha.offline".to_owned(),
                environment: "production".to_owned(),
            }];

            assert_policy_update_rejected(policy, "surrounding whitespace");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_invalid_ios_environment() {
            let mut policy = default_policy_for_tests();
            policy.ios_apps = vec![OfflineIosAppAttestationPolicy {
                team_id: "TEAMID1234".to_owned(),
                bundle_id: "jp.co.soramitsu.iroha.offline".to_owned(),
                environment: "staging".to_owned(),
            }];

            assert_policy_update_rejected(policy, "production or development");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_duplicate_android_app_identity() {
            let mut policy = default_policy_for_tests();
            policy.android_apps = vec![
                OfflineAndroidAppAttestationPolicy {
                    package_name: ANDROID_TEST_PACKAGE_NAME.to_owned(),
                    signing_certificate_sha256: vec![
                        vec![0x11; 32],
                        ANDROID_TEST_SIGNING_CERT_SHA256.to_vec(),
                    ],
                },
                OfflineAndroidAppAttestationPolicy {
                    package_name: ANDROID_TEST_PACKAGE_NAME.to_owned(),
                    signing_certificate_sha256: vec![
                        ANDROID_TEST_SIGNING_CERT_SHA256.to_vec(),
                        vec![0x11; 32],
                    ],
                },
            ];

            assert_policy_update_rejected(policy, "duplicate Android app identity");
        }

        #[test]
        fn offline_device_attestation_policy_rejects_padded_android_app_identity() {
            let mut policy = default_policy_for_tests();
            policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
                package_name: format!(" {ANDROID_TEST_PACKAGE_NAME} "),
                signing_certificate_sha256: vec![ANDROID_TEST_SIGNING_CERT_SHA256.to_vec()],
            }];

            assert_policy_update_rejected(policy, "surrounding whitespace");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_ios_app_disallowed_by_policy() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let mut policy = default_policy_for_tests();
            policy.require_ios_app_policy = true;
            policy.ios_apps = vec![OfflineIosAppAttestationPolicy {
                team_id: "TEAMID1234".to_owned(),
                bundle_id: "jp.co.soramitsu.iroha.other".to_owned(),
                environment: "production".to_owned(),
            }];
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("iOS app not allowed by policy must reject");
            assert_offline_rejection(
                err,
                "invalid_attestation_policy",
                "iOS App Attest app identity",
            );
        }

        #[test]
        fn offline_device_attestation_registration_accepts_ios_app_allowed_by_policy() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let mut policy = default_policy_for_tests();
            policy.require_ios_app_policy = true;
            policy.ios_apps = vec![OfflineIosAppAttestationPolicy {
                team_id: "teamid1234".to_owned(),
                bundle_id: "jp.co.soramitsu.iroha.offline".to_owned(),
                environment: "PRODUCTION".to_owned(),
            }];
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect("iOS app allowed by policy must be accepted");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_revoked_ios_leaf_certificate() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let leaf_der = ios_report_certificate_der(&registration.attestation_report, 0);
            let mut policy = default_policy_for_tests();
            policy
                .revoked_certificate_sha256
                .push(sha256_bytes(&leaf_der).to_vec());
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("revoked iOS leaf certificate must reject");
            assert_offline_rejection(err, "revoked_attestation", "certificate is revoked");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_inactive_policy_root() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let mut policy = default_policy_for_tests();
            policy.trusted_roots.iter_mut().for_each(|root| {
                root.not_before_ms = Some(1);
                root.not_after_ms = Some(2);
            });
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("inactive policy roots must reject");
            assert_offline_rejection(err, "invalid_attestation_policy", "no active trusted root");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_malformed_android_report() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = Some(1);
            registration.attestation_report = b"not-cbor".to_vec();
            refresh_attestation_hashes(&mut registration);
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("malformed Android KeyMint report must reject");
            assert_offline_rejection(err, "invalid_attestation", "CBOR certificate array");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_report_trailing_cbor_bytes() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            registration.attestation_report.push(0xF6);
            refresh_attestation_hashes(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint report with trailing CBOR data must reject");
            assert_offline_rejection(err, "invalid_attestation", "trailing CBOR bytes");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_challenge_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = Some(1);
            refresh_attestation_material(&mut registration);
            registration.device_id.push_str("-after-report");
            refresh_attestation_challenge(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint challenge mismatch must reject");
            assert_offline_rejection(err, "invalid_attestation", "attestation challenge");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_software_security_level() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let mut registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            registration.platform = OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.to_owned();
            registration.assertion_scheme = OFFLINE_ATTESTATION_ANDROID_ASSERTION_SCHEME.to_owned();
            registration.assertion_key_algorithm =
                OFFLINE_ATTESTATION_ANDROID_ASSERTION_ALGORITHM.to_owned();
            registration.assertion_usage_count_limit = Some(1);
            refresh_android_attestation_material(&mut registration, false);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint software security level must reject");
            assert_offline_rejection(err, "invalid_attestation", "hardware-backed");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_missing_application_id() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_options(
                &mut registration,
                true,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                false,
                false,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint attestation without app id must reject");
            assert_offline_rejection(err, "invalid_attestation", "attestationApplicationId");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_leaf_without_keymint_extension()
        {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_without_leaf_extension(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint leaf without attestation extension must reject");
            assert_offline_rejection(err, "invalid_attestation", "leaf certificate");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_keymint_extension() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_duplicate_leaf_extension(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android duplicate KeyMint extensions must reject");
            assert_offline_rejection(
                err,
                "invalid_attestation",
                "duplicate attestation extensions",
            );
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_missing_usage_limit() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_options(
                &mut registration,
                true,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                None,
                false,
                true,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint attestation without usageCountLimit must reject");
            assert_offline_rejection(err, "invalid_attestation", "usageCountLimit");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_all_applications_scope() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_options(
                &mut registration,
                true,
                ANDROID_TEST_PACKAGE_NAME,
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                true,
                true,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android allApplications attestation must reject");
            assert_offline_rejection(err, "invalid_attestation", "all applications");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_application_id_package()
         {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_application_entries(
                &mut registration,
                &[ANDROID_TEST_PACKAGE_NAME, ANDROID_TEST_PACKAGE_NAME],
                &[ANDROID_TEST_SIGNING_CERT_SHA256],
                0,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("duplicate Android package attestation entry must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicates a package name");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_application_id_signing_digest()
         {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_application_entries(
                &mut registration,
                &[ANDROID_TEST_PACKAGE_NAME],
                &[
                    ANDROID_TEST_SIGNING_CERT_SHA256,
                    ANDROID_TEST_SIGNING_CERT_SHA256,
                ],
                0,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("duplicate Android signing digest attestation entry must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicates a signing digest");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_all_applications_tag()
        {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_application_entries(
                &mut registration,
                &[ANDROID_TEST_PACKAGE_NAME],
                &[ANDROID_TEST_SIGNING_CERT_SHA256],
                2,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("duplicate Android allApplications tag must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicates allApplications");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_usage_limit_across_lists()
         {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_split_authorizations(
                &mut registration,
                Some(1),
                Some(1),
                true,
                false,
                0,
                0,
                0,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("usageCountLimit duplicated across authorization lists must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicate usageCountLimit");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_application_id_across_lists()
         {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_split_authorizations(
                &mut registration,
                None,
                Some(1),
                true,
                true,
                0,
                0,
                0,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err(
                    "attestationApplicationId duplicated across authorization lists must reject",
                );
            assert_offline_rejection(
                err,
                "invalid_attestation",
                "duplicate attestationApplicationId",
            );
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_duplicate_all_applications_across_lists()
         {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_split_authorizations(
                &mut registration,
                None,
                Some(1),
                true,
                false,
                1,
                1,
                0,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("allApplications duplicated across authorization lists must reject");
            assert_offline_rejection(err, "invalid_attestation", "duplicate allApplications");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_trailing_key_description_fields()
        {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_split_authorizations(
                &mut registration,
                None,
                Some(1),
                true,
                false,
                0,
                0,
                1,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("KeyDescription with trailing authorization fields must reject");
            assert_offline_rejection(err, "invalid_attestation", "trailing fields");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_package_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_options(
                &mut registration,
                true,
                "jp.co.soramitsu.iroha.attacker",
                &ANDROID_TEST_SIGNING_CERT_SHA256,
                Some(1),
                false,
                true,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android package substitution must reject");
            assert_offline_rejection(err, "invalid_attestation", "package name");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_signing_digest_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_options(
                &mut registration,
                true,
                ANDROID_TEST_PACKAGE_NAME,
                &[0xD4; 32],
                Some(1),
                false,
                true,
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android signing certificate substitution must reject");
            assert_offline_rejection(err, "invalid_attestation", "signing digest");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_key_id_mismatch() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_key_id_override(
                &mut registration,
                "00".repeat(32),
            );
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint key_id substitution must reject");
            assert_offline_rejection(err, "invalid_attestation", "key_id");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_uppercase_key_id() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let mut registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            refresh_android_attestation_material_with_uppercase_key_id(&mut registration);
            let authority = registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android KeyMint uppercase key_id must reject");
            assert_offline_rejection(err, "invalid_attestation", "lowercase hex");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_android_app_disallowed_by_policy() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let mut policy = default_policy_for_tests();
            policy.require_android_app_policy = true;
            policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
                package_name: "jp.co.soramitsu.iroha.other".to_owned(),
                signing_certificate_sha256: vec![ANDROID_TEST_SIGNING_CERT_SHA256.to_vec()],
            }];
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("Android app not allowed by policy must reject");
            assert_offline_rejection(
                err,
                "invalid_attestation_policy",
                "Android KeyMint app identity",
            );
        }

        #[test]
        fn offline_device_attestation_registration_accepts_android_app_allowed_by_policy() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let mut policy = default_policy_for_tests();
            policy.require_android_app_policy = true;
            policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
                package_name: ANDROID_TEST_PACKAGE_NAME.to_owned(),
                signing_certificate_sha256: vec![ANDROID_TEST_SIGNING_CERT_SHA256.to_vec()],
            }];
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect("Android app allowed by policy must be accepted");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_revoked_android_leaf_certificate() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let registration =
                android_attestation_registration(recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let leaf_der = android_report_certificate_der(&registration.attestation_report, 0);
            let mut policy = default_policy_for_tests();
            policy
                .revoked_certificate_sha256
                .push(sha256_bytes(&leaf_der).to_vec());
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            store_attestation_policy_for_tests(&mut transaction, policy);

            let err = RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect_err("revoked Android leaf certificate must reject");
            assert_offline_rejection(err, "revoked_attestation", "certificate is revoked");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_reused_evidence_with_new_key() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let first_certificate = sample_certificate();
            let first_registration = attestation_registration(
                &first_certificate,
                recent_block_height,
                recent_block_hash,
            );
            let mut second_certificate = sample_certificate();
            second_certificate.key_id = "one-use-key-2".to_owned();
            second_certificate.assertion_public_key = {
                let mut public_key = Vec::with_capacity(65);
                public_key.push(0x04);
                public_key.extend_from_slice(&[0x12; 32]);
                public_key.extend_from_slice(&[0x23; 32]);
                public_key
            };
            let mut second_registration = attestation_registration(
                &second_certificate,
                recent_block_height,
                recent_block_hash,
            );
            second_registration.evidence_hash = first_registration.evidence_hash;
            second_registration.evidence = first_registration.evidence.clone();
            refresh_attestation_challenge(&mut second_registration);
            let authority = first_registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            RegisterOfflineDeviceAttestation::new(first_registration)
                .execute(&authority, &mut transaction)
                .expect("first registration records evidence marker");
            let err = RegisterOfflineDeviceAttestation::new(second_registration)
                .execute(&authority, &mut transaction)
                .expect_err("reused evidence must not bind a new key id registration");
            assert_offline_rejection(err, "invalid_attestation", "evidence envelope");
        }

        #[test]
        fn offline_device_attestation_registration_rejects_certificate_evidence_substitution() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let first_registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let mut second_registration = first_registration.clone();
            second_registration.evidence = b"second-evidence".to_vec();
            second_registration.evidence_hash = Hash::new(&second_registration.evidence);
            let authority = first_registration.account_id.clone();
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            RegisterOfflineDeviceAttestation::new(first_registration)
                .execute(&authority, &mut transaction)
                .expect("first registration records certificate marker");
            let err = RegisterOfflineDeviceAttestation::new(second_registration)
                .execute(&authority, &mut transaction)
                .expect_err("same attested certificate cannot be paired with arbitrary evidence");
            assert_offline_rejection(err, "invalid_attestation", "evidence envelope");
        }

        #[test]
        fn on_chain_attested_marker_does_not_authorize_mutated_certificate() {
            let (state, recent_block_height, recent_block_hash) = state_with_attestation_anchor();
            let certificate = sample_certificate();
            let registration =
                attestation_registration(&certificate, recent_block_height, recent_block_hash);
            let authority = registration.account_id.clone();
            let mut mutated_certificate = registration.key_certificate();
            mutated_certificate.key_id.push_str("-mutated");
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            RegisterOfflineDeviceAttestation::new(registration)
                .execute(&authority, &mut transaction)
                .expect("valid attestation registration");
            let err = ensure_offline_note_certificate_authorized(
                &mutated_certificate,
                &authority,
                &transaction,
            )
            .expect_err("mutated certificate payload must not inherit attested marker");
            assert_offline_rejection(err, "invalid_issuer_cert", "signature");
        }

        #[test]
        fn middleware_signed_certificate_authorization_still_works_without_attestation_marker() {
            let state = State::new(
                World::default(),
                Arc::clone(&Kura::blank_kura_for_testing()),
                LiveQueryStore::start_test(),
            );
            let issuer = fixture_key_pair(0x01);
            let account_id = AccountId::new(issuer.public_key().clone());
            let certificate =
                signed_sample_certificate(&issuer, account_id.clone(), 0x66, "middleware-key");
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            ensure_offline_note_certificate_authorized(&certificate, &account_id, &transaction)
                .expect("middleware issuer signature should remain a valid fallback");
        }

        #[test]
        fn duplicate_hashes_are_rejected() {
            let hash = Hash::new(b"duplicate");
            let err = ensure_unique_hashes(&[hash, hash], "duplicate_hash", "duplicate hashes")
                .expect_err("duplicate hash should fail");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::duplicate_hash:"));
        }

        #[test]
        fn audit_output_claim_count_must_match_commitments_one_to_one() {
            let err = ensure_offline_audit_output_claim_count(2, 1)
                .expect_err("hidden audit output commitments must be rejected");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::invalid_proof:"));
            assert!(message.contains("one-to-one"));
        }

        #[test]
        fn audit_output_claims_must_be_ordered_one_to_one_with_commitments() {
            let input = sample_issued_claim();
            let commitment = Hash::new(b"offline-note-commitment-a");
            let claims = [OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-commitment-b"),
                key_certificate: sample_certificate(),
                asset: input.asset,
                amount: Numeric::new(10, 0),
            }];

            let err = ensure_offline_audit_output_claim_binding(&[commitment], &claims)
                .expect_err("mismatched output claim commitment must reject");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::proof_binding:"));
            assert!(message.contains("ordered one-to-one"));
        }

        #[test]
        fn audit_input_claims_must_be_anchored_to_sender_certificate() {
            let claim = sample_issued_claim();
            let mut other_certificate = sample_certificate();
            other_certificate.key_id = "different-one-use-key".to_owned();
            let other_hash = other_certificate.payload_hash().expect("certificate hash");

            let err = ensure_offline_audit_input_claim_anchor(&[claim], &other_hash)
                .expect_err("input claim must match sender certificate hash");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::proof_binding:"));
            assert!(message.contains("sender key certificate"));
        }

        #[test]
        fn audit_public_amounts_must_conserve_one_asset_definition() {
            let input = sample_issued_claim();
            let output = OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-output-note"),
                key_certificate: sample_certificate(),
                asset: input.asset.clone(),
                amount: Numeric::new(9, 0),
            };

            let err = ensure_offline_audit_conserves_asset_amounts(&[input], &[output])
                .expect_err("audit must conserve public input and output amounts");
            assert_offline_rejection(
                err,
                "amount_conservation",
                "input amount total must equal output amount total",
            );
        }

        #[test]
        fn audit_rejects_certificate_anchor_without_topup_issued_claim() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let account_keypair = fixture_key_pair(0x01);
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x78, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let relayer = sample_account(0x71);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("certificate-only lineage must not anchor an input claim");
            assert_offline_rejection(err, "note_not_issued", "input claim was not issued");
        }

        #[test]
        fn audit_rejects_input_claim_without_sender_certificate_topup_anchor() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let account_keypair = fixture_key_pair(0x01);
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x79, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let relayer = sample_account(0x72);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit must require a sender certificate anchored by topup");
            assert_offline_rejection(err, "invalid_issuer_cert", "key certificate was not issued");
        }

        #[test]
        fn audit_rejects_claim_anchor_without_topup_commitment_lineage() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let account_keypair = fixture_key_pair(0x01);
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x7F, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let relayer = sample_account(0x7E);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("claim-only lineage must not anchor an input note commitment");
            assert_offline_rejection(
                err,
                "note_not_issued",
                "input note commitment was not issued",
            );
        }

        #[test]
        fn redeem_rejects_claim_anchor_without_topup_commitment_lineage() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-redeem-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let redemption = sample_redemption_for_issue(&issue, account_id.clone());
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());

            let err = RedeemOfflineNote::new(redemption)
                .execute(&account_id, &mut transaction)
                .expect_err("claim-only lineage must not anchor a redeem source commitment");
            assert_offline_rejection(
                err,
                "note_not_issued",
                "source note commitment was not issued",
            );
        }

        #[test]
        fn redeem_accepts_topup_commitment_anchor_until_proof_verification() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-redeem-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let redemption = sample_redemption_for_issue(&issue, account_id.clone());
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let issued_commitment_key = offline_note_issue_key(&issue.note_commitment);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_commitment_key, ());

            let err = RedeemOfflineNote::new(redemption)
                .execute(&account_id, &mut transaction)
                .expect_err("placeholder proof should reject after lineage admission");
            let message = err.to_string();
            assert!(
                !message.contains("note_not_issued")
                    && (message.contains("verifier_key")
                        || message.contains("invalid_proof")
                        || message.contains("OpenVerifyEnvelope")),
                "unexpected redeem error after topup commitment anchor: {message}"
            );
        }

        #[test]
        fn redeem_rejects_forged_source_commitment_even_when_claim_key_is_anchored() {
            let (state, asset_id, account_id, _definition_id) =
                distinct_escrow_test_state(Numeric::new(100, 0), 0x7C);
            let account_keypair = fixture_key_pair(0x01);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x7A,
                "topup-input-key",
            );
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-redeem-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let mut redemption = sample_redemption_for_issue(&issue, account_id.clone());
            redemption.source_note_commitment = Hash::new(b"offline-redeem-forged-source-note");
            let public_inputs_hash = redemption
                .public_inputs_hash()
                .expect("forged redemption public-input hash");
            redemption.recursive_proof = placeholder_recursive_proof(public_inputs_hash);
            let forged_claim_hash = offline_note_issued_claim_hash(
                OfflineNoteIssuedClaim::from_redemption(&redemption)
                    .expect("forged redemption claim"),
            )
            .expect("forged redemption claim hash");
            let forged_claim_key = offline_note_issued_claim_key(&forged_claim_hash);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.world.account_permissions.insert(
                account_id.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            IssueOfflineNote::new(issue)
                .execute(&account_id, &mut transaction)
                .expect("online-to-offline topup should issue the original source commitment");
            transaction
                .world
                .offline_note_replay_keys
                .insert(forged_claim_key, ());

            let err = RedeemOfflineNote::new(redemption)
                .execute(&account_id, &mut transaction)
                .expect_err("redeem must require the source commitment topup anchor");
            assert_offline_rejection(
                err,
                "note_not_issued",
                "source note commitment was not issued",
            );
        }

        #[test]
        fn audit_rejects_mutated_input_claim_even_when_certificate_topup_is_anchored() {
            let (state, asset_id, account_id, _definition_id) =
                distinct_escrow_test_state(Numeric::new(100, 0), 0x7D);
            let account_keypair = fixture_key_pair(0x01);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x7A,
                "topup-input-key",
            );
            let output_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x7B,
                "audit-output-key",
            );
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let mut audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            audit.input_claims[0].note_commitment = Hash::new(b"offline-forged-topup-note");
            let public_inputs_hash = audit.public_inputs_hash().expect("mutated audit hash");
            audit.recursive_proof = placeholder_recursive_proof(public_inputs_hash);

            let relayer = sample_account(0x7C);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.world.account_permissions.insert(
                account_id.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            IssueOfflineNote::new(issue)
                .execute(&account_id, &mut transaction)
                .expect("online-to-offline topup should issue the original claim");

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit must reject a claim mutation under the issued certificate");
            assert_offline_rejection(err, "note_not_issued", "input claim was not issued");
        }

        #[test]
        fn issue_rejects_note_commitment_reused_from_audit_output() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = fixture_key_pair(0x01);
            let key_certificate =
                signed_sample_certificate(&account_keypair, account_id.clone(), 0x86, "topup-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-reused-audit-output-note"),
                key_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit_output_key = offline_note_audit_output_key(&issue.note_commitment);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.world.account_permissions.insert(
                account_id.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            transaction
                .world
                .offline_note_replay_keys
                .insert(audit_output_key, ());

            let err = IssueOfflineNote::new(issue)
                .execute(&account_id, &mut transaction)
                .expect_err("topup issue must not reuse a prior audit output commitment");
            assert_offline_rejection(err, "duplicate_issue", "commitment is already issued");
        }

        #[test]
        fn audit_rejects_output_commitment_reused_from_topup_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = fixture_key_pair(0x01);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x87,
                "topup-input-key",
            );
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x88, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let issued_input_commitment_key = offline_note_issue_key(&issue.note_commitment);
            let issued_output_commitment_key =
                offline_note_issue_key(audit.output_commitments.first().expect("output"));
            let relayer = sample_account(0x89);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_input_commitment_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_output_commitment_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit output must not reuse a topup note commitment");
            assert_offline_rejection(
                err,
                "duplicate_issue",
                "output commitment is already issued",
            );
        }

        #[test]
        fn audit_rejects_reused_output_certificate_from_topup_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = fixture_key_pair(0x01);
            let input_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x84, "topup-input-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, input_certificate.clone());
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let issued_input_commitment_key = offline_note_issue_key(&issue.note_commitment);
            let relayer = sample_account(0x85);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_input_commitment_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit output must not reuse an issued topup key certificate");
            assert_offline_rejection(
                err,
                "duplicate_key_certificate",
                "output key certificate is already issued",
            );
        }

        #[test]
        fn audit_rejects_output_certificate_signature_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = fixture_key_pair(0x01);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x80,
                "topup-input-key",
            );
            let mut output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x81, "audit-output-key");
            output_certificate.issuer_signature = sample_signature(0x82);
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let relayer = sample_account(0x83);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audited output certificate signatures must be verified");
            assert_offline_rejection(
                err,
                "invalid_issuer_cert",
                "signature does not match issuer account",
            );
        }

        #[test]
        fn audit_rejects_nullifier_output_overlap_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = fixture_key_pair(0x01);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x84,
                "topup-input-key",
            );
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x85, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let mut audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            audit.output_commitments[0] = audit.input_nullifiers[0];
            audit.output_claims[0].note_commitment = audit.input_nullifiers[0];
            audit.recursive_proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
            let relayer = sample_account(0x86);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("offline audit input/output overlap must reject before proof decode");
            assert_offline_rejection(
                err,
                "proof_binding",
                "output commitments must be disjoint from input nullifiers",
            );
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        fn audit_accepts_independent_relayer_when_topup_claim_is_anchored() {
            let issuer = fixture_key_pair(0x73);
            let authority = AccountId::new(issuer.public_key().clone());
            let relayer = sample_account(0x74);
            let escrow_account_id = sample_account(0x77);
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "xor".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), authority.clone());
            let escrow_asset_id = AssetId::new(definition_id.clone(), escrow_account_id.clone());
            let domain = Domain::new(domain_id).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let relayer_account = Account::new(relayer.clone()).build(&authority);
            let escrow_account = Account::new(escrow_account_id.clone()).build(&authority);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("xor".to_owned())
                    .build(&authority);
            let asset = Asset::new(asset_id.clone(), Numeric::new(100, 0));
            let escrow_asset = Asset::new(escrow_asset_id, Numeric::zero());
            let verifier_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            );
            let verifier_record =
                crate::zk::offline_note_recursive_vk_record(OFFLINE_NOTE_VERIFIER_NAMESPACE, 1)
                    .expect("offline recursive verifier record");
            let verifier_key = verifier_record.key.clone().expect("inline verifier key");
            let mut world = World::with_assets(
                [domain],
                [account, relayer_account, escrow_account],
                [asset_definition],
                [asset, escrow_asset],
                [],
            );
            world.account_permissions.insert(
                authority.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            world
                .verifying_keys
                .insert(verifier_id.clone(), verifier_record);
            world.verifying_keys_by_circuit.insert(
                (crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID.to_owned(), 1),
                verifier_id.clone(),
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id, escrow_account_id);
            let mut zk = state.zk.clone();
            zk.halo2.enabled = true;
            zk.halo2.max_envelope_bytes = usize::MAX;
            zk.halo2.max_proof_bytes = usize::MAX;
            state.set_zk(zk);

            let input_certificate =
                signed_sample_certificate(&issuer, authority.clone(), 0x75, "topup-input-key");
            let output_certificate =
                signed_sample_certificate(&issuer, authority.clone(), 0x76, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id.clone(),
                amount: Numeric::new(10, 0),
            };
            let mut audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let proving_key =
                crate::zk::derive_halo2_ipa_offline_note_proving_key_bytes(&verifier_key)
                    .expect("offline recursive proving key");
            let audit_hash = audit.public_inputs_hash().expect("audit public-input hash");
            let proof = crate::zk::prove_offline_note_audit(
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                &verifier_key,
                &audit,
                Some(&proving_key),
            )
            .expect("real offline audit proof");
            audit.recursive_proof = OfflineNoteRecursiveProof {
                verifier_key_id: verifier_id.clone(),
                public_inputs_hash: audit_hash,
                proof,
            };

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            IssueOfflineNote::new(issue)
                .execute(&authority, &mut transaction)
                .expect("online-to-offline topup anchors input claim");
            AuditOfflineNote::new(audit.clone())
                .execute(&relayer, &mut transaction)
                .expect("bearer audit lineage may be submitted by an independent relayer");

            let output_claim = OfflineNoteIssuedClaim::from_audit_output(
                audit.output_claims.first().expect("output claim"),
            )
            .expect("issued output claim");
            let output_claim_hash =
                offline_note_issued_claim_hash(output_claim.clone()).expect("output claim hash");
            let output_claim_key = offline_note_issued_claim_key(&output_claim_hash);
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&output_claim_key)
                    .is_some(),
                "audit should anchor output claim lineage for later redemption"
            );
            let output_commitment_key =
                offline_note_issue_key(audit.output_commitments.first().expect("output"));
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&output_commitment_key)
                    .is_some(),
                "audit should anchor output commitment lineage for later offline-offline hops"
            );
            let first_audit_output_claim = audit.output_claims.first().expect("output claim");
            let second_output_certificate =
                signed_sample_certificate(&issuer, authority.clone(), 0x79, "audit-output-key-2");
            let second_output_commitment = Hash::new(b"offline-audit-output-note-2");
            let mut second_audit = OfflineNoteAuditBundle {
                token_id: Hash::new(b"offline-audit-token-2"),
                sender_key_certificate: first_audit_output_claim.key_certificate.clone(),
                input_nullifiers: vec![Hash::new(b"offline-audit-input-nullifier-2")],
                input_claims: vec![output_claim],
                output_commitments: vec![second_output_commitment],
                output_claims: vec![OfflineNoteAuditOutputClaim {
                    note_commitment: second_output_commitment,
                    key_certificate: second_output_certificate,
                    asset: asset_id.clone(),
                    amount: Numeric::new(10, 0),
                }],
                recursive_proof: placeholder_recursive_proof(Hash::new(
                    b"offline-placeholder-public-inputs-2",
                )),
            };
            let second_audit_hash = second_audit
                .public_inputs_hash()
                .expect("second audit public-input hash");
            let second_proof = crate::zk::prove_offline_note_audit(
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                &verifier_key,
                &second_audit,
                Some(&proving_key),
            )
            .expect("real second-hop offline audit proof");
            second_audit.recursive_proof = OfflineNoteRecursiveProof {
                verifier_key_id: verifier_id,
                public_inputs_hash: second_audit_hash,
                proof: second_proof,
            };
            AuditOfflineNote::new(second_audit.clone())
                .execute(&relayer, &mut transaction)
                .expect("prior audit output should anchor the next offline-offline hop");
            let spent_output_claim_key = offline_note_spent_claim_key(&output_claim_hash);
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&spent_output_claim_key)
                    .is_some(),
                "second audit should consume the first audit output claim"
            );
            let second_output_claim = OfflineNoteIssuedClaim::from_audit_output(
                second_audit
                    .output_claims
                    .first()
                    .expect("second output claim"),
            )
            .expect("second issued output claim");
            let second_output_claim_hash =
                offline_note_issued_claim_hash(second_output_claim).expect("second claim hash");
            let second_output_claim_key = offline_note_issued_claim_key(&second_output_claim_hash);
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&second_output_claim_key)
                    .is_some(),
                "second audit should anchor the next output claim lineage"
            );
            let second_output_commitment_key = offline_note_issue_key(
                second_audit
                    .output_commitments
                    .first()
                    .expect("second output commitment"),
            );
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&second_output_commitment_key)
                    .is_some(),
                "second audit should anchor the next output commitment lineage"
            );
            let balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::new(90, 0));
        }

        #[test]
        fn offline_note_rejects_non_open_verify_envelope_proof_bytes() {
            let (state, mut proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            proof.proof.bytes = b"legacy transcript payload".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("legacy transcript bytes must not decode as OpenVerifyEnvelope");
            assert_offline_rejection(err, "invalid_proof", "OpenVerifyEnvelope");
        }

        #[test]
        fn offline_note_rejects_wrong_verifier_key_id_backend() {
            let (state, mut proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            proof.verifier_key_id =
                VerifyingKeyId::new("stark/fri", crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("proof backend must match the selected verifier key id");
            assert_offline_rejection(err, "verifier_key_invalid", "backend");
        }

        #[test]
        fn offline_note_rejects_non_transparent_proof_backends() {
            for backend in [
                "halo2/pasta",
                "stark/fri/",
                "halo2/unknown-native-v1",
                "halo2/ipa:tiny-add-public",
                "halo2/pasta/tiny-add",
                "halo2/pasta/ivm-execution-v2",
                "halo2/pasta/unknown-native-v1",
            ] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                proof.verifier_key_id =
                    VerifyingKeyId::new(backend, crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
                proof.proof.backend = backend.to_owned();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline proofs must not accept trusted-setup backend labels");
                assert_offline_rejection(err, "verifier_key_invalid", "transparent");
            }
        }

        #[test]
        fn offline_note_rejects_non_production_backend_labels_before_registry_lookup() {
            for backend in [
                "groth16/bn254",
                "halo2/kzg",
                "halo2/ipa:KZG",
                "halo2/ipa: KZG",
                "halo2/bn254",
                "debug-proof",
                "Debug-Proof",
                "D-e-b-u-g-Proof",
                "mock-proof",
                "Mock-Proof",
                "M-o-c-k-Proof",
                "stark/fri/debug-proof",
                "stark/fri/Debug-Proof",
                "stark/fri/d-e-b-u-g-proof",
                "stark/fri/mock-proof",
                "stark/fri/m-o-c-k-proof",
                "halo2/ipa:mock-proof",
                "halo2/ipa:Mock-Proof",
                "halo2/ipa:m-o-c-k-proof",
            ] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                proof.verifier_key_id =
                    VerifyingKeyId::new(backend, crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
                proof.proof.backend = backend.to_owned();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline proofs must not accept non-production backend labels");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "trusted-setup or developer-only",
                );
            }
        }

        #[test]
        fn offline_note_rejects_pending_production_backend_labels_before_registry_lookup() {
            for backend in PENDING_PRODUCTION_BACKEND_LABELS {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                proof.verifier_key_id =
                    VerifyingKeyId::new(backend, crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
                proof.proof.backend = backend.to_owned();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline proofs must not accept pending-production backends");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "pending-production proof backends",
                );
            }
        }

        #[test]
        fn offline_note_rejects_production_claim_backend_labels_before_registry_lookup() {
            for backend in [
                "halo2/ipa:production-ready",
                "halo2/ipa:mainnet-ready",
                "halo2/ipa/orchard:production-ready",
                "orchard:mainnet-ready",
                "penumbra-masp:external-security-review",
                "jindo-lattice-pcs-zk:release-ready",
                "stark/fri/audit-signoff",
                "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
                "stark/fri/a-u-d-i-t-c-l-a-i-m",
                "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            ] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                proof.verifier_key_id =
                    VerifyingKeyId::new(backend, crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
                proof.proof.backend = backend.to_owned();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline proofs must not accept production-claim backends");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "production-claim proof backends",
                );
            }
        }

        #[test]
        fn offline_note_rejects_transparent_backend_tag_mismatch() {
            let (mut state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            let verifier_id = proof.verifier_key_id.clone();
            let vk_box = VerifyingKeyBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                b"offline-note-test-verifying-key".to_vec(),
            );
            let commitment = crate::zk::hash_vk(&vk_box);
            let mut record = VerifyingKeyRecord::new_with_owner(
                1,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                None,
                OFFLINE_NOTE_VERIFIER_NAMESPACE,
                BackendTag::Halo2IpaPasta,
                "pasta",
                offline_note_recursive_public_inputs_schema_hash(),
                commitment,
            );
            record.key = Some(vk_box);
            record.status = ConfidentialStatus::Active;
            record.max_proof_bytes = 4096;
            record.vk_len = b"offline-note-test-verifying-key".len() as u32;
            record.backend = BackendTag::Halo2Bn254;
            state.world.verifying_keys.insert(verifier_id, record);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("offline verifier backend tag must match backend label");
            assert_offline_rejection(err, "verifier_key_invalid", "backend tag");
        }

        #[test]
        fn offline_note_rejects_pending_production_verifier_record_tags() {
            for backend_tag in PENDING_PRODUCTION_BACKEND_TAGS {
                assert_offline_note_record_mutation_rejects(
                    move |state, verifier_id, _proof| {
                        mutate_verifier_record(state, verifier_id, |record| {
                            record.backend = backend_tag;
                        });
                    },
                    "verifier_key_invalid",
                    "pending-production backend tags",
                );
            }
        }

        #[test]
        fn offline_note_rejects_recursive_envelope_unbound_metadata() {
            let cases: [(&str, fn(&mut OpenVerifyEnvelope)); 2] = [
                ("non-zero", |envelope| {
                    envelope.vk_hash = [0u8; 32];
                }),
                ("empty auxiliary bytes", |envelope| {
                    envelope.aux = b"offline-note-forged-aux".to_vec();
                }),
            ];

            for (detail, mutate) in cases {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                mutate_offline_note_recursive_envelope(&mut proof, mutate);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline recursive envelope metadata substitution must reject");
                assert_offline_rejection(err, "invalid_proof", detail);
            }
        }

        #[test]
        fn offline_note_rejects_empty_recursive_verifier_key_id_name() {
            let (state, mut proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            proof.verifier_key_id.name = "   ".to_owned();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("empty Offline recursive verifier-key id name must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "verifier key id name must be non-empty",
            );
        }

        #[test]
        fn offline_note_rejects_self_consistent_noncanonical_recursive_circuit() {
            for forged_circuit in [
                "halo2/ipa:offline-note-recursive",
                "halo2/ipa/offline-note-recursive",
                "halo2/pasta/offline-note-recursive",
                "halo2/pasta/ipa/offline-note-recursive",
                "halo2/ipa:offline-note-recursive-shadow",
            ] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                mutate_offline_note_recursive_envelope(&mut proof, |envelope| {
                    envelope.circuit_id = forged_circuit.to_owned();
                });
                let verifier_id = proof.verifier_key_id.clone();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                {
                    let mut mutation_transaction = block.transaction();
                    let mut record = mutation_transaction
                        .world
                        .verifying_keys
                        .get(&verifier_id)
                        .expect("verifier record")
                        .clone();
                    mutation_transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id.clone(), record.version));
                    record.circuit_id = forged_circuit.to_owned();
                    mutation_transaction.world.verifying_keys_by_circuit.insert(
                        (record.circuit_id.clone(), record.version),
                        verifier_id.clone(),
                    );
                    mutation_transaction
                        .world
                        .verifying_keys
                        .insert(verifier_id.clone(), record);
                    mutation_transaction.apply();
                }
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction).expect_err(
                    "self-consistent noncanonical Offline recursive circuit must reject",
                );
                assert_offline_rejection(err, "verifier_key_invalid", "offline-note-recursive");
            }
        }

        #[test]
        fn offline_note_rejects_recursive_verifier_record_mismatches() {
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.key = None;
                    });
                },
                "verifier_key_invalid",
                "not available inline",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        let empty_key = VerifyingKeyBox::new(
                            record.key.as_ref().expect("key").backend.clone(),
                            Vec::new(),
                        );
                        record.commitment = crate::zk::hash_vk(&empty_key);
                        record.vk_len = 0;
                        record.key = Some(empty_key);
                    });
                },
                "verifier_key_invalid",
                "key bytes must be non-empty",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.vk_len += 1;
                    });
                },
                "verifier_key_invalid",
                "key length",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.max_proof_bytes = 0;
                    });
                },
                "verifier_key_invalid",
                "max_proof_bytes",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.max_proof_bytes =
                            u32::try_from(proof.proof.bytes.len().saturating_sub(1))
                                .expect("proof length fits u32");
                    });
                },
                "invalid_proof",
                "max_proof_bytes",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.namespace = "not_offline_note".to_owned();
                    });
                },
                "verifier_schema_mismatch",
                "Offline namespace",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.public_inputs_schema_hash = [0xA5; 32];
                    });
                },
                "verifier_schema_mismatch",
                "public-input schema",
            );
            assert_offline_note_record_mutation_rejects(
                |transaction, verifier_id, _proof| {
                    let record = transaction
                        .world
                        .verifying_keys
                        .get(verifier_id)
                        .expect("verifier record")
                        .clone();
                    transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id, record.version));
                },
                "verifier_key_inactive",
                "circuit/version",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.commitment = [0x77; 32];
                    });
                },
                "verifier_key_invalid",
                "commitment",
            );
            #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        let mut noncanonical_key = crate::zk::offline_note_recursive_vk_box()
                            .expect("Offline recursive key");
                        let last = noncanonical_key
                            .bytes
                            .last_mut()
                            .expect("Offline recursive key bytes");
                        *last ^= 0x01;
                        record.commitment = crate::zk::hash_vk(&noncanonical_key);
                        record.vk_len = u32::try_from(noncanonical_key.bytes.len())
                            .expect("Offline recursive verifier key length fits u32");
                        record.key = Some(noncanonical_key);
                    });
                },
                "verifier_key_invalid",
                "canonical semantic circuit key",
            );
        }

        #[test]
        fn offline_note_rejects_inactive_verifier_key() {
            let (state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Proposed);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("inactive Offline verifier keys must reject proofs");
            assert_offline_rejection(err, "verifier_key_inactive", "not active");
        }

        #[test]
        fn offline_note_redeem_and_audit_reject_public_input_hash_mismatch() {
            let (state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let redeem_err = verify_offline_note_recursive_proof(
                &proof,
                &Hash::new(b"offline-note-wrong-redeem-inputs"),
                Vec::new(),
                &mut transaction,
            )
            .expect_err("redeem proof must be bound to the expected public inputs");
            assert_offline_rejection(redeem_err, "proof_binding", "expected public inputs");

            let audit_err = verify_offline_note_recursive_proof(
                &proof,
                &Hash::new(b"offline-note-wrong-audit-inputs"),
                Vec::new(),
                &mut transaction,
            )
            .expect_err("audit proof must be bound to the expected public inputs");
            assert_offline_rejection(audit_err, "proof_binding", "expected public inputs");
        }

        #[test]
        fn kagemusha_transfer_rejects_disabled_config() {
            let authority = sample_account(0x41);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), Arc::clone(&kura), query);
            state.settlement.offline.kagemusha_enabled = false;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA)
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha config gate must reject");
            assert_offline_rejection(err, "kagemusha_disabled", "Kagemusha");
        }

        #[test]
        fn kagemusha_transfer_rejects_trusted_setup_backend_labels() {
            let authority = sample_account(0x42);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            for backend in [
                "halo2/pasta",
                "halo2/ipa:KZG",
                "halo2/ipa: KZG",
                "halo2/ipa:Mock-Proof",
                "halo2/ipa:M-o-c-k-Proof",
                "halo2/ipa:tiny-add-public",
                "halo2/pasta/tiny-add",
                "stark/fri/d-e-b-u-g",
            ] {
                let err = sample_kagemusha_transfer(backend)
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha must reject non-transparent proof backends");
                assert_offline_rejection(err, "verifier_key_invalid", "transparent");
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_unbound_asset_verifier() {
            let authority = sample_account(0x43);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA)
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha must require an asset-bound verifier");
            assert_offline_rejection(err, "verifier_key_invalid", "configured shielded asset");
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_executes_real_confidential_transfer_v2_proof() {
            let (
                state,
                authority,
                definition_id,
                transfer,
                expected_commitments,
                expected_new_roots,
            ) = real_kagemusha_test_state();
            let input = transfer.inputs[0];
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            transfer
                .execute(&authority, &mut transaction)
                .expect("real Halo2 IPA Kagemusha transfer should execute");

            let shielded_state = transaction
                .world
                .zk_assets
                .get(&definition_id)
                .expect("Kagemusha transfer must create shielded asset state");
            assert!(
                shielded_state.nullifiers.contains(&input),
                "input nullifier should be recorded as spent"
            );
            assert_eq!(shielded_state.commitments, expected_commitments);
            assert_eq!(
                shielded_state.root_history.last().copied(),
                expected_new_roots.last().copied(),
                "final confidential root should be recorded after appending outputs"
            );
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_tampered_real_halo2_ipa_proof() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            tamper_open_verify_envelope_inner_proof_byte(&mut transfer.proof.proof.bytes);
            transfer.proof.envelope_hash = Some(Hash::new(&transfer.proof.proof.bytes).into());
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("tampered real Kagemusha proof must reject");
            let message = err.to_string();
            assert!(
                message.contains("invalid transfer proof")
                    || message.contains("OpenVerifyEnvelope")
                    || message.contains("invalid OpenVerifyEnvelope payload")
                    || message.contains("invalid confidential transfer v2 public inputs"),
                "unexpected error: {message}"
            );
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_trust_flag_rejects_tampered_real_halo2_ipa_proof() {
            let (state, authority, definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            let input = transfer.inputs[0];
            let output = transfer.outputs[0];
            tamper_open_verify_envelope_inner_proof_byte(&mut transfer.proof.proof.bytes);
            transfer.proof.envelope_hash = Some(Hash::new(&transfer.proof.proof.bytes).into());
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            block.trust_committed_execution_results = true;
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("committed-result trust must not bypass invalid transfer proof");
            let message = err.to_string();
            assert!(
                message.contains("invalid transfer proof"),
                "unexpected error: {message}"
            );
            let shielded_state = transaction
                .world
                .zk_assets
                .get(&definition_id)
                .expect("Kagemusha transfer keeps shielded state");
            assert!(
                !shielded_state.nullifiers.contains(&input),
                "invalid trusted replay must not consume nullifier"
            );
            assert!(
                !shielded_state.commitments.contains(&output),
                "invalid trusted replay must not append output commitment"
            );
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_semantic_recursive_spend_before_mint() {
            let (state, authority, recipient, definition_id, instruction) =
                real_recursive_kagemusha_redeem_test_state();
            let spend_nullifier = instruction.bundle.accumulator.current_note.spend_nullifier;
            let topup_anchor_nullifiers = instruction
                .bundle
                .accumulator
                .topup_anchor_nullifiers
                .clone();
            let recipient_asset_id = AssetId::new(definition_id.clone(), recipient);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = instruction
                .execute(&authority, &mut transaction)
                .expect_err("semantic recursive Kagemusha spend proof must not mint");
            assert_offline_rejection(err, "invalid_recursive_bundle", "private-hop lineage");

            let shielded_state = transaction
                .world
                .zk_assets
                .get(&definition_id)
                .expect("recursive redeem keeps shielded asset state");
            assert!(
                !shielded_state.nullifiers.contains(&spend_nullifier),
                "rejected recursive redeem must not consume the final spendable note nullifier"
            );
            for nullifier in topup_anchor_nullifiers {
                assert!(
                    !shielded_state.nullifiers.contains(&nullifier),
                    "rejected recursive redeem must not consume the top-up anchor nullifier"
                );
            }
            let balance = transaction
                .world
                .assets
                .get(&recipient_asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::zero());
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_record_backed_multi_hop_mints_and_rejects_replay() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let fixture = real_recursive_kagemusha_redeem_record_backed_multi_hop_fixture();
                let spend_nullifier = fixture
                    .instruction
                    .bundle
                    .accumulator
                    .current_note
                    .spend_nullifier;
                let topup_anchor_nullifiers = fixture
                    .instruction
                    .bundle
                    .accumulator
                    .topup_anchor_nullifiers
                    .clone();
                let replay_instruction = fixture.instruction.clone();
                let recipient_asset_id =
                    AssetId::new(fixture.definition_id.clone(), fixture.recipient.clone());
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = fixture.state.block(header);
                {
                    let mut transaction = block.transaction();
                    fixture
                        .instruction
                        .execute(&fixture.authority, &mut transaction)
                        .expect("record-backed multi-hop recursive Kagemusha redeem should mint");

                    let shielded_state = transaction
                        .world
                        .zk_assets
                        .get(&fixture.definition_id)
                        .expect("recursive redeem keeps shielded asset state");
                    assert!(
                        shielded_state.nullifiers.contains(&spend_nullifier),
                        "redeem must consume the final spendable note nullifier"
                    );
                    for nullifier in &topup_anchor_nullifiers {
                        assert!(
                            shielded_state.nullifiers.contains(nullifier),
                            "redeem must consume every top-up anchor nullifier"
                        );
                    }
                    let balance = transaction
                        .world
                        .assets
                        .get(&recipient_asset_id)
                        .map(|asset| asset.as_ref().clone())
                        .unwrap_or_else(Numeric::zero);
                    assert_eq!(balance, Numeric::new(42, 0));
                    transaction.apply();
                }

                let mut replay_transaction = block.transaction();
                let err = replay_instruction
                    .execute(&fixture.authority, &mut replay_transaction)
                    .expect_err(
                        "record-backed multi-hop recursive Kagemusha redeem must not replay",
                    );
                assert_offline_rejection(err, "duplicate_nullifier", "already spent");
                let balance = replay_transaction
                    .world
                    .assets
                    .get(&recipient_asset_id)
                    .map(|asset| asset.as_ref().clone())
                    .unwrap_or_else(Numeric::zero);
                assert_eq!(balance, Numeric::new(42, 0));
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_reserved_lineage_profile_verifies_backend_before_mint() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let (mut state, authority, recipient, definition_id, mut instruction) =
                    real_recursive_kagemusha_redeem_test_state();
                install_reserved_lineage_spend_profile_for_tests(&mut state, &mut instruction);
                let spend_nullifier = instruction.bundle.accumulator.current_note.spend_nullifier;
                let topup_anchor_nullifiers = instruction
                    .bundle
                    .accumulator
                    .topup_anchor_nullifiers
                    .clone();
                let recipient_asset_id = AssetId::new(definition_id.clone(), recipient);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = instruction
                    .execute(&authority, &mut transaction)
                    .expect_err("reserved lineage recursive Kagemusha spend proof must verify verifier-record backend before mint");
                // one-hop verifier-slice coverage marker for the Reserved-lineage policy guard.
                assert_offline_rejection(
                    err,
                    "invalid_recursive_bundle",
                    "missing verifier-slice public instance columns",
                );

                let shielded_state = transaction
                    .world
                    .zk_assets
                    .get(&definition_id)
                    .expect("recursive redeem keeps shielded asset state");
                assert!(
                    !shielded_state.nullifiers.contains(&spend_nullifier),
                    "rejected lineage recursive redeem must not consume the final spendable note nullifier"
                );
                for nullifier in topup_anchor_nullifiers {
                    assert!(
                        !shielded_state.nullifiers.contains(&nullifier),
                        "rejected lineage recursive redeem must not consume the top-up anchor nullifier"
                    );
                }
                let balance = transaction
                    .world
                    .assets
                    .get(&recipient_asset_id)
                    .map(|asset| asset.as_ref().clone())
                    .unwrap_or_else(Numeric::zero);
                assert_eq!(balance, Numeric::zero());
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_reserved_lineage_checks_final_proof_before_gate() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let (mut state, authority, _, _, mut instruction) =
                    real_recursive_kagemusha_redeem_test_state();
                install_reserved_lineage_spend_profile_for_tests(&mut state, &mut instruction);
                mutate_proof_attachment_envelope(&mut instruction.redeem_proof, |envelope| {
                    *envelope
                        .proof_bytes
                        .last_mut()
                        .expect("recursive redeem proof public instances") ^= 0x01;
                });
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = instruction
                    .execute(&authority, &mut transaction)
                    .expect_err("reserved lineage profile must still validate final proof binding");
                assert_offline_rejection(err, "final_commitment_mismatch", "final spendable note");
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_double_spend_disabled_and_stale_roots() {
            let (state, authority, _recipient, definition_id, instruction) =
                real_recursive_kagemusha_redeem_test_state();
            let spend_nullifier = instruction.bundle.accumulator.current_note.spend_nullifier;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation = block.transaction();
                mutation
                    .world_mut_for_testing()
                    .zk_assets_mut_for_testing()
                    .get_mut(&definition_id)
                    .expect("recursive Kagemusha shielded state")
                    .nullifiers
                    .insert(spend_nullifier);
                mutation.apply();
            }
            let mut transaction = block.transaction();
            let err = instruction
                .execute(&authority, &mut transaction)
                .expect_err("recursive Kagemusha redeem must reject spent final note");
            assert_offline_rejection(err, "duplicate_nullifier", "current note");

            let (mut disabled_state, disabled_authority, _, _, disabled_instruction) =
                real_recursive_kagemusha_redeem_test_state();
            disabled_state.settlement.offline.kagemusha_enabled = false;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = disabled_state.block(header);
            let mut transaction = block.transaction();
            let err = disabled_instruction
                .execute(&disabled_authority, &mut transaction)
                .expect_err("disabled recursive Kagemusha must reject");
            assert_offline_rejection(err, "kagemusha_disabled", "disabled");

            let (stale_state, stale_authority, _, stale_definition_id, stale_instruction) =
                real_recursive_kagemusha_redeem_test_state();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = stale_state.block(header);
            {
                let mut mutation = block.transaction();
                mutation
                    .world_mut_for_testing()
                    .zk_assets_mut_for_testing()
                    .get_mut(&stale_definition_id)
                    .expect("recursive Kagemusha shielded state")
                    .root_history
                    .clear();
                mutation.apply();
            }
            let mut transaction = block.transaction();
            let err = stale_instruction
                .execute(&stale_authority, &mut transaction)
                .expect_err("stale recursive Kagemusha root must reject");
            assert_offline_rejection(err, "stale_root", "stale or unknown");

            let (anchor_state, anchor_authority, _, anchor_definition_id, anchor_instruction) =
                real_recursive_kagemusha_redeem_test_state();
            let topup_anchor = anchor_instruction
                .bundle
                .accumulator
                .topup_anchor_nullifiers[0];
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = anchor_state.block(header);
            {
                let mut mutation = block.transaction();
                mutation
                    .world_mut_for_testing()
                    .zk_assets_mut_for_testing()
                    .get_mut(&anchor_definition_id)
                    .expect("recursive Kagemusha shielded state")
                    .nullifiers
                    .insert(topup_anchor);
                mutation.apply();
            }
            let mut transaction = block.transaction();
            let err = anchor_instruction
                .execute(&anchor_authority, &mut transaction)
                .expect_err("recursive Kagemusha redeem must reject reused top-up anchor");
            assert_offline_rejection(err, "duplicate_nullifier", "top-up anchor");
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn assert_recursive_redeem_rejection_preserves_state(
            fixture: RecursiveKagemushaRedeemTestFixture,
            label: &str,
            detail: &str,
        ) {
            let RecursiveKagemushaRedeemTestFixture {
                state,
                authority,
                recipient,
                definition_id,
                instruction,
                ..
            } = fixture;
            let spend_nullifier = instruction.bundle.accumulator.current_note.spend_nullifier;
            let topup_anchor_nullifiers = instruction
                .bundle
                .accumulator
                .topup_anchor_nullifiers
                .clone();
            let recipient_asset_id = AssetId::new(definition_id.clone(), recipient);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = instruction
                .execute(&authority, &mut transaction)
                .expect_err("recursive Kagemusha redeem mutation must reject");
            assert_offline_rejection(err, label, detail);

            let shielded_state = transaction
                .world
                .zk_assets
                .get(&definition_id)
                .expect("recursive redeem keeps shielded asset state");
            assert!(
                !shielded_state.nullifiers.contains(&spend_nullifier),
                "rejected recursive redeem must not consume the final spendable note nullifier"
            );
            for nullifier in topup_anchor_nullifiers {
                assert!(
                    !shielded_state.nullifiers.contains(&nullifier),
                    "rejected recursive redeem must not consume a top-up anchor nullifier"
                );
            }
            let balance = transaction
                .world
                .assets
                .get(&recipient_asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::zero());
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn assert_recursive_redeem_lineage_record_case_rejects(
            mutate_witness: impl FnOnce(
                &mut KagemushaRecursiveSpendLineageWitnessV1,
                &VerifyingKeyId,
                &VerifyingKeyRecord,
            ),
            mutate_state: impl FnOnce(&mut State, &VerifyingKeyId, &VerifyingKeyRecord),
            detail: &str,
        ) {
            let mut fixture = real_recursive_kagemusha_redeem_test_fixture();
            let mut witness = fixture.lineage_witness.clone();
            mutate_witness(
                &mut witness,
                &fixture.lineage_record_id,
                &fixture.lineage_record,
            );
            mutate_state(
                &mut fixture.state,
                &fixture.lineage_record_id,
                &fixture.lineage_record,
            );
            fixture.instruction.lineage_witness = Some(witness);
            assert_recursive_redeem_rejection_preserves_state(
                fixture,
                "invalid_recursive_lineage",
                detail,
            );
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_adversarial_lineage_verifier_records_before_mint() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                assert_recursive_redeem_lineage_record_case_rejects(
                    |_, _, _| {},
                    |_, _, _| {},
                    "not registered",
                );

                assert_recursive_redeem_lineage_record_case_rejects(
                    |_, _, _| {},
                    |state, id, record| {
                        let mut stale_record = record.clone();
                        stale_record.version = stale_record.version.saturating_add(1);
                        state.world.verifying_keys.insert(id.clone(), stale_record);
                    },
                    "does not match the registered record",
                );

                assert_recursive_redeem_lineage_record_case_rejects(
                    |witness, _, _| {
                        witness.record_bundle.verifier_records.clear();
                    },
                    |state, id, record| {
                        state
                            .world
                            .verifying_keys
                            .insert(id.clone(), record.clone());
                    },
                    "is missing",
                );

                assert_recursive_redeem_lineage_record_case_rejects(
                    |witness, _, _| {
                        let duplicate = witness.record_bundle.verifier_records[0].clone();
                        witness.record_bundle.verifier_records.push(duplicate);
                    },
                    |state, id, record| {
                        state
                            .world
                            .verifying_keys
                            .insert(id.clone(), record.clone());
                    },
                    "is duplicated",
                );

                assert_recursive_redeem_lineage_record_case_rejects(
                    |witness, _, _| {
                        let mut extra = witness.record_bundle.verifier_records[0].clone();
                        extra.id = VerifyingKeyId::new(
                            crate::zk::ZK_BACKEND_HALO2_IPA,
                            "unreferenced-recursive-kagemusha-lineage-record",
                        );
                        witness.record_bundle.verifier_records.push(extra);
                    },
                    |state, id, record| {
                        state
                            .world
                            .verifying_keys
                            .insert(id.clone(), record.clone());
                    },
                    "not referenced",
                );
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_malformed_lineage_hop_proof_before_mint() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let mut fixture = real_recursive_kagemusha_redeem_test_fixture();
                fixture.state.world.verifying_keys.insert(
                    fixture.lineage_record_id.clone(),
                    fixture.lineage_record.clone(),
                );
                let mut witness = fixture.lineage_witness.clone();
                witness.record_bundle.bundle.steps[0].attachment.proof.bytes =
                    b"not-a-valid-kagemusha-lineage-hop-proof".to_vec();
                fixture.instruction.lineage_witness = Some(witness);

                assert_recursive_redeem_rejection_preserves_state(
                    fixture,
                    "invalid_recursive_lineage",
                    "OpenVerifyEnvelope",
                );
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_lineage_final_nullifier_collisions_before_mint() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let mut input_collision = real_recursive_kagemusha_redeem_test_fixture();
                input_collision.state.world.verifying_keys.insert(
                    input_collision.lineage_record_id.clone(),
                    input_collision.lineage_record.clone(),
                );
                let mut input_collision_witness = input_collision.lineage_witness.clone();
                let final_note_index = input_collision_witness
                    .current_notes
                    .len()
                    .checked_sub(1)
                    .expect("recursive lineage fixture has a final note");
                let topup_anchor =
                    input_collision_witness.record_bundle.bundle.steps[0].input_nullifiers[0];
                input_collision_witness.current_notes[final_note_index].spend_nullifier =
                    topup_anchor;
                input_collision.instruction.lineage_witness = Some(input_collision_witness);
                assert_recursive_redeem_rejection_preserves_state(
                    input_collision,
                    "invalid_recursive_lineage",
                    "final note spend nullifier collides with a lineage input nullifier",
                );

                let mut output_collision = real_recursive_kagemusha_redeem_test_fixture();
                output_collision.state.world.verifying_keys.insert(
                    output_collision.lineage_record_id.clone(),
                    output_collision.lineage_record.clone(),
                );
                let mut output_collision_witness = output_collision.lineage_witness.clone();
                let final_note_index = output_collision_witness
                    .current_notes
                    .len()
                    .checked_sub(1)
                    .expect("recursive lineage fixture has a final note");
                let final_note_commitment =
                    output_collision_witness.current_notes[final_note_index].note_commitment;
                let final_step =
                    &output_collision_witness.record_bundle.bundle.steps[final_note_index];
                let final_step_outputs = &final_step.output_commitments;
                let sibling_output = final_step_outputs
                    .iter()
                    .copied()
                    .find(|commitment| *commitment != final_note_commitment)
                    .expect("recursive lineage final hop has a sibling output commitment");
                output_collision_witness.current_notes[final_note_index].spend_nullifier =
                    sibling_output;
                output_collision.instruction.lineage_witness = Some(output_collision_witness);
                assert_recursive_redeem_rejection_preserves_state(
                    output_collision,
                    "invalid_recursive_lineage",
                    "spend nullifier collides with a lineage output commitment",
                );
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        fn run_recursive_kagemusha_redeem_large_stack(test: impl FnOnce() + Send + 'static) {
            std::thread::Builder::new()
                .name("recursive-kagemusha-redeem-test".to_owned())
                .stack_size(512 * 1024 * 1024)
                .spawn(test)
                .expect("spawn recursive Kagemusha redeem test")
                .join()
                .expect("recursive Kagemusha redeem test panicked");
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_verifier_and_policy_misconfigurations() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let (
                    no_unshield_state,
                    no_unshield_authority,
                    _,
                    no_unshield_definition_id,
                    no_unshield_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = no_unshield_state.block(header);
                {
                    let mut mutation = block.transaction();
                    mutation
                        .world_mut_for_testing()
                        .zk_assets_mut_for_testing()
                        .get_mut(&no_unshield_definition_id)
                        .expect("recursive Kagemusha shielded state")
                        .allow_unshield = false;
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = no_unshield_instruction
                    .execute(&no_unshield_authority, &mut transaction)
                    .expect_err("recursive Kagemusha redeem must reject disabled unshielding");
                assert_offline_rejection(err, "unshield_not_permitted", "asset policy");

                let (
                    missing_recursive_vk_state,
                    missing_recursive_vk_authority,
                    _,
                    _,
                    missing_recursive_vk_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = missing_recursive_vk_state.block(header);
                {
                    let mut mutation = block.transaction();
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(
                            missing_recursive_vk_instruction
                                .bundle
                                .recursive_proof
                                .verifier_key_id
                                .clone(),
                        );
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = missing_recursive_vk_instruction
                    .execute(&missing_recursive_vk_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject missing recursive verifier",
                    );
                assert_offline_rejection(err, "verifier_key_invalid", "spend verifier key");

                let (
                    missing_unshield_binding_state,
                    missing_unshield_binding_authority,
                    _,
                    missing_unshield_binding_definition_id,
                    missing_unshield_binding_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = missing_unshield_binding_state.block(header);
                {
                    let mut mutation = block.transaction();
                    mutation
                        .world_mut_for_testing()
                        .zk_assets_mut_for_testing()
                        .get_mut(&missing_unshield_binding_definition_id)
                        .expect("recursive Kagemusha shielded state")
                        .vk_unshield = None;
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = missing_unshield_binding_instruction
                    .execute(&missing_unshield_binding_authority, &mut transaction)
                    .expect_err("recursive Kagemusha redeem must reject missing unshield binding");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "bound unshield verifier key",
                );

                let (
                    missing_unshield_vk_state,
                    missing_unshield_vk_authority,
                    _,
                    _,
                    missing_unshield_vk_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = missing_unshield_vk_state.block(header);
                {
                    let mut mutation = block.transaction();
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(missing_unshield_vk_instruction.redeem_proof.vk_ref.clone());
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = missing_unshield_vk_instruction
                    .execute(&missing_unshield_vk_authority, &mut transaction)
                    .expect_err("recursive Kagemusha redeem must reject missing unshield verifier");
                assert_offline_rejection(err, "verifier_key_invalid", "not registered");

                let (
                    inactive_recursive_vk_state,
                    inactive_recursive_vk_authority,
                    _,
                    _,
                    inactive_recursive_vk_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = inactive_recursive_vk_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = inactive_recursive_vk_instruction
                        .bundle
                        .recursive_proof
                        .verifier_key_id
                        .clone();
                    let mut record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(vk_id.clone())
                        .expect("recursive verifier record");
                    record.status = ConfidentialStatus::Withdrawn;
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .insert(vk_id, record);
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = inactive_recursive_vk_instruction
                    .execute(&inactive_recursive_vk_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject inactive recursive verifier",
                    );
                assert_offline_rejection(err, "invalid_recursive_bundle", "not active");

                let (
                    tiny_recursive_vk_cap_state,
                    tiny_recursive_vk_cap_authority,
                    _,
                    _,
                    tiny_recursive_vk_cap_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = tiny_recursive_vk_cap_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = tiny_recursive_vk_cap_instruction
                        .bundle
                        .recursive_proof
                        .verifier_key_id
                        .clone();
                    let mut record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(vk_id.clone())
                        .expect("recursive verifier record");
                    record.max_proof_bytes = 1;
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .insert(vk_id, record);
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = tiny_recursive_vk_cap_instruction
                    .execute(&tiny_recursive_vk_cap_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject verifier proof-size cap mismatch",
                    );
                assert_offline_rejection(
                    err,
                    "invalid_recursive_bundle",
                    "exceeds verifier record",
                );

                let (
                    inactive_unshield_vk_state,
                    inactive_unshield_vk_authority,
                    _,
                    _,
                    inactive_unshield_vk_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = inactive_unshield_vk_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = inactive_unshield_vk_instruction.redeem_proof.vk_ref.clone();
                    let mut record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(vk_id.clone())
                        .expect("unshield verifier record");
                    record.status = ConfidentialStatus::Withdrawn;
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .insert(vk_id, record);
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = inactive_unshield_vk_instruction
                    .execute(&inactive_unshield_vk_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject inactive unshield verifier",
                    );
                assert_offline_rejection(err, "verifier_key_inactive", "not active");

                let (
                    wrong_unshield_namespace_state,
                    wrong_unshield_namespace_authority,
                    _,
                    _,
                    wrong_unshield_namespace_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = wrong_unshield_namespace_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = wrong_unshield_namespace_instruction
                        .redeem_proof
                        .vk_ref
                        .clone();
                    let mut record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(vk_id.clone())
                        .expect("unshield verifier record");
                    record.namespace = "generic_confidential_transfer".to_owned();
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .insert(vk_id, record);
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = wrong_unshield_namespace_instruction
                    .execute(&wrong_unshield_namespace_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject wrong unshield verifier namespace",
                    );
                assert_offline_rejection(err, "verifier_key_invalid", "Kagemusha namespace");

                let (
                    inactive_unshield_circuit_state,
                    inactive_unshield_circuit_authority,
                    _,
                    _,
                    inactive_unshield_circuit_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = inactive_unshield_circuit_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = inactive_unshield_circuit_instruction
                        .redeem_proof
                        .vk_ref
                        .clone();
                    let record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .get(&vk_id)
                        .expect("unshield verifier record")
                        .clone();
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_by_circuit_mut_for_testing()
                        .remove((record.circuit_id, record.version));
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = inactive_unshield_circuit_instruction
                    .execute(&inactive_unshield_circuit_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject inactive unshield verifier circuit",
                    );
                assert_offline_rejection(err, "verifier_key_inactive", "circuit/version");

                let (
                    substituted_unshield_key_state,
                    substituted_unshield_key_authority,
                    _,
                    substituted_unshield_key_definition_id,
                    mut substituted_unshield_key_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = substituted_unshield_key_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = substituted_unshield_key_instruction
                        .redeem_proof
                        .vk_ref
                        .clone();
                    let mut record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(vk_id.clone())
                        .expect("unshield verifier record");
                    let mut substituted_key = record.key.clone().expect("unshield verifier key");
                    *substituted_key
                        .bytes
                        .last_mut()
                        .expect("unshield verifier key bytes") ^= 0x01;
                    let substituted_commitment = crate::zk::hash_vk(&substituted_key);
                    record.commitment = substituted_commitment;
                    record.vk_len = u32::try_from(substituted_key.bytes.len())
                        .expect("test verifier key length");
                    record.key = Some(substituted_key);
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .insert(vk_id, record);
                    mutation
                        .world_mut_for_testing()
                        .zk_assets_mut_for_testing()
                        .get_mut(&substituted_unshield_key_definition_id)
                        .expect("recursive Kagemusha shielded state")
                        .vk_unshield
                        .as_mut()
                        .expect("unshield verifier binding")
                        .commitment = substituted_commitment;
                    substituted_unshield_key_instruction
                        .redeem_proof
                        .vk_commitment = Some(substituted_commitment);
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = substituted_unshield_key_instruction
                    .execute(&substituted_unshield_key_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject substituted unshield verifier key",
                    );
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "canonical semantic circuit key",
                );

                let (
                    wrong_unshield_curve_state,
                    wrong_unshield_curve_authority,
                    _,
                    _,
                    wrong_unshield_curve_instruction,
                ) = real_recursive_kagemusha_redeem_test_state();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = wrong_unshield_curve_state.block(header);
                {
                    let mut mutation = block.transaction();
                    let vk_id = wrong_unshield_curve_instruction.redeem_proof.vk_ref.clone();
                    let mut record = mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .remove(vk_id.clone())
                        .expect("unshield verifier record");
                    record.curve = "pasta".to_owned();
                    mutation
                        .world_mut_for_testing()
                        .verifying_keys_mut_for_testing()
                        .insert(vk_id, record);
                    mutation.apply();
                }
                let mut transaction = block.transaction();
                let err = wrong_unshield_curve_instruction
                    .execute(&wrong_unshield_curve_authority, &mut transaction)
                    .expect_err(
                        "recursive Kagemusha redeem must reject wrong unshield verifier curve",
                    );
                assert_offline_rejection(err, "verifier_key_invalid", "curve");
            });
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        #[ignore = "heavy Kagemusha recursive Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_recursive_redeem_rejects_amount_and_final_binding_mismatches() {
            run_recursive_kagemusha_redeem_large_stack(|| {
                let (state, authority, _, _, mut wrong_amount) =
                    real_recursive_kagemusha_redeem_test_state();
                wrong_amount.public_amount = 41;
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = wrong_amount
                    .execute(&authority, &mut transaction)
                    .expect_err("wrong recursive Kagemusha public amount must reject");
                assert_offline_rejection(err, "amount_mismatch", "amount");

                let (mut wrong_chain_state, wrong_chain_authority, _, _, wrong_chain_instruction) =
                    real_recursive_kagemusha_redeem_test_state();
                wrong_chain_state.chain_id = "kagemusha-recursive-redeem-other-chain"
                    .parse()
                    .expect("wrong chain id");
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = wrong_chain_state.block(header);
                let mut transaction = block.transaction();
                let err = wrong_chain_instruction
                    .execute(&wrong_chain_authority, &mut transaction)
                    .expect_err("wrong-chain recursive Kagemusha bundle must reject");
                assert_offline_rejection(err, "wrong_chain", "chain id");

                let (state, authority, _, _, mut wrong_final_note) =
                    real_recursive_kagemusha_redeem_test_state();
                wrong_final_note
                    .bundle
                    .accumulator
                    .current_note
                    .note_commitment[0] ^= 0x01;
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = wrong_final_note
                    .execute(&authority, &mut transaction)
                    .expect_err("tampered recursive Kagemusha final note must reject");
                assert_offline_rejection(err, "invalid_recursive_bundle", "public input");

                let (state, authority, _, _, mut wrong_proof_chain_public_input) =
                    real_recursive_kagemusha_redeem_test_state();
                wrong_proof_chain_public_input
                    .bundle
                    .recursive_proof
                    .public_inputs
                    .recursive_proof_chain_digest[0] ^= 0x01;
                wrong_proof_chain_public_input
                    .bundle
                    .recursive_proof
                    .public_inputs_hash = wrong_proof_chain_public_input
                    .bundle
                    .recursive_proof
                    .public_inputs
                    .public_inputs_hash()
                    .expect("mutated recursive proof public-input hash");
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = wrong_proof_chain_public_input
                    .execute(&authority, &mut transaction)
                    .expect_err("mutated recursive proof-chain public input must reject");
                assert_offline_rejection(err, "invalid_recursive_bundle", "public input");

                let (state, authority, _, _, mut wrong_scalar_projection_public_input) =
                    real_recursive_kagemusha_redeem_test_state();
                wrong_scalar_projection_public_input
                    .bundle
                    .recursive_proof
                    .public_inputs
                    .recursive_verifier_scalar_projection_digest[0] ^= 0x01;
                wrong_scalar_projection_public_input
                    .bundle
                    .recursive_proof
                    .public_inputs_hash = wrong_scalar_projection_public_input
                    .bundle
                    .recursive_proof
                    .public_inputs
                    .public_inputs_hash()
                    .expect("mutated recursive scalar projection public-input hash");
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = wrong_scalar_projection_public_input
                    .execute(&authority, &mut transaction)
                    .expect_err("mutated recursive scalar-projection public input must reject");
                assert_offline_rejection(err, "invalid_recursive_bundle", "public input");

                let (state, authority, _, _, mut trusted_setup_recursive_backend) =
                    real_recursive_kagemusha_redeem_test_state();
                trusted_setup_recursive_backend.bundle.recursive_proof.proof =
                    ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = trusted_setup_recursive_backend
                    .execute(&authority, &mut transaction)
                    .expect_err("trusted-setup recursive spend proof backend must reject");
                assert_offline_rejection(err, "invalid_recursive_bundle", "not supported");

                let (state, authority, _, _, mut transparent_wrong_family_recursive_backend) =
                    real_recursive_kagemusha_redeem_test_state();
                transparent_wrong_family_recursive_backend
                    .bundle
                    .recursive_proof
                    .proof = ProofBox::new("stark/fri/transparent-v1".into(), vec![0xA5; 64]);
                transparent_wrong_family_recursive_backend
                    .bundle
                    .recursive_proof
                    .verifier_key_id = VerifyingKeyId::new(
                    "stark/fri/transparent-v1",
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                );
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = transparent_wrong_family_recursive_backend
                    .execute(&authority, &mut transaction)
                    .expect_err("STARK/FRI recursive spend proof bundle must reject");
                assert_offline_rejection(err, "invalid_recursive_bundle", "proof.backend");

                let (state, authority, _, _, mut empty_recursive_proof) =
                    real_recursive_kagemusha_redeem_test_state();
                empty_recursive_proof.bundle.recursive_proof.proof =
                    ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.into(), Vec::new());
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = empty_recursive_proof
                    .execute(&authority, &mut transaction)
                    .expect_err("empty recursive spend proof payload must reject");
                assert_offline_rejection(err, "invalid_recursive_bundle", "proof.bytes");

                let (state, authority, _, _, mut wrong_redeem_public_inputs) =
                    real_recursive_kagemusha_redeem_test_state();
                mutate_proof_attachment_envelope(
                    &mut wrong_redeem_public_inputs.redeem_proof,
                    |envelope| {
                        *envelope
                            .proof_bytes
                            .last_mut()
                            .expect("recursive redeem proof public instances") ^= 0x01;
                    },
                );
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = wrong_redeem_public_inputs
                    .execute(&authority, &mut transaction)
                    .expect_err("mutated recursive Kagemusha redeem public inputs must reject");
                assert_offline_rejection(err, "final_commitment_mismatch", "final spendable note");

                let (state, authority, _, _, mut zero_redeem_vk_hash) =
                    real_recursive_kagemusha_redeem_test_state();
                mutate_proof_attachment_envelope(
                    &mut zero_redeem_vk_hash.redeem_proof,
                    |envelope| {
                        envelope.vk_hash = [0u8; 32];
                    },
                );
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = zero_redeem_vk_hash
                    .execute(&authority, &mut transaction)
                    .expect_err("zero recursive Kagemusha redeem verifier hash must reject");
                assert_offline_rejection(err, "verifier_key_invalid", "non-zero");

                let (state, authority, _, _, mut tampered_redeem_proof) =
                    real_recursive_kagemusha_redeem_test_state();
                *tampered_redeem_proof
                    .redeem_proof
                    .proof
                    .bytes
                    .last_mut()
                    .expect("recursive redeem proof bytes") ^= 0x01;
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();
                let err = tampered_redeem_proof
                    .execute(&authority, &mut transaction)
                    .expect_err("tampered recursive Kagemusha redeem proof must reject");
                assert_offline_rejection(err, "invalid_proof", "OpenVerifyEnvelope");
            });
        }

        #[test]
        fn kagemusha_recursive_redeem_resolver_rejects_missing_or_zero_vk_commitment_metadata() {
            fn assert_rejects(commitment: Option<[u8; 32]>, detail: &str) {
                let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
                let definition_id = AssetDefinitionId::new(
                    domain_id,
                    "kgmr".parse().expect("asset definition name"),
                );
                let vk_id = VerifyingKeyId::new(
                    crate::zk::ZK_BACKEND_HALO2_IPA,
                    "recursive-kagemusha-unshield-v3",
                );
                let binding_commitment = fixed_bytes(b"recursive-redeem-unshield-vk-binding");
                assert_ne!(binding_commitment, [0u8; 32]);
                let mut world = World::default();
                world.zk_assets.insert(definition_id.clone(), {
                    let mut zk_state = ZkAssetState::default();
                    zk_state.vk_unshield = Some(ZkAssetVerifierBinding {
                        id: vk_id.clone(),
                        commitment: binding_commitment,
                    });
                    zk_state
                });
                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let state = State::new(world, Arc::clone(&kura), query);
                let mut proof = ProofAttachment::new_ref(
                    crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                    ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.into(), vec![0xC0]),
                    vk_id,
                );
                proof.vk_commitment = commitment;
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = resolve_kagemusha_unshield_verifier(&definition_id, &proof, &transaction)
                    .expect_err("malformed verifier-key commitment metadata must reject");
                assert_offline_rejection(err, "verifier_key_invalid", detail);
            }

            assert_rejects(None, "must publish the asset-bound verifier-key commitment");
            assert_rejects(
                Some([0u8; 32]),
                "non-zero asset-bound verifier-key commitment",
            );
        }

        #[test]
        fn kagemusha_transfer_rejects_pending_production_backend_labels_before_state_lookup() {
            let authority = sample_account(0x68);
            for backend in PENDING_PRODUCTION_BACKEND_LABELS {
                let kura = Kura::blank_kura_for_testing();
                let state = State::new(
                    World::default(),
                    Arc::clone(&kura),
                    LiveQueryStore::start_test(),
                );
                let transfer = sample_kagemusha_transfer(backend);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer.execute(&authority, &mut transaction).expect_err(
                    "pending-production Kagemusha labels must reject before WSV lookup",
                );
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "pending-production proof backends",
                );
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_pending_production_verifier_record_tags_before_proof_decode()
        {
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id,
                "kgm_pending".parse().expect("asset definition name"),
            );
            let vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "offline-kagemusha-confidential-transfer-v2",
            );
            let commitment = fixed_bytes(b"kagemusha-pending-transfer-vk-binding");
            let schema_hash: [u8; 32] = Hash::new(
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1,
            )
            .into();

            for backend_tag in PENDING_PRODUCTION_BACKEND_TAGS {
                let mut record = VerifyingKeyRecord::new(
                    1,
                    crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                    backend_tag,
                    "pallas",
                    schema_hash,
                    commitment,
                );
                record.namespace = crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
                record.status = ConfidentialStatus::Active;
                record.max_proof_bytes = 4096;
                record.vk_len = 1;

                let mut world = World::default();
                world
                    .verifying_keys_by_circuit
                    .insert((record.circuit_id.clone(), record.version), vk_id.clone());
                world.verifying_keys.insert(vk_id.clone(), record);
                world.zk_assets.insert(definition_id.clone(), {
                    let mut zk_state = ZkAssetState::default();
                    zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                        id: vk_id.clone(),
                        commitment,
                    });
                    zk_state
                });
                let kura = Kura::blank_kura_for_testing();
                let state = State::new(world, Arc::clone(&kura), LiveQueryStore::start_test());
                let mut proof = ProofAttachment::new_ref(
                    crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                    ProofBox::new(
                        crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                        b"not-an-open-verify-envelope".to_vec(),
                    ),
                    vk_id.clone(),
                );
                proof.vk_commitment = Some(commitment);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = ensure_kagemusha_transfer_verifier_binding(
                    &definition_id,
                    &proof,
                    Some([0x33; 32]),
                    &transaction,
                )
                .expect_err(
                    "pending Kagemusha transfer record tags must reject before proof decode",
                );
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "pending-production backend tag",
                );
            }
        }

        #[test]
        fn kagemusha_recursive_redeem_resolver_rejects_pending_production_backend_labels_before_state_lookup()
         {
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id,
                "kgmr_pending_label".parse().expect("asset definition name"),
            );

            for backend in PENDING_PRODUCTION_BACKEND_LABELS {
                let kura = Kura::blank_kura_for_testing();
                let state = State::new(
                    World::default(),
                    Arc::clone(&kura),
                    LiveQueryStore::start_test(),
                );
                let mut proof = ProofAttachment::new_ref(
                    backend.into(),
                    ProofBox::new(backend.into(), vec![0xC0]),
                    VerifyingKeyId::new(backend, "recursive-kagemusha-unshield-v3"),
                );
                proof.vk_commitment = Some(fixed_bytes(b"unused-pending-label-vk-binding"));
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = resolve_kagemusha_unshield_verifier(&definition_id, &proof, &transaction)
                    .expect_err("pending-production unshield labels must reject before WSV lookup");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "pending-production proof backends",
                );
            }
        }

        #[test]
        fn kagemusha_recursive_redeem_resolver_rejects_pending_production_unshield_record_tags() {
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id,
                "kgmr_pending_record"
                    .parse()
                    .expect("asset definition name"),
            );
            let vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "recursive-kagemusha-unshield-v3",
            );
            let commitment = fixed_bytes(b"kagemusha-pending-unshield-vk-binding");
            let schema_hash: [u8; 32] = Hash::new(
                crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1,
            )
            .into();

            for backend_tag in PENDING_PRODUCTION_BACKEND_TAGS {
                let mut record = VerifyingKeyRecord::new(
                    1,
                    crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                    backend_tag,
                    "pallas",
                    schema_hash,
                    commitment,
                );
                record.namespace = crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
                record.status = ConfidentialStatus::Active;
                record.max_proof_bytes = 4096;
                record.vk_len = 1;

                let mut world = World::default();
                world.verifying_keys.insert(vk_id.clone(), record);
                world.zk_assets.insert(definition_id.clone(), {
                    let mut zk_state = ZkAssetState::default();
                    zk_state.vk_unshield = Some(ZkAssetVerifierBinding {
                        id: vk_id.clone(),
                        commitment,
                    });
                    zk_state
                });
                let kura = Kura::blank_kura_for_testing();
                let state = State::new(world, Arc::clone(&kura), LiveQueryStore::start_test());
                let mut proof = ProofAttachment::new_ref(
                    crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                    ProofBox::new(
                        crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                        b"not-an-open-verify-envelope".to_vec(),
                    ),
                    vk_id.clone(),
                );
                proof.vk_commitment = Some(commitment);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = resolve_kagemusha_unshield_verifier(&definition_id, &proof, &transaction)
                    .expect_err("pending unshield record tags must reject before proof decode");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "pending-production backend tag",
                );
            }
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_verifier_record_mismatches_before_proof_decode() {
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    let record = transaction
                        .world
                        .verifying_keys
                        .get(verifier_id)
                        .expect("verifier record")
                        .clone();
                    transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id, record.version));
                },
                "verifier_key_inactive",
                "circuit/version",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.namespace = "generic_confidential_transfer".to_owned();
                    });
                },
                "verifier_key_invalid",
                "Kagemusha namespace",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.curve = "pasta".to_owned();
                    });
                },
                "verifier_key_invalid",
                "curve",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    let mut record = transaction
                        .world
                        .verifying_keys
                        .get(verifier_id)
                        .expect("verifier record")
                        .clone();
                    transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id.clone(), record.version));
                    record.circuit_id =
                        "anon-transfer-2x2-merkle16-poseidon-diversified".to_owned();
                    transaction.world.verifying_keys_by_circuit.insert(
                        (record.circuit_id.clone(), record.version),
                        verifier_id.clone(),
                    );
                    transaction
                        .world
                        .verifying_keys
                        .insert(verifier_id.clone(), record);
                },
                "verifier_key_invalid",
                "canonical",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.key = None;
                    });
                },
                "verifier_key_invalid",
                "not available inline",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.vk_len += 1;
                    });
                },
                "verifier_key_invalid",
                "key length",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.max_proof_bytes = 0;
                    });
                },
                "verifier_key_invalid",
                "non-zero max_proof_bytes",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.max_proof_bytes = 1;
                    });
                },
                "invalid_proof",
                "max_proof_bytes",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        let key = record.key.as_mut().expect("inline verifier key");
                        key.backend = "stark/fri".to_owned();
                    });
                },
                "verifier_key_invalid",
                "inline verifier key backend",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        let key = record.key.as_mut().expect("inline verifier key");
                        key.bytes.clear();
                        record.vk_len = 0;
                    });
                },
                "verifier_key_invalid",
                "key bytes must be non-empty",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        let key = record.key.as_mut().expect("inline verifier key");
                        let first = key.bytes.first_mut().expect("non-empty verifier key");
                        *first ^= 0x01;
                    });
                },
                "verifier_key_invalid",
                "inline verifier-key commitment",
            );

            let (state, authority, definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            let verifier_id = transfer.proof.vk_ref.clone();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation_transaction = block.transaction();
                let mut noncanonical_key = {
                    let record = mutation_transaction
                        .world
                        .verifying_keys
                        .get(&verifier_id)
                        .expect("verifier record");
                    record.key.clone().expect("inline verifier key")
                };
                let last = noncanonical_key
                    .bytes
                    .last_mut()
                    .expect("non-empty verifier key");
                *last ^= 0x01;
                let noncanonical_commitment = crate::zk::hash_vk(&noncanonical_key);
                transfer.proof.vk_commitment = Some(noncanonical_commitment);
                transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
                mutate_verifier_record(&mut mutation_transaction, &verifier_id, |record| {
                    record.commitment = noncanonical_commitment;
                    record.vk_len = u32::try_from(noncanonical_key.bytes.len())
                        .expect("verifier key length fits u32");
                    record.key = Some(noncanonical_key);
                });
                let zk_state = mutation_transaction
                    .world
                    .zk_assets
                    .get_mut(&definition_id)
                    .expect("Kagemusha zk asset state");
                let binding = zk_state
                    .vk_transfer
                    .as_mut()
                    .expect("Kagemusha transfer verifier binding");
                binding.commitment = noncanonical_commitment;
                mutation_transaction.apply();
            }
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("self-consistent noncanonical transfer verifier must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "canonical semantic circuit key",
            );
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_confidential_v2_public_input_mismatches() {
            let cases: [(
                &str,
                fn(&mut State, &AssetDefinitionId, &mut KagemushaTransfer),
            ); 5] = [
                ("root_hint mismatch", |state, definition_id, transfer| {
                    let forged_root = [0xA4; 32];
                    let envelope: OpenVerifyEnvelope =
                        norito::decode_from_bytes(&transfer.proof.proof.bytes)
                            .expect("OpenVerifyEnvelope");
                    let mut zk_state = ZkAssetState::default();
                    zk_state
                        .root_history
                        .push(transfer.root_hint.expect("sample has root hint"));
                    zk_state.root_history.push(forged_root);
                    zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                        id: transfer.proof.vk_ref.clone(),
                        commitment: envelope.vk_hash,
                    });
                    state
                        .world
                        .zk_assets
                        .insert(definition_id.clone(), zk_state);
                    transfer.root_hint = Some(forged_root);
                }),
                ("nullifier mismatch", |_state, _definition_id, transfer| {
                    transfer.inputs[0][0] ^= 0x01;
                }),
                (
                    "output commitment mismatch",
                    |_state, _definition_id, transfer| {
                        transfer.outputs[0][0] ^= 0x01;
                    },
                ),
                ("chain tag mismatch", |state, _definition_id, _transfer| {
                    state.chain_id = "kagemusha-transfer-other-chain".parse().expect("chain id");
                }),
                ("asset tag mismatch", |state, definition_id, transfer| {
                    let other_definition_id = AssetDefinitionId::new(
                        definition_id.domain().clone(),
                        "kgm-other".parse().expect("asset definition name"),
                    );
                    let other_definition =
                        AssetDefinition::new(other_definition_id.clone(), NumericSpec::integer())
                            .with_name("kgm-other".to_owned())
                            .confidential_policy(AssetConfidentialPolicy::convertible())
                            .build(&sample_account(0x46));
                    state
                        .world
                        .asset_definitions
                        .insert(other_definition_id.clone(), other_definition);
                    let envelope: OpenVerifyEnvelope =
                        norito::decode_from_bytes(&transfer.proof.proof.bytes)
                            .expect("OpenVerifyEnvelope");
                    let mut zk_state = ZkAssetState::default();
                    zk_state
                        .root_history
                        .push(transfer.root_hint.expect("sample has root hint"));
                    zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                        id: transfer.proof.vk_ref.clone(),
                        commitment: envelope.vk_hash,
                    });
                    state
                        .world
                        .zk_assets
                        .insert(other_definition_id.clone(), zk_state);
                    transfer.asset = other_definition_id;
                }),
            ];

            for (expected, mutate) in cases {
                let (mut state, authority, definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate(&mut state, &definition_id, &mut transfer);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("forged Kagemusha public inputs must reject");
                let message = err.to_string();
                assert!(
                    message.contains(expected),
                    "expected {expected:?} rejection, got {message}"
                );
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_backend_field_mismatches() {
            let authority = sample_account(0x44);
            let cases: [fn(&mut KagemushaTransfer); 2] = [
                |transfer: &mut KagemushaTransfer| {
                    transfer.proof.proof.backend = "stark/fri".to_owned();
                },
                |transfer: &mut KagemushaTransfer| {
                    transfer.proof.vk_ref =
                        VerifyingKeyId::new("stark/fri", "offline-kagemusha-transfer");
                },
            ];
            for mutate in cases {
                let mut transfer = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA);
                mutate(&mut transfer);
                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let state = State::new(World::default(), Arc::clone(&kura), query);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha must bind attachment backend fields");
                assert_offline_rejection(err, "proof_binding", "must match");
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_empty_input_or_output_shape() {
            let authority = sample_account(0x43);
            let mut transfer = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA);
            transfer.inputs.clear();
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha must reject empty input nullifiers");
            assert_offline_rejection(err, "invalid_proof", "1 to 2 input nullifiers");
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_duplicate_sets_before_proof_decode() {
            let cases: [(&str, &str, fn(&mut KagemushaTransfer)); 2] = [
                (
                    "duplicate_nullifier",
                    "input nullifiers must be unique",
                    |transfer: &mut KagemushaTransfer| {
                        transfer.inputs.push(transfer.inputs[0]);
                    },
                ),
                (
                    "duplicate_output",
                    "output commitments must be unique",
                    |transfer: &mut KagemushaTransfer| {
                        transfer.outputs.push(transfer.outputs[0]);
                    },
                ),
            ];

            for (label, detail, mutate) in cases {
                let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate(&mut transfer);
                transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer.execute(&authority, &mut transaction).expect_err(
                    "duplicate Kagemusha transfer sets must reject before proof decode",
                );
                assert_offline_rejection(err, label, detail);
            }
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_nullifier_output_overlap_before_proof_decode() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.outputs[0] = transfer.inputs[0];
            transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha input/output overlap must reject before proof decode");
            assert_offline_rejection(
                err,
                "proof_binding",
                "output commitments must be disjoint from input nullifiers",
            );
        }

        #[test]
        #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
        fn kagemusha_transfer_rejects_zero_sets_before_proof_decode() {
            let cases: [(&str, fn(&mut KagemushaTransfer)); 2] = [
                ("input nullifiers must be non-zero", |transfer| {
                    transfer.inputs[0] = [0u8; 32];
                }),
                ("output commitments must be non-zero", |transfer| {
                    transfer.outputs[0] = [0u8; 32];
                }),
            ];

            for (detail, mutate) in cases {
                let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate(&mut transfer);
                transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("zero Kagemusha transfer sets must reject before proof decode");
                assert_offline_rejection(err, "invalid_proof", detail);
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_oversized_input_or_output_shape() {
            let authority = sample_account(0x45);
            let cases: [fn(&mut KagemushaTransfer); 2] = [
                |transfer: &mut KagemushaTransfer| transfer.inputs.push([0x44; 32]),
                |transfer: &mut KagemushaTransfer| transfer.outputs.push([0x55; 32]),
            ];
            for mutate in cases {
                let mut transfer = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA);
                transfer.inputs.push([0x66; 32]);
                transfer.outputs.push([0x77; 32]);
                mutate(&mut transfer);
                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let state = State::new(World::default(), Arc::clone(&kura), query);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha must reject more than two inputs or outputs");
                assert_offline_rejection(err, "invalid_proof", "1 to 2 input nullifiers");
            }
        }

        #[test]
        fn audit_replay_keys_cover_input_spend_and_output_issue_domains() {
            let claim_hash =
                offline_note_issued_claim_hash(sample_issued_claim()).expect("claim hash");
            let nullifier = Hash::new(b"offline-note-input-nullifier");
            let output_commitment = Hash::new(b"offline-note-output-commitment");

            let issued_claim_key = offline_note_issued_claim_key(&claim_hash);
            let spent_claim_key = offline_note_spent_claim_key(&claim_hash);
            let nullifier_key = offline_note_nullifier_key(&nullifier);
            let audit_nullifier_key = offline_note_audit_nullifier_key(&nullifier);
            let audit_output_key = offline_note_audit_output_key(&output_commitment);
            let output_issue_key = offline_note_issue_key(&output_commitment);

            assert_eq!(
                issued_claim_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUED_CLAIM_DOMAIN, &claim_hash)
            );
            assert_eq!(
                spent_claim_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_SPENT_CLAIM_DOMAIN, &claim_hash)
            );
            assert_eq!(
                nullifier_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_NULLIFIER_DOMAIN, &nullifier)
            );
            assert_eq!(
                audit_output_key,
                offline_note_replay_key(
                    OFFLINE_NOTE_REPLAY_AUDIT_OUTPUT_DOMAIN,
                    &output_commitment,
                )
            );
            assert_eq!(
                output_issue_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUE_DOMAIN, &output_commitment)
            );
            assert_eq!(
                BTreeSet::from([
                    issued_claim_key,
                    spent_claim_key,
                    nullifier_key,
                    audit_nullifier_key,
                    audit_output_key,
                    output_issue_key,
                ])
                .len(),
                6
            );
        }
    }
}
