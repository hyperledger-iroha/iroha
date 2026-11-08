//! Native projection of the whole authenticated BOI v1 application original.
//!
//! Core's service admission stays with Core. This module binds
//! only the fields consumed by this installation owner; all other signed fields
//! remain retained DATA and cannot grant a monetary or service capability.

use super::boi_registry::*;
use super::*;
use std::collections::BTreeSet;

// The cutover removed `firstDeviceAuthentication` (Google sign-up key, OAuth client and Play
// Integrity project), the retail OTP `authentication` policy and the retail stage anchor
// authority (Core verifies native checkpoint-rooted finality). They are refused, not ignored.
const APP_FIELDS: &[&str] = &[
    "schema",
    "environment",
    "generatedAt",
    "ledger",
    "consensus",
    "digitalShekel",
    "routing",
    "feeSponsor",
    "contracts",
    "authorities",
    "services",
    "enrollment",
    "participants",
    "walletRuntime",
    "evidence",
];

pub(super) fn application(document: &Value) -> Result<&Map> {
    // The retired retailPolicy field is forbidden, including null or signed offers.
    let app = exact(document, APP_FIELDS)?;
    if text(app, "schema")? != "boi.taira-app-runtime-manifest.v1"
        || text(app, "environment")? != BOI_ENVIRONMENT
    {
        return Err(invalid());
    }
    // Validate the closed outer envelope while leaving unrelated Core policies as
    // authenticated originals, not a second interpretation of Core authority.
    for name in ["feeSponsor", "enrollment", "evidence"] {
        object(field(app, name)?)?;
    }
    for name in ["contracts", "authorities"] {
        if field(app, name)?.as_array().is_none() {
            return Err(invalid());
        }
    }
    let timestamp = text(app, "generatedAt")?.as_bytes();
    if timestamp.len() != 20
        || timestamp.iter().enumerate().any(|(i, b)| match i {
            4 | 7 => *b != b'-',
            10 => *b != b'T',
            13 | 16 => *b != b':',
            19 => *b != b'Z',
            _ => !b.is_ascii_digit(),
        })
    {
        return Err(invalid());
    }
    let decimal = |start: usize, end: usize| {
        timestamp[start..end]
            .iter()
            .fold(0u32, |value, digit| value * 10 + u32::from(digit - b'0'))
    };
    let year = decimal(0, 4);
    let month = decimal(5, 7);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return Err(invalid()),
    };
    if !(1..=days).contains(&decimal(8, 10))
        || decimal(11, 13) > 23
        || decimal(14, 16) > 59
        || decimal(17, 19) > 59
    {
        return Err(invalid());
    }
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
    let commit = text(ledger, "irohaSourceCommit")?;
    if text(ledger, "toriiUrl")? != BOI_TORII_URL
        || field(ledger, "networkPrefix")?.as_u64() != Some(u64::from(BOI_ADDRESS_PREFIX))
        || text(ledger, "chainId")? != BOI_CHAIN_ID
        || commit.len() != 40
        || commit.bytes().all(|byte| byte == b'0')
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    sha(text(ledger, "irohaBuildSha256")?)?;
    asset(app)?;
    services(app)?;
    participants(app)?;
    enrollment_routes(app)?;
    let consensus = object(field(app, "consensus")?)?;
    let validators = field(consensus, "validators")?
        .as_array()
        .ok_or(invalid())?;
    if validators.len() != BOI_VALIDATOR_BASES.len() {
        return Err(invalid());
    }
    let mut peers = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for (row, base) in validators.iter().zip(BOI_VALIDATOR_BASES) {
        let row = exact(
            row,
            &[
                "peerId",
                "directToriiUrl",
                "nodeFingerprint",
                "buildFingerprint",
                "configFingerprint",
            ],
        )?;
        if text(row, "directToriiUrl")? != base
            || !peers.insert(text(row, "peerId")?)
            || !nodes.insert(sha(text(row, "nodeFingerprint")?)?)
        {
            return Err(invalid());
        }
        for name in ["buildFingerprint", "configFingerprint"] {
            sha(text(row, name)?)?;
        }
    }
    Ok(app)
}

