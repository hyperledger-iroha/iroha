//! Native Darwin edge receiver inside the same authenticated host-request corridor.
//!
//! Guest artifacts are never executed here. Native release files are prepared by
//! the signed Mac producer and streamed uploads certify their exact public bytes.
//! The persistent forwarding service remains owned by its original publisher.

use super::super::{host_pair, native_edge_protocol as protocol, validate_lower_hex};
use super::*;
use host_pair::{NativeObservedFileV1, NativePublicFileV1, SignedHostPhaseV1};
use iroha_fs::{PrivateDirectory, PublishMode, ReaderDirectory, RetainedFile};

const MAX_PLAN: usize = 1024 * 1024;

#[path = "taira_public_reset_native_custody.rs"]
mod native_custody;
pub(in crate::taira_public_reset) use native_custody::{
    InitializeNativeEdgeCustody, initialize_native_edge_custody,
};

#[path = "taira_public_reset_native_python.rs"]
mod native_python;
pub(in crate::taira_public_reset) use native_python::{CaptureNativePython, capture_native_python};

#[path = "taira_public_reset_native_edge_adoption.rs"]
mod adoption;
pub(in crate::taira_public_reset) use adoption::{
    AdoptNativeEdgeOwner, AuthorizeNativeEdgeOwner, PrepareNativeEdgeOwner,
};
pub(in crate::taira_public_reset) use adoption::{
    adopt as adopt_native_edge_owner, authorize as authorize_native_edge_owner,
    prepare_native_edge_owner,
};

/// The native OS account selects the guard; no inventory path or HOME value can do so.
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "getpwuid_r is the native Darwin bounded account-home authority"
)]
fn native_account() -> Result<(u32, u32, String, String)> {
    #[repr(C)]
    struct Passwd {
        name: *mut std::ffi::c_char,
        password: *mut std::ffi::c_char,
        uid: u32,
        gid: u32,
        change: i64,
        class: *mut std::ffi::c_char,
        gecos: *mut std::ffi::c_char,
        directory: *mut std::ffi::c_char,
        shell: *mut std::ffi::c_char,
        expire: i64,
    }
    unsafe extern "C" {
        fn getpwuid_r(
            uid: u32,
            record: *mut Passwd,
            buffer: *mut std::ffi::c_char,
            length: usize,
            result: *mut *mut Passwd,
        ) -> std::ffi::c_int;
    }
    let uid = rustix::process::geteuid().as_raw();
    let mut record = std::mem::MaybeUninit::<Passwd>::uninit();
    let mut buffer = zeroize::Zeroizing::new(vec![0_u8; 64 * 1024]);
    let mut result = std::ptr::null_mut();
    let code = unsafe {
        getpwuid_r(
            uid,
            record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &raw mut result,
        )
    };
    if code != 0 || result != record.as_mut_ptr() {
        return Err(eyre!("native OS account lookup failed"));
    }
    let record = unsafe { record.assume_init() };
    let bounded = |pointer: *mut std::ffi::c_char| -> Result<String> {
        let offset = (pointer as usize)
            .checked_sub(buffer.as_ptr() as usize)
            .filter(|offset| *offset < buffer.len())
            .ok_or_else(|| eyre!("native account field escaped its bounded OS buffer"))?;
        let source = &buffer[offset..];
        let end = source
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| eyre!("native account field lacks a bounded terminator"))?;
        Ok(std::str::from_utf8(&source[..end])?.into())
    };
    let name = bounded(record.name)?;
    let home = bounded(record.directory)?;
    if uid == 0
        || record.uid != uid
        || record.gid != rustix::process::getegid().as_raw()
        || name.is_empty()
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
        || home != format!("/Users/{name}")
    {
        return Err(eyre!(
            "native edge requires its canonical named Mac OS account"
        ));
    }
    Ok((uid, record.gid, name, home))
}

#[cfg(not(target_os = "macos"))]
fn native_account() -> Result<(u32, u32, String, String)> {
    Err(eyre!(
        "native edge custody requires the Darwin OS account authority"
    ))
}

pub(super) fn canonical_guard_path(host: &host_pair::ResetHostV1) -> Result<PathBuf> {
    let (uid, gid, user, home) = native_account()?;
    let custody = Path::new(&home).join(".local/share/iroha/taira/public-reset-v1");
    let dispatcher = custody.join("dispatcher/iroha");
    if uid != host.owner_uid
        || gid != host.owner_gid
        || user != host.endpoint.user
        || home != host.owner_home
        || custody != Path::new(&host.custody_root)
        || dispatcher != Path::new(&host.dispatcher_path)
        || std::env::current_exe()? != dispatcher
    {
        return Err(eyre!(
            "incoming native host cannot choose the independently provisioned custodian"
        ));
    }
    Ok(custody.join("taira-edge/guard.json"))
}
const SOURCES: [(&str, &[u8]); 4] = [
    (
        "taira_native_nginx_check.py",
        include_bytes!("../../../scripts/taira_native_nginx_check.py"),
    ),
    (
        "taira_native_nginx_apply.py",
        include_bytes!("../../../scripts/taira_native_nginx_apply.py"),
    ),
    (
        "taira_native_validator_forwarding.py",
        include_bytes!("../../../scripts/taira_native_validator_forwarding.py"),
    ),
    (
        "taira_native_edge_completion.py",
        include_bytes!("../../../scripts/taira_native_edge_completion.py"),
    ),
];
const BOOTSTRAP: &str = r#"import json, os, sys, types
root, action, input_fd = sys.argv[1], sys.argv[2], int(sys.argv[3])
runtime, runtime_fd = json.loads(sys.argv[4]), int(sys.argv[5])
if list(sys.version_info[:3]) != runtime['version'] or sys.version_info[:2] < (3,11):
    raise RuntimeError('native_python_version_changed')
names = ('taira_native_nginx_check','taira_native_nginx_apply','taira_native_validator_forwarding','taira_native_edge_completion')
for name, raw_fd in zip(names, sys.argv[6:], strict=True):
    fd = int(raw_fd)
    source = os.pread(fd, 1048577, 0)
    if not source or len(source) > 1048576 or len(source) != os.fstat(fd).st_size: raise RuntimeError('source_bound')
    module = types.ModuleType(name)
    module.__file__ = root + '/' + name + '.py'
    sys.modules[name] = module
    exec(compile(source, module.__file__, 'exec'), module.__dict__)
native = sys.modules['taira_native_edge_completion']
image = runtime['executable']
def verify_runtime():
    if sys.platform != 'darwin' or native.kernel_executable_path(os.getpid()) != image['file']['path']:
        raise RuntimeError('native_python_image_changed')
    if native.identity(os.fstat(runtime_fd)) != image['file']['identity'] or native.identity(os.stat(image['file']['path'], follow_symlinks=False)) != image['file']['identity']:
        raise RuntimeError('native_python_identity_changed')
verify_runtime()
import hashlib
digest, offset = hashlib.sha256(), 0
while offset < image['file']['identity']['size']:
    chunk = os.pread(runtime_fd, min(65536, image['file']['identity']['size'] - offset), offset)
    if not chunk: raise RuntimeError('native_python_extent_changed')
    digest.update(chunk)
    offset += len(chunk)
if digest.hexdigest() != image['sha256']: raise RuntimeError('native_python_digest_changed')
verify_runtime()
body = os.pread(input_fd, 1048577, 0)
if not body or len(body) > 1048576 or len(body) != os.fstat(input_fd).st_size: raise RuntimeError('request_bound')
request = json.loads(body)
owner = sys.modules['taira_native_nginx_apply']
if action == 'python-preflight': result = {}
elif action == 'apply': result = owner.apply_owned_publication(request)
elif action == 'recover': result = owner.reconcile_interrupted_publication(request['original_plan'], request['journal'], request['direction'])
elif action == 'inspect': result = owner.inspect_owned_publication(request)
elif action == 'validate-plan':
    owner.validate_plan(request)
    result = {}
elif action == 'validate-owner-inputs':
    owner.validate_plan(request)
    def check_existing(receipt, plan, context):
        def guard():
            for key, opened in context['bound'].items():
                context['revalidate'](plan['native'][key], opened)
            context['revalidate'](plan['native']['directory'], context['directory'], True)
            if owner.observe_master(plan['master']) != plan['master']:
                raise RuntimeError('master_identity_changed')
            if context['identity'](os.fstat(context['lock'])) != context['lock_identity']:
                raise RuntimeError('check_lock_identity_changed')
            if context['identity'](os.stat('.taira-native-nginx-check.lock', dir_fd=context['directory'], follow_symlinks=False)) != context['lock_identity']:
                raise RuntimeError('check_lock_identity_changed')
        guard()
        sys.modules['taira_native_edge_completion'].native_context_check(context, plan, owner, includes=True)
        guard()
        receipt['exit_code'] = 0
    result = owner.adopt_owned_publication(request, check_existing)
    if result.get('exit_code') != 0 or result.get('validation_files_removed') is not True:
        raise RuntimeError('native_owner_inputs_refused')
    result = {}
