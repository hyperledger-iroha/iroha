//! Closed normalization of the two first-release mobile FI request formats, each
//! with one explicit schema per operation: `LinkApproval` and `Transfer`.
//!
//! Proposed destination: `iroha_data_model::asset::fi_request`. Parsing yields
//! selectors only. Every field still needs comparison with independently admitted
//! local inputs; neither a request nor its SHA256 authenticates ledger state.

use std::collections::BTreeMap;

use iroha_primitives::numeric::{Numeric, Quantity};
use norito::json::{self, Value};
use sha2::{Digest, Sha256};

use super::{AssetDefinitionId, fi_context::AssetFiContextTargetV1, fi_link::AssetFiLinkV1};
use crate::{
    NetworkId,
    account::{AccountId, address::AccountAddress},
    isi::asset_fi::{SetAssetFiLinkV1, TransferToLinkedFiV1},
    smart_contract::ContractAddress,
};

/// Maximum original platform request; the outer HTTP envelope has a separate cap.
pub const MAX_ASSET_FI_CONTEXT_REQUEST_BYTES_V1: usize = 64 * 1024;

/// Unauthenticated, lossless selectors extracted from exact retained request bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedAssetFiContextRequestV1 {
    /// SHA256 of the original bytes, before any interpretation or normalization.
    pub original_sha256: [u8; 32],
    /// Fresh caller-selected challenge, not a server-provided nonce.
    pub challenge: [u8; 32],
    /// Independently selected checkpoint height, compared by the verifier.
    pub minimum_height: u64,
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Locally selected chain label.
    pub chain_id: String,
    /// Exact asset selector; Global scope must be established by native evidence.
    pub asset_id: AssetDefinitionId,
    /// Selected wallet identity.
    pub wallet: AccountId,
    /// Opaque selected UID.
    pub uid: [u8; 32],
    /// Runtime participant selector; this is not proof of an FI role.
    pub fi_participant: String,
    /// SHA256 of the independently admitted runtime manifest.
    pub runtime_manifest_sha256: [u8; 32],
    /// Protected local key generation/lease identity.
    pub owner_key_identity: String,
    /// iOS explicitly retains its policy-controller selection; Android V1 does
    /// not carry this selector separately from the complete target.
    pub declared_uid_controller: Option<Option<ContractAddress>>,
    /// Exact canonical base wallet scope, retaining all sponsor and route fields.
    pub wallet_scope_canonical: Vec<u8>,
    /// Complete native target of the one operation the schema selects.
    pub target: AssetFiContextTargetV1,
}

type Object = BTreeMap<String, Value>;

/// Normalize the complete independently selected twelve-field `LinkApproval` target.
/// This yields comparison inputs only; it does not authenticate ledger state.
///
/// # Errors
/// Rejects excessive size, missing/unknown fields and invalid native selectors.
pub fn normalize_asset_fi_link_target_v1(value: &Value) -> Result<SetAssetFiLinkV1, String> {
    link_target(bounded_target(value)?)
}