pub(super) fn asset(app: &Map) -> Result<&Map> {
    let asset = exact(
        field(app, "digitalShekel")?,
        &[
            "assetAlias",
            "assetDefinitionId",
            "name",
            "isoCurrency",
            "symbol",
            "scale",
            "owningDomain",
            "owningDataspace",
            "balanceScopePolicy",
            "physicalLaneId",
            "physicalLaneAlias",
            "physicalDataspaceId",
            "physicalDataspaceAlias",
            "registrationTransactionHash",
        ],
    )?;
    if text(asset, "assetAlias")? != BOI_ASSET_ALIAS
        || field(asset, "scale")?.as_u64() != Some(u64::from(BOI_ASSET_SCALE))
        || text(asset, "owningDomain")? != BOI_OWNING_DOMAIN
        || text(asset, "name")? != BOI_ASSET_NAME
        || text(asset, "isoCurrency")? != BOI_ASSET_ISO_CURRENCY
        || text(asset, "symbol")? != "₪"
        || text(asset, "owningDataspace")? != BOI_DATASPACE_ID_TEXT
        || text(asset, "balanceScopePolicy")? != BOI_BALANCE_POLICY
        || field(asset, "physicalLaneId")?.as_u64() != Some(u64::from(BOI_LANE_ID))
        || text(asset, "physicalLaneAlias")? != BOI_DATASPACE_ALIAS
        || text(asset, "physicalDataspaceId")? != BOI_DATASPACE_ID_TEXT
        || text(asset, "physicalDataspaceAlias")? != BOI_DATASPACE_ALIAS
    {
        return Err(invalid());
    }
    // The signed release selects the current registered asset. Selection::load
    // additionally binds it to the exact canonical Native asset original.
    AssetDefinitionId::from_str(text(asset, "assetDefinitionId")?).map_err(|_| invalid())?;
    if field(asset, "owningDataspace")? != field(asset, "physicalDataspaceId")? {
        return Err(invalid());
    }
    sha(text(asset, "registrationTransactionHash")?)?;
    let routing = exact(
        field(app, "routing")?,
        &[
            "physicalLaneId",
            "physicalLaneAlias",
            "physicalDataspaceId",
            "physicalDataspaceAlias",
            "centralAliasScope",
            "participantAliasScopes",
        ],
    )?;
    for name in [
        "physicalLaneId",
        "physicalLaneAlias",
        "physicalDataspaceId",
        "physicalDataspaceAlias",
    ] {
        if field(asset, name)? != field(routing, name)? {
            return Err(invalid());
        }
    }
    if text(routing, "centralAliasScope")? != BOI_CENTRAL_ALIAS_SCOPE {
        return Err(invalid());
    }
    let scopes = field(routing, "participantAliasScopes")?
        .as_array()
        .ok_or(invalid())?;
    if scopes.len() != BOI_PARTICIPANTS.len()
        || scopes
            .iter()
            .zip(BOI_PARTICIPANTS)
            .any(|(scope, bank)| scope.as_str() != Some(bank.alias_scope))
    {
        return Err(invalid());
    }
    Ok(asset)
}

fn services(app: &Map) -> Result<()> {
    let expected = [
        ("publicSiteUrl", "https://digital-shekel.soramitsu.io"),
        ("operationsPortalUrl", "https://boi.soramitsu.io"),
        ("participantPortalUrl", "https://boi-banks.soramitsu.io"),
        ("coreApiUrl", "https://boi-core.soramitsu.io/boi"),
        (
            "operatorApiUrl",
            "https://boi-core.soramitsu.io/boi/operator",
        ),
        ("explorerUrl", "https://explorer-boi.soramitsu.io"),
        (
            "androidApkUrl",
            "https://boi.soramitsu.io/downloads/android/digital-shekel.apk",
        ),
    ];
    let mut fields: Vec<&str> = expected.iter().map(|(name, _)| *name).collect();
    fields.push("walletArtifactsUrl");
    let services = exact(field(app, "services")?, &fields)?;
    for (name, value) in expected {
        if text(services, name)? != value {
            return Err(invalid());
        }
    }
    // A signed generation is one path segment, never a caller-selected origin.
    let generation = text(services, "walletArtifactsUrl")?
        .strip_prefix("https://boi.soramitsu.io/wallet-artifacts/")
        .and_then(|s| s.strip_suffix('/'))
        .ok_or(invalid())?;
    if generation.is_empty()
        || generation.len() > 64
        || !generation.as_bytes()[0].is_ascii_alphanumeric()
        || !generation
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return Err(invalid());
    }
    Ok(())
}

