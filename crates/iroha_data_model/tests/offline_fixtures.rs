//! Regression tests for offline allowance fixtures.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hex::encode_upper;
use iroha_crypto::{Hash, Signature};
use iroha_data_model::{account::AccountId, asset::AssetId};
use iroha_data_model::{
    metadata::Metadata,
    offline::{OfflineAllowanceCommitment, OfflineWalletCertificate, OfflineWalletPolicy},
};
use norito::{
    decode_from_bytes,
    json::{self, Value as JsonValue},
};

#[test]
fn offline_allowance_fixtures_roundtrip() {
    let root = fixture_root();
    let manifest_bytes =
        fs::read(root.join("allowance_fixtures.manifest.json")).expect("read fixture manifest");
    let manifest = json::from_slice_value(&manifest_bytes).expect("parse manifest json");
    let allowances = manifest
        .get("allowances")
        .and_then(JsonValue::as_array)
        .expect("allowances array");

    for entry in allowances {
        let label = entry
            .get("label")
            .and_then(JsonValue::as_str)
            .expect("label field");
        let certificate_path = fixture_path(&root, entry, "certificate_norito");
        let certificate_json_path = fixture_path(&root, entry, "certificate_json");

        let certificate_bytes = fs::read(&certificate_path).expect("read certificate.norito");
        let certificate = decode_from_bytes::<OfflineWalletCertificate>(&certificate_bytes)
            .expect("decode certificate");

        let disk_certificate =
            parse_certificate_json(&read_json_value(&certificate_json_path), label);
        let disk_value = normalize_certificate_json(certificate_to_value(&disk_certificate));
        let canonical_value = normalize_certificate_json(certificate_to_value(&certificate));
        assert_eq!(
            disk_value, canonical_value,
            "certificate JSON mismatch for allowance `{label}`"
        );

        let actual_keys = metadata_key_list(&certificate.metadata);
        let manifest_keys = entry
            .get("metadata_keys")
            .and_then(JsonValue::as_array)
            .expect("metadata_keys array")
            .iter()
            .map(|value| value.as_str().expect("metadata key string").to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            manifest_keys, actual_keys,
            "metadata key list mismatch for allowance `{label}`"
        );

        let expected_nonce = certificate
            .attestation_nonce
            .as_ref()
            .map(|hash| encode_upper(hash.as_ref()));
        let manifest_nonce = entry.get("attestation_nonce_hex").and_then(|value| {
            if value.is_null() {
                None
            } else {
                value.as_str().map(str::to_string)
            }
        });
        assert_eq!(
            manifest_nonce, expected_nonce,
            "attestation nonce mismatch for `{label}`"
        );

        let manifest_refresh = entry.get("refresh_at_ms").and_then(JsonValue::as_u64);
        assert_eq!(
            manifest_refresh, certificate.refresh_at_ms,
            "refresh timestamp mismatch for `{label}`"
        );
    }
}

#[test]
#[ignore = "regenerates fixtures for offline allowances"]
fn regenerate_offline_allowance_certificates() {
    let root = fixture_root();
    let manifest_bytes =
        fs::read(root.join("allowance_fixtures.manifest.json")).expect("read fixture manifest");
    let manifest: JsonValue = json::from_slice_value(&manifest_bytes).expect("parse manifest json");
    let allowances = manifest
        .get("allowances")
        .and_then(JsonValue::as_array)
        .expect("allowances array");

    for entry in allowances {
        let label = entry
            .get("label")
            .and_then(JsonValue::as_str)
            .expect("label field");
        let certificate_json_path = fixture_path(&root, entry, "certificate_json");
        let certificate_bytes = fs::read(&certificate_json_path)
            .unwrap_or_else(|err| panic!("read certificate json for `{label}`: {err}"));
        let certificate_value: JsonValue = json::from_slice_value(&certificate_bytes)
            .unwrap_or_else(|err| panic!("parse certificate json for `{label}`: {err}"));
        let certificate: OfflineWalletCertificate =
            parse_certificate_json(&certificate_value, label);
        let encoded = norito::to_bytes(&certificate)
            .unwrap_or_else(|err| panic!("encode certificate `{label}`: {err}"));
        let certificate_out = fixture_path(&root, entry, "certificate_norito");
        fs::write(&certificate_out, encoded)
            .unwrap_or_else(|err| panic!("write certificate norito for `{label}`: {err}"));
    }
}

fn fixture_path(root: &Path, entry: &JsonValue, field: &str) -> PathBuf {
    let rel = entry
        .get(field)
        .and_then(JsonValue::as_str)
        .unwrap_or_else(|| panic!("missing `{field}` field"));
    root.join(rel)
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("offline_allowance")
}

fn read_json_value(path: &Path) -> JsonValue {
    let bytes = fs::read(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    json::from_slice_value(&bytes).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    })
}

