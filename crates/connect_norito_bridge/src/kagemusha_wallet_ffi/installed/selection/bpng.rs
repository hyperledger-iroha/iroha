//! Native projection of the whole authenticated BPNG v7 application original.
//!
//! Core's service admission stays with Core. This module binds
//! only the fields consumed by this installation owner; all other signed fields
//! remain retained DATA and cannot grant a monetary or service capability.

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
    "digitalKina",
    "routing",
    "feeSponsor",
    "contracts",
    "authorities",
    "services",
    "kasumi",
    "mibankCentralPaymentAdapter",
    "enrollment",
    "walletRuntime",
    "evidence",
];
pub(super) const VALIDATOR_BASES: [&str; 4] = [
    "https://taira.sora.org:9441",
    "https://taira.sora.org:9442",
    "https://taira.sora.org:9443",
    "https://taira.sora.org:9444",
];

pub(super) fn application(document: &Value) -> Result<&Map> {
    // The retired retailPolicy field is forbidden, including null or signed offers.
    let app = exact(document, APP_FIELDS)?;
    if text(app, "schema")? != "bpng.taira-app-runtime-manifest.v7"
        || text(app, "environment")? != "taira-testnet"
    {
        return Err(invalid());
    }
    // Validate the closed outer envelope while leaving unrelated Core policies as
    // authenticated originals, not a second interpretation of Core authority.
    for name in [
        "feeSponsor",
        "kasumi",
        "mibankCentralPaymentAdapter",
        "enrollment",
        "evidence",
    ] {
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
    if text(ledger, "toriiUrl")? != "https://taira.sora.org"
        || field(ledger, "networkPrefix")?.as_u64() != Some(369)
        || text(ledger, "chainId")? != "fc56984b-2be7-431d-840e-21514d1883f0"
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
    let consensus = object(field(app, "consensus")?)?;
    let validators = field(consensus, "validators")?
        .as_array()
        .ok_or(invalid())?;
    if validators.len() != VALIDATOR_BASES.len() {
        return Err(invalid());
    }
    let mut peers = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for (row, base) in validators.iter().zip(VALIDATOR_BASES) {
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
        field(app, "digitalKina")?,
        &[
            "assetAlias",
            "assetDefinitionId",
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
    if text(asset, "assetAlias")? != "kina#bpng"
        || field(asset, "scale")?.as_u64() != Some(2)
        || field(asset, "owningDomain")? != &Value::Null
        || text(asset, "owningDataspace")? != "8648377547929788715"
        || !matches!(
            text(asset, "balanceScopePolicy")?,
            "Global" | "DataspaceRestricted"
        )
        || !field(asset, "physicalLaneId")?
            .as_u64()
            .is_some_and(|id| (1..=u32::MAX as u64).contains(&id) && id != 3)
        || text(asset, "physicalLaneAlias")? != "bpng"
        || text(asset, "physicalDataspaceId")? != "8648377547929788715"
        || text(asset, "physicalDataspaceAlias")? != "bpng"
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
            "bpngAliasScope",
            "mibankAliasScope",
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
    if text(routing, "bpngAliasScope")? != "bpng"
        || text(routing, "mibankAliasScope")? != "mibank.bpng"
    {
        return Err(invalid());
    }
    Ok(asset)
}

fn services(app: &Map) -> Result<()> {
    let expected = [
        ("bpngPortalUrl", "https://bpng.soramitsu.io"),
        ("mibankPortalUrl", "https://mibank.soramitsu.io"),
        ("coreApiUrl", "https://bpng-core.soramitsu.io"),
        ("operatorApiUrl", "https://bpng-core.soramitsu.io/operator"),
        ("mibankCoreApiUrl", "https://bpng-core.soramitsu.io/mibank"),
        (
            "mibankOperatorApiUrl",
            "https://bpng-core.soramitsu.io/mibank/operator",
        ),
        ("kycVaultUrl", "https://mibank-kyc.soramitsu.io"),
        ("explorerUrl", "https://explorer-bpng.soramitsu.io"),
    ];
    let services = exact(field(app, "services")?, &expected.map(|(name, _)| name))?;
    for (name, value) in expected {
        if text(services, name)? != value {
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
/// Retained wallet graph selection of the signed `financialOriginals` record.
const FINANCIAL_SCHEMA: &str = "iroha.kagemusha.wallet-financial-originals.v1";
/// Explicit Spartan profile of the same signed record. Only this exact schema selects the
/// single-original carrier; no shape, size or absent member of another record does.
const SPARTAN_SCHEMA: &str = "iroha.kagemusha.spartan.wallet-financial-originals.v1";
// Signed-pin ceilings of the small Spartan carrier metadata. They refuse an oversized pin
// before any file is opened; the carrier reader enforces its exact bounds on the installed
// files themselves.
const SPARTAN_PACK_MAX: usize = 16 * 1024 + 2048;
const SPARTAN_METADATA_MAX: usize = 2048;

/// At most one financial profile of one signed release: the retained graph or Spartan.
type FinancialProfile = (Option<FinancialSelection>, Option<SpartanSelection>);

fn financial(value: &Value, input: &RuntimeOriginals<'_>) -> Result<FinancialProfile> {
    if matches!(value, Value::Null) {
        if !input.verifier_pack.is_empty()
            || !input.producer_inventory.is_empty()
            || !input.originals_root.is_empty()
        {
            return Err(invalid());
        }
        return Ok((None, None));
    }
    if input.verifier_pack.is_empty()
        || input.producer_inventory.is_empty()
        || input.originals_root.is_empty()
    {
        return Err(invalid()); // Signed full selection cannot be downgraded to initial absence.
    }
    // The signed schema alone selects the profile. Each profile is a closed record, so a
    // member of one can never be offered under the other.
    match text(object(value)?, "schema")? {
        FINANCIAL_SCHEMA => Ok((Some(retained_graph(value, input)?), None)),
        SPARTAN_SCHEMA => Ok((None, Some(spartan_profile(value, input)?))),
        _ => Err(invalid()),
    }
}
fn retained_graph(value: &Value, input: &RuntimeOriginals<'_>) -> Result<FinancialSelection> {
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
    let pack = blob(field(record, "verifierPack")?, VERIFIER_PACK_MAX_BYTES_V1)?;
    let catalog = blob(field(record, "producerInventory")?, CATALOG_MAX_BYTES_V1)?;
    let transport = blob(field(record, "transport")?, 32 * 1024 * 1024)?;
    if !input.verifier_pack.is_empty()
        && (pack != BlobV1::of(input.verifier_pack)
            || catalog != BlobV1::of(input.producer_inventory))
    {
        return Err(invalid());
    }
    Ok(FinancialSelection {
        pack_identity: pack,
        catalog_identity: catalog,
        producer_catalog_digest: sha(text(record, "producerCatalogDigest")?)?,
        transport_identity: transport,
    })
}
/// The four carrier pins plus the registered asset and manifest pins, all signed. The two
/// offered metadata frames must be byte-identical to their pins; the programs themselves are
/// never offered and are read only from the installed carrier.
fn spartan_profile(value: &Value, input: &RuntimeOriginals<'_>) -> Result<SpartanSelection> {
    let record = exact(
        value,
        &[
            "schema",
            "verifierPack",
            "producerInventory",
            "transport",
            "producerCatalogDigest",
            "assetDigest",
            "manifestDigest",
        ],
    )?;
    let pack = blob(field(record, "verifierPack")?, SPARTAN_PACK_MAX)?;
    let inventory = blob(field(record, "producerInventory")?, SPARTAN_METADATA_MAX)?;
    let transport = blob(field(record, "transport")?, SPARTAN_METADATA_MAX)?;
    if pack != BlobV1::of(input.verifier_pack)
        || inventory != BlobV1::of(input.producer_inventory)
    {
        return Err(invalid());
    }
    Ok(SpartanSelection {
        release: SpartanReleasePins {
            pack,
            inventory,
            catalog_digest: sha(text(record, "producerCatalogDigest")?)?,
            transport,
        },
        asset: sha(text(record, "assetDigest")?)?,
        manifest_digest: sha(text(record, "manifestDigest")?)?,
    })
}
pub(super) fn require_runtime(
    app: &Map,
    input: &RuntimeOriginals<'_>,
) -> Result<([u8; 32], Option<FinancialSelection>, Option<SpartanSelection>)> {
    let pin = exact(
        field(app, "walletRuntime")?,
        &["schema", "currentRuntimeSha256", "financialOriginals"],
    )?;
    let digest = sha(text(pin, "currentRuntimeSha256")?)?;
    if text(pin, "schema")? != "bpng.current-wallet-runtime-pin.v1"
        || BlobV1::of(input.wallet_runtime).sha256 != digest
    {
        return Err(invalid());
    }
    let (graph, profile) = financial(field(pin, "financialOriginals")?, input)?;
    Ok((digest, graph, profile))
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

// Signed-record grammar only. These DATA frames satisfy no carrier reader, signature,
// program import or custody check and establish no financial readiness.
#[cfg(test)]
mod financial_profile_tests {
    use super::*;

    const PACK: &[u8] = b"PUBLIC_SOFTWARE_DATA_UNADMITTED_SPARTAN_PACK";
    const INVENTORY: &[u8] = b"PUBLIC_SOFTWARE_DATA_UNADMITTED_SPARTAN_INVENTORY";
    const TRANSPORT: &[u8] = b"PUBLIC_SOFTWARE_DATA_UNADMITTED_SPARTAN_TRANSPORT";
    const ROOT: &[u8] = b"/PUBLIC_SOFTWARE_DATA_UNADMITTED_CARRIER";
    const MEMBERS: [&str; 7] = [
        "schema",
        "verifierPack",
        "producerInventory",
        "transport",
        "producerCatalogDigest",
        "assetDigest",
        "manifestDigest",
    ];

    fn offered<'a>(pack: &'a [u8], inventory: &'a [u8], root: &'a [u8]) -> RuntimeOriginals<'a> {
        RuntimeOriginals {
            app_manifest: b"app",
            envelope: b"envelope",
            wallet_runtime: b"runtime",
            verifier_pack: pack,
            producer_inventory: inventory,
            signed_genesis: b"genesis",
            originals_root: root,
            registration_source: b"",
        }
    }
    fn identity(bytes: &[u8]) -> Value {
        norito::json!({"bytes":(bytes.len()),"sha256":(hex::encode(BlobV1::of(bytes).sha256))})
    }
    fn record(schema: &str) -> Value {
        norito::json!({"schema":(schema),
            "verifierPack":(identity(PACK)),"producerInventory":(identity(INVENTORY)),
            "transport":(identity(TRANSPORT)),"producerCatalogDigest":(hex::encode([0x13; 32])),
            "assetDigest":(hex::encode([0x21; 32])),"manifestDigest":(hex::encode([0x34; 32]))})
    }
    fn changed(mut value: Value, key: &str, replacement: Option<Value>) -> Value {
        let Value::Object(map) = &mut value else {
            panic!("fixture object")
        };
        if let Some(replacement) = replacement {
            map.insert(key.into(), replacement);
        } else {
            map.remove(key);
        }
        value
    }
    fn retained_record() -> Value {
        let mut value = record(FINANCIAL_SCHEMA);
        for member in ["assetDigest", "manifestDigest"] {
            value = changed(value, member, None);
        }
        value
    }

    #[test]
    fn explicit_spartan_schema_selects_only_the_spartan_profile_pins() {
        let input = offered(PACK, INVENTORY, ROOT);
        let (graph, profile) = financial(&record(SPARTAN_SCHEMA), &input).unwrap();
        assert!(graph.is_none());
        let profile = profile.unwrap();
        assert_eq!(profile.release.pack, BlobV1::of(PACK));
        assert_eq!(profile.release.inventory, BlobV1::of(INVENTORY));
        assert_eq!(profile.release.transport, BlobV1::of(TRANSPORT));
        assert_eq!(profile.release.catalog_digest, [0x13; 32]);
        assert_eq!(profile.asset, [0x21; 32]);
        assert_eq!(profile.manifest_digest, [0x34; 32]);
        // The retained graph record keeps its exact members and never yields a profile.
        let (graph, profile) = financial(&retained_record(), &input).unwrap();
        assert!(profile.is_none());
        assert_eq!(graph.unwrap().pack_identity, BlobV1::of(PACK));
    }

    #[test]
    fn profiles_are_closed_and_selected_by_schema_never_by_shape() {
        let input = offered(PACK, INVENTORY, ROOT);
        // Spartan members under the retained schema, and the reverse, are refused.
        assert!(financial(&record(FINANCIAL_SCHEMA), &input).is_err());
        let reversed = changed(
            retained_record(),
            "schema",
            Some(Value::String(SPARTAN_SCHEMA.into())),
        );
        assert!(financial(&reversed, &input).is_err());
        for schema in [
            "iroha.kagemusha.spartan.wallet-financial-originals.v2",
            "iroha.kagemusha.wallet-financial-originals",
            "",
        ] {
            assert!(financial(&record(schema), &input).is_err(), "{schema}");
        }
        for member in MEMBERS {
            let missing = changed(record(SPARTAN_SCHEMA), member, None);
            assert!(financial(&missing, &input).is_err(), "{member}");
        }
        for (member, value) in [
            ("authority", Value::Bool(true)),
            ("programs", Value::String("00".into())),
            ("schema", Value::Null),
        ] {
            let candidate = changed(record(SPARTAN_SCHEMA), member, Some(value));
            assert!(financial(&candidate, &input).is_err(), "{member}");
        }
    }

    #[test]
    fn spartan_pins_are_bounded_nonzero_and_canonical() {
        let input = offered(PACK, INVENTORY, ROOT);
        for (pin, bound) in [
            ("verifierPack", SPARTAN_PACK_MAX),
            ("producerInventory", SPARTAN_METADATA_MAX),
            ("transport", SPARTAN_METADATA_MAX),
        ] {
            for (member, value) in [
                ("bytes", Value::from(0u64)),
                ("bytes", Value::from(bound as u64 + 1)),
                ("sha256", Value::String("0".repeat(64))),
                ("sha256", Value::String("A".repeat(64))),
                ("stored", Value::from(1u64)),
            ] {
                let pinned = changed(record(SPARTAN_SCHEMA)[pin].clone(), member, Some(value));
                let candidate = changed(record(SPARTAN_SCHEMA), pin, Some(pinned));
                assert!(financial(&candidate, &input).is_err(), "{pin}.{member}");
            }
        }
        for pin in ["producerCatalogDigest", "assetDigest", "manifestDigest"] {
            for value in [
                Value::String("0".repeat(64)),
                Value::String("A".repeat(64)),
                Value::String("ab".repeat(31)),
                Value::Null,
            ] {
                let candidate = changed(record(SPARTAN_SCHEMA), pin, Some(value));
                assert!(financial(&candidate, &input).is_err(), "{pin}");
            }
        }
    }

    #[test]
    fn spartan_profile_binds_the_complete_offered_metadata_and_refuses_downgrade() {
        let selected = record(SPARTAN_SCHEMA);
        for (pack, inventory, root) in [
            (&b"other pack"[..], INVENTORY, ROOT),
            (PACK, &b"other inventory"[..], ROOT),
            (INVENTORY, PACK, ROOT),
            (&b""[..], INVENTORY, ROOT),
            (PACK, &b""[..], ROOT),
            (PACK, INVENTORY, &b""[..]),
            (&b""[..], &b""[..], &b""[..]),
        ] {
            assert!(financial(&selected, &offered(pack, inventory, root)).is_err());
        }
        // Signed absence is complete absence; it never becomes either profile.
        let (graph, profile) = financial(&Value::Null, &offered(b"", b"", b"")).unwrap();
        assert!(graph.is_none() && profile.is_none());
        assert!(financial(&Value::Null, &offered(PACK, INVENTORY, ROOT)).is_err());
    }
}
