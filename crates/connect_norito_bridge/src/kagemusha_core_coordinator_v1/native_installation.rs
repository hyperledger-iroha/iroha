//! Native provisioning and install-once composition, with no application authority intake.
//!
//! An OEM/platform integration registers its Rust owner before the application calls install.
//! The application supplies only the already selected storage path. The owner must independently
//! authenticate its original configuration, non-forking hardware and rollback-protected store.
//! Registering a trait implementation or parsing policy bytes does not establish that authority.

use std::sync::{Arc, Mutex, OnceLock};

use iroha_data_model::kagemusha::{
    KagemushaAppAttestationAuthorityPolicyV1, KagemushaAuthenticatedReleaseV1,
    KagemushaDevicePublicKeyV1, KagemushaReleasePurposeV1, KagemushaRetailEnrollmentIssuerPolicyV1,
};

use super::{
    FreshIssuerAdmissionV1, KagemushaCoreCoordinatorBackendErrorV1 as Error,
    KagemushaCoreCoordinatorBackendV1, KagemushaCoreCoordinatorMethodV1,
    KagemushaEnrollmentAttemptJournalV1, KagemushaEnrollmentContextProviderV1,
    KagemushaEnrollmentJournalPinsV1, KagemushaEnrollmentJournalStoreV1,
    KagemushaEnrollmentLiveSelectionV1, KagemushaEnrollmentPhaseOneBackendV1,
    KagemushaEnrollmentProvisionedContextV1, KagemushaKernelEnrollmentDelegateV1,
    install_kagemusha_core_coordinator_backend_v1, kagemusha_core_coordinator_decode_request_v1,
    kagemusha_core_coordinator_validate_storage_path_v1,
};

/// Original, independently selected Rust dependencies for one native installation.
///
/// The private fields prevent a mobile frame from creating or modifying this selection. Its
/// constructor is a trusted Rust provisioning boundary, not a policy authenticator: the caller
/// must already hold the independently authenticated issuer/app policies and platform owners.
pub struct KagemushaNativeEnrollmentProvisioningV1 {
    selection: ProvisioningSelection,
}

enum ProvisioningSelection {
    Fresh(FreshProvisioning),
    Recovered {
        path: String,
        backend: Arc<super::KagemushaAuthenticatedRecoveredCoordinatorV1>,
    },
}

struct FreshProvisioning {
    store: Arc<dyn KagemushaEnrollmentJournalStoreV1>,
    context: Arc<dyn KagemushaEnrollmentContextProviderV1>,
    pins: KagemushaEnrollmentJournalPinsV1,
    policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
    app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    native_key: KagemushaDevicePublicKeyV1,
}

