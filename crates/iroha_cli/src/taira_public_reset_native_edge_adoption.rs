//! Explicit first-release ownership transition for retained opaque incident journals.
//!
//! No old journal is decoded. The independently provisioned native guard admits
//! the owner's signature over exact public references before a source-bound
//! helper may archive them and adopt the unchanged rendered include.

use super::*;
use iroha_crypto::{
    Algorithm, KeyPair, PublicKey, Signature, ed25519_parse_signature,
    verify_signature_for_admission,
};

#[path = "taira_public_reset_native_edge_owner_prepare.rs"]
mod prepare;
pub(in crate::taira_public_reset) use prepare::{
    PrepareNativeEdgeOwner, prepare as prepare_native_edge_owner,
};

const REQUEST_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-request.v1";
const PLAN_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-plan.v1";
const AUTHORIZATION_SCHEMA: &str =
    "iroha.taira.public-reset.native-owner-adoption-authorization.v1";
const CLAIMS_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-claims.v1";
const PROGRESS_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-progress.v1";
const LEASE_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-lease.v1";
const ADMISSION_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-admission.v1";
const RECEIPT_SCHEMA: &str = "iroha.taira.public-reset.native-owner-adoption-receipt.v1";
const DOMAIN: &[u8] = b"iroha:taira:public-reset:native-owner-adoption:v1\0";
const MAX_REQUEST: u64 = 128 * 1024;
const MAX_JOURNALS: usize = 32;

