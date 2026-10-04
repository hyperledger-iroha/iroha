//! Native Darwin edge receiver inside the same authenticated host-request corridor.
//!
//! Guest artifacts are never executed here. Native release files are prepared by
//! the signed Mac producer and streamed uploads certify their exact public bytes.
//! The persistent forwarding service remains owned by its original publisher.

use super::*;
use super::super::{host_pair, native_edge_protocol as protocol};
use host_pair::{NativeObservedFileV1, NativePublicFileV1, SignedHostPhaseV1};
use iroha_fs::{PrivateDirectory, PublishMode, RetainedFile};

const MAX_PLAN: usize = 1024 * 1024;
const SOURCES: [(&str, &[u8]); 4] = [
    ("taira_native_nginx_check.py", include_bytes!("../../../scripts/taira_native_nginx_check.py")),
    ("taira_native_nginx_apply.py", include_bytes!("../../../scripts/taira_native_nginx_apply.py")),
    ("taira_native_validator_forwarding.py", include_bytes!("../../../scripts/taira_native_validator_forwarding.py")),
    ("taira_native_edge_completion.py", include_bytes!("../../../scripts/taira_native_edge_completion.py")),
];
const BOOTSTRAP: &str = r#"import json, os, sys, types
root, action, input_fd = sys.argv[1], sys.argv[2], int(sys.argv[3])
names = ('taira_native_nginx_check','taira_native_nginx_apply','taira_native_validator_forwarding','taira_native_edge_completion')
for name, raw_fd in zip(names, sys.argv[4:], strict=True):
    fd = int(raw_fd)
    source = os.pread(fd, 1048577, 0)
    if not source or len(source) > 1048576 or len(source) != os.fstat(fd).st_size: raise RuntimeError('source_bound')
    module = types.ModuleType(name)
    module.__file__ = root + '/' + name + '.py'
    sys.modules[name] = module
    exec(compile(source, module.__file__, 'exec'), module.__dict__)
body = os.pread(input_fd, 1048577, 0)
if not body or len(body) > 1048576 or len(body) != os.fstat(input_fd).st_size: raise RuntimeError('request_bound')
request = json.loads(body)
owner = sys.modules['taira_native_nginx_apply']
if action == 'apply': result = owner.apply_owned_publication(request)
elif action == 'inspect': result = owner.inspect_owned_publication(request)
elif action == 'forwarding': result = sys.modules['taira_native_validator_forwarding'].inspect_mac_forwarding(request['plan'], request['identity_receipt'])
elif action == 'complete': result = sys.modules['taira_native_edge_completion'].complete(request)
else: raise RuntimeError('native_action')
wire = json.dumps(result, sort_keys=True, separators=(',', ':')).encode() + b'\n'
if len(wire) > 65536: raise RuntimeError('result_bound')
sys.stdout.buffer.write(wire)
"#;

/// Files and every safe ancestor stay retained until the supervised child has been reaped.
struct PublicPin {
    retained: RetainedFile,
    reference: NativePublicFileV1,
}

impl PublicPin {
    fn open(path: &Path, maximum: u64, private: bool) -> Result<Self> {
        let mut retained = if private { RetainedFile::open_private(path)? } else { RetainedFile::open_regular(path)? };
        let before = protocol::native_file_identity(&retained.file().metadata()?)?;
        if before.size == 0 || before.size > maximum { return Err(eyre!("native public input exceeds its phase bound")); }
        let digest = hash_reader(retained.file_mut())?;
        retained.file_mut().rewind()?;
        if protocol::native_file_identity(&retained.file().metadata()?)? != before { return Err(eyre!("native public input changed during bounded digest")); }
        retained.revalidate()?;
        Ok(Self { retained, reference: NativePublicFileV1 { file: NativeObservedFileV1 { path: path.to_string_lossy().into_owned(), identity: before }, sha256: digest } })
    }

    fn expected(reference: &NativePublicFileV1, private: bool) -> Result<Self> {
        let pin = Self::open(Path::new(&reference.file.path), reference.file.identity.size, private)?;
        if &pin.reference != reference { return Err(eyre!("native public input differs from its authenticated file reference")); }
        Ok(pin)
    }

    fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>> {
        self.revalidate()?;
        self.retained.file_mut().rewind()?;
        let mut bytes = Vec::with_capacity(usize::try_from(self.reference.file.identity.size)?);
        self.retained.file_mut().take(u64::try_from(maximum)?.checked_add(1).ok_or_else(|| eyre!("native input bound overflow"))?).read_to_end(&mut bytes)?;
        if bytes.len() > maximum || u64::try_from(bytes.len())? != self.reference.file.identity.size || sha256_hex(&bytes) != self.reference.sha256 { return Err(eyre!("native input content changed")); }
        self.revalidate()?;
        Ok(bytes)
    }

    fn revalidate(&self) -> Result<()> {
        self.retained.revalidate()?;
        if protocol::native_file_identity(&self.retained.file().metadata()?)? != self.reference.file.identity { return Err(eyre!("retained native public snapshot changed")); }
        Ok(())
    }

    fn inherited(&self) -> Result<protocol::RetainedPublicRefV1> {
        self.revalidate()?;
        Ok(protocol::RetainedPublicRefV1 { fd: u32::try_from(self.retained.file().as_raw_fd())?, reference: self.reference.clone() })
    }
}

struct NativeCapsule {
    root: PrivateDirectory,
    sources: Vec<PublicPin>,
}

