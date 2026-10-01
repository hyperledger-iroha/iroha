//! Called native C21 owner. App frames supply an original enrollment selector and bounded raw
//! platform originals; they cannot create C, replace policy/issuer/time or approve themselves.
//! Monetary methods stay unavailable until a separate genuine constrained financial owner exists.
use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error, KagemushaCoreCoordinatorBackendV1,
    KagemushaCoreCoordinatorMethodV1 as Method, install_kagemusha_core_coordinator_backend_v1,
    kagemusha_core_coordinator_decode_request_v1, kagemusha_core_coordinator_encode_response_v1,
    kagemusha_core_coordinator_validate_method_request_v1,
    kagemusha_core_coordinator_validate_method_response_v1,
    kagemusha_core_coordinator_validate_storage_path_v1,
};
use iroha_core_zk::kagemusha_v1_recursion::KagemushaAuthenticatedRecursiveVerifierV1;
use iroha_core_zk::kagemusha_v1_recursion::{
    KagemushaArtifactByteResolverV1, KagemushaProductionProverV1,
    KagemushaRecursiveVerifierProfileV1,
};
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaDurableCapacityV1, KagemushaNativeOrdinaryBootstrapOwnerV1 as BootstrapOwner,
    KagemushaOrdinaryAppEnrollmentAttemptV1 as Attempt,
    KagemushaOrdinaryAppPossessionAttemptV1 as Possession,
    KagemushaOrdinaryEnrolledFinancialOwnerV1 as FinancialOwner,
    KagemushaOrdinaryPreparationReservationV1 as Reservation,
    KagemushaOrdinaryPreparationSelectedOriginalsV1 as Selected,
    KagemushaOrdinaryRetailEnrollmentAttemptV1 as Retail,
};
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
};
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

