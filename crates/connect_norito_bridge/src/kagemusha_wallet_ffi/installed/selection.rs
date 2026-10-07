//! Exact signed application/runtime originals selected by immutable Native build trust.

use super::*;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    kagemusha::kagemusha_wallet_v1::*,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier, genesis_epoch},
};
use norito::json::{Map, Value};
use std::str::FromStr as _;

include!(concat!(
    env!("OUT_DIR"),
    "/kagemusha_wallet_runtime_trust.rs"
));

const DOMAIN: &[u8] = b"bpng:taira-app-runtime-manifest:v6\0";
const APP_FIELDS: &[&str] = &[
    "schema",
    "environment",
    "generatedAt",
    "ledger",
    "consensus",
    "digitalKina",
    "routing",
    "feeSponsor",
    "contracts",
    "authorities",
    "services",
    "authentication",
    "kasumi",
    "mibankCentralPaymentAdapter",
    "retailPolicy",
    "retailStageAnchorAuthority",
    "enrollment",
    "walletRuntime",
    "firstDeviceAuthentication",
    "evidence",
];

/// Construction is private to installed Native policy; offered originals cannot choose a key.
struct RuntimeTrust(PublicKey);
impl RuntimeTrust {
    fn installed() -> Result<Self> {
        let key = INSTALLED_RUNTIME_TRUST.ok_or(Failure::code(ARTIFACTS_UNAVAILABLE))?;
        Ok(Self(
            PublicKey::from_bytes(Algorithm::Ed25519, &key).map_err(|_| invalid())?,
        ))
    }
    #[cfg(test)]
    fn test(key: PublicKey) -> Self {
        Self(key)
    }
}

struct RetainedBaseOriginals {
    _app_manifest: Box<[u8]>,
    _envelope: Box<[u8]>,
    _wallet_runtime: Box<[u8]>,
    _signed_genesis: Box<[u8]>,
}

pub(super) struct Selection {
    _originals: RetainedBaseOriginals,
    pub scheme: KagemushaWalletSchemeV1,
    pub artifact_certificate: Vec<u8>,
    pub artifact_manifest: Vec<u8>,
    pub installation: InstallationV1,
    pub asset: KagemushaWalletAssetScopeV1,
    pub asset_original: Vec<u8>,
    pub enrollment_certificate: KagemushaWalletSignerCertificateV1,
    pub android_app_policy: [u8; 32],
    pub apple_app_policy: [u8; 32],
    pub android_enrollment_policy: [u8; 32],
    pub apple_enrollment_policy: [u8; 32],
    pub regulatory_policy: KagemushaWalletRegulatoryPolicyV1,
    pub genesis: Arc<SumeragiFinalityVerifier>,
}