impl KagemushaNativeEnrollmentProvisioningV1 {
    /// Bind independently authenticated originals and actual qualified platform services.
    /// The factory supplies the concrete initial selection owner and actual enrollment kernel;
    /// the platform supplies physical custody/context/journals, never Core frame dispatch.
    ///
    /// No ordinary file store, Java callback, host selected root, inventory-only readiness or
    /// decoded policy qualifies these services. The separately governed platform integration
    /// owns that authentication, and `recheck_originals` must retain its custody throughout use.
    /// # Errors
    /// Rejects nonproduction release, policy/profile/class/scope/key mismatches before store I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn from_trusted_platform(
        store: Arc<dyn KagemushaEnrollmentJournalStoreV1>,
        context: Arc<dyn KagemushaEnrollmentContextProviderV1>,
        policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
        app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        native_key: KagemushaDevicePublicKeyV1,
        hardware_profile_id: [u8; 32],
    ) -> Result<Self, Error> {
        policy.validate().map_err(|_| Error::Rejected)?;
        native_key.validate().map_err(|_| Error::Rejected)?;
        let app_digest = app_policy.canonical_digest().map_err(|_| Error::Rejected)?;
        let enabled = release
            .enabled_profile(hardware_profile_id)
            .ok_or(Error::Rejected)?;
        if release.purpose() != KagemushaReleasePurposeV1::Production
            || release.network_id() != policy.runtime.network_id
            || enabled.hardware_profile.platform_class != app_policy.platform_class
            || enabled
                .hardware_profile
                .app_attestation_authority_policy_digest
                != app_digest
        {
            return Err(Error::Rejected);
        }
        let pins = KagemushaEnrollmentJournalPinsV1 {
            release_id: release.release_id(),
            hardware_profile_id,
            issuer_policy_id: policy.issuer_policy_id,
            app_policy_digest: app_digest,
        };
        pins.validate().map_err(|_| Error::Rejected)?;
        Ok(Self {
            selection: ProvisioningSelection::Fresh(FreshProvisioning {
                store,
                context,
                pins,
                policy,
                app_policy,
                release,
                native_key,
            }),
        })
    }

    /// Retain a completed production Core and its original native storage/key selection.
    /// Unlike fresh enrollment this never reopens an enrollment journal or recreates an issuer
    /// certificate. The held Core's real hardware and original journals authenticate recovery.
    /// # Errors
    /// Rejects invalid path, stale hardware, changed journals or absent production Core authority.
    pub fn from_recovered_core(
        native_storage_path: String,
        core: iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedCoreOwnerV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self, Error> {
        let backend = Arc::new(
            super::KagemushaAuthenticatedRecoveredCoordinatorV1::from_native_owner(
                native_storage_path.clone(),
                core,
                native_key,
                signer,
            )?,
        );
        Ok(Self {
            selection: ProvisioningSelection::Recovered {
                path: native_storage_path,
                backend,
            },
        })
    }
}

/// Trusted Rust-only platform intake; the C/JNI application cannot register or replace it.
pub trait KagemushaNativeEnrollmentProvisionerV1: Send + Sync + 'static {
    /// Recheck the held original provisioning and hardware/store authority, failing on drift.
    fn recheck_originals(&self) -> Result<(), Error>;
    /// Select the actual qualified owner for exactly this validated durable storage path.
    fn provision(
        &self,
        storage_path: &str,
    ) -> Result<KagemushaNativeEnrollmentProvisioningV1, Error>;
    /// Select independently retained physical bootstrap inputs for this native admission.
    /// The concrete bridge owns initialization, exact WAL retry and software Core dispatch.
    /// No Core backend, decoded owner or public checkpoint may be returned here.
    fn bootstrap_source(
        &self,
        storage_path: &str,
        handle: u64,
    ) -> Result<Arc<dyn super::KagemushaNativeCoreBootstrapSourceV1>, Error>;
}

/// Independently retained physical installation originals. This is a native OEM custody
/// boundary, not a role marker or public policy parser; C/JNI cannot provide this object.
pub trait KagemushaNativeEnrollmentOriginalCustodyV1: Send + Sync + 'static {
    /// Reauthenticate actual held policy/configuration, key, context and rollback-protected
    /// enrollment-store ownership. Refuse drift, unavailable hardware or uncertain publication.
    fn recheck_originals(&self) -> Result<(), Error>;
}