elif action == 'forwarding': result = sys.modules['taira_native_validator_forwarding'].inspect_mac_forwarding(request['plan'], request['identity_receipt'])
elif action == 'complete': result = sys.modules['taira_native_edge_completion'].complete(request)
elif action == 'adopt': result = sys.modules['taira_native_edge_completion'].adopt(request)
else: raise RuntimeError('native_action')
verify_runtime()
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
        let retained = if private {
            RetainedFile::open_private(path)?
        } else {
            RetainedFile::open_regular(path)?
        };
        Self::from_retained(path, retained, maximum)
    }

    fn private_child(directory: &PrivateDirectory, name: &str, maximum: u64) -> Result<Self> {
        Self::from_retained(
            &directory.path().join(name),
            directory.open_retained_private(name)?,
            maximum,
        )
    }

    fn reader_child(
        directory: &ReaderDirectory,
        name: &OsStr,
        maximum: u64,
        private: bool,
    ) -> Result<Self> {
        let retained = if private {
            directory.open_retained_private(name)?
        } else {
            directory.open_retained_regular(name)?
        };
        Self::from_retained(&directory.path().join(name), retained, maximum)
    }

    fn from_retained(path: &Path, mut retained: RetainedFile, maximum: u64) -> Result<Self> {
        let before = protocol::native_file_identity(&retained.file().metadata()?)?;
        if before.size == 0 || before.size > maximum {
            return Err(eyre!("native public input exceeds its phase bound"));
        }
        let digest = hash_reader(
            &mut retained.file_mut().take(
                before
                    .size
                    .checked_add(1)
                    .ok_or_else(|| eyre!("native digest extent overflow"))?,
            ),
        )?;
        retained.file_mut().rewind()?;
        if protocol::native_file_identity(&retained.file().metadata()?)? != before {
            return Err(eyre!("native public input changed during bounded digest"));
        }
        retained.revalidate()?;
        Ok(Self {
            retained,
            reference: NativePublicFileV1 {
                file: NativeObservedFileV1 {
                    path: path.to_string_lossy().into_owned(),
                    identity: before,
                },
                sha256: digest,
            },
        })
    }

    fn expected(reference: &NativePublicFileV1, private: bool) -> Result<Self> {
        let pin = Self::open(
            Path::new(&reference.file.path),
            reference.file.identity.size,
            private,
        )?;
        if &pin.reference != reference {
            return Err(eyre!(
                "native public input differs from its authenticated file reference"
            ));
        }
        Ok(pin)
    }

    fn expected_child(
        directory: &ReaderDirectory,
        reference: &NativePublicFileV1,
        private: bool,
    ) -> Result<Self> {
        let path = Path::new(&reference.file.path);
        if path.parent() != Some(directory.path()) {
            return Err(eyre!(
                "native public child selected another retained directory"
            ));
        }
        let name = path
            .file_name()
            .ok_or_else(|| eyre!("native public child basename missing"))?;
        let pin = Self::reader_child(directory, name, reference.file.identity.size, private)?;
        if &pin.reference != reference {
            return Err(eyre!(
                "native public child differs from its authenticated file reference"
            ));
        }
        Ok(pin)
    }

    fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>> {
        if self.reference.file.identity.size > u64::try_from(maximum)? {
            return Err(eyre!(
                "native public input exceeds the selected decoder extent"
            ));
        }
        self.revalidate()?;
        self.retained.file_mut().rewind()?;
        let mut bytes = Vec::with_capacity(usize::try_from(self.reference.file.identity.size)?);
        self.retained
            .file_mut()
            .take(
                u64::try_from(maximum)?
                    .checked_add(1)
                    .ok_or_else(|| eyre!("native input bound overflow"))?,
            )
            .read_to_end(&mut bytes)?;
        if bytes.len() > maximum
            || u64::try_from(bytes.len())? != self.reference.file.identity.size
            || sha256_hex(&bytes) != self.reference.sha256
        {
            return Err(eyre!("native input content changed"));
        }
        self.revalidate()?;
        Ok(bytes)
    }

    fn revalidate(&self) -> Result<()> {
        self.retained.revalidate()?;
        if protocol::native_file_identity(&self.retained.file().metadata()?)?
            != self.reference.file.identity
        {
            return Err(eyre!("retained native public snapshot changed"));
        }
        Ok(())
    }

    fn inherited(&self) -> Result<protocol::RetainedPublicRefV1> {
        self.revalidate()?;
        Ok(protocol::RetainedPublicRefV1 {
            fd: u32::try_from(self.retained.file().as_raw_fd())?,
            reference: self.reference.clone(),
        })
    }
}

struct NativeCapsule {
    root: PrivateDirectory,
    sources: Vec<PublicPin>,
    runtime: PublicPin,
    runtime_capability: host_pair::NativePythonRuntimeV1,
}

impl NativeCapsule {
    fn admit(admitted: &HostAdmission) -> Result<Self> {
        Self::admit_host(&admitted.inventory.hosts.native_edge)
    }

    fn admit_host(host: &host_pair::ResetHostV1) -> Result<Self> {
        let runtime_capability = host.native_python()?.clone();
        runtime_capability.validate(host.owner_uid)?;
        let runtime = PublicPin::expected(&runtime_capability.executable, false)?;
        let custody = PrivateDirectory::open(&host.custody_root)?;
        let helpers = custody.ensure_child("helpers")?;
        let name = host_pair::helper_source_closure_sha256();
        let root = helpers.ensure_child(&name)?;
        let mut sources = Vec::with_capacity(SOURCES.len());
        for (filename, source) in SOURCES {
            match root.read(filename, MAX_PLAN) {
                Ok(existing) if existing.as_slice() == source => {}
                Ok(_) => return Err(eyre!("native helper capsule differs from embedded source")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    root.write_atomic(filename, source, PublishMode::CreateNew)?
                }
                Err(error) => return Err(error.into()),
            }
            let pin = PublicPin::private_child(&root, filename, MAX_PLAN as u64)?;
            if pin.reference.sha256 != sha256_hex(source) {
                return Err(eyre!("native helper publication changed"));
            }
            sources.push(pin);
        }
        root.sync()?;
        custody.revalidate()?;
        helpers.revalidate()?;
        let capsule = Self {
            root,
            sources,
            runtime,
            runtime_capability,
        };
        let ready = capsule.run(
            &capsule.root,
            "python-preflight",
            b"{}",
            &[],
            Instant::now() + Duration::from_secs(20),
        )?;
        if json::from_slice::<json::Value>(&ready)? != norito::json!({}) {
            return Err(eyre!("native Python preflight returned another capability"));
        }
        Ok(capsule)
    }

    fn run(
        &self,
        directory: &PrivateDirectory,
        action: &str,
        body: &[u8],
        inherited: &[&File],
        deadline: Instant,
    ) -> Result<Vec<u8>> {
        if body.is_empty() || body.len() > MAX_PLAN {
            return Err(eyre!("native helper request exceeds its finite body bound"));
        }
        let mut inherited_files = Vec::with_capacity(self.sources.len() + inherited.len() + 1);
        self.runtime.revalidate()?;
        let runtime_file = self.runtime.retained.file().try_clone()?;
        for source in &self.sources {
            source.revalidate()?;
            inherited_files.push(source.retained.file().try_clone()?);
        }
        let mut descriptor_map = BTreeMap::new();
        for file in inherited {
            let clone = file.try_clone()?;
            descriptor_map.insert(
                u32::try_from(file.as_raw_fd())?,
                u32::try_from(clone.as_raw_fd())?,
            );
            inherited_files.push(clone);
        }
        let bytes = if matches!(action, "complete" | "adopt") {
            let mut packet: json::Value = json::from_slice(body)?;
            rebase_descriptors(&mut packet, &descriptor_map)?;
            json::to_json(&packet)?.into_bytes()
        } else {
            body.to_vec()
        };
        if matches!(action, "complete" | "adopt")
            && bytes.len() > protocol::MAX_COMPLETION_ADMISSION_BYTES
        {
            return Err(eyre!("native completion admission exceeds 64KiB"));
        }
        // The request capsule is software custody, outside the immutable native
        // operation directory and its fence/held-directory snapshot.
        let requests = self.root.ensure_child("requests")?;
        let request_owner =
            requests.ensure_child(sha256_hex(directory.path().as_os_str().as_encoded_bytes()))?;
        let name = format!("{action}.json");
        request_owner.write_atomic(&name, &bytes, PublishMode::Replace)?;
        request_owner.sync()?;
        let request = PublicPin::private_child(&request_owner, &name, MAX_PLAN as u64)?;
        let request_file = request.retained.file().try_clone()?;
        let mut args = vec![
            OsString::from("-B"),
            OsString::from("-I"),
            OsString::from("-c"),
            OsString::from(BOOTSTRAP),
            self.root.path().into(),
            action.into(),
            request_file.as_raw_fd().to_string().into(),
            json::to_json(&self.runtime_capability)?.into(),
            runtime_file.as_raw_fd().to_string().into(),
        ];
        for source in &inherited_files[..self.sources.len()] {
            args.push(source.as_raw_fd().to_string().into());
        }
        inherited_files.push(request_file);
        inherited_files.push(runtime_file);
        self.runtime.revalidate()?;
        self.root.revalidate()?;
        let output = require_success(
            RealProcessRunner.run(&ProcessSpec {
                program: PathBuf::from(&self.runtime.reference.file.path),
                args,
                stdin_prefix: Vec::new(),
                stdin_file: None,
                stdin_files: Vec::new(),
                inherited_files,
                deadline,
            })?,
            "native edge owner helper",
        )?;
        for source in &self.sources {
            source.revalidate()?;
        }
        request.revalidate()?;
        self.runtime.revalidate()?;
        self.root.revalidate()?;
        directory.revalidate()?;
        if output.is_empty() || output.len() > 64 * 1024 {
            return Err(eyre!(
                "native helper result exceeds its finite public bound"
            ));
        }
        Ok(output)
    }
}

