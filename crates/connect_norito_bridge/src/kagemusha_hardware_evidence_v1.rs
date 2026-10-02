//! Actual separate hardware-evidence service. No monetary coordinator/backend is selected.
//! Only a Rust startup holding a real threshold-authenticated compiled binding can install it.
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaCompiledHardwareBootstrapBindingV1 as Binding,
    KagemushaFirstDeviceHardwareEvidenceOwnerV1 as Owner,
    KagemushaHardwareEvidenceErrorV1 as Error,
};
use iroha_data_model::kagemusha::{
    KagemushaAppKeySecurityLevelV1, KagemushaDevicePublicKeyV1,
    KagemushaHardwareEvidenceReservationV1, hardware_bootstrap_decode_v1,
};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use zeroize::Zeroizing;
#[cfg(target_os = "android")]
pub(crate) mod android_startup;
pub(crate) mod frame;
type Result<T> = std::result::Result<T, Error>;
static SOURCE: OnceLock<Arc<KagemushaNativeHardwareEvidenceSourceV1>> = OnceLock::new();

/// Actual Native startup source: one fixed app-owned directory and one original evidence attempt.
/// Closing JNI keeps the original owner/WAL; no timer, UI retry or monetary factory replaces it.
pub struct KagemushaNativeHardwareEvidenceSourceV1 {
    root: PathBuf,
    binding: Arc<Binding>,
    inner: Mutex<Inner>,
}
struct Inner {
    owner: Option<Owner>,
    handles: BTreeMap<u64, ()>,
    next: u64,
}
impl KagemushaNativeHardwareEvidenceSourceV1 {
    /// Retain independently authenticated static release/package/JNI/clock originals. The closed
    /// binding was made by real Native startup; there is no C/JNI pin, measurement or DTO intake.
    /// Fixed root is selected by that startup and must be the application's original storage.
    pub fn from_authenticated_native_startup(root: PathBuf, binding: Arc<Binding>) -> Result<Self> {
        validate_path(&root)?;
        binding.public_signed_release_original()?;
        Ok(Self {
            root,
            binding,
            inner: Mutex::new(Inner {
                owner: None,
                handles: BTreeMap::new(),
                next: 1,
            }),
        })
    }
    fn open(&self, path: &Path) -> Result<u64> {
        validate_path(path)?;
        #[cfg(target_os = "android")]
        android_startup::recheck_installed_package()?;
        if path != self.root {
            return Err(Error::Rejected);
        }
        self.binding.public_signed_release_original()?;
        let mut held = self.inner.lock().map_err(|_| Error::Custody)?;
        if held.owner.is_none() {
            let owner = match std::fs::symlink_metadata(&self.root) {
                Ok(_) => Owner::recover(&self.root, self.binding.clone())?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Owner::create(&self.root, self.binding.clone())?
                }
                Err(_) => return Err(Error::Custody),
            };
            held.owner = Some(owner);
        }
        held.owner
            .as_ref()
            .ok_or(Error::Custody)?
            .recheck_custody()?;
        if held.handles.len() >= 8 {
            return Err(Error::Custody);
        }
        let handle = held.next;
        held.next = handle.checked_add(1).ok_or(Error::Custody)?;
        held.handles.insert(handle, ());
        Ok(handle)
    }
    fn close(&self, handle: u64) -> Result<()> {
        let mut held = self.inner.lock().map_err(|_| Error::Custody)?;
        held.handles.remove(&handle).ok_or(Error::Rejected)?;
        Ok(()) // Owner and exact unknown invocations remain held; never erase WAL/key.
    }
    fn invoke(&self, handle: u64, method: i32, fields: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
        #[cfg(target_os = "android")]
        android_startup::recheck_installed_package()?;
        if handle == 0 || !frame::valid_request(method, fields) {
            return Err(Error::Rejected);
        }
        let mut held = self.inner.lock().map_err(|_| Error::Custody)?;
        if !held.handles.contains_key(&handle) {
            return Err(Error::Rejected);
        }
        let owner = held.owner.as_mut().ok_or(Error::Custody)?;
        owner.recheck_custody()?;
        let output = match method {
            1 => {
                let reservation: KagemushaHardwareEvidenceReservationV1 =
                    hardware_bootstrap_decode_v1(&owner.reservation_original()?)
                        .map_err(|_| Error::Rejected)?;
                let scope = owner.transport_scope()?;
                vec![
                    reservation.operation_id.to_vec(),
                    owner.authoritative_deadline_ms()?.to_le_bytes().to_vec(),
                    vec![owner.completed_step()?],
                    vec![owner.pending_step()?.unwrap_or(0)],
                    scope.0.as_bytes().to_vec(),
                    scope.1.as_bytes().to_vec(),
                    scope.2.as_bytes().to_vec(),
                ]
            }
            2 => {
                owner.recheck_pending_effect(fields[0][0])?;
                vec![]
            }
            3 => vec![owner.fence_prepare(&fields[0])?],
            4 => {
                owner.accept_challenge(&fields[0])?;
                vec![]
            }
            5 | 6 => {
                let (alias, challenge, levels) = if method == 5 {
                    owner.fence_android_key()?
                } else {
                    owner.recover_android_key_selection()?
                };
                vec![
                    alias.into_bytes(),
                    challenge.to_vec(),
                    vec![level_mask(&levels)?],
                ]
            }
            7 => {
                let point: [u8; 65] = fields[0]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?;
                let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&point)
                    .map_err(|_| Error::Rejected)?;
                owner.capture_android_original(key, &fields[1])?;
                vec![]
            }
            8 => {
                let (c, raw) = owner.fence_raw_issuer()?;
                vec![c, raw]
            }
            9 => {
                owner.accept_raw_admission(&fields[0])?;
                vec![]
            }
            10 => {
                owner.fence_possession()?;
                let (alias, c, point, id, e, levels) = owner.pending_possession_selection()?;
                vec![
                    alias.into_bytes(),
                    c.to_vec(),
                    vec![level_mask(&levels)?],
                    point.to_vec(),
                    id.to_vec(),
                    e,
                ]
            }
            11 => {
                owner.capture_possession(&fields[0])?;
                vec![]
            }
            12 => {
                let (project, hash) = owner.fence_integrity()?;
                vec![project.to_le_bytes().to_vec(), hash.to_vec()]
            }
            13 => {
                owner.capture_integrity_original(&fields[0])?;
                vec![]
            }
            14 => {
                let (c, raw, admission, e, der, token) = owner.fence_receipt()?;
                vec![c, raw, admission, e, der, token]
            }
            15 => {
                owner.accept_hardware_receipt(&fields[0])?;
                vec![]
            }
            16 => owner
                .original_receipt()?
                .map(|b| vec![b.to_vec()])
                .unwrap_or_default(),
            17 => {
                owner.request_cancel()?;
                vec![]
            }
            18 => {
                owner.dispose_terminal()?;
                vec![]
            }
            _ => return Err(Error::Rejected),
        };
        owner.recheck_custody()?;
        #[cfg(target_os = "android")]
        android_startup::recheck_installed_package()?;
        if output.len() > frame::MAX_FIELDS || output.iter().any(|b| b.len() > frame::MAX_FIELD) {
            return Err(Error::Custody);
        }
        Ok(output)
    }
}
fn validate_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path.components().any(|c| {
            matches!(
                c,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::Rejected);
    }
    Ok(())
}
fn level_mask(levels: &[KagemushaAppKeySecurityLevelV1]) -> Result<u8> {
    let mut mask = 0;
    for level in levels {
        mask |= match level {
            KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment => 1,
            KagemushaAppKeySecurityLevelV1::StrongBox => 2,
            _ => return Err(Error::Rejected),
        };
    }
    if !(1..=3).contains(&mask) {
        return Err(Error::Rejected);
    }
    Ok(mask)
}
/// Concrete trusted Rust startup factory for the separate hardware-only service. Static root pins
/// come from independently accepted authority policy inputs; the externally signed manifest
/// is authenticated under them and is not embedded/hash-pinned inside its own measured JNI; measurements are actual Native platform
/// observations and source lineage, never a managed frame. This function authenticates and
/// joins them before any WAL reservation. It grants no account/session/runtime money authority.
#[allow(clippy::too_many_arguments)]
pub fn bootstrap_kagemusha_native_hardware_evidence_v1(
    root: PathBuf,
    signed_release_original: &[u8],
    authority_original: &[u8],
    independently_compiled_authority_digest: [u8; 32],
    measured: iroha_core_zk::kagemusha_v1_state::KagemushaHardwareBootstrapArtifactMeasurementsV1,
    actual_signed_clock: Arc<
        Mutex<iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryNativeClockOwnerV1>,
    >,
) -> Result<()> {
    let binding = Arc::new(Binding::authenticate_compiled_originals(
        signed_release_original,
        authority_original,
        independently_compiled_authority_digest,
        measured,
        actual_signed_clock,
    )?);
    let source = Arc::new(
        KagemushaNativeHardwareEvidenceSourceV1::from_authenticated_native_startup(root, binding)?,
    );
    register_kagemusha_native_hardware_evidence_source_v1(source)
}
/// Install exactly once; absence is an explicit unconfigured service, never a financial fallback.
pub fn register_kagemusha_native_hardware_evidence_source_v1(
    source: Arc<KagemushaNativeHardwareEvidenceSourceV1>,
) -> Result<()> {
    source.binding.public_signed_release_original()?;
    SOURCE.set(source).map_err(|_| Error::Rejected)
}
pub(crate) fn open(path: &Path) -> Result<u64> {
    SOURCE.get().ok_or(Error::Custody)?.open(path)
}
pub(crate) fn close(handle: u64) -> Result<()> {
    SOURCE.get().ok_or(Error::Custody)?.close(handle)
}
pub(crate) fn invoke(handle: u64, method: i32, fields: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>> {
    let fields = Zeroizing::new(fields);
    SOURCE
        .get()
        .ok_or(Error::Custody)?
        .invoke(handle, method, &fields)
}