#[allow(clippy::too_many_lines)]
fn parse_certificate_json(value: &JsonValue, label: &str) -> OfflineWalletCertificate {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("certificate `{label}` JSON must be an object"));
    let allowance = obj
        .get("allowance")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| panic!("certificate `{label}` missing allowance block"));
    let allowance_asset = allowance
        .get("asset")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("certificate `{label}` allowance missing asset"));
    let controller = parse_account_literal_strict(
        obj.get("controller")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("certificate `{label}` missing controller")),
        label,
        "controller",
    );
    let operator = parse_account_literal_strict(
        obj.get("operator")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("certificate `{label}` missing operator")),
        label,
        "operator",
    );
    let asset = parse_asset_literal_strict(allowance_asset, label, "allowance asset");
    let amount_str = allowance
        .get("amount")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("certificate `{label}` allowance missing amount"));
    let amount = amount_str
        .parse()
        .unwrap_or_else(|err| panic!("parse allowance amount `{amount_str}`: {err}"));
    let commitment_hex = allowance
        .get("commitment_hex")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("certificate `{label}` allowance missing commitment_hex"));
    let commitment =
        hex::decode(commitment_hex).unwrap_or_else(|err| panic!("commitment hex: {err}"));
    let spend_public_key = obj
        .get("spend_public_key")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("certificate `{label}` missing spend_public_key"));
    let spend_public_key = spend_public_key
        .parse()
        .unwrap_or_else(|err| panic!("invalid spend public key `{spend_public_key}`: {err}"));
    let attestation_report_b64 = obj
        .get("attestation_report_b64")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("certificate `{label}` missing attestation_report_b64"));
    let attestation_report = BASE64
        .decode(attestation_report_b64)
        .unwrap_or_else(|err| panic!("attestation base64: {err}"));
    let issued_at_ms = obj
        .get("issued_at_ms")
        .and_then(JsonValue::as_u64)
        .unwrap_or_else(|| panic!("certificate `{label}` missing issued_at_ms"));
    let expires_at_ms = obj
        .get("expires_at_ms")
        .and_then(JsonValue::as_u64)
        .unwrap_or_else(|| panic!("certificate `{label}` missing expires_at_ms"));
    let policy = obj
        .get("policy")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| panic!("certificate `{label}` missing policy"));
    let max_balance = policy
        .get("max_balance")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("policy missing max_balance for `{label}`"));
    let max_tx_value = policy
        .get("max_tx_value")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("policy missing max_tx_value for `{label}`"));
    let policy_expires = policy
        .get("expires_at_ms")
        .and_then(JsonValue::as_u64)
        .unwrap_or_else(|| panic!("policy missing expires_at_ms for `{label}`"));
    let operator_signature_hex = obj
        .get("operator_signature_hex")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("certificate `{label}` missing operator_signature_hex"));
    let operator_signature = Signature::from_hex(operator_signature_hex)
        .unwrap_or_else(|err| panic!("operator signature hex for `{label}`: {err}"));
    let metadata_value = obj
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| JsonValue::Object(BTreeMap::default()));
    let metadata: Metadata = json::from_value(metadata_value)
        .unwrap_or_else(|err| panic!("parse metadata for `{label}`: {err}"));
    let verdict_id = parse_optional_hash(obj.get("verdict_id_hex"), label);
    let attestation_nonce = parse_optional_hash(obj.get("attestation_nonce_hex"), label);
    let refresh_at_ms = obj.get("refresh_at_ms").and_then(JsonValue::as_u64);

    OfflineWalletCertificate {
        controller,
        operator,
        allowance: OfflineAllowanceCommitment {
            asset,
            amount,
            commitment,
        },
        spend_public_key,
        attestation_report,
        issued_at_ms,
        expires_at_ms,
        policy: OfflineWalletPolicy {
            max_balance: max_balance
                .parse()
                .unwrap_or_else(|err| panic!("max_balance parse for `{label}`: {err}")),
            max_tx_value: max_tx_value
                .parse()
                .unwrap_or_else(|err| panic!("max_tx_value parse for `{label}`: {err}")),
            expires_at_ms: policy_expires,
        },
        operator_signature,
        metadata,
        verdict_id,
        attestation_nonce,
        refresh_at_ms,
    }
}

fn parse_account_literal_strict(account_literal: &str, label: &str, field: &str) -> AccountId {
    AccountId::parse_encoded(account_literal)
        .unwrap_or_else(|err| {
            panic!("parse {field} `{account_literal}` for `{label}` failed: {err}")
        })
        .into_account_id()
}

fn parse_asset_literal_strict(asset_literal: &str, label: &str, field: &str) -> AssetId {
    AssetId::parse_encoded(asset_literal)
        .unwrap_or_else(|err| panic!("parse {field} `{asset_literal}` for `{label}` failed: {err}"))
}