fn retain_exact(directory: &PrivateDirectory, filename: &str, body: &[u8]) -> Result<()> {
    match directory.read(filename, MAX_PLAN) {
        Ok(existing) if existing.as_slice() == body => {}
        Ok(_) => {
            return Err(eyre!(
                "native immutable owner file differs from its admitted body"
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            directory.write_atomic(filename, body, PublishMode::CreateNew)?
        }
        Err(error) => return Err(error.into()),
    }
    directory.sync()?;
    directory.revalidate()?;
    Ok(())
}

/// All packet descriptors name the exact handles whose CLOEXEC flag the
/// supervised runner clears; there is no numeric alias or ambient-FD fallback.
fn rebase_descriptors(value: &mut json::Value, descriptors: &BTreeMap<u32, u32>) -> Result<()> {
    match value {
        json::Value::Object(object) => {
            for (name, child) in object {
                if name == "fd" || name == "directory_fd" {
                    let old =
                        u32::try_from(child.as_u64().ok_or_else(|| {
                            eyre!("native descriptor is not an unsigned integer")
                        })?)?;
                    let new = descriptors
                        .get(&old)
                        .ok_or_else(|| eyre!("native descriptor has no retained source handle"))?;
                    *child = json::Value::from(u64::from(*new));
                } else {
                    rebase_descriptors(child, descriptors)?;
                }
            }
        }
        json::Value::Array(array) => {
            for child in array {
                rebase_descriptors(child, descriptors)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct NativeOperation {
    root: PrivateDirectory,
    lock: File,
    lock_identity: host_pair::NativeFileIdentityV1,
    host_root: PrivateDirectory,
    host_lock: File,
    host_lock_identity: host_pair::NativeFileIdentityV1,
    lease_sha256: String,
    progress: protocol::NativeEdgeProgressV1,
}

impl NativeOperation {
    fn acquire(admitted: &HostAdmission) -> Result<Self> {
        let native = &admitted.inventory.hosts.native_edge;
        let custody = PrivateDirectory::open(&native.custody_root)?;
        let edge = custody.ensure_child("taira-edge")?;
        let host_lock = edge.open_lock("host-operation.lock")?;
        host_lock
            .try_lock()
            .wrap_err("another native edge operation owns the host")?;
        let host_lock_identity = protocol::native_file_identity(&host_lock.metadata()?)?;
        if host_lock_identity.uid != native.owner_uid
            || host_lock_identity.gid != native.owner_gid
            || host_lock_identity.mode != 0o600
            || host_lock_identity.links != 1
            || host_lock_identity.size != 0
        {
            return Err(eyre!("native host ownership lock has unsafe custody"));
        }
        require_path_absent(
            &edge.path().join("active-adoption.json"),
            "unresolved native publication adoption",
        )?;
        let operations = edge.ensure_child("operations")?;
        let root = operations.ensure_child(&admitted.authorization_sha256)?;
        let lock = root.open_lock("operation.lock")?;
        lock.try_lock()
            .wrap_err("native edge operation is already running")?;
        let lock_identity = protocol::native_file_identity(&lock.metadata()?)?;
        if lock_identity.uid != native.owner_uid
            || lock_identity.gid != native.owner_gid
            || lock_identity.mode != 0o600
            || lock_identity.links != 1
            || lock_identity.size != 0
        {
            return Err(eyre!("native edge operation lock custody changed"));
        }
        retain_exact(
            &root,
            "inventory.json",
            &BASE64.decode(&admitted.request.inventory_base64)?,
        )?;
        retain_exact(
            &root,
            "authorization.json",
            &BASE64.decode(&admitted.request.authorization_base64)?,
        )?;
        retain_exact(
            &root,
            "trusted-key.json",
            &BASE64.decode(&admitted.request.trusted_key_base64)?,
        )?;
        let progress = match root.read("progress.json", protocol::MAX_PROGRESS_BYTES) {
            Ok(bytes) => {
                let progress: protocol::NativeEdgeProgressV1 = json::from_slice(&bytes)?;
                progress.validate(
                    &admitted.inventory,
                    &admitted.inventory_sha256,
                    &admitted.authorization_sha256,
                )?;
                if progress.digest()? != sha256_hex(&bytes) {
                    return Err(eyre!("native progress is not canonical"));
                }
                progress
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && !admitted.execution_expired =>
            {
                let progress = protocol::NativeEdgeProgressV1 {
                    schema: protocol::PROGRESS_SCHEMA.into(),
                    operation_id: hex::encode(rand::random::<[u8; 16]>()),
                    inventory_sha256: admitted.inventory_sha256.clone(),
                    authorization_sha256: admitted.authorization_sha256.clone(),
                    authorization_nonce: admitted.inventory.authorization_nonce.clone(),
                    host_pair_sha256: admitted.inventory.hosts.digest()?,
                    host_identity_sha256: native.endpoint.host_identity_sha256.clone(),
                    custody_root: native.custody_root.clone(),
                    sequence: 1,
                    predecessor_sha256: None,
                    request_sha256: admitted.request_sha256.clone(),
                    publication_operation_id: Some(plan_operation(&load_apply_plan(
                        &admitted.inventory.edge,
                    )?)?),
                    status: "admitted".into(),
                    checkpoint_sha256: None,
                    completion_receipt_sha256: None,
                };
                root.write_atomic(
                    "progress.json",
                    json::to_json(&progress)?.as_bytes(),
                    PublishMode::CreateNew,
                )?;
                root.sync()?;
                progress
            }
            Err(error) => {
                return Err(error)
                    .wrap_err("native edge recovery requires its original durable lease");
            }
        };
        // The actual signed incumbent is the only authority for replacing a
        // previous lease. An unresolved operation is never silently displaced.
        let lease = HostLeaseV1 {
            schema: LEASE_SCHEMA_V1.into(),
            inventory_sha256: admitted.inventory_sha256.clone(),
            authorization_semantic_sha256: admitted.authorization_sha256.clone(),
            authorization_nonce: admitted.inventory.authorization_nonce.clone(),
            execution_expires_at_unix_ms: admitted
                .authorization
                .claims
                .execution_expires_at_unix_ms,
        };
        match edge.read("active-lease.json", 64 * 1024) {
            Ok(bytes) => {
                let previous: HostLeaseV1 = json::from_slice(&bytes)?;
                if json::to_json(&previous)? != json::to_json(&lease)? {
                    admit_previous_terminal(admitted, &previous, &operations)?;
                    edge.write_atomic(
                        "active-lease.json",
                        json::to_json(&lease)?.as_bytes(),
                        PublishMode::Replace,
                    )?;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && !admitted.execution_expired =>
            {
                edge.write_atomic(
                    "active-lease.json",
                    json::to_json(&lease)?.as_bytes(),
                    PublishMode::CreateNew,
                )?
            }
            Err(error) => return Err(error).wrap_err("native host durable lease is unavailable"),
        }
        edge.sync()?;
        let mut operation = Self {
            root,
            lock,
            lock_identity,
            host_root: edge,
            host_lock,
            host_lock_identity,
            lease_sha256: sha256_hex(json::to_json(&lease)?.as_bytes()),
            progress,
        };
        operation.retain_checkpoints(admitted)?;
        Ok(operation)
    }

    fn retain_checkpoints(&mut self, admitted: &HostAdmission) -> Result<()> {
        for (index, checkpoint) in admitted.request.phase_checkpoints.iter().enumerate() {
            retain_exact(
                &self.root,
                &format!("checkpoint-{}.json", index + 1),
                json::to_json(checkpoint)?.as_bytes(),
            )?;
        }
        Ok(())
    }

    fn revalidate(&self) -> Result<()> {
        self.host_root.revalidate()?;
        if protocol::native_file_identity(&self.host_lock.metadata()?)? != self.host_lock_identity
            || protocol::native_file_identity(&fs::symlink_metadata(
                self.host_root.path().join("host-operation.lock"),
            )?)? != self.host_lock_identity
            || sha256_hex(&self.host_root.read("active-lease.json", 64 * 1024)?)
                != self.lease_sha256
        {
            return Err(eyre!("native physical-host lock or durable lease changed"));
        }
        self.root.revalidate()?;
        let path = self.root.path().join("operation.lock");
        if protocol::native_file_identity(&self.lock.metadata()?)? != self.lock_identity
            || protocol::native_file_identity(&fs::symlink_metadata(path)?)? != self.lock_identity
        {
            return Err(eyre!(
                "native operation lock pathname or descriptor changed"
            ));
        }
        let body = self
            .root
            .read("progress.json", protocol::MAX_PROGRESS_BYTES)?;
        if sha256_hex(&body) != self.progress.digest()? {
            return Err(eyre!(
                "native progress changed beside its retained operation"
            ));
        }
        Ok(())
    }

    fn commit(
        &mut self,
        admitted: &HostAdmission,
        status: &str,
        publication: Option<String>,
        checkpoint: Option<String>,
        receipt: Option<String>,
    ) -> Result<()> {
        self.revalidate()?;
        let mut next = self.progress.clone();
        next.sequence = next
            .sequence
            .checked_add(1)
            .ok_or_else(|| eyre!("native progress sequence overflow"))?;
        next.predecessor_sha256 = Some(self.progress.digest()?);
        next.request_sha256 = admitted.request_sha256.clone();
        next.status = status.into();
        next.publication_operation_id = publication;
        next.checkpoint_sha256 = checkpoint;
        next.completion_receipt_sha256 = receipt;
        next.validate(
            &admitted.inventory,
            &admitted.inventory_sha256,
            &admitted.authorization_sha256,
        )?;
        self.root.write_atomic(
            "progress.json",
            json::to_json(&next)?.as_bytes(),
            PublishMode::Replace,
        )?;
        self.root.sync()?;
        self.progress = next;
        self.revalidate()
    }
}

fn admit_previous_terminal(
    admitted: &HostAdmission,
    lease: &HostLeaseV1,
    operations: &PrivateDirectory,
) -> Result<()> {
    let edge = &admitted.inventory.edge;
    let capture = &edge.native_capability.incumbent;
    if lease.schema != LEASE_SCHEMA_V1
        || capture.claims.retained_inventory_sha256 != lease.inventory_sha256
        || capture.claims.authorization_sha256 != lease.authorization_semantic_sha256
        || capture.claims.authorization_nonce != lease.authorization_nonce
    {
        return Err(eyre!("native host retains another unresolved lease"));
    }
    let host_pair::NativeEdgeCompletionProvenanceV1::ResetTerminal { status, .. } =
        &capture.claims.completion
    else {
        return Err(eyre!(
            "native host replacement requires actual distributed terminal custody"
        ));
    };
    if !matches!(status.as_str(), "cleaned" | "rolled_back") {
        return Err(eyre!("native predecessor cleanup has not completed"));
    }
    let previous = operations.open_child(&lease.authorization_semantic_sha256)?;
    let inventory_bytes = previous.read("inventory.json", super::super::MAX_JSON_BYTES as usize)?;
    let (inventory, _chain_guard) =
        super::super::decode_inventory(&inventory_bytes, "native predecessor inventory")?;
    let auth: AuthorizationEnvelopeV1 = json::from_slice(
        &previous.read("authorization.json", super::super::MAX_JSON_BYTES as usize)?,
    )?;
    let trusted: TrustedKeyV1 = json::from_slice(
        &previous.read("trusted-key.json", super::super::MAX_JSON_BYTES as usize)?,
    )?;
    if sha256_hex(&inventory_bytes) != lease.inventory_sha256
        || authorization_semantic_sha256(&auth, &trusted)? != lease.authorization_semantic_sha256
    {
        return Err(eyre!("native predecessor lease signed bytes changed"));
    }
    verify_execution_authorization(
        &inventory,
        &lease.inventory_sha256,
        &auth,
        &trusted,
        auth.claims.not_before_unix_ms,
    )?;
    protocol::validate_terminal_provenance(
        &capture.claims.completion,
        &capture.claims.owned_publication,
        &inventory,
        &lease.inventory_sha256,
        &lease.authorization_semantic_sha256,
        lease.execution_expires_at_unix_ms,
    )?;
    previous
        .revalidate()
        .wrap_err("native predecessor custody changed")
}

pub(super) fn dispatch(
    admitted: &HostAdmission,
    action: HostAction,
    body: &mut impl Read,
) -> Result<HostReceiptV1> {
    let HostTarget::Edge(edge) = &admitted.target else {
        return Err(eyre!("native edge dispatcher requires the Mac role"));
    };
    admit_native_platform(admitted)?;
    edge.native_capability.validate(&admitted.inventory.hosts)?;
    if action == HostAction::Preflight {
        verify_native_artifacts(edge)?;
        return Ok(host_receipt(
            admitted,
            action,
            false,
            0,
            0,
            "native Darwin custody and prepared artifact preflight",
        ));
    }
    if !matches!(
        action,
        HostAction::Upload
            | HostAction::EdgeStage
            | HostAction::EdgeCutover
            | HostAction::EdgeVerify
            | HostAction::Rollback
            | HostAction::Seal
            | HostAction::Cleanup
    ) {
        return Err(eyre!(
            "the native Mac edge cannot execute a Linux validator action"
        ));
    }
    let mut operation = NativeOperation::acquire(admitted)?;
    let capsule = NativeCapsule::admit(admitted)?;
    if action == HostAction::Upload {
        let selected = artifact(&edge.artifacts, &admitted.request.artifact_role)?;
        verify_prepared_upload(selected, body)?;
        return Ok(host_receipt(
            admitted,
            action,
            true,
            0,
            0,
            "exact native prepared artifact stream certified",
        ));
    }
    require_stream_eof(body)?;
    verify_native_artifacts(edge)?;
    match action {
        HostAction::EdgeStage => {
            if !matches!(operation.progress.status.as_str(), "admitted" | "staged") {
                return Err(eyre!("native staging cannot rewind an effected operation"));
            }
            let plan = load_apply_plan(edge)?;
            admit_apply_plan(&plan, edge)?;
            operation.commit(
                admitted,
                "staged",
                Some(plan_operation(&plan)?),
                Some(admitted.request.phase_checkpoints[0].digest()?),
                None,
            )?;
        }
        HostAction::EdgeCutover => cutover(admitted, edge, &capsule, &mut operation)?,
        HostAction::EdgeVerify => verify_ready(admitted, edge, &capsule, &mut operation)?,
        HostAction::Rollback | HostAction::Seal | HostAction::Cleanup => {
            complete(admitted, action, edge, &capsule, &mut operation)?
        }
        _ => unreachable!("closed native action admitted above"),
    }
    operation.revalidate()?;
    Ok(host_receipt(
        admitted,
        action,
        false,
        0,
        0,
        "authenticated native edge custody phase completed",
    ))
}

fn admit_native_platform(admitted: &HostAdmission) -> Result<()> {
    let native = &admitted.inventory.hosts.native_edge;
    if !cfg!(all(target_os = "macos", target_arch = "aarch64"))
        || rustix::process::geteuid().as_raw() != native.owner_uid
        || rustix::process::getegid().as_raw() != native.owner_gid
    {
        return Err(eyre!(
            "native edge requires its actual Darwin/AArch64 named owner"
        ));
    }
    if std::env::current_exe()? != Path::new(&native.dispatcher_path)
        || Path::new(&native.dispatcher_path)
            .parent()
            .and_then(Path::parent)
            != Some(Path::new(&native.custody_root))
    {
        return Err(eyre!(
            "native custodian guard root does not derive from the actual executable"
        ));
    }
    native.native_python()?.validate(native.owner_uid)?;
    PrivateDirectory::open(&native.custody_root)?.revalidate()?;
    Ok(())
}

/// Native captures use the same retained interpreter and four-source capsule as effects.
pub(in crate::taira_public_reset) fn inspect_native_owner_inputs(
    host: &host_pair::ResetHostV1,
    action: &str,
    request: &[u8],
    directory: &Path,
) -> Result<Vec<u8>> {
    if !matches!(action, "inspect" | "validate-plan") {
        return Err(eyre!("native capture selected a mutating capsule action"));
    }
    let (_, _, owner, home) = native_account()?;
    if owner != host.endpoint.user || home != host.owner_home {
        return Err(eyre!("native capture differs from its actual OS owner"));
    }
    let private = PrivateDirectory::open(directory)?;
    let capsule = NativeCapsule::admit_host(host)?;
    capsule.run(
        &private,
        action,
        request,
        &[],
        Instant::now() + Duration::from_secs(60),
    )
}

fn verify_native_artifacts(edge: &EdgeV1) -> Result<()> {
    for selected in &edge.artifacts {
        let pin = PublicPin::open(
            Path::new(&selected.remote_path),
            selected.size,
            selected.mode == 0o600,
        )?;
        if pin.reference.sha256 != selected.sha256
            || pin.reference.file.identity.size != selected.size
            || pin.reference.file.identity.mode != selected.mode
            || selected.target != host_pair::NATIVE_EDGE_TARGET
        {
            return Err(eyre!(
                "native prepared artifact differs from the signed Darwin closure"
            ));
        }
    }
    Ok(())
}

fn verify_prepared_upload(selected: &ArtifactV1, body: &mut impl Read) -> Result<()> {
    let pin = PublicPin::open(
        Path::new(&selected.remote_path),
        selected.size,
        selected.mode == 0o600,
    )?;
    if pin.reference.sha256 != selected.sha256
        || pin.reference.file.identity.size != selected.size
        || pin.reference.file.identity.mode != selected.mode
    {
        return Err(eyre!(
            "native prepared artifact changed before streamed certification"
        ));
    }
    let mut digest = Sha256::new();
    let mut remaining = selected.size;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let capacity = usize::try_from(remaining.min(buffer.len() as u64))?;
        let count = body.read(&mut buffer[..capacity])?;
        if count == 0 {
            return Err(eyre!("native prepared artifact stream was truncated"));
        }
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    require_stream_eof(body)?;
    if hex::encode(digest.finalize()) != selected.sha256 {
        return Err(eyre!("native artifact stream has another source digest"));
    }
    pin.revalidate()
}

fn load_apply_plan(edge: &EdgeV1) -> Result<json::Value> {
    let mut pin = PublicPin::expected(&edge.native_capability.nginx_apply_plan, true)?;
    Ok(json::from_slice(&pin.bytes(MAX_PLAN)?)?)
}

fn plan_operation(plan: &json::Value) -> Result<String> {
    let operation = plan
        .get("operation_id")
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native plan lacks an operation identity"))?;
    validate_lower_hex("native publication operation", operation, 32)?;
    Ok(operation.into())
}

fn admit_apply_plan(plan: &json::Value, edge: &EdgeV1) -> Result<()> {
    let candidate = plan
        .get("candidate")
        .ok_or_else(|| eyre!("native plan lacks its public candidate"))?;
    let selected = artifact(&edge.artifacts, "edge_config")?;
    let destination = plan
        .get("destination")
        .ok_or_else(|| eyre!("native plan lacks its destination"))?;
    let directory = destination
        .get("directory")
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native destination lacks its path"))?;
    let basename = destination
        .get("basename")
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native destination lacks its basename"))?;
    if plan.get("schema").and_then(json::Value::as_str)
        != Some("iroha.taira.native-nginx-apply.plan.v1")
        || plan.get("provider").and_then(json::Value::as_str) != Some("macstadium-dublin")
        || plan.get("host_kind").and_then(json::Value::as_str) != Some("macos")
        || candidate.get("path").and_then(json::Value::as_str)
            != Some(selected.remote_path.as_str())
        || candidate.get("sha256").and_then(json::Value::as_str) != Some(selected.sha256.as_str())
        || Path::new(directory).join(basename) != Path::new(&edge.nginx_config)
    {
        return Err(eyre!(
            "native plan is not joined to the signed edge renderer artifact and installed destination"
        ));
    }
    plan_operation(plan)?;
    Ok(())
}

fn cutover(
    admitted: &HostAdmission,
    edge: &EdgeV1,
    capsule: &NativeCapsule,
    operation: &mut NativeOperation,
) -> Result<()> {
    if !matches!(
        operation.progress.status.as_str(),
        "staged" | "cutover_requested" | "awaiting_readiness" | "edge_ready_unqualified"
    ) {
        return Err(eyre!("native cutover lacks its durable staged frontier"));
    }
    let plan = load_apply_plan(edge)?;
    admit_apply_plan(&plan, edge)?;
    let publication = plan_operation(&plan)?;
    if operation.progress.publication_operation_id.as_ref() != Some(&publication) {
        return Err(eyre!(
            "native publication operation drifted from durable staging"
        ));
    }
    let mut applied_now = false;
    if operation.progress.status == "staged" {
        operation.commit(
            admitted,
            "cutover_requested",
            Some(publication.clone()),
            operation.progress.checkpoint_sha256.clone(),
            None,
        )?;
        let output = capsule.run(
            &operation.root,
            "apply",
            json::to_json(&plan)?.as_bytes(),
            &[&operation.lock, &operation.host_lock],
            admitted.action_deadline,
        )?;
        operation.revalidate()?;
        let result: json::Value = json::from_slice(&output)?;
        if result.get("exit_code").and_then(json::Value::as_u64) != Some(0)
            || result.get("operation_id").and_then(json::Value::as_str)
                != Some(publication.as_str())
            || result.get("qualified").and_then(json::Value::as_bool) != Some(false)
            || result.get("phase").and_then(json::Value::as_str) != Some("awaiting_readiness")
        {
            return Err(eyre!(
                "native publication has not completed its exact unqualified cutover"
            ));
        }
        let witness = admit_journal_write_intent(&result, &plan, true)?;
        retain_exact(
            &operation.root,
            &format!("publisher-apply-receipt-{}.json", sha256_hex(&output)),
            &output,
        )?;
        witness
            .as_ref()
            .expect("successful publisher witness")
            .revalidate()?;
        applied_now = true;
    }
    if !applied_now
        && operation.progress.status == "cutover_requested"
        && !operation
            .root
            .entries(128)?
            .iter()
            .any(|name| name == "completion-plan.json")
    {
        let receipt = recover_publication(admitted, capsule, operation, &plan, "resume")?;
        if receipt.get("phase").and_then(json::Value::as_str) != Some("awaiting_readiness") {
            return Err(eyre!("interrupted cutover did not recover its exact owner"));
        }
    }
    // A completed owner is observed before any retry. In particular, a lost
    // HTTP response or interrupted progress publication never authorizes HUP.
    let completion = match operation.root.read("completion-plan.json", MAX_PLAN) {
        Ok(bytes) => {
            let completion: protocol::NativeNginxCompletionPlanV1 = json::from_slice(&bytes)?;
            completion.validate(&publication)?;
            if !matches!(
                completion.publication_effect,
                protocol::NativePublicationEffectV1::Owned
            ) {
                return Err(eyre!(
                    "native cutover cannot reuse a no-effect rollback plan"
                ));
            }
            completion
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            protocol::NativeNginxCompletionPlanV1 {
                schema: "iroha.taira.public-reset.native-nginx-completion-plan.v1".into(),
                nginx: reconcile_plan(&plan)?,
                publication_effect: protocol::NativePublicationEffectV1::Owned,
                completion_journal_basename: "native-completion.ndjson".into(),
            }
        }
        Err(error) => return Err(error.into()),
    };
    let request = inspection_request(&completion.nginx)?;
    operation.revalidate()?;
    let inspected = capsule.run(
        &operation.root,
        "inspect",
        json::to_json(&request)?.as_bytes(),
        &[&operation.lock, &operation.host_lock],
        admitted.action_deadline,
    )?;
    operation.revalidate()?;
    let observation: protocol::NativeOwnerObservationV1 = json::from_slice(&inspected)?;
    observation.validate(&publication)?;
    if observation.owned_publication.publication.sha256
        != artifact(&edge.artifacts, "edge_config")?.sha256
    {
        return Err(eyre!(
            "native cutover owner has another admitted public artifact"
        ));
    }
    retain_exact(
        &operation.root,
        "cutover-receipt.json",
        json::to_json(&observation)?.as_bytes(),
    )?;
    retain_exact(
        &operation.root,
        "completion-plan.json",
        json::to_json(&completion)?.as_bytes(),
    )?;
    if operation.progress.status != "awaiting_readiness"
        && operation.progress.status != "edge_ready_unqualified"
    {
        operation.commit(
            admitted,
            "awaiting_readiness",
            Some(publication),
            operation.progress.checkpoint_sha256.clone(),
            None,
        )?;
    }
    Ok(())
}

fn recover_publication(
    admitted: &HostAdmission,
    capsule: &NativeCapsule,
    operation: &NativeOperation,
    plan: &json::Value,
    direction: &'static str,
) -> Result<json::Value> {
    if !matches!(direction, "resume" | "rollback") {
        return Err(eyre!("native publisher recovery has another direction"));
    }
    let directory_path = plan
        .get("native")
        .and_then(|value| value.get("directory"))
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native recovery publisher journal directory missing"))?;
    let directory = iroha_fs::OwnerDirectory::open(directory_path)?;
    let journal_path = protocol::publisher_journal_path(plan, &plan_operation(plan)?)?;
    let mut journal = match fs::symlink_metadata(&journal_path) {
        Ok(_) => Some(PublicPin::open(&journal_path, MAX_PLAN as u64, true)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let request = norito::json!({ "original_plan": plan, "journal": (journal.as_ref().map(owner_reference)), "direction": direction });
    let mut inherited = vec![&operation.lock, &operation.host_lock];
    if let Some(journal) = journal.as_ref() {
        inherited.push(journal.retained.file());
    }
    operation.revalidate()?;
    directory.revalidate()?;
    let output = capsule.run(
        &operation.root,
        "recover",
        json::to_json(&request)?.as_bytes(),
        &inherited,
        admitted.action_deadline,
    )?;
    operation.revalidate()?;
    directory.revalidate()?;
    let receipt: json::Value = json::from_slice(&output)?;
    if receipt.get("exit_code").and_then(json::Value::as_u64) != Some(0)
        || receipt
            .get("recovery_direction")
            .and_then(json::Value::as_str)
            != Some(direction)
        || receipt.get("operation_id").and_then(json::Value::as_str)
            != Some(plan_operation(plan)?.as_str())
        || receipt.get("qualified").and_then(json::Value::as_bool) != Some(false)
    {
        return Err(eyre!(
            "interrupted native publisher preserves exact-operation recovery pending"
        ));
    }
    let successor: Option<NativePublicFileV1> = json::from_slice(
        json::to_json(
            receipt
                .get("journal")
                .ok_or_else(|| eyre!("native recovery lacks its required journal result"))?,
        )?
        .as_bytes(),
    )?;
    let mut successor = match successor {
        Some(reference) => {
            reference.validate_public(admitted.inventory.hosts.native_edge.owner_uid)?;
            if Path::new(&reference.file.path) != journal_path
                || reference.file.identity.size > MAX_PLAN as u64
                || reference.file.identity.mode != 0o600
            {
                return Err(eyre!("native recovery returned another owner journal"));
            }
            Some(PublicPin::expected(&reference, true)?)
        }
        None if journal.is_none()
            && direction == "rollback"
            && receipt.get("phase").and_then(json::Value::as_str) == Some("not_requested") =>
        {
            require_path_absent(&journal_path, "unchanged native publisher journal")?;
            None
        }
        None => return Err(eyre!("native recovery omitted its admitted owner journal")),
    };
    if let (Some(predecessor), Some(successor)) = (journal.as_mut(), successor.as_mut()) {
        admit_owned_journal_successor(predecessor, successor)?;
    }
    let witness = admit_journal_write_intent(&receipt, plan, successor.is_some())?;
    retain_exact(
        &operation.root,
        &format!("publisher-recovery-receipt-{}.json", sha256_hex(&output)),
        &output,
    )?;
    if let Some(successor) = successor.as_ref() {
        successor.revalidate()?;
    }
    if let Some(witness) = witness.as_ref() {
        witness.revalidate()?;
    }
    directory.revalidate()?;
    operation.revalidate()?;
    Ok(receipt)
}

fn admit_journal_write_intent(
    receipt: &json::Value,
    plan: &json::Value,
    journal_exists: bool,
) -> Result<Option<PublicPin>> {
    let witness: Option<NativePublicFileV1> =
        json::from_slice(
            json::to_json(receipt.get("journal_write_intent").ok_or_else(|| {
                eyre!("native publisher lacks its required durable intent result")
            })?)?
            .as_bytes(),
        )?;
    let Some(witness) = witness else {
        if journal_exists {
            return Err(eyre!(
                "native publisher journal lacks its independently durable intent"
            ));
        }
        return Ok(None);
    };
    let operation = plan_operation(plan)?;
    let directory = protocol::publisher_journal_path(plan, &operation)?
        .parent()
        .ok_or_else(|| eyre!("native publisher intent directory missing"))?
        .to_path_buf();
    let path = Path::new(&witness.file.path);
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|value| value.strip_prefix(&format!(".taira-native-nginx-write-{operation}-")))
        .and_then(|value| value.strip_suffix(".intent.json"))
        .ok_or_else(|| eyre!("native publisher intent has another operation namespace"))?;
    let (sequence_text, nonce) = name
        .split_once('-')
        .ok_or_else(|| eyre!("native publisher intent sequence missing"))?;
    let sequence = sequence_text.parse::<u64>()?;
    validate_lower_hex("native publisher intent nonce", nonce, 32)?;
    let owner = plan
        .get("native")
        .and_then(|value| value.get("owner_uid"))
        .and_then(json::Value::as_u64)
        .ok_or_else(|| eyre!("native publisher owner missing"))?;
    witness.validate_public(u32::try_from(owner)?)?;
    if !journal_exists
        || path.parent() != Some(directory.as_path())
        || !(1..=128).contains(&sequence)
        || sequence.to_string() != sequence_text
        || receipt
            .get("journal_sequence")
            .and_then(json::Value::as_u64)
            != Some(sequence)
        || witness.file.identity.mode != 0o600
        || u64::from(witness.file.identity.uid) != owner
        || witness.file.identity.size > 64 * 1024
    {
        return Err(eyre!(
            "native publisher intent does not join its complete owner result"
        ));
    }
    Ok(Some(PublicPin::expected(&witness, true)?))
}

/// The maintained publisher alone authenticates its independently durable
/// created-inode intent. Here the receiver retains both returned custody and the
/// immutable old raw prefix; it never reconstructs phase authority from rows.
fn admit_owned_journal_successor(
    predecessor: &mut PublicPin,
    successor: &mut PublicPin,
) -> Result<()> {
    successor.revalidate()?;
    let current = protocol::native_file_identity(&predecessor.retained.file().metadata()?)?;
    let initial = &predecessor.reference.file.identity;
    let unchanged = successor.reference == predecessor.reference;
    if current.device != initial.device
        || current.inode != initial.inode
        || current.uid != initial.uid
        || current.gid != initial.gid
        || current.mode != initial.mode
        || current.size != initial.size
        || current.mtime_ns != initial.mtime_ns
        || current.links != if unchanged { initial.links } else { 0 }
        || (unchanged && current.ctime_ns != initial.ctime_ns)
        || successor.reference.file.path != predecessor.reference.file.path
        || successor.reference.file.identity.size < initial.size
        || successor.reference.file.identity.size > MAX_PLAN as u64
    {
        return Err(eyre!(
            "native recovered owner journal changed its immutable custody"
        ));
    }
    predecessor.retained.file_mut().rewind()?;
    if hash_reader(&mut predecessor.retained.file_mut().take(initial.size))?
        != predecessor.reference.sha256
    {
        return Err(eyre!(
            "native recovered journal changed its immutable admitted prefix"
        ));
    }
    successor.retained.file_mut().rewind()?;
    if hash_reader(&mut successor.retained.file_mut().take(initial.size))?
        != predecessor.reference.sha256
        || protocol::native_file_identity(&predecessor.retained.file().metadata()?)? != current
    {
        return Err(eyre!(
            "native recovered journal changed beside its retained prefix read"
        ));
    }
    successor.revalidate()?;
    Ok(())
}

fn verify_ready(
    admitted: &HostAdmission,
    edge: &EdgeV1,
    capsule: &NativeCapsule,
    operation: &mut NativeOperation,
) -> Result<()> {
    if !matches!(
        operation.progress.status.as_str(),
        "awaiting_readiness" | "edge_ready_unqualified"
    ) {
        return Err(eyre!("native readiness lacks its owned cutover"));
    }
    let completion: protocol::NativeNginxCompletionPlanV1 =
        json::from_slice(&operation.root.read("completion-plan.json", MAX_PLAN)?)?;
    let publication = operation
        .progress
        .publication_operation_id
        .clone()
        .ok_or_else(|| eyre!("native readiness lacks its publication owner"))?;
    completion.validate(&publication)?;
    let request = inspection_request(&completion.nginx)?;
    operation.revalidate()?;
    let _observation = capsule.run(
        &operation.root,
        "inspect",
        json::to_json(&request)?.as_bytes(),
        &[&operation.lock, &operation.host_lock],
        admitted.action_deadline,
    )?;
    operation.revalidate()?;
    let mut plan_pin = PublicPin::expected(&edge.native_capability.forwarding_plan, true)?;
    let plan = plan_pin.bytes(MAX_PLAN)?;
    let mut identity_pin =
        PublicPin::expected(&edge.native_capability.forwarding_identity_receipt, true)?;
    let identity = identity_pin.bytes(MAX_PLAN)?;
    let request = norito::json!({ "plan": (json::from_slice::<json::Value>(&plan)?), "identity_receipt": (json::from_slice::<json::Value>(&identity)?) });
    let forwarding = capsule.run(
        &operation.root,
        "forwarding",
        json::to_json(&request)?.as_bytes(),
        &[
            &operation.lock,
            &operation.host_lock,
            plan_pin.retained.file(),
            identity_pin.retained.file(),
        ],
        admitted.action_deadline,
    )?;
    plan_pin.revalidate()?;
    identity_pin.revalidate()?;
    operation.revalidate()?;
    let forwarding: json::Value = json::from_slice(&forwarding)?;
    let journal: NativePublicFileV1 =
        json::from_slice(
            json::to_json(forwarding.get("journal").ok_or_else(|| {
                eyre!("native forwarding inspection lacks its retained journal")
            })?)?
            .as_bytes(),
        )?;
    if journal != edge.native_capability.incumbent.claims.forwarding_journal {
        return Err(eyre!(
            "native forwarding inspection changed its signed incumbent journal"
        ));
    }
    operation.commit(
        admitted,
        "edge_ready_unqualified",
        Some(publication),
        operation.progress.checkpoint_sha256.clone(),
        None,
    )
}

fn native_identity_json(identity: &host_pair::NativeFileIdentityV1) -> json::Value {
    norito::json!({ "device": (identity.device.to_string()), "inode": (identity.inode.to_string()), "uid": (identity.uid.to_string()), "gid": (identity.gid.to_string()), "mode": (identity.mode.to_string()), "links": (identity.links.to_string()), "size": (identity.size.to_string()), "mtime_ns": (identity.mtime_ns.to_string()), "ctime_ns": (identity.ctime_ns.to_string()) })
}

fn owner_reference(pin: &PublicPin) -> json::Value {
    norito::json!({ "path": (&pin.reference.file.path), "identity": (native_identity_json(&pin.reference.file.identity)), "sha256": (&pin.reference.sha256) })
}

fn reconcile_plan(plan: &json::Value) -> Result<json::Value> {
    let mut result = plan.clone();
    let op = plan_operation(plan)?;
    let destination = plan
        .get("destination")
        .ok_or_else(|| eyre!("native plan destination missing"))?;
    let directory = destination
        .get("directory")
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native plan directory missing"))?;
    let basename = destination
        .get("basename")
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native plan basename missing"))?;
    let journal = PublicPin::open(
        &protocol::publisher_journal_path(plan, &op)?,
        MAX_PLAN as u64,
        true,
    )?;
    let publication =
        PublicPin::open(&Path::new(directory).join(basename), MAX_PLAN as u64, false)?;
    result.as_object_mut().ok_or_else(|| eyre!("native plan object missing"))?.insert("publication".into(), norito::json!({ "kind": "reconcile", "prior": { "operation_id": op, "journal": (owner_reference(&journal)), "publication": { "identity": (native_identity_json(&publication.reference.file.identity)), "sha256": (&publication.reference.sha256) } } }));
    Ok(result)
}

fn inspection_request(plan: &json::Value) -> Result<json::Value> {
    let path = plan
        .get("candidate")
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native candidate path missing"))?;
    let mut candidate = PublicPin::open(Path::new(path), MAX_PLAN as u64, false)?;
    let bytes = candidate.bytes(MAX_PLAN)?;
    Ok(
        norito::json!({ "host_kind": (plan.get("host_kind").ok_or_else(|| eyre!("native host kind missing"))?), "native": (plan.get("native").ok_or_else(|| eyre!("native plan metadata missing"))?), "candidate_sha256": (&candidate.reference.sha256), "candidate_base64": (BASE64.encode(bytes)), "renderer_source_sha256": (plan.get("renderer_source").and_then(|value| value.get("sha256")).ok_or_else(|| eyre!("renderer SHA missing"))?), "master": (plan.get("master").ok_or_else(|| eyre!("native master missing"))?), "destination": (plan.get("destination").ok_or_else(|| eyre!("native destination missing"))?), "operation_id": (plan_operation(plan)?), "publication": (plan.get("publication").ok_or_else(|| eyre!("native publisher missing"))?) }),
    )
}

fn ensure_global_fence(admitted: &HostAdmission, operation: &mut NativeOperation) -> Result<()> {
    let chain = &admitted.request.phase_checkpoints;
    host_pair::verify_checkpoint_chain(
        chain,
        &admitted.inventory,
        &admitted.inventory_sha256,
        &admitted.authorization_sha256,
        admitted.authorization.claims.execution_expires_at_unix_ms,
        host_pair::HostPhaseV1::DeploymentProven,
    )?;
    match operation
        .root
        .read("global-proof.json", protocol::MAX_PROGRESS_BYTES)
    {
        Ok(body) => {
            let fence: protocol::NativeGlobalProofFenceV1 = json::from_slice(&body)?;
            let predecessor_body = operation.root.read(
                "global-proof-predecessor.json",
                protocol::MAX_PROGRESS_BYTES,
            )?;
            let predecessor: protocol::NativeEdgeProgressV1 = json::from_slice(&predecessor_body)?;
            if predecessor.digest()? != sha256_hex(&predecessor_body)
                || sha256_hex(&body) != sha256_hex(json::to_json(&fence)?.as_bytes())
            {
                return Err(eyre!(
                    "native immutable global fence files are not canonical"
                ));
            }
            fence.validate(
                &admitted.inventory,
                &admitted.inventory_sha256,
                &admitted.authorization_sha256,
                admitted.authorization.claims.execution_expires_at_unix_ms,
                chain,
                &predecessor,
            )?;
            if fence.operation_id != operation.progress.operation_id
                || Some(&fence.publication_operation_id)
                    != operation.progress.publication_operation_id.as_ref()
            {
                return Err(eyre!(
                    "native immutable fence belongs to another operation owner"
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if operation.progress.status != "edge_ready_unqualified" {
                return Err(eyre!(
                    "global proof cannot bypass actual native edge readiness"
                ));
            }
            let ready_digest = chain[1].digest()?;
            if operation.progress.checkpoint_sha256.as_ref() != Some(&ready_digest) {
                operation.commit(
                    admitted,
                    "edge_ready_unqualified",
                    operation.progress.publication_operation_id.clone(),
                    Some(ready_digest),
                    None,
                )?;
            }
            retain_exact(
                &operation.root,
                "global-proof-predecessor.json",
                json::to_json(&operation.progress)?.as_bytes(),
            )?;
            let digests = chain
                .iter()
                .map(SignedHostPhaseV1::digest)
                .collect::<Result<Vec<_>>>()?;
            let fence = protocol::NativeGlobalProofFenceV1 {
                schema: protocol::FENCE_SCHEMA.into(),
                operation_id: operation.progress.operation_id.clone(),
                inventory_sha256: admitted.inventory_sha256.clone(),
                authorization_sha256: admitted.authorization_sha256.clone(),
                authorization_nonce: admitted.inventory.authorization_nonce.clone(),
                host_pair_sha256: admitted.inventory.hosts.digest()?,
                host_identity_sha256: admitted
                    .inventory
                    .hosts
                    .native_edge
                    .endpoint
                    .host_identity_sha256
                    .clone(),
                custody_root: admitted.inventory.hosts.native_edge.custody_root.clone(),
                final_checkpoint_sha256: digests[2].clone(),
                checkpoint_sha256: digests,
                progress_predecessor_sha256: operation.progress.digest()?,
                publication_operation_id: operation
                    .progress
                    .publication_operation_id
                    .clone()
                    .ok_or_else(|| eyre!("global fence lacks its native publisher"))?,
            };
            fence.validate(
                &admitted.inventory,
                &admitted.inventory_sha256,
                &admitted.authorization_sha256,
                admitted.authorization.claims.execution_expires_at_unix_ms,
                chain,
                &operation.progress,
            )?;
            retain_exact(
                &operation.root,
                "global-proof.json",
                json::to_json(&fence)?.as_bytes(),
            )?;
            // Once this create-only sync completes, every native path refuses rollback.
            operation.root.sync()?;
        }
        Err(error) => return Err(error.into()),
    }
    operation.revalidate()
}

fn no_effect_completion_plan(
    admitted: &HostAdmission,
    edge: &EdgeV1,
    capsule: &NativeCapsule,
    operation: &NativeOperation,
) -> Result<protocol::NativeNginxCompletionPlanV1> {
    let mut request_pin =
        PublicPin::expected(&edge.native_capability.incumbent_nginx_request, true)?;
    let request = request_pin.bytes(MAX_PLAN)?;
    let request: json::Value = json::from_slice(&request)?;
    operation.revalidate()?;
    let observation = capsule.run(
        &operation.root,
        "inspect",
        json::to_json(&request)?.as_bytes(),
        &[
            &operation.lock,
            &operation.host_lock,
            request_pin.retained.file(),
        ],
        admitted.action_deadline,
    )?;
    request_pin.revalidate()?;
    operation.revalidate()?;
    let observation: json::Value = json::from_slice(&observation)?;
    let observed: host_pair::NativeOwnedPublicationV1 = json::from_slice(
        json::to_json(
            observation
                .get("owned_publication")
                .ok_or_else(|| eyre!("native owner observation lacks its publication"))?,
        )?
        .as_bytes(),
    )?;
    if observed != edge.native_capability.incumbent.claims.owned_publication {
        return Err(eyre!("native no-effect rollback incumbent changed"));
    }
    let successor = load_apply_plan(edge)?;
    let intended = plan_operation(&successor)?;
    let mut nginx = successor;
    let object = nginx
        .as_object_mut()
        .ok_or_else(|| eyre!("native nginx plan object missing"))?;
    for name in [
        "native",
        "master",
        "destination",
        "publication",
        "operation_id",
    ] {
        object.insert(
            name.into(),
            request
                .get(name)
                .ok_or_else(|| eyre!("native incumbent request field missing"))?
                .clone(),
        );
    }
    object.insert("candidate".into(), norito::json!({ "path": (&observed.publication.file.path), "owner_uid": (admitted.inventory.hosts.native_edge.owner_uid), "sha256": (&observed.publication.sha256) }));
    object
        .get_mut("renderer_source")
        .and_then(json::Value::as_object_mut)
        .ok_or_else(|| eyre!("native renderer source missing"))?
        .insert(
            "sha256".into(),
            request
                .get("renderer_source_sha256")
                .ok_or_else(|| eyre!("native incumbent renderer digest missing"))?
                .clone(),
        );
    let plan = protocol::NativeNginxCompletionPlanV1 {
        schema: "iroha.taira.public-reset.native-nginx-completion-plan.v1".into(),
        nginx,
        publication_effect: protocol::NativePublicationEffectV1::NotRequested {
            intended_operation_id: intended.clone(),
            incumbent: observed,
        },
        completion_journal_basename: "native-completion.ndjson".into(),
    };
    plan.validate(&intended)?;
    Ok(plan)
}

/// Restore an interrupted publisher directly, before the completion journal
/// acknowledges it. The original signed plan and publisher journal remain the
/// sole source of inode arrangements; this path never completes candidate HUP.
fn interrupted_rollback_plan(
    admitted: &HostAdmission,
    edge: &EdgeV1,
    capsule: &NativeCapsule,
    operation: &NativeOperation,
) -> Result<protocol::NativeNginxCompletionPlanV1> {
    if operation.progress.status != "rollback_requested" {
        return Err(eyre!(
            "interrupted publisher rollback lacks its durable requested direction"
        ));
    }
    let original = load_apply_plan(edge)?;
    admit_apply_plan(&original, edge)?;
    let intended = plan_operation(&original)?;
    let receipt = recover_publication(admitted, capsule, operation, &original, "rollback")?;
    let journal: Option<NativePublicFileV1> =
        json::from_slice(
            json::to_json(receipt.get("journal").ok_or_else(|| {
                eyre!("native rollback recovery lacks its required journal result")
            })?)?
            .as_bytes(),
        )?;
    let restored: Option<host_pair::NativeOwnedPublicationV1> = json::from_slice(
        json::to_json(receipt.get("restored_owned_publication").ok_or_else(|| {
            eyre!("native rollback recovery lacks its required restored owner result")
        })?)?
        .as_bytes(),
    )?;
    let phase = receipt.get("phase").and_then(json::Value::as_str);
    if phase == Some("not_requested") {
        if journal.is_some()
            || restored.as_ref() != Some(&edge.native_capability.incumbent.claims.owned_publication)
        {
            return Err(eyre!(
                "native no-effect recovery changed its unchanged incumbent or created a journal"
            ));
        }
        return no_effect_completion_plan(admitted, edge, capsule, operation);
    }
    if phase != Some("rolled_back_unqualified") {
        return Err(eyre!(
            "native interrupted publisher has not proven its direct restoration"
        ));
    }
    let journal =
        journal.ok_or_else(|| eyre!("native restored publisher has no terminal journal"))?;
    let retained = PublicPin::expected(&journal, true)?;
    let mut restored_pins = Vec::new();
    if let Some(restored) = restored.as_ref() {
        if restored.operation_id
            != edge
                .native_capability
                .incumbent
                .claims
                .owned_publication
                .operation_id
        {
            return Err(eyre!("native publisher restored another predecessor owner"));
        }
        restored_pins.push(PublicPin::expected(&restored.journal, true)?);
        restored_pins.push(PublicPin::expected(&restored.publication, false)?);
    }
    let plan = protocol::NativeNginxCompletionPlanV1 {
        schema: "iroha.taira.public-reset.native-nginx-completion-plan.v1".into(),
        nginx: original,
        publication_effect: protocol::NativePublicationEffectV1::PublisherRolledBack {
            journal,
            restored_owned_publication: restored,
        },
        completion_journal_basename: "native-completion.ndjson".into(),
    };
    plan.validate(&intended)?;
    retained.revalidate()?;
    for pin in restored_pins {
        pin.revalidate()?;
    }
    operation.revalidate()?;
    Ok(plan)
}

fn completion_packet(
    admitted: &HostAdmission,
    action: HostAction,
    operation: &NativeOperation,
    pins: &[PublicPin],
    directory: Option<&File>,
) -> Result<protocol::NativeCompletionAdmissionV1> {
    let mut index = 0;
    let mut next = || -> Result<protocol::RetainedPublicRefV1> {
        let reference = pins
            .get(index)
            .ok_or_else(|| eyre!("completion public pin inventory is incomplete"))?
            .inherited()?;
        index += 1;
        Ok(reference)
    };
    let executable = next()?;
    let guard = next()?;
    let inventory = next()?;
    let authorization = next()?;
    let progress = next()?;
    let plan = next()?;
    let checkpoints = (0..admitted.request.phase_checkpoints.len())
        .map(|_| next())
        .collect::<Result<Vec<_>>>()?;
    let fence = if action == HostAction::Rollback {
        let directory =
            directory.ok_or_else(|| eyre!("rollback lacks its retained absent-fence directory"))?;
        protocol::NativeFenceCustodyV1::Absent {
            directory_fd: u32::try_from(directory.as_raw_fd())?,
            directory: NativeObservedFileV1 {
                path: operation.root.path().to_string_lossy().into_owned(),
                identity: protocol::native_file_identity(&directory.metadata()?)?,
            },
            basename: "global-proof.json".into(),
        }
    } else {
        protocol::NativeFenceCustodyV1::Present { reference: next()? }
    };
    let native = &admitted.inventory.hosts.native_edge;
    let started = run_host_command(
        "/bin/ps",
        &["-p", &std::process::id().to_string(), "-o", "lstart="],
        admitted.action_deadline,
    )?;
    let started = canonical_parent_start(&started)?;
    if started.len() < 20
        || started.len() > 32
        || !started
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b' '))
    {
        return Err(eyre!(
            "native parent start identity is outside its exact public form"
        ));
    }
    Ok(protocol::NativeCompletionAdmissionV1 {
        schema: protocol::ADMISSION_SCHEMA.into(),
        action: match action {
            HostAction::Rollback => "rollback",
            HostAction::Seal => "seal",
            HostAction::Cleanup => "cleanup",
            _ => return Err(eyre!("unsupported native terminal action")),
        }
        .into(),
        operation_id: operation.progress.operation_id.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        host_pair_sha256: admitted.inventory.hosts.digest()?,
        host_identity_sha256: native.endpoint.host_identity_sha256.clone(),
        custody_root: native.custody_root.clone(),
        helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
        parent: protocol::NativeParentV1 {
            pid: std::process::id(),
            uid: native.owner_uid,
            started,
            executable,
        },
        host_lock: protocol::NativeRetainedLockV1 {
            fd: u32::try_from(operation.host_lock.as_raw_fd())?,
            file: NativeObservedFileV1 {
                path: operation
                    .host_root
                    .path()
                    .join("host-operation.lock")
                    .to_string_lossy()
                    .into_owned(),
                identity: operation.host_lock_identity.clone(),
            },
        },
        lock: protocol::NativeRetainedLockV1 {
            fd: u32::try_from(operation.lock.as_raw_fd())?,
            file: NativeObservedFileV1 {
                path: operation
                    .root
                    .path()
                    .join("operation.lock")
                    .to_string_lossy()
                    .into_owned(),
                identity: operation.lock_identity.clone(),
            },
        },
        guard,
        inventory,
        authorization,
        progress,
        checkpoints,
        fence,
        plan,
    })
}

fn complete(
    admitted: &HostAdmission,
    action: HostAction,
    edge: &EdgeV1,
    capsule: &NativeCapsule,
    operation: &mut NativeOperation,
) -> Result<()> {
    if operation.progress.status == "recovery_pending" {
        let digest = operation
            .progress
            .completion_receipt_sha256
            .as_ref()
            .ok_or_else(|| eyre!("native terminal recovery has no retained action receipt"))?;
        let bytes = operation
            .root
            .read(format!("completion-receipt-{digest}.json"), 64 * 1024)?;
        let prior: protocol::NativeCompletionReceiptV1 = json::from_slice(&bytes)?;
        let requested = match action {
            HostAction::Rollback => "rollback",
            HostAction::Seal => "seal",
            HostAction::Cleanup => "cleanup",
            _ => return Err(eyre!("native terminal recovery has another action")),
        };
        if sha256_hex(&bytes) != *digest
            || prior.status != "recovery_pending"
            || prior.action != requested
            || prior.operation_id != operation.progress.operation_id
            || prior.inventory_sha256 != admitted.inventory_sha256
            || prior.authorization_sha256 != admitted.authorization_sha256
        {
            return Err(eyre!(
                "native recovery cannot switch its retained terminal action"
            ));
        }
    }
    let publication = operation
        .progress
        .publication_operation_id
        .clone()
        .ok_or_else(|| eyre!("native completion lacks its intended publication owner"))?;
    if action == HostAction::Rollback {
        if operation
            .root
            .entries(128)?
            .iter()
            .any(|name| name == "global-proof.json")
        {
            return Err(eyre!(
                "native rollback is forbidden beyond the immutable global proof fence"
            ));
        }
        if matches!(operation.progress.status.as_str(), "admitted" | "staged") {
            let plan = no_effect_completion_plan(admitted, edge, capsule, operation)?;
            retain_exact(
                &operation.root,
                "completion-plan.json",
                json::to_json(&plan)?.as_bytes(),
            )?;
        } else if !matches!(
            operation.progress.status.as_str(),
            "cutover_requested"
                | "awaiting_readiness"
                | "edge_ready_unqualified"
                | "rollback_requested"
                | "rolled_back"
                | "recovery_pending"
        ) {
            return Err(eyre!("native rollback cannot revoke a terminal seal"));
        }
        if matches!(
            operation.progress.status.as_str(),
            "cutover_requested" | "rollback_requested"
        ) && !operation
            .root
            .entries(128)?
            .iter()
            .any(|name| name == "completion-plan.json")
        {
            if operation.progress.status != "rollback_requested" {
                operation.commit(
                    admitted,
                    "rollback_requested",
                    Some(publication.clone()),
                    operation.progress.checkpoint_sha256.clone(),
                    None,
                )?;
            }
            let plan = interrupted_rollback_plan(admitted, edge, capsule, operation)?;
            retain_exact(
                &operation.root,
                "completion-plan.json",
                json::to_json(&plan)?.as_bytes(),
            )?;
        }
    } else {
        ensure_global_fence(admitted, operation)?;
        let valid = match action {
            HostAction::Seal => matches!(
                operation.progress.status.as_str(),
                "edge_ready_unqualified" | "sealing" | "sealed" | "recovery_pending"
            ),
            HostAction::Cleanup => matches!(
                operation.progress.status.as_str(),
                "sealed" | "cleanup_requested" | "cleaned" | "recovery_pending"
            ),
            _ => false,
        };
        if !valid {
            return Err(eyre!(
                "native completion does not follow its durable terminal phase"
            ));
        }
    }
    let plan: protocol::NativeNginxCompletionPlanV1 =
        json::from_slice(&operation.root.read("completion-plan.json", MAX_PLAN)?)?;
    plan.validate(&publication)?;
    if action != HostAction::Rollback
        && !matches!(
            plan.publication_effect,
            protocol::NativePublicationEffectV1::Owned
        )
    {
        return Err(eyre!(
            "a no-effect rollback plan cannot authorize native sealing"
        ));
    }
    let intent = match action {
        HostAction::Rollback => "rollback_requested",
        HostAction::Seal => "sealing",
        HostAction::Cleanup => "cleanup_requested",
        _ => unreachable!("closed caller"),
    };
    operation.commit(
        admitted,
        intent,
        Some(publication.clone()),
        admitted
            .request
            .phase_checkpoints
            .last()
            .map(SignedHostPhaseV1::digest)
            .transpose()?,
        None,
    )?;
    let progress_before = operation.progress.digest()?;
    let mut pins = Vec::new();
    pins.push(PublicPin::open(
        &std::env::current_exe()?,
        512 * 1024 * 1024,
        false,
    )?);
    pins.push(PublicPin::open(
        &canonical_guard_path(&admitted.inventory.hosts.native_edge)?,
        64 * 1024,
        true,
    )?);
    for name in [
        "inventory.json",
        "authorization.json",
        "progress.json",
        "completion-plan.json",
    ] {
        pins.push(PublicPin::private_child(
            &operation.root,
            name,
            MAX_PLAN as u64,
        )?);
    }
    for index in 0..admitted.request.phase_checkpoints.len() {
        pins.push(PublicPin::private_child(
            &operation.root,
            &format!("checkpoint-{}.json", index + 1),
            host_pair::MAX_CHECKPOINT_BYTES as u64,
        )?);
    }
    let directory = if action == HostAction::Rollback {
        let descriptor = File::from(rustix::fs::open(
            operation.root.path(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        if protocol::native_file_identity(&descriptor.metadata()?)?
            != protocol::native_file_identity(&fs::symlink_metadata(operation.root.path())?)?
        {
            return Err(eyre!(
                "native absent-fence directory changed beside its retained owner"
            ));
        }
        Some(descriptor)
    } else {
        pins.push(PublicPin::private_child(
            &operation.root,
            "global-proof.json",
            protocol::MAX_PROGRESS_BYTES as u64,
        )?);
        None
    };
    let packet = completion_packet(admitted, action, operation, &pins, directory.as_ref())?;
    let mut inherited = pins
        .iter()
        .map(|pin| pin.retained.file())
        .collect::<Vec<_>>();
    inherited.push(&operation.lock);
    inherited.push(&operation.host_lock);
    if let Some(directory) = directory.as_ref() {
        inherited.push(directory);
    }
    operation.revalidate()?;
    let output = capsule.run(
        &operation.root,
        "complete",
        json::to_json(&packet)?.as_bytes(),
        &inherited,
        admitted.action_deadline,
    )?;
    for pin in &pins {
        pin.revalidate()?;
    }
    operation.revalidate()?;
    let receipt: protocol::NativeCompletionReceiptV1 = json::from_slice(&output)?;
    let terminal = match action {
        HostAction::Rollback => "rolled_back",
        HostAction::Seal => "sealed",
        HostAction::Cleanup => "cleaned",
        _ => unreachable!("closed caller"),
    };
    let fence_sha = if action == HostAction::Rollback {
        None
    } else {
        Some(
            pins.last()
                .ok_or_else(|| eyre!("native fence pin missing"))?
                .reference
                .sha256
                .clone(),
        )
    };
    if receipt.schema != "iroha.taira.public-reset.native-edge-completion-receipt.v1"
        || receipt.action != packet.action
        || receipt.operation_id != packet.operation_id
        || receipt.inventory_sha256 != packet.inventory_sha256
        || receipt.authorization_sha256 != packet.authorization_sha256
        || receipt.authorization_nonce != packet.authorization_nonce
        || receipt.host_pair_sha256 != packet.host_pair_sha256
        || receipt.host_identity_sha256 != packet.host_identity_sha256
        || receipt.custody_root != packet.custody_root
        || receipt.progress_before_sha256 != progress_before
        || receipt.publication_operation_id != publication
        || receipt.global_proof_sha256 != fence_sha
        || Path::new(&receipt.completion_journal.file.path)
            != operation.root.path().join("native-completion.ndjson")
        || (receipt.status != terminal && receipt.status != "recovery_pending")
        || (receipt.status == terminal) != receipt.error_code.is_none()
        || (action != HostAction::Rollback && receipt.restored_owned_publication.is_some())
    {
        return Err(eyre!(
            "native completion returned another owner or unverified terminal receipt"
        ));
    }
    protocol::read_native_public(
        &receipt.completion_journal,
        admitted.inventory.hosts.native_edge.owner_uid,
        MAX_PLAN,
    )?;
    let raw_digest = sha256_hex(&output);
    retain_exact(
        &operation.root,
        &format!("completion-receipt-{raw_digest}.json"),
        &output,
    )?;
    let status = receipt.status.clone();
    operation.commit(
        admitted,
        &status,
        Some(publication),
        packet
            .checkpoints
            .last()
            .map(|reference| reference.reference.sha256.clone()),
        Some(raw_digest),
    )?;
    if status == "recovery_pending" {
        return Err(eyre!(
            "native completion retained ambiguous custody for exact-owner recovery"
        ));
    }
    Ok(())
}

fn canonical_parent_start(bytes: &[u8]) -> Result<String> {
    if bytes.len() > 64 {
        return Err(eyre!(
            "native parent start identity exceeds its fixed native extent"
        ));
    }
    let value = std::str::from_utf8(bytes)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if value.len() < 20
        || value.len() > 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b' '))
    {
        return Err(eyre!(
            "native parent start identity is outside its exact public form"
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_parent_start_uses_the_maintained_single_digit_day_wire() {
        assert_eq!(
            canonical_parent_start(b"  Sun Oct  4 20:03:58 2026\n").unwrap(),
            "Sun Oct 4 20:03:58 2026"
        );
        assert!(canonical_parent_start(b"Sun Oct 4 20:03:58 2026;evil").is_err());
        assert!(canonical_parent_start(&[b' '; 65]).is_err());
    }

    #[test]
    fn inherited_descriptor_rebase_is_closed_and_requires_every_retained_owner() {
        let mut packet = norito::json!({ "lock": { "fd": 7 }, "fence": { "kind": "absent", "value": { "directory_fd": 8 } }, "source": [{ "fd": 9 }] });
        let descriptors = BTreeMap::from([(7, 17), (8, 18), (9, 19)]);
        rebase_descriptors(&mut packet, &descriptors).unwrap();
        assert_eq!(
            packet.get("lock").unwrap().get("fd").unwrap().as_u64(),
            Some(17)
        );
        assert_eq!(
            packet
                .get("fence")
                .unwrap()
                .get("value")
                .unwrap()
                .get("directory_fd")
                .unwrap()
                .as_u64(),
            Some(18)
        );
        let mut unowned = norito::json!({ "fd": 10 });
        assert!(rebase_descriptors(&mut unowned, &descriptors).is_err());
        let mut wrong_wire = norito::json!({ "fd": "7" });
        assert!(rebase_descriptors(&mut wrong_wire, &descriptors).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_public_pin_rejects_digest_snapshot_and_path_substitution() {
        let directory = tempfile::Builder::new()
            .prefix(".native-edge-pin-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("input.json");
        fs::write(&path, b"first-native-record").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut pin = PublicPin::open(&path, 128, true).unwrap();
        let reference = pin.reference.clone();
        assert_eq!(pin.bytes(128).unwrap(), b"first-native-record");
        assert!(pin.bytes(8).is_err());
        let original = directory.path().join("original.json");
        fs::rename(&path, &original).unwrap();
        fs::write(&path, b"second-native-record").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(pin.revalidate().is_err());
        assert!(PublicPin::expected(&reference, true).is_err());
        assert_eq!(fs::read(&original).unwrap(), b"first-native-record");
        assert_eq!(fs::read(&path).unwrap(), b"second-native-record");
    }

    #[cfg(unix)]
    #[test]
    fn recovered_journal_retains_the_unlinked_raw_predecessor_and_exact_successor() {
        let directory = tempfile::Builder::new()
            .prefix(".native-journal-successor-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("journal.ndjson");
        let displaced = directory.path().join("displaced.ndjson");
        let first = b"{\"sequence\":1}\n";
        fs::write(&path, first).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut predecessor = PublicPin::open(&path, 256, true).unwrap();
        let mut unchanged = PublicPin::expected(&predecessor.reference, true).unwrap();
        admit_owned_journal_successor(&mut predecessor, &mut unchanged).unwrap();
        fs::rename(&path, &displaced).unwrap();
        fs::write(&path, [first.as_slice(), b"{\"sequence\":2}\n"].concat()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut successor = PublicPin::open(&path, 256, true).unwrap();
        assert!(admit_owned_journal_successor(&mut predecessor, &mut successor).is_err());
        fs::remove_file(&displaced).unwrap();
        admit_owned_journal_successor(&mut predecessor, &mut successor).unwrap();
        fs::write(&path, b"foreign-prefix\n{\"sequence\":2}\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut foreign = PublicPin::open(&path, 256, true).unwrap();
        assert!(admit_owned_journal_successor(&mut predecessor, &mut foreign).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn reconciliation_retains_the_main_directory_journal_and_child_publication() {
        let directory = tempfile::Builder::new()
            .prefix(".native-reconcile-layout-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let includes = directory.path().join("servers");
        fs::create_dir(&includes).unwrap();
        fs::set_permissions(&includes, fs::Permissions::from_mode(0o700)).unwrap();
        let operation = "b".repeat(32);
        let journal = directory.path().join(format!(
            ".taira-native-nginx-apply-{operation}.receipt.ndjson"
        ));
        fs::write(&journal, b"retained-public-owner-journal\n").unwrap();
        fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
        let public = includes.join("taira-public.conf");
        fs::write(&public, b"server { listen 8443; }\n").unwrap();
        fs::set_permissions(&public, fs::Permissions::from_mode(0o644)).unwrap();
        let original = norito::json!({ "operation_id": operation, "native": {"directory":{"path":(directory.path().to_string_lossy().into_owned())}}, "destination":{"directory":{"path":(includes.to_string_lossy().into_owned())},"basename":"taira-public.conf"},"publication":{"kind":"create"} });
        let result = reconcile_plan(&original).unwrap();
        let prior = result.get("publication").unwrap().get("prior").unwrap();
        assert_eq!(
            prior.get("journal").unwrap().get("path").unwrap().as_str(),
            journal.to_str()
        );
        let public_ref = prior.get("publication").unwrap().as_object().unwrap();
        assert_eq!(
            public_ref
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["identity", "sha256"])
        );
        assert_eq!(
            public_ref.get("sha256").unwrap().as_str(),
            Some(sha256_hex(b"server { listen 8443; }\n").as_str())
        );
        assert_eq!(
            fs::read(&journal).unwrap(),
            b"retained-public-owner-journal\n"
        );
        assert!(!includes.join(journal.file_name().unwrap()).exists());
    }

    #[test]
    fn completion_plan_requires_explicit_effect_and_exact_incumbent_join() {
        let inventory = super::super::super::sample_inventory_fixture();
        let incumbent = inventory
            .edge
            .native_capability
            .incumbent
            .claims
            .owned_publication;
        let mut plan = protocol::NativeNginxCompletionPlanV1 {
            schema: "iroha.taira.public-reset.native-nginx-completion-plan.v1".into(),
            nginx: norito::json!({"schema":"iroha.taira.native-nginx-apply.plan.v1","provider":"macstadium-dublin","host_kind":"macos","operation_id":(&incumbent.operation_id),"publication":{"kind":"reconcile","prior":{"operation_id":(&incumbent.operation_id)}}}),
            publication_effect: protocol::NativePublicationEffectV1::NotRequested {
                intended_operation_id: "b".repeat(32),
                incumbent,
            },
            completion_journal_basename: "native-completion.ndjson".into(),
        };
        plan.validate(&"b".repeat(32)).unwrap();
        let mut wire: json::Value =
            json::from_slice(json::to_json(&plan).unwrap().as_bytes()).unwrap();
        wire.as_object_mut().unwrap().remove("publication_effect");
        assert!(
            json::from_slice::<protocol::NativeNginxCompletionPlanV1>(
                json::to_json(&wire).unwrap().as_bytes()
            )
            .is_err()
        );
        plan.publication_effect = protocol::NativePublicationEffectV1::Owned;
        assert!(plan.validate(&"b".repeat(32)).is_err());
    }

    #[test]
    fn apply_plan_binds_the_admitted_native_publication_destination() {
        let inventory = super::super::super::sample_inventory_fixture();
        let edge = &inventory.edge;
        let selected = artifact(&edge.artifacts, "edge_config").unwrap();
        let destination = Path::new(&edge.nginx_config);
        let plan = norito::json!({
            "schema": "iroha.taira.native-nginx-apply.plan.v1",
            "provider": "macstadium-dublin",
            "host_kind": "macos",
            "operation_id": ("a".repeat(32)),
            "candidate": {
                "path": (&selected.remote_path),
                "sha256": (&selected.sha256)
            },
            "destination": {
                "directory": {"path": (destination.parent().unwrap().to_str().unwrap())},
                "basename": (destination.file_name().unwrap().to_str().unwrap())
            }
        });
        admit_apply_plan(&plan, edge).expect("exact native publication destination");

        for (field, replacement) in [
            (
                "directory",
                norito::json!({"path": "/another/native/publication"}),
            ),
            ("basename", json::Value::String("another.conf".into())),
        ] {
            let mut changed = plan.clone();
            changed
                .get_mut("destination")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), replacement);
            assert!(admit_apply_plan(&changed, edge).is_err(), "{field}");
        }
    }
}
