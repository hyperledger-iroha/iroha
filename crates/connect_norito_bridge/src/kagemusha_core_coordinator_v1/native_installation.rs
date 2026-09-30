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
    inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
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
    ///
    /// No ordinary file store, Java callback, host selected root, inventory-only readiness or
    /// decoded policy qualifies these services. The separately governed platform integration
    /// owns that authentication, and `recheck_originals` must retain its custody throughout use.
    /// # Errors
    /// Rejects nonproduction release, policy/profile/class/scope/key mismatches before store I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn from_trusted_platform(
        inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
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
            inner,
            store,
            context,
            pins,
            policy,
            app_policy,
            release,
            native_key,
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
    /// Consume the exact freshly verified admission after its durable phase-5 publication.
    ///
    /// The platform must retain this nonserializable value and perform the independent enrolled
    /// account/device open and paired-proof/hardware/journal bootstrap. This admission alone
    /// grants no monetary authority. Uncertainty consumes the attempt; it cannot be recreated
    /// from the response or a persisted certificate. A successful return means the owner retained
    /// that consuming handoff, not that any wallet funds or hardware bootstrap were fabricated.
    fn retain_fresh_admission(
        &self,
        storage_path: &str,
        handle: u64,
        admission: FreshIssuerAdmissionV1,
    ) -> Result<(), Error>;
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
                selected.inner,
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
        });
        provisioner.recheck_originals()?;
        install(backend)?;
        self.installed_path = Some(path.into());
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
}

impl KagemushaCoreCoordinatorBackendV1 for InstalledEnrollmentBackend {
    fn open(&self, path: &str) -> Result<u64, Error> {
        self.provisioner.recheck_originals()?;
        let handle = self.adapter.open(path)?;
        if let Err(error) = self.provisioner.recheck_originals() {
            let _ = self.adapter.close(handle);
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
        let response = self.adapter.invoke(handle, method, frame)?;
        self.provisioner.recheck_originals()?;
        Ok(response)
    }
    fn invoke_initial_enrollment(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        self.provisioner.recheck_originals()?;
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
        let response = self.adapter.invoke_initial_enrollment(handle, frame)?;
        if phase_five && handoff.is_none() {
            let admission = self.adapter.take_fresh_admission(handle)?;
            // Durable publication already occurred. Failure drops the only consuming admission;
            // the global exclusive bridge then revokes the handle. No retry reconstructs it.
            self.provisioner
                .retain_fresh_admission(&self.path, handle, admission)?;
            self.provisioner.recheck_originals()?;
            *handoff = Some(frame.to_vec());
        }
        self.provisioner.recheck_originals()?;
        Ok(response)
    }
    fn acknowledge_committed_app_attest(
        &self,
        handle: u64,
        frame: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.provisioner.recheck_originals()?;
        let response = self
            .adapter
            .acknowledge_committed_app_attest(handle, frame)?;
        self.provisioner.recheck_originals()?;
        Ok(response)
    }
    fn export_outgoing_state_proof(
        &self,
        handle: u64,
        id: [u8; 32],
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1, Error>
    {
        self.provisioner.recheck_originals()?;
        let proof = self.adapter.export_outgoing_state_proof(handle, id)?;
        self.provisioner.recheck_originals()?;
        Ok(proof)
    }
    fn close(&self, handle: u64) -> Result<(), Error> {
        self.adapter.close(handle)
    }
}

#[cfg(test)]
mod tests;