fn parse_optional_hash(value: Option<&JsonValue>, label: &str) -> Option<Hash> {
    value.and_then(|v| {
        v.as_str().map(|hex| {
            Hash::from_str(hex)
                .unwrap_or_else(|err| panic!("invalid hash `{hex}` for allowance `{label}`: {err}"))
        })
    })
}

fn normalize_certificate_json(mut value: JsonValue) -> JsonValue {
    if let Some(root) = value.as_object_mut() {
        if let Some(JsonValue::String(controller)) = root.get_mut("controller")
            && let Ok(account) = AccountId::parse_encoded(controller)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        {
            *controller = account.to_string();
        }
        if let Some(JsonValue::String(operator)) = root.get_mut("operator")
            && let Ok(account) = AccountId::parse_encoded(operator)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        {
            *operator = account.to_string();
        }
        if let Some(JsonValue::Object(allowance)) = root.get_mut("allowance") {
            if let Some(JsonValue::String(asset)) = allowance.get_mut("asset")
                && let Ok(parsed_asset) = AssetId::parse_encoded(asset)
            {
                *asset = parsed_asset.to_string();
            }
            allowance.remove("commitment");
        }
        root.remove("attestation_report");
        root.remove("operator_signature");
    }
    value
}

fn certificate_to_value(certificate: &OfflineWalletCertificate) -> JsonValue {
    let mut allowance = json::Map::new();
    allowance.insert(
        "asset".into(),
        json::Value::String(certificate.allowance.asset.to_string()),
    );
    allowance.insert(
        "amount".into(),
        json::Value::String(certificate.allowance.amount.to_string()),
    );
    allowance.insert(
        "commitment_hex".into(),
        json::Value::String(encode_upper(&certificate.allowance.commitment)),
    );

    let mut policy = json::Map::new();
    policy.insert(
        "max_balance".into(),
        json::Value::String(certificate.policy.max_balance.to_string()),
    );
    policy.insert(
        "max_tx_value".into(),
        json::Value::String(certificate.policy.max_tx_value.to_string()),
    );
    policy.insert(
        "expires_at_ms".into(),
        json::to_value(&certificate.policy.expires_at_ms).expect("serialize policy expiry"),
    );

    let mut root = json::Map::new();
    root.insert(
        "controller".into(),
        json::Value::String(certificate.controller.to_string()),
    );
    root.insert(
        "operator".into(),
        json::Value::String(certificate.operator.to_string()),
    );
    root.insert("allowance".into(), JsonValue::Object(allowance));
    root.insert(
        "spend_public_key".into(),
        json::Value::String(certificate.spend_public_key.to_string()),
    );
    root.insert(
        "attestation_report_b64".into(),
        json::Value::String(BASE64.encode(&certificate.attestation_report)),
    );
    root.insert(
        "issued_at_ms".into(),
        json::to_value(&certificate.issued_at_ms).expect("serialize issued_at_ms"),
    );
    root.insert(
        "expires_at_ms".into(),
        json::to_value(&certificate.expires_at_ms).expect("serialize expires_at_ms"),
    );
    root.insert("policy".into(), JsonValue::Object(policy));
    root.insert(
        "operator_signature_hex".into(),
        json::Value::String(encode_upper(certificate.operator_signature.payload())),
    );
    root.insert("metadata".into(), metadata_to_value(&certificate.metadata));
    root.insert(
        "verdict_id_hex".into(),
        certificate
            .verdict_id
            .as_ref()
            .map_or(JsonValue::Null, |hash| {
                JsonValue::String(encode_upper(hash.as_ref()))
            }),
    );
    root.insert(
        "attestation_nonce_hex".into(),
        certificate
            .attestation_nonce
            .as_ref()
            .map_or(JsonValue::Null, |hash| {
                JsonValue::String(encode_upper(hash.as_ref()))
            }),
    );
    root.insert(
        "refresh_at_ms".into(),
        certificate.refresh_at_ms.map_or(JsonValue::Null, |value| {
            json::to_value(&value).expect("serialize refresh_at_ms")
        }),
    );
    JsonValue::Object(root)
}

fn metadata_to_value(metadata: &Metadata) -> JsonValue {
    let mut map = json::Map::new();
    for (name, value) in metadata.iter() {
        let parsed = json::from_slice_value(value.get().as_bytes()).unwrap_or(JsonValue::Null);
        map.insert(name.to_string(), parsed);
    }
    JsonValue::Object(map)
}

fn metadata_key_list(metadata: &Metadata) -> Vec<String> {
    let mut keys = metadata
        .iter()
        .map(|(name, _)| name.to_string())
        .collect::<Vec<_>>();
    keys.sort();
    keys
}