#[derive(clap::Args, Debug)]
pub(crate) struct AuthorizeNativeEdgeOwner {
    /// Closed public request whose exact bytes the independent owner approves.
    #[arg(long, value_name = "PATH")]
    request: PathBuf,
    /// SHA-256 of the retained request, supplied independently of its contents.
    #[arg(long, value_name = "SHA256")]
    expected_request_sha256: String,
    /// Independently selected Ed25519 reset-owner public key.
    #[arg(long, value_name = "PATH")]
    trusted_public_key: PathBuf,
    /// Inherited private key descriptor; key material never enters arguments or output.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    signing_key_fd: u32,
    /// Fresh authorization file in existing private custody; never replaced.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(crate) struct AdoptNativeEdgeOwner {
    /// Exact independently approved public request.
    #[arg(long, value_name = "PATH")]
    request: PathBuf,
    /// Independent pin of the retained request bytes.
    #[arg(long, value_name = "SHA256")]
    expected_request_sha256: String,
    /// Required signed adoption authorization, distinct from reset execution authorization.
    #[arg(long, value_name = "PATH")]
    authorization: PathBuf,
    /// Public owner key whose raw digest the fixed native guard independently pins.
    #[arg(long, value_name = "PATH")]
    trusted_public_key: PathBuf,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionRequestV1 {
    schema: String,
    hosts: host_pair::ResetHostPairV1,
    operation_id: String,
    authorization_nonce: String,
    helper_source_closure_sha256: String,
    adoption_plan: NativePublicFileV1,
    not_before_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionPlanV1 {
    schema: String,
    nginx: json::Value,
    publication: NativePublicFileV1,
    opaque_journals: Vec<NativePublicFileV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionClaimsV1 {
    schema: String,
    request_sha256: String,
    operation_id: String,
    authorization_nonce: String,
    host_pair_sha256: String,
    helper_source_closure_sha256: String,
    not_before_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionAuthorizationV1 {
    schema: String,
    claims: AdoptionClaimsV1,
    signature_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionLeaseV1 {
    schema: String,
    operation_id: String,
    request_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    host_pair_sha256: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionProgressV1 {
    schema: String,
    operation_id: String,
    request_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    host_pair_sha256: String,
    host_identity_sha256: String,
    custody_root: String,
    helper_source_closure_sha256: String,
    status: String,
    #[norito(required)]
    receipt_sha256: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionAdmissionV1 {
    schema: String,
    operation_id: String,
    request_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    host_pair_sha256: String,
    host_identity_sha256: String,
    custody_root: String,
    helper_source_closure_sha256: String,
    parent: protocol::NativeParentV1,
    host_lock: protocol::NativeRetainedLockV1,
    lock: protocol::NativeRetainedLockV1,
    guard: protocol::RetainedPublicRefV1,
    request: protocol::RetainedPublicRefV1,
    authorization: protocol::RetainedPublicRefV1,
    trusted_key: protocol::RetainedPublicRefV1,
    progress: protocol::RetainedPublicRefV1,
    plan: protocol::RetainedPublicRefV1,
    source_plan: protocol::RetainedPublicRefV1,
    lease: protocol::RetainedPublicRefV1,
    publication: protocol::RetainedPublicRefV1,
    opaque_journals: Vec<protocol::RetainedPublicRefV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AdoptionReceiptV1 {
    schema: String,
    operation_id: String,
    request_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    host_pair_sha256: String,
    custody_root: String,
    helper_source_closure_sha256: String,
    progress_before_sha256: String,
    status: String,
    archived_journals: Vec<NativePublicFileV1>,
    #[norito(required)]
    owned_publication: Option<host_pair::NativeOwnedPublicationV1>,
    #[norito(required)]
    error_code: Option<String>,
}

#[derive(Debug, JsonSerialize)]
struct AdoptionAuthorizationExportV1 {
    schema: String,
    request_sha256: String,
    authorization: NativePublicFileV1,
}

fn claims(request: &AdoptionRequestV1, request_sha256: &str) -> Result<AdoptionClaimsV1> {
    request.hosts.validate()?;
    validate_lower_hex("native adoption request", request_sha256, 64)?;
    validate_lower_hex("native adoption operation", &request.operation_id, 32)?;
    validate_lower_hex("native adoption nonce", &request.authorization_nonce, 32)?;
    request
        .adoption_plan
        .validate_public(request.hosts.native_edge.owner_uid)?;
    if request.schema != REQUEST_SCHEMA
        || request.helper_source_closure_sha256 != host_pair::helper_source_closure_sha256()
        || request.adoption_plan.file.identity.size > MAX_PLAN as u64
        || request.adoption_plan.file.identity.mode != 0o600
        || request.expires_at_unix_ms <= request.not_before_unix_ms
        || request.expires_at_unix_ms - request.not_before_unix_ms
            > super::super::super::MAX_AUTHORIZATION_LIFETIME_MS
    {
        return Err(eyre!(
            "native adoption request has an invalid closed custody or time window"
        ));
    }
    Ok(AdoptionClaimsV1 {
        schema: CLAIMS_SCHEMA.into(),
        request_sha256: request_sha256.into(),
        operation_id: request.operation_id.clone(),
        authorization_nonce: request.authorization_nonce.clone(),
        host_pair_sha256: request.hosts.digest()?,
        helper_source_closure_sha256: request.helper_source_closure_sha256.clone(),
        not_before_unix_ms: request.not_before_unix_ms,
        expires_at_unix_ms: request.expires_at_unix_ms,
    })
}

fn message(claims: &AdoptionClaimsV1) -> Result<Vec<u8>> {
    let body = json::to_json(claims)?;
    if body.len() > 16 * 1024 {
        return Err(eyre!(
            "native adoption signature claims exceed their finite bound"
        ));
    }
    Ok([DOMAIN, body.as_bytes()].concat())
}

fn trusted_key(bytes: &[u8]) -> Result<(TrustedKeyV1, PublicKey)> {
    let trusted: TrustedKeyV1 = json::from_slice(bytes)?;
    let key = PublicKey::from_str(&trusted.public_key)?;
    if trusted.schema != super::super::super::TRUSTED_KEY_SCHEMA_V1
        || trusted.algorithm != "ed25519"
        || key.try_algorithm()? != Algorithm::Ed25519
    {
        return Err(eyre!(
            "native adoption requires the independent Ed25519 owner"
        ));
    }
    Ok((trusted, key))
}

fn verify_authorization(
    authorization: &AdoptionAuthorizationV1,
    expected: &AdoptionClaimsV1,
    key: &PublicKey,
) -> Result<()> {
    if authorization.schema != AUTHORIZATION_SCHEMA || &authorization.claims != expected {
        return Err(eyre!(
            "native adoption authorization does not bind the exact request"
        ));
    }
    validate_lower_hex(
        "native adoption signature",
        &authorization.signature_hex,
        128,
    )?;
    let signature = ed25519_parse_signature(&hex::decode(&authorization.signature_hex)?)?;
    verify_signature_for_admission(&signature, key, &message(expected)?)
        .wrap_err("native adoption owner authorization failed")
}

fn read_request(path: &Path, expected: &str) -> Result<(PublicPin, AdoptionRequestV1)> {
    validate_lower_hex("native adoption independent request pin", expected, 64)?;
    let mut pin = PublicPin::open(path, MAX_REQUEST, true)?;
    if pin.reference.sha256 != expected {
        return Err(eyre!(
            "native adoption request changed from its independent pin"
        ));
    }
    let request = json::from_slice(&pin.bytes(MAX_REQUEST as usize)?)?;
    Ok((pin, request))
}

pub(crate) fn authorize<W: Write>(args: &AuthorizeNativeEdgeOwner, output: &mut W) -> Result<()> {
    let (request_pin, request) = read_request(&args.request, &args.expected_request_sha256)?;
    let expected = claims(&request, &args.expected_request_sha256)?;
    let mut trusted = PublicPin::open(&args.trusted_public_key, 16 * 1024, true)?;
    let (_, key) = trusted_key(&trusted.bytes(16 * 1024)?)?;
    let pair: KeyPair =
        super::super::super::inputs::inherited_signing_key(args.signing_key_fd, &key)?;
    let signature = Signature::try_new(pair.private_key(), &message(&expected)?)
        .map_err(|_| eyre!("native adoption owner signing failed"))?;
    let envelope = AdoptionAuthorizationV1 {
        schema: AUTHORIZATION_SCHEMA.into(),
        claims: expected,
        signature_hex: hex::encode(signature.payload()),
    };
    let body = json::to_json(&envelope)?;
    let parent = PrivateDirectory::open(
        args.output
            .parent()
            .ok_or_else(|| eyre!("native adoption authorization has no parent"))?,
    )?;
    let name = args
        .output
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| eyre!("native adoption authorization basename is invalid"))?;
    request_pin.revalidate()?;
    trusted.revalidate()?;
    parent.write_atomic(name, body.as_bytes(), PublishMode::CreateNew)?;
    parent.sync()?;
    let publication = PublicPin::open(&args.output, 16 * 1024, true)?;
    if publication.reference.sha256 != sha256_hex(body.as_bytes()) {
        return Err(eyre!(
            "native adoption authorization changed during publication"
        ));
    }
    let report = AdoptionAuthorizationExportV1 {
        schema: "iroha.taira.public-reset.native-owner-adoption-authorization-export.v1".into(),
        request_sha256: args.expected_request_sha256.clone(),
        authorization: publication.reference.clone(),
    };
    writeln!(output, "{}", json::to_json(&report)?)?;
    publication.revalidate()?;
    Ok(())
}

fn validate_plan(plan: &AdoptionPlanV1, request: &AdoptionRequestV1) -> Result<()> {
    let host = &request.hosts.native_edge;
    let nginx = &plan.nginx;
    let directory = nginx
        .get("native")
        .and_then(|value| value.get("directory"))
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native adoption plan lacks its publisher directory"))?;
    let destination = nginx
        .get("destination")
        .and_then(|value| value.get("directory"))
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native adoption plan lacks its include directory"))?;
    let basename = nginx
        .get("destination")
        .and_then(|value| value.get("basename"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native adoption destination name missing"))?;
    if plan.schema != PLAN_SCHEMA
        || plan_operation(nginx)? != request.operation_id
        || nginx.get("publication") != Some(&norito::json!({"kind":"create"}))
        || nginx.get("provider").and_then(json::Value::as_str) != Some("macstadium-dublin")
        || nginx.get("host_kind").and_then(json::Value::as_str) != Some("macos")
        || nginx
            .get("native")
            .and_then(|value| value.get("owner_uid"))
            .and_then(json::Value::as_u64)
            != Some(u64::from(host.owner_uid))
        || Path::new(&plan.publication.file.path) != Path::new(destination).join(basename)
        || plan.opaque_journals.is_empty()
        || plan.opaque_journals.len() > MAX_JOURNALS
    {
        return Err(eyre!(
            "native adoption plan does not bind one unchanged owned public include"
        ));
    }
    plan.publication.validate_public(host.owner_uid)?;
    if plan.publication.file.identity.mode != 0o600
        || plan.publication.file.identity.size > MAX_PLAN as u64
        || nginx
            .get("candidate")
            .and_then(|value| value.get("sha256"))
            .and_then(json::Value::as_str)
            != Some(plan.publication.sha256.as_str())
    {
        return Err(eyre!(
            "native adoption include differs from its exact public candidate"
        ));
    }
    let mut paths = BTreeSet::new();
    for journal in &plan.opaque_journals {
        journal.validate_public(host.owner_uid)?;
        let path = Path::new(&journal.file.path);
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| eyre!("native opaque journal basename missing"))?;
        let operation = name
            .strip_prefix(".taira-native-nginx-apply-")
            .and_then(|value| value.strip_suffix(".receipt.ndjson"))
            .ok_or_else(|| eyre!("native adoption cannot archive another owner namespace"))?;
        validate_lower_hex("opaque native journal operation", operation, 32)?;
        if path.parent() != Some(Path::new(directory))
            || operation == request.operation_id
            || !paths.insert(&journal.file.path)
            || journal.file.identity.mode != 0o600
            || journal.file.identity.size > MAX_PLAN as u64
        {
            return Err(eyre!(
                "native adoption opaque journal pins are not a closed distinct set"
            ));
        }
    }
    Ok(())
}

fn lock_reference(
    root: &PrivateDirectory,
    name: &str,
    file: &File,
) -> Result<protocol::NativeRetainedLockV1> {
    let identity = protocol::native_file_identity(&file.metadata()?)?;
    if identity.mode != 0o600
        || identity.links != 1
        || identity.size != 0
        || identity
            != protocol::native_file_identity(&fs::symlink_metadata(root.path().join(name))?)?
    {
        return Err(eyre!("native adoption held lock changed its exact custody"));
    }
    Ok(protocol::NativeRetainedLockV1 {
        fd: u32::try_from(file.as_raw_fd())?,
        file: NativeObservedFileV1 {
            path: root.path().join(name).to_string_lossy().into_owned(),
            identity,
        },
    })
}

pub(crate) fn adopt<W: Write>(args: &AdoptNativeEdgeOwner, output: &mut W) -> Result<()> {
    let (_, _, _, native_home) = native_account()?;
    let fixed_custody = Path::new(&native_home).join(".local/share/iroha/taira/public-reset-v1");
    if std::env::current_exe()? != fixed_custody.join("dispatcher/iroha") {
        return Err(eyre!(
            "native adoption requires its independently installed custodian"
        ));
    }
    let mut guard_pin = PublicPin::open(
        &fixed_custody.join("taira-edge/guard.json"),
        16 * 1024,
        true,
    )?;
    let (mut request_pin, request) = read_request(&args.request, &args.expected_request_sha256)?;
    let expected = claims(&request, &args.expected_request_sha256)?;
    let host = &request.hosts.native_edge;
    // The OS account and fixed executable select this guard before any selected
    // authorization, plan or journal path can supply authority.
    if canonical_guard_path(host)? != Path::new(&guard_pin.reference.file.path) {
        return Err(eyre!(
            "native adoption request selected another OS custodian"
        ));
    }
    let guard: HostGuardV1 = json::from_slice(&guard_pin.bytes(16 * 1024)?)?;
    let mut trusted_pin = PublicPin::open(&args.trusted_public_key, 16 * 1024, true)?;
    let (_, key) = trusted_key(&trusted_pin.bytes(16 * 1024)?)?;
    if guard_pin.reference.sha256 != host.guard_sha256
        || guard.schema != HOST_GUARD_SCHEMA_V1
        || guard.host_slug != "taira-edge"
        || guard.trusted_key_sha256 != trusted_pin.reference.sha256
        || guard.dispatcher_path != host.dispatcher_path
        || guard.dispatcher_sha256 != host.dispatcher_sha256
        || guard.service_root != format!("{}/.local/share/iroha/taira/edge", host.owner_home)
        || guard.state_root != format!("{}/.local/share/iroha/taira/edge/state", host.owner_home)
        || guard.upload_parent != upload_parent(&guard.service_root)
    {
        return Err(eyre!(
            "native adoption differs from the independently provisioned guard"
        ));
    }
    verify_dispatcher_identity(&guard, host)?;
    let mut authorization_pin = PublicPin::open(&args.authorization, 16 * 1024, true)?;
    let raw_authorization = authorization_pin.bytes(16 * 1024)?;
    let authorization: AdoptionAuthorizationV1 = json::from_slice(&raw_authorization)?;
    if json::to_json(&authorization)?.as_bytes() != raw_authorization {
        return Err(eyre!(
            "native adoption authorization must use its sole canonical wire"
        ));
    }
    verify_authorization(&authorization, &expected, &key)?;
    let authorization_sha256 = authorization_pin.reference.sha256.clone();
    let custody = PrivateDirectory::open(&host.custody_root)?;
    let edge = custody.ensure_child("taira-edge")?;
    let host_lock = edge.open_lock("host-operation.lock")?;
    host_lock
        .try_lock()
        .wrap_err("another native operation owns this host")?;
    require_path_absent(
        &edge.path().join("active-lease.json"),
        "unresolved native reset lease",
    )?;
    let operations = edge.ensure_child("operations")?;
    let operation = operations.ensure_child(&authorization_sha256)?;
    let lock = operation.open_lock("operation.lock")?;
    lock.try_lock()
        .wrap_err("native adoption operation is already running")?;
    let lease = AdoptionLeaseV1 {
        schema: LEASE_SCHEMA.into(),
        operation_id: request.operation_id.clone(),
        request_sha256: args.expected_request_sha256.clone(),
        authorization_sha256: authorization_sha256.clone(),
        authorization_nonce: request.authorization_nonce.clone(),
        host_pair_sha256: expected.host_pair_sha256.clone(),
    };
    let lease_raw = json::to_json(&lease)?.into_bytes();
    let existing = operation.read("progress.json", MAX_REQUEST as usize);
    let resumed = existing.is_ok();
    let now = now_unix_ms()?;
    if !resumed && (now < request.not_before_unix_ms || now > request.expires_at_unix_ms) {
        return Err(eyre!(
            "native adoption authorization is outside its admission window"
        ));
    }
    let mut progress = match existing {
        Ok(body) => {
            let progress: AdoptionProgressV1 = json::from_slice(&body)?;
            if json::to_json(&progress)?.as_bytes() != body.as_slice()
                || progress.schema != PROGRESS_SCHEMA
                || progress.operation_id != request.operation_id
                || progress.request_sha256 != args.expected_request_sha256
                || progress.authorization_sha256 != authorization_sha256
                || progress.authorization_nonce != request.authorization_nonce
                || progress.host_pair_sha256 != expected.host_pair_sha256
                || progress.host_identity_sha256 != host.endpoint.host_identity_sha256
                || progress.custody_root != host.custody_root
                || progress.helper_source_closure_sha256 != request.helper_source_closure_sha256
                || !matches!(
                    progress.status.as_str(),
                    "admitted" | "adoption_requested" | "adopted_unqualified" | "recovery_pending"
                )
                || matches!(progress.status.as_str(), "admitted" | "adoption_requested")
                    != progress.receipt_sha256.is_none()
            {
                return Err(eyre!(
                    "native adoption progress is not this exact durable authorization"
                ));
            }
            progress
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AdoptionProgressV1 {
            schema: PROGRESS_SCHEMA.into(),
            operation_id: request.operation_id.clone(),
            request_sha256: args.expected_request_sha256.clone(),
            authorization_sha256: authorization_sha256.clone(),
            authorization_nonce: request.authorization_nonce.clone(),
            host_pair_sha256: expected.host_pair_sha256.clone(),
            host_identity_sha256: host.endpoint.host_identity_sha256.clone(),
            custody_root: host.custody_root.clone(),
            helper_source_closure_sha256: request.helper_source_closure_sha256.clone(),
            status: "admitted".into(),
            receipt_sha256: None,
        },
        Err(error) => return Err(error.into()),
    };
    retain_exact(
        &operation,
        "request.json",
        &request_pin.bytes(MAX_REQUEST as usize)?,
    )?;
    retain_exact(&operation, "authorization.json", &raw_authorization)?;
    retain_exact(
        &operation,
        "trusted-key.json",
        &trusted_pin.bytes(16 * 1024)?,
    )?;
    let mut plan_pin = PublicPin::expected(&request.adoption_plan, true)?;
    let plan_raw = plan_pin.bytes(MAX_PLAN)?;
    let plan: AdoptionPlanV1 = json::from_slice(&plan_raw)?;
    validate_plan(&plan, &request)?;
    let opaque_directory = ReaderDirectory::open(publisher_directory(&plan)?)?;
    retain_exact(&operation, "adoption-plan.json", &plan_raw)?;
    if progress.status == "adopted_unqualified" {
        let receipt_raw = operation.read("receipt.json", 64 * 1024)?;
        if progress.receipt_sha256.as_ref() != Some(&sha256_hex(&receipt_raw)) {
            return Err(eyre!("native adoption terminal receipt changed"));
        }
        let receipt: AdoptionReceiptV1 = json::from_slice(&receipt_raw)?;
        validate_receipt(
            &receipt,
            &request,
            &plan,
            &args.expected_request_sha256,
            &authorization_sha256,
            None,
        )?;
        let owned = receipt
            .owned_publication
            .as_ref()
            .ok_or_else(|| eyre!("native adopted owner missing"))?;
        PublicPin::expected_child(&opaque_directory, &owned.journal, true)?.revalidate()?;
        PublicPin::expected(&owned.publication, false)?.revalidate()?;
        for (ordinal, reference) in receipt.archived_journals.iter().enumerate() {
            if Path::new(&reference.file.path) != archive_path(&plan, &request, ordinal)?
                || !same_incident_inode(reference, &plan.opaque_journals[ordinal])
            {
                return Err(eyre!(
                    "native adoption terminal archive changed its exact incident binding"
                ));
            }
            PublicPin::expected_child(&opaque_directory, reference, true)?.revalidate()?;
        }
        release_adoption_lease(&edge, &lease_raw)?;
        output.write_all(&receipt_raw)?;
        return Ok(());
    }
    if !resumed {
        operation.write_atomic(
            "progress.json",
            json::to_json(&progress)?.as_bytes(),
            PublishMode::CreateNew,
        )?;
        operation.sync()?;
    }
    retain_exact(&edge, "active-adoption.json", &lease_raw)?;
    progress.status = "adoption_requested".into();
    progress.receipt_sha256 = None;
    operation.write_atomic(
        "progress.json",
        json::to_json(&progress)?.as_bytes(),
        PublishMode::Replace,
    )?;
    operation.sync()?;
    let capsule = NativeCapsule::admit_host(host)?;
    let mut pins = vec![
        PublicPin::open(Path::new(&host.dispatcher_path), 512 * 1024 * 1024, false)?,
        guard_pin,
        PublicPin::private_child(&operation, "request.json", MAX_REQUEST)?,
        PublicPin::private_child(&operation, "authorization.json", 16 * 1024)?,
        PublicPin::private_child(&operation, "trusted-key.json", 16 * 1024)?,
        PublicPin::private_child(&operation, "progress.json", MAX_REQUEST)?,
        PublicPin::private_child(&operation, "adoption-plan.json", MAX_PLAN as u64)?,
        PublicPin::private_child(&edge, "active-adoption.json", MAX_REQUEST)?,
        PublicPin::expected(&plan.publication, false)?,
    ];
    // During an interrupted archive the source-bound helper joins these actual
    // descriptors to independently durable archive intent and the signed raw
    // original references. Opening a recorded inode does not establish a phase.
    for reference in &plan.opaque_journals {
        pins.push(open_opaque_incident(
            reference,
            &plan,
            &request,
            &opaque_directory,
        )?);
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    let packet = adoption_packet(
        &request,
        &authorization_sha256,
        &operation,
        &edge,
        &lock,
        &host_lock,
        &pins,
        &plan_pin,
        deadline,
    )?;
    let mut inherited = vec![&lock, &host_lock];
    inherited.push(plan_pin.retained.file());
    inherited.extend(pins.iter().map(|pin| pin.retained.file()));
    let lock_before = packet.lock.clone();
    let host_lock_before = packet.host_lock.clone();
    let body = json::to_json(&packet)?;
    let receipt_raw = capsule.run(&operation, "adopt", body.as_bytes(), &inherited, deadline)?;
    if lock_reference(&operation, "operation.lock", &lock)? != lock_before
        || lock_reference(&edge, "host-operation.lock", &host_lock)? != host_lock_before
    {
        return Err(eyre!(
            "native adoption held host custody changed around its child"
        ));
    }
    for pin in &pins[..9] {
        pin.revalidate()?;
    }
    plan_pin.revalidate()?;
    let receipt: AdoptionReceiptV1 = json::from_slice(&receipt_raw)?;
    validate_receipt(
        &receipt,
        &request,
        &plan,
        &args.expected_request_sha256,
        &authorization_sha256,
        Some(&pins[5].reference.sha256),
    )?;
    admit_archives(
        &mut pins[9..],
        &receipt.archived_journals,
        &plan,
        &request,
        &opaque_directory,
    )?;
    if receipt.status != "adopted_unqualified" {
        retain_exact(
            &operation,
            &format!("pending-{}.json", sha256_hex(&receipt_raw)),
            &receipt_raw,
        )?;
        progress.status = "recovery_pending".into();
        progress.receipt_sha256 = Some(sha256_hex(&receipt_raw));
        operation.write_atomic(
            "progress.json",
            json::to_json(&progress)?.as_bytes(),
            PublishMode::Replace,
        )?;
        operation.sync()?;
        return Err(eyre!("native ownership adoption remains recovery pending"));
    }
    let owned = receipt
        .owned_publication
        .as_ref()
        .ok_or_else(|| eyre!("native adoption owner missing"))?;
    PublicPin::expected_child(&opaque_directory, &owned.journal, true)?.revalidate()?;
    PublicPin::expected(&owned.publication, false)?.revalidate()?;
    retain_exact(&operation, "receipt.json", &receipt_raw)?;
    progress.status = "adopted_unqualified".into();
    progress.receipt_sha256 = Some(sha256_hex(&receipt_raw));
    operation.write_atomic(
        "progress.json",
        json::to_json(&progress)?.as_bytes(),
        PublishMode::Replace,
    )?;
    operation.sync()?;
    pins[7].revalidate()?;
    release_adoption_lease(&edge, &lease_raw)?;
    output.write_all(&receipt_raw)?;
    Ok(())
}

fn archive_path(
    plan: &AdoptionPlanV1,
    request: &AdoptionRequestV1,
    ordinal: usize,
) -> Result<PathBuf> {
    Ok(publisher_directory(plan)?.join(format!(
        ".taira-native-nginx-adoption-{}-opaque-{}.ndjson",
        request.operation_id, ordinal
    )))
}

fn publisher_directory(plan: &AdoptionPlanV1) -> Result<&Path> {
    let directory = plan
        .nginx
        .get("native")
        .and_then(|value| value.get("directory"))
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native adoption publisher directory missing"))?;
    Ok(Path::new(directory))
}

fn same_incident_inode(actual: &NativePublicFileV1, original: &NativePublicFileV1) -> bool {
    let mut identity = actual.file.identity.clone();
    identity.ctime_ns = original.file.identity.ctime_ns;
    identity == original.file.identity && actual.sha256 == original.sha256
}

fn open_opaque_incident(
    reference: &NativePublicFileV1,
    plan: &AdoptionPlanV1,
    request: &AdoptionRequestV1,
    directory: &ReaderDirectory,
) -> Result<PublicPin> {
    match PublicPin::expected_child(directory, reference, true) {
        Ok(pin) => Ok(pin),
        Err(original_error) => {
            let ordinal = plan
                .opaque_journals
                .iter()
                .position(|item| item == reference)
                .ok_or_else(|| eyre!("native opaque incident is outside its signed ordered set"))?;
            require_path_absent(
                Path::new(&reference.file.path),
                "archived opaque journal original name",
            )?;
            let archive = archive_path(plan, request, ordinal)?;
            let pin = PublicPin::reader_child(
                directory,
                archive
                    .file_name()
                    .ok_or_else(|| eyre!("native opaque archive basename missing"))?,
                MAX_PLAN as u64,
                true,
            )
            .wrap_err_with(|| {
                format!("native opaque incident original admission failed: {original_error}")
            })?;
            if !same_incident_inode(&pin.reference, reference) {
                return Err(eyre!(
                    "native opaque incident archive differs from its signed original inode"
                ));
            }
            Ok(pin)
        }
    }
}

fn adoption_packet(
    request: &AdoptionRequestV1,
    authorization_sha256: &str,
    operation: &PrivateDirectory,
    edge: &PrivateDirectory,
    lock: &File,
    host_lock: &File,
    pins: &[PublicPin],
    source_plan: &PublicPin,
    deadline: Instant,
) -> Result<AdoptionAdmissionV1> {
    let native = &request.hosts.native_edge;
    let started = canonical_parent_start(&run_host_command(
        "/bin/ps",
        &["-p", &std::process::id().to_string(), "-o", "lstart="],
        deadline,
    )?)?;
    if pins[0].reference.sha256 != native.dispatcher_sha256 {
        return Err(eyre!("native adoption parent changed"));
    }
    Ok(AdoptionAdmissionV1 {
        schema: ADMISSION_SCHEMA.into(),
        operation_id: request.operation_id.clone(),
        request_sha256: pins[2].reference.sha256.clone(),
        authorization_sha256: authorization_sha256.into(),
        authorization_nonce: request.authorization_nonce.clone(),
        host_pair_sha256: request.hosts.digest()?,
        host_identity_sha256: native.endpoint.host_identity_sha256.clone(),
        custody_root: native.custody_root.clone(),
        helper_source_closure_sha256: request.helper_source_closure_sha256.clone(),
        parent: protocol::NativeParentV1 {
            pid: std::process::id(),
            uid: native.owner_uid,
            started,
            executable: pins[0].inherited()?,
        },
        host_lock: lock_reference(edge, "host-operation.lock", host_lock)?,
        lock: lock_reference(operation, "operation.lock", lock)?,
        guard: pins[1].inherited()?,
        request: pins[2].inherited()?,
        authorization: pins[3].inherited()?,
        trusted_key: pins[4].inherited()?,
        progress: pins[5].inherited()?,
        plan: pins[6].inherited()?,
        source_plan: source_plan.inherited()?,
        lease: pins[7].inherited()?,
        publication: pins[8].inherited()?,
        opaque_journals: pins[9..]
            .iter()
            .map(PublicPin::inherited)
            .collect::<Result<_>>()?,
    })
}

fn validate_receipt(
    receipt: &AdoptionReceiptV1,
    request: &AdoptionRequestV1,
    plan: &AdoptionPlanV1,
    request_sha256: &str,
    authorization_sha256: &str,
    progress_before: Option<&str>,
) -> Result<()> {
    if receipt.schema != RECEIPT_SCHEMA
        || receipt.operation_id != request.operation_id
        || receipt.request_sha256 != request_sha256
        || receipt.authorization_sha256 != authorization_sha256
        || receipt.authorization_nonce != request.authorization_nonce
        || receipt.host_pair_sha256 != request.hosts.digest()?
        || receipt.custody_root != request.hosts.native_edge.custody_root
        || receipt.helper_source_closure_sha256 != request.helper_source_closure_sha256
        || progress_before.is_some_and(|before| receipt.progress_before_sha256 != before)
        || !matches!(
            receipt.status.as_str(),
            "adopted_unqualified" | "recovery_pending"
        )
        || (receipt.status == "adopted_unqualified") != receipt.error_code.is_none()
        || receipt.archived_journals.len() > plan.opaque_journals.len()
    {
        return Err(eyre!(
            "native adoption receipt does not join its exact retained authority"
        ));
    }
    if receipt.status == "adopted_unqualified" {
        let owned = receipt
            .owned_publication
            .as_ref()
            .ok_or_else(|| eyre!("adopted publication missing"))?;
        if owned.operation_id != request.operation_id
            || owned.publication != plan.publication
            || Path::new(&owned.journal.file.path)
                != protocol::publisher_journal_path(&plan.nginx, &request.operation_id)?
            || receipt.archived_journals.len() != plan.opaque_journals.len()
        {
            return Err(eyre!(
                "native adopted publication changed the current public include or archive set"
            ));
        }
        owned
            .journal
            .validate_public(request.hosts.native_edge.owner_uid)?;
        if owned.journal.file.identity.size > MAX_PLAN as u64
            || owned.journal.file.identity.mode != 0o600
        {
            return Err(eyre!(
                "native adopted journal exceeds its exact owner bound"
            ));
        }
    } else if receipt.owned_publication.is_some() {
        return Err(eyre!("pending native adoption cannot qualify an owner"));
    }
    Ok(())
}

fn release_adoption_lease(edge: &PrivateDirectory, expected: &[u8]) -> Result<()> {
    let path = edge.path().join("active-adoption.json");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            edge.sync()?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let mut lease = match PublicPin::open(&path, MAX_REQUEST, true) {
        Ok(pin) => pin,
        Err(error) => return Err(error),
    };
    if lease.bytes(MAX_REQUEST as usize)? != expected {
        return Err(eyre!(
            "native adoption cannot release another durable host lease"
        ));
    }
    edge.revalidate()?;
    let directory = File::from(rustix::fs::open(
        edge.path(),
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    if protocol::native_file_identity(&directory.metadata()?)?
        != protocol::native_file_identity(&fs::symlink_metadata(edge.path())?)?
        || iroha_fs::FileIdentity::of(&directory)? != edge.identity()?
    {
        return Err(eyre!("native adoption lease parent changed"));
    }
    lease.revalidate()?;
    rustix::fs::unlinkat(
        &directory,
        "active-adoption.json",
        rustix::fs::AtFlags::empty(),
    )?;
    directory.sync_all()?;
    edge.revalidate()?;
    Ok(())
}

fn admit_archives(
    originals: &mut [PublicPin],
    archives: &[NativePublicFileV1],
    plan: &AdoptionPlanV1,
    request: &AdoptionRequestV1,
    directory: &ReaderDirectory,
) -> Result<()> {
    for (ordinal, archive) in archives.iter().enumerate() {
        let original = &plan.opaque_journals[ordinal];
        if Path::new(&archive.file.path) != archive_path(plan, request, ordinal)?
            || !same_incident_inode(archive, original)
        {
            return Err(eyre!("native adoption archived a different incident inode"));
        }
        let retained = PublicPin::expected_child(directory, archive, true)?;
        let prior = &mut originals[ordinal];
        let actual = protocol::native_file_identity(&prior.retained.file().metadata()?)?;
        if actual != archive.file.identity {
            return Err(eyre!(
                "native adoption archive detached from its original retained descriptor"
            ));
        }
        prior.retained.file_mut().rewind()?;
        if hash_reader(
            &mut prior
                .retained
                .file_mut()
                .take(original.file.identity.size + 1),
        )? != original.sha256
        {
            return Err(eyre!("native adoption changed opaque incident bytes"));
        }
        require_path_absent(
            Path::new(&original.file.path),
            "adopted opaque journal original path",
        )?;
        retained.revalidate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(path: String, hosts: &host_pair::ResetHostPairV1) -> NativePublicFileV1 {
        NativePublicFileV1 {
            file: NativeObservedFileV1 {
                path,
                identity: host_pair::NativeFileIdentityV1 {
                    device: 1,
                    inode: 2,
                    uid: hosts.native_edge.owner_uid,
                    gid: hosts.native_edge.owner_gid,
                    mode: 0o600,
                    links: 1,
                    size: 64,
                    mtime_ns: 1,
                    ctime_ns: 1,
                },
            },
            sha256: "f".repeat(64),
        }
    }

    fn fixture() -> (AdoptionRequestV1, AdoptionPlanV1) {
        let hosts = host_pair::fixture_pair();
        let request = AdoptionRequestV1 {
            schema: REQUEST_SCHEMA.into(),
            adoption_plan: reference(
                format!("{}/adoption-plan.json", hosts.native_edge.custody_root),
                &hosts,
            ),
            hosts,
            operation_id: "a".repeat(32),
            authorization_nonce: "b".repeat(32),
            helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
            not_before_unix_ms: 1,
            expires_at_unix_ms: 1_000,
        };
        let publication = reference(
            "/opt/homebrew/etc/nginx/servers/taira-public.conf".into(),
            &request.hosts,
        );
        let opaque = reference(
            format!(
                "/opt/homebrew/etc/nginx/.taira-native-nginx-apply-{}.receipt.ndjson",
                "c".repeat(32)
            ),
            &request.hosts,
        );
        let plan = AdoptionPlanV1 {
            schema: PLAN_SCHEMA.into(),
            nginx: norito::json!({"operation_id":(&request.operation_id),"provider":"macstadium-dublin","host_kind":"macos", "publication":{"kind":"create"},"candidate":{"sha256":(&publication.sha256)},"native":{"owner_uid":(request.hosts.native_edge.owner_uid),"directory":{"path":"/opt/homebrew/etc/nginx"}},"destination":{"directory":{"path":"/opt/homebrew/etc/nginx/servers"},"basename":"taira-public.conf"}}),
            publication,
            opaque_journals: vec![opaque],
        };
        (request, plan)
    }

    #[test]
    fn adoption_owner_signature_binds_exact_raw_request_and_native_host_source() {
        let (request, _) = fixture();
        let original = claims(&request, &"d".repeat(64)).unwrap();
        let owner = KeyPair::from_seed(
            b"independent-native-adoption-owner".to_vec(),
            Algorithm::Ed25519,
        );
        let signature =
            Signature::try_new(owner.private_key(), &message(&original).unwrap()).unwrap();
        let envelope = AdoptionAuthorizationV1 {
            schema: AUTHORIZATION_SCHEMA.into(),
            claims: original.clone(),
            signature_hex: hex::encode(signature.payload()),
        };
        verify_authorization(&envelope, &original, owner.public_key()).unwrap();
        let other = KeyPair::from_seed(
            b"another-native-adoption-owner".to_vec(),
            Algorithm::Ed25519,
        );
        assert!(verify_authorization(&envelope, &original, other.public_key()).is_err());
        for changed in [
            AdoptionClaimsV1 {
                request_sha256: "e".repeat(64),
                ..original.clone()
            },
            AdoptionClaimsV1 {
                host_pair_sha256: "e".repeat(64),
                ..original.clone()
            },
            AdoptionClaimsV1 {
                helper_source_closure_sha256: "e".repeat(64),
                ..original.clone()
            },
            AdoptionClaimsV1 {
                authorization_nonce: "e".repeat(32),
                ..original.clone()
            },
        ] {
            assert!(verify_authorization(&envelope, &changed, owner.public_key()).is_err());
        }
        let raw = json::to_json(&envelope).unwrap();
        let mut object: json::Value = json::from_slice(raw.as_bytes()).unwrap();
        object
            .as_object_mut()
            .unwrap()
            .insert("legacy_authorization".into(), json::Value::Null);
        assert!(
            json::from_slice::<AdoptionAuthorizationV1>(json::to_json(&object).unwrap().as_bytes())
                .is_err()
        );
    }

    #[test]
    fn adoption_requires_one_closed_exact_include_and_ordered_opaque_set() {
        let (request, plan) = fixture();
        validate_plan(&plan, &request).unwrap();
        for change in 0..5 {
            let mut changed = plan.clone();
            match change {
                0 => changed.opaque_journals.clear(),
                1 => changed
                    .opaque_journals
                    .push(changed.opaque_journals[0].clone()),
                2 => changed.publication.file.path = "/opt/homebrew/etc/nginx/nginx.conf".into(),
                3 => changed.opaque_journals[0].file.path = "/private/foreign.ndjson".into(),
                4 => {
                    changed
                        .nginx
                        .as_object_mut()
                        .unwrap()
                        .insert("publication".into(), norito::json!({"kind":"reconcile"}));
                }
                _ => unreachable!(),
            }
            assert!(validate_plan(&changed, &request).is_err());
        }
        let mut expired_shape = request.clone();
        expired_shape.expires_at_unix_ms = expired_shape.not_before_unix_ms;
        assert!(claims(&expired_shape, &"d".repeat(64)).is_err());
        let mut missing: json::Value =
            json::from_slice(json::to_json(&request).unwrap().as_bytes()).unwrap();
        missing.as_object_mut().unwrap().remove("adoption_plan");
        assert!(
            json::from_slice::<AdoptionRequestV1>(json::to_json(&missing).unwrap().as_bytes())
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn opaque_incident_archive_keeps_exact_original_inode_bytes_without_decoding() {
        let directory = tempfile::Builder::new()
            .prefix(".native-opaque-adoption-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let (request, mut plan) = fixture();
        plan.nginx
            .get_mut("native")
            .unwrap()
            .get_mut("directory")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                "path".into(),
                json::Value::from(directory.path().to_string_lossy().into_owned()),
            );
        let old = directory.path().join(format!(
            ".taira-native-nginx-apply-{}.receipt.ndjson",
            "c".repeat(32)
        ));
        let opaque = b"retained incident is opaque, not a JSON journal\n";
        fs::write(&old, opaque).unwrap();
        fs::set_permissions(&old, fs::Permissions::from_mode(0o600)).unwrap();
        let original = PublicPin::open(&old, 128, true).unwrap();
        let reader = ReaderDirectory::open(directory.path()).unwrap();
        plan.opaque_journals = vec![original.reference.clone()];
        let archive = archive_path(&plan, &request, 0).unwrap();
        fs::rename(&old, &archive).unwrap();
        let pin = open_opaque_incident(&original.reference, &plan, &request, &reader).unwrap();
        assert!(same_incident_inode(&pin.reference, &original.reference));
        assert_eq!(fs::read(&archive).unwrap(), opaque);
        fs::write(&archive, b"foreign incident\n").unwrap();
        assert!(open_opaque_incident(&original.reference, &plan, &request, &reader).is_err());
        assert_eq!(fs::read(&archive).unwrap(), b"foreign incident\n");
    }

    #[test]
    fn actual_cli_adoption_commands_require_auth_pin_and_inherited_key() {
        use crate::Args;
        use clap::Parser as _;
        let authorize = vec![
            "iroha",
            "taira",
            "public-reset",
            "authorize-native-edge-owner",
            "--request",
            "request.json",
            "--expected-request-sha256",
            "pin",
            "--trusted-public-key",
            "owner.json",
            "--signing-key-fd",
            "3",
            "--output",
            "authorization.json",
        ];
        assert!(Args::try_parse_from(authorize.clone()).is_ok());
        for flag in [
            "--expected-request-sha256",
            "--trusted-public-key",
            "--signing-key-fd",
        ] {
            let mut missing = authorize.clone();
            let index = missing.iter().position(|arg| *arg == flag).unwrap();
            missing.drain(index..index + 2);
            assert!(Args::try_parse_from(missing).is_err());
        }
        let adopt = vec![
            "iroha",
            "taira",
            "public-reset",
            "adopt-native-edge-owner",
            "--request",
            "request.json",
            "--expected-request-sha256",
            "pin",
            "--trusted-public-key",
            "owner.json",
            "--authorization",
            "authorization.json",
        ];
        assert!(Args::try_parse_from(adopt.clone()).is_ok());
        let mut missing = adopt;
        let index = missing
            .iter()
            .position(|arg| *arg == "--authorization")
            .unwrap();
        missing.drain(index..index + 2);
        assert!(Args::try_parse_from(missing).is_err());
    }

    #[test]
    fn adoption_rejects_a_foreign_executable_before_selected_request_body() {
        let args = AdoptNativeEdgeOwner {
            request: PathBuf::from("/this-selected-input-must-not-be-opened/request.json"),
            expected_request_sha256: "a".repeat(64),
            authorization: PathBuf::from(
                "/this-selected-input-must-not-be-opened/authorization.json",
            ),
            trusted_public_key: PathBuf::from("/this-selected-input-must-not-be-opened/key.json"),
        };
        let error = adopt(&args, &mut Vec::new()).unwrap_err().to_string();
        #[cfg(target_os = "macos")]
        assert!(
            error.contains("independently installed custodian"),
            "{error}"
        );
        #[cfg(not(target_os = "macos"))]
        assert!(error.contains("Darwin OS account authority"), "{error}");
    }

    #[test]
    fn adoption_receipt_requires_explicit_nullable_owner_and_error_fields() {
        let (request, _) = fixture();
        let receipt = AdoptionReceiptV1 {
            schema: RECEIPT_SCHEMA.into(),
            operation_id: request.operation_id.clone(),
            request_sha256: "d".repeat(64),
            authorization_sha256: "e".repeat(64),
            authorization_nonce: request.authorization_nonce.clone(),
            host_pair_sha256: request.hosts.digest().unwrap(),
            custody_root: request.hosts.native_edge.custody_root.clone(),
            helper_source_closure_sha256: request.helper_source_closure_sha256.clone(),
            progress_before_sha256: "f".repeat(64),
            status: "recovery_pending".into(),
            archived_journals: Vec::new(),
            owned_publication: None,
            error_code: Some("owned_recovery_pending".into()),
        };
        let wire = json::to_json(&receipt).unwrap();
        json::from_slice::<AdoptionReceiptV1>(wire.as_bytes()).unwrap();
        for field in ["owned_publication", "error_code"] {
            let mut missing: json::Value = json::from_slice(wire.as_bytes()).unwrap();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                json::from_slice::<AdoptionReceiptV1>(json::to_json(&missing).unwrap().as_bytes())
                    .is_err()
            );
        }
    }
}
