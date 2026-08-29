//! Canonical owner-only execution bundle for the isolated privacy wallet worker.
//!
//! The outer framing is deliberately binary.  In particular, the transaction
//! signer seed is never materialized as a JSON string.  Protocol-specific
//! witness bytes are decoded only after an authenticated execute request has
//! passed every public binding and transaction-plan check.
use crate::privacy_native_actions::{
    AnonymousPgcPaymentActionRequestV1, BootleLanternPresentationActionRequestV1,
    FcmpMembershipPaymentActionRequestV1, FcmpWalletOutputRequestV1, IvmPrivateNoteActionRequestV1,
    IvmPrivateNoteOutputRequestV1, JindoPolynomialEvaluationActionRequestV1,
    OrchardNoteActionRequestV1, PRIVACY_NATIVE_ACTION_MAX_SECRET_BUNDLE_BYTES_V1,
    PqMaspNoteActionRequestV1, PqMaspOutputRequestV1, PrivacyNativeActionRequestV1,
    VeRangeActionRequestV1, VegaCredentialPresentationActionRequestV1,
    ZkAceAuthorizationActionRequestV1, ZkAmsActionRequestV1, ZkAmsAdmissionCredentialRequestV1,
    ZkAmsBatchAdmissionActionRequestV1, ZkAmsProvisionAccountActionRequestV1,
    parse_canonical_public_balance_scope_v1, privacy_native_action_capability_for_protocol_v1,
};
use core::fmt;
use iroha_core::privacy_engines::{
    bootle_lantern::{relation::BootleLanternPresentationWitnessV1, ring::ApplicationPolynomialV1},
    fcmp_plus_plus::{
        FcmpInputRerandomizationV1, FcmpOutputTupleV1, FcmpProverInputV1, FcmpTreeRootV1,
        FcmpWalletNoteV1,
    },
    ivm_private_note::{
        IvmPrivateNoteInputWitnessV1, IvmPrivateNoteOutputWitnessV1, PrivateInstructionV1,
        PrivateNotePlaintextV1, PrivateProgramV1,
    },
    jindo::JindoPrivacyActionWitnessV1,
    orchard::{OrchardChangeProverInputV1, OrchardSpendProverInputV1},
    p256::{DeviceSigningKeyV1, SecretScalarV1},
    pq_masp::{PqMaspInputWitnessV1, PqMaspNotePlaintextV1, PqMaspOutputWitnessV1},
    vega::{VegaPrivacyActionPublicInputV1, VegaPrivacyActionWitnessMaterialV1},
    verange::VeRangeBitLengthV1,
    zk_ace::ZkAcePrivacyWitnessV1,
    zk_ams::{ZkAmsPrivacyActionGovernanceV1, ZkAmsSeedSecretV1},
};
use iroha_crypto::{Algorithm, PrivateKey, PublicKey};
use iroha_data_model::{
    asset::AssetDefinitionId,
    prelude::AccountId,
    privacy::{
        BootleLanternIssuerPolicyV1, PrivacyAuthorizationKeyDigestV1, PrivacyChallengeV1,
        PrivacyIssuerIdV1, PrivacyJindoFieldElementV1, PrivacyP256PointV1,
        PrivacyPgcAccountBootstrapDigestV1, PrivacyPgcAccountV1, PrivacyPgcBootstrapProofDigestV1,
        PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyPoolIdV1, PrivacyProtocolIdV1,
        PrivacyRecipientIdV1, PrivacyRootV1, PrivacySessionTranscriptDigestV1,
        PrivacyVegaIssuerRecordV1, PrivacyVegaMdlDateV1, PrivacyZkAcePolicyRecordV1,
        PrivacyZkAmsIssuerPolicyRecordDigestV1, PrivacyZkAmsPersonhoodCredentialV1,
        PrivacyZkAmsRegistryIdV1, PrivacyZkAmsRegistryRecordDigestV1, PrivacyZkAmsSeedPublicKeyV1,
    },
};
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize, Zeroizing};
use zk_ace_prover::ZkAcePrivacyTransferV1;
const MAGIC: &[u8; 4] = b"IPWB";
const SCHEMA_VERSION: u8 = 1;
const MAX_WALLET_ID_BYTES: usize = 512;
const MAX_AUTHORITY_BYTES: usize = 512;
const MAX_PROTOCOL_BYTES: usize = 96;
const MAX_OPERATION_SCHEMA_BYTES: usize = 128;
const MAX_PUBLIC_ACTION_BYTES: usize = 512 * 1024;
const PUBLIC_ACTION_DIGEST_DOMAIN: &[u8] = b"iroha-privacy-wallet-bundle-public-action-v1\0";
/// Public, non-secret identity derived from one owner-only execution bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivacyWalletExecutionBundleManifestV1 {
    /// Sole accepted outer bundle schema.
    pub schema_version: u8,
    /// Renderer wallet identifier bound by the opaque-handle import request.
    pub wallet_id: String,
    /// Exact single-key transaction authority.
    pub authority: AccountId,
    /// Public key derived from the bundle-owned transaction signer seed.
    pub public_key: PublicKey,
    /// Exact consensus protocol.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact operation schema selected by the native dispatcher.
    pub operation_schema: &'static str,
}
/// Public import inspection retained beside the zeroizing bundle bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectedPrivacyWalletExecutionBundleV1 {
    /// Public bundle manifest.
    pub manifest: PrivacyWalletExecutionBundleManifestV1,
    /// Domain-separated digest of the exact canonical public action.
    pub public_action_digest: [u8; 32],
}
/// Fully decoded single-use bundle.
///
/// This type intentionally implements neither `Clone` nor `Debug`.
pub(crate) struct DecodedPrivacyWalletExecutionBundleV1 {
    /// Public bundle manifest.
    pub(crate) manifest: PrivacyWalletExecutionBundleManifestV1,
    /// Exact all-native typed dispatcher request.
    pub(crate) request: PrivacyNativeActionRequestV1,
    /// Sole transaction signing key.  `PrivateKey` owns zeroizing material.
    pub(crate) signer_private_key: PrivateKey,
}
/// Stable, non-secret bundle rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivacyWalletBundleErrorV1 {
    stage: &'static str,
}
impl PrivacyWalletBundleErrorV1 {
    const fn at(stage: &'static str) -> Self {
        Self { stage }
    }
    /// Stable non-secret failure stage.
    #[must_use]
    pub const fn stage(self) -> &'static str {
        self.stage
    }
}
impl fmt::Display for PrivacyWalletBundleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "privacy wallet bundle rejected at {}",
            self.stage
        )
    }
}
impl std::error::Error for PrivacyWalletBundleErrorV1 {}
struct BundleParts<'a> {
    wallet_id: &'a str,
    authority: &'a str,
    protocol_id: PrivacyProtocolIdV1,
    public_action: &'a [u8],
    signer_seed: &'a [u8; 32],
    protocol_witness: &'a [u8],
}
struct Cursor<'a> {
    source: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(source: &'a [u8]) -> Self {
        Self { source, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], PrivacyWalletBundleErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| PrivacyWalletBundleErrorV1::at("length-overflow"))?;
        let value = self
            .source
            .get(self.offset..end)
            .ok_or_else(|| PrivacyWalletBundleErrorV1::at("truncated"))?;
        self.offset = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, PrivacyWalletBundleErrorV1> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| PrivacyWalletBundleErrorV1::at("u16"))?;
        Ok(u16::from_be_bytes(bytes))
    }
    fn u32(&mut self) -> Result<u32, PrivacyWalletBundleErrorV1> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| PrivacyWalletBundleErrorV1::at("u32"))?;
        Ok(u32::from_be_bytes(bytes))
    }
    fn text(
        &mut self,
        maximum: usize,
        stage: &'static str,
    ) -> Result<&'a str, PrivacyWalletBundleErrorV1> {
        let length = usize::from(self.u16()?);
        if length == 0 || length > maximum {
            return Err(PrivacyWalletBundleErrorV1::at(stage));
        }
        let value = core::str::from_utf8(self.take(length)?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?;
        validate_text(value, stage)?;
        Ok(value)
    }
    fn bytes_u32(
        &mut self,
        maximum: usize,
        stage: &'static str,
    ) -> Result<&'a [u8], PrivacyWalletBundleErrorV1> {
        let length =
            usize::try_from(self.u32()?).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?;
        if length == 0 || length > maximum {
            return Err(PrivacyWalletBundleErrorV1::at(stage));
        }
        self.take(length)
    }
    fn finish(self) -> Result<(), PrivacyWalletBundleErrorV1> {
        if self.offset == self.source.len() {
            Ok(())
        } else {
            Err(PrivacyWalletBundleErrorV1::at("trailing-bytes"))
        }
    }
}
fn validate_text(value: &str, stage: &'static str) -> Result<(), PrivacyWalletBundleErrorV1> {
    if value.trim() != value
        || value
            .chars()
            .any(|character| character.is_control() || character == '\0')
    {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    Ok(())
}
fn validate_wallet_id(value: &str) -> Result<(), PrivacyWalletBundleErrorV1> {
    if value.len() > MAX_WALLET_ID_BYTES
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric()
                || (index > 0
                    && matches!(byte, b'_' | b'-' | b'.' | b':' | b'+' | b'/' | b'@' | b'#'))
        })
        || !value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || value.contains("..")
    {
        return Err(PrivacyWalletBundleErrorV1::at("wallet-id"));
    }
    Ok(())
}
fn validate_canonical_json_object(
    bytes: &[u8],
    maximum: usize,
    stage: &'static str,
) -> Result<(), PrivacyWalletBundleErrorV1> {
    if bytes.is_empty() || bytes.len() > maximum || bytes.contains(&0) {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    let text = core::str::from_utf8(bytes).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?;
    let value =
        norito::json::parse_value(text).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?;
    if value.as_object().is_none_or(norito::json::Map::is_empty) {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    let canonical =
        norito::json::to_json(&value).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?;
    if canonical.as_bytes() != bytes {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    Ok(())
}
fn parse_parts(bytes: &[u8]) -> Result<BundleParts<'_>, PrivacyWalletBundleErrorV1> {
    if bytes.len() > PRIVACY_NATIVE_ACTION_MAX_SECRET_BUNDLE_BYTES_V1 {
        return Err(PrivacyWalletBundleErrorV1::at("bundle-size"));
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(PrivacyWalletBundleErrorV1::at("magic"));
    }
    if cursor.take(1)? != [SCHEMA_VERSION] {
        return Err(PrivacyWalletBundleErrorV1::at("schema-version"));
    }
    let wallet_id = cursor.text(MAX_WALLET_ID_BYTES, "wallet-id")?;
    validate_wallet_id(wallet_id)?;
    let authority = cursor.text(MAX_AUTHORITY_BYTES, "authority")?;
    let protocol_label = cursor.text(MAX_PROTOCOL_BYTES, "protocol-id")?;
    let protocol_id = PrivacyProtocolIdV1::from_canonical_label(protocol_label)
        .ok_or_else(|| PrivacyWalletBundleErrorV1::at("protocol-id"))?;
    let operation_schema = cursor.text(MAX_OPERATION_SCHEMA_BYTES, "operation-schema")?;
    let public_action = cursor.bytes_u32(MAX_PUBLIC_ACTION_BYTES, "public-action")?;
    validate_canonical_json_object(public_action, MAX_PUBLIC_ACTION_BYTES, "public-action")?;
    let signer_seed: &[u8; 32] = cursor
        .take(32)?
        .try_into()
        .map_err(|_| PrivacyWalletBundleErrorV1::at("signer-seed"))?;
    if signer_seed.iter().all(|byte| *byte == 0) {
        return Err(PrivacyWalletBundleErrorV1::at("signer-seed"));
    }
    let remaining = bytes.len().saturating_sub(cursor.offset);
    let protocol_witness = cursor.bytes_u32(remaining.saturating_sub(4), "protocol-witness")?;
    cursor.finish()?;
    let capability = privacy_native_action_capability_for_protocol_v1(protocol_id)
        .ok_or_else(|| PrivacyWalletBundleErrorV1::at("unsupported-protocol"))?;
    if capability.operation_schema != operation_schema {
        return Err(PrivacyWalletBundleErrorV1::at("operation-schema"));
    }
    Ok(BundleParts {
        wallet_id,
        authority,
        protocol_id,
        public_action,
        signer_seed,
        protocol_witness,
    })
}
fn derive_manifest(
    parts: &BundleParts<'_>,
) -> Result<(PrivacyWalletExecutionBundleManifestV1, PrivateKey), PrivacyWalletBundleErrorV1> {
    let authority = AccountId::parse_encoded(parts.authority)
        .map_err(|_| PrivacyWalletBundleErrorV1::at("authority"))?;
    let mut seed = Zeroizing::new([0_u8; 32]);
    seed.copy_from_slice(parts.signer_seed);
    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, seed.as_slice())
        .map_err(|_| PrivacyWalletBundleErrorV1::at("signer-seed"))?;
    let public_key = PublicKey::from(private_key.clone());
    if authority.try_signatory() != Some(&public_key) {
        return Err(PrivacyWalletBundleErrorV1::at("authority-key-mismatch"));
    }
    Ok((
        PrivacyWalletExecutionBundleManifestV1 {
            schema_version: SCHEMA_VERSION,
            wallet_id: parts.wallet_id.to_owned(),
            authority,
            public_key,
            protocol_id: parts.protocol_id,
            operation_schema: privacy_native_action_capability_for_protocol_v1(parts.protocol_id)
                .expect("validated capability")
                .operation_schema,
        },
        private_key,
    ))
}
/// Domain-separated digest of one exact canonical public-action object.
#[must_use]
pub fn privacy_wallet_bundle_public_action_digest_v1(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PUBLIC_ACTION_DIGEST_DOMAIN);
    digest.update(bytes);
    digest.finalize().into()
}
/// Inspect public identity without releasing any bundle byte.
pub fn inspect_privacy_wallet_execution_bundle_v1(
    bytes: &[u8],
) -> Result<InspectedPrivacyWalletExecutionBundleV1, PrivacyWalletBundleErrorV1> {
    let parts = parse_parts(bytes)?;
    let (manifest, private_key) = derive_manifest(&parts)?;
    drop(private_key);
    Ok(InspectedPrivacyWalletExecutionBundleV1 {
        manifest,
        public_action_digest: privacy_wallet_bundle_public_action_digest_v1(parts.public_action),
    })
}
/// Decode the exact typed request only inside a single-use vault callback.
pub(crate) fn decode_privacy_wallet_execution_bundle_v1(
    bytes: &mut [u8],
    expected_public_action: &[u8],
) -> Result<DecodedPrivacyWalletExecutionBundleV1, PrivacyWalletBundleErrorV1> {
    let parts = parse_parts(bytes)?;
    if parts.public_action != expected_public_action {
        return Err(PrivacyWalletBundleErrorV1::at("public-action-mismatch"));
    }
    let (manifest, signer_private_key) = derive_manifest(&parts)?;
    let request = decode_protocol_request_v1(
        parts.protocol_id,
        parts.public_action,
        parts.protocol_witness,
    )?;
    Ok(DecodedPrivacyWalletExecutionBundleV1 {
        manifest,
        request,
        signer_private_key,
    })
}
fn json_object(
    bytes: &[u8],
    maximum: usize,
    stage: &'static str,
) -> Result<norito::json::Map, PrivacyWalletBundleErrorV1> {
    validate_canonical_json_object(bytes, maximum, stage)?;
    let value = norito::json::parse_value(
        core::str::from_utf8(bytes).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?,
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?;
    let norito::json::Value::Object(object) = value else {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    };
    Ok(object)
}
fn exact_fields(
    object: &norito::json::Map,
    fields: &[&str],
    stage: &'static str,
) -> Result<(), PrivacyWalletBundleErrorV1> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    Ok(())
}
fn take_value(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<norito::json::Value, PrivacyWalletBundleErrorV1> {
    object
        .remove(field)
        .ok_or_else(|| PrivacyWalletBundleErrorV1::at(stage))
}
fn take_text(
    object: &mut norito::json::Map,
    field: &str,
    maximum: usize,
    stage: &'static str,
) -> Result<String, PrivacyWalletBundleErrorV1> {
    let norito::json::Value::String(value) = take_value(object, field, stage)? else {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    };
    if value.is_empty() || value.len() > maximum {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    validate_text(&value, stage)?;
    Ok(value)
}
fn take_u64(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<u64, PrivacyWalletBundleErrorV1> {
    take_value(object, field, stage)?
        .as_u64()
        .ok_or_else(|| PrivacyWalletBundleErrorV1::at(stage))
}
fn take_u32(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<u32, PrivacyWalletBundleErrorV1> {
    u32::try_from(take_u64(object, field, stage)?)
        .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn take_u8(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<u8, PrivacyWalletBundleErrorV1> {
    u8::try_from(take_u64(object, field, stage)?).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn decode_lower_hex<const N: usize>(
    mut value: String,
    stage: &'static str,
    allow_zero: bool,
) -> Result<[u8; N], PrivacyWalletBundleErrorV1> {
    let valid = value.len() == N * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let mut output = [0_u8; N];
    let decoded = valid && hex::decode_to_slice(&value, &mut output).is_ok();
    value.zeroize();
    if !decoded || (!allow_zero && output.iter().all(|byte| *byte == 0)) {
        output.zeroize();
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    Ok(output)
}
fn take_hex<const N: usize>(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
    allow_zero: bool,
) -> Result<[u8; N], PrivacyWalletBundleErrorV1> {
    decode_lower_hex(take_text(object, field, N * 2, stage)?, stage, allow_zero)
}
fn parse_decimal_u128(
    mut value: String,
    stage: &'static str,
    allow_zero: bool,
) -> Result<u128, PrivacyWalletBundleErrorV1> {
    let canonical = !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'));
    let parsed = if canonical {
        value.parse::<u128>().ok()
    } else {
        None
    };
    value.zeroize();
    match parsed {
        Some(number) if allow_zero || number != 0 => Ok(number),
        _ => Err(PrivacyWalletBundleErrorV1::at(stage)),
    }
}
fn parse_decimal_i64(
    mut value: String,
    stage: &'static str,
) -> Result<i64, PrivacyWalletBundleErrorV1> {
    let digits = value.strip_prefix('-').unwrap_or(&value);
    let canonical = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && value != "-0"
        && !value.starts_with('+');
    let parsed = if canonical {
        value.parse::<i64>().ok()
    } else {
        None
    };
    value.zeroize();
    parsed.ok_or_else(|| PrivacyWalletBundleErrorV1::at(stage))
}
fn decode_lower_hex_vec(
    mut value: String,
    minimum_bytes: usize,
    maximum_bytes: usize,
    stage: &'static str,
) -> Result<Zeroizing<Vec<u8>>, PrivacyWalletBundleErrorV1> {
    let valid_length = value.len().is_multiple_of(2)
        && (minimum_bytes.saturating_mul(2)..=maximum_bytes.saturating_mul(2))
            .contains(&value.len());
    let valid_alphabet = value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let decoded = if valid_length && valid_alphabet {
        hex::decode(&value).ok().map(Zeroizing::new)
    } else {
        None
    };
    value.zeroize();
    decoded.ok_or_else(|| PrivacyWalletBundleErrorV1::at(stage))
}
fn take_hex_vec(
    object: &mut norito::json::Map,
    field: &str,
    minimum_bytes: usize,
    maximum_bytes: usize,
    stage: &'static str,
) -> Result<Zeroizing<Vec<u8>>, PrivacyWalletBundleErrorV1> {
    decode_lower_hex_vec(
        take_text(object, field, maximum_bytes.saturating_mul(2), stage)?,
        minimum_bytes,
        maximum_bytes,
        stage,
    )
}
fn value_object(
    value: norito::json::Value,
    fields: &[&str],
    stage: &'static str,
) -> Result<norito::json::Map, PrivacyWalletBundleErrorV1> {
    let norito::json::Value::Object(object) = value else {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    };
    exact_fields(&object, fields, stage)?;
    Ok(object)
}
fn decode_fixed_hex_values<const N: usize>(
    values: Vec<norito::json::Value>,
    stage: &'static str,
    allow_zero: bool,
) -> Result<Vec<[u8; N]>, PrivacyWalletBundleErrorV1> {
    values
        .into_iter()
        .map(|value| {
            let norito::json::Value::String(value) = value else {
                return Err(PrivacyWalletBundleErrorV1::at(stage));
            };
            decode_lower_hex(value, stage, allow_zero)
        })
        .collect()
}
fn decode_path_v1(
    values: Vec<norito::json::Value>,
    stage: &'static str,
) -> Result<[[u8; 32]; 32], PrivacyWalletBundleErrorV1> {
    decode_fixed_hex_values(values, stage, false)?
        .try_into()
        .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn take_decimal_u128(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
    allow_zero: bool,
) -> Result<u128, PrivacyWalletBundleErrorV1> {
    parse_decimal_u128(take_text(object, field, 39, stage)?, stage, allow_zero)
}
fn take_array(
    object: &mut norito::json::Map,
    field: &str,
    minimum: usize,
    maximum: usize,
    stage: &'static str,
) -> Result<Vec<norito::json::Value>, PrivacyWalletBundleErrorV1> {
    let norito::json::Value::Array(values) = take_value(object, field, stage)? else {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    };
    if !(minimum..=maximum).contains(&values.len()) {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    Ok(values)
}
fn account_id(value: String, stage: &'static str) -> Result<AccountId, PrivacyWalletBundleErrorV1> {
    AccountId::parse_encoded(&value).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn asset_definition_id(
    value: String,
    stage: &'static str,
) -> Result<AssetDefinitionId, PrivacyWalletBundleErrorV1> {
    value
        .parse::<AssetDefinitionId>()
        .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
struct SecretJsonObject(norito::json::Map);
impl Drop for SecretJsonObject {
    fn drop(&mut self) {
        let value = norito::json::Value::Object(core::mem::take(&mut self.0));
        zeroize_json_value(value);
    }
}
fn zeroize_json_value(value: norito::json::Value) {
    match value {
        norito::json::Value::String(mut string) => string.zeroize(),
        norito::json::Value::Array(values) => {
            for value in values {
                zeroize_json_value(value);
            }
        }
        norito::json::Value::Object(values) => {
            for (mut key, value) in values {
                key.zeroize();
                zeroize_json_value(value);
            }
        }
        norito::json::Value::Null
        | norito::json::Value::Bool(_)
        | norito::json::Value::Number(_) => {}
    }
}
fn secret_object(bytes: &[u8]) -> Result<SecretJsonObject, PrivacyWalletBundleErrorV1> {
    json_object(
        bytes,
        PRIVACY_NATIVE_ACTION_MAX_SECRET_BUNDLE_BYTES_V1,
        "protocol-witness",
    )
    .map(SecretJsonObject)
}
fn decode_zk_ace_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "zk-ace-public-action",
    )?;
    exact_fields(
        &public,
        &[
            "amount_decimal",
            "destination",
            "policy",
            "public_balance_scope",
            "source",
        ],
        "zk-ace-public-action",
    )?;
    let policy = norito::json::from_value::<PrivacyZkAcePolicyRecordV1>(take_value(
        &mut public,
        "policy",
        "zk-ace-policy",
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("zk-ace-policy"))?;
    let source = account_id(
        take_text(&mut public, "source", MAX_AUTHORITY_BYTES, "zk-ace-source")?,
        "zk-ace-source",
    )?;
    let destination = account_id(
        take_text(
            &mut public,
            "destination",
            MAX_AUTHORITY_BYTES,
            "zk-ace-destination",
        )?,
        "zk-ace-destination",
    )?;
    let public_balance_scope = parse_canonical_public_balance_scope_v1(&take_text(
        &mut public,
        "public_balance_scope",
        30,
        "zk-ace-public-balance-scope",
    )?)
    .ok_or_else(|| PrivacyWalletBundleErrorV1::at("zk-ace-public-balance-scope"))?;
    let amount = take_decimal_u128(&mut public, "amount_decimal", "zk-ace-amount", false)?;
    let transfer =
        ZkAcePrivacyTransferV1::try_new(policy, source, destination, public_balance_scope, amount)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("zk-ace-transfer"))?;
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &[
            "identity_blinding_hex",
            "identity_root_hex",
            "replay_secret_hex",
        ],
        "zk-ace-witness",
    )?;
    let identity_root = take_hex(
        &mut secret.0,
        "identity_root_hex",
        "zk-ace-identity-root",
        false,
    )?;
    let identity_blinding = take_hex(
        &mut secret.0,
        "identity_blinding_hex",
        "zk-ace-identity-blinding",
        false,
    )?;
    let replay_secret = take_hex(
        &mut secret.0,
        "replay_secret_hex",
        "zk-ace-replay-secret",
        false,
    )?;
    let witness = ZkAcePrivacyWitnessV1::try_new(identity_root, identity_blinding, replay_secret)
        .map_err(|_| PrivacyWalletBundleErrorV1::at("zk-ace-witness"))?;
    Ok(PrivacyNativeActionRequestV1::ZkAce(
        ZkAceAuthorizationActionRequestV1 { transfer, witness },
    ))
}
fn decode_verange_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "verange-public-action",
    )?;
    exact_fields(
        &public,
        &["asset_definition_id", "bit_length", "policy_id_hex"],
        "verange-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(&mut public, "asset_definition_id", 512, "verange-asset")?,
        "verange-asset",
    )?;
    let policy_id = PrivacyPolicyIdV1::new(take_hex(
        &mut public,
        "policy_id_hex",
        "verange-policy",
        false,
    )?);
    let bit_length = match take_u64(&mut public, "bit_length", "verange-bit-length")? {
        32 => VeRangeBitLengthV1::Bits32,
        64 => VeRangeBitLengthV1::Bits64,
        _ => return Err(PrivacyWalletBundleErrorV1::at("verange-bit-length")),
    };
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &["blindings_hex", "values_decimal"],
        "verange-witness",
    )?;
    let value_strings = take_array(&mut secret.0, "values_decimal", 1, 8, "verange-values")?;
    let mut values = Vec::with_capacity(value_strings.len());
    for value in value_strings {
        let norito::json::Value::String(value) = value else {
            return Err(PrivacyWalletBundleErrorV1::at("verange-values"));
        };
        let parsed = parse_decimal_u128(value, "verange-values", true)?;
        let value =
            u64::try_from(parsed).map_err(|_| PrivacyWalletBundleErrorV1::at("verange-values"))?;
        if bit_length == VeRangeBitLengthV1::Bits32 && value >= 1_u64 << 32 {
            return Err(PrivacyWalletBundleErrorV1::at("verange-values"));
        }
        values.push(value);
    }
    let blinding_values = take_array(
        &mut secret.0,
        "blindings_hex",
        values.len(),
        values.len(),
        "verange-blindings",
    )?;
    let mut blindings = Vec::with_capacity(blinding_values.len());
    for value in blinding_values {
        let norito::json::Value::String(value) = value else {
            return Err(PrivacyWalletBundleErrorV1::at("verange-blindings"));
        };
        let bytes = decode_lower_hex(value, "verange-blindings", false)?;
        blindings.push(
            SecretScalarV1::from_bytes(bytes)
                .map_err(|_| PrivacyWalletBundleErrorV1::at("verange-blindings"))?,
        );
    }
    Ok(PrivacyNativeActionRequestV1::VeRange(
        VeRangeActionRequestV1 {
            asset_definition_id,
            policy_id,
            bit_length,
            values,
            blindings,
        },
    ))
}
fn decode_zk_ams_governance_v1(
    public: &mut norito::json::Map,
) -> Result<ZkAmsPrivacyActionGovernanceV1, PrivacyWalletBundleErrorV1> {
    Ok(ZkAmsPrivacyActionGovernanceV1 {
        issuer_id: PrivacyIssuerIdV1::new(take_hex(
            public,
            "issuer_id_hex",
            "zk-ams-issuer-id",
            false,
        )?),
        issuer_public_key: PrivacyP256PointV1::new(take_hex(
            public,
            "issuer_public_key_hex",
            "zk-ams-issuer-public-key",
            false,
        )?),
        issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1::new(take_hex(
            public,
            "issuer_policy_record_digest_hex",
            "zk-ams-issuer-policy-record",
            false,
        )?),
        registry_id: PrivacyZkAmsRegistryIdV1::new(take_hex(
            public,
            "registry_id_hex",
            "zk-ams-registry-id",
            false,
        )?),
        registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1::new(take_hex(
            public,
            "registry_record_digest_hex",
            "zk-ams-registry-record",
            false,
        )?),
        policy_id: PrivacyPolicyIdV1::new(take_hex(
            public,
            "policy_id_hex",
            "zk-ams-policy-id",
            false,
        )?),
        policy_digest: PrivacyPolicyDigestV1::new(take_hex(
            public,
            "policy_digest_hex",
            "zk-ams-policy-digest",
            false,
        )?),
    })
}
fn decode_zk_ams_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    const GOVERNANCE_FIELDS: &[&str] = &[
        "account_registry_root_epoch",
        "account_registry_root_hex",
        "action",
        "issuer_id_hex",
        "issuer_policy_record_digest_hex",
        "issuer_public_key_hex",
        "policy_digest_hex",
        "policy_id_hex",
        "registry_id_hex",
        "registry_record_digest_hex",
    ];
    const PROVISION_FIELDS: &[&str] = &[
        "account_id",
        "account_registry_root_epoch",
        "account_registry_root_hex",
        "action",
        "admitted_seed_key_ring_hex",
        "issuer_id_hex",
        "issuer_policy_record_digest_hex",
        "issuer_public_key_hex",
        "policy_digest_hex",
        "policy_id_hex",
        "registry_id_hex",
        "registry_record_digest_hex",
    ];
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "zk-ams-public-action",
    )?;
    let action = take_text(&mut public, "action", 32, "zk-ams-action")?;
    match action.as_str() {
        "batch_admission" => {
            if public.len() + 1 != GOVERNANCE_FIELDS.len()
                || GOVERNANCE_FIELDS
                    .iter()
                    .filter(|field| **field != "action")
                    .any(|field| !public.contains_key(*field))
            {
                return Err(PrivacyWalletBundleErrorV1::at("zk-ams-public-action"));
            }
            let governance = decode_zk_ams_governance_v1(&mut public)?;
            let account_registry_root = PrivacyRootV1::new(take_hex(
                &mut public,
                "account_registry_root_hex",
                "zk-ams-account-registry-root",
                false,
            )?);
            let account_registry_root_epoch = take_u64(
                &mut public,
                "account_registry_root_epoch",
                "zk-ams-account-registry-epoch",
            )?;
            let mut secret = secret_object(protocol_witness)?;
            exact_fields(&secret.0, &["credentials"], "zk-ams-witness")?;
            let credentials = take_array(&mut secret.0, "credentials", 1, 8, "zk-ams-credentials")?
                .into_iter()
                .map(|value| {
                    let mut credential_object = value_object(
                        value,
                        &["credential", "issuer_signature_hex", "seed_secret_hex"],
                        "zk-ams-credential",
                    )?;
                    let credential =
                        norito::json::from_value::<PrivacyZkAmsPersonhoodCredentialV1>(take_value(
                            &mut credential_object,
                            "credential",
                            "zk-ams-credential",
                        )?)
                        .map_err(|_| PrivacyWalletBundleErrorV1::at("zk-ams-credential"))?;
                    let issuer_signature: Zeroizing<[u8; 64]> = Zeroizing::new(take_hex(
                        &mut credential_object,
                        "issuer_signature_hex",
                        "zk-ams-issuer-signature",
                        false,
                    )?);
                    let seed_secret = ZkAmsSeedSecretV1::from_bytes(take_hex(
                        &mut credential_object,
                        "seed_secret_hex",
                        "zk-ams-seed-secret",
                        false,
                    )?)
                    .map_err(|_| PrivacyWalletBundleErrorV1::at("zk-ams-seed-secret"))?;
                    Ok(ZkAmsAdmissionCredentialRequestV1 {
                        credential,
                        issuer_signature,
                        seed_secret,
                    })
                })
                .collect::<Result<Vec<_>, PrivacyWalletBundleErrorV1>>()?;
            Ok(PrivacyNativeActionRequestV1::ZkAms(
                ZkAmsActionRequestV1::BatchAdmission(ZkAmsBatchAdmissionActionRequestV1 {
                    governance,
                    account_registry_root,
                    account_registry_root_epoch,
                    credentials,
                }),
            ))
        }
        "provision_account" => {
            if public.len() + 1 != PROVISION_FIELDS.len()
                || PROVISION_FIELDS
                    .iter()
                    .filter(|field| **field != "action")
                    .any(|field| !public.contains_key(*field))
            {
                return Err(PrivacyWalletBundleErrorV1::at("zk-ams-public-action"));
            }
            let governance = decode_zk_ams_governance_v1(&mut public)?;
            let account_registry_root = PrivacyRootV1::new(take_hex(
                &mut public,
                "account_registry_root_hex",
                "zk-ams-account-registry-root",
                false,
            )?);
            let account_registry_root_epoch = take_u64(
                &mut public,
                "account_registry_root_epoch",
                "zk-ams-account-registry-epoch",
            )?;
            let account_id = account_id(
                take_text(
                    &mut public,
                    "account_id",
                    MAX_AUTHORITY_BYTES,
                    "zk-ams-account-id",
                )?,
                "zk-ams-account-id",
            )?;
            let key_values = take_array(
                &mut public,
                "admitted_seed_key_ring_hex",
                2,
                64,
                "zk-ams-seed-key-ring",
            )?;
            let mut admitted_seed_key_ring = Vec::with_capacity(key_values.len());
            let mut previous = None;
            for bytes in decode_fixed_hex_values(key_values, "zk-ams-seed-key-ring", false)? {
                if previous.is_some_and(|prior| prior >= bytes) {
                    return Err(PrivacyWalletBundleErrorV1::at("zk-ams-seed-key-ring"));
                }
                previous = Some(bytes);
                admitted_seed_key_ring.push(PrivacyZkAmsSeedPublicKeyV1::new(bytes));
            }
            let mut secret = secret_object(protocol_witness)?;
            exact_fields(&secret.0, &["seed_secret_hex"], "zk-ams-witness")?;
            let seed_secret = ZkAmsSeedSecretV1::from_bytes(take_hex(
                &mut secret.0,
                "seed_secret_hex",
                "zk-ams-seed-secret",
                false,
            )?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("zk-ams-seed-secret"))?;
            Ok(PrivacyNativeActionRequestV1::ZkAms(
                ZkAmsActionRequestV1::ProvisionAccount(ZkAmsProvisionAccountActionRequestV1 {
                    governance,
                    account_registry_root,
                    account_registry_root_epoch,
                    admitted_seed_key_ring,
                    account_id,
                    seed_secret,
                }),
            ))
        }
        _ => Err(PrivacyWalletBundleErrorV1::at("zk-ams-action")),
    }
}
fn decode_vega_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(public_action, MAX_PUBLIC_ACTION_BYTES, "vega-public-action")?;
    exact_fields(
        &public,
        &[
            "issuer_record",
            "minimum_age_years",
            "presentation_date",
            "reader_challenge_hex",
            "session_transcript_digest_hex",
            "trusted_block_timestamp_ms",
        ],
        "vega-public-action",
    )?;
    let issuer_record = norito::json::from_value::<PrivacyVegaIssuerRecordV1>(take_value(
        &mut public,
        "issuer_record",
        "vega-issuer-record",
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("vega-issuer-record"))?;
    let presentation_date = norito::json::from_value::<PrivacyVegaMdlDateV1>(take_value(
        &mut public,
        "presentation_date",
        "vega-presentation-date",
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("vega-presentation-date"))?;
    let minimum_age_years = take_u8(&mut public, "minimum_age_years", "vega-minimum-age")?;
    if minimum_age_years == 0 {
        return Err(PrivacyWalletBundleErrorV1::at("vega-minimum-age"));
    }
    let input = VegaPrivacyActionPublicInputV1 {
        issuer_record,
        presentation_date,
        minimum_age_years,
        reader_challenge: PrivacyChallengeV1::new(take_hex(
            &mut public,
            "reader_challenge_hex",
            "vega-reader-challenge",
            false,
        )?),
        session_transcript_digest: PrivacySessionTranscriptDigestV1::new(take_hex(
            &mut public,
            "session_transcript_digest_hex",
            "vega-session-transcript",
            false,
        )?),
    };
    let trusted_block_timestamp_ms = take_u64(
        &mut public,
        "trusted_block_timestamp_ms",
        "vega-trusted-block-time",
    )?;
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &[
            "birth_date_issuer_signed_item_hex",
            "device_signing_key_hex",
            "issuer_authentication_sig_structure_hex",
            "issuer_signature_hex",
            "mobile_security_object_payload_hex",
        ],
        "vega-witness",
    )?;
    let issuer_authentication_sig_structure = take_hex_vec(
        &mut secret.0,
        "issuer_authentication_sig_structure_hex",
        1,
        65_536,
        "vega-issuer-authentication",
    )?;
    let mobile_security_object_payload = take_hex_vec(
        &mut secret.0,
        "mobile_security_object_payload_hex",
        1,
        65_536,
        "vega-mobile-security-object",
    )?;
    let birth_date_issuer_signed_item = take_hex_vec(
        &mut secret.0,
        "birth_date_issuer_signed_item_hex",
        1,
        65_536,
        "vega-birth-date-item",
    )?;
    let issuer_signature: Zeroizing<[u8; 64]> = Zeroizing::new(take_hex(
        &mut secret.0,
        "issuer_signature_hex",
        "vega-issuer-signature",
        false,
    )?);
    let witness_material = VegaPrivacyActionWitnessMaterialV1::new(
        Vec::from(issuer_authentication_sig_structure.as_slice()),
        Vec::from(mobile_security_object_payload.as_slice()),
        Vec::from(birth_date_issuer_signed_item.as_slice()),
        issuer_signature.as_slice(),
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at("vega-witness"))?;
    let mut device_key = Zeroizing::new(take_hex(
        &mut secret.0,
        "device_signing_key_hex",
        "vega-device-signing-key",
        false,
    )?);
    let device_signing_key = DeviceSigningKeyV1::from_bytes((&*device_key).into())
        .map_err(|_| PrivacyWalletBundleErrorV1::at("vega-device-signing-key"))?;
    device_key.zeroize();
    Ok(PrivacyNativeActionRequestV1::Vega(
        VegaCredentialPresentationActionRequestV1 {
            input,
            witness_material,
            device_signing_key,
            trusted_block_timestamp_ms,
        },
    ))
}
fn decode_application_polynomial_v1(
    value: norito::json::Value,
    stage: &'static str,
) -> Result<ApplicationPolynomialV1, PrivacyWalletBundleErrorV1> {
    let norito::json::Value::Array(values) = value else {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    };
    if values.len() != 64 {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    let coefficients: Vec<u16> = values
        .into_iter()
        .map(|value| {
            let coefficient = value
                .as_u64()
                .ok_or_else(|| PrivacyWalletBundleErrorV1::at(stage))?;
            u16::try_from(coefficient).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
        })
        .collect::<Result<_, _>>()?;
    ApplicationPolynomialV1::new(
        coefficients
            .try_into()
            .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))?,
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn take_polynomial_array<const N: usize>(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<[ApplicationPolynomialV1; N], PrivacyWalletBundleErrorV1> {
    take_array(object, field, N, N, stage)?
        .into_iter()
        .map(|value| decode_application_polynomial_v1(value, stage))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn decode_bootle_lantern_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "bootle-public-action",
    )?;
    exact_fields(
        &public,
        &["disclosure_indices", "policy"],
        "bootle-public-action",
    )?;
    let policy = norito::json::from_value::<BootleLanternIssuerPolicyV1>(take_value(
        &mut public,
        "policy",
        "bootle-policy",
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("bootle-policy"))?;
    let disclosure_values = take_array(
        &mut public,
        "disclosure_indices",
        1,
        8,
        "bootle-disclosure-indices",
    )?;
    let mut disclosure_indices = Vec::with_capacity(disclosure_values.len());
    for value in disclosure_values {
        let index = u8::try_from(
            value
                .as_u64()
                .ok_or_else(|| PrivacyWalletBundleErrorV1::at("bootle-disclosure-indices"))?,
        )
        .map_err(|_| PrivacyWalletBundleErrorV1::at("bootle-disclosure-indices"))?;
        if index >= 8
            || disclosure_indices
                .last()
                .is_some_and(|previous| *previous >= index)
        {
            return Err(PrivacyWalletBundleErrorV1::at("bootle-disclosure-indices"));
        }
        disclosure_indices.push(index);
    }
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &[
            "attributes_hex",
            "randomness",
            "signature_one",
            "signature_two",
            "tag",
        ],
        "bootle-witness",
    )?;
    let attributes: [[u8; 8]; 8] = decode_fixed_hex_values(
        take_array(&mut secret.0, "attributes_hex", 8, 8, "bootle-attributes")?,
        "bootle-attributes",
        true,
    )?
    .try_into()
    .map_err(|_| PrivacyWalletBundleErrorV1::at("bootle-attributes"))?;
    let witness = BootleLanternPresentationWitnessV1 {
        randomness: take_polynomial_array(&mut secret.0, "randomness", "bootle-randomness")?,
        tag: take_polynomial_array(&mut secret.0, "tag", "bootle-tag")?,
        signature_one: take_polynomial_array(
            &mut secret.0,
            "signature_one",
            "bootle-signature-one",
        )?,
        signature_two: take_polynomial_array(
            &mut secret.0,
            "signature_two",
            "bootle-signature-two",
        )?,
        attributes,
    };
    Ok(PrivacyNativeActionRequestV1::BootleLantern(
        BootleLanternPresentationActionRequestV1 {
            policy,
            disclosure_indices,
            witness,
        },
    ))
}
fn decode_anonymous_pgc_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "anonymous-pgc-public-action",
    )?;
    exact_fields(
        &public,
        &[
            "asset_definition_id",
            "bootstrap_digest_hex",
            "bootstrap_proof_digest_hex",
            "current_accounts",
            "current_epoch",
            "pool_id_hex",
            "total_supply",
        ],
        "anonymous-pgc-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(
            &mut public,
            "asset_definition_id",
            512,
            "anonymous-pgc-asset",
        )?,
        "anonymous-pgc-asset",
    )?;
    let pool_id = PrivacyPoolIdV1::new(take_hex(
        &mut public,
        "pool_id_hex",
        "anonymous-pgc-pool",
        false,
    )?);
    let current_epoch = take_u64(&mut public, "current_epoch", "anonymous-pgc-epoch")?;
    let total_supply = take_u32(&mut public, "total_supply", "anonymous-pgc-supply")?;
    if total_supply == 0 {
        return Err(PrivacyWalletBundleErrorV1::at("anonymous-pgc-supply"));
    }
    let bootstrap_digest = PrivacyPgcAccountBootstrapDigestV1::new(take_hex(
        &mut public,
        "bootstrap_digest_hex",
        "anonymous-pgc-bootstrap",
        false,
    )?);
    let bootstrap_proof_digest = PrivacyPgcBootstrapProofDigestV1::new(take_hex(
        &mut public,
        "bootstrap_proof_digest_hex",
        "anonymous-pgc-bootstrap-proof",
        false,
    )?);
    let current_accounts = norito::json::from_value::<Vec<PrivacyPgcAccountV1>>(take_value(
        &mut public,
        "current_accounts",
        "anonymous-pgc-accounts",
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("anonymous-pgc-accounts"))?;
    if current_accounts.is_empty() || current_accounts.len() > 64 {
        return Err(PrivacyWalletBundleErrorV1::at("anonymous-pgc-accounts"));
    }
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &[
            "sender_index",
            "sender_secret_hex",
            "transfer_randomness_hex",
            "transfer_values_decimal",
        ],
        "anonymous-pgc-witness",
    )?;
    let transfer_values_json = take_array(
        &mut secret.0,
        "transfer_values_decimal",
        current_accounts.len(),
        current_accounts.len(),
        "anonymous-pgc-transfer-values",
    )?;
    let transfer_values = transfer_values_json
        .into_iter()
        .map(|value| {
            let norito::json::Value::String(value) = value else {
                return Err(PrivacyWalletBundleErrorV1::at(
                    "anonymous-pgc-transfer-values",
                ));
            };
            parse_decimal_i64(value, "anonymous-pgc-transfer-values")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let transfer_randomness = decode_fixed_hex_values(
        take_array(
            &mut secret.0,
            "transfer_randomness_hex",
            current_accounts.len(),
            current_accounts.len(),
            "anonymous-pgc-transfer-randomness",
        )?,
        "anonymous-pgc-transfer-randomness",
        false,
    )?
    .into_iter()
    .map(|bytes| {
        SecretScalarV1::from_bytes(bytes)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("anonymous-pgc-transfer-randomness"))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let sender_index = usize::try_from(take_u64(
        &mut secret.0,
        "sender_index",
        "anonymous-pgc-sender-index",
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("anonymous-pgc-sender-index"))?;
    if sender_index >= current_accounts.len() {
        return Err(PrivacyWalletBundleErrorV1::at("anonymous-pgc-sender-index"));
    }
    let sender_secret = SecretScalarV1::from_bytes(take_hex(
        &mut secret.0,
        "sender_secret_hex",
        "anonymous-pgc-sender-secret",
        false,
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("anonymous-pgc-sender-secret"))?;
    Ok(PrivacyNativeActionRequestV1::AnonymousPgc(
        AnonymousPgcPaymentActionRequestV1 {
            asset_definition_id,
            pool_id,
            current_epoch,
            total_supply,
            bootstrap_digest,
            bootstrap_proof_digest,
            current_accounts,
            transfer_values,
            transfer_randomness,
            sender_index,
            sender_secret,
        },
    ))
}
fn decode_orchard_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "orchard-public-action",
    )?;
    exact_fields(
        &public,
        &[
            "anchor_epoch",
            "anchor_hex",
            "asset_definition_id",
            "expiry_height",
            "minimum_action_count",
            "pool_id_hex",
            "public_balance_scope",
        ],
        "orchard-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(&mut public, "asset_definition_id", 512, "orchard-asset")?,
        "orchard-asset",
    )?;
    let public_balance_scope = parse_canonical_public_balance_scope_v1(&take_text(
        &mut public,
        "public_balance_scope",
        30,
        "orchard-public-balance-scope",
    )?)
    .ok_or_else(|| PrivacyWalletBundleErrorV1::at("orchard-public-balance-scope"))?;
    let pool_id =
        PrivacyPoolIdV1::new(take_hex(&mut public, "pool_id_hex", "orchard-pool", false)?);
    let anchor_bytes = take_hex(&mut public, "anchor_hex", "orchard-anchor", false)?;
    let anchor = PrivacyRootV1::new(anchor_bytes);
    let anchor_epoch = take_u64(&mut public, "anchor_epoch", "orchard-anchor-epoch")?;
    let expiry_height = take_u64(&mut public, "expiry_height", "orchard-expiry-height")?;
    let minimum_action_count = take_u8(
        &mut public,
        "minimum_action_count",
        "orchard-minimum-action-count",
    )?;
    if !(1..=2).contains(&minimum_action_count) {
        return Err(PrivacyWalletBundleErrorV1::at(
            "orchard-minimum-action-count",
        ));
    }
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(&secret.0, &["changes", "spends"], "orchard-witness")?;
    let spends = take_array(&mut secret.0, "spends", 1, 2, "orchard-spends")?
        .into_iter()
        .map(|value| {
            let mut spend = value_object(
                value,
                &[
                    "authentication_path_hex",
                    "leaf_position",
                    "random_seed_hex",
                    "recipient_hex",
                    "rho_hex",
                    "spending_key_hex",
                    "value_decimal",
                ],
                "orchard-spend",
            )?;
            let spending_key = take_hex(
                &mut spend,
                "spending_key_hex",
                "orchard-spending-key",
                false,
            )?;
            let recipient = take_hex(&mut spend, "recipient_hex", "orchard-recipient", false)?;
            let value = u64::try_from(take_decimal_u128(
                &mut spend,
                "value_decimal",
                "orchard-value",
                false,
            )?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("orchard-value"))?;
            let rho = take_hex(&mut spend, "rho_hex", "orchard-rho", false)?;
            let random_seed =
                take_hex(&mut spend, "random_seed_hex", "orchard-random-seed", false)?;
            let leaf_position = take_u32(&mut spend, "leaf_position", "orchard-leaf-position")?;
            let path = decode_path_v1(
                take_array(
                    &mut spend,
                    "authentication_path_hex",
                    32,
                    32,
                    "orchard-authentication-path",
                )?,
                "orchard-authentication-path",
            )?;
            OrchardSpendProverInputV1::from_wallet_parts_v1(
                spending_key,
                recipient,
                value,
                rho,
                random_seed,
                leaf_position,
                path,
                anchor_bytes,
            )
            .map_err(|_| PrivacyWalletBundleErrorV1::at("orchard-spend"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let changes = take_array(&mut secret.0, "changes", 0, 2, "orchard-changes")?
        .into_iter()
        .map(|value| {
            let mut change = value_object(
                value,
                &[
                    "diversifier_index",
                    "memo_hex",
                    "scope",
                    "spending_key_hex",
                    "value_decimal",
                ],
                "orchard-change",
            )?;
            let spending_key = take_hex(
                &mut change,
                "spending_key_hex",
                "orchard-change-spending-key",
                false,
            )?;
            let internal_scope =
                match take_text(&mut change, "scope", 16, "orchard-scope")?.as_str() {
                    "external" => false,
                    "internal" => true,
                    _ => {
                        return Err(PrivacyWalletBundleErrorV1::at("orchard-scope"));
                    }
                };
            let diversifier_index = take_u32(
                &mut change,
                "diversifier_index",
                "orchard-diversifier-index",
            )?;
            let value = u64::try_from(take_decimal_u128(
                &mut change,
                "value_decimal",
                "orchard-change-value",
                false,
            )?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("orchard-change-value"))?;
            let memo = take_hex(&mut change, "memo_hex", "orchard-memo", true)?;
            OrchardChangeProverInputV1::from_wallet_parts_v1(
                spending_key,
                internal_scope,
                diversifier_index,
                value,
                memo,
            )
            .map_err(|_| PrivacyWalletBundleErrorV1::at("orchard-change"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PrivacyNativeActionRequestV1::Orchard(
        OrchardNoteActionRequestV1 {
            asset_definition_id,
            public_balance_scope,
            pool_id,
            anchor,
            anchor_epoch,
            expiry_height,
            spends,
            changes,
            minimum_action_count,
        },
    ))
}
fn decode_fcmp_tuple_v1(
    value: String,
    stage: &'static str,
) -> Result<FcmpOutputTupleV1, PrivacyWalletBundleErrorV1> {
    let bytes = decode_lower_hex::<96>(value, stage, false)?;
    FcmpOutputTupleV1::decode(&bytes).map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn decode_fcmp_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(public_action, MAX_PUBLIC_ACTION_BYTES, "fcmp-public-action")?;
    exact_fields(
        &public,
        &[
            "asset_definition_id",
            "output_set_root",
            "pool_id_hex",
            "root_epoch",
        ],
        "fcmp-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(&mut public, "asset_definition_id", 512, "fcmp-asset")?,
        "fcmp-asset",
    )?;
    let pool_id = PrivacyPoolIdV1::new(take_hex(&mut public, "pool_id_hex", "fcmp-pool", false)?);
    let mut root = value_object(
        take_value(&mut public, "output_set_root", "fcmp-root")?,
        &["layers", "point_hex"],
        "fcmp-root",
    )?;
    let output_set_root = FcmpTreeRootV1::new(
        take_u8(&mut root, "layers", "fcmp-root-layers")?,
        take_hex(&mut root, "point_hex", "fcmp-root-point", false)?,
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at("fcmp-root"))?;
    let root_epoch = take_u64(&mut public, "root_epoch", "fcmp-root-epoch")?;
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(&secret.0, &["inputs", "outputs"], "fcmp-witness")?;
    let inputs = take_array(&mut secret.0, "inputs", 1, 2, "fcmp-inputs")?
        .into_iter()
        .map(|value| {
            let mut input = value_object(
                value,
                &[
                    "additional_branches_hex",
                    "leaves_hex",
                    "output_tuple_hex",
                    "output_y_hex",
                    "rerandomization",
                    "spend_x_hex",
                ],
                "fcmp-input",
            )?;
            let output = decode_fcmp_tuple_v1(
                take_text(&mut input, "output_tuple_hex", 192, "fcmp-input-output")?,
                "fcmp-input-output",
            )?;
            let spend_x = take_hex(&mut input, "spend_x_hex", "fcmp-spend-x", false)?;
            let output_y = take_hex(&mut input, "output_y_hex", "fcmp-output-y", true)?;
            let mut rerandomization = value_object(
                take_value(&mut input, "rerandomization", "fcmp-rerandomization")?,
                &[
                    "commitment_hex",
                    "linking_hex",
                    "output_hex",
                    "rerandomization_blind_hex",
                ],
                "fcmp-rerandomization",
            )?;
            let rerandomization = FcmpInputRerandomizationV1::new(
                take_hex(
                    &mut rerandomization,
                    "output_hex",
                    "fcmp-rerandomization-output",
                    false,
                )?,
                take_hex(
                    &mut rerandomization,
                    "linking_hex",
                    "fcmp-rerandomization-linking",
                    false,
                )?,
                take_hex(
                    &mut rerandomization,
                    "rerandomization_blind_hex",
                    "fcmp-rerandomization-blind",
                    false,
                )?,
                take_hex(
                    &mut rerandomization,
                    "commitment_hex",
                    "fcmp-rerandomization-commitment",
                    false,
                )?,
            )
            .map_err(|_| PrivacyWalletBundleErrorV1::at("fcmp-rerandomization"))?;
            let leaves = take_array(&mut input, "leaves_hex", 1, 16, "fcmp-leaves")?
                .into_iter()
                .map(|value| {
                    let norito::json::Value::String(value) = value else {
                        return Err(PrivacyWalletBundleErrorV1::at("fcmp-leaves"));
                    };
                    decode_fcmp_tuple_v1(value, "fcmp-leaves")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let additional_branches = take_array(
                &mut input,
                "additional_branches_hex",
                0,
                15,
                "fcmp-additional-branches",
            )?
            .into_iter()
            .map(|branch| {
                let norito::json::Value::Array(values) = branch else {
                    return Err(PrivacyWalletBundleErrorV1::at("fcmp-additional-branches"));
                };
                if values.is_empty() || values.len() > 38 {
                    return Err(PrivacyWalletBundleErrorV1::at("fcmp-additional-branches"));
                }
                decode_fixed_hex_values(values, "fcmp-additional-branches", true)
            })
            .collect::<Result<Vec<_>, _>>()?;
            FcmpProverInputV1::new(
                output,
                spend_x,
                output_y,
                rerandomization,
                leaves,
                additional_branches,
            )
            .map_err(|_| PrivacyWalletBundleErrorV1::at("fcmp-input"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = take_array(&mut secret.0, "outputs", 1, 4, "fcmp-outputs")?
        .into_iter()
        .map(|value| {
            let mut output = value_object(
                value,
                &[
                    "amount_decimal",
                    "commitment_mask_hex",
                    "output_tuple_hex",
                    "output_y_hex",
                    "recipient_public_key_hex",
                    "spend_x_hex",
                ],
                "fcmp-output",
            )?;
            let tuple = decode_fcmp_tuple_v1(
                take_text(&mut output, "output_tuple_hex", 192, "fcmp-output-tuple")?,
                "fcmp-output-tuple",
            )?;
            let spend_x = take_hex(&mut output, "spend_x_hex", "fcmp-output-spend-x", false)?;
            let output_y = take_hex(&mut output, "output_y_hex", "fcmp-output-output-y", true)?;
            let amount = u64::try_from(take_decimal_u128(
                &mut output,
                "amount_decimal",
                "fcmp-output-amount",
                false,
            )?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("fcmp-output-amount"))?;
            let commitment_mask = take_hex(
                &mut output,
                "commitment_mask_hex",
                "fcmp-output-mask",
                false,
            )?;
            let note = FcmpWalletNoteV1::new(tuple, spend_x, output_y, amount, commitment_mask)
                .map_err(|_| PrivacyWalletBundleErrorV1::at("fcmp-output-note"))?;
            let recipient_public_key = take_hex(
                &mut output,
                "recipient_public_key_hex",
                "fcmp-output-recipient",
                false,
            )?;
            Ok(FcmpWalletOutputRequestV1 {
                note,
                recipient_public_key,
            })
        })
        .collect::<Result<Vec<_>, PrivacyWalletBundleErrorV1>>()?;
    Ok(PrivacyNativeActionRequestV1::FcmpPlusPlus(
        FcmpMembershipPaymentActionRequestV1 {
            asset_definition_id,
            pool_id,
            output_set_root,
            root_epoch,
            inputs,
            outputs,
        },
    ))
}
fn decode_ivm_note_v1(
    value: norito::json::Value,
    stage: &'static str,
) -> Result<PrivateNotePlaintextV1, PrivacyWalletBundleErrorV1> {
    let mut note = value_object(
        value,
        &[
            "blinding_hex",
            "memo_digest_hex",
            "rho_hex",
            "spending_authority_hex",
            "value_decimal",
        ],
        stage,
    )?;
    PrivateNotePlaintextV1::new(
        take_decimal_u128(&mut note, "value_decimal", stage, false)?,
        take_hex(&mut note, "spending_authority_hex", stage, false)?,
        take_hex(&mut note, "rho_hex", stage, false)?,
        take_hex(&mut note, "blinding_hex", stage, false)?,
        take_hex(&mut note, "memo_digest_hex", stage, true)?,
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn decode_ivm_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(public_action, MAX_PUBLIC_ACTION_BYTES, "ivm-public-action")?;
    exact_fields(
        &public,
        &[
            "asset_definition_id",
            "pool_id_hex",
            "public_balance_scope",
            "root_epoch",
            "state_root_hex",
        ],
        "ivm-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(&mut public, "asset_definition_id", 512, "ivm-asset")?,
        "ivm-asset",
    )?;
    let public_balance_scope = parse_canonical_public_balance_scope_v1(&take_text(
        &mut public,
        "public_balance_scope",
        30,
        "ivm-public-balance-scope",
    )?)
    .ok_or_else(|| PrivacyWalletBundleErrorV1::at("ivm-public-balance-scope"))?;
    let pool_id = PrivacyPoolIdV1::new(take_hex(&mut public, "pool_id_hex", "ivm-pool", false)?);
    let state_root = PrivacyRootV1::new(take_hex(
        &mut public,
        "state_root_hex",
        "ivm-state-root",
        false,
    )?);
    let root_epoch = take_u64(&mut public, "root_epoch", "ivm-root-epoch")?;
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &["inputs", "outputs", "program_instructions_hex"],
        "ivm-witness",
    )?;
    let instructions = take_array(
        &mut secret.0,
        "program_instructions_hex",
        16,
        16,
        "ivm-program",
    )?
    .into_iter()
    .map(|value| {
        let norito::json::Value::String(value) = value else {
            return Err(PrivacyWalletBundleErrorV1::at("ivm-program"));
        };
        PrivateInstructionV1::from_bytes(decode_lower_hex(value, "ivm-program", true)?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("ivm-program"))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let program = PrivateProgramV1::new(
        instructions
            .try_into()
            .map_err(|_| PrivacyWalletBundleErrorV1::at("ivm-program"))?,
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at("ivm-program"))?;
    let inputs = take_array(&mut secret.0, "inputs", 1, 2, "ivm-inputs")?
        .into_iter()
        .map(|value| {
            let mut input = value_object(
                value,
                &[
                    "authentication_path_hex",
                    "leaf_position",
                    "note",
                    "spending_secret_hex",
                ],
                "ivm-input",
            )?;
            let note = decode_ivm_note_v1(
                take_value(&mut input, "note", "ivm-input-note")?,
                "ivm-input-note",
            )?;
            let spending_secret = take_hex(
                &mut input,
                "spending_secret_hex",
                "ivm-spending-secret",
                false,
            )?;
            let leaf_position = take_u32(&mut input, "leaf_position", "ivm-leaf-position")?;
            let authentication_path = decode_path_v1(
                take_array(
                    &mut input,
                    "authentication_path_hex",
                    32,
                    32,
                    "ivm-authentication-path",
                )?,
                "ivm-authentication-path",
            )?;
            IvmPrivateNoteInputWitnessV1::new(
                note,
                spending_secret,
                leaf_position,
                authentication_path,
            )
            .map_err(|_| PrivacyWalletBundleErrorV1::at("ivm-input"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = take_array(&mut secret.0, "outputs", 1, 2, "ivm-outputs")?
        .into_iter()
        .map(|value| {
            let mut output =
                value_object(value, &["note", "recipient_public_key_hex"], "ivm-output")?;
            let note = decode_ivm_note_v1(
                take_value(&mut output, "note", "ivm-output-note")?,
                "ivm-output-note",
            )?;
            let witness = IvmPrivateNoteOutputWitnessV1::new(note)
                .map_err(|_| PrivacyWalletBundleErrorV1::at("ivm-output"))?;
            let recipient_public_key = take_hex(
                &mut output,
                "recipient_public_key_hex",
                "ivm-output-recipient",
                false,
            )?;
            Ok(IvmPrivateNoteOutputRequestV1 {
                witness,
                recipient_public_key,
            })
        })
        .collect::<Result<Vec<_>, PrivacyWalletBundleErrorV1>>()?;
    Ok(PrivacyNativeActionRequestV1::IvmPrivateNote(
        IvmPrivateNoteActionRequestV1 {
            asset_definition_id,
            public_balance_scope,
            pool_id,
            state_root,
            root_epoch,
            program,
            inputs,
            outputs,
        },
    ))
}
fn decode_pq_masp_note_v1(
    value: norito::json::Value,
    stage: &'static str,
) -> Result<PqMaspNotePlaintextV1, PrivacyWalletBundleErrorV1> {
    let mut note = value_object(
        value,
        &[
            "authorization_key_digest_hex",
            "blinding_hex",
            "memo_digest_hex",
            "nullifier_key_digest_hex",
            "recipient_key_digest_hex",
            "rho_hex",
            "value_decimal",
        ],
        stage,
    )?;
    PqMaspNotePlaintextV1::new(
        take_decimal_u128(&mut note, "value_decimal", stage, false)?,
        PrivacyAuthorizationKeyDigestV1::new(take_hex(
            &mut note,
            "authorization_key_digest_hex",
            stage,
            false,
        )?),
        PrivacyRecipientIdV1::new(take_hex(
            &mut note,
            "recipient_key_digest_hex",
            stage,
            false,
        )?),
        take_hex(&mut note, "nullifier_key_digest_hex", stage, false)?,
        take_hex(&mut note, "rho_hex", stage, false)?,
        take_hex(&mut note, "blinding_hex", stage, false)?,
        take_hex(&mut note, "memo_digest_hex", stage, true)?,
    )
    .map_err(|_| PrivacyWalletBundleErrorV1::at(stage))
}
fn decode_pq_masp_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "pq-masp-public-action",
    )?;
    exact_fields(
        &public,
        &[
            "anchor_epoch",
            "anchor_hex",
            "asset_definition_id",
            "pool_id_hex",
        ],
        "pq-masp-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(&mut public, "asset_definition_id", 512, "pq-masp-asset")?,
        "pq-masp-asset",
    )?;
    let pool_id =
        PrivacyPoolIdV1::new(take_hex(&mut public, "pool_id_hex", "pq-masp-pool", false)?);
    let anchor = PrivacyRootV1::new(take_hex(
        &mut public,
        "anchor_hex",
        "pq-masp-anchor",
        false,
    )?);
    let anchor_epoch = take_u64(&mut public, "anchor_epoch", "pq-masp-anchor-epoch")?;
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(
        &secret.0,
        &["authorization_secret_key_hex", "inputs", "outputs"],
        "pq-masp-witness",
    )?;
    let inputs = take_array(&mut secret.0, "inputs", 1, 2, "pq-masp-inputs")?
        .into_iter()
        .map(|value| {
            let mut input = value_object(
                value,
                &[
                    "authentication_path_hex",
                    "leaf_position",
                    "note",
                    "nullifier_secret_hex",
                ],
                "pq-masp-input",
            )?;
            let note = decode_pq_masp_note_v1(
                take_value(&mut input, "note", "pq-masp-input-note")?,
                "pq-masp-input-note",
            )?;
            let nullifier_secret = take_hex(
                &mut input,
                "nullifier_secret_hex",
                "pq-masp-nullifier-secret",
                false,
            )?;
            let leaf_position = take_u32(&mut input, "leaf_position", "pq-masp-leaf-position")?;
            let authentication_path = decode_path_v1(
                take_array(
                    &mut input,
                    "authentication_path_hex",
                    32,
                    32,
                    "pq-masp-authentication-path",
                )?,
                "pq-masp-authentication-path",
            )?;
            PqMaspInputWitnessV1::new(note, nullifier_secret, leaf_position, authentication_path)
                .map_err(|_| PrivacyWalletBundleErrorV1::at("pq-masp-input"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = take_array(&mut secret.0, "outputs", 1, 2, "pq-masp-outputs")?
        .into_iter()
        .map(|value| {
            let mut output = value_object(
                value,
                &["note", "recipient_public_key_hex"],
                "pq-masp-output",
            )?;
            let note = decode_pq_masp_note_v1(
                take_value(&mut output, "note", "pq-masp-output-note")?,
                "pq-masp-output-note",
            )?;
            let witness = PqMaspOutputWitnessV1::new(note)
                .map_err(|_| PrivacyWalletBundleErrorV1::at("pq-masp-output"))?;
            let recipient_public_key = take_hex_vec(
                &mut output,
                "recipient_public_key_hex",
                1_184,
                1_184,
                "pq-masp-output-recipient",
            )?;
            Ok(PqMaspOutputRequestV1 {
                witness,
                recipient_public_key: Vec::from(recipient_public_key.as_slice()),
            })
        })
        .collect::<Result<Vec<_>, PrivacyWalletBundleErrorV1>>()?;
    let authorization_secret_key = take_hex_vec(
        &mut secret.0,
        "authorization_secret_key_hex",
        4_032,
        4_032,
        "pq-masp-authorization-secret",
    )?;
    Ok(PrivacyNativeActionRequestV1::PqMasp(
        PqMaspNoteActionRequestV1 {
            asset_definition_id,
            pool_id,
            anchor,
            anchor_epoch,
            inputs,
            outputs,
            authorization_secret_key,
        },
    ))
}
fn decode_jindo_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "jindo-public-action",
    )?;
    exact_fields(&public, &["evaluation_point_hex"], "jindo-public-action")?;
    let evaluation_point = PrivacyJindoFieldElementV1::new(take_hex(
        &mut public,
        "evaluation_point_hex",
        "jindo-evaluation-point",
        true,
    )?);
    let mut secret = secret_object(protocol_witness)?;
    exact_fields(&secret.0, &["polynomials_hex"], "jindo-witness")?;
    let polynomial_values =
        take_array(&mut secret.0, "polynomials_hex", 1, 8, "jindo-polynomials")?;
    let mut polynomials = Vec::with_capacity(polynomial_values.len());
    for polynomial in polynomial_values {
        let norito::json::Value::Array(coefficients) = polynomial else {
            return Err(PrivacyWalletBundleErrorV1::at("jindo-polynomials"));
        };
        if coefficients.is_empty() || coefficients.len() > 64 {
            return Err(PrivacyWalletBundleErrorV1::at("jindo-polynomials"));
        }
        let mut decoded = Vec::with_capacity(coefficients.len());
        for coefficient in coefficients {
            let norito::json::Value::String(coefficient) = coefficient else {
                return Err(PrivacyWalletBundleErrorV1::at("jindo-coefficient"));
            };
            decoded.push(PrivacyJindoFieldElementV1::new(decode_lower_hex(
                coefficient,
                "jindo-coefficient",
                true,
            )?));
        }
        polynomials.push(decoded);
    }
    let witness = JindoPrivacyActionWitnessV1::try_new(polynomials, evaluation_point)
        .map_err(|_| PrivacyWalletBundleErrorV1::at("jindo-witness"))?;
    Ok(PrivacyNativeActionRequestV1::Jindo(
        JindoPolynomialEvaluationActionRequestV1 { witness },
    ))
}
fn decode_protocol_request_v1(
    protocol_id: PrivacyProtocolIdV1,
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    match protocol_id {
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV1 => {
            decode_zk_ace_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
            decode_anonymous_pgc_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
            decode_verange_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::IrohaZkAmsV1 => {
            decode_zk_ams_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::VegaExistingCredentialZkV1 => {
            decode_vega_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1 => {
            decode_jindo_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
            decode_bootle_lantern_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
            decode_orchard_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
            decode_fcmp_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
            decode_ivm_request_v1(public_action, protocol_witness)
        }
        PrivacyProtocolIdV1::PqMaspStarkV1 => {
            decode_pq_masp_request_v1(public_action, protocol_witness)
        }
        _ => Err(PrivacyWalletBundleErrorV1::at("unsupported-protocol")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const WALLET_ID: &str = "wallet-retail-adult-001";
    const PROTOCOL: &str = "iroha-jindo-polynomial-commitment-v1";
    const OPERATION_SCHEMA: &str = "jindo_polynomial_evaluation_v1";
    const PUBLIC_ACTION: &str = concat!(
        "{\"evaluation_point_hex\":\"",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "\"}"
    );
    const WITNESS: &str = concat!(
        "{\"polynomials_hex\":[[\"",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "\"],[\"",
        "0100000000000000000000000000000000000000000000000000000000000000",
        "\"],[\"",
        "0200000000000000000000000000000000000000000000000000000000000000",
        "\"],[\"",
        "0300000000000000000000000000000000000000000000000000000000000000",
        "\"]]}"
    );
    fn text(output: &mut Vec<u8>, value: &str) {
        output.extend_from_slice(
            &u16::try_from(value.len())
                .expect("test text length")
                .to_be_bytes(),
        );
        output.extend_from_slice(value.as_bytes());
    }
    fn bytes_u32(output: &mut Vec<u8>, value: &[u8]) {
        output.extend_from_slice(
            &u32::try_from(value.len())
                .expect("test byte length")
                .to_be_bytes(),
        );
        output.extend_from_slice(value);
    }
    fn authority_for_seed(seed: [u8; 32]) -> String {
        let private_key =
            PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("test private key");
        AccountId::new(PublicKey::from(private_key)).to_string()
    }
    fn bundle_with(
        seed: [u8; 32],
        authority: &str,
        protocol: &str,
        operation_schema: &str,
        public_action: &[u8],
        witness: &[u8],
    ) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        output.push(SCHEMA_VERSION);
        text(&mut output, WALLET_ID);
        text(&mut output, authority);
        text(&mut output, protocol);
        text(&mut output, operation_schema);
        bytes_u32(&mut output, public_action);
        output.extend_from_slice(&seed);
        bytes_u32(&mut output, witness);
        output
    }
    fn canonical_bundle() -> Vec<u8> {
        let seed = [7; 32];
        bundle_with(
            seed,
            &authority_for_seed(seed),
            PROTOCOL,
            OPERATION_SCHEMA,
            PUBLIC_ACTION.as_bytes(),
            WITNESS.as_bytes(),
        )
    }
    #[test]
    fn canonical_bundle_inspects_and_decodes_exact_jindo_request() {
        let mut bundle = canonical_bundle();
        let inspected =
            inspect_privacy_wallet_execution_bundle_v1(&bundle).expect("inspect bundle");
        assert_eq!(inspected.manifest.schema_version, 1);
        assert_eq!(inspected.manifest.wallet_id, WALLET_ID);
        assert_eq!(
            inspected.manifest.authority.to_string(),
            authority_for_seed([7; 32])
        );
        assert_eq!(
            inspected.manifest.public_key,
            PublicKey::from(
                PrivateKey::from_bytes(Algorithm::Ed25519, &[7; 32]).expect("private key")
            )
        );
        assert_eq!(
            inspected.manifest.protocol_id,
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1
        );
        assert_eq!(inspected.manifest.operation_schema, OPERATION_SCHEMA);
        assert_eq!(
            inspected.public_action_digest,
            privacy_wallet_bundle_public_action_digest_v1(PUBLIC_ACTION.as_bytes())
        );
        let decoded =
            decode_privacy_wallet_execution_bundle_v1(&mut bundle, PUBLIC_ACTION.as_bytes())
                .expect("decode bundle");
        assert_eq!(
            decoded.request.protocol_id(),
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1
        );
    }
    #[test]
    fn every_truncation_and_every_suffix_is_rejected() {
        let bundle = canonical_bundle();
        for length in 0..bundle.len() {
            assert!(
                inspect_privacy_wallet_execution_bundle_v1(&bundle[..length]).is_err(),
                "truncation at byte {length} was accepted"
            );
        }
        for suffix in [vec![0], vec![0, 0, 0, 0], b"secret".to_vec()] {
            let mut changed = bundle.clone();
            changed.extend_from_slice(&suffix);
            assert!(
                inspect_privacy_wallet_execution_bundle_v1(&changed).is_err(),
                "suffix {suffix:?} was accepted"
            );
        }
    }
    #[test]
    fn outer_lengths_utf8_schema_protocol_and_seed_fail_closed() {
        let bundle = canonical_bundle();
        let mut bad_magic = bundle.clone();
        bad_magic[0] ^= 1;
        assert_eq!(
            inspect_privacy_wallet_execution_bundle_v1(&bad_magic)
                .expect_err("bad magic")
                .stage(),
            "magic"
        );
        let mut bad_schema = bundle.clone();
        bad_schema[4] = 2;
        assert_eq!(
            inspect_privacy_wallet_execution_bundle_v1(&bad_schema)
                .expect_err("bad schema")
                .stage(),
            "schema-version"
        );
        let mut length_overflow = bundle.clone();
        length_overflow[5..7].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(inspect_privacy_wallet_execution_bundle_v1(&length_overflow).is_err());
        let mut invalid_utf8 = bundle.clone();
        invalid_utf8[7] = 0xff;
        assert_eq!(
            inspect_privacy_wallet_execution_bundle_v1(&invalid_utf8)
                .expect_err("invalid utf8")
                .stage(),
            "wallet-id"
        );
        let mut zero_seed_bundle = bundle_with(
            [0; 32],
            &authority_for_seed([7; 32]),
            PROTOCOL,
            OPERATION_SCHEMA,
            PUBLIC_ACTION.as_bytes(),
            WITNESS.as_bytes(),
        );
        assert_eq!(
            decode_privacy_wallet_execution_bundle_v1(
                &mut zero_seed_bundle,
                PUBLIC_ACTION.as_bytes(),
            )
            .err()
            .expect("zero seed")
            .stage(),
            "signer-seed"
        );
        let unsupported = bundle_with(
            [7; 32],
            &authority_for_seed([7; 32]),
            "iroha-zk-x509-stark-p256-v1",
            "zk_x509_identity_presentation_v1",
            PUBLIC_ACTION.as_bytes(),
            WITNESS.as_bytes(),
        );
        assert!(inspect_privacy_wallet_execution_bundle_v1(&unsupported).is_err());
    }
    #[test]
    fn authority_key_public_action_and_operation_schema_cannot_be_substituted() {
        let wrong_authority = bundle_with(
            [7; 32],
            &authority_for_seed([8; 32]),
            PROTOCOL,
            OPERATION_SCHEMA,
            PUBLIC_ACTION.as_bytes(),
            WITNESS.as_bytes(),
        );
        assert_eq!(
            inspect_privacy_wallet_execution_bundle_v1(&wrong_authority)
                .expect_err("authority mismatch")
                .stage(),
            "authority-key-mismatch"
        );
        let wrong_schema = bundle_with(
            [7; 32],
            &authority_for_seed([7; 32]),
            PROTOCOL,
            "jindo_polynomial_evaluation_v2",
            PUBLIC_ACTION.as_bytes(),
            WITNESS.as_bytes(),
        );
        assert_eq!(
            inspect_privacy_wallet_execution_bundle_v1(&wrong_schema)
                .expect_err("operation schema")
                .stage(),
            "operation-schema"
        );
        let mut bundle = canonical_bundle();
        assert_eq!(
            decode_privacy_wallet_execution_bundle_v1(
                &mut bundle,
                br#"{"evaluation_point_hex":"0100000000000000000000000000000000000000000000000000000000000000"}"#,
            )
            .err()
            .expect("public action substitution")
            .stage(),
            "public-action-mismatch"
        );
    }
    #[test]
    fn duplicate_extra_reordered_and_noncanonical_witness_fields_are_rejected() {
        for witness in [
            concat!(
                "{\"polynomials_hex\":[[\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"]],\"polynomials_hex\":[]}"
            ),
            concat!(
                "{\"extra\":0,\"polynomials_hex\":[[\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"]]}"
            ),
            concat!(
                "{\"polynomials_hex\":[[\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"]],\"extra\":0}"
            ),
            concat!(
                "{ \"polynomials_hex\":[[\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"]]}"
            ),
            "{\"polynomials_hex\":[]}",
            "{\"polynomials_hex\":[[\"00\"]]}",
        ] {
            let mut bundle = bundle_with(
                [7; 32],
                &authority_for_seed([7; 32]),
                PROTOCOL,
                OPERATION_SCHEMA,
                PUBLIC_ACTION.as_bytes(),
                witness.as_bytes(),
            );
            assert!(
                decode_privacy_wallet_execution_bundle_v1(&mut bundle, PUBLIC_ACTION.as_bytes(),)
                    .is_err(),
                "adversarial witness was accepted: {witness}"
            );
        }
    }
    #[test]
    fn duplicate_extra_empty_noncanonical_and_malformed_public_actions_are_rejected() {
        for public_action in [
            concat!(
                "{\"evaluation_point_hex\":\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\",\"evaluation_point_hex\":\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"}"
            ),
            concat!(
                "{\"evaluation_point_hex\":\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\",\"unexpected\":true}"
            ),
            concat!(
                "{\"unexpected\":true,\"evaluation_point_hex\":\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"}"
            ),
            concat!(
                "{ \"evaluation_point_hex\":\"",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "\"}"
            ),
            "{}",
            "{\"evaluation_point_hex\":\"00\"}",
            concat!(
                "{\"evaluation_point_hex\":\"",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "\"}"
            ),
        ] {
            let mut bundle = bundle_with(
                [7; 32],
                &authority_for_seed([7; 32]),
                PROTOCOL,
                OPERATION_SCHEMA,
                public_action.as_bytes(),
                WITNESS.as_bytes(),
            );
            assert!(
                decode_privacy_wallet_execution_bundle_v1(&mut bundle, public_action.as_bytes(),)
                    .is_err(),
                "adversarial public action was accepted: {public_action}"
            );
        }
    }
    #[test]
    fn reserve_backed_public_actions_require_scope_and_reject_unknown_fields_first() {
        for public_action in [
            br#"{"amount_decimal":"1","destination":"x","policy":null,"source":"x"}"#.as_slice(),
            br#"{"amount_decimal":"1","destination":"x","policy":null,"public_balance_scope":"global","source":"x","unexpected":0}"#.as_slice(),
        ] {
            assert_eq!(
                decode_zk_ace_request_v1(public_action, b"{}")
                    .err()
                    .expect("closed ZK-ACE public schema")
                    .stage(),
                "zk-ace-public-action"
            );
        }
        for public_action in [
            br#"{"anchor_epoch":1,"anchor_hex":"00","asset_definition_id":"x","expiry_height":1,"minimum_action_count":1,"pool_id_hex":"00"}"#.as_slice(),
            br#"{"anchor_epoch":1,"anchor_hex":"00","asset_definition_id":"x","expiry_height":1,"minimum_action_count":1,"pool_id_hex":"00","public_balance_scope":"global","unexpected":0}"#.as_slice(),
        ] {
            assert_eq!(
                decode_orchard_request_v1(public_action, b"{}")
                    .err()
                    .expect("closed Orchard public schema")
                    .stage(),
                "orchard-public-action"
            );
        }
        for public_action in [
            br#"{"asset_definition_id":"x","pool_id_hex":"00","root_epoch":1,"state_root_hex":"00"}"#.as_slice(),
            br#"{"asset_definition_id":"x","pool_id_hex":"00","public_balance_scope":"global","root_epoch":1,"state_root_hex":"00","unexpected":0}"#.as_slice(),
        ] {
            assert_eq!(
                decode_ivm_request_v1(public_action, b"{}")
                    .err()
                    .expect("closed private-IVM public schema")
                    .stage(),
                "ivm-public-action"
            );
        }
    }
    #[test]
    fn public_action_and_witness_u32_overflow_lengths_are_rejected() {
        let bundle = canonical_bundle();
        let mut cursor = Cursor::new(&bundle);
        cursor.take(5).expect("header");
        cursor.text(MAX_WALLET_ID_BYTES, "wallet").expect("wallet");
        cursor
            .text(MAX_AUTHORITY_BYTES, "authority")
            .expect("authority");
        cursor
            .text(MAX_PROTOCOL_BYTES, "protocol")
            .expect("protocol");
        cursor
            .text(MAX_OPERATION_SCHEMA_BYTES, "schema")
            .expect("schema");
        let public_length_offset = cursor.offset;
        let mut public_overflow = bundle.clone();
        public_overflow[public_length_offset..public_length_offset + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(inspect_privacy_wallet_execution_bundle_v1(&public_overflow).is_err());
        let public_length = u32::from_be_bytes(
            bundle[public_length_offset..public_length_offset + 4]
                .try_into()
                .expect("public length"),
        ) as usize;
        let witness_length_offset = public_length_offset + 4 + public_length + 32;
        let mut witness_overflow = bundle;
        witness_overflow[witness_length_offset..witness_length_offset + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(inspect_privacy_wallet_execution_bundle_v1(&witness_overflow).is_err());
    }
}
