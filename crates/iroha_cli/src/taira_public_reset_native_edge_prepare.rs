//! Native Mac predecessor capture and independently authorized Darwin candidate.
//!
//! The existing nginx owner admits its retained journal and publication. Rust
//! retains public inputs and metadata-only private-main custody, signs the native
//! records with inherited keys, and publishes all outputs as one fresh directory.

use super::*;
use host_pair::{
    NativeEdgeCandidateClaimsV1, NativeEdgeCaptureClaimsV1, NativeEdgeCompletionProvenanceV1,
    NativeFileIdentityV1, NativeObservedFileV1, NativeOwnedPublicationV1, NativePublicFileV1,
    ResetHostPairV1, SignedNativeEdgeCandidateV1, SignedNativeEdgeCaptureV1,
};
use iroha_crypto::KeyPair;
use iroha_fs::OwnerDirectory;

const REQUEST_SCHEMA: &str = "iroha.taira.public-reset.native-edge-prepare-request.v1";
const MAX_REQUEST: u64 = 128 * 1024;
const MAX_PUBLIC_PLAN: u64 = 1024 * 1024;

/// Prepare public native records without changing nginx, validators or ledger state.
#[derive(clap::Args, Debug)]
pub(super) struct PrepareNativeEdge {
    /// Closed public owner selection, source and actual native input references.
    #[arg(long, value_name = "PATH")]
    request: PathBuf,
    /// Independently admitted SHA-256 of the exact request bytes.
    #[arg(long, value_name = "SHA256")]
    expected_request_sha256: String,
    /// Inherited owner-private native custody key; never accepted in argv or output.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    native_signing_key_fd: u32,
    /// Inherited independently selected release-owner key.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    owner_signing_key_fd: u32,
    /// Fresh directory beneath native custody, published with all public records atomically.
    #[arg(long, value_name = "DIR")]
    output: PathBuf,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct TerminalAuthorityV1 {
    inventory: NativePublicFileV1,
    authorization: NativePublicFileV1,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PrepareRequestV1 {
    schema: String,
    hosts: ResetHostPairV1,
    retained_inventory_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    next_genesis_hash: String,
    incumbent: EdgeAdmittedReleaseV1,
    source_root: String,
    source_manifest: NativePublicFileV1,
    trusted_public_key: NativePublicFileV1,
    candidate_cli: NativePublicFileV1,
    /// Retained controller copy path; its receiver verifies the exact signed bytes.
    controller_cli_projection: String,
    current_nginx_request: NativePublicFileV1,
    new_nginx_apply_plan: NativePublicFileV1,
    forwarding_plan: NativePublicFileV1,
    forwarding_identity_receipt: NativePublicFileV1,
    forwarding_journal: NativePublicFileV1,
    completion: NativeEdgeCompletionProvenanceV1,
    #[norito(required)]
    terminal_authority: Option<TerminalAuthorityV1>,
}

/// Sole maintained owner output; private-main content has no representation here.
type OwnerObservationV1 = native_edge_protocol::NativeOwnerObservationV1;

struct RetainedInput {
    parent: OwnerDirectory,
    pin: PinnedInput,
    identity: NativeFileIdentityV1,
}

#[cfg(unix)]
fn native_identity(metadata: &fs::Metadata) -> Result<NativeFileIdentityV1> {
    native_edge_protocol::native_file_identity(metadata)
}

#[cfg(not(unix))]
fn native_identity(_: &fs::Metadata) -> Result<NativeFileIdentityV1> {
    Err(eyre!("native edge input custody requires Unix"))
}

impl RetainedInput {
    /// Metadata-only acquisition also retains every native ancestor; no body is read.
    fn metadata(expected: &NativeObservedFileV1, owner: u32) -> Result<Self> {
        expected.validate_metadata(owner)?;
        let path = Path::new(&expected.path);
        let parent = OwnerDirectory::open(
            path.parent()
                .ok_or_else(|| eyre!("native input has no parent"))?,
        )?;
        let (file, snapshot) = open_pinned_regular(path, "native captured input")?;
        let identity = native_identity(&file.metadata()?)?;
        if identity != expected.identity {
            return Err(eyre!(
                "native input differs from its independently selected identity"
            ));
        }
        let retained = Self {
            parent,
            pin: PinnedInput {
                path: path.into(),
                file,
                snapshot,
            },
            identity,
        };
        retained.revalidate()?;
        Ok(retained)
    }

    fn public(expected: &NativePublicFileV1, owner: u32, maximum: u64) -> Result<Self> {
        expected.validate_public(owner)?;
        if expected.file.identity.size > maximum {
            return Err(eyre!("native public input exceeds its role bound"));
        }
        let mut retained = Self::metadata(&expected.file, owner)?;
        if sha256_reader(&mut retained.pin.file, &retained.pin.path)? != expected.sha256 {
            return Err(eyre!(
                "native public input differs from its independently selected digest"
            ));
        }
        retained.revalidate()?;
        Ok(retained)
    }

    fn bytes(&self, maximum: u64) -> Result<Vec<u8>> {
        let mut file = self.pin.file.try_clone()?;
        file.rewind()?;
        let bytes = read_pinned_bytes(
            &self.pin.path,
            "native public input",
            file,
            &self.pin.snapshot,
            maximum,
        )?;
        self.revalidate()?;
        Ok(bytes)
    }

    fn revalidate(&self) -> Result<()> {
        self.parent.revalidate()?;
        revalidate_pinned(&self.pin, "native retained input")?;
        if native_identity(&self.pin.file.metadata()?)? != self.identity
            || native_identity(&fs::symlink_metadata(&self.pin.path)?)? != self.identity
        {
            return Err(eyre!("native input metadata changed while retained"));
        }
        Ok(())
    }
}

fn retain_public(
    refs: &mut Vec<RetainedInput>,
    reference: &NativePublicFileV1,
    owner: u32,
    maximum: u64,
) -> Result<Vec<u8>> {
    let input = RetainedInput::public(reference, owner, maximum)?;
    let bytes = input.bytes(maximum)?;
    refs.push(input);
    Ok(bytes)
}

fn read_request(args: &PrepareNativeEdge) -> Result<(PrepareRequestV1, PinnedInput)> {
    validate_lower_hex("native request pin", &args.expected_request_sha256, 64)?;
    let pin = pin_owner_private_file(&args.request, "native prepare request")?;
    let raw = read_pinned_bytes(
        &args.request,
        "native prepare request",
        pin.file.try_clone()?,
        &pin.snapshot,
        MAX_REQUEST,
    )?;
    if sha256_hex(&raw) != args.expected_request_sha256 {
        return Err(eyre!("native request changed from its independent pin"));
    }
    let request = json::from_slice(&raw)?;
    revalidate_pinned(&pin, "native prepare request")?;
    Ok((request, pin))
}

fn validate_selected_refs(request: &PrepareRequestV1) -> Result<()> {
    request.hosts.validate()?;
    let host = &request.hosts.native_edge;
    validate_lower_hex("native incumbent commit", &request.incumbent.commit, 40)?;
    if request.incumbent.release_root
        != format!(
            "{}/.local/share/iroha/taira/edge/releases/{}",
            host.owner_home, request.incumbent.commit
        )
    {
        return Err(eyre!(
            "native incumbent escaped its independently selected release root"
        ));
    }
    for reference in [
        &request.source_manifest,
        &request.trusted_public_key,
        &request.current_nginx_request,
        &request.new_nginx_apply_plan,
        &request.forwarding_plan,
        &request.forwarding_identity_receipt,
        &request.forwarding_journal,
    ] {
        reference.validate_public(host.owner_uid)?;
        if reference.file.identity.uid != host.owner_uid
            || reference.file.identity.mode != 0o600
            || !Path::new(&reference.file.path).starts_with(&host.custody_root)
        {
            return Err(eyre!(
                "native public input escaped its selected owner-private custody"
            ));
        }
    }
    Ok(())
}

fn run_owner(
    host: &host_pair::ResetHostV1,
    action: &str,
    request: &[u8],
    directory: &Path,
) -> Result<Vec<u8>> {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = tempfile::Builder::new()
        .prefix(".native-edge-inspect-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(directory)?;
    host::inspect_native_owner_inputs(host, action, request, scratch.path())
}

fn inspect_owner(
    host: &host_pair::ResetHostV1,
    request: &[u8],
    directory: &Path,
) -> Result<OwnerObservationV1> {
    let observed: OwnerObservationV1 =
        json::from_slice(&run_owner(host, "inspect", request, directory)?)?;
    if observed.schema != "iroha.taira.native-nginx-owned-publication-inspection.v1"
        || observed.phase != "awaiting_readiness"
    {
        return Err(eyre!(
            "native publication inspection has no capturable owner phase"
        ));
    }
    Ok(observed)
}

fn validate_new_plan(
    host: &host_pair::ResetHostV1,
    current: &[u8],
    proposed: &[u8],
    directory: &Path,
) -> Result<()> {
    let incumbent: Value = json::from_slice(current)?;
    let plan: Value = json::from_slice(proposed)?;
    let current = incumbent
        .as_object()
        .ok_or_else(|| eyre!("incumbent nginx request is not an object"))?;
    let next = plan
        .as_object()
        .ok_or_else(|| eyre!("native nginx plan is not an object"))?;
    let publication = next
        .get("publication")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("native candidate lacks its owned replacement"))?;
    let prior = publication
        .get("prior")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("native replacement lacks its admitted incumbent"))?;
    if next.get("host_kind").and_then(Value::as_str) != Some("macos")
        || publication.get("kind").and_then(Value::as_str) != Some("replace")
        || prior.get("operation_id") != current.get("operation_id")
        || next.get("operation_id") == current.get("operation_id")
        || ["native", "master", "destination"]
            .iter()
            .any(|name| next.get(*name) != current.get(*name))
    {
        return Err(eyre!(
            "new native nginx plan changes the captured host, namespace, master or predecessor"
        ));
    }
    if json::from_slice::<Value>(&run_owner(host, "validate-plan", proposed, directory)?)?
        != norito::json!({})
    {
        return Err(eyre!(
            "maintained native nginx owner refused the proposed plan"
        ));
    }
    Ok(())
}

fn derive_revision(request: &PrepareRequestV1, bytes: &[u8]) -> Result<RevisionV1> {
    let manifest: SourceManifestV1 = json::from_slice(bytes)?;
    let revision = RevisionV1 {
        branch: manifest.branch,
        commit: manifest.head_commit_sha1.clone(),
        tree: manifest.head_tree_sha1,
        cargo_lock_sha256: manifest.cargo_lock_sha256,
        source_root: request.source_root.clone(),
        source_manifest_path: request.source_manifest.file.path.clone(),
        source_manifest_sha256: sha256_hex(bytes),
        source_closure_sha256: manifest.closure_sha256,
        target: BUILD_TARGET.into(),
        profile: BUILD_PROFILE.into(),
        build_id: manifest.head_commit_sha1,
    };
    validate_revision(&revision)?;
    validate_source_closure(&revision)?;
    Ok(revision)
}

fn validate_darwin_header(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 32
        || bytes[..4] != [0xcf, 0xfa, 0xed, 0xfe]
        || bytes[4..8] != [0x0c, 0x00, 0x00, 0x01]
        || bytes[12..16] != [0x02, 0x00, 0x00, 0x00]
    {
        return Err(eyre!(
            "native candidate must be a thin Darwin AArch64 executable"
        ));
    }
    Ok(())
}

fn sign_records(
    request: &PrepareRequestV1,
    revision: &RevisionV1,
    observation: OwnerObservationV1,
    dispatcher: NativePublicFileV1,
    guard: NativePublicFileV1,
    trusted: &TrustedKeyV1,
    native_key: &KeyPair,
    owner_key: &KeyPair,
    captured_at_unix_ms: u64,
) -> Result<(
    SignedNativeEdgeCaptureV1,
    SignedNativeEdgeCandidateV1,
    native_edge_protocol::NativeEdgeCapabilityV1,
)> {
    let native = &request.hosts.native_edge;
    let release = request.incumbent.clone();
    if release.config_sha256 != observation.owned_publication.publication.sha256 {
        return Err(eyre!(
            "incumbent public config differs from its actual owned publication"
        ));
    }
    // The source manifest and actual CLI bytes are admitted before this function.
    let capture = SignedNativeEdgeCaptureV1::sign(
        NativeEdgeCaptureClaimsV1 {
            schema: "iroha.taira.public-reset.native-edge-capture.v1".into(),
            host_pair_sha256: request.hosts.digest()?,
            host_identity_sha256: native.endpoint.host_identity_sha256.clone(),
            owner_uid: native.owner_uid,
            owner_gid: native.owner_gid,
            custody_root: native.custody_root.clone(),
            retained_inventory_sha256: request.retained_inventory_sha256.clone(),
            authorization_sha256: request.authorization_sha256.clone(),
            authorization_nonce: request.authorization_nonce.clone(),
            next_genesis_hash: request.next_genesis_hash.clone(),
            release,
            dispatcher,
            native_guard: guard.clone(),
            nginx: observation.nginx,
            main_configuration: observation.main_configuration,
            master: observation.master,
            owned_publication: observation.owned_publication,
            completion: request.completion.clone(),
            forwarding_plan: request.forwarding_plan.clone(),
            forwarding_identity_receipt: request.forwarding_identity_receipt.clone(),
            forwarding_journal: request.forwarding_journal.clone(),
            helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
            captured_at_unix_ms,
        },
        native_key,
    )?;
    capture.verify(
        &request.hosts,
        &request.retained_inventory_sha256,
        &request.authorization_sha256,
        &request.authorization_nonce,
        &request.next_genesis_hash,
    )?;
    let candidate = SignedNativeEdgeCandidateV1::sign(
        NativeEdgeCandidateClaimsV1 {
            schema: "iroha.taira.public-reset.native-edge-candidate.v1".into(),
            commit: revision.commit.clone(),
            tree: revision.tree.clone(),
            cargo_lock_sha256: revision.cargo_lock_sha256.clone(),
            source_closure_sha256: revision.source_closure_sha256.clone(),
            target: host_pair::NATIVE_EDGE_TARGET.into(),
            host_pair_sha256: request.hosts.digest()?,
            iroha_cli: ArtifactV1 {
                role: "iroha_cli".into(),
                local_path: request.controller_cli_projection.clone(),
                remote_path: request.candidate_cli.file.path.clone(),
                sha256: request.candidate_cli.sha256.clone(),
                size: request.candidate_cli.file.identity.size,
                mode: request.candidate_cli.file.identity.mode,
                source_commit: revision.commit.clone(),
                target: host_pair::NATIVE_EDGE_TARGET.into(),
            },
            native_guard: guard,
            helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
        },
        owner_key,
    )?;
    candidate.verify(
        &request.hosts,
        &revision.commit,
        &revision.tree,
        &revision.cargo_lock_sha256,
        &revision.source_closure_sha256,
        trusted,
    )?;
    let capability = native_edge_protocol::NativeEdgeCapabilityV1 {
        schema: "iroha.taira.public-reset.native-edge-capability.v1".into(),
        captured_hosts: request.hosts.clone(),
        host_pair_sha256: request.hosts.digest()?,
        helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
        incumbent: capture.clone(),
        incumbent_nginx_request: request.current_nginx_request.clone(),
        nginx_apply_plan: request.new_nginx_apply_plan.clone(),
        forwarding_plan: request.forwarding_plan.clone(),
        forwarding_identity_receipt: request.forwarding_identity_receipt.clone(),
    };
    capability.validate(&request.hosts)?;
    Ok((capture, candidate, capability))
}

/// Native-only preparation; output has no claim of reset completion or live qualification.
pub(super) fn prepare(args: &PrepareNativeEdge, writer: &mut impl Write) -> Result<()> {
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (args, writer);
        Err(eyre!(
            "native edge preparation requires its admitted Darwin AArch64 host"
        ))
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        prepare_native(args, writer)
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn prepare_native(args: &PrepareNativeEdge, writer: &mut impl Write) -> Result<()> {
    let (request, request_pin) = read_request(args)?;
    validate_selected_refs(&request)?;
    let native = &request.hosts.native_edge;
    if request.schema != REQUEST_SCHEMA
        || native.owner_uid != rustix::process::geteuid().as_raw()
        || native.owner_gid != rustix::process::getegid().as_raw()
    {
        return Err(eyre!(
            "native prepare request names another physical software custodian"
        ));
    }
    for digest in [
        &request.retained_inventory_sha256,
        &request.authorization_sha256,
    ] {
        validate_lower_hex("native predecessor pin", digest, 64)?;
    }
    validate_nonce(&request.authorization_nonce)?;
    if Hash::from_str(&request.next_genesis_hash)?.to_string() != request.next_genesis_hash {
        return Err(eyre!("native predecessor genesis hash is not canonical"));
    }
    validate_absolute_normal_path(&args.output, "native output directory")?;
    if !args.output.starts_with(&native.custody_root)
        || args.output == Path::new(&native.custody_root)
    {
        return Err(eyre!(
            "native public output must stay beneath its admitted private custody"
        ));
    }
    let parent_path = args
        .output
        .parent()
        .ok_or_else(|| eyre!("native output has no parent"))?;
    validate_owner_private_dir(parent_path, "native output parent")?;
    let parent = OwnerDirectory::open(parent_path)?;
    if fs::symlink_metadata(&args.output).is_ok() {
        return Err(eyre!("native output already exists"));
    }
    let mut pins = Vec::new();
    let source = retain_public(
        &mut pins,
        &request.source_manifest,
        native.owner_uid,
        MAX_JSON_BYTES,
    )?;
    let revision = derive_revision(&request, &source)?;
    let trusted: TrustedKeyV1 = json::from_slice(&retain_public(
        &mut pins,
        &request.trusted_public_key,
        native.owner_uid,
        4096,
    )?)?;
    let owner_public = PublicKey::from_str(&trusted.public_key)?;
    if trusted.schema != TRUSTED_KEY_SCHEMA_V1
        || trusted.algorithm != "ed25519"
        || owner_public.try_algorithm()? != Algorithm::Ed25519
        || owner_public.to_string() != trusted.public_key
    {
        return Err(eyre!(
            "native release authority is not independently selected canonical Ed25519"
        ));
    }
    let expected_cli_path = format!(
        "{}/.local/share/iroha/taira/edge/releases/{}/bin/iroha",
        native.owner_home, revision.commit
    );
    if request.candidate_cli.file.path != expected_cli_path {
        return Err(eyre!(
            "native candidate escaped its exact signed-source release root"
        ));
    }
    if request.candidate_cli.file.identity.uid != native.owner_uid
        || request.candidate_cli.file.identity.gid != native.owner_gid
        || request.candidate_cli.file.identity.mode != 0o755
    {
        return Err(eyre!(
            "native candidate lacks exact executable owner custody"
        ));
    }
    let cli = RetainedInput::public(&request.candidate_cli, native.owner_uid, 512 * 1024 * 1024)?;
    let mut header = [0u8; 32];
    let mut opened = cli.pin.file.try_clone()?;
    opened.rewind()?;
    opened.read_exact(&mut header)?;
    validate_darwin_header(&header)?;
    cli.revalidate()?;
    pins.push(cli);
    let observed_raw = retain_public(
        &mut pins,
        &request.current_nginx_request,
        native.owner_uid,
        MAX_PUBLIC_PLAN,
    )?;
    let observed = inspect_owner(native, &observed_raw, parent_path)?;
    pins.push(RetainedInput::metadata(&observed.nginx, native.owner_uid)?);
    pins.push(RetainedInput::metadata(
        &observed.main_configuration,
        native.owner_uid,
    )?);
    for reference in [
        &observed.owned_publication.journal,
        &observed.owned_publication.publication,
    ] {
        retain_public(&mut pins, reference, native.owner_uid, MAX_PUBLIC_PLAN)?;
    }
    let proposed = retain_public(
        &mut pins,
        &request.new_nginx_apply_plan,
        native.owner_uid,
        MAX_PUBLIC_PLAN,
    )?;
    validate_new_plan(native, &observed_raw, &proposed, parent_path)?;
    for reference in [
        &request.forwarding_plan,
        &request.forwarding_identity_receipt,
        &request.forwarding_journal,
    ] {
        retain_public(&mut pins, reference, native.owner_uid, MAX_PUBLIC_PLAN)?;
    }
    let dispatcher_path = PathBuf::from(&native.dispatcher_path);
    let (mut dispatcher_file, dispatcher_snapshot) =
        open_pinned_regular(&dispatcher_path, "native installed dispatcher")?;
    let dispatcher_identity = native_identity(&dispatcher_file.metadata()?)?;
    let dispatcher = NativePublicFileV1 {
        file: NativeObservedFileV1 {
            path: native.dispatcher_path.clone(),
            identity: dispatcher_identity,
        },
        sha256: sha256_reader(&mut dispatcher_file, &dispatcher_path)?,
    };
    let dispatcher_pin = RetainedInput::public(&dispatcher, native.owner_uid, 512 * 1024 * 1024)?;
    ensure_pinned_unchanged(
        &dispatcher_path,
        "native installed dispatcher",
        &dispatcher_file,
        &dispatcher_snapshot,
    )?;
    if dispatcher.sha256 != native.dispatcher_sha256 {
        return Err(eyre!(
            "installed native dispatcher differs from admitted host"
        ));
    }
    if dispatcher.file.identity.uid != native.owner_uid
        || dispatcher.file.identity.gid != native.owner_gid
        || dispatcher.file.identity.mode != 0o755
    {
        return Err(eyre!(
            "native dispatcher lacks exact executable owner custody"
        ));
    }
    pins.push(dispatcher_pin);
    let guard_path = PathBuf::from(format!("{}/taira-edge/guard.json", native.custody_root));
    let (mut guard_file, guard_snapshot) =
        open_pinned_regular(&guard_path, "native installed guard")?;
    let guard = NativePublicFileV1 {
        file: NativeObservedFileV1 {
            path: guard_path
                .to_str()
                .ok_or_else(|| eyre!("native guard path is not UTF-8"))?
                .into(),
            identity: native_identity(&guard_file.metadata()?)?,
        },
        sha256: sha256_reader(&mut guard_file, &guard_path)?,
    };
    ensure_pinned_unchanged(
        &guard_path,
        "native installed guard",
        &guard_file,
        &guard_snapshot,
    )?;
    if guard.file.identity.gid != native.owner_gid {
        return Err(eyre!(
            "native guard group differs from its admitted custodian"
        ));
    }
    pins.push(RetainedInput::public(&guard, native.owner_uid, 16 * 1024)?);
    validate_completion(&request, &trusted, &mut pins, &observed.owned_publication)?;
    let incumbent_path = PathBuf::from(&request.incumbent.release_root).join("bin/iroha");
    let (mut incumbent, incumbent_snapshot) =
        open_pinned_regular(&incumbent_path, "native incumbent CLI")?;
    let incumbent_identity = native_identity(&incumbent.metadata()?)?;
    if incumbent_identity.uid != native.owner_uid
        || incumbent_identity.gid != native.owner_gid
        || incumbent_identity.mode != 0o755
        || !(64..=512 * 1024 * 1024).contains(&incumbent_identity.size)
    {
        return Err(eyre!(
            "native incumbent lacks bounded executable owner custody"
        ));
    }
    incumbent.read_exact(&mut header)?;
    validate_darwin_header(&header)?;
    incumbent.rewind()?;
    if sha256_reader(&mut incumbent, &incumbent_path)? != request.incumbent.cli_sha256 {
        return Err(eyre!(
            "native incumbent differs from independently selected prior release"
        ));
    }
    ensure_pinned_unchanged(
        &incumbent_path,
        "native incumbent CLI",
        &incumbent,
        &incumbent_snapshot,
    )?;
    let incumbent_ref = NativePublicFileV1 {
        file: NativeObservedFileV1 {
            path: incumbent_path
                .to_str()
                .ok_or_else(|| eyre!("native incumbent path is not UTF-8"))?
                .into(),
            identity: incumbent_identity,
        },
        sha256: request.incumbent.cli_sha256.clone(),
    };
    pins.push(RetainedInput::public(
        &incumbent_ref,
        native.owner_uid,
        512 * 1024 * 1024,
    )?);
    let native_public = PublicKey::from_str(&native.capture_public_key)?;
    let native_key = inputs::inherited_signing_key(args.native_signing_key_fd, &native_public)?;
    let owner_key = inputs::inherited_signing_key(args.owner_signing_key_fd, &owner_public)?;
    let (capture, candidate, capability) = sign_records(
        &request,
        &revision,
        observed.clone(),
        dispatcher,
        guard,
        &trusted,
        &native_key,
        &owner_key,
        now_unix_ms()?,
    )?;
    for pin in &pins {
        pin.revalidate()?;
        if pin.pin.path.starts_with(&args.output) {
            return Err(eyre!("native output overlaps retained input"));
        }
    }
    revalidate_pinned(&request_pin, "native prepare request")?;
    validate_source_closure(&revision)?;
    if inspect_owner(native, &observed_raw, parent_path)? != observed {
        return Err(eyre!(
            "native nginx ownership changed before signed publication"
        ));
    }
    for pin in &pins {
        pin.revalidate()?;
    }
    revalidate_pinned(&request_pin, "native prepare request")?;
    parent.revalidate()?;
    let capture_wire = json::to_json(&capture)?;
    let candidate_wire = json::to_json(&candidate)?;
    let capability_wire = json::to_json(&capability)?;
    let output = parent.publish_private_child(
        args.output
            .file_name()
            .ok_or_else(|| eyre!("native output has no name"))?,
        &[
            ("native-edge-capture.json", capture_wire.as_bytes()),
            ("native-edge-candidate.json", candidate_wire.as_bytes()),
            ("native-edge-capability.json", capability_wire.as_bytes()),
        ],
    )?;
    output.revalidate()?;
    #[derive(JsonSerialize)]
    struct Receipt {
        schema: String,
        output: String,
        request_sha256: String,
        capture_sha256: String,
        candidate_sha256: String,
        capability_sha256: String,
        qualified: bool,
    }
    writeln!(
        writer,
        "{}",
        json::to_json(&Receipt {
            schema: "iroha.taira.public-reset.native-edge-prepare-receipt.v1".into(),
            output: args
                .output
                .to_str()
                .ok_or_else(|| eyre!("native output is not UTF-8"))?
                .into(),
            request_sha256: args.expected_request_sha256.clone(),
            capture_sha256: sha256_hex(capture_wire.as_bytes()),
            candidate_sha256: sha256_hex(candidate_wire.as_bytes()),
            capability_sha256: sha256_hex(capability_wire.as_bytes()),
            qualified: false
        })?
    )?;
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn validate_completion(
    request: &PrepareRequestV1,
    trusted: &TrustedKeyV1,
    pins: &mut Vec<RetainedInput>,
    owned_publication: &NativeOwnedPublicationV1,
) -> Result<()> {
    match (&request.completion, &request.terminal_authority) {
        (NativeEdgeCompletionProvenanceV1::PublicationOnly { .. }, None) => Ok(()),
        (
            NativeEdgeCompletionProvenanceV1::ResetTerminal {
                progress,
                completion_receipt,
                checkpoints,
                global_proof,
                global_proof_predecessor,
                ..
            },
            Some(authority),
        ) => {
            for reference in std::iter::once(progress)
                .chain(std::iter::once(completion_receipt))
                .chain(checkpoints.iter())
                .chain(global_proof.iter())
                .chain(global_proof_predecessor.iter())
            {
                retain_public(
                    pins,
                    reference,
                    request.hosts.native_edge.owner_uid,
                    64 * 1024,
                )?;
            }
            let inventory_bytes = retain_public(
                pins,
                &authority.inventory,
                request.hosts.native_edge.owner_uid,
                MAX_JSON_BYTES,
            )?;
            let inventory: InventoryV1 = json::from_slice(&inventory_bytes)?;
            let authorization: AuthorizationEnvelopeV1 = json::from_slice(&retain_public(
                pins,
                &authority.authorization,
                request.hosts.native_edge.owner_uid,
                MAX_JSON_BYTES,
            )?)?;
            if authority.inventory.sha256 != request.retained_inventory_sha256
                || authorization_semantic_sha256(&authorization, trusted)?
                    != request.authorization_sha256
                || inventory.authorization_nonce != request.authorization_nonce
                || inventory.next_genesis_hash != request.next_genesis_hash
            {
                return Err(eyre!(
                    "native terminal authority differs from independently selected predecessor"
                ));
            }
            inventory.hosts.validate_physical_binding(&request.hosts)?;
            // Historical completion authentication admits no new execution; expiry remains
            // signed and is checked against every phase in the canonical proof owner.
            verify_execution_authorization(
                &inventory,
                &request.retained_inventory_sha256,
                &authorization,
                trusted,
                authorization.claims.not_before_unix_ms,
            )?;
            native_edge_protocol::validate_terminal_provenance(
                &request.completion,
                owned_publication,
                &inventory,
                &request.retained_inventory_sha256,
                &request.authorization_sha256,
                authorization.claims.execution_expires_at_unix_ms,
            )
        }
        _ => Err(eyre!(
            "native completion selection lacks its exact independent authority"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_fixture() -> (
        PrepareRequestV1,
        RevisionV1,
        OwnerObservationV1,
        NativePublicFileV1,
        NativePublicFileV1,
        TrustedKeyV1,
        KeyPair,
        KeyPair,
    ) {
        let hosts = host_pair::fixture_pair();
        let revision = sample_inventory_fixture().revision;
        let native_key = KeyPair::from_seed(b"mac-native-capture".to_vec(), Algorithm::Ed25519);
        let owner_key =
            KeyPair::from_seed(b"independent-producer-owner".to_vec(), Algorithm::Ed25519);
        let trusted = TrustedKeyV1 {
            schema: TRUSTED_KEY_SCHEMA_V1.into(),
            algorithm: "ed25519".into(),
            public_key: owner_key.public_key().to_string(),
        };
        let incumbent = EdgeAdmittedReleaseV1 {
            commit: "a".repeat(40),
            release_root: format!(
                "{}/.local/share/iroha/taira/edge/releases/{}",
                hosts.native_edge.owner_home,
                "a".repeat(40)
            ),
            cli_sha256: "b".repeat(64),
            config_sha256: "c".repeat(64),
        };
        let capture = host_pair::fixture_native_edge_capture(
            &hosts,
            incumbent.clone(),
            &"d".repeat(64),
            &"e".repeat(64),
            "fixture-native-edge-prepare",
            &Hash::new(b"genesis").to_string(),
        );
        let candidate = host_pair::fixture_native_edge_candidate(
            &hosts,
            &revision,
            "/native/retained-controller/iroha-darwin".into(),
        );
        let mut cli = capture.claims.dispatcher.clone();
        cli.file.path = candidate.claims.iroha_cli.remote_path;
        cli.file.identity.inode = 20;
        let mut apply = capture.claims.forwarding_plan.clone();
        apply.file.path = format!(
            "{}/new-nginx-apply-plan.json",
            hosts.native_edge.custody_root
        );
        apply.file.identity.inode = 21;
        let request = PrepareRequestV1 {
            schema: REQUEST_SCHEMA.into(),
            hosts,
            retained_inventory_sha256: capture.claims.retained_inventory_sha256.clone(),
            authorization_sha256: capture.claims.authorization_sha256.clone(),
            authorization_nonce: capture.claims.authorization_nonce.clone(),
            next_genesis_hash: capture.claims.next_genesis_hash.clone(),
            incumbent,
            source_root: revision.source_root.clone(),
            source_manifest: capture.claims.forwarding_plan.clone(),
            trusted_public_key: capture.claims.forwarding_plan.clone(),
            candidate_cli: cli,
            controller_cli_projection: "/native/retained-controller/iroha-darwin".into(),
            current_nginx_request: capture.claims.forwarding_plan.clone(),
            new_nginx_apply_plan: apply,
            forwarding_plan: capture.claims.forwarding_plan.clone(),
            forwarding_identity_receipt: capture.claims.forwarding_identity_receipt.clone(),
            forwarding_journal: capture.claims.forwarding_journal.clone(),
            completion: capture.claims.completion.clone(),
            terminal_authority: None,
        };
        let observation = OwnerObservationV1 {
            schema: "iroha.taira.native-nginx-owned-publication-inspection.v1".into(),
            owned_publication: capture.claims.owned_publication,
            nginx: capture.claims.nginx,
            main_configuration: capture.claims.main_configuration,
            master: capture.claims.master,
            phase: "awaiting_readiness".into(),
        };
        (
            request,
            revision,
            observation,
            capture.claims.dispatcher,
            capture.claims.native_guard,
            trusted,
            native_key,
            owner_key,
        )
    }

    #[test]
    fn native_edge_records_use_independent_real_signatures_and_reject_substitution() {
        let (request, revision, observation, dispatcher, guard, trusted, native_key, owner_key) =
            signed_fixture();
        let (capture, candidate, capability) = sign_records(
            &request,
            &revision,
            observation.clone(),
            dispatcher.clone(),
            guard.clone(),
            &trusted,
            &native_key,
            &owner_key,
            1_234,
        )
        .unwrap();
        assert_eq!(
            capture.claims.dispatcher.file.identity.inode,
            dispatcher.file.identity.inode
        );
        assert_ne!(
            capture.claims.dispatcher.file.identity.inode,
            request.candidate_cli.file.identity.inode
        );
        candidate
            .verify(
                &request.hosts,
                &revision.commit,
                &revision.tree,
                &revision.cargo_lock_sha256,
                &revision.source_closure_sha256,
                &trusted,
            )
            .unwrap();
        capability.validate(&request.hosts).unwrap();
        assert_eq!(
            capability.incumbent_nginx_request,
            request.current_nginx_request
        );
        let mut substituted = capture.clone();
        substituted.claims.main_configuration.identity.inode += 1;
        assert!(
            substituted
                .verify(
                    &request.hosts,
                    &request.retained_inventory_sha256,
                    &request.authorization_sha256,
                    &request.authorization_nonce,
                    &request.next_genesis_hash
                )
                .is_err()
        );
        let mut substituted = candidate;
        substituted.claims.iroha_cli.target = BUILD_TARGET.into();
        assert!(
            substituted
                .verify(
                    &request.hosts,
                    &revision.commit,
                    &revision.tree,
                    &revision.cargo_lock_sha256,
                    &revision.source_closure_sha256,
                    &trusted
                )
                .is_err()
        );
        let other = KeyPair::from_seed(b"untrusted-native-custodian".to_vec(), Algorithm::Ed25519);
        assert!(
            sign_records(
                &request,
                &revision,
                observation.clone(),
                dispatcher.clone(),
                guard.clone(),
                &trusted,
                &other,
                &owner_key,
                1_234
            )
            .is_err()
        );
        assert!(
            sign_records(
                &request,
                &revision,
                observation,
                dispatcher,
                guard,
                &trusted,
                &native_key,
                &other,
                1_234
            )
            .is_err()
        );
    }

    #[test]
    fn native_selected_public_paths_cannot_hash_private_nginx_main_or_foreign_release() {
        let (mut request, ..) = signed_fixture();
        validate_selected_refs(&request).unwrap();
        request.forwarding_plan.file.path = "/opt/homebrew/etc/nginx/nginx.conf".into();
        assert!(validate_selected_refs(&request).is_err());
        let (mut request, ..) = signed_fixture();
        request.incumbent.release_root = "/foreign/private-custody".into();
        assert!(validate_selected_refs(&request).is_err());
        let (mut request, ..) = signed_fixture();
        request.trusted_public_key.file.identity.mode = 0o644;
        assert!(validate_selected_refs(&request).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_edge_three_record_publication_is_complete_private_and_create_only() {
        let (request, revision, observation, dispatcher, guard, trusted, native_key, owner_key) =
            signed_fixture();
        let (capture, candidate, capability) = sign_records(
            &request,
            &revision,
            observation,
            dispatcher,
            guard,
            &trusted,
            &native_key,
            &owner_key,
            1_234,
        )
        .unwrap();
        let wires = [
            json::to_json(&capture).unwrap(),
            json::to_json(&candidate).unwrap(),
            json::to_json(&capability).unwrap(),
        ];
        let dir = tempfile::Builder::new()
            .prefix(".native-complete-publication-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let parent = OwnerDirectory::open(dir.path()).unwrap();
        let files = [
            ("native-edge-capture.json", wires[0].as_bytes()),
            ("native-edge-candidate.json", wires[1].as_bytes()),
            ("native-edge-capability.json", wires[2].as_bytes()),
        ];
        let output = parent.publish_private_child("complete", &files).unwrap();
        output.revalidate().unwrap();
        assert_eq!(output.path().metadata().unwrap().mode() & 0o7777, 0o700);
        for (name, bytes) in files {
            let path = output.path().join(name);
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(path.metadata().unwrap().mode() & 0o7777, 0o600);
            assert_eq!(path.metadata().unwrap().nlink(), 1);
        }
        let reread: SignedNativeEdgeCaptureV1 =
            json::from_slice(&fs::read(output.path().join("native-edge-capture.json")).unwrap())
                .unwrap();
        reread
            .verify(
                &request.hosts,
                &request.retained_inventory_sha256,
                &request.authorization_sha256,
                &request.authorization_nonce,
                &request.next_genesis_hash,
            )
            .unwrap();
        assert!(parent.publish_private_child("complete", &files).is_err());
        fs::create_dir(dir.path().join("existing-empty")).unwrap();
        assert!(
            parent
                .publish_private_child("existing-empty", &files)
                .is_err()
        );
    }

    #[test]
    fn native_edge_prepare_requires_both_inherited_keys_and_independent_request_pin() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct Cli {
            #[command(flatten)]
            args: PrepareNativeEdge,
        }
        let digest = "a".repeat(64);
        let args = [
            "test",
            "--request",
            "/native/request.json",
            "--expected-request-sha256",
            digest.as_str(),
            "--native-signing-key-fd",
            "198",
            "--owner-signing-key-fd",
            "199",
            "--output",
            "/native/fresh",
        ];
        assert!(Cli::try_parse_from(args.clone()).is_ok());
        for name in [
            "--request",
            "--expected-request-sha256",
            "--native-signing-key-fd",
            "--owner-signing-key-fd",
            "--output",
        ] {
            let mut missing = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
            let at = missing.iter().position(|arg| arg == name).unwrap();
            missing.drain(at..at + 2);
            assert!(Cli::try_parse_from(missing).is_err(), "{name}");
        }
    }

    #[test]
    fn native_edge_candidate_header_refuses_linux_wrong_arch_and_library() {
        let mut bytes = [0u8; 32];
        bytes[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        bytes[4..8].copy_from_slice(&[0x0c, 0, 0, 1]);
        bytes[12..16].copy_from_slice(&[2, 0, 0, 0]);
        assert!(validate_darwin_header(&bytes).is_ok());
        for (offset, value) in [(0, 0x7f), (4, 7), (12, 6)] {
            let mut wrong = bytes;
            wrong[offset] = value;
            assert!(validate_darwin_header(&wrong).is_err());
        }
        assert!(validate_darwin_header(&bytes[..31]).is_err());
    }

    #[test]
    fn native_owner_observation_rejects_extra_fields_and_decimal_string_metadata() {
        let hosts = host_pair::fixture_pair();
        let capture = host_pair::fixture_native_edge_capture(
            &hosts,
            EdgeAdmittedReleaseV1 {
                commit: "1".repeat(40),
                release_root: format!(
                    "{}/.local/share/iroha/taira/edge/releases/{}",
                    hosts.native_edge.owner_home,
                    "1".repeat(40)
                ),
                cli_sha256: "2".repeat(64),
                config_sha256: "3".repeat(64),
            },
            &"4".repeat(64),
            &"5".repeat(64),
            "fixture-nonce",
            &Hash::new(b"genesis").to_string(),
        );
        let observation = OwnerObservationV1 {
            schema: "iroha.taira.native-nginx-owned-publication-inspection.v1".into(),
            owned_publication: capture.claims.owned_publication,
            nginx: capture.claims.nginx,
            main_configuration: capture.claims.main_configuration,
            master: capture.claims.master,
            phase: "awaiting_readiness".into(),
        };
        let wire = json::to_json(&observation).unwrap();
        assert!(json::from_str::<OwnerObservationV1>(&wire).is_ok());
        let mut extra: Value = json::from_str(&wire).unwrap();
        extra.as_object_mut().unwrap().insert(
            "private_config_sha256".into(),
            Value::String("a".repeat(64)),
        );
        assert!(json::from_str::<OwnerObservationV1>(&json::to_json(&extra).unwrap()).is_err());
        let mut wrong: Value = json::from_str(&wire).unwrap();
        wrong
            .as_object_mut()
            .unwrap()
            .get_mut("nginx")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .get_mut("identity")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("inode".into(), Value::String("9007199254740993".into()));
        assert!(json::from_str::<OwnerObservationV1>(&json::to_json(&wrong).unwrap()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_public_input_retains_identity_digest_and_role_bound() {
        let dir = tempfile::Builder::new()
            .prefix(".native-public-input-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("public.json");
        fs::write(&path, b"public-body").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let reference = NativePublicFileV1 {
            file: NativeObservedFileV1 {
                path: path.to_str().unwrap().into(),
                identity: native_identity(&path.metadata().unwrap()).unwrap(),
            },
            sha256: sha256_hex(b"public-body"),
        };
        let uid = rustix::process::geteuid().as_raw();
        let retained = RetainedInput::public(&reference, uid, 128).unwrap();
        assert_eq!(retained.bytes(128).unwrap(), b"public-body");
        assert!(RetainedInput::public(&reference, uid, 10).is_err());
        let mut wrong = reference.clone();
        wrong.sha256 = "a".repeat(64);
        assert!(RetainedInput::public(&wrong, uid, 128).is_err());
        fs::write(&path, b"changed").unwrap();
        assert!(retained.revalidate().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_request_pin_and_size_fail_before_decoding_or_opening_any_signing_fd() {
        let (request, ..) = signed_fixture();
        let dir = tempfile::Builder::new()
            .prefix(".native-request-pin-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("request.json");
        let wire = json::to_json(&request).unwrap();
        fs::write(&path, &wire).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut args = PrepareNativeEdge {
            request: path.clone(),
            expected_request_sha256: sha256_hex(wire.as_bytes()),
            native_signing_key_fd: 65534,
            owner_signing_key_fd: 65535,
            output: dir.path().join("fresh"),
        };
        assert!(read_request(&args).is_ok());
        fs::write(&path, b"not even JSON").unwrap();
        let error = read_request(&args).err().unwrap();
        assert!(error.to_string().contains("independent pin"));
        let oversized = vec![b' '; usize::try_from(MAX_REQUEST + 1).unwrap()];
        fs::write(&path, &oversized).unwrap();
        args.expected_request_sha256 = sha256_hex(&oversized);
        let error = read_request(&args).err().unwrap();
        assert!(error.to_string().contains("limit"));
        assert!(!args.output.exists());
    }
}