/// Concrete immutable path/selection owner for the native installer. Platform code supplies
/// physical custody and already authenticated typed selection, never software `provision`
/// dispatch. This wrapper consumes that selection once and rechecks both physical originals.
/// It does not manufacture OEM journal monotonicity, key custody, witnesses or release authority.
pub struct KagemushaPinnedNativeEnrollmentProvisionerV1 {
    path: Box<str>,
    selection: Mutex<Option<KagemushaNativeEnrollmentProvisioningV1>>,
    custody: Arc<dyn KagemushaNativeEnrollmentOriginalCustodyV1>,
    bootstrap: Arc<dyn super::KagemushaNativeCoreBootstrapSourceV1>,
}
impl KagemushaPinnedNativeEnrollmentProvisionerV1 {
    /// Pin the original native storage selection and physical services before application
    /// install. `selection` must come from the trusted typed fresh/recovered constructor;
    /// neither a public snapshot nor arbitrary backend can reach that intake.
    pub fn from_original_selection(
        path: String,
        selection: KagemushaNativeEnrollmentProvisioningV1,
        custody: Arc<dyn KagemushaNativeEnrollmentOriginalCustodyV1>,
        bootstrap: Arc<dyn super::KagemushaNativeCoreBootstrapSourceV1>,
    ) -> Result<Self, Error> {
        kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
            .map_err(|_| Error::Rejected)?;
        custody.recheck_originals()?;
        bootstrap.recheck_originals()?;
        if let ProvisioningSelection::Recovered { path: original, .. } = &selection.selection {
            if original != &path {
                return Err(Error::Rejected);
            }
        }
        custody.recheck_originals()?;
        Ok(Self {
            path: path.into_boxed_str(),
            selection: Mutex::new(Some(selection)),
            custody,
            bootstrap,
        })
    }
}
impl KagemushaNativeEnrollmentProvisionerV1 for KagemushaPinnedNativeEnrollmentProvisionerV1 {
    fn recheck_originals(&self) -> Result<(), Error> {
        self.custody.recheck_originals()?;
        self.bootstrap.recheck_originals()?;
        self.custody.recheck_originals()
    }
    fn provision(&self, path: &str) -> Result<KagemushaNativeEnrollmentProvisioningV1, Error> {
        self.recheck_originals()?;
        if path != self.path.as_ref() {
            return Err(Error::Rejected);
        }
        let selection = self
            .selection
            .lock()
            .map_err(|_| Error::Rejected)?
            .take()
            .ok_or(Error::Rejected)?;
        self.recheck_originals()?;
        Ok(selection)
    }
    fn bootstrap_source(
        &self,
        path: &str,
        original_handle: u64,
    ) -> Result<Arc<dyn super::KagemushaNativeCoreBootstrapSourceV1>, Error> {
        self.recheck_originals()?;
        if path != self.path.as_ref() || original_handle == 0 {
            return Err(Error::Rejected);
        }
        Ok(self.bootstrap.clone())
    }
}

/// Install-time failure for the independently governed native provisioning owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaNativeProvisionerInstallErrorV1 {
    /// A native owner already occupies the process-lifetime slot.
    AlreadyInstalled,
    /// Held original authority failed its independent recheck.
    Rejected,
}

static PROVISIONER: OnceLock<Arc<dyn KagemushaNativeEnrollmentProvisionerV1>> = OnceLock::new();
static INSTALLATION: Mutex<InstallationState> = Mutex::new(InstallationState::new());

/// Register the independently governed Rust platform owner once before mobile startup.
/// # Errors
/// Rejects changed originals or a previously registered owner; no replacement is permitted.
///
/// The trusted integration uses the public Rust intake, before any C/JNI install call:
/// ```no_run
/// use std::sync::Arc;
/// use connect_norito_bridge::{
///     KagemushaNativeEnrollmentProvisionerV1, KagemushaNativeProvisionerInstallErrorV1,
///     register_kagemusha_native_enrollment_provisioner_v1,
/// };
/// fn register_original_owner(
///     owner: Arc<dyn KagemushaNativeEnrollmentProvisionerV1>,
/// ) -> Result<(), KagemushaNativeProvisionerInstallErrorV1> {
///     register_kagemusha_native_enrollment_provisioner_v1(owner)
/// }
/// ```
pub fn register_kagemusha_native_enrollment_provisioner_v1(
    provisioner: Arc<dyn KagemushaNativeEnrollmentProvisionerV1>,
) -> Result<(), KagemushaNativeProvisionerInstallErrorV1> {
    provisioner
        .recheck_originals()
        .map_err(|_| KagemushaNativeProvisionerInstallErrorV1::Rejected)?;
    PROVISIONER
        .set(provisioner)
        .map_err(|_| KagemushaNativeProvisionerInstallErrorV1::AlreadyInstalled)
}

struct InstallationState {
    attempted: bool,
    installed_path: Option<Box<str>>,
}