/// Normalize one exact, closed, canonical iOS or Android request. Its schema alone
/// selects `LinkApproval` or `Transfer`; no target field can select the operation.
///
/// Rejects duplicate/unknown fields, numeric coercions, reserialized originals,
/// mismatched nested identities and unsupported kinds. Full `u64` values and
/// amounts are decoded as integers; no floating-point conversion participates.
///
/// # Errors
/// Returns a validation error without returning a partially normalized request.
pub fn normalize_asset_fi_context_request_v1(
    raw: &[u8],
) -> Result<NormalizedAssetFiContextRequestV1, String> {
    let value = canonical_document(raw, MAX_ASSET_FI_CONTEXT_REQUEST_BYTES_V1)?;
    let request = object(&value)?;
    let schema = text(request, "schema", 64)?;
    let (ios, transfer) = match schema {
        "boi.wallet-fi-context-evidence.ios.v1" => (true, false),
        "boi.wallet-fi-context-evidence.android.v1" => (false, false),
        "boi.wallet-fi-context-evidence.ios.transfer.v1" => (true, true),
        "boi.wallet-fi-context-evidence.android.transfer.v1" => (false, true),
        _ => return Err("unsupported FI request schema or operation".into()),
    };
    let (scope_value, runtime_manifest_sha256, uid, fi_participant) = if ios {
        closed(
            request,
            &[
                "schema",
                "binding",
                "ownerKeyIdentity",
                "minimumCertifiedHeight",
                "challengeHex",
                "target",
            ],
        )?;
        let binding = object(field(request, "binding")?)?;
        closed(
            binding,
            &[
                "schema",
                "walletScope",
                "runtimeManifestDigest",
                "uid",
                "fiParticipant",
                "fiAccountId",
                "uidController",
            ],
        )?;
        if text(binding, "schema", 64)? != "boi.wallet-fi-context.v1" {
            return Err("unsupported FI binding schema".into());
        }
        if json::to_vec(field(request, "binding")?)
            .map_err(|error| error.to_string())?
            .len()
            > 32768
        {
            return Err("FI binding exceeds bound".into());
        }
        (
            field(binding, "walletScope")?.clone(),
            hash(binding, "runtimeManifestDigest")?,
            hash(binding, "uid")?,
            text(binding, "fiParticipant", 256)?.to_owned(),
        )
    } else {
        closed(
            request,
            &[
                "schema",
                "walletScope",
                "runtimeManifestDigest",
                "ownerKeyIdentity",
                "networkId",
                "chainId",
                "assetDefinitionId",
                "walletAccount",
                "uid",
                "fiParticipant",
                "referenceDigest",
                "minimumCertifiedHeight",
                "challengeHex",
                "target",
            ],
        )?;
        (
            canonical_document(text(request, "walletScope", 16384)?.as_bytes(), 16384)?,
            hash(request, "runtimeManifestDigest")?,
            hash(request, "uid")?,
            text(request, "fiParticipant", 256)?.to_owned(),
        )
    };
    let scope = object(&scope_value)?;
    validate_scope(scope, ios)?;
    let wallet_scope_canonical = json::to_vec(&scope_value).map_err(|error| error.to_string())?;
    if wallet_scope_canonical.len() > 16384 {
        return Err("FI wallet scope exceeds bound".into());
    }
    let network_id = text(scope, "networkId", 74)?
        .parse::<NetworkId>()
        .map_err(|error| error.to_string())?;
    let chain_id = text(scope, "chainId", 1024)?.to_owned();
    let asset_id = asset(scope, "assetDefinitionId")?;
    let wallet = account(scope, "accountId")?;
    let target_object = bounded_target(field(request, "target")?)?;
    let target = if transfer {
        transfer_target(target_object)?
    } else {
        AssetFiContextTargetV1::LinkApproval(link_target(target_object)?)
    };
    let (target_reference, target_controller) = match &target {
        AssetFiContextTargetV1::LinkApproval(approval) => (
            approval.link.reference_digest,
            approval.link.controller.as_ref(),
        ),
        // A transfer retains no link capability, so it declares no target controller.
        AssetFiContextTargetV1::Transfer {
            reference_digest, ..
        } => (*reference_digest, None),
    };
    if text(target_object, "assetDefinitionId", 128)? != text(scope, "assetDefinitionId", 128)?
        || text(target_object, "accountId", 1024)? != text(scope, "accountId", 1024)?
    {
        return Err("FI target differs from retained wallet scope".into());
    }
    let declared_uid_controller = if ios {
        let binding = object(field(request, "binding")?)?;
        if text(binding, "fiAccountId", 1024)? != text(target_object, "fiAccountId", 1024)? {
            return Err("FI target differs from retained FI account".into());
        }
        let uid_controller = optional_controller(binding, "uidController")?;
        if let Some(controller) = target_controller
            && uid_controller.as_ref() != Some(controller)
        {
            return Err("FI target differs from retained UID controller".into());
        }
        Some(uid_controller)
    } else {
        for (outer, inner) in [
            ("networkId", "networkId"),
            ("chainId", "chainId"),
            ("assetDefinitionId", "assetDefinitionId"),
            ("walletAccount", "accountId"),
        ] {
            if field(request, outer)? != field(scope, inner)? {
                return Err("FI request differs from retained wallet scope".into());
            }
        }
        if hash(request, "referenceDigest")? != target_reference {
            return Err("FI request differs from target reference".into());
        }
        None
    };
    let minimum_height = positive_u64(request, "minimumCertifiedHeight")?;
    if minimum_height == u64::MAX {
        return Err("FI checkpoint height cannot be the terminal u64 value".into());
    }
    Ok(NormalizedAssetFiContextRequestV1 {
        original_sha256: Sha256::digest(raw).into(),
        challenge: hash(request, "challengeHex")?,
        minimum_height,
        network_id,
        chain_id,
        asset_id,
        wallet,
        uid,
        fi_participant,
        runtime_manifest_sha256,
        owner_key_identity: text(request, "ownerKeyIdentity", 256)?.to_owned(),
        declared_uid_controller,
        wallet_scope_canonical,
        target,
    })
}

fn canonical_document(raw: &[u8], maximum: usize) -> Result<Value, String> {
    if raw.is_empty() || raw.len() > maximum {
        return Err("FI request document exceeds bound".into());
    }
    // Norito's Value parser rejects duplicates, invalid UTF-8, trailing input,
    // invalid surrogate pairs and excessive nesting before returning the tree.
    let value = json::from_slice_value(raw).map_err(|error| error.to_string())?;
    if json::to_vec(&value).map_err(|error| error.to_string())? != raw {
        return Err("FI request must retain exact canonical JSON bytes".into());
    }
    Ok(value)
}

fn bounded_target(value: &Value) -> Result<&Object, String> {
    if json::to_vec(value)
        .map_err(|error| error.to_string())?
        .len()
        > 16 * 1024
    {
        return Err("FI target exceeds bound".into());
    }
    object(value)
}

fn object(value: &Value) -> Result<&Object, String> {
    value
        .as_object()
        .ok_or_else(|| "FI request requires a JSON object".into())
}

fn field<'a>(value: &'a Object, key: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing FI request field {key}"))
}

fn closed(value: &Object, keys: &[&str]) -> Result<(), String> {
    if value.len() != keys.len() || keys.iter().any(|key| !value.contains_key(*key)) {
        return Err("unknown or missing FI request fields".into());
    }
    Ok(())
}

fn text<'a>(value: &'a Object, key: &str, maximum: usize) -> Result<&'a str, String> {
    let text = field(value, key)?
        .as_str()
        .ok_or_else(|| format!("FI field {key} must be text"))?;
    if text.is_empty()
        || text.len() > maximum
        || text.trim() != text
        || text.chars().any(char::is_control)
    {
        return Err(format!("FI field {key} has invalid text"));
    }
    Ok(text)
}

fn hash(value: &Object, key: &str) -> Result<[u8; 32], String> {
    let text = text(value, key, 64)?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!("FI field {key} must be lowercase raw32 hex"));
    }
    let bytes: [u8; 32] = hex::decode(text)
        .map_err(|error| error.to_string())?
        .try_into()
        .map_err(|_| "invalid FI raw32 length".to_owned())?;
    if bytes == [0; 32] {
        return Err(format!("FI field {key} cannot be zero"));
    }
    Ok(bytes)
}

fn positive_u64(value: &Object, key: &str) -> Result<u64, String> {
    field(value, key)?
        .as_u64()
        .filter(|number| *number > 0)
        .ok_or_else(|| format!("FI field {key} must be an exact positive u64 integer"))
}

fn boolean(value: &Object, key: &str) -> Result<bool, String> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| format!("FI field {key} must be boolean"))
}