/// Actual native choice between a never-created attempt and recovery of its original WAL.
/// This is supplied by the installed account/custody source, never a mobile request flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryEnrollmentDispositionV1 {
    /// Reserve exactly one fresh original; existing storage is refused without a fallback.
    Fresh,
    /// Reopen the exact previously retained original; missing storage is never recreated.
    Recover,
}
/// Concrete retained native account/release/policy selection and original storage directory.
/// This source performs no issuer HTTP callback. Mobile transports return untrusted originals
/// through explicit C21 intake, and the actual reservation/Attempt owners authenticate them.
pub struct KagemushaNativeOrdinaryAppIdentitySourceV1 {
    path: PathBuf,
    directory: File,
    selected: Arc<Selected>,
    preparation_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    platform_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    integrity_policy_original: Option<Vec<u8>>,
    bootstrap: Option<BootstrapMaterial>,
}
struct BootstrapMaterial {
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    capacity: KagemushaDurableCapacityV1,
    disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
}
impl KagemushaNativeOrdinaryAppIdentitySourceV1 {
    /// Bind actual independently selected native originals before any mobile call.
    /// Dispositions originate in native startup's exact original recovery state, never frame data.
    /// No decoded account/settings or unsigned policy can supply the selected opaque holder.
    /// # Errors
    /// Rejects another directory, expired selection or an Integrity original not pinned by trust.
    pub fn from_native_selected_originals(
        path: PathBuf,
        selected: Arc<Selected>,
        preparation_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
        platform_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
        integrity_policy_original: Option<Vec<u8>>,
    ) -> Result<Self, Error> {
        use sha2::{Digest as _, Sha256};
        kagemusha_core_coordinator_validate_storage_path_v1(
            path.to_str().ok_or(Error::Rejected)?.as_bytes(),
        )
        .map_err(|_| Error::Rejected)?;
        if path.canonicalize().map_err(|_| Error::Rejected)? != path {
            return Err(Error::Rejected);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| Error::Rejected)?;
        match (
            selected
                .integrity_policy_digest()
                .map_err(|_| Error::Rejected)?,
            &integrity_policy_original,
        ) {
            (None, None) => {}
            (Some(expected), Some(raw))
                if !raw.is_empty()
                    && raw.len() <= 16 * 1024
                    && <[u8; 32]>::from(Sha256::digest(raw)) == expected =>
            {
                iroha_data_model::kagemusha::kagemusha_play_integrity_provider_policy_projection_v1(raw)
                    .map_err(|_| Error::Rejected)?;
            }
            _ => return Err(Error::Rejected),
        }
        let this = Self {
            path,
            directory,
            selected,
            preparation_disposition,
            platform_disposition,
            integrity_policy_original,
            bootstrap: None,
        };
        this.recheck_originals(&this.path)?;
        Ok(this)
    }
    /// Bind the actual release verifier and native original recovery choice before installation.
    /// Verified lease holders come from independent native admission; decoded mobile fields do
    /// not create them. This supplies proving inputs, without asserting an initialized State.
    /// # Errors
    /// Rejects duplicate material, stale source, invalid capacity or fresh recovery lease inputs.
    pub fn with_native_bootstrap_material(
        mut self,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        capacity: KagemushaDurableCapacityV1,
        disposition: KagemushaOrdinaryEnrollmentDispositionV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ) -> Result<Self, Error> {
        self.recheck_originals(&self.path)?;
        capacity.validate().map_err(|_| Error::Rejected)?;
        if self.bootstrap.is_some()
            || (disposition == KagemushaOrdinaryEnrollmentDispositionV1::Fresh
                && !integrity_leases.is_empty())
        {
            return Err(Error::Rejected);
        }
        self.bootstrap = Some(BootstrapMaterial {
            verifier,
            capacity,
            disposition,
            integrity_leases,
        });
        self.recheck_originals(&self.path)?;
        Ok(self)
    }
    fn recheck_originals(&self, path: &Path) -> Result<(), Error> {
        self.selected
            .trusted_time_ms()
            .map_err(|_| Error::Rejected)?;
        let held = self.directory.metadata().map_err(|_| Error::Rejected)?;
        let current = std::fs::symlink_metadata(&self.path).map_err(|_| Error::Rejected)?;
        if path != self.path
            || !held.is_dir()
            || !current.is_dir()
            || current.file_type().is_symlink()
            || held.dev() != current.dev()
            || held.ino() != current.ino()
            || held.uid() != current.uid()
            || held.gid() != current.gid()
            || held.mode() != current.mode()
        {
            return Err(Error::Rejected);
        }
        Ok(())
    }
    fn original_enrollment_id(&self, path: &Path) -> Result<[u8; 32], Error> {
        self.recheck_originals(path)?;
        self.selected.enrollment_id().map_err(|_| Error::Rejected)
    }
    fn reserve_preparation(&self, path: &Path) -> Result<Reservation, Error> {
        self.recheck_originals(path)?;
        let now = self
            .selected
            .trusted_time_ms()
            .map_err(|_| Error::Rejected)?;
        let result = match self.preparation_disposition {
            KagemushaOrdinaryEnrollmentDispositionV1::Fresh => {
                Reservation::create(path, self.selected.clone(), now)
            }
            KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                Reservation::open_existing(path, self.selected.clone(), now)
            }
        }
        .map_err(|_| Error::Rejected)?;
        self.recheck_originals(path)?;
        Ok(result)
    }
}
/// Install-once ordinary source registration failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryAppIdentityInstallErrorV1 {
    /// The process already retains an independently selected ordinary source.
    AlreadyInstalled,
}
static SOURCE: OnceLock<Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>> = OnceLock::new();
static ACTIVE: OnceLock<Arc<OrdinaryBackend>> = OnceLock::new();
struct Installation {
    attempted_path: Option<Box<str>>,
    succeeded: bool,
}
static INSTALL: Mutex<Installation> = Mutex::new(Installation {
    attempted_path: None,
    succeeded: false,
});
/// Register the actual native original source before the application calls install.
/// C/JNI cannot register policy/keys/providers or construct an opaque C owner from decoded bytes.
/// This does not install financial capability or qualify a deployment.
/// # Errors
/// Refuses replacing an existing source.
pub fn register_kagemusha_native_ordinary_app_identity_source_v1(
    source: Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>,
) -> Result<(), KagemushaOrdinaryAppIdentityInstallErrorV1> {
    SOURCE
        .set(source)
        .map_err(|_| KagemushaOrdinaryAppIdentityInstallErrorV1::AlreadyInstalled)
}
/// Bootstrap the existing native installer from an already independently admitted account selection.
/// This Rust-only path calls actual source registration and the same production install dispatch
/// used by mobile startup. No C/JNI frame or decoded public policy can supply `selected`.
/// The selected root/account policies and native Fresh/Recover choice must come from product
/// provisioning; this function does not create them or grant an offline financial capability.
/// # Errors
/// Rejects foreign directory/policy/originals, occupied registration, or uncertain installation.
pub fn bootstrap_kagemusha_native_ordinary_app_identity_v1(
    path: PathBuf,
    selected: Arc<Selected>,
    preparation_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    platform_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    integrity_policy_original: Option<Vec<u8>>,
) -> Result<(), Error> {
    let source = Arc::new(
        KagemushaNativeOrdinaryAppIdentitySourceV1::from_native_selected_originals(
            path,
            selected,
            preparation_disposition,
            platform_disposition,
            integrity_policy_original,
        )?,
    );
    install_kagemusha_native_ordinary_source_v1(source)
}
/// Register and install the same actual native selected source, including its held bootstrap
/// material when present. C/JNI callers cannot create the required opaque source.
/// # Errors
/// Rejects an occupied registration, foreign original directory or uncertain installation.
pub fn install_kagemusha_native_ordinary_source_v1(
    source: Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>,
) -> Result<(), Error> {
    bootstrap_registered_source(
        source,
        |original| {
            register_kagemusha_native_ordinary_app_identity_source_v1(original)
                .map_err(|_| Error::Rejected)
        },
        super::provision_and_install_kagemusha_native_enrollment_v1,
    )
}
fn with_installed_bootstrap<T>(
    handle: u64,
    consume: impl FnOnce(&mut BootstrapOwner) -> Result<T, Error>,
) -> Result<T, Error> {
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable);
    }
    let backend = ACTIVE.get().ok_or(Error::Unavailable)?;
    if installed.attempted_path.as_deref() != backend.path.to_str() {
        return Err(Error::Rejected);
    }
    backend.source.recheck_originals(&backend.path)?;
    drop(installed);
    let mut owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
    if owner.handle != Some(handle) {
        return Err(Error::Rejected);
    }
    let initial = owner.bootstrap.as_mut().ok_or(Error::Unavailable)?;
    initial.enrollment().map_err(|_| Error::Rejected)?;
    let result = consume(initial)?;
    backend.source.recheck_originals(&backend.path)?;
    initial.enrollment().map_err(|_| Error::Rejected)?;
    Ok(result)
}
/// Generate the actual initial ordinary paired Guard under the same installed Native holder.
/// Loading release-pinned material and proving never dispatches another platform signature.
/// # Errors
/// Rejects a foreign/closed handle, missing captured W, stale custody or invalid released keys.
pub fn prove_kagemusha_native_ordinary_initial_guard_v1<R: KagemushaArtifactByteResolverV1>(
    handle: u64,
    profile: KagemushaRecursiveVerifierProfileV1,
    resolver: R,
) -> Result<Vec<u8>, Error> {
    with_installed_bootstrap(handle, |initial| {
        initial
            .with_selected_bootstrap(|selection, approval, financial| {
                let prover = KagemushaProductionProverV1::load_ordinary_bootstrap(
                    selection, profile, resolver,
                )
                .map_err(|error| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::RecoveryMaterial(
                        error.to_string(),
                    )
                })?;
                prover
                    .prove_ordinary_bootstrap_guard(selection, approval, financial)
                    .map_err(|error| {
                        iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::RecoveryMaterial(
                            error.to_string(),
                        )
                    })
            })
            .map_err(|_| Error::Rejected)
    })
}
/// Generate both real initial State parities and durably publish their exact Guard and W.
/// Native builds the public auxiliaries from its actual release material and Guard original,
/// then binds the held financial secret and admission. Success is the actual current publication.
/// # Errors
/// Rejects another owner/release, substituted proof inputs, stale current lease or failed proof.
pub fn publish_kagemusha_native_ordinary_initial_state_v1<R: KagemushaArtifactByteResolverV1>(
    handle: u64,
    profile: KagemushaRecursiveVerifierProfileV1,
    resolver: R,
    paired_guard_original: Vec<u8>,
) -> Result<(), Error> {
    with_installed_bootstrap(handle, |initial| {
        let prover = initial
            .with_selected_bootstrap(|selection, _, _| {
                KagemushaProductionProverV1::load_ordinary_bootstrap(selection, profile, resolver)
                    .map_err(|error| {
                        iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::RecoveryMaterial(
                            error.to_string(),
                        )
                    })
            })
            .map_err(|_| Error::Rejected)?;
        initial
            .prove_and_publish(&prover, paired_guard_original)
            .map_err(|_| Error::Rejected)
    })
}
/// Reopen the same actual published State after independent native FI and lease admission.
/// # Errors
/// Rejects absent, mixed or uncertain publication originals; missing storage is not recreated.
pub fn recover_kagemusha_native_ordinary_current_publication_v1(handle: u64) -> Result<(), Error> {
    with_installed_bootstrap(handle, |initial| {
        initial.recover_publication().map_err(|_| Error::Rejected)
    })
}
fn bootstrap_registered_source(
    source: Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>,
    register: impl FnOnce(Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>) -> Result<(), Error>,
    install: impl FnOnce(&str) -> Result<(), Error>,
) -> Result<(), Error> {
    source.recheck_originals(&source.path)?;
    register(source.clone())?;
    source.recheck_originals(&source.path)?;
    install(source.path.to_str().ok_or(Error::Rejected)?)?;
    source.recheck_originals(&source.path)
}
pub(super) fn has_registered_source() -> bool {
    SOURCE.get().is_some()
}
pub(super) fn provision_and_install(path: &str) -> Result<(), Error> {
    kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
        .map_err(|_| Error::Rejected)?;
    let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
    let path_buf = PathBuf::from(path);
    source.recheck_originals(&path_buf)?;
    let mut installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if let Some(original) = installed.attempted_path.as_deref() {
        return if original == path && installed.succeeded {
            source.recheck_originals(&path_buf)
        } else {
            Err(Error::Rejected)
        };
    }
    let backend = Arc::new(OrdinaryBackend {
        path: path_buf,
        source,
        owner: Mutex::new(Owner::default()),
    });
    backend.source.recheck_originals(&backend.path)?;
    // Any uncertain installation is frozen. Never retry replacement of process-global ownership.
    installed.attempted_path = Some(path.into());
    install_kagemusha_core_coordinator_backend_v1(backend.clone()).map_err(|_| Error::Rejected)?;
    ACTIVE.set(backend).map_err(|_| Error::Rejected)?;
    installed.succeeded = true;
    Ok(())
}
#[derive(Default)]
struct Owner {
    opened: bool,
    handle: Option<u64>,
    attempted: Option<[u8; 32]>,
    reservation_started: bool,
    reservation: Option<Reservation>,
    attempt: Option<Attempt>,
    possession_started: bool,
    possession: Option<Possession>,
    retail_started: bool,
    retail: Option<Retail>,
    financial: Option<FinancialOwner>,
    bootstrap_started: bool,
    bootstrap: Option<BootstrapOwner>,
}
struct OrdinaryBackend {
    path: PathBuf,
    source: Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>,
    owner: Mutex<Owner>,
}
impl OrdinaryBackend {
    fn invoke_bootstrap_approval(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        use iroha_data_model::kagemusha::{
            KagemushaAppKeySecurityLevelV1 as Security,
            KagemushaHardwarePlatformClassV1 as Platform,
        };
        let method = Method::PreparedAppOperationApproval;
        kagemusha_core_coordinator_validate_method_request_v1(method, frame)
            .map_err(|_| Error::Rejected)?;
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase = u32::from_le_bytes(
            fields[0]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?,
        );
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        // This owner has only the zero-State bootstrap. The ordinary monetary entry cannot
        // reserve it, even if the app presents the same operation identifier.
        if phase == 1 {
            return Err(Error::Unavailable);
        }
        let Owner {
            attempt, bootstrap, ..
        } = &mut *owner;
        let initial = bootstrap.as_mut().ok_or(Error::Unavailable)?;
        let response = if phase == 8 {
            use sha2::{Digest as _, Sha256};
            let operation_id: [u8; 32] = fields[1]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?;
            let mut hash = Sha256::new();
            hash.update(b"iroha:kagemusha:v1:ordinary-bootstrap-operation-id\0");
            hash.update(
                initial
                    .enrollment()
                    .map_err(|_| Error::Rejected)?
                    .certificate()
                    .canonical_bytes()
                    .map_err(|_| Error::Rejected)?,
            );
            if operation_id != <[u8; 32]>::from(hash.finalize()) {
                return Err(Error::Rejected);
            }
            let native = initial
                .prepare_bootstrap_platform(operation_id)
                .map_err(|_| Error::Rejected)?;
            if native.len() != 4 {
                return Err(Error::Rejected);
            }
            let enrollment = initial.enrollment().map_err(|_| Error::Rejected)?;
            let credential = enrollment.app_credential();
            let subject = credential.subject();
            let challenge = &enrollment.possession().challenge().preparation.challenge;
            let pending = attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .retained_pending_identity()
                .map_err(|_| Error::Rejected)?;
            if pending.raw_admission().subject().app_public_key != subject.app_public_key
                || pending.raw_admission().subject().attested_key_id != subject.attested_key_id
            {
                return Err(Error::Rejected);
            }
            let (platform, mask, floor) = match (subject.platform_class, subject.security_level) {
                (Platform::AndroidKeyMint, Security::TrustedExecutionEnvironment) => {
                    (5, 1, Vec::new())
                }
                (Platform::AndroidKeyMint, Security::StrongBox) => (5, 2, Vec::new()),
                (Platform::AppleAppAttest, Security::AppleAppAttest) => (
                    4,
                    0,
                    enrollment
                        .possession()
                        .app_attest_counter()
                        .ok_or(Error::Rejected)?
                        .max(subject.app_attest_counter_floor)
                        .to_le_bytes()
                        .to_vec(),
                ),
                _ => return Err(Error::Rejected),
            };
            // C and the persistent alias are data from the same verified FI enrollment. Their
            // old preparation interval is not renewed; the new W is reserved by Native custody.
            vec![
                native[0].clone(),
                native[1].clone(),
                vec![platform],
                pending.original_alias().as_bytes().to_vec(),
                challenge
                    .attestation_challenge()
                    .map_err(|_| Error::Rejected)?
                    .to_vec(),
                subject.app_public_key.as_sec1_bytes().to_vec(),
                subject.attested_key_id.to_vec(),
                challenge
                    .canonical_signing_bytes()
                    .map_err(|_| Error::Rejected)?,
                credential.digest().to_vec(),
                native[2].clone(),
                floor,
                vec![mask],
                subject.app_signing_identity_digest.to_vec(),
                native[3].clone(),
            ]
        } else {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            match phase {
                2 => initial
                    .fence_bootstrap_platform(ticket)
                    .map_err(|_| Error::Rejected)?,
                3 => vec![
                    initial
                        .retain_bootstrap_platform_original(ticket, &fields[2])
                        .map_err(|_| Error::Rejected)?
                        .to_vec(),
                ],
                4 => vec![
                    initial
                        .consume_bootstrap_platform(ticket)
                        .map_err(|_| Error::Rejected)?,
                ],
                5 => initial
                    .recover_bootstrap_platform(ticket)
                    .map_err(|_| Error::Rejected)?,
                6 => initial
                    .recheck_bootstrap_platform(ticket)
                    .map_err(|_| Error::Rejected)?,
                7 => {
                    initial
                        .cancel_bootstrap_platform(ticket)
                        .map_err(|_| Error::Rejected)?;
                    Vec::new()
                }
                _ => return Err(Error::Rejected),
            }
        };
        self.source.recheck_originals(&self.path)?;
        let response = kagemusha_core_coordinator_encode_response_v1(&response)
            .map_err(|_| Error::Rejected)?;
        kagemusha_core_coordinator_validate_method_response_v1(method, frame, &response)
            .map_err(|_| Error::Rejected)?;
        Ok(response)
    }
    fn invoke_retail(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        let method = Method::PreparedAppEnrollmentPossession;
        let f = kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase = u32::from_le_bytes(f[0].as_slice().try_into().map_err(|_| Error::Rejected)?);
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        if phase == 9 {
            let Owner {
                attempt,
                possession,
                reservation,
                retail_started,
                retail,
                ..
            } = &mut *owner;
            let pending = attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .retained_pending_identity()
                .map_err(|_| Error::Rejected)?;
            let app = possession.as_ref().ok_or(Error::Rejected)?;
            if app.ticket()
                != u64::from_le_bytes(f[1].as_slice().try_into().map_err(|_| Error::Rejected)?)
            {
                return Err(Error::Rejected);
            }
            if retail.is_none() {
                if *retail_started {
                    return Err(Error::Rejected);
                }
                *retail_started = true;
                let root = self
                    .path
                    .join(hex::encode(self.source.original_enrollment_id(&self.path)?))
                    .join("retail-enrollment");
                let reserve = reservation.as_ref().ok_or(Error::Rejected)?;
                *retail = Some(
                    match self.source.platform_disposition {
                        KagemushaOrdinaryEnrollmentDispositionV1::Fresh => Retail::create(
                            &root,
                            pending,
                            app,
                            reserve,
                            &f[2],
                            f[3].as_slice().try_into().map_err(|_| Error::Rejected)?,
                        ),
                        KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                            Retail::open_existing(&root, pending, app, reserve)
                        }
                    }
                    .map_err(|_| Error::Rejected)?,
                );
            }
            let fields = retail
                .as_ref()
                .ok_or(Error::Rejected)?
                .preparation_fields(pending, app)
                .map_err(|_| Error::Rejected)?;
            if fields[1] != f[2] || fields[2] != f[3] {
                return Err(Error::Rejected);
            }
        }
        let response = {
            let Owner {
                attempt,
                possession,
                reservation,
                retail,
                financial,
                bootstrap_started,
                bootstrap,
                ..
            } = &mut *owner;
            let pending = attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .retained_pending_identity()
                .map_err(|_| Error::Rejected)?;
            let app = possession.as_ref().ok_or(Error::Rejected)?;
            let held = retail.as_mut().ok_or(Error::Rejected)?;
            if phase != 9
                && held.ticket()
                    != u64::from_le_bytes(f[1].as_slice().try_into().map_err(|_| Error::Rejected)?)
            {
                return Err(Error::Rejected);
            }
            let response = match phase {
                9 => held
                    .preparation_fields(pending, app)
                    .map_err(|_| Error::Rejected)?,
                10 => held.fence(pending, app).map_err(|_| Error::Rejected)?,
                11 => vec![
                    held.retain_account_signature(
                        pending,
                        app,
                        f[2].as_slice().try_into().map_err(|_| Error::Rejected)?,
                    )
                    .map_err(|_| Error::Rejected)?
                    .to_vec(),
                ],
                12 => {
                    let enrollment = held
                        .accept_certificate(pending, app, &f[2])
                        .map_err(|_| Error::Rejected)?;
                    if let Some(initial) = bootstrap {
                        if !Arc::ptr_eq(
                            initial.enrollment().map_err(|_| Error::Rejected)?,
                            &enrollment,
                        ) {
                            return Err(Error::Rejected);
                        }
                    } else if *bootstrap_started {
                        return Err(Error::Rejected);
                    } else if let Some(fin) = financial {
                        fin.recheck().map_err(|_| Error::Rejected)?;
                        if !Arc::ptr_eq(fin.enrollment(), &enrollment) {
                            return Err(Error::Rejected);
                        }
                    } else {
                        let reserve = reservation.take().ok_or(Error::Rejected)?;
                        match reserve.complete_enrollment_or_retain(enrollment.clone()) {
                            Ok(fin) => *financial = Some(fin),
                            Err((original, _)) => {
                                *reservation = Some(original);
                                return Err(Error::Rejected);
                            }
                        }
                    }
                    if bootstrap.is_none()
                        && let Some(material) = &self.source.bootstrap
                    {
                        *bootstrap_started = true;
                        let financial = financial.take().ok_or(Error::Rejected)?;
                        let path = self
                            .path
                            .join(hex::encode(enrollment.certificate().subject.enrollment_id));
                        *bootstrap = Some(
                            match material.disposition {
                                KagemushaOrdinaryEnrollmentDispositionV1::Fresh => {
                                    BootstrapOwner::create_new(
                                        path,
                                        financial,
                                        material.verifier.clone(),
                                        material.capacity,
                                    )
                                }
                                KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                                    BootstrapOwner::open_existing(
                                        path,
                                        financial,
                                        material.verifier.clone(),
                                        material.capacity,
                                        &material.integrity_leases,
                                    )
                                }
                            }
                            .map_err(|_| Error::Rejected)?,
                        );
                    }
                    vec![
                        enrollment.certificate().subject.enrollment_id.to_vec(),
                        pending.native_scope().to_vec(),
                    ]
                }
                13 => held
                    .recovery_fields(pending, app)
                    .map_err(|_| Error::Rejected)?,
                14 => {
                    held.cancel(pending, app).map_err(|_| Error::Rejected)?;
                    Vec::new()
                }
                _ => return Err(Error::Rejected),
            };
            if phase != 14 {
                held.recheck(pending, app).map_err(|_| Error::Rejected)?;
            }
            response
        };
        self.source.recheck_originals(&self.path)?;
        let response = kagemusha_core_coordinator_encode_response_v1(&response)
            .map_err(|_| Error::Rejected)?;
        kagemusha_core_coordinator_validate_method_response_v1(method, frame, &response)
            .map_err(|_| Error::Rejected)?;
        Ok(response)
    }
    fn invoke_possession(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        let method = Method::PreparedAppEnrollmentPossession;
        kagemusha_core_coordinator_validate_method_request_v1(method, frame)
            .map_err(|_| Error::Rejected)?;
        let f = kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase = u32::from_le_bytes(f[0].as_slice().try_into().map_err(|_| Error::Rejected)?);
        if phase >= 9 {
            return self.invoke_retail(handle, frame);
        }
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        // Waiting for another native holder never backdates a new E consumption.
        let now = self
            .source
            .selected
            .trusted_time_ms()
            .map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        if phase == 1 {
            let selector: [u8; 32] = f[1].as_slice().try_into().map_err(|_| Error::Rejected)?;
            // Cold recovery reads existing originals only. It never reserves another secret,
            // generates C/key, fetches issuer data or retries an unknown device invocation.
            if owner.attempt.is_none() {
                if owner.attempted.is_some()
                    || self.source.platform_disposition
                        != KagemushaOrdinaryEnrollmentDispositionV1::Recover
                {
                    return Err(Error::Rejected);
                }
                owner.attempted = Some(self.source.original_enrollment_id(&self.path)?);
                if owner.reservation.is_none() {
                    owner.reservation_started = true;
                    owner.reservation = Some(
                        Reservation::open_retained_originals(
                            &self.path,
                            self.source.selected.clone(),
                            now,
                        )
                        .map_err(|_| Error::Rejected)?,
                    );
                }
                let prepared = owner
                    .reservation
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .retained_prepared_owner()
                    .map_err(|_| Error::Rejected)?;
                owner.attempt = Some(
                    Attempt::open_retained_originals(&self.path, prepared, now)
                        .map_err(|_| Error::Rejected)?,
                );
            }
            let pending = owner
                .attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .retained_pending_identity()
                .map_err(|_| Error::Rejected)?;
            if pending
                .preparation()
                .retained_preparation(now)
                .map_err(|_| Error::Rejected)?
                .challenge
                .attestation_challenge()
                .map_err(|_| Error::Rejected)?
                != selector
            {
                return Err(Error::Rejected);
            }
            if owner.possession.is_none() {
                if owner.possession_started {
                    return Err(Error::Rejected);
                }
                owner.possession_started = true;
                let pending = owner
                    .attempt
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .retained_pending_identity()
                    .map_err(|_| Error::Rejected)?;
                let root = self
                    .path
                    .join(hex::encode(self.source.original_enrollment_id(&self.path)?))
                    .join("possession");
                let result = match self.source.platform_disposition {
                    KagemushaOrdinaryEnrollmentDispositionV1::Fresh => {
                        Possession::create(&root, pending, now)
                    }
                    KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                        Possession::open_existing(&root, pending, now)
                    }
                }
                .map_err(|_| Error::Rejected)?;
                owner.possession = Some(result);
            }
        }
        let Owner {
            attempt,
            possession,
            ..
        } = &mut *owner;
        let pending = attempt
            .as_ref()
            .ok_or(Error::Rejected)?
            .retained_pending_identity()
            .map_err(|_| Error::Rejected)?;
        let held = possession.as_mut().ok_or(Error::Rejected)?;
        let response = if phase == 1 {
            held.preparation_fields(
                pending,
                f[1].as_slice().try_into().map_err(|_| Error::Rejected)?,
                now,
            )
            .map_err(|_| Error::Rejected)?
        } else {
            if held.ticket()
                != u64::from_le_bytes(f[1].as_slice().try_into().map_err(|_| Error::Rejected)?)
            {
                return Err(Error::Rejected);
            }
            match phase {
                2 => held.fence(pending, now).map_err(|_| Error::Rejected)?,
                3 => vec![
                    held.retain(pending, &f[2], now)
                        .map_err(|_| Error::Rejected)?
                        .to_vec(),
                ],
                4 => vec![held.consume(pending, now).map_err(|_| Error::Rejected)?],
                5 => held
                    .recovery_fields(pending, now)
                    .map_err(|_| Error::Rejected)?,
                6 => held
                    .recheck_fields(pending, now)
                    .map_err(|_| Error::Rejected)?,
                7 => {
                    held.cancel(pending, now).map_err(|_| Error::Rejected)?;
                    Vec::new()
                }
                8 => held
                    .accept_final_credential(pending, &f[2], now)
                    .map_err(|_| Error::Rejected)?,
                _ => return Err(Error::Rejected),
            }
        };
        self.source.recheck_originals(&self.path)?;
        let after = self
            .source
            .selected
            .trusted_time_ms()
            .map_err(|_| Error::Rejected)?;
        if phase == 1 {
            held.preparation_fields(
                pending,
                f[1].as_slice().try_into().map_err(|_| Error::Rejected)?,
                after,
            )
            .map_err(|_| Error::Rejected)?;
        } else if phase != 7 {
            held.recheck(pending, after).map_err(|_| Error::Rejected)?;
        }
        let response = kagemusha_core_coordinator_encode_response_v1(&response)
            .map_err(|_| Error::Rejected)?;
        kagemusha_core_coordinator_validate_method_response_v1(method, frame, &response)
            .map_err(|_| Error::Rejected)?;
        Ok(response)
    }
    fn invoke_identity(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        let method = Method::PreparedOrdinaryAppIdentity;
        kagemusha_core_coordinator_validate_method_request_v1(method, frame)
            .map_err(|_| Error::Rejected)?;
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase = u32::from_le_bytes(
            fields[0]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?,
        );
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        let response = if phase == 11 {
            let id = self.source.original_enrollment_id(&self.path)?;
            self.source.recheck_originals(&self.path)?;
            if id == [0; 32] || owner.attempted.is_some_and(|original| original != id) {
                return Err(Error::Rejected);
            }
            vec![id.to_vec()]
        } else if phase == 12 {
            if owner.reservation.is_none() {
                if owner.reservation_started {
                    return Err(Error::Rejected);
                }
                owner.reservation_started = true;
                owner.reservation = Some(self.source.reserve_preparation(&self.path)?);
            }
            let held = owner.reservation.as_ref().ok_or(Error::Rejected)?;
            let carrier = held.carrier().map_err(|_| Error::Rejected)?;
            let mut uuid = carrier.client_nonce[..16].to_vec();
            uuid[6] = (uuid[6] & 0x0f) | 0x40;
            uuid[8] = (uuid[8] & 0x3f) | 0x80;
            let u = hex::encode(uuid);
            let request_id = format!(
                "{}-{}-{}-{}-{}",
                &u[..8],
                &u[8..12],
                &u[12..16],
                &u[16..20],
                &u[20..]
            );
            vec![
                held.ticket()
                    .map_err(|_| Error::Rejected)?
                    .to_le_bytes()
                    .to_vec(),
                carrier.account_i105.as_bytes().to_vec(),
                carrier.client_nonce.to_vec(),
                carrier.release_id.to_vec(),
                carrier.hardware_profile_id.to_vec(),
                carrier.lane_id.to_vec(),
                carrier.financial_authority_commitment.to_vec(),
                request_id.into_bytes(),
            ]
        } else if phase == 13 {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            let reservation = owner.reservation.as_mut().ok_or(Error::Rejected)?;
            if reservation.ticket().map_err(|_| Error::Rejected)? != ticket {
                return Err(Error::Rejected);
            }
            reservation
                .retain_preparation(&fields[2])
                .map_err(|_| Error::Rejected)?;
            if owner.attempt.is_none() {
                let id = self.source.original_enrollment_id(&self.path)?;
                if owner.attempted.is_some() {
                    return Err(Error::Rejected);
                }
                owner.attempted = Some(id);
                let prepared = owner
                    .reservation
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .prepared_owner()
                    .map_err(|_| Error::Rejected)?;
                let now = self
                    .source
                    .selected
                    .trusted_time_ms()
                    .map_err(|_| Error::Rejected)?;
                let attempt = match self.source.platform_disposition {
                    KagemushaOrdinaryEnrollmentDispositionV1::Fresh => {
                        Attempt::create(&self.path, prepared, now)
                    }
                    KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                        Attempt::open_existing(&self.path, prepared, now)
                    }
                }
                .map_err(|_| Error::Rejected)?;
                owner.attempt = Some(attempt);
            }
            owner
                .attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .preparation_fields()
                .map_err(|_| Error::Rejected)?
        } else if phase == 14 {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            if owner
                .reservation
                .as_ref()
                .ok_or(Error::Rejected)?
                .ticket()
                .map_err(|_| Error::Rejected)?
                != ticket
            {
                return Err(Error::Rejected);
            }
            vec![
                self.source
                    .integrity_policy_original
                    .clone()
                    .unwrap_or_default(),
            ]
        } else {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            let attempt = owner.attempt.as_mut().ok_or(Error::Rejected)?;
            if attempt.ticket() != ticket {
                return Err(Error::Rejected);
            }
            attempt.recheck().map_err(|_| Error::Rejected)?;
            match phase {
                2 => attempt.fence_generation().map_err(|_| Error::Rejected)?,
                3 => vec![
                    attempt
                        .retain_key_reference(
                            std::str::from_utf8(&fields[2]).map_err(|_| Error::Rejected)?,
                        )
                        .map_err(|_| Error::Rejected)?
                        .to_vec(),
                ],
                4 => attempt.fence_attestation().map_err(|_| Error::Rejected)?,
                5 => {
                    let point = KagemushaDevicePublicKeyV1::from_sec1_bytes(&fields[2])
                        .map_err(|_| Error::Rejected)?;
                    // Shape checked above before allocation; at most128KiB original retained once.
                    let mut original = Vec::with_capacity(fields[3].len() + fields[4].len());
                    original.extend_from_slice(&fields[3]);
                    original.extend_from_slice(&fields[4]);
                    attempt
                        .retain_raw(point, &original)
                        .map_err(|_| Error::Rejected)?
                }
                6 => attempt
                    .accept_raw_admission(&fields[2])
                    .map_err(|_| Error::Rejected)?,
                7 => attempt.recovery_fields().map_err(|_| Error::Rejected)?,
                8 => attempt.recheck_fields().map_err(|_| Error::Rejected)?,
                9 => {
                    attempt.cancel().map_err(|_| Error::Rejected)?;
                    Vec::new()
                }
                10 => attempt
                    .raw_chunk_fields(u32::from_le_bytes(
                        fields[2]
                            .as_slice()
                            .try_into()
                            .map_err(|_| Error::Rejected)?,
                    ))
                    .map_err(|_| Error::Rejected)?,
                _ => return Err(Error::Rejected),
            }
        };
        self.source.recheck_originals(&self.path)?;
        if !matches!(phase, 9 | 11 | 12 | 14) {
            owner
                .attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .recheck()
                .map_err(|_| Error::Rejected)?;
        }
        let response = kagemusha_core_coordinator_encode_response_v1(&response)
            .map_err(|_| Error::Rejected)?;
        kagemusha_core_coordinator_validate_method_response_v1(method, frame, &response)
            .map_err(|_| Error::Rejected)?;
        Ok(response)
    }
}
impl KagemushaCoreCoordinatorBackendV1 for OrdinaryBackend {
    fn open(&self, path: &str) -> Result<u64, Error> {
        if self.path != Path::new(path) {
            return Err(Error::Rejected);
        }
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.opened {
            return Err(Error::Rejected);
        }
        owner.opened = true;
        owner.handle = Some(1);
        self.source.recheck_originals(&self.path)?;
        Ok(1)
    }
    fn invoke(&self, handle: u64, method: Method, frame: &[u8]) -> Result<Vec<u8>, Error> {
        match method {
            Method::PreparedAppOperationApproval => self.invoke_bootstrap_approval(handle, frame),
            Method::PreparedOrdinaryAppIdentity => self.invoke_identity(handle, frame),
            Method::PreparedAppEnrollmentPossession => self.invoke_possession(handle, frame),
            // No ordinary identity credential or possession receipt supplies the independently
            // constrained State/Guard, nonce and protection owner required by financial methods.
            _ => Err(Error::Unavailable),
        }
    }
    fn close(&self, handle: u64) -> Result<(), Error> {
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        owner.handle = None;
        owner.bootstrap = None;
        owner.financial = None;
        owner.retail = None;
        owner.possession = None;
        owner.attempt = None;
        owner.reservation = None;
        self.source.recheck_originals(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        kagemusha::*,
        testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture,
    };
    use p256::ecdsa::SigningKey;
    use sha2::{Digest as _, Sha256};
    fn backend() -> (tempfile::TempDir, OrdinaryBackend, Fixture) {
        let f = Fixture::new(true);
        let key = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
        let core_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let selected = Arc::new(
            Selected::from_selected_originals(
                f.selection.owner.clone(),
                f.release.clone(),
                f.issuer_policy.clone(),
                f.trust.clone(),
                f.app_authority.clone(),
                f.selection.preparation.challenge.hardware_profile_id,
                &core_key,
                300,
            )
            .unwrap(),
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        let source = Arc::new(
            KagemushaNativeOrdinaryAppIdentitySourceV1::from_native_selected_originals(
                path.clone(),
                selected,
                KagemushaOrdinaryEnrollmentDispositionV1::Fresh,
                KagemushaOrdinaryEnrollmentDispositionV1::Fresh,
                None,
            )
            .unwrap(),
        );
        (
            temp,
            OrdinaryBackend {
                path,
                source,
                owner: Mutex::new(Owner::default()),
            },
            f,
        )
    }
    fn call(
        b: &OrdinaryBackend,
        h: u64,
        phase: u32,
        mut fields: Vec<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>, Error> {
        fields.insert(0, phase.to_le_bytes().to_vec());
        let q = super::super::kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
        let result = b.invoke(h, Method::PreparedOrdinaryAppIdentity, &q)?;
        super::super::kagemusha_core_coordinator_decode_response_v1(&result)
            .map_err(|_| Error::Rejected)
    }
    fn prepare(b: &OrdinaryBackend, h: u64, f: &Fixture) -> Vec<Vec<u8>> {
        prepare_with_lifetime(b, h, f, 1900)
    }
    fn prepare_with_lifetime(
        b: &OrdinaryBackend,
        h: u64,
        f: &Fixture,
        lifetime_ms: u64,
    ) -> Vec<Vec<u8>> {
        let carrier = call(b, h, 12, vec![]).unwrap();
        assert_eq!(carrier.len(), 8);
        assert_eq!(call(b, h, 12, vec![]).unwrap(), carrier);
        let mut c = f.selection.preparation.challenge;
        c.client_nonce = carrier[2].as_slice().try_into().unwrap();
        c.financial_authority_commitment = carrier[6].as_slice().try_into().unwrap();
        c.issued_at_ms = b.source.selected.trusted_time_ms().unwrap();
        c.expires_at_ms = c.issued_at_ms + lifetime_ms;
        let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let signed = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
            challenge: c,
            signature: Signature::try_new(key.private_key(), &c.canonical_signing_bytes().unwrap())
                .unwrap(),
        }
        .to_transport_bytes()
        .unwrap();
        let result = call(b, h, 13, vec![carrier[0].clone(), signed.clone()]).unwrap();
        assert_eq!(result.len(), 8);
        assert_eq!(result[1], signed);
        assert_eq!(
            call(b, h, 13, vec![carrier[0].clone(), signed]).unwrap(),
            result
        );
        assert_eq!(
            call(b, h, 14, vec![carrier[0].clone()]).unwrap(),
            vec![Vec::<u8>::new()]
        );
        result
    }
    #[test]
    fn called_c21_reserves_before_http_and_fences_identity_without_money() {
        let (_temp, b, f) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        let id = call(&b, h, 11, vec![]).unwrap();
        assert_eq!(
            id,
            vec![f.selection.preparation.challenge.enrollment_id.to_vec()]
        );
        assert!(b.owner.lock().unwrap().reservation.is_none());
        assert!(call(&b, h, 1, vec![id[0].clone()]).is_err());
        let prepared = prepare(&b, h, &f);
        let ticket = prepared[0].clone();
        assert_eq!(
            call(&b, h, 2, vec![ticket.clone()]).unwrap(),
            vec![vec![1], vec![]]
        );
        assert!(call(&b, h, 2, vec![ticket.clone()]).is_err());
        assert!(b.invoke(h, Method::InitialEnrollment, &[]).is_err());
        assert!(b.invoke(h, Method::BeginSenderTransition, &[]).is_err());
        assert!(
            b.invoke(h, Method::PreparedAppOperationApproval, &[])
                .is_err()
        );
        assert_eq!(call(&b, h, 7, vec![ticket.clone()]).unwrap()[0], vec![1]);
        assert!(call(&b, h, 9, vec![ticket]).is_err());
    }
    #[test]
    fn called_c21_authenticates_explicit_raw_original_and_preserves_two_chunks() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let (_temp, b, f) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        let prepared = prepare(&b, h, &f);
        let ticket = prepared[0].clone();
        let c = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&prepared[1])
            .unwrap()
            .challenge;
        let app = f.selection.issuance.credential.subject;
        call(&b, h, 2, vec![ticket.clone()]).unwrap();
        call(
            &b,
            h,
            3,
            vec![
                ticket.clone(),
                STANDARD.encode(app.attested_key_id).into_bytes(),
            ],
        )
        .unwrap();
        call(&b, h, 4, vec![ticket.clone()]).unwrap();
        assert!(call(&b, h, 4, vec![ticket.clone()]).is_err());
        let raw = vec![23; 131072];
        let retained = call(
            &b,
            h,
            5,
            vec![
                ticket.clone(),
                app.app_public_key.as_sec1_bytes().to_vec(),
                raw[..65536].to_vec(),
                raw[65536..].to_vec(),
            ],
        )
        .unwrap();
        assert_eq!(retained[0], Sha256::digest(&raw).to_vec());
        // A real governed Ed signature isolates raw issuer/custody joins; synthetic raw bytes do
        // not establish Apple attestation, a financial proof or a physically qualified deployment.
        let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            platform_class: c.platform_class,
            security_level: KagemushaAppKeySecurityLevelV1::AppleAppAttest,
            app_public_key: app.app_public_key,
            attested_key_id: app.attested_key_id,
            raw_platform_evidence_digest: Sha256::digest(&raw).into(),
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let original = KagemushaRawAppAttestationAdmissionV1 {
            subject,
            signature: Signature::try_new(
                key.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        }
        .to_transport_bytes()
        .unwrap();
        assert!(call(&b, h, 6, vec![ticket.clone()]).is_err());
        assert!(call(&b, h, 6, vec![ticket.clone(), vec![1; 314]]).is_err());
        let result = call(&b, h, 6, vec![ticket.clone(), original.clone()]).unwrap();
        assert_eq!(
            call(&b, h, 6, vec![ticket.clone(), original]).unwrap(),
            result
        );
        let recovered = call(&b, h, 7, vec![ticket.clone()]).unwrap();
        assert_eq!(recovered[0], vec![5]);
        assert_eq!(recovered[5].len(), 314);
        assert_eq!(recovered[6], result[0]);
        let mut joined =
            call(&b, h, 10, vec![ticket.clone(), 0u32.to_le_bytes().to_vec()]).unwrap()[1].clone();
        joined.extend(
            call(&b, h, 10, vec![ticket.clone(), 1u32.to_le_bytes().to_vec()]).unwrap()[1].iter(),
        );
        assert_eq!(joined, raw);
        assert!(
            call(
                &b,
                h,
                5,
                vec![
                    ticket.clone(),
                    app.app_public_key.as_sec1_bytes().to_vec(),
                    vec![1],
                    vec![]
                ]
            )
            .is_err()
        );
        assert!(call(&b, h, 8, vec![2u64.to_le_bytes().to_vec()]).is_err());
    }
    fn possession_call(
        b: &OrdinaryBackend,
        h: u64,
        phase: u32,
        mut fields: Vec<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>, Error> {
        fields.insert(0, phase.to_le_bytes().to_vec());
        let q = super::super::kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
        let response = b.invoke(h, Method::PreparedAppEnrollmentPossession, &q)?;
        super::super::kagemusha_core_coordinator_decode_response_v1(&response)
            .map_err(|_| Error::Rejected)
    }
    // Reopen only the actual descriptor-held retail ceremony from called_e20's public native
    // constructors. The original reservation has not moved into a financial owner yet.
    fn reopen_called_retail_original(b: &OrdinaryBackend) -> Vec<Vec<u8>> {
        let mut owner = b.owner.lock().unwrap();
        let held = owner.retail.take().unwrap();
        let pending = owner
            .attempt
            .as_ref()
            .unwrap()
            .retained_pending_identity()
            .unwrap();
        let app = owner.possession.as_ref().unwrap();
        let reserve = owner.reservation.as_ref().unwrap();
        let ticket = held.ticket();
        let preparation = held.preparation_fields(pending, app).unwrap();
        let original = held.recovery_fields(pending, app).unwrap();
        let root = b
            .path
            .join(hex::encode(
                b.source.original_enrollment_id(&b.path).unwrap(),
            ))
            .join("retail-enrollment");
        let wal = root.join("ordinary-retail-enrollment.wal");
        let before = std::fs::read(&wal).unwrap();
        drop(held);
        let recovered = Retail::open_existing(&root, pending, app, reserve).unwrap();
        assert_eq!(recovered.ticket(), ticket);
        assert_eq!(
            recovered.preparation_fields(pending, app).unwrap(),
            preparation
        );
        assert_eq!(recovered.recovery_fields(pending, app).unwrap(), original);
        assert_eq!(std::fs::read(&wal).unwrap(), before);
        owner.retail = Some(recovered);
        original
    }
    #[test]
    fn called_e20_joins_existing_c21_raw_original_and_final_identity_without_money() {
        use p256::ecdsa::{Signature as P256Signature, signature::Signer as _};
        let (_temp, b, f) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        // Give this genuine crypto/WAL restart case a protocol-valid signed interval.
        let prep = prepare_with_lifetime(&b, h, &f, 9000);
        let t = prep[0].clone();
        let c = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&prep[1])
            .unwrap()
            .challenge;
        let app = f.selection.issuance.credential.subject;
        let alias = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            app.attested_key_id,
        );
        call(&b, h, 2, vec![t.clone()]).unwrap();
        call(&b, h, 3, vec![t.clone(), alias.as_bytes().to_vec()]).unwrap();
        call(&b, h, 4, vec![t.clone()]).unwrap();
        // Fixture-only opaque raw bytes are governed by a real signed raw admission; they are
        // explicitly not a claim of physical Apple enrollment or deployment qualification.
        let raw = vec![23; 100];
        call(
            &b,
            h,
            5,
            vec![
                t.clone(),
                app.app_public_key.as_sec1_bytes().to_vec(),
                raw.clone(),
                vec![],
            ],
        )
        .unwrap();
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            platform_class: c.platform_class,
            security_level: app.security_level,
            app_public_key: app.app_public_key,
            attested_key_id: app.attested_key_id,
            raw_platform_evidence_digest: Sha256::digest(&raw).into(),
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let signed = KagemushaRawAppAttestationAdmissionV1 {
            subject,
            signature: Signature::try_new(
                issuer.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        }
        .to_transport_bytes()
        .unwrap();
        let pending = call(&b, h, 6, vec![t, signed]).unwrap();
        let enrollment_root = b.path.join(hex::encode(c.enrollment_id));
        let c21_path = enrollment_root.join("ordinary-app-enrollment.wal");
        let c21_original = std::fs::read(&c21_path).unwrap();
        // A stable enrollment ID cannot substitute for the sole SHA(full C) E selector.
        assert!(possession_call(&b, h, 1, vec![c.enrollment_id.to_vec()]).is_err());
        let fields =
            possession_call(&b, h, 1, vec![c.attestation_challenge().unwrap().to_vec()]).unwrap();
        assert_eq!(fields.len(), 14);
        assert_eq!(std::fs::read(&c21_path).unwrap(), c21_original);
        assert!(
            enrollment_root
                .join("possession/ordinary-app-possession.wal")
                .is_file()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(enrollment_root.join("possession"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        assert!(fields[8].is_empty());
        assert_eq!(fields[9], pending[0]);
        assert_eq!(fields[12], subject.app_signing_identity_digest);
        let et = fields[0].clone();
        assert_eq!(
            possession_call(&b, h, 6, vec![et.clone()]).unwrap()[0],
            pending[0]
        );
        possession_call(&b, h, 2, vec![et.clone()]).unwrap();
        assert!(possession_call(&b, h, 5, vec![et.clone()]).is_err());
        let mut auth = f.app_authority.app_signing_identity_digest.to_vec();
        auth.push(0x40);
        auth.extend_from_slice(&11u32.to_be_bytes());
        let mut n = Sha256::new();
        n.update(&auth);
        n.update(Sha256::digest(&fields[1]));
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let sig: P256Signature = key.sign(&n.finalize());
        let der = sig.to_der();
        let mut assertion = vec![0xa2, 0x71];
        assertion.extend_from_slice(b"authenticatorData");
        assertion.extend_from_slice(&[0x58, 37]);
        assertion.extend_from_slice(&auth);
        assertion.push(0x69);
        assertion.extend_from_slice(b"signature");
        assertion.extend_from_slice(&[0x58, der.as_bytes().len() as u8]);
        assertion.extend_from_slice(der.as_bytes());
        possession_call(&b, h, 3, vec![et.clone(), assertion.clone()]).unwrap();
        let receipt = possession_call(&b, h, 4, vec![et.clone()]).unwrap();
        assert_eq!(receipt[0].len(), 184);
        assert_eq!(&receipt[0][147..179], pending[0]);
        let mut cred = app;
        cred.client_nonce = c.client_nonce;
        cred.server_nonce = c.server_nonce;
        cred.financial_authority_commitment = c.financial_authority_commitment;
        cred.enrollment_challenge_digest = c.attestation_challenge().unwrap();
        cred.platform_evidence_digest =
            kagemusha_ordinary_app_enrollment_evidence_digest_v1(&raw, &assertion).unwrap();
        cred.issued_at_ms = b.source.selected.trusted_time_ms().unwrap();
        cred.expires_at_ms = cred.issued_at_ms + 9000;
        let signature = Signature::try_new(
            issuer.private_key(),
            &cred.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        let circuit_admission =
            iroha_data_model::testing::ordinary_app_enrollment::ordinary_test_issuer_admission_v1(
                KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(&cred, &signature)
                    .unwrap(),
            );
        let original = norito::encode_canonical(&KagemushaOrdinaryAppCredentialV1 {
            subject: cred,
            signature,
            circuit_admission,
        })
        .unwrap();
        let identity = possession_call(&b, h, 8, vec![et.clone(), original.clone()]).unwrap();
        assert_eq!(identity.len(), 2);
        assert_eq!(identity[1], pending[0]);
        assert_eq!(
            possession_call(&b, h, 8, vec![et.clone(), original.clone()]).unwrap(),
            identity
        );
        // Native FI admission reuses the same C, key, consumed platform original and financial
        // witness; wallet/issuer signatures are real known-public synthetic signatures.
        let signed_c =
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&prep[1])
                .unwrap();
        let credential: KagemushaOrdinaryAppCredentialV1 =
            norito::decode_from_bytes(&original).unwrap();
        let core = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
        let core_point = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            core.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let selection = KagemushaOrdinaryRetailEnrollmentSelectionV1 {
            owner: f.selection.owner.clone(),
            issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1 {
                release_id: f.release.release_id(),
                hardware_policy_digest: f.release.hardware_policy_digest(),
                core_authorization_key_reference: kagemusha_core_authorization_key_reference_v1(
                    &core_point,
                ),
                credential,
            },
            preparation: signed_c,
        };
        let now = b.source.selected.trusted_time_ms().unwrap();
        let mut challenge = f.challenge.clone();
        challenge.issuance = selection.issuance.clone();
        challenge.preparation = selection.preparation.clone();
        challenge.issued_at_ms = now;
        challenge.expires_at_ms = c.expires_at_ms;
        let challenge_original = challenge.canonical_bytes().unwrap();
        let message = challenge.account_signing_message().unwrap();
        let actual_before_retail = b.source.selected.trusted_time_ms().unwrap();
        assert!(
            actual_before_retail < c.expires_at_ms,
            "fixture C expired before retail preparation: now={actual_before_retail}, expires={}",
            c.expires_at_ms
        );
        let prepared_fi = possession_call(
            &b,
            h,
            9,
            vec![et, challenge_original.clone(), message.to_vec()],
        )
        .unwrap();
        assert_eq!(std::fs::read(&c21_path).unwrap(), c21_original);
        assert!(
            enrollment_root
                .join("possession/ordinary-app-possession.wal")
                .is_file()
        );
        assert!(enrollment_root.join("retail-enrollment").is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(enrollment_root.join("retail-enrollment"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        assert_eq!(prepared_fi[1], challenge_original);
        assert_eq!(prepared_fi[2], message);
        let mut wrong = message;
        wrong[0] ^= 1;
        assert!(
            possession_call(
                &b,
                h,
                9,
                vec![
                    fields[0].clone(),
                    challenge_original.clone(),
                    wrong.to_vec()
                ]
            )
            .is_err()
        );
        // Recreate the native owner with its explicit recovery policy. Both child journals
        // must reopen their original tickets without changing any retained enrollment WAL.
        let retained_wals = [
            b.path
                .join(format!("{}-preparation", hex::encode(c.enrollment_id)))
                .join("ordinary-preparation.norito.wal"),
            c21_path.clone(),
            enrollment_root.join("possession/ordinary-app-possession.wal"),
            enrollment_root.join("retail-enrollment/ordinary-retail-enrollment.wal"),
        ];
        let retained_before = retained_wals
            .each_ref()
            .map(|path| std::fs::read(path).unwrap());
        b.close(h).unwrap();
        assert!(
            possession_call(&b, h, 1, vec![c.attestation_challenge().unwrap().to_vec()]).is_err()
        );
        let recovered_source = Arc::new(
            KagemushaNativeOrdinaryAppIdentitySourceV1::from_native_selected_originals(
                b.path.clone(),
                b.source.selected.clone(),
                KagemushaOrdinaryEnrollmentDispositionV1::Recover,
                KagemushaOrdinaryEnrollmentDispositionV1::Recover,
                b.source.integrity_policy_original.clone(),
            )
            .unwrap(),
        );
        let b = OrdinaryBackend {
            path: b.path.clone(),
            source: recovered_source,
            owner: Mutex::new(Owner::default()),
        };
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        assert_eq!(
            possession_call(&b, h, 1, vec![c.attestation_challenge().unwrap().to_vec()]).unwrap(),
            fields
        );
        assert_eq!(
            possession_call(&b, h, 8, vec![fields[0].clone(), original.clone()]).unwrap(),
            identity
        );
        assert_eq!(
            possession_call(
                &b,
                h,
                9,
                vec![
                    fields[0].clone(),
                    challenge_original.clone(),
                    message.to_vec()
                ],
            )
            .unwrap(),
            prepared_fi
        );
        for (path, original_wal) in retained_wals.iter().zip(&retained_before) {
            assert_eq!(std::fs::read(path).unwrap(), *original_wal);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                assert_eq!(
                    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
                assert_eq!(
                    std::fs::metadata(path.parent().unwrap())
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o700
                );
            }
        }
        let ft = prepared_fi[0].clone();
        assert_eq!(
            reopen_called_retail_original(&b),
            vec![vec![0], vec![], vec![]]
        );
        let actual_before_wallet = b.source.selected.trusted_time_ms().unwrap();
        assert!(
            actual_before_wallet < c.expires_at_ms,
            "fixture C expired before wallet fence: now={actual_before_wallet}, expires={}",
            c.expires_at_ms
        );
        assert_eq!(
            possession_call(&b, h, 10, vec![ft.clone()]).unwrap(),
            vec![vec![1], vec![]]
        );
        assert_eq!(
            reopen_called_retail_original(&b),
            vec![vec![1], vec![], vec![]]
        );
        assert!(possession_call(&b, h, 10, vec![ft.clone()]).is_err());
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let wallet_signature = Signature::try_new(wallet.private_key(), &message).unwrap();
        let wallet_raw = wallet_signature.payload().to_vec();
        assert_eq!(
            possession_call(&b, h, 11, vec![ft.clone(), wallet_raw.clone()]).unwrap(),
            vec![Sha256::digest(&wallet_raw).to_vec()]
        );
        assert_eq!(
            reopen_called_retail_original(&b),
            vec![vec![2], wallet_raw.clone(), vec![]]
        );
        let now = b.source.selected.trusted_time_ms().unwrap();
        let app = selection
            .issuance
            .credential
            .authenticate(
                &f.release,
                &f.trust,
                &f.app_authority,
                &c,
                &cred.app_public_key,
                now,
            )
            .unwrap();
        let proof = KagemushaOrdinaryRetailEnrollmentPossessionProofV1 {
            challenge: challenge.clone(),
            account_signature: iroha_crypto::SignatureOf::from_signature(wallet_signature),
            raw_attestation: raw,
            app_possession: KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                raw_assertion: assertion,
            },
        };
        let possession = proof
            .authenticate(
                &challenge,
                &selection,
                &f.issuer_policy,
                &f.release,
                &app,
                Some(0),
                now,
            )
            .unwrap();
        let mut certificate = f.certificate.clone();
        certificate.subject.issuance = selection.issuance;
        certificate.subject.challenge_evidence_digest = possession.evidence_digest();
        certificate.subject.ordinary_app_credential_digest = app.digest();
        certificate.subject.issued_at_ms = now;
        certificate.subject.expires_at_ms = cred.expires_at_ms;
        certificate.signature = iroha_crypto::SignatureOf::try_new(
            issuer.private_key(),
            &certificate.subject.approval_payload().unwrap(),
        )
        .unwrap();
        let fi_original = certificate.canonical_bytes().unwrap();
        let mut foreign = certificate.clone();
        foreign.subject.ordinary_app_credential_digest[0] ^= 1;
        assert!(
            possession_call(
                &b,
                h,
                12,
                vec![ft.clone(), foreign.canonical_bytes().unwrap()]
            )
            .is_err()
        );
        // Simulate loss of the response after actual FI admission but before the Bridge moves
        // the reservation into financial custody. Recovery uses the same actual holders and
        // full signed originals; it installs no fabricated Native artifact or monetary grant.
        {
            let mut owner = b.owner.lock().unwrap();
            let Owner {
                attempt,
                possession,
                retail,
                ..
            } = &mut *owner;
            let pending = attempt
                .as_ref()
                .unwrap()
                .retained_pending_identity()
                .unwrap();
            let app = possession.as_ref().unwrap();
            retail
                .as_mut()
                .unwrap()
                .accept_certificate(pending, app, &fi_original)
                .unwrap();
        }
        assert_eq!(
            reopen_called_retail_original(&b),
            vec![vec![3], wallet_raw.clone(), fi_original.clone()]
        );
        let accepted = possession_call(&b, h, 12, vec![ft.clone(), fi_original.clone()]).unwrap();
        assert_eq!(
            accepted,
            vec![
                certificate.subject.enrollment_id.to_vec(),
                identity[1].clone()
            ]
        );
        assert_eq!(
            possession_call(&b, h, 12, vec![ft.clone(), fi_original.clone()]).unwrap(),
            accepted
        );
        assert_eq!(
            possession_call(&b, h, 13, vec![ft.clone()]).unwrap(),
            vec![vec![3], wallet_raw, fi_original]
        );
        assert!(possession_call(&b, h, 14, vec![ft]).is_err());
        assert!(b.owner.lock().unwrap().financial.is_some());
        assert_eq!(
            b.invoke(h, Method::PreparedAppOperationApproval, &[]),
            Err(Error::Rejected)
        );
        let request = super::super::kagemusha_core_coordinator_encode_request_v1(&[
            1u32.to_le_bytes().to_vec(),
            [17; 32].to_vec(),
        ])
        .unwrap();
        assert_eq!(
            b.invoke(h, Method::PreparedAppOperationApproval, &request),
            Err(Error::Unavailable)
        );
        let bootstrap_request = super::super::kagemusha_core_coordinator_encode_request_v1(&[
            8u32.to_le_bytes().to_vec(),
            [17; 32].to_vec(),
        ])
        .unwrap();
        assert_eq!(
            b.invoke(h, Method::PreparedAppOperationApproval, &bootstrap_request),
            Err(Error::Unavailable)
        );
        // A well-formed typed entry cannot manufacture missing bootstrap verifier/custody.
        assert!(b.owner.lock().unwrap().financial.is_some());
        assert!(b.owner.lock().unwrap().bootstrap.is_none());
        assert_eq!(
            b.invoke(h, Method::InitialEnrollment, &[]),
            Err(Error::Unavailable)
        );
    }
    #[test]
    fn ordinary_bootstrap_calls_existing_registration_then_install_for_same_native_source() {
        use std::cell::RefCell;
        let (_temp, b, _f) = backend();
        let steps = RefCell::new(Vec::new());
        bootstrap_registered_source(
            b.source.clone(),
            |source| {
                assert!(Arc::ptr_eq(&source, &b.source));
                source.recheck_originals(&b.path)?;
                steps.borrow_mut().push("register");
                Ok(())
            },
            |path| {
                assert_eq!(path, b.path.to_str().unwrap());
                assert_eq!(*steps.borrow(), vec!["register"]);
                steps.borrow_mut().push("install");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*steps.borrow(), vec!["register", "install"]);
        let called = RefCell::new(false);
        assert!(
            bootstrap_registered_source(
                b.source.clone(),
                |_| Err(Error::Rejected),
                |_| {
                    *called.borrow_mut() = true;
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!*called.borrow());
        // Only called ordering/current native custody is tested; closures do not manufacture
        // a globally installed backend, root account authority or physical platform qualification.
    }
}