impl InstallationState {
    const fn new() -> Self {
        Self {
            attempted: false,
            installed_path: None,
        }
    }

    fn install(
        &mut self,
        provisioner: Arc<dyn KagemushaNativeEnrollmentProvisionerV1>,
        path: &str,
        install: impl FnOnce(Arc<dyn KagemushaCoreCoordinatorBackendV1>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
            .map_err(|_| Error::Rejected)?;
        provisioner.recheck_originals()?;
        if let Some(original_path) = &self.installed_path {
            return if original_path.as_ref() == path {
                Ok(())
            } else {
                Err(Error::Rejected)
            };
        }
        if self.attempted {
            return Err(Error::Rejected);
        }
        // Consume the installation attempt before any platform/store operation. Unknown results
        // cannot reopen a journal, select another owner or attempt a second global installation.
        self.attempted = true;
        let selected = provisioner.provision(path)?;
        provisioner.recheck_originals()?;
        let selected = match selected.selection {
            ProvisioningSelection::Fresh(selected) => selected,
            ProvisioningSelection::Recovered {
                path: original,
                backend,
            } => {
                if original != path {
                    return Err(Error::Rejected);
                }
                let installed = Arc::new(InstalledRecoveredBackend {
                    backend,
                    provisioner: provisioner.clone(),
                });
                install(installed)?;
                provisioner.recheck_originals()?;
                self.installed_path = Some(path.to_owned().into_boxed_str());
                return Ok(());
            }
        };
        let journal = Arc::new(
            KagemushaEnrollmentAttemptJournalV1::open(selected.store)
                .map_err(|_| Error::Rejected)?,
        );
        let context = Arc::new(PinnedContext {
            original: selected.context,
            provisioner: provisioner.clone(),
            policy: selected.policy,
            app_policy: selected.app_policy,
            release: selected.release,
            native_key: selected.native_key,
        });
        let delegate = Arc::new(KagemushaKernelEnrollmentDelegateV1::new(context));
        let adapter = Arc::new(
            KagemushaEnrollmentPhaseOneBackendV1::new_with_qualified_enrollment(
                Arc::new(NativeInitialSelectionBackend::new(path)),
                delegate,
                journal,
                selected.pins,
                path,
            )?,
        );
        let backend = Arc::new(InstalledEnrollmentBackend {
            adapter,
            provisioner: provisioner.clone(),
            path: path.into(),
            handoff: Mutex::new(None),
            bootstrap: Mutex::new(None),
            recovered: Mutex::new(None),
            routes: Mutex::new(InstalledRoutes {
                next: 1,
                opening: false,
                current: None,
            }),
        });
        provisioner.recheck_originals()?;
        install(backend)?;
        self.installed_path = Some(path.into());
        Ok(())
    }
}

// Concrete process selection only. The real PhaseOne kernel owns qualification/issuer work;
// consuming issuer completion never turns this handle owner into a monetary Core backend.
struct NativeInitialSelectionBackend {
    path: Box<str>,
    selection: Mutex<(u64, Option<u64>)>,
}
impl NativeInitialSelectionBackend {
    fn new(path: &str) -> Self {
        Self {
            path: path.into(),
            selection: Mutex::new((1, None)),
        }
    }
}
impl KagemushaCoreCoordinatorBackendV1 for NativeInitialSelectionBackend {
    fn open(&self, path: &str) -> Result<u64, Error> {
        if path != self.path.as_ref() {
            return Err(Error::Rejected);
        }
        let mut selection = self.selection.lock().map_err(|_| Error::Rejected)?;
        let handle = selection.0;
        selection.0 = handle.checked_add(1).ok_or(Error::Rejected)?;
        selection.1 = Some(handle);
        Ok(handle)
    }
    fn invoke(
        &self,
        handle: u64,
        _: KagemushaCoreCoordinatorMethodV1,
        _: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let selection = self.selection.lock().map_err(|_| Error::Rejected)?;
        if selection.1 != Some(handle) {
            return Err(Error::Rejected);
        }
        Err(Error::Unavailable)
    }
    fn close(&self, handle: u64) -> Result<(), Error> {
        let mut selection = self.selection.lock().map_err(|_| Error::Rejected)?;
        if selection.1 != Some(handle) {
            return Err(Error::Rejected);
        }
        selection.1 = None;
        Ok(())
    }
}

/// Compose and install only the retained native owner's selection for this storage path.
///
/// This mobile-callable operation accepts no policy, key, root, provider or hardware claim.
/// An exact successful same-path retry is idempotent; a different path, prior unknown install
/// or uncertain platform result is rejected. Inventory readiness is not an installed backend.
/// # Errors
/// Returns `Unavailable` without a registered OEM owner, and rejects substituted/uncertain use.
pub fn provision_and_install_kagemusha_native_enrollment_v1(path: &str) -> Result<(), Error> {
    kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
        .map_err(|_| Error::Rejected)?;
    let source = PROVISIONER.get().ok_or(Error::Unavailable)?.clone();
    INSTALLATION
        .lock()
        .map_err(|_| Error::Rejected)?
        .install(source, path, |backend| {
            install_kagemusha_core_coordinator_backend_v1(backend).map_err(|_| Error::Rejected)
        })
}

struct InstalledRecoveredBackend {
    backend: Arc<super::KagemushaAuthenticatedRecoveredCoordinatorV1>,
    provisioner: Arc<dyn KagemushaNativeEnrollmentProvisionerV1>,
}
impl KagemushaCoreCoordinatorBackendV1 for InstalledRecoveredBackend {
    fn open(&self, path: &str) -> Result<u64, Error> {
        self.provisioner.recheck_originals()?;
        let handle = self.backend.open(path)?;
        if let Err(error) = self.provisioner.recheck_originals() {
            let _ = self.backend.close(handle);
            return Err(error);
        }
        Ok(handle)
    }
    fn invoke(
        &self,
        handle: u64,
        method: KagemushaCoreCoordinatorMethodV1,
        frame: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.provisioner.recheck_originals()?;
        let response = self.backend.invoke(handle, method, frame)?;
        self.provisioner.recheck_originals()?;
        Ok(response)
    }
    fn invoke_initial_enrollment(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        self.invoke(
            handle,
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
            frame,
        )
    }
    fn export_outgoing_state_proof(
        &self,
        handle: u64,
        operation: [u8; 32],
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1, Error>
    {
        self.provisioner.recheck_originals()?;
        let proof = self
            .backend
            .export_outgoing_state_proof(handle, operation)?;
        if let Err(error) = self.provisioner.recheck_originals() {
            let _ = self.backend.close(handle);
            return Err(error);
        }
        Ok(proof)
    }
    fn close(&self, handle: u64) -> Result<(), Error> {
        let closed = self.backend.close(handle);
        self.provisioner.recheck_originals()?;
        closed
    }
}

struct PinnedContext {
    original: Arc<dyn KagemushaEnrollmentContextProviderV1>,
    provisioner: Arc<dyn KagemushaNativeEnrollmentProvisionerV1>,
    policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
    app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    native_key: KagemushaDevicePublicKeyV1,
}

impl KagemushaEnrollmentContextProviderV1 for PinnedContext {
    fn context_for_selection(
        &self,
        handle: u64,
        live: &KagemushaEnrollmentLiveSelectionV1,
    ) -> Result<KagemushaEnrollmentProvisionedContextV1, Error> {
        self.provisioner.recheck_originals()?;
        live.require_live().map_err(|_| Error::Rejected)?;
        let context = self.original.context_for_selection(handle, live)?;
        if context.policy.as_ref() != self.policy.as_ref()
            || context.app_policy.as_ref() != self.app_policy.as_ref()
            || context.native_authorization_public_key != self.native_key
            || context.release.release_id() != self.release.release_id()
            || context.release.manifest_digest() != self.release.manifest_digest()
            || context.release.receipt_digest() != self.release.receipt_digest()
            || context.release.authority_policy_digest() != self.release.authority_policy_digest()
            || context.release.attestation_digest() != self.release.attestation_digest()
        {
            return Err(Error::Rejected);
        }
        self.provisioner.recheck_originals()?;
        live.require_live().map_err(|_| Error::Rejected)?;
        Ok(context)
    }
}

struct InstalledEnrollmentBackend {
    adapter: Arc<KagemushaEnrollmentPhaseOneBackendV1>,
    provisioner: Arc<dyn KagemushaNativeEnrollmentProvisionerV1>,
    path: Box<str>,
    // Only an exact successfully handed-off phase-5 request can replay its retained response.
    handoff: Mutex<Option<Vec<u8>>>,
    bootstrap: Mutex<Option<super::KagemushaNativeFreshCoreBootstrapV1>>,
    recovered: Mutex<Option<Arc<super::KagemushaAuthenticatedRecoveredCoordinatorV1>>>,
    routes: Mutex<InstalledRoutes>,
}

#[derive(Clone)]
enum InstalledRoute {
    Initial(u64),
    Recovered(
        Arc<super::KagemushaAuthenticatedRecoveredCoordinatorV1>,
        u64,
    ),
}
struct InstalledRoutes {
    next: u64,
    opening: bool,
    current: Option<(u64, InstalledRoute)>,
}
impl InstalledEnrollmentBackend {
    fn route(&self, handle: u64) -> Result<InstalledRoute, Error> {
        self.routes
            .lock()
            .map_err(|_| Error::Rejected)?
            .current
            .as_ref()
            .filter(|(original, _)| *original == handle)
            .map(|(_, route)| route.clone())
            .ok_or(Error::Rejected)
    }
    fn close_original(&self, route: &InstalledRoute) -> Result<(), Error> {
        match route {
            InstalledRoute::Initial(handle) => self.adapter.close(*handle),
            InstalledRoute::Recovered(backend, handle) => backend.close(*handle),
        }
    }
}

impl KagemushaCoreCoordinatorBackendV1 for InstalledEnrollmentBackend {
    fn open(&self, path: &str) -> Result<u64, Error> {
        self.provisioner.recheck_originals()?;
        if path != self.path.as_ref() {
            return Err(Error::Rejected);
        }
        let old = {
            let mut routes = self.routes.lock().map_err(|_| Error::Rejected)?;
            if routes.opening {
                return Err(Error::Rejected);
            }
            routes.opening = true;
            routes.current.take()
        };
        let result = (|| {
            if let Some((_, old)) = old {
                self.close_original(&old)?;
            }
            let handoff_done = self.handoff.lock().map_err(|_| Error::Rejected)?.is_some();
            let route = if handoff_done {
                let cached = self.recovered.lock().map_err(|_| Error::Rejected)?.clone();
                let owner = match cached {
                    Some(owner) => owner,
                    None => {
                        let mut pending = self
                            .bootstrap
                            .lock()
                            .map_err(|_| Error::Rejected)?
                            .take()
                            .ok_or(Error::Rejected)?;
                        // The exclusive pending owner remains retained even if hardware delivery
                        // is uncertain. Never keep a routing/global lock across physical calls.
                        let advanced = pending.advance();
                        *self.bootstrap.lock().map_err(|_| Error::Rejected)? = Some(pending);
                        let owner = advanced?;
                        self.provisioner.recheck_originals()?;
                        *self.recovered.lock().map_err(|_| Error::Rejected)? = Some(owner.clone());
                        owner
                    }
                };
                let handle = owner.open(path)?;
                InstalledRoute::Recovered(owner, handle)
            } else {
                InstalledRoute::Initial(self.adapter.open(path)?)
            };
            if let Err(error) = self.provisioner.recheck_originals() {
                let _ = self.close_original(&route);
                return Err(error);
            }
            Ok(route)
        })();
        let mut routes = self.routes.lock().map_err(|_| Error::Rejected)?;
        routes.opening = false;
        let route = result?;
        let handle = routes.next;
        let Some(next) = routes.next.checked_add(1) else {
            drop(routes);
            let _ = self.close_original(&route);
            return Err(Error::Rejected);
        };
        routes.next = next;
        routes.current = Some((handle, route));
        Ok(handle)
    }