fn account(value: &Object, key: &str) -> Result<AccountId, String> {
    // AccountId::parse_encoded reads a process-global display discriminant.
    // Mobile input is already a canonical I105 literal; infer only that display
    // prefix, then compare its domainless native identity with the local pin.
    AccountAddress::parse_encoded(text(value, key, 1024)?, None)
        .and_then(|address| address.to_account_id())
        .map_err(|error| error.to_string())
}

fn asset(value: &Object, key: &str) -> Result<AssetDefinitionId, String> {
    text(value, key, 128)?
        .parse::<AssetDefinitionId>()
        .map_err(|error| error.to_string())
}

fn optional_controller(value: &Object, key: &str) -> Result<Option<ContractAddress>, String> {
    if field(value, key)?.is_null() {
        return Ok(None);
    }
    text(value, key, 256)?
        .parse::<ContractAddress>()
        .map(Some)
        .map_err(|error| error.to_string())
}

fn link_target(value: &Object) -> Result<SetAssetFiLinkV1, String> {
    closed(
        value,
        &[
            "assetDefinitionId",
            "accountId",
            "fiAccountId",
            "referenceDigest",
            "fiRevision",
            "attestationRevision",
            "revision",
            "expectedPolicyRevision",
            "enabled",
            "controller",
            "waterfall",
            "reverseWaterfall",
        ],
    )?;
    let link = AssetFiLinkV1 {
        asset_definition_id: asset(value, "assetDefinitionId")?,
        account_id: account(value, "accountId")?,
        fi_account_id: account(value, "fiAccountId")?,
        reference_digest: hash(value, "referenceDigest")?,
        fi_revision: positive_u64(value, "fiRevision")?,
        attestation_revision: positive_u64(value, "attestationRevision")?,
        revision: positive_u64(value, "revision")?,
        enabled: boolean(value, "enabled")?,
        controller: optional_controller(value, "controller")?,
        waterfall: boolean(value, "waterfall")?,
        reverse_waterfall: boolean(value, "reverseWaterfall")?,
    };
    if link.account_id == link.fi_account_id
        || ((link.waterfall || link.reverse_waterfall)
            && (!link.enabled || link.controller.is_none()))
    {
        return Err("invalid FI target capability or wallet destination".into());
    }
    Ok(SetAssetFiLinkV1 {
        link,
        expected_policy_revision: positive_u64(value, "expectedPolicyRevision")?,
    })
}

/// The closed nine-field target: the five-field native transfer plus the exact FI
/// selectors and revisions its consent retained. Capability and destination fields
/// are refused, never synthesized; the native executor derives the destination.
fn transfer_target(value: &Object) -> Result<AssetFiContextTargetV1, String> {
    closed(
        value,
        &[
            "assetDefinitionId",
            "accountId",
            "amount",
            "expectedLinkRevision",
            "expectedPolicyRevision",
            "fiAccountId",
            "referenceDigest",
            "expectedFiRevision",
            "expectedAttestationRevision",
        ],
    )?;
    let instruction = TransferToLinkedFiV1 {
        asset_definition_id: asset(value, "assetDefinitionId")?,
        account_id: account(value, "accountId")?,
        amount: positive_minor_unit_amount(value, "amount")?,
        expected_link_revision: positive_u64(value, "expectedLinkRevision")?,
        expected_policy_revision: positive_u64(value, "expectedPolicyRevision")?,
    };
    let fi_account_id = account(value, "fiAccountId")?;
    if instruction.account_id == fi_account_id {
        return Err("invalid FI target capability or wallet destination".into());
    }
    Ok(AssetFiContextTargetV1::Transfer {
        instruction,
        fi_account_id,
        reference_digest: hash(value, "referenceDigest")?,
        expected_fi_revision: positive_u64(value, "expectedFiRevision")?,
        expected_attestation_revision: positive_u64(value, "expectedAttestationRevision")?,
    })
}

/// Exactly `0.dd` or `[1-9][0-9]*.dd`, positive and within `u64` minor units, the
/// only amount text a retained consent can hold. Integer arithmetic only: nothing
/// is rounded, and a sign, exponent or JSON number never reaches a parser.
fn positive_minor_unit_amount(value: &Object, key: &str) -> Result<Quantity, String> {
    // The longest admitted spelling is the u64 maximum, `184467440737095516.15`.
    let amount = text(value, key, 21)?;
    let invalid = || format!("FI field {key} must be a canonical positive two-decimal amount");
    let (integer, fraction) = amount.split_once('.').ok_or_else(invalid)?;
    if integer.is_empty()
        || fraction.len() != 2
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
    {
        return Err(invalid());
    }
    let whole = integer.parse::<u64>().map_err(|_| invalid())?;
    let fraction = fraction.parse::<u64>().map_err(|_| invalid())?;
    let minor = whole
        .checked_mul(100)
        .and_then(|minor| minor.checked_add(fraction))
        .filter(|minor| *minor > 0)
        .ok_or_else(invalid)?;
    Numeric::try_new(minor, 2)
        .ok()
        .and_then(|value| Quantity::try_from(value).ok())
        .ok_or_else(invalid)
}

fn validate_scope(scope: &Object, ios: bool) -> Result<(), String> {
    let mut keys = vec![
        "accountId",
        "bank",
        "participantScope",
        "coreBaseUrl",
        "chainId",
        "networkId",
        "assetDefinitionId",
        "fees",
    ];
    if ios {
        keys.push("feeSponsorProgramId");
    }
    closed(scope, &keys)?;
    text(scope, "bank", 256)?;
    text(scope, "participantScope", 1024)?;
    text(scope, "coreBaseUrl", 2048)?;
    let fees = field(scope, "fees")?;
    if json::to_vec(fees).map_err(|error| error.to_string())?.len() > 8192 {
        return Err("FI wallet fee scope exceeds bound".into());
    }
    let program = validate_sponsor_shape(object(fees)?)?;
    if ios && text(scope, "feeSponsorProgramId", 1536)? != program {
        return Err("FI wallet sponsor selectors differ".into());
    }
    Ok(())
}