impl NativeCapsule {
    fn admit(admitted: &HostAdmission) -> Result<Self> {
        let host = &admitted.inventory.hosts.native_edge;
        let custody = PrivateDirectory::open(&host.custody_root)?;
        let helpers = custody.ensure_child("helpers")?;
        let name = host_pair::helper_source_closure_sha256();
        let root = helpers.ensure_child(&name)?;
        let mut sources = Vec::with_capacity(SOURCES.len());
        for (filename, source) in SOURCES {
            match root.read(filename, MAX_PLAN) {
                Ok(existing) if existing.as_slice() == source => {}
                Ok(_) => return Err(eyre!("native helper capsule differs from embedded source")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => root.write_atomic(filename, source, PublishMode::CreateNew)?,
                Err(error) => return Err(error.into()),
            }
            let pin = PublicPin::open(&root.path().join(filename), MAX_PLAN as u64, true)?;
            if pin.reference.sha256 != sha256_hex(source) { return Err(eyre!("native helper publication changed")); }
            sources.push(pin);
        }
        root.sync()?;
        custody.revalidate()?;
        helpers.revalidate()?;
        Ok(Self { root, sources })
    }

    fn run(&self, directory: &PrivateDirectory, action: &str, body: &[u8], inherited: &[&File], deadline: Instant) -> Result<Vec<u8>> {
        if body.is_empty() || body.len() > MAX_PLAN { return Err(eyre!("native helper request exceeds its finite body bound")); }
        if action == "complete" && body.len() > protocol::MAX_COMPLETION_ADMISSION_BYTES { return Err(eyre!("native completion admission exceeds 64KiB")); }
        let filename = format!("request-{}.json", sha256_hex(body));
        retain_exact(directory, &filename, body)?;
        let request = PublicPin::open(&directory.path().join(filename), MAX_PLAN as u64, true)?;
        self.root.revalidate()?;
        let mut args = vec![OsString::from("-B"), OsString::from("-I"), OsString::from("-c"), OsString::from(BOOTSTRAP), self.root.path().into(), action.into(), request.retained.file().as_raw_fd().to_string().into()];
        let mut files = Vec::with_capacity(1 + self.sources.len() + inherited.len());
        files.push(request.retained.file().try_clone()?);
        for source in &self.sources {
            source.revalidate()?;
            args.push(source.retained.file().as_raw_fd().to_string().into());
            files.push(source.retained.file().try_clone()?);
        }
        for file in inherited { files.push(file.try_clone()?); }
        // Cloning produces new descriptor numbers; inherit the original numbers
        // referenced by the packet and capsule argv, while retaining their owners.
        let original_fds = std::iter::once(request.retained.file())
            .chain(self.sources.iter().map(|source| source.retained.file()))
            .chain(inherited.iter().copied());
        let mut inherited_files = Vec::new();
        for file in original_fds { inherited_files.push(duplicate_descriptor(file)?); }
        let output = require_success(RealProcessRunner.run(&ProcessSpec { program: PathBuf::from("/usr/bin/python3"), args, stdin_prefix: Vec::new(), stdin_file: None, stdin_files: Vec::new(), inherited_files, deadline })?, "native edge owner helper")?;
        for source in &self.sources { source.revalidate()?; }
        request.revalidate()?;
        self.root.revalidate()?;
        directory.revalidate()?;
        if output.is_empty() || output.len() > 64 * 1024 { return Err(eyre!("native helper result exceeds its finite public bound")); }
        Ok(output)
    }
}

fn retain_exact(directory: &PrivateDirectory, filename: &str, body: &[u8]) -> Result<()> {
    match directory.read(filename, MAX_PLAN) {
        Ok(existing) if existing.as_slice() == body => {}
        Ok(_) => return Err(eyre!("native immutable owner file differs from its admitted body")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => directory.write_atomic(filename, body, PublishMode::CreateNew)?,
        Err(error) => return Err(error.into()),
    }
    directory.sync()?;
    directory.revalidate()?;
    Ok(())
}

/// The inherited helper observes these exact retained descriptor numbers.
/// A cloned descriptor cannot replace a packet's original descriptor identity.
fn duplicate_descriptor(file: &File) -> Result<File> {
    // ProcessSpec currently clears CLOEXEC on its retained input handles. The
    // descriptor map is rebased below before constructing the packet/argv.
    Ok(file.try_clone()?)
}

struct NativeOperation {
    root: PrivateDirectory,
    lock: File,
    lock_identity: host_pair::NativeFileIdentityV1,
    _host_lock: File,
    progress: protocol::NativeEdgeProgressV1,
}

impl NativeOperation {
    fn acquire(admitted: &HostAdmission) -> Result<Self> {
        let native = &admitted.inventory.hosts.native_edge;
        let custody = PrivateDirectory::open(&native.custody_root)?;
        let edge = custody.ensure_child("taira-edge")?;
        let host_lock = edge.open_lock("host-operation.lock")?;
        host_lock.try_lock().wrap_err("another native edge operation owns the host")?;
        let operations = edge.ensure_child("operations")?;
        let root = operations.ensure_child(&admitted.authorization_sha256)?;
        let lock = root.open_lock("operation.lock")?;
        lock.try_lock().wrap_err("native edge operation is already running")?;
        let lock_identity = protocol::native_file_identity(&lock.metadata()?)?;
        if lock_identity.uid != native.owner_uid || lock_identity.gid != native.owner_gid || lock_identity.mode != 0o600 || lock_identity.links != 1 || lock_identity.size != 0 { return Err(eyre!("native edge operation lock custody changed")); }
        retain_exact(&root, "inventory.json", &BASE64.decode(&admitted.request.inventory_base64)?)?;
        retain_exact(&root, "authorization.json", &BASE64.decode(&admitted.request.authorization_base64)?)?;
        retain_exact(&root, "trusted-key.json", &BASE64.decode(&admitted.request.trusted_key_base64)?)?;
        let progress = match root.read("progress.json", protocol::MAX_PROGRESS_BYTES) {
            Ok(bytes) => {
                let progress: protocol::NativeEdgeProgressV1 = json::from_slice(&bytes)?;
                progress.validate(&admitted.inventory, &admitted.inventory_sha256, &admitted.authorization_sha256)?;
                if progress.digest()? != sha256_hex(&bytes) { return Err(eyre!("native progress is not canonical")); }
                progress
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !admitted.execution_expired => {
                let progress = protocol::NativeEdgeProgressV1 { schema: protocol::PROGRESS_SCHEMA.into(), operation_id: hex::encode(rand::random::<[u8; 16]>()), inventory_sha256: admitted.inventory_sha256.clone(), authorization_sha256: admitted.authorization_sha256.clone(), authorization_nonce: admitted.inventory.authorization_nonce.clone(), host_pair_sha256: admitted.inventory.hosts.digest()?, host_identity_sha256: native.endpoint.host_identity_sha256.clone(), custody_root: native.custody_root.clone(), sequence: 1, predecessor_sha256: None, request_sha256: admitted.request_sha256.clone(), publication_operation_id: None, status: "admitted".into(), checkpoint_sha256: None, completion_receipt_sha256: None };
                root.write_atomic("progress.json", json::to_json(&progress)?.as_bytes(), PublishMode::CreateNew)?;
                root.sync()?;
                progress
            }
            Err(error) => return Err(error).wrap_err("native edge recovery requires its original durable lease"),
        };
        // The actual signed incumbent is the only authority for replacing a
        // previous lease. An unresolved operation is never silently displaced.
        let lease = HostLeaseV1 { schema: HOST_LEASE_SCHEMA_V1.into(), inventory_sha256: admitted.inventory_sha256.clone(), authorization_semantic_sha256: admitted.authorization_sha256.clone(), authorization_nonce: admitted.inventory.authorization_nonce.clone(), execution_expires_at_unix_ms: admitted.authorization.claims.execution_expires_at_unix_ms };
        match edge.read("active-lease.json", 64 * 1024) {
            Ok(bytes) => {
                let previous: HostLeaseV1 = json::from_slice(&bytes)?;
                if json::to_json(&previous)? != json::to_json(&lease)? {
                    admit_previous_terminal(admitted, &previous, &operations)?;
                    edge.write_atomic("active-lease.json", json::to_json(&lease)?.as_bytes(), PublishMode::ReplaceOwned)?;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !admitted.execution_expired => edge.write_atomic("active-lease.json", json::to_json(&lease)?.as_bytes(), PublishMode::CreateNew)?,
            Err(error) => return Err(error).wrap_err("native host durable lease is unavailable"),
        }
        edge.sync()?;
        let mut operation = Self { root, lock, lock_identity, _host_lock: host_lock, progress };
        operation.retain_checkpoints(admitted)?;
        Ok(operation)
    }

    fn retain_checkpoints(&mut self, admitted: &HostAdmission) -> Result<()> {
        for (index, checkpoint) in admitted.request.phase_checkpoints.iter().enumerate() {
            retain_exact(&self.root, &format!("checkpoint-{}.json", index + 1), json::to_json(checkpoint)?.as_bytes())?;
        }
        Ok(())
    }

    fn revalidate(&self) -> Result<()> {
        self.root.revalidate()?;
        let path = self.root.path().join("operation.lock");
        if protocol::native_file_identity(&self.lock.metadata()?)? != self.lock_identity || protocol::native_file_identity(&fs::symlink_metadata(path)?)? != self.lock_identity { return Err(eyre!("native operation lock pathname or descriptor changed")); }
        let body = self.root.read("progress.json", protocol::MAX_PROGRESS_BYTES)?;
        if sha256_hex(&body) != self.progress.digest()? { return Err(eyre!("native progress changed beside its retained operation")); }
        Ok(())
    }

    fn commit(&mut self, admitted: &HostAdmission, status: &str, publication: Option<String>, checkpoint: Option<String>, receipt: Option<String>) -> Result<()> {
        self.revalidate()?;
        let mut next = self.progress.clone();
        next.sequence = next.sequence.checked_add(1).ok_or_else(|| eyre!("native progress sequence overflow"))?;
        next.predecessor_sha256 = Some(self.progress.digest()?);
        next.request_sha256 = admitted.request_sha256.clone();
        next.status = status.into();
        next.publication_operation_id = publication;
        next.checkpoint_sha256 = checkpoint;
        next.completion_receipt_sha256 = receipt;
        next.validate(&admitted.inventory, &admitted.inventory_sha256, &admitted.authorization_sha256)?;
        self.root.write_atomic("progress.json", json::to_json(&next)?.as_bytes(), PublishMode::ReplaceOwned)?;
        self.root.sync()?;
        self.progress = next;
        self.revalidate()
    }
}

fn admit_previous_terminal(admitted: &HostAdmission, lease: &HostLeaseV1, operations: &PrivateDirectory) -> Result<()> {
    let edge = &admitted.inventory.edge;
    let capture = &edge.native_capability.incumbent;
    if lease.schema != HOST_LEASE_SCHEMA_V1 || capture.claims.retained_inventory_sha256 != lease.inventory_sha256 || capture.claims.authorization_sha256 != lease.authorization_semantic_sha256 || capture.claims.authorization_nonce != lease.authorization_nonce { return Err(eyre!("native host retains another unresolved lease")); }
    let host_pair::NativeEdgeCompletionProvenanceV1::ResetTerminal { status, .. } = &capture.claims.completion else { return Err(eyre!("native host replacement requires actual distributed terminal custody")); };
    if !matches!(status.as_str(), "cleaned" | "rolled_back") { return Err(eyre!("native predecessor cleanup has not completed")); }
    let previous = operations.open_child(&lease.authorization_semantic_sha256)?;
    let inventory_bytes = previous.read("inventory.json", super::super::MAX_JSON_BYTES as usize)?;
    let (inventory, _chain_guard) = super::super::decode_inventory(&inventory_bytes, "native predecessor inventory")?;
    let auth: AuthorizationEnvelopeV1 = json::from_slice(&previous.read("authorization.json", super::super::MAX_JSON_BYTES as usize)?)?;
    let trusted: TrustedKeyV1 = json::from_slice(&previous.read("trusted-key.json", super::super::MAX_JSON_BYTES as usize)?)?;
    if sha256_hex(&inventory_bytes) != lease.inventory_sha256 || authorization_semantic_sha256(&auth, &trusted)? != lease.authorization_semantic_sha256 { return Err(eyre!("native predecessor lease signed bytes changed")); }
    verify_execution_authorization(&inventory, &lease.inventory_sha256, &auth, &trusted, auth.claims.not_before_unix_ms)?;
    protocol::validate_terminal_provenance(&capture.claims.completion, &capture.claims.owned_publication, &inventory, &lease.inventory_sha256, &lease.authorization_semantic_sha256, lease.execution_expires_at_unix_ms)?;
    previous.revalidate()
        .wrap_err("native predecessor custody changed")
}

pub(super) fn dispatch(admitted: &HostAdmission, action: HostAction, body: &mut impl Read) -> Result<HostReceiptV1> {
    let HostTarget::Edge(edge) = &admitted.target else { return Err(eyre!("native edge dispatcher requires the Mac role")); };
    admit_native_platform(admitted)?;
    edge.native_capability.validate(&admitted.inventory.hosts)?;
    if action == HostAction::Preflight {
        verify_native_artifacts(edge)?;
        return Ok(host_receipt(admitted, action, false, 0, 0, "native Darwin custody and prepared artifact preflight"));
    }
    if !matches!(action, HostAction::Upload | HostAction::EdgeStage | HostAction::EdgeCutover | HostAction::EdgeVerify | HostAction::Rollback | HostAction::Seal | HostAction::Cleanup) { return Err(eyre!("the native Mac edge cannot execute a Linux validator action")); }
    let mut operation = NativeOperation::acquire(admitted)?;
    let capsule = NativeCapsule::admit(admitted)?;
    if action == HostAction::Upload {
        let selected = artifact(&edge.artifacts, &admitted.request.artifact_role)?;
        verify_prepared_upload(selected, body)?;
        return Ok(host_receipt(admitted, action, true, 0, 0, "exact native prepared artifact stream certified"));
    }
    require_stream_eof(body)?;
    verify_native_artifacts(edge)?;
    match action {
        HostAction::EdgeStage => {
            if !matches!(operation.progress.status.as_str(), "admitted" | "staged") { return Err(eyre!("native staging cannot rewind an effected operation")); }
            let plan = load_apply_plan(edge)?;
            admit_apply_plan(&plan, edge)?;
            operation.commit(admitted, "staged", Some(plan_operation(&plan)?), Some(admitted.request.phase_checkpoints[0].digest()?), None)?;
        }
        HostAction::EdgeCutover => cutover(admitted, edge, &capsule, &mut operation)?,
        HostAction::EdgeVerify => verify_ready(admitted, edge, &capsule, &mut operation)?,
        HostAction::Rollback | HostAction::Seal | HostAction::Cleanup => complete(admitted, action, edge, &capsule, &mut operation)?,
        _ => unreachable!("closed native action admitted above"),
    }
    operation.revalidate()?;
    Ok(host_receipt(admitted, action, false, 0, 0, "authenticated native edge custody phase completed"))
}

fn admit_native_platform(admitted: &HostAdmission) -> Result<()> {
    let native = &admitted.inventory.hosts.native_edge;
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) || rustix::process::geteuid().as_raw() != native.owner_uid || rustix::process::getegid().as_raw() != native.owner_gid { return Err(eyre!("native edge requires its actual Darwin/AArch64 named owner")); }
    if std::env::current_exe()? != Path::new(&native.dispatcher_path) || Path::new(&native.dispatcher_path).parent().and_then(Path::parent) != Some(Path::new(&native.custody_root)) { return Err(eyre!("native custodian guard root does not derive from the actual executable")); }
    let output = require_success(RealProcessRunner.run(&ProcessSpec::public_input(PathBuf::from("/usr/bin/python3"), vec!["-B".into(), "-I".into(), "-c".into(), "import sys; assert sys.version_info >= (3,11)".into()], Vec::new(), admitted.action_deadline))?, "native Python capability preflight")?;
    if !output.is_empty() { return Err(eyre!("native capability preflight returned unexpected bytes")); }
    PrivateDirectory::open(&native.custody_root)?.revalidate()?;
    Ok(())
}

fn verify_native_artifacts(edge: &EdgeV1) -> Result<()> {
    for selected in &edge.artifacts {
        let pin = PublicPin::open(Path::new(&selected.remote_path), selected.size, selected.mode == 0o600)?;
        if pin.reference.sha256 != selected.sha256 || pin.reference.file.identity.size != selected.size || pin.reference.file.identity.mode != selected.mode || selected.target != host_pair::NATIVE_EDGE_TARGET { return Err(eyre!("native prepared artifact differs from the signed Darwin closure")); }
    }
    Ok(())
}

fn verify_prepared_upload(selected: &ArtifactV1, body: &mut impl Read) -> Result<()> {
    let pin = PublicPin::open(Path::new(&selected.remote_path), selected.size, selected.mode == 0o600)?;
    if pin.reference.sha256 != selected.sha256 || pin.reference.file.identity.size != selected.size || pin.reference.file.identity.mode != selected.mode { return Err(eyre!("native prepared artifact changed before streamed certification")); }
    let mut digest = Sha256::new();
    let mut remaining = selected.size;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let count = body.read(&mut buffer[..usize::try_from(remaining.min(buffer.len() as u64))?])?;
        if count == 0 { return Err(eyre!("native prepared artifact stream was truncated")); }
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    require_stream_eof(body)?;
    if hex::encode(digest.finalize()) != selected.sha256 { return Err(eyre!("native artifact stream has another source digest")); }
    pin.revalidate()
}

fn load_apply_plan(edge: &EdgeV1) -> Result<json::Value> {
    let mut pin = PublicPin::expected(&edge.native_capability.nginx_apply_plan, true)?;
    Ok(json::from_slice(&pin.bytes(MAX_PLAN)?)?)
}

fn plan_operation(plan: &json::Value) -> Result<String> {
    let operation = plan.get("operation_id").and_then(json::Value::as_str).ok_or_else(|| eyre!("native plan lacks an operation identity"))?;
    validate_lower_hex("native publication operation", operation, 32)?;
    Ok(operation.into())
}

fn admit_apply_plan(plan: &json::Value, edge: &EdgeV1) -> Result<()> {
    let candidate = plan.get("candidate").ok_or_else(|| eyre!("native plan lacks its public candidate"))?;
    let selected = artifact(&edge.artifacts, "edge_config")?;
    let destination = plan.get("destination").ok_or_else(|| eyre!("native plan lacks its destination"))?;
    let directory = destination.get("directory").and_then(|value| value.get("path")).and_then(json::Value::as_str).ok_or_else(|| eyre!("native destination lacks its path"))?;
    let basename = destination.get("basename").and_then(json::Value::as_str).ok_or_else(|| eyre!("native destination lacks its basename"))?;
    if plan.get("schema").and_then(json::Value::as_str) != Some("iroha.taira.native-nginx-apply.plan.v1") || plan.get("provider").and_then(json::Value::as_str) != Some("macstadium-dublin") || plan.get("host_kind").and_then(json::Value::as_str) != Some("macos") || candidate.get("path").and_then(json::Value::as_str) != Some(selected.remote_path.as_str()) || candidate.get("sha256").and_then(json::Value::as_str) != Some(selected.sha256.as_str()) || Path::new(directory).join(basename) != Path::new(&edge.nginx_configuration) { return Err(eyre!("native plan is not joined to the signed edge renderer artifact and installed destination")); }
    plan_operation(plan)?;
    Ok(())
}

fn cutover(admitted: &HostAdmission, edge: &EdgeV1, capsule: &NativeCapsule, operation: &mut NativeOperation) -> Result<()> {
    if !matches!(operation.progress.status.as_str(), "staged" | "cutover_requested" | "awaiting_readiness" | "edge_ready_unqualified") { return Err(eyre!("native cutover lacks its durable staged frontier")); }
    let mut plan = load_apply_plan(edge)?;
    admit_apply_plan(&plan, edge)?;
    let publication = plan_operation(&plan)?;
    if operation.progress.publication_operation_id.as_ref() != Some(&publication) { return Err(eyre!("native publication operation drifted from durable staging")); }
    if operation.progress.status != "staged" { plan = reconcile_plan(&plan)?; }
    operation.commit(admitted, "cutover_requested", Some(publication.clone()), operation.progress.checkpoint_sha256.clone(), None)?;
    let output = capsule.run(&operation.root, "apply", json::to_json(&plan)?.as_bytes(), &[], admitted.action_deadline)?;
    let result: json::Value = json::from_slice(&output)?;
    if result.get("exit_code").and_then(json::Value::as_u64) != Some(0) || result.get("operation_id").and_then(json::Value::as_str) != Some(publication.as_str()) || result.get("qualified").and_then(json::Value::as_bool) != Some(false) || result.get("phase").and_then(json::Value::as_str) != Some("awaiting_readiness") { return Err(eyre!("native publication has not completed its exact unqualified cutover")); }
    retain_exact(&operation.root, "cutover-receipt.json", &output)?;
    let reconciled = reconcile_plan(&plan)?;
    let completion = protocol::NativeNginxCompletionPlanV1 { schema: "iroha.taira.public-reset.native-nginx-completion-plan.v1".into(), nginx: reconciled, completion_journal_basename: "native-completion.ndjson".into() };
    completion.validate(&publication)?;
    retain_exact(&operation.root, "completion-plan.json", json::to_json(&completion)?.as_bytes())?;
    operation.commit(admitted, "awaiting_readiness", Some(publication), operation.progress.checkpoint_sha256.clone(), None)
}

fn verify_ready(admitted: &HostAdmission, edge: &EdgeV1, capsule: &NativeCapsule, operation: &mut NativeOperation) -> Result<()> {
    if !matches!(operation.progress.status.as_str(), "awaiting_readiness" | "edge_ready_unqualified") { return Err(eyre!("native readiness lacks its owned cutover")); }
    let completion: protocol::NativeNginxCompletionPlanV1 = json::from_slice(&operation.root.read("completion-plan.json", MAX_PLAN)?)?;
    let publication = operation.progress.publication_operation_id.clone().ok_or_else(|| eyre!("native readiness lacks its publication owner"))?;
    completion.validate(&publication)?;
    let request = inspection_request(&completion.nginx)?;
    let _observation = capsule.run(&operation.root, "inspect", json::to_json(&request)?.as_bytes(), &[], admitted.action_deadline)?;
    let plan = protocol::read_native_public(&edge.native_capability.forwarding_plan, admitted.inventory.hosts.native_edge.owner_uid, MAX_PLAN)?;
    let identity = protocol::read_native_public(&edge.native_capability.forwarding_identity_receipt, admitted.inventory.hosts.native_edge.owner_uid, MAX_PLAN)?;
    let request = json::json!({ "plan": json::from_slice::<json::Value>(&plan)?, "identity_receipt": json::from_slice::<json::Value>(&identity)? });
    let _forwarding = capsule.run(&operation.root, "forwarding", json::to_json(&request)?.as_bytes(), &[], admitted.action_deadline)?;
    operation.commit(admitted, "edge_ready_unqualified", Some(publication), operation.progress.checkpoint_sha256.clone(), None)
}

fn native_identity_json(identity: &host_pair::NativeFileIdentityV1) -> json::Value {
    json::json!({ "device": identity.device.to_string(), "inode": identity.inode.to_string(), "uid": identity.uid.to_string(), "gid": identity.gid.to_string(), "mode": identity.mode.to_string(), "links": identity.links.to_string(), "size": identity.size.to_string(), "mtime_ns": identity.mtime_ns.to_string(), "ctime_ns": identity.ctime_ns.to_string() })
}

fn owner_reference(pin: &PublicPin) -> json::Value {
    json::json!({ "path": pin.reference.file.path, "identity": native_identity_json(&pin.reference.file.identity), "sha256": pin.reference.sha256 })
}

fn reconcile_plan(plan: &json::Value) -> Result<json::Value> {
    let mut result = plan.clone();
    let op = plan_operation(plan)?;
    let destination = plan.get("destination").ok_or_else(|| eyre!("native plan destination missing"))?;
    let directory = destination.get("directory").and_then(|value| value.get("path")).and_then(json::Value::as_str).ok_or_else(|| eyre!("native plan directory missing"))?;
    let basename = destination.get("basename").and_then(json::Value::as_str).ok_or_else(|| eyre!("native plan basename missing"))?;
    let journal = PublicPin::open(&Path::new(directory).join(format!(".taira-native-nginx-apply-{op}.receipt.ndjson")), MAX_PLAN as u64, true)?;
    let publication = PublicPin::open(&Path::new(directory).join(basename), MAX_PLAN as u64, false)?;
    result.as_object_mut().ok_or_else(|| eyre!("native plan object missing"))?.insert("publication".into(), json::json!({ "kind": "reconcile", "prior": { "operation_id": op, "journal": owner_reference(&journal), "publication": owner_reference(&publication) } }));
    Ok(result)
}

fn inspection_request(plan: &json::Value) -> Result<json::Value> {
    let path = plan.get("candidate").and_then(|value| value.get("path")).and_then(json::Value::as_str).ok_or_else(|| eyre!("native candidate path missing"))?;
    let mut candidate = PublicPin::open(Path::new(path), MAX_PLAN as u64, false)?;
    let bytes = candidate.bytes(MAX_PLAN)?;
    Ok(json::json!({ "host_kind": plan.get("host_kind").ok_or_else(|| eyre!("native host kind missing"))?, "native": plan.get("native").ok_or_else(|| eyre!("native plan metadata missing"))?, "candidate_sha256": candidate.reference.sha256, "candidate_base64": BASE64.encode(bytes), "renderer_source_sha256": plan.get("renderer_source").and_then(|value| value.get("sha256")).ok_or_else(|| eyre!("renderer SHA missing"))?, "master": plan.get("master").ok_or_else(|| eyre!("native master missing"))?, "destination": plan.get("destination").ok_or_else(|| eyre!("native destination missing"))?, "operation_id": plan_operation(plan)?, "publication": plan.get("publication").ok_or_else(|| eyre!("native publisher missing"))? }))
}

fn complete(admitted: &HostAdmission, action: HostAction, _edge: &EdgeV1, _capsule: &NativeCapsule, _operation: &mut NativeOperation) -> Result<()> {
    let _ = (admitted, action);
    Err(eyre!("native completion receiver is not yet source-qualified"))
}