    fn invoke(
        &self,
        handle: u64,
        method: KagemushaCoreCoordinatorMethodV1,
        frame: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.provisioner.recheck_originals()?;
        let response = match self.route(handle)? {
            InstalledRoute::Initial(original) => self.adapter.invoke(original, method, frame)?,
            InstalledRoute::Recovered(owner, original) => owner.invoke(original, method, frame)?,
        };
        self.provisioner.recheck_originals()?;
        self.route(handle)?;
        Ok(response)
    }
    fn invoke_initial_enrollment(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        self.provisioner.recheck_originals()?;
        let original = match self.route(handle)? {
            InstalledRoute::Initial(original) => original,
            InstalledRoute::Recovered(owner, original) => {
                let response = owner.invoke_initial_enrollment(original, frame)?;
                self.provisioner.recheck_originals()?;
                self.route(handle)?;
                return Ok(response);
            }
        };
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase_five = fields
            .first()
            .is_some_and(|phase| phase.as_slice() == 5_u32.to_le_bytes());
        let mut handoff = self.handoff.lock().map_err(|_| Error::Rejected)?;
        if phase_five
            && handoff
                .as_ref()
                .is_some_and(|original| original.as_slice() != frame)
        {
            return Err(Error::Rejected);
        }
        let response = self.adapter.invoke_initial_enrollment(original, frame)?;
        if phase_five && handoff.is_none() {
            let admission = self.adapter.take_fresh_admission(original)?;
            // Durable publication already occurred. Failure drops the only consuming admission;
            // the global exclusive bridge then revokes the handle. No retry reconstructs it.
            let source = self.provisioner.bootstrap_source(&self.path, original)?;
            let pending = super::KagemushaNativeFreshCoreBootstrapV1::retain(
                self.path.to_string(),
                admission,
                source,
            )?;
            *self.bootstrap.lock().map_err(|_| Error::Rejected)? = Some(pending);
            self.provisioner.recheck_originals()?;
            *handoff = Some(frame.to_vec());
        }
        drop(handoff);
        self.provisioner.recheck_originals()?;
        self.route(handle)?;
        Ok(response)
    }
    fn acknowledge_committed_app_attest(
        &self,
        handle: u64,
        frame: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.provisioner.recheck_originals()?;
        let response = match self.route(handle)? {
            InstalledRoute::Initial(original) => self
                .adapter
                .acknowledge_committed_app_attest(original, frame)?,
            InstalledRoute::Recovered(owner, original) => {
                owner.acknowledge_committed_app_attest(original, frame)?
            }
        };
        self.provisioner.recheck_originals()?;
        self.route(handle)?;
        Ok(response)
    }
    fn export_outgoing_state_proof(
        &self,
        handle: u64,
        id: [u8; 32],
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1, Error>
    {
        self.provisioner.recheck_originals()?;
        let proof = match self.route(handle)? {
            InstalledRoute::Initial(original) => {
                self.adapter.export_outgoing_state_proof(original, id)?
            }
            InstalledRoute::Recovered(owner, original) => {
                owner.export_outgoing_state_proof(original, id)?
            }
        };
        self.provisioner.recheck_originals()?;
        self.route(handle)?;
        Ok(proof)
    }
    fn close(&self, handle: u64) -> Result<(), Error> {
        let mut routes = self.routes.lock().map_err(|_| Error::Rejected)?;
        if routes
            .current
            .as_ref()
            .is_none_or(|(original, _)| *original != handle)
        {
            return Err(Error::Rejected);
        }
        let (_, route) = routes.current.take().ok_or(Error::Rejected)?;
        drop(routes);
        self.close_original(&route)
    }
}

#[cfg(test)]
mod tests;