// Scope fees are retained comparison inputs, never authorization to charge them.
// Validate the actual mobile Sponsor JSON layout without invoking a global-prefix
// account decoder or inventing a second fee-policy validation algorithm.
fn validate_sponsor_shape(fees: &Object) -> Result<String, String> {
    closed(fees, &["payer", "value"])?;
    if text(fees, "payer", 16)? != "sponsor" {
        return Err("FI wallet requires sponsor fee scope".into());
    }
    let value = object(field(fees, "value")?)?;
    closed(
        value,
        &[
            "program_id",
            "program_revision",
            "charge_limits",
            "gas_limit",
        ],
    )?;
    let program = object(field(value, "program_id")?)?;
    closed(program, &["sponsor", "name"])?;
    account(program, "sponsor")?;
    let name = text(program, "name", 255)?;
    if name.contains('/') {
        return Err("invalid FI sponsor program name".into());
    }
    positive_u64(value, "program_revision")?;
    if !field(value, "gas_limit")?.is_null() {
        positive_u64(value, "gas_limit")?;
    }
    let charges = field(value, "charge_limits")?
        .as_array()
        .ok_or("invalid FI fee charge array")?;
    if charges.len() > 2 {
        return Err("too many FI fee charge kinds".into());
    }
    let mut previous_kind = None;
    for charge in charges {
        let charge = object(charge)?;
        closed(charge, &["kind", "asset_definition_id", "max_amount"])?;
        let kind = object(field(charge, "kind")?)?;
        closed(kind, &["kind", "value"])?;
        let current_kind = match text(kind, "kind", 32)? {
            "nexus" => 0,
            "pipeline_gas" => 1,
            _ => return Err("unsupported FI fee charge kind".into()),
        };
        if !field(kind, "value")?.is_null()
            || previous_kind.is_some_and(|previous| previous >= current_kind)
        {
            return Err("noncanonical FI fee charge kinds".into());
        }
        previous_kind = Some(current_kind);
        asset(charge, "asset_definition_id")?;
        text(charge, "max_amount", 256)?;
    }
    Ok(format!("{}/{}", text(program, "sponsor", 1024)?, name))
}