fn participants(app: &Map) -> Result<()> {
    let rows = field(app, "participants")?.as_array().ok_or(invalid())?;
    if rows.len() != BOI_PARTICIPANTS.len() {
        return Err(invalid());
    }
    let mut accounts = BTreeSet::new();
    for (row, bank) in rows.iter().zip(BOI_PARTICIPANTS) {
        let row = exact(
            row,
            &[
                "id",
                "ordinal",
                "aliasScope",
                "displayName",
                "legalName",
                "brandColorHex",
                "portalUrl",
                "coreApiUrl",
                "operatorApiUrl",
                "kycVaultUrl",
                "sessionIssuer",
                "sessionAudience",
                "kagemushaIssuer",
                "settlementAccountId",
                "runtimeAccountId",
                "enrollmentAccountId",
            ],
        )?;
        for (name, value) in [
            ("id", bank.id),
            ("aliasScope", bank.alias_scope),
            ("portalUrl", bank.portal_url),
            ("coreApiUrl", bank.core_api_url),
            ("operatorApiUrl", bank.operator_api_url),
            ("kycVaultUrl", bank.kyc_vault_url),
            ("sessionIssuer", bank.session_issuer),
            ("sessionAudience", bank.session_audience),
        ] {
            if text(row, name)? != value {
                return Err(invalid());
            }
        }
        if field(row, "ordinal")?.as_u64() != Some(u64::from(bank.ordinal))
            || field(row, "kagemushaIssuer")?.as_bool() != Some(bank.kagemusha_issuer)
        {
            return Err(invalid());
        }
        // Display strings remain authenticated DATA, never enrollment authority.
        let names = exact(field(row, "displayName")?, &["en", "he", "ar", "ja"])?;
        for language in ["en", "he", "ar", "ja"] {
            text(names, language)?;
        }
        text(row, "legalName")?;
        text(row, "brandColorHex")?;
        for name in [
            "settlementAccountId",
            "runtimeAccountId",
            "enrollmentAccountId",
        ] {
            let account = iroha_data_model::account::address::AccountAddress::parse_encoded(
                text(row, name)?,
                Some(BOI_ADDRESS_PREFIX),
            )
            .and_then(|address| address.to_account_id())
            .map_err(|_| invalid())?;
            if !accounts.insert(account) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn enrollment_routes(app: &Map) -> Result<()> {
    let enrollment = object(field(app, "enrollment")?)?;
    let routes = exact(
        field(enrollment, "stageRoutes")?,
        &[
            "accountSetup",
            "uaidBindingManifest",
            "feeSponsorEnrollment",
            "bankReadGrant",
        ],
    )?;
    for name in [
        "accountSetup",
        "uaidBindingManifest",
        "feeSponsorEnrollment",
        "bankReadGrant",
    ] {
        let row = exact(field(routes, name)?, &["laneId", "physicalDataspaceId"])?;
        let home = matches!(name, "uaidBindingManifest" | "bankReadGrant");
        if field(row, "laneId")?.as_u64() != Some(if home { u64::from(BOI_LANE_ID) } else { 0 })
            || text(row, "physicalDataspaceId")? != if home { BOI_DATASPACE_ID_TEXT } else { "0" }
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn blob(value: &Value, maximum: usize) -> Result<BlobV1> {
    let value = exact(value, &["bytes", "sha256"])?;
    let bytes = field(value, "bytes")?
        .as_u64()
        .filter(|bytes| (1..=maximum as u64).contains(bytes))
        .ok_or(invalid())?;
    Ok(BlobV1 {
        bytes,
        sha256: sha(text(value, "sha256")?)?,
    })
}
fn financial(value: &Value, input: &RuntimeOriginals<'_>) -> Result<Option<FinancialSelection>> {
    if matches!(value, Value::Null) {
        if !input.verifier_pack.is_empty()
            || !input.producer_inventory.is_empty()
            || !input.originals_root.is_empty()
        {
            return Err(invalid());
        }
        return Ok(None);
    }
    if input.verifier_pack.is_empty()
        || input.producer_inventory.is_empty()
        || input.originals_root.is_empty()
    {
        return Err(invalid()); // Signed full selection cannot be downgraded to initial absence.
    }
    let record = exact(
        value,
        &[
            "schema",
            "verifierPack",
            "producerInventory",
            "transport",
            "producerCatalogDigest",
        ],
    )?;
    if text(record, "schema")? != "iroha.kagemusha.wallet-financial-originals.v1" {
        return Err(invalid());
    }
    let pack = blob(field(record, "verifierPack")?, VERIFIER_PACK_MAX_BYTES_V1)?;
    let catalog = blob(field(record, "producerInventory")?, CATALOG_MAX_BYTES_V1)?;
    let transport = blob(field(record, "transport")?, 32 * 1024 * 1024)?;
    if !input.verifier_pack.is_empty()
        && (pack != BlobV1::of(input.verifier_pack)
            || catalog != BlobV1::of(input.producer_inventory))
    {
        return Err(invalid());
    }
    Ok(Some(FinancialSelection {
        pack_identity: pack,
        catalog_identity: catalog,
        producer_catalog_digest: sha(text(record, "producerCatalogDigest")?)?,
        transport_identity: transport,
    }))
}
pub(super) fn require_runtime(
    app: &Map,
    input: &RuntimeOriginals<'_>,
) -> Result<([u8; 32], Option<FinancialSelection>)> {
    let pin = exact(
        field(app, "walletRuntime")?,
        &[
            "schema",
            "currentRuntimeSha256",
            "financialOriginals",
            "feeSponsor",
        ],
    )?;
    // Fee policy is interpreted by the app and Core, but must remain inside the
    // authenticated offline selection rather than borrowing the retail program.
    object(field(pin, "feeSponsor")?)?;
    let digest = sha(text(pin, "currentRuntimeSha256")?)?;
    if text(pin, "schema")? != "boi.current-wallet-runtime-pin.v1"
        || BlobV1::of(input.wallet_runtime).sha256 != digest
    {
        return Err(invalid());
    }
    Ok((digest, financial(field(pin, "financialOriginals")?, input)?))
}

pub(super) fn runtime_tools(runtime: &Map) -> Result<()> {
    // These exact public paths are signed Core configuration DATA. Native neither
    // executes them nor treats them as a verifier or producer capability.
    for name in ["python_path", "openssl_path"] {
        let path = text(runtime, name)?;
        if !path.starts_with('/')
            || path.contains('\0')
            || path[1..]
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(invalid());
        }
    }
    for name in ["python_sha256", "openssl_sha256"] {
        sha(text(runtime, name)?)?;
    }
    Ok(())
}

pub(super) fn checkpoint(consensus: &Map) -> Result<()> {
    if [
        "checkpointSha256",
        "checkpointHeight",
        "checkpointContextId",
    ]
    .iter()
    .all(|name| consensus.get(*name) == Some(&Value::Null))
    {
        return Ok(());
    }
    sha(text(consensus, "checkpointSha256")?)?;
    let context = sha(text(consensus, "checkpointContextId")?)?;
    if !field(consensus, "checkpointHeight")?
        .as_u64()
        .is_some_and(|h| (2..=9_007_199_254_740_991).contains(&h))
        || context[31] & 1 != 1
        || (context[..31] == [0; 31] && context[31] == 1)
    {
        return Err(invalid());
    }
    Ok(())
}
