//! Shared installed-runtime Native startup with real account custody and session revocation.
//! The application frame supplies only raw original current-cut evidence and lifecycle selectors.
use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error,
    native_deadline::NativeDeadlineV1,
    ordinary_app_identity::{
        KagemushaNativeOrdinaryAppIdentitySourceV1 as Source,
        KagemushaOrdinaryEnrollmentDispositionV1 as Disposition,
        install_kagemusha_native_ordinary_source_v1,
    },
    session_registry::{RegistryError, SessionRegistry},
};
use iroha::client::{
    AccountClient, KagemushaAdmittedOrdinaryNativeInventoryV1 as Inventory,
    KagemushaNativeAccountCustodyV1 as Custody, KagemushaNativeCurrentWalletReadV1 as Read,
    KagemushaNativeInstalledRuntimeAuthorityV1 as Authority,
    KagemushaOrdinaryNativeArtifactResolverV1 as ArtifactResolver,
};
use iroha::participant_enrollment_request::VerifiedEnrollmentWalletSignatoryV1;
use iroha_core_zk::{
    kagemusha_v1_recursion::{
        KagemushaArtifactByteResolverV1, KagemushaAuthenticatedRecursiveVerifierV1,
        KagemushaRecursiveVerifierProfileV1,
    },
    kagemusha_v1_state::{
        KagemushaDurableCapacityV1, KagemushaOrdinaryAppPossessionAttemptV1,
        KagemushaOrdinaryNativeClockOwnerV1 as Clock, KagemushaOrdinaryRetailEnrollmentAttemptV1,
        KagemushaPendingAppIdentityV1,
    },
};
use iroha_data_model::{
    account::AccountId, kagemusha::KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

const MAX: usize = 64 * 1024 * 1024 + 1024;
/// First-release bounded Native startup request; decoding creates no authority.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeStartupRequestV1")]
pub struct KagemushaOrdinaryNativeStartupRequestV1 {
    /// Exactly one; older framing is rejected.
    pub version: u16,
    /// 1 reserve; 2 consume complete original; 3 cancel; 4 close; 5 logout; 6 Native fetch/consume.
    pub phase: u8,
    /// Actual retained read/session ID, absent (zero) for phases 1/5.
    pub id: u64,
    /// Sole complete canonical current-cut original for phase 2; empty otherwise.
    pub original: Vec<u8>,
}
/// Public retained correlation data only; no financial/current money grant occurs here.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeStartupResponseV1")]
pub struct KagemushaOrdinaryNativeStartupResponseV1 {
    /// Exactly one.
    pub version: u16,
    /// Same request phase.
    pub phase: u8,
    /// Actual retained read/session ID when applicable.
    pub id: u64,
    /// Phase 1: original nonce, canonical S, canonical W; other phases: empty.
    pub fields: Vec<Vec<u8>>,
}
struct Owner {
    account: AccountClient,
    custody: Option<Custody>,
    source_installed: bool,
    pending_source: Option<Source>,
    installation_failed: bool,
}
struct Pending {
    read: Read,
}
struct Prepared {
    custody: Custody,
    source: Option<Source>,
}
/// Actual installed Native runtime/account producer, shared by all FI applications.
/// No deserializer, raw-key callback, caller clock or bank-specific native binary is used.
pub struct KagemushaNativeOrdinaryRuntimeStartupV1 {
    inventory: Arc<Inventory>,
    clock: Arc<Mutex<Clock>>,
    owner: Arc<Mutex<Owner>>,
    registry: SessionRegistry<AccountId, Owner, Pending>,
    active: Mutex<Option<u64>>,
    storage: PathBuf,
    preparation: Disposition,
    platform: Disposition,
    bootstrap: Disposition,
    cash: Disposition,
    cash_integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    cash_receivers: Vec<
        Arc<iroha_data_model::kagemusha::KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    >,
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    profile: KagemushaRecursiveVerifierProfileV1,
    resolver: Arc<dyn KagemushaArtifactByteResolverV1>,
    capacity: KagemushaDurableCapacityV1,
    integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
}
impl KagemushaNativeOrdinaryRuntimeStartupV1 {
    /// Actual startup producer: authenticate the installed public package, load its real released
    /// proof keys, recover/create its sole clock journal, retain the actual AccountClient and
    /// start the private session kernel. A fresh four-node read is performed when phase 1 starts.
    /// The authority/account and recovery choices are independent Native owner inputs; C/JNI
    /// cannot construct them. This supplies no FI status, current State or monetary capability.
    /// # Errors
    /// Rejects changed originals, unavailable genuine release keys or unsafe original storage.
    #[allow(clippy::too_many_arguments)]
    pub fn from_installed_runtime_and_native_account(
        authority: Arc<Authority>,
        package_path: &Path,
        package_sha256: [u8; 32],
        original_root: &Path,
        storage: PathBuf,
        account: AccountClient,
        clock_disposition: Disposition,
        preparation: Disposition,
        platform: Disposition,
        bootstrap: Disposition,
        cash: Disposition,
        cash_integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        cash_receivers: Vec<
            Arc<
                iroha_data_model::kagemusha::KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
            >,
        >,
        profile: KagemushaRecursiveVerifierProfileV1,
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ) -> Result<Arc<Self>, Error> {
        let inventory = Arc::new(
            Inventory::intake(authority, package_path, package_sha256, original_root)
                .map_err(|_| Error::Rejected)?,
        );
        let verifier = inventory
            .load_recursive_verifier(profile.clone())
            .map_err(|_| Error::Rejected)?;
        capacity.validate().map_err(|_| Error::Rejected)?;
        if cash_integrity_leases.len() > 1024
            || cash_receivers.len() > 1024
            || (cash == Disposition::Fresh
                && (!cash_integrity_leases.is_empty() || !cash_receivers.is_empty()))
        {
            return Err(Error::Rejected);
        }
        let resolver: Arc<dyn KagemushaArtifactByteResolverV1> =
            Arc::new(ArtifactResolver(inventory.clone()));
        let selected = inventory.clock_originals().map_err(|_| Error::Rejected)?;
        let clock = Arc::new(Mutex::new(
            match clock_disposition {
                Disposition::Fresh => Clock::create(&storage, selected),
                Disposition::Recover => Clock::open_existing(&storage, selected),
            }
            .map_err(|_| Error::Rejected)?,
        ));
        inventory
            .clock_transport(&account, &clock)
            .map_err(|_| Error::Rejected)?;
        Ok(Arc::new(Self {
            inventory,
            clock,
            owner: Arc::new(Mutex::new(Owner {
                account,
                custody: None,
                source_installed: false,
                pending_source: None,
                installation_failed: false,
            })),
            registry: SessionRegistry::new(),
            active: Mutex::new(None),
            storage,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            verifier,
            profile,
            resolver,
            capacity,
            integrity_leases,
        }))
    }
    fn begin(&self) -> Result<(u64, Vec<Vec<u8>>), Error> {
        let deadline =
            NativeDeadlineV1::start(Duration::from_secs(10)).map_err(|_| Error::Rejected)?;
        let mut fields = None;
        let id = self
            .registry
            .begin(deadline, |permit| {
                let owner = self.owner.lock().map_err(|_| RegistryError::Poisoned)?;
                permit.require_current()?;
                if owner.installation_failed {
                    return Err(RegistryError::Rejected);
                }
                self.inventory
                    .recheck()
                    .map_err(|_| RegistryError::Rejected)?;
                let transport = self
                    .inventory
                    .clock_transport(&owner.account, &self.clock)
                    .map_err(|_| RegistryError::Rejected)?;
                transport
                    .refresh_current_clock(&self.clock)
                    .map_err(|_| RegistryError::Rejected)?;
                permit.require_current()?;
                let read = Read::reserve(
                    owner.account.clone(),
                    self.inventory.clone(),
                    self.clock.clone(),
                )
                .map_err(|_| RegistryError::Rejected)?;
                fields = Some(vec![
                    read.nonce().to_vec(),
                    norito::encode_canonical(read.signatory())
                        .map_err(|_| RegistryError::Rejected)?,
                    norito::encode_canonical(read.wallet()).map_err(|_| RegistryError::Rejected)?,
                ]);
                permit.require_current()?;
                Ok((
                    owner.account.authority().clone(),
                    self.owner.clone(),
                    Pending { read },
                ))
            })
            .map_err(|_| Error::Rejected)?;
        Ok((id, fields.ok_or(Error::Rejected)?))
    }
    fn finish(self: &Arc<Self>, id: u64, original: Option<&[u8]>) -> Result<u64, Error> {
        let completion = self
            .registry
            .take_completion(id)
            .map_err(|_| Error::Rejected)?;
        let handle = self
            .registry
            .finish(
                completion,
                |pending| {
                    match original {
                        Some(original) => pending.read.authenticate(original),
                        None => pending.read.fetch_and_authenticate(),
                    }
                    .map_err(|_| RegistryError::Rejected)
                },
                |owner, current: VerifiedEnrollmentWalletSignatoryV1| {
                    let (selected, custody) = self
                        .inventory
                        .select_account(owner.account.clone(), current, self.clock.clone())
                        .map_err(|_| RegistryError::Rejected)?;
                    let source = if owner.source_installed {
                        None
                    } else {
                        Some(
                            Source::from_native_selected_originals(
                                self.storage.clone(),
                                selected,
                                self.preparation,
                                self.platform,
                                self.inventory
                                    .integrity_policy_original()
                                    .map_err(|_| RegistryError::Rejected)?,
                            )
                            .and_then(|source| {
                                source.with_native_bootstrap_material(
                                    self.verifier.clone(),
                                    self.capacity,
                                    self.bootstrap,
                                    self.integrity_leases.clone(),
                                )
                            })
                            .and_then(|source| {
                                source.with_native_bootstrap_proving_material(
                                    self.profile.clone(),
                                    self.resolver.clone(),
                                )
                            })
                            .and_then(|source| {
                                source.with_native_cash_recovery_material(
                                    Arc::clone(&self.inventory),
                                    self.cash,
                                    self.cash_integrity_leases.clone(),
                                    self.cash_receivers.clone(),
                                )
                            })
                            .map_err(|_| RegistryError::Rejected)?,
                        )
                    };
                    custody.recheck().map_err(|_| RegistryError::Rejected)?;
                    Ok(Prepared { custody, source })
                },
                |owner, prepared| {
                    owner.custody = Some(prepared.custody);
                    owner.source_installed |= prepared.source.is_some();
                    // Pure session publication only: actual installer runs below, outside registry locks.
                    owner.pending_source = prepared.source;
                    Ok(())
                },
            )
            .map_err(|_| Error::Rejected)?;
        *self.active.lock().map_err(|_| Error::Rejected)? = Some(handle);
        let invocation = self
            .registry
            .invocation(handle)
            .map_err(|_| Error::Rejected)?;
        let retained = self
            .registry
            .dispatch(invocation, |owner| owner.pending_source.take())
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        let source = retained.value;
        if let Some(source) = source {
            let source =
                source.with_native_account_session(Arc::new(BoundNativeAccountSessionV1 {
                    startup: self.clone(),
                }));
            if self
                .require_current()
                .and_then(|()| install_kagemusha_native_ordinary_source_v1(Arc::new(source)))
                .is_err()
            {
                self.owner
                    .lock()
                    .map_err(|_| Error::Rejected)?
                    .installation_failed = true;
                self.registry
                    .revoke_selection()
                    .map_err(|_| Error::Rejected)?;
                *self.active.lock().map_err(|_| Error::Rejected)? = None;
                return Err(Error::Rejected);
            }
        }
        self.require_current()?;
        Ok(handle)
    }
    fn handle(&self) -> Result<u64, Error> {
        self.active
            .lock()
            .map_err(|_| Error::Rejected)?
            .ok_or(Error::Rejected)
    }
    fn require_current(&self) -> Result<(), Error> {
        self.registry
            .require_current_handle(self.handle()?)
            .map_err(|_| Error::Rejected)?;
        self.inventory.recheck().map_err(|_| Error::Rejected)?;
        self.registry
            .require_current_handle(self.handle()?)
            .map_err(|_| Error::Rejected)
    }
    /// Execute the distinct exact bounded startup lifecycle grammar under this actual retained root.
    /// # Errors
    /// Rejects unknown versions/phases, caller root fields, foreign IDs, noncanonical data or expiry.
    pub fn invoke(self: &Arc<Self>, frame: &[u8]) -> Result<Vec<u8>, Error> {
        if frame.is_empty() || frame.len() > MAX {
            return Err(Error::Rejected);
        }
        let request: KagemushaOrdinaryNativeStartupRequestV1 =
            norito::decode_canonical_with_limits(frame, norito::canonical_decode_limits(MAX))
                .map_err(|_| Error::Rejected)?;
        if request.version != 1
            || norito::encode_canonical(&request).map_err(|_| Error::Rejected)? != frame
        {
            return Err(Error::Rejected);
        }
        let (id, fields) = match request.phase {
            1 if request.id == 0 && request.original.is_empty() => self.begin()?,
            2 if request.id != 0 && !request.original.is_empty() => (
                self.finish(request.id, Some(&request.original))?,
                Vec::new(),
            ),
            6 if request.id != 0 && request.original.is_empty() => {
                (self.finish(request.id, None)?, Vec::new())
            }
            3 if request.id != 0 && request.original.is_empty() => {
                self.registry
                    .cancel(request.id)
                    .map_err(|_| Error::Rejected)?;
                (0, Vec::new())
            }
            4 if request.id != 0 && request.original.is_empty() => {
                self.registry
                    .close(request.id)
                    .map_err(|_| Error::Rejected)?;
                let mut active = self.active.lock().map_err(|_| Error::Rejected)?;
                if *active == Some(request.id) {
                    *active = None;
                }
                (0, Vec::new())
            }
            5 if request.id == 0 && request.original.is_empty() => {
                self.registry
                    .revoke_selection()
                    .map_err(|_| Error::Rejected)?;
                *self.active.lock().map_err(|_| Error::Rejected)? = None;
                (0, Vec::new())
            }
            _ => return Err(Error::Rejected),
        };
        norito::encode_canonical(&KagemushaOrdinaryNativeStartupResponseV1 {
            version: 1,
            phase: request.phase,
            id,
            fields,
        })
        .map_err(|_| Error::Rejected)
    }
}
pub(super) struct BoundNativeAccountSessionV1 {
    startup: Arc<KagemushaNativeOrdinaryRuntimeStartupV1>,
}
impl BoundNativeAccountSessionV1 {
    pub(super) fn recheck(&self) -> Result<(), Error> {
        let invocation = self
            .startup
            .registry
            .invocation(self.startup.handle()?)
            .map_err(|_| Error::Rejected)?;
        let retained = self
            .startup
            .registry
            .dispatch(invocation, |owner| {
                self.startup.require_current()?;
                owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .recheck()
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    pub(super) fn sign_lineage(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
    ) -> Result<[u8; 64], Error> {
        let invocation = self
            .startup
            .registry
            .invocation(self.startup.handle()?)
            .map_err(|_| Error::Rejected)?;
        let retained = self
            .startup
            .registry
            .dispatch(invocation, |owner| {
                self.startup.require_current()?;
                let signature = owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .sign_retained_lineage_request(&self.startup.inventory, original)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(signature)
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    pub(super) fn sign_current_control(
        &self,
        financial: &iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryEnrolledFinancialOwnerV1,
        control:&mut iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryCurrentFinancialControlOwnerV1,
    ) -> Result<Vec<Vec<u8>>, Error> {
        let invocation = self
            .startup
            .registry
            .invocation(self.startup.handle()?)
            .map_err(|_| Error::Rejected)?;
        let retained = self
            .startup
            .registry
            .dispatch(invocation, |owner| {
                self.startup.require_current()?;
                let fields = owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .sign_retained_current_fi_control(&self.startup.inventory, control, financial)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(fields)
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    pub(super) fn sign_retail(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        app: &KagemushaOrdinaryAppPossessionAttemptV1,
        retail: &mut KagemushaOrdinaryRetailEnrollmentAttemptV1,
    ) -> Result<[u8; 64], Error> {
        let invocation = self
            .startup
            .registry
            .invocation(self.startup.handle()?)
            .map_err(|_| Error::Rejected)?;
        let result = self
            .startup
            .registry
            .dispatch(invocation, |owner| {
                self.startup.require_current()?;
                let custody = owner.custody.as_ref().ok_or(Error::Rejected)?;
                let signature = custody
                    .sign_retained_retail_enrollment(pending, app, retail)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(signature)
            })
            .map_err(|_| Error::Rejected)?;
        if !result.session_is_current {
            return Err(Error::Rejected);
        }
        result.value
    }
}
static STARTUP: OnceLock<Arc<KagemushaNativeOrdinaryRuntimeStartupV1>> = OnceLock::new();
/// Install the actual Native runtime/account startup producer once, before managed startup.
/// It must originate from `from_installed_runtime_and_native_account`; app frames cannot create it.
/// # Errors
/// Rejects replacing an installed Native authority/account producer.
pub fn register_kagemusha_native_ordinary_runtime_startup_v1(
    startup: Arc<KagemushaNativeOrdinaryRuntimeStartupV1>,
) -> Result<(), Error> {
    STARTUP.set(startup).map_err(|_| Error::Rejected)
}
/// Dedicated bounded managed entry. The request carries no authority/key/time/root constructor.
/// # Errors
/// Rejects absent actual Native startup, malformed frame, stale owner or failed original proof.
pub fn invoke_kagemusha_native_ordinary_runtime_startup_v1(frame: &[u8]) -> Result<Vec<u8>, Error> {
    STARTUP.get().ok_or(Error::Unavailable)?.invoke(frame)
}

/// Dedicated first-release C startup entry. The independently installed Native root/account
/// producer must already exist. `phase/id/original` are the closed lifecycle grammar only.
/// Successful output is canonical `KagemushaOrdinaryNativeStartupResponseV1`; free with the
/// standard bridge allocator. No source installation, FI status or money is fabricated.
/// # Safety
/// `original_ptr` must cover `original_len` readable bytes when nonzero. Both output pointers
/// must be nonnull, aligned and writable. They are reset before any rejected request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_runtime_startup_v1(
    phase: u8,
    id: u64,
    original_ptr: *const u8,
    original_len: usize,
    output_ptr: *mut *mut u8,
    output_len: *mut usize,
) -> std::ffi::c_int {
    if output_ptr.is_null() || output_len.is_null() {
        return crate::ERR_NULL_PTR;
    }
    unsafe {
        *output_ptr = std::ptr::null_mut();
        *output_len = 0;
    }
    if original_len > 64 * 1024 * 1024 || (original_len != 0 && original_ptr.is_null()) {
        return crate::ERR_KAGEMUSHA_V1;
    }
    if !matches!((phase, id, original_len), (1, 0, 0) | (5, 0, 0)) && !matches!(phase, 2..=4 | 6) {
        return crate::ERR_KAGEMUSHA_V1;
    }
    let original = zeroize::Zeroizing::new(if original_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(original_ptr, original_len) }.to_vec()
    });
    let frame = match norito::encode_canonical(&KagemushaOrdinaryNativeStartupRequestV1 {
        version: 1,
        phase,
        id,
        original: original.to_vec(),
    }) {
        Ok(frame) => frame,
        Err(_) => return crate::ERR_KAGEMUSHA_V1,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        invoke_kagemusha_native_ordinary_runtime_startup_v1(&frame)
    })) {
        Ok(Ok(response)) => unsafe { crate::write_bytes_usize(output_ptr, output_len, &response) }
            .map_or_else(|error| error, |()| 0),
        Ok(Err(Error::Unavailable)) => crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        Ok(Err(Error::Rejected)) | Err(_) => crate::ERR_KAGEMUSHA_V1,
    }
}

#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeStartupV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    phase: jni::sys::jint,
    id: jni::sys::jlong,
    original: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<jni::sys::jobjectArray, ()> {
            let phase = u8::try_from(phase).map_err(|_| ())?;
            let length = usize::try_from(env.get_array_length(&original).map_err(|_| ())?)
                .map_err(|_| ())?;
            if length > 64 * 1024 * 1024 {
                return Err(());
            }
            let bytes = zeroize::Zeroizing::new(env.convert_byte_array(&original).map_err(|_| ())?);
            if bytes.len() != length {
                return Err(());
            }
            let frame = norito::encode_canonical(&KagemushaOrdinaryNativeStartupRequestV1 {
                version: 1,
                phase,
                id: u64::from_ne_bytes(id.to_ne_bytes()),
                original: bytes.to_vec(),
            })
            .map_err(|_| ())?;
            let response =
                invoke_kagemusha_native_ordinary_runtime_startup_v1(&frame).map_err(|_| ())?;
            let response: KagemushaOrdinaryNativeStartupResponseV1 =
                norito::decode_canonical(&response).map_err(|_| ())?;
            let mut fields = vec![
                response.version.to_le_bytes().to_vec(),
                vec![response.phase],
                response.id.to_le_bytes().to_vec(),
            ];
            fields.extend(response.fields);
            let output = env
                .new_object_array(
                    fields.len() as jni::sys::jsize,
                    "[B",
                    jni::objects::JObject::null(),
                )
                .map_err(|_| ())?;
            for (index, field) in fields.iter().enumerate() {
                let array = env.byte_array_from_slice(field).map_err(|_| ())?;
                env.set_object_array_element(&output, index as jni::sys::jsize, &array)
                    .map_err(|_| ())?;
            }
            Ok(output.into_raw())
        },
    ));
    match result {
        Ok(Ok(output)) => output,
        _ => std::ptr::null_mut(),
    }
}