/// Parser fixture only: no signed ledger or proof-authority claim.
#[cfg(test)]
pub(crate) fn fixture_request_v1(ios: bool) -> Vec<u8> {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_model_base::domain::DomainId;
    let wallet = AccountId::new(
        KeyPair::try_from_seed(vec![21; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
    .to_string();
    let fi = AccountId::new(
        KeyPair::try_from_seed(vec![22; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
    .to_string();
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("mobile-fi-parser", "universal").unwrap(),
        "fixture".parse().unwrap(),
    )
    .to_string();
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"FI parser fixture only",
    )))
    .to_string();
    let mut scope = norito::json!({
        "accountId": (wallet.clone()), "bank": "bank-fixture", "participantScope": "bank-fixture.universal",
        "coreBaseUrl": "https://example.invalid/core", "chainId": "fi-parser-fixture", "networkId": (network.clone()),
        "assetDefinitionId": (asset.clone()),
        "fees": {"payer": "sponsor", "value": {"program_id": {"sponsor": (wallet.clone()), "name": "demo"},
            "program_revision": 1, "charge_limits": [], "gas_limit": null}}
    });
    let target = norito::json!({
        "assetDefinitionId": (asset.clone()), "accountId": (wallet.clone()), "fiAccountId": (fi.clone()),
        "referenceDigest": ("33".repeat(32)), "fiRevision": 9_007_199_254_740_993_u64,
        "attestationRevision": 2, "revision": 3, "expectedPolicyRevision": 4,
        "enabled": true, "controller": null, "waterfall": false, "reverseWaterfall": false
    });
    let request = if ios {
        scope.as_object_mut().unwrap().insert(
            "feeSponsorProgramId".into(),
            Value::from(format!("{wallet}/demo")),
        );
        norito::json!({
            "schema": "boi.wallet-fi-context-evidence.ios.v1",
            "binding": {"schema": "boi.wallet-fi-context.v1", "walletScope": scope,
                "runtimeManifestDigest": ("44".repeat(32)), "uid": ("55".repeat(32)),
                "fiParticipant": "fi-fixture", "fiAccountId": fi, "uidController": null},
            "ownerKeyIdentity": "protected-key-generation-7", "minimumCertifiedHeight": 9_007_199_254_740_993_u64,
            "challengeHex": ("66".repeat(32)), "target": target
        })
    } else {
        norito::json!({
            "schema": "boi.wallet-fi-context-evidence.android.v1", "walletScope": (json::to_json(&scope).unwrap()),
            "runtimeManifestDigest": ("44".repeat(32)), "ownerKeyIdentity": "protected-key-generation-7",
            "networkId": network, "chainId": "fi-parser-fixture", "assetDefinitionId": asset,
            "walletAccount": wallet, "uid": ("55".repeat(32)), "fiParticipant": "fi-fixture",
            "referenceDigest": ("33".repeat(32)), "minimumCertifiedHeight": 9_007_199_254_740_993_u64,
            "challengeHex": ("66".repeat(32)), "target": target
        })
    };
    json::to_vec(&request).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(ios: bool) -> Value {
        json::from_slice_value(&fixture_request_v1(ios)).unwrap()
    }

    fn encoded(value: &Value) -> Vec<u8> {
        json::to_vec(value).unwrap()
    }

    #[test]
    fn both_platforms_preserve_exact_original_digest_and_same_complete_target() {
        let ios_bytes = fixture_request_v1(true);
        let android_bytes = fixture_request_v1(false);
        let ios = normalize_asset_fi_context_request_v1(&ios_bytes).unwrap();
        let android = normalize_asset_fi_context_request_v1(&android_bytes).unwrap();
        assert_eq!(
            ios.original_sha256,
            <[u8; 32]>::from(Sha256::digest(&ios_bytes))
        );
        assert_eq!(
            android.original_sha256,
            <[u8; 32]>::from(Sha256::digest(&android_bytes))
        );
        assert_ne!(ios.original_sha256, android.original_sha256);
        assert_eq!(ios.target, android.target);
        assert_eq!(ios.network_id, android.network_id);
        assert_eq!(ios.wallet, android.wallet);
        assert_eq!(ios.asset_id, android.asset_id);
        assert_eq!(ios.minimum_height, 9_007_199_254_740_993);
        assert_eq!(ios.challenge, [0x66; 32]);
        assert_eq!(ios.uid, [0x55; 32]);
        assert_eq!(ios.runtime_manifest_sha256, [0x44; 32]);
        assert_eq!(ios.owner_key_identity, "protected-key-generation-7");
        assert_eq!(ios.declared_uid_controller, Some(None));
        assert_eq!(android.declared_uid_controller, None);
        assert_eq!(ios.fi_participant, "fi-fixture");
        let AssetFiContextTargetV1::LinkApproval(approval) = ios.target else {
            panic!("wrong kind");
        };
        assert_eq!(approval.link.fi_revision, 9_007_199_254_740_993);
        assert_eq!(approval.link.attestation_revision, 2);
        assert_eq!(approval.link.revision, 3);
        assert_eq!(approval.expected_policy_revision, 4);
        assert_eq!(approval.link.reference_digest, [0x33; 32]);
        assert_eq!(
            normalize_asset_fi_link_target_v1(&parsed(true)["target"]).unwrap(),
            approval
        );
        assert!(approval.link.enabled);
        assert!(approval.link.controller.is_none());
        assert!(!approval.link.waterfall && !approval.link.reverse_waterfall);
        assert_eq!(
            ios.wallet_scope_canonical,
            encoded(&parsed(true)["binding"]["walletScope"])
        );
        assert_eq!(
            android.wallet_scope_canonical,
            parsed(false)["walletScope"].as_str().unwrap().as_bytes()
        );
    }

    #[test]
    fn rejects_noncanonical_duplicate_unknown_trailing_and_oversized_raw() {
        for ios in [false, true] {
            let raw = fixture_request_v1(ios);
            let original = String::from_utf8(raw.clone()).unwrap();
            for invalid in [
                format!(" {original}"),
                format!("{original}\n"),
                original.replacen('{', "{\"unknown\":null,", 1),
                original.replacen('{', "{\"schema\":\"duplicate\",", 1),
                original.replace("https://", "https:\\/\\/"),
                format!("{original}{{}}"),
            ] {
                assert!(normalize_asset_fi_context_request_v1(invalid.as_bytes()).is_err());
            }
            let mut invalid = parsed(ios);
            invalid
                .as_object_mut()
                .unwrap()
                .insert("operation".into(), Value::from("Transfer"));
            assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
        }
        assert!(normalize_asset_fi_context_request_v1(&[]).is_err());
        assert!(normalize_asset_fi_context_request_v1(&[0xff]).is_err());
        assert!(
            normalize_asset_fi_context_request_v1(&vec![
                b' ';
                MAX_ASSET_FI_CONTEXT_REQUEST_BYTES_V1 + 1
            ])
            .is_err()
        );
    }

    #[test]
    fn rejects_float_exponent_string_boolean_zero_and_overflow_numbers() {
        for ios in [false, true] {
            let original = String::from_utf8(fixture_request_v1(ios)).unwrap();
            for literal in [
                "0",
                "-1",
                "1.0",
                "1e0",
                "true",
                "\"1\"",
                "18446744073709551616",
                "18446744073709551615",
            ] {
                let invalid = original.replace(
                    "\"minimumCertifiedHeight\":9007199254740993",
                    &format!("\"minimumCertifiedHeight\":{literal}"),
                );
                assert!(
                    normalize_asset_fi_context_request_v1(invalid.as_bytes()).is_err(),
                    "{literal}"
                );
            }
            for key in [
                "fiRevision",
                "attestationRevision",
                "revision",
                "expectedPolicyRevision",
            ] {
                let mut invalid = parsed(ios);
                invalid
                    .as_object_mut()
                    .unwrap()
                    .get_mut("target")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), Value::from(0));
                assert!(
                    normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err(),
                    "{key}"
                );
            }
            let max = original.replace(
                "\"fiRevision\":9007199254740993",
                "\"fiRevision\":18446744073709551615",
            );
            let accepted = normalize_asset_fi_context_request_v1(max.as_bytes()).unwrap();
            let AssetFiContextTargetV1::LinkApproval(approval) = accepted.target else {
                panic!("wrong kind");
            };
            assert_eq!(approval.link.fi_revision, u64::MAX);
        }
    }

    #[test]
    fn rejects_incomplete_target_or_new_operation_and_binds_redundant_scope() {
        for ios in [false, true] {
            for key in [
                "assetDefinitionId",
                "accountId",
                "fiAccountId",
                "referenceDigest",
                "fiRevision",
                "attestationRevision",
                "revision",
                "expectedPolicyRevision",
                "enabled",
                "controller",
                "waterfall",
                "reverseWaterfall",
            ] {
                let mut invalid = parsed(ios);
                invalid
                    .as_object_mut()
                    .unwrap()
                    .get_mut("target")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err(),
                    "{key}"
                );
            }
            let mut invalid = parsed(ios);
            invalid
                .as_object_mut()
                .unwrap()
                .insert("target".into(), norito::json!({"Transfer": {}}));
            assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
            let mut invalid = parsed(ios);
            let target = invalid
                .as_object_mut()
                .unwrap()
                .get_mut("target")
                .unwrap()
                .as_object_mut()
                .unwrap();
            target.insert("accountId".into(), target["fiAccountId"].clone());
            assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
        }
        for key in [
            "networkId",
            "chainId",
            "assetDefinitionId",
            "walletAccount",
            "referenceDigest",
        ] {
            let mut invalid = parsed(false);
            invalid
                .as_object_mut()
                .unwrap()
                .insert(key.into(), Value::from("different"));
            assert!(
                normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err(),
                "{key}"
            );
        }
        let mut invalid = parsed(true);
        invalid
            .as_object_mut()
            .unwrap()
            .get_mut("binding")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("fiAccountId".into(), Value::from("different"));
        assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
    }

    #[test]
    fn closes_nested_scope_binding_and_fees_and_preserves_platform_encoding() {
        for ios in [false, true] {
            let original = parsed(ios);
            let scope = if ios {
                original["binding"]["walletScope"].clone()
            } else {
                json::from_slice_value(original["walletScope"].as_str().unwrap().as_bytes())
                    .unwrap()
            };
            for key in ["fiContext", "fiContextV1", "unknown"] {
                let mut altered_scope = scope.clone();
                altered_scope
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), Value::Null);
                let mut altered = original.clone();
                if ios {
                    altered
                        .as_object_mut()
                        .unwrap()
                        .get_mut("binding")
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .insert("walletScope".into(), altered_scope);
                } else {
                    altered.as_object_mut().unwrap().insert(
                        "walletScope".into(),
                        Value::from(json::to_json(&altered_scope).unwrap()),
                    );
                }
                assert!(normalize_asset_fi_context_request_v1(&encoded(&altered)).is_err());
            }
        }
        let mut invalid = parsed(false);
        let scope = invalid["walletScope"].as_str().unwrap().to_owned();
        invalid
            .as_object_mut()
            .unwrap()
            .insert("walletScope".into(), Value::from(format!(" {scope}")));
        assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
        let mut invalid = parsed(true);
        invalid
            .as_object_mut()
            .unwrap()
            .get_mut("binding")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), Value::Null);
        assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
        let mut invalid = parsed(true);
        let scope = invalid
            .as_object_mut()
            .unwrap()
            .get_mut("binding")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("walletScope")
            .unwrap()
            .as_object_mut()
            .unwrap();
        scope.insert(
            "feeSponsorProgramId".into(),
            Value::from("different/program"),
        );
        assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
        let mut invalid = parsed(true);
        invalid
            .as_object_mut()
            .unwrap()
            .get_mut("binding")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("walletScope")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("fees")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("value")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), Value::Null);
        assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
    }

    #[test]
    fn rejects_zero_or_noncanonical_hashes_invalid_accounts_and_capabilities() {
        for ios in [false, true] {
            for challenge in ["00".repeat(32), "AA".repeat(32), "ab".repeat(31)] {
                let mut invalid = parsed(ios);
                invalid
                    .as_object_mut()
                    .unwrap()
                    .insert("challengeHex".into(), Value::from(challenge));
                assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
            }
            for key in ["waterfall", "reverseWaterfall"] {
                let mut invalid = parsed(ios);
                invalid
                    .as_object_mut()
                    .unwrap()
                    .get_mut("target")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), Value::from(true));
                assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
            }
            let mut invalid = parsed(ios);
            invalid
                .as_object_mut()
                .unwrap()
                .get_mut("target")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("fiAccountId".into(), Value::from("alias@universal"));
            assert!(normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err());
        }
    }

    /// One exact transfer request, sharing the link fixture's platform envelope.
    fn transfer_value(ios: bool) -> Value {
        let mut value = parsed(ios);
        let request = value.as_object_mut().unwrap();
        let link = request["target"].clone();
        let schema = if ios {
            "boi.wallet-fi-context-evidence.ios.transfer.v1"
        } else {
            "boi.wallet-fi-context-evidence.android.transfer.v1"
        };
        request.insert("schema".into(), Value::from(schema));
        request.insert(
            "target".into(),
            norito::json!({
                "assetDefinitionId": (link["assetDefinitionId"].clone()),
                "accountId": (link["accountId"].clone()),
                "amount": "10.00",
                "expectedLinkRevision": 3,
                "expectedPolicyRevision": 4,
                "fiAccountId": (link["fiAccountId"].clone()),
                "referenceDigest": (link["referenceDigest"].clone()),
                "expectedFiRevision": 9_007_199_254_740_993_u64,
                "expectedAttestationRevision": 2
            }),
        );
        value
    }

    fn transfer_target_mut(value: &mut Value) -> &mut Object {
        value
            .as_object_mut()
            .unwrap()
            .get_mut("target")
            .unwrap()
            .as_object_mut()
            .unwrap()
    }

    fn with_target_field(ios: bool, key: &str, replacement: Value) -> Vec<u8> {
        let mut value = transfer_value(ios);
        transfer_target_mut(&mut value).insert(key.into(), replacement);
        encoded(&value)
    }

    fn agorot(minor: u64) -> Quantity {
        Quantity::try_from(Numeric::try_new(minor, 2).unwrap()).unwrap()
    }

    #[test]
    fn transfer_schemas_normalize_both_platforms_to_the_same_exact_target() {
        let ios_bytes = encoded(&transfer_value(true));
        let android_bytes = encoded(&transfer_value(false));
        let ios = normalize_asset_fi_context_request_v1(&ios_bytes).unwrap();
        let android = normalize_asset_fi_context_request_v1(&android_bytes).unwrap();
        assert_eq!(
            ios.original_sha256,
            <[u8; 32]>::from(Sha256::digest(&ios_bytes))
        );
        assert_eq!(
            android.original_sha256,
            <[u8; 32]>::from(Sha256::digest(&android_bytes))
        );
        assert_ne!(ios.original_sha256, android.original_sha256);
        assert_eq!(ios.target, android.target);
        // A transfer keeps iOS's independent UID-controller declaration and invents
        // no Android one, exactly as the link formats do.
        assert_eq!(ios.declared_uid_controller, Some(None));
        assert_eq!(android.declared_uid_controller, None);
        assert_eq!(
            ios.wallet_scope_canonical,
            encoded(&parsed(true)["binding"]["walletScope"])
        );
        assert_eq!(
            android.wallet_scope_canonical,
            parsed(false)["walletScope"].as_str().unwrap().as_bytes()
        );
        let AssetFiContextTargetV1::Transfer {
            instruction,
            fi_account_id,
            reference_digest,
            expected_fi_revision,
            expected_attestation_revision,
        } = ios.target
        else {
            panic!("wrong kind");
        };
        assert_eq!(instruction.asset_definition_id, ios.asset_id);
        assert_eq!(instruction.account_id, ios.wallet);
        assert_eq!(instruction.amount, agorot(1000));
        assert_eq!(instruction.expected_link_revision, 3);
        assert_eq!(instruction.expected_policy_revision, 4);
        assert_ne!(fi_account_id, ios.wallet);
        assert_eq!(reference_digest, [0x33; 32]);
        assert_eq!(expected_fi_revision, 9_007_199_254_740_993);
        assert_eq!(expected_attestation_revision, 2);
    }

    #[test]
    fn transfer_amount_is_exact_positive_two_decimal_minor_units() {
        for ios in [false, true] {
            for (text, minor) in [
                ("0.01", 1),
                ("0.10", 10),
                ("1.00", 100),
                ("10.01", 1001),
                ("184467440737095516.15", u64::MAX),
            ] {
                let request = normalize_asset_fi_context_request_v1(&with_target_field(
                    ios,
                    "amount",
                    Value::from(text),
                ))
                .unwrap();
                let AssetFiContextTargetV1::Transfer { instruction, .. } = request.target else {
                    panic!("wrong kind");
                };
                assert_eq!(instruction.amount, agorot(minor), "{text}");
            }
            for text in [
                "0.00",
                "00.01",
                "010.00",
                "+10.00",
                "-10.00",
                "10",
                "10.0",
                "10.000",
                ".10",
                "10.",
                "1e1",
                "1.0e1",
                "1,000.00",
                " 10.00",
                "10.00 ",
                "\u{0661}\u{0660}.\u{0660}\u{0660}",
                "184467440737095516.16",
                "184467440737095517.00",
                "999999999999999999.99",
                "1000000000000000000.00",
            ] {
                assert!(
                    normalize_asset_fi_context_request_v1(&with_target_field(
                        ios,
                        "amount",
                        Value::from(text),
                    ))
                    .is_err(),
                    "{text}"
                );
            }
            for number in [
                Value::from(10_u64),
                json::from_slice_value(b"10.5").unwrap(),
                Value::from(true),
                Value::Null,
            ] {
                assert!(
                    normalize_asset_fi_context_request_v1(&with_target_field(
                        ios, "amount", number,
                    ))
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn transfer_revisions_preserve_every_u64_and_refuse_zero_or_coercions() {
        for ios in [false, true] {
            for key in [
                "expectedLinkRevision",
                "expectedPolicyRevision",
                "expectedFiRevision",
                "expectedAttestationRevision",
            ] {
                for revision in [9_007_199_254_740_993_u64, u64::MAX - 1, u64::MAX] {
                    let request = normalize_asset_fi_context_request_v1(&with_target_field(
                        ios,
                        key,
                        Value::from(revision),
                    ))
                    .unwrap();
                    let AssetFiContextTargetV1::Transfer {
                        instruction,
                        expected_fi_revision,
                        expected_attestation_revision,
                        ..
                    } = request.target
                    else {
                        panic!("wrong kind");
                    };
                    assert_eq!(
                        match key {
                            "expectedLinkRevision" => instruction.expected_link_revision,
                            "expectedPolicyRevision" => instruction.expected_policy_revision,
                            "expectedFiRevision" => expected_fi_revision,
                            _ => expected_attestation_revision,
                        },
                        revision,
                        "{key}"
                    );
                }
                let original =
                    String::from_utf8(with_target_field(ios, key, Value::from(7_u64))).unwrap();
                for literal in [
                    "0",
                    "-1",
                    "1.0",
                    "1e0",
                    "true",
                    "null",
                    "\"1\"",
                    "18446744073709551616",
                ] {
                    let invalid =
                        original.replace(&format!("\"{key}\":7"), &format!("\"{key}\":{literal}"));
                    assert_ne!(invalid, original, "{key}");
                    assert!(
                        normalize_asset_fi_context_request_v1(invalid.as_bytes()).is_err(),
                        "{key}: {literal}"
                    );
                }
            }
        }
    }

    #[test]
    fn transfer_and_link_schemas_and_targets_stay_disjoint() {
        for ios in [false, true] {
            let transfer = transfer_value(ios);
            let link = parsed(ios);
            let mut mixed = transfer.clone();
            mixed
                .as_object_mut()
                .unwrap()
                .insert("schema".into(), link["schema"].clone());
            assert!(normalize_asset_fi_context_request_v1(&encoded(&mixed)).is_err());
            let mut mixed = link.clone();
            mixed
                .as_object_mut()
                .unwrap()
                .insert("schema".into(), transfer["schema"].clone());
            assert!(normalize_asset_fi_context_request_v1(&encoded(&mixed)).is_err());
            assert!(normalize_asset_fi_link_target_v1(&transfer["target"]).is_err());
            let AssetFiContextTargetV1::LinkApproval(_) =
                normalize_asset_fi_context_request_v1(&encoded(&link))
                    .unwrap()
                    .target
            else {
                panic!("the link schema selected another operation");
            };
            let mut extra = transfer.clone();
            extra
                .as_object_mut()
                .unwrap()
                .insert("operation".into(), Value::from("LinkApproval"));
            assert!(normalize_asset_fi_context_request_v1(&encoded(&extra)).is_err());
        }
    }

    #[test]
    fn transfer_target_is_closed_against_capabilities_destinations_and_omissions() {
        for ios in [false, true] {
            for key in [
                "enabled",
                "controller",
                "waterfall",
                "reverseWaterfall",
                "revision",
                "fiRevision",
                "attestationRevision",
                "destination",
                "unknown",
            ] {
                assert!(
                    normalize_asset_fi_context_request_v1(&with_target_field(
                        ios,
                        key,
                        Value::Null
                    ))
                    .is_err(),
                    "{key}"
                );
            }
            for key in [
                "assetDefinitionId",
                "accountId",
                "amount",
                "expectedLinkRevision",
                "expectedPolicyRevision",
                "fiAccountId",
                "referenceDigest",
                "expectedFiRevision",
                "expectedAttestationRevision",
            ] {
                let mut invalid = transfer_value(ios);
                transfer_target_mut(&mut invalid).remove(key);
                assert!(
                    normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err(),
                    "{key}"
                );
            }
            for reference in ["00".repeat(32), "AA".repeat(32), "ab".repeat(31)] {
                assert!(
                    normalize_asset_fi_context_request_v1(&with_target_field(
                        ios,
                        "referenceDigest",
                        Value::from(reference),
                    ))
                    .is_err()
                );
            }
            assert!(
                normalize_asset_fi_context_request_v1(&with_target_field(
                    ios,
                    "fiAccountId",
                    Value::from("alias@universal"),
                ))
                .is_err()
            );
            let oversized = with_target_field(ios, "unknown", Value::from("x".repeat(17 * 1024)));
            assert_eq!(
                normalize_asset_fi_context_request_v1(&oversized).unwrap_err(),
                "FI target exceeds bound"
            );
        }
    }

    #[test]
    fn transfer_binds_wallet_destination_and_redundant_platform_selectors() {
        for ios in [false, true] {
            let mut invalid = transfer_value(ios);
            let target = transfer_target_mut(&mut invalid);
            target.insert("fiAccountId".into(), target["accountId"].clone());
            assert_eq!(
                normalize_asset_fi_context_request_v1(&encoded(&invalid)).unwrap_err(),
                "invalid FI target capability or wallet destination"
            );
            // A well-formed target for another wallet or asset differs from the scope.
            let other_wallet = AccountId::new(
                iroha_crypto::KeyPair::try_from_seed(
                    vec![23; 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
                .unwrap()
                .public_key()
                .clone(),
            )
            .to_string();
            assert_eq!(
                normalize_asset_fi_context_request_v1(&with_target_field(
                    ios,
                    "accountId",
                    Value::from(other_wallet),
                ))
                .unwrap_err(),
                "FI target differs from retained wallet scope"
            );
            let other_asset = AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("mobile-fi-other", "universal")
                    .unwrap(),
                "fixture".parse().unwrap(),
            )
            .to_string();
            assert_eq!(
                normalize_asset_fi_context_request_v1(&with_target_field(
                    ios,
                    "assetDefinitionId",
                    Value::from(other_asset),
                ))
                .unwrap_err(),
                "FI target differs from retained wallet scope"
            );
        }
        for key in [
            "networkId",
            "chainId",
            "assetDefinitionId",
            "walletAccount",
            "referenceDigest",
        ] {
            let mut invalid = transfer_value(false);
            invalid
                .as_object_mut()
                .unwrap()
                .insert(key.into(), Value::from("different"));
            assert!(
                normalize_asset_fi_context_request_v1(&encoded(&invalid)).is_err(),
                "{key}"
            );
        }
        let mut invalid = transfer_value(false);
        invalid
            .as_object_mut()
            .unwrap()
            .insert("referenceDigest".into(), Value::from("67".repeat(32)));
        assert_eq!(
            normalize_asset_fi_context_request_v1(&encoded(&invalid)).unwrap_err(),
            "FI request differs from target reference"
        );
        let mut invalid = transfer_value(true);
        invalid
            .as_object_mut()
            .unwrap()
            .get_mut("binding")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("fiAccountId".into(), Value::from("different"));
        assert_eq!(
            normalize_asset_fi_context_request_v1(&encoded(&invalid)).unwrap_err(),
            "FI target differs from retained FI account"
        );
    }

    #[test]
    fn transfer_substituted_selectors_still_normalize_for_the_caller_to_compare() {
        // Normalization authenticates nothing: a self-consistent changed selector is
        // returned exactly, so the caller's retained-consent comparison must refuse it.
        for ios in [false, true] {
            let original =
                normalize_asset_fi_context_request_v1(&encoded(&transfer_value(ios))).unwrap();
            for (key, replacement) in [
                ("amount", Value::from("10.01")),
                ("expectedLinkRevision", Value::from(8_u64)),
                ("expectedPolicyRevision", Value::from(8_u64)),
                ("expectedFiRevision", Value::from(8_u64)),
                ("expectedAttestationRevision", Value::from(8_u64)),
            ] {
                let changed = normalize_asset_fi_context_request_v1(&with_target_field(
                    ios,
                    key,
                    replacement,
                ))
                .unwrap();
                assert_ne!(changed.target, original.target, "{key}");
            }
        }
    }

    #[test]
    fn transfer_rejects_noncanonical_duplicate_and_trailing_raw_documents() {
        for ios in [false, true] {
            let original = String::from_utf8(encoded(&transfer_value(ios))).unwrap();
            for invalid in [
                format!(" {original}"),
                format!("{original}\n"),
                format!("{original}{{}}"),
                original.replacen('{', "{\"unknown\":null,", 1),
                original.replacen('{', "{\"schema\":\"duplicate\",", 1),
                original.replace(
                    "\"amount\":\"10.00\"",
                    "\"amount\":\"10.00\",\"amount\":\"10.00\"",
                ),
                original.replace("https://", "https:\\/\\/"),
            ] {
                assert_ne!(invalid, original);
                assert!(normalize_asset_fi_context_request_v1(invalid.as_bytes()).is_err());
            }
        }
    }
}