fn invalid() -> Failure {
    Failure::code(INVALID)
}
pub(super) fn bounded<'a>(bytes: &'a [u8], maximum: usize) -> Result<&'a [u8]> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid());
    }
    Ok(bytes)
}
fn safe_numbers(value: &Value) -> Result<()> {
    match value {
        Value::Number(number) => {
            if !number
                .as_i64()
                .is_some_and(|v| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&v))
                && !number.as_u64().is_some_and(|v| v <= 9_007_199_254_740_991)
            {
                return Err(invalid());
            }
        }
        Value::Array(values) => {
            for v in values {
                safe_numbers(v)?;
            }
        }
        Value::Object(values) => {
            for (key, v) in values {
                // The maintained schema uses ASCII property names. This keeps Native's sorted
                // writer identical to the maintained ECMAScript canonical writer.
                if !key.is_ascii() {
                    return Err(invalid());
                }
                safe_numbers(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn json(bytes: &[u8], maximum: usize, canonical: bool) -> Result<Value> {
    bounded(bytes, maximum)?;
    let limits = norito::json::JsonPreflightLimits::new(
        maximum, 65_536, maximum, maximum, maximum, 16_384, 32_768, 32_768, 65_536, 32,
    );
    json_limits(bytes, maximum, canonical, limits)
}

pub(super) fn json_limits(
    bytes: &[u8],
    maximum: usize,
    canonical: bool,
    limits: norito::json::JsonPreflightLimits,
) -> Result<Value> {
    bounded(bytes, maximum)?;
    norito::json::preflight_slice(bytes, limits).map_err(|_| invalid())?;
    let value = norito::json::from_slice(bytes).map_err(|_| invalid())?;
    safe_numbers(&value)?;
    if canonical {
        let mut original = norito::json::to_json_bounded(&value, maximum.saturating_sub(1))
            .map_err(|_| invalid())?
            .into_bytes();
        original.push(b'\n');
        if original != bytes {
            return Err(invalid());
        }
    }
    Ok(value)
}
fn object(value: &Value) -> Result<&Map> {
    value.as_object().ok_or(invalid())
}
pub(super) fn exact<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a Map> {
    let value = object(value)?;
    if value.len() != fields.len() || fields.iter().any(|key| !value.contains_key(*key)) {
        return Err(invalid());
    }
    Ok(value)
}
fn field<'a>(map: &'a Map, key: &str) -> Result<&'a Value> {
    map.get(key).ok_or(invalid())
}
pub(super) fn text<'a>(map: &'a Map, key: &str) -> Result<&'a str> {
    field(map, key)?.as_str().ok_or(invalid())
}
pub(super) fn sha(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(invalid());
    }
    let value: [u8; 32] = hex::decode(value)
        .map_err(|_| invalid())?
        .try_into()
        .map_err(|_| invalid())?;
    if value == [0; 32] {
        return Err(invalid());
    }
    Ok(value)
}
fn raw(map: &Map, key: &str, maximum: usize) -> Result<Vec<u8>> {
    let value = text(map, key)?;
    if value.is_empty() || value.len() > maximum.div_ceil(3) * 4 {
        return Err(invalid());
    }
    let bytes = STANDARD.decode(value).map_err(|_| invalid())?;
    bounded(&bytes, maximum)?;
    if STANDARD.encode(&bytes) != value {
        return Err(invalid());
    }
    Ok(bytes)
}
fn signed_app(trust: &RuntimeTrust, manifest: &[u8], envelope: &[u8]) -> Result<Value> {
    bounded(manifest, APP_MANIFEST_MAX)?;
    let signature = json(envelope, ENVELOPE_MAX, true)?;
    let signature = exact(
        &signature,
        &[
            "schema",
            "algorithm",
            "domain",
            "keyId",
            "manifestSha256",
            "signatureBase64Url",
        ],
    )?;
    let (_, public) = trust.0.to_bytes();
    if text(signature, "schema")? != "bpng.taira-app-runtime-manifest-signature.v6"
        || text(signature, "algorithm")? != "ed25519"
        || text(signature, "domain")? != "bpng:taira-app-runtime-manifest:v6"
        || text(signature, "keyId")? != format!("sha256:{}", hex::encode(BlobV1::of(public).sha256))
        || sha(text(signature, "manifestSha256")?)? != BlobV1::of(manifest).sha256
    {
        return Err(invalid());
    }
    let encoded = text(signature, "signatureBase64Url")?;
    if encoded.len() != 86 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    if bytes.len() != 64 || URL_SAFE_NO_PAD.encode(&bytes) != encoded {
        return Err(invalid());
    }
    let mut message = Vec::with_capacity(DOMAIN.len() + manifest.len());
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(manifest);
    Signature::from_bytes(&bytes)
        .verify(&trust.0, &message)
        .map_err(|_| invalid())?;
    let manifest = json(manifest, APP_MANIFEST_MAX, true)?;
    let body = exact(&manifest, APP_FIELDS)?;
    if text(body, "schema")? != "bpng.taira-app-runtime-manifest.v6"
        || text(body, "environment")? != "taira-testnet"
    {
        return Err(invalid());
    }
    first_device_authentication(body)?;
    Ok(manifest)
}

// This public selection is signed DATA; admission still authenticates the complete originals.
fn first_device_authentication(app: &Map) -> Result<()> {
    let selection = exact(
        field(app, "firstDeviceAuthentication")?,
        &[
            "schema",
            "googleOAuthClientId",
            "googleOAuthIssuer",
            "integrityCloudProjectNumber",
            "originalAuthPolicySha256",
            "verifierConfigurationSha256",
            "googlePolicySha256",
        ],
    )?;
    let client = text(selection, "googleOAuthClientId")?;
    if text(selection, "schema")? != "bpng.first-device-auth-runtime-selection.v1"
        || !(1..=1024).contains(&client.len())
        || !client.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
        || !matches!(
            text(selection, "googleOAuthIssuer")?,
            "accounts.google.com" | "https://accounts.google.com"
        )
        || !field(selection, "integrityCloudProjectNumber")?
            .as_u64()
            .is_some_and(|value| (1..=9_007_199_254_740_991).contains(&value))
    {
        return Err(invalid());
    }
    for name in [
        "originalAuthPolicySha256",
        "verifierConfigurationSha256",
        "googlePolicySha256",
    ] {
        sha(text(selection, name)?)?;
    }
    Ok(())
}

impl Selection {
    pub(super) fn installed(input: &RuntimeOriginals<'_>) -> Result<Self> {
        Self::load(&RuntimeTrust::installed()?, input)
    }
    fn load(trust: &RuntimeTrust, input: &RuntimeOriginals<'_>) -> Result<Self> {
        let app = signed_app(trust, input.app_manifest, input.envelope)?;
        let app = object(&app)?;
        let pin = exact(
            field(app, "walletRuntime")?,
            &["schema", "currentRuntimeSha256"],
        )?;
        if text(pin, "schema")? != "bpng.current-wallet-runtime-pin.v1"
            || sha(text(pin, "currentRuntimeSha256")?)?
                != BlobV1::of(bounded(input.wallet_runtime, WALLET_RUNTIME_MAX)?).sha256
        {
            return Err(invalid());
        }
        // Unlike the signed app catalog, the complete Core runtime original is an exact
        // selected public file, not necessarily a compact canonical JSON encoding.
        let runtime = json(input.wallet_runtime, WALLET_RUNTIME_MAX, false)?;
        let runtime = exact(
            &runtime,
            &[
                "schema",
                "version",
                "scheme_id",
                "scheme_original_base64",
                "asset_original_base64",
                "enrollment_certificate_original_base64",
                "artifact_signer_certificate_original_base64",
                "artifact_manifest_original_base64",
                "regulatory_policy_original_base64",
                "challenge_lifetime_ms",
                "python_path",
                "python_sha256",
                "openssl_path",
                "openssl_sha256",
                "android",
                "apple",
            ],
        )?;
        if text(runtime, "schema")? != "bpng.current-wallet-core-runtime.v1"
            || field(runtime, "version")?.as_u64() != Some(1)
        {
            return Err(invalid());
        }
        // These are authenticated issuer tool metadata, never executable Native inputs.
        // Validate the maintained original contract while keeping offline payment local.
        if !field(runtime, "challenge_lifetime_ms")?
            .as_u64()
            .is_some_and(|v| (1..=600_000).contains(&v))
        {
            return Err(invalid());
        }
        for name in ["python", "openssl"] {
            let path = text(runtime, &format!("{name}_path"))?;
            if path.is_empty()
                || path.len() > ROOT_MAX
                || path.contains('\0')
                || !std::path::Path::new(path).is_absolute()
            {
                return Err(invalid());
            }
            sha(text(runtime, &format!("{name}_sha256"))?)?;
        }
        let scheme = KagemushaWalletSchemeV1::decode_canonical(
            &raw(
                runtime,
                "scheme_original_base64",
                KAGEMUSHA_WALLET_SCHEME_MAX_BYTES_V1,
            )?,
            &sha(text(runtime, "scheme_id")?)?,
        )
        .map_err(|_| invalid())?;
        let artifact_certificate = raw(
            runtime,
            "artifact_signer_certificate_original_base64",
            KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
        )?;
        let signer =
            KagemushaWalletSignerCertificateV1::decode_canonical(&artifact_certificate, &scheme)
                .map_err(|_| invalid())?;
        signer
            .verify_role(&scheme, KagemushaWalletSignerRoleV1::Artifact)
            .map_err(|_| invalid())?;
        let artifact_manifest = raw(
            runtime,
            "artifact_manifest_original_base64",
            KAGEMUSHA_WALLET_ARTIFACT_MANIFEST_MAX_BYTES_V1,
        )?;
        let manifest =
            KagemushaWalletArtifactManifestV1::decode_canonical(&artifact_manifest, &scheme)
                .map_err(|_| invalid())?;
        manifest.verify(&scheme, &signer).map_err(|_| invalid())?;
        let asset_original = raw(
            runtime,
            "asset_original_base64",
            iroha_core_zk::kagemusha_wallet_intake_v1::ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1,
        )?;
        let asset: KagemushaWalletAssetScopeV1 = norito::decode_canonical_with_limits(
            &asset_original,
            norito::canonical_decode_limits(asset_original.len()),
        )
        .map_err(|_| invalid())?;
        asset.validate().map_err(|_| invalid())?;
        let enrollment_certificate = KagemushaWalletSignerCertificateV1::decode_canonical(
            &raw(
                runtime,
                "enrollment_certificate_original_base64",
                KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
            )?,
            &scheme,
        )
        .map_err(|_| invalid())?;
        enrollment_certificate
            .verify_role(&scheme, KagemushaWalletSignerRoleV1::Enrollment)
            .map_err(|_| invalid())?;
        let policy_bytes = raw(runtime, "regulatory_policy_original_base64", 4096)?;
        let regulatory_policy: KagemushaWalletRegulatoryPolicyV1 =
            norito::decode_canonical_with_limits(
                &policy_bytes,
                norito::canonical_decode_limits(policy_bytes.len()),
            )
            .map_err(|_| invalid())?;
        regulatory_policy.validate().map_err(|_| invalid())?;
        let platform_policy = |name| -> Result<[u8; 32]> {
            let platform = exact(
                field(runtime, name)?,
                &[
                    "app_policy_hex",
                    "enrollment_policy_hex",
                    "verifier_configuration_path",
                ],
            )?;
            sha(text(platform, "enrollment_policy_hex")?)?;
            if text(platform, "verifier_configuration_path")?
                != format!("srv/etc/kagemusha/wallet-e1-{name}.json")
            {
                return Err(invalid());
            }
            sha(text(platform, "app_policy_hex")?)
        };
        let ledger = exact(
            field(app, "ledger")?,
            &[
                "toriiUrl",
                "networkId",
                "networkPrefix",
                "chainId",
                "irohaSourceCommit",
                "irohaBuildSha256",
            ],
        )?;
        let consensus = exact(
            field(app, "consensus")?,
            &[
                "mode",
                "protocolVersion",
                "networkId",
                "genesisBlockHash",
                "signedGenesisSha256",
                "genesisPublicKey",
                "finalityVerifierSha256",
                "validators",
                "checkpointSha256",
                "checkpointHeight",
                "checkpointContextId",
            ],
        )?;
        sha(text(consensus, "signedGenesisSha256")?)?;
        sha(text(consensus, "genesisBlockHash")?)?;
        sha(text(consensus, "finalityVerifierSha256")?)?;
        let original = bounded(input.signed_genesis, GENESIS_MAX)?;
        if BlobV1::of(original).sha256 != sha(text(consensus, "signedGenesisSha256")?)?
            || text(consensus, "networkId")? != text(ledger, "networkId")?
            || field(consensus, "protocolVersion")?.as_u64() != Some(1)
        {
            return Err(invalid());
        }
        let genesis = iroha_genesis::decode_signed_genesis(original).map_err(|_| invalid())?;
        if *genesis.hash().as_ref() != sha(text(consensus, "genesisBlockHash")?)? {
            return Err(invalid());
        }
        let network = NetworkId::from_genesis_hash(genesis.hash());
        if network.to_string() != text(ledger, "networkId")?
            || *network.as_bytes() != scheme.network_id
        {
            return Err(invalid());
        }
        let public =
            PublicKey::from_str(text(consensus, "genesisPublicKey")?).map_err(|_| invalid())?;
        if public.algorithm() != Algorithm::Ed25519
            || genesis
                .external_transactions()
                .next()
                .and_then(|t| t.authority().try_signatory())
                != Some(&public)
        {
            return Err(invalid());
        }
        // This actual native reader verifies the full original block/transaction signatures,
        // commitments, PoPs and every signed initial-epoch/schedule parameter.
        let epoch = genesis_epoch(&genesis).map_err(|_| invalid())?;
        let mode = if epoch.mode.is_permissioned() {
            "iroha3-consensus::permissioned-sumeragi@v1"
        } else {
            "iroha3-consensus::npos-sumeragi@v1"
        };
        if text(consensus, "mode")? != mode {
            return Err(invalid());
        }
        let validators = field(consensus, "validators")?
            .as_array()
            .ok_or(invalid())?;
        if validators.len() != epoch.committee.len() {
            return Err(invalid());
        }
        let mut roster = Vec::with_capacity(validators.len());
        for (selected, member) in validators.iter().zip(&epoch.committee) {
            let selected = exact(
                selected,
                &[
                    "peerId",
                    "directToriiUrl",
                    "nodeFingerprint",
                    "buildFingerprint",
                    "configFingerprint",
                ],
            )?;
            if PublicKey::from_str(text(selected, "peerId")?).map_err(|_| invalid())?
                != *member.validator.public_key()
            {
                return Err(invalid());
            }
            roster.push(FinalityValidator {
                public_key: member.validator.public_key().clone(),
                proof_of_possession: member.proof_of_possession.clone(),
            });
        }
        let native = SumeragiFinalityVerifier::new(&genesis, text(ledger, "chainId")?, roster)
            .map_err(|_| invalid())?;
        let kina = exact(
            field(app, "digitalKina")?,
            &[
                "assetAlias",
                "assetDefinitionId",
                "scale",
                "owningDomain",
                "physicalLaneId",
                "physicalLaneAlias",
                "physicalDataspaceId",
                "physicalDataspaceAlias",
                "registrationTransactionHash",
            ],
        )?;
        if AssetDefinitionId::from_str(text(kina, "assetDefinitionId")?).map_err(|_| invalid())?
            != asset.asset
            || field(kina, "scale")?.as_u64() != Some(u64::from(asset.scale))
        {
            return Err(invalid());
        }
        Ok(Self {
            _originals: RetainedBaseOriginals {
                _app_manifest: input.app_manifest.into(),
                _envelope: input.envelope.into(),
                _wallet_runtime: input.wallet_runtime.into(),
                _signed_genesis: input.signed_genesis.into(),
            },
            scheme,
            artifact_certificate,
            artifact_manifest,
            installation: InstallationV1 {
                scheme_id: scheme.scheme_id(),
                manifest_digest: manifest.manifest_digest(),
            },
            asset,
            asset_original,
            enrollment_certificate,
            android_app_policy: platform_policy("android")?,
            apple_app_policy: platform_policy("apple")?,
            android_enrollment_policy: sha(text(
                object(field(runtime, "android")?)?,
                "enrollment_policy_hex",
            )?)?,
            apple_enrollment_policy: sha(text(
                object(field(runtime, "apple")?)?,
                "enrollment_policy_hex",
            )?)?,
            regulatory_policy,
            genesis: Arc::new(native),
        })
    }
}

#[cfg(test)]
mod tests;
