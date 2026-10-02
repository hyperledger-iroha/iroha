//! Shared installed-runtime Native startup with real account custody and session revocation.
//! The application frame supplies only raw original current-cut evidence and lifecycle selectors.
use super::ordinary_android_installed_context::AndroidOrdinaryInstalledContextOwnerV1 as InstalledContext;
use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error,
    kagemusha_core_coordinator_validate_storage_path_v1,
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
    KagemushaNativeEnrollmentRequestContextV1,
    KagemushaNativeInstalledRuntimeAuthorityV1 as Authority,
    KagemushaOrdinaryNativeArtifactResolverV1 as ArtifactResolver,
};
use iroha::participant_enrollment_dispatch::RetainedParticipantEnrollmentRequestV1;
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
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
    time::Duration,
};

#[path = "ordinary_android_existing_account.rs"]
mod android_existing_account;

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
    selection_identity: StableAccountSelectionIdentity<AccountId>,
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
    installed_context: Option<Arc<InstalledContext>>,
    inventory: Arc<Inventory>,
    clock: Arc<Mutex<Clock>>,
    owner: Arc<Mutex<Owner>>,
    registry: SessionRegistry<AccountId, Owner, Pending>,
    active: Mutex<Option<u64>>,
    initial_acquisition: Mutex<InitialAcquisition>,
    retirement: Arc<StartupRetirement>,
    account_publication: ProtectedAccountPublication,
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
    /// Authenticate and register the sole installed Native account/runtime producer.
    /// Registration is part of construction: a second or uncertain construction is refused
    /// before another clock journal or released-key load can be attempted.
    /// Authenticate the installed public package, load its real released
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
        let original = claim_native_composition_original()?;
        Self::from_installed_runtime_and_native_account_inner(
            original,
            None,
            false,
            authority,
            package_path,
            package_sha256,
            original_root,
            storage,
            account,
            clock_disposition,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            profile,
            capacity,
            integrity_leases,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn from_installed_runtime_and_native_account_inner(
        original: NativeCompositionOriginal,
        installed_context: Option<Arc<InstalledContext>>,
        protected_account_pending: bool,
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
        original.install(
            |retirement| {
                retirement.require_original(0)?;
                kagemusha_core_coordinator_validate_storage_path_v1(
                    storage.to_str().ok_or(Error::Rejected)?.as_bytes(),
                )
                .map_err(|_| Error::Rejected)?;
                if let Some(context) = installed_context.as_ref() {
                    context.recheck()?;
                }
                retirement.require_original(0)?;
                let inventory = Arc::new(
                    Inventory::intake(authority, package_path, package_sha256, original_root)
                        .map_err(|_| Error::Rejected)?,
                );
                retirement.require_original(0)?;
                let verifier = inventory
                    .load_recursive_verifier(profile.clone())
                    .map_err(|_| Error::Rejected)?;
                retirement.require_original(0)?;
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
                retirement.require_original(0)?;
                if let Some(context) = installed_context.as_ref() {
                    context.recheck()?;
                }
                retirement.require_original(0)?;
                let clock = Arc::new(Mutex::new(
                    match clock_disposition {
                        Disposition::Fresh => Clock::create(&storage, selected),
                        Disposition::Recover => Clock::open_existing(&storage, selected),
                    }
                    .map_err(|_| Error::Rejected)?,
                ));
                retirement.require_original(0)?;
                inventory
                    .clock_transport(&account, &clock)
                    .map_err(|_| Error::Rejected)?;
                if let Some(context) = installed_context.as_ref() {
                    context.recheck()?;
                }
                retirement.require_original(0)?;
                Ok(Arc::new(Self {
                    installed_context,
                    inventory,
                    clock,
                    owner: Arc::new(Mutex::new(Owner {
                        account,
                        custody: None,
                        source_installed: false,
                        pending_source: None,
                        installation_failed: false,
                        selection_identity: StableAccountSelectionIdentity::new(),
                    })),
                    registry: SessionRegistry::new(),
                    active: Mutex::new(None),
                    initial_acquisition: Mutex::new(InitialAcquisition::new()),
                    retirement,
                    account_publication: ProtectedAccountPublication::new(
                        !protected_account_pending,
                    ),
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
            },
            |startup| STARTUP.set(startup).map_err(|_| Error::Rejected),
        )
    }
    /// Construct the existing actual Native startup from the closed Android package/original
    /// owner and a genuine immutable Native AccountClient. The context supplies
    /// authority/packet/profile originals; it cannot supply current S/W, FI or proof results.
    /// This Rust producer path is not exposed through a caller-controlled C/JNI frame.
    /// # Errors
    /// Rejects changed context originals and every existing registration/clock/account/proof gate.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_admitted_context_and_native_account(
        context: Arc<InstalledContext>,
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
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ) -> Result<Arc<Self>, Error> {
        let original = claim_native_composition_original()?;
        Self::from_admitted_context_and_native_account_for_original(
            original,
            context,
            storage,
            account,
            clock_disposition,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            capacity,
            integrity_leases,
        )
    }

    /// Finish the sole earlier claimed Native composition with the same installed context.
    /// The fixed existing-record reader must claim before its first read and pass that original;
    /// no account material, released authority or recovery choice is constructed by this permit.
    /// # Errors
    /// Refuses retirement, uncertainty, changed originals and all existing constructor gates.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_admitted_context_and_native_account_for_original(
        original: NativeCompositionOriginal,
        context: Arc<InstalledContext>,
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
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ) -> Result<Arc<Self>, Error> {
        Self::from_admitted_context_and_native_account_with_publication(
            original,
            context,
            storage,
            account,
            clock_disposition,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            capacity,
            integrity_leases,
            false,
        )
    }
    #[cfg(any(target_os = "android", all(test, unix)))]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_protected_android_storage_and_native_account(
        original: NativeCompositionOriginal,
        context: Arc<InstalledContext>,
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
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ) -> Result<Arc<Self>, Error> {
        Self::from_admitted_context_and_native_account_with_publication(
            original,
            context,
            storage,
            account,
            clock_disposition,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            capacity,
            integrity_leases,
            true,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn from_admitted_context_and_native_account_with_publication(
        original: NativeCompositionOriginal,
        context: Arc<InstalledContext>,
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
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        protected_account_pending: bool,
    ) -> Result<Arc<Self>, Error> {
        original.require_current()?;
        context.recheck().map_err(|_| Error::Rejected)?;
        Self::from_installed_runtime_and_native_account_inner(
            original,
            Some(context.clone()),
            protected_account_pending,
            context.runtime_authority().map_err(|_| Error::Rejected)?,
            context.inventory_path(),
            context.inventory_sha256(),
            context.public_original_root(),
            storage,
            account,
            clock_disposition,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            context.recursive_profile().map_err(|_| Error::Rejected)?,
            capacity,
            integrity_leases,
        )
    }

    /// Fixed Android producer: retain and repeatedly measure the actual Application/APK/JNI,
    /// admit the source-approved ordinary context/inventory/profile, then reuse the sole existing
    /// startup registration with an actual Native-owned AccountClient. No Root, public S/W,
    /// profile, measurement or raw private key can be supplied through a JNI frame.
    /// This private consumer is for the fixed existing-S custody reader; it does not construct
    /// account custody from a public projection and is not yet an available shipping JNI route.
    /// # Errors
    /// Refuses absent released inputs, package/source drift and all original startup gates.
    #[cfg(target_os = "android")]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_android_application_and_native_account<'local>(
        env: &mut jni::JNIEnv<'local>,
        application: &jni::objects::JObject<'local>,
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
        capacity: KagemushaDurableCapacityV1,
        integrity_leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ) -> Result<Arc<Self>, Error> {
        let original = claim_native_composition_original()?;
        original.require_current()?;
        let context = InstalledContext::from_application(env, application)?;
        original.require_current()?;
        Self::from_admitted_context_and_native_account_for_original(
            original,
            context,
            storage,
            account,
            clock_disposition,
            preparation,
            platform,
            bootstrap,
            cash,
            cash_integrity_leases,
            cash_receivers,
            capacity,
            integrity_leases,
        )
    }

    fn acquire_before_install(self: &Arc<Self>, path: &str) -> Result<(), Error> {
        self.account_publication
            .require_operational(&self.retirement)?;
        // Capture the original Native retirement generation before even package/path rechecks.
        // Logout before registry.begin exists must still retire this original acquisition.
        let generation = self.retirement.capture_original()?;
        let requested = Path::new(path);
        if requested != self.storage {
            return Err(Error::Rejected);
        }
        #[cfg(unix)]
        if requested.canonicalize().map_err(|_| Error::Rejected)? != self.storage {
            return Err(Error::Rejected);
        }
        if let Some(context) = self.installed_context.as_ref() {
            context.recheck().map_err(|_| Error::Rejected)?;
        }
        self.inventory.recheck().map_err(|_| Error::Rejected)?;
        let mut acquisition = self
            .initial_acquisition
            .lock()
            .map_err(|_| Error::Rejected)?;
        acquisition.acquire_original(
            &self.retirement,
            generation,
            || self.require_current(),
            || {
                // The same actual clock/AccountClient owns both phases; no managed CURRENT
                // status or already-open coordinator is an input to initial acquisition.
                let (id, _) = self.begin()?;
                self.finish(id, None)?;
                self.require_current()
            },
        )
    }
    fn begin(&self) -> Result<(u64, Vec<Vec<u8>>), Error> {
        self.account_publication
            .require_operational(&self.retirement)?;
        let generation = self.retirement.capture_original()?;
        if let Some(context) = self.installed_context.as_ref() {
            context.recheck().map_err(|_| Error::Rejected)?;
        }
        let deadline =
            NativeDeadlineV1::start(Duration::from_secs(10)).map_err(|_| Error::Rejected)?;
        let mut fields = None;
        let id = self
            .registry
            .begin(deadline, |permit| {
                let owner = self.owner.lock().map_err(|_| RegistryError::Poisoned)?;
                self.retirement
                    .require_original(generation)
                    .map_err(|_| RegistryError::Rejected)?;
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
                self.retirement
                    .require_original(generation)
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
                self.retirement
                    .require_original(generation)
                    .map_err(|_| RegistryError::Rejected)?;
                permit.require_current()?;
                Ok((
                    owner.account.authority().clone(),
                    self.owner.clone(),
                    Pending { read },
                ))
            })
            .map_err(|_| Error::Rejected)?;
        self.retirement.require_original(generation)?;
        Ok((id, fields.ok_or(Error::Rejected)?))
    }
    fn finish(self: &Arc<Self>, id: u64, original: Option<&[u8]>) -> Result<u64, Error> {
        self.account_publication
            .require_operational(&self.retirement)?;
        let generation = self.retirement.capture_original()?;
        if let Some(context) = self.installed_context.as_ref() {
            context.recheck().map_err(|_| Error::Rejected)?;
        }
        let completion = self
            .registry
            .take_completion(id)
            .map_err(|_| Error::Rejected)?;
        let handle = self
            .registry
            .finish(
                completion,
                |pending| {
                    self.retirement
                        .require_original(generation)
                        .map_err(|_| RegistryError::Rejected)?;
                    match original {
                        Some(original) => pending.read.authenticate(original),
                        None => pending.read.fetch_and_authenticate(),
                    }
                    .map_err(|_| RegistryError::Rejected)
                },
                |owner, current: VerifiedEnrollmentWalletSignatoryV1| {
                    self.retirement
                        .require_original(generation)
                        .map_err(|_| RegistryError::Rejected)?;
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
                            .map(|mut source| {
                                // Only the measured Android producer chooses exact local WAL
                                // recovery. Offered frames cannot select Fresh or erase history.
                                if self.installed_context.is_some() {
                                    source.enable_native_local_recovery();
                                }
                                source
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
                    owner
                        .selection_identity
                        .require_same_accounts(generation, custody.wallet(), custody.signatory())
                        .map_err(|_| RegistryError::Rejected)?;
                    self.retirement
                        .require_original(generation)
                        .map_err(|_| RegistryError::Rejected)?;
                    Ok(Prepared { custody, source })
                },
                |owner, prepared| {
                    self.retirement
                        .require_original(generation)
                        .map_err(|_| RegistryError::Rejected)?;
                    owner.custody = Some(prepared.custody);
                    owner.source_installed |= prepared.source.is_some();
                    // Pure session publication only: actual installer runs below, outside registry locks.
                    owner.pending_source = prepared.source;
                    Ok(())
                },
            )
            .map_err(|_| Error::Rejected)?;
        self.retirement.require_original(generation)?;
        *self.active.lock().map_err(|_| Error::Rejected)? = Some(handle);
        let invocation = self
            .registry
            .invocation(handle)
            .map_err(|_| Error::Rejected)?;
        let retained = self
            .registry
            .dispatch(invocation, |owner| {
                let custody = owner.custody.as_ref().ok_or(Error::Rejected)?;
                custody.recheck().map_err(|_| Error::Rejected)?;
                owner.selection_identity.complete_read(
                    handle,
                    generation,
                    custody.wallet(),
                    custody.signatory(),
                )?;
                Ok(owner.pending_source.take())
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        let source = retained.value?;
        if let Some(source) = source {
            let source =
                source.with_native_account_session(Arc::new(BoundNativeAccountSessionV1 {
                    startup: self.clone(),
                    retirement_generation: generation,
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
        self.account_publication
            .require_operational(&self.retirement)?;
        let generation = self.retirement.capture_original()?;
        self.registry
            .require_current_handle(self.handle()?)
            .map_err(|_| Error::Rejected)?;
        if let Some(context) = self.installed_context.as_ref() {
            context.recheck().map_err(|_| Error::Rejected)?;
        }
        self.inventory.recheck().map_err(|_| Error::Rejected)?;
        self.retirement.require_original(generation)?;
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
        // An uncertain initial acquisition freezes every authority-bearing phase. Canonical
        // cancellation/close/logout must still retire the held selection; cleanup never
        // resets the uncertainty fence or creates another source/account owner.
        self.account_publication
            .require_phase(&self.retirement, request.phase)?;
        require_startup_phase(&self.initial_acquisition, request.phase)?;
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
                // Retire first, without the initializer's mutex. Even a preparation not yet
                // registered cannot turn a post-logout registry permit into original authority.
                let retirement = self.retirement.retire();
                self.registry
                    .revoke_selection()
                    .map_err(|_| Error::Rejected)?;
                *self.active.lock().map_err(|_| Error::Rejected)? = None;
                retirement?;
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
    retirement_generation: u64,
}
impl BoundNativeAccountSessionV1 {
    pub(super) fn recheck(&self) -> Result<(), Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
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
    // This gate checks the held installed identity and selected actual account registry,
    // deliberately without treating an expired S/W observation as a renewal authority.
    pub(super) fn recheck_installed_account_for_refresh(&self) -> Result<(), Error> {
        self.startup.require_current()
    }
    // Pure proof/recovery retains the original selected registry and installed root.
    // The finite S/W read is required again by every signer/effect after real renewal.
    // Do not turn historical approval capture into a current account grant.
    pub(super) fn recheck_retained_account_identity(&self) -> Result<(), Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
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
                    .recheck_retained_identity()
                    .map_err(|_| Error::Rejected)?;
                self.startup
                    .retirement
                    .require_original(self.retirement_generation)?;
                self.startup.require_current()
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    // This returns only routing DATA from the independently signed, still-held inventory.
    // It neither renews S/W nor admits an FI token/session or financial operation.
    pub(super) fn fi_http_endpoint_originals(&self) -> Result<(String, String), Error> {
        self.recheck_retained_account_identity()?;
        let originals = self
            .startup
            .inventory
            .fi_http_endpoint_originals()
            .map_err(|_| Error::Rejected)?;
        self.recheck_retained_account_identity()?;
        Ok(originals)
    }
    pub(super) fn refresh_incoming_clock(&self) -> Result<(), Error> {
        self.startup.require_current()?;
        // The real startup renews both the four-node clock and the same immutable S/W read,
        // then installs fresh custody in that same registry owner. No offered current DTO,
        // callback, account replacement or widened expiry enters this path.
        let (id, _) = self.startup.begin()?;
        self.startup.finish(id, None)?;
        self.recheck()
    }
    // Only the actual Main-created private loan crosses to the retained AccountClient.
    // Registry dispatch serializes the original owner, but never holds the registry mutex
    // while this closure rechecks the selected handle or performs the actual network operation.
    pub(super) fn sign_mint_consent(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintAccountSigningV1<'_>,
    ) -> Result<[u8; 64], Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
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
                let value = owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .sign_retained_mint_consent(&self.startup.inventory, original)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(value)
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    // Only the actual Main-created private loan crosses to the retained AccountClient.
    // Registry dispatch serializes the original owner, but never holds the registry mutex
    // while this closure rechecks the selected handle or performs the actual network operation.
    pub(super) fn sign_mint_transaction(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintTransactionSigningV1<'_>,
    ) -> Result<[Vec<u8>; 2], Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
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
                let value = owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .sign_retained_mint_transaction(&self.startup.inventory, original)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(value)
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    // Only the actual Main-created private loan crosses to the retained AccountClient.
    // Registry dispatch serializes the original owner, but never holds the registry mutex
    // while this closure rechecks the selected handle or performs the actual network operation.
    pub(super) fn submit_mint_transaction(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'_>,
    ) -> Result<(), Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
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
                let value = owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .submit_retained_mint_transaction(&self.startup.inventory, original)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(value)
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        retained.value
    }
    // Only the actual Main-created private loan crosses to the retained AccountClient.
    // Registry dispatch serializes the original owner, but never holds the registry mutex
    // while this closure rechecks the selected handle or performs the actual network operation.
    pub(super) fn read_mint_finality(
        &self,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'_>,
    ) -> Result<Option<Vec<u8>>, Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
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
                let value = owner
                    .custody
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .read_retained_mint_finality(&self.startup.inventory, original)
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                Ok(value)
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
    /// Data-only exact current W/S from the same installed Native account session.
    /// No nonce, clock refresh, key, preparation, signature or financial grant is created.
    /// # Errors
    /// Refuses an absent, closed, replaced or expired original Native account session/custody.
    pub(super) fn current_account_selection_originals(&self) -> Result<Vec<Vec<u8>>, Error> {
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
        let handle = self.startup.handle()?;
        let invocation = self
            .startup
            .registry
            .invocation(handle)
            .map_err(|_| Error::Rejected)?;
        let retained = self
            .startup
            .registry
            .dispatch(invocation, |owner| {
                if self.startup.handle()? != handle {
                    return Err(Error::Rejected);
                }
                self.startup.require_current()?;
                let custody = owner.custody.as_ref().ok_or(Error::Unavailable)?;
                custody
                    .recheck_retained_identity()
                    .map_err(|_| Error::Rejected)?;
                let wallet = custody
                    .wallet()
                    .canonical_i105()
                    .map_err(|_| Error::Rejected)?;
                let signatory = custody
                    .signatory()
                    .canonical_i105()
                    .map_err(|_| Error::Rejected)?;
                custody
                    .recheck_retained_identity()
                    .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                if self.startup.handle()? != handle {
                    return Err(Error::Rejected);
                }
                let selection = owner.selection_identity.original(
                    self.retirement_generation,
                    custody.wallet(),
                    custody.signatory(),
                )?;
                self.startup
                    .retirement
                    .require_original(self.retirement_generation)?;
                Ok(vec![
                    selection.to_le_bytes().to_vec(),
                    wallet.into_bytes(),
                    signatory.into_bytes(),
                ])
            })
            .map_err(|_| Error::Rejected)?;
        if !retained.session_is_current {
            return Err(Error::Rejected);
        }
        self.startup.require_current()?;
        if self.startup.handle()? != handle {
            return Err(Error::Rejected);
        }
        self.startup
            .retirement
            .require_original(self.retirement_generation)?;
        retained.value
    }

    /// Prepare, fetch and sign the exact enrollment request through this current Native session.
    /// The body is retained together with the actual verified request after the prepared holder
    /// is consumed. FI customer admission and protected issuer dispatch remain separate owners.
    /// No managed frame, signature callback or raw key can construct this bound session.
    /// # Errors
    /// Refuses stale/revoked account selection, expired request/current read, changed installed
    /// inventory/clock/prefix, failed actual Native signing or altered retained original bytes.
    pub(super) fn sign_enrollment_request(
        &self,
        context: KagemushaNativeEnrollmentRequestContextV1,
    ) -> Result<
        (
            iroha_crypto::Signature,
            RetainedParticipantEnrollmentRequestV1,
        ),
        Error,
    > {
        self.startup.require_current()?;
        let invocation = self
            .startup
            .registry
            .invocation(self.startup.handle()?)
            .map_err(|_| Error::Rejected)?;
        let result = self
            .startup
            .registry
            .dispatch(invocation, |owner| {
                // The registry rechecks this exact invocation after the original owner lock.
                self.startup.require_current()?;
                let custody = owner.custody.as_ref().ok_or(Error::Rejected)?;
                let prepared = custody
                    .prepare_enrollment_request(context, self.startup.clock.clone())
                    .map_err(|_| Error::Rejected)?;
                // Preparation already validates the complete original body and its bound.
                // Copy only those exact validated bytes; neither decode nor rebuild JSON.
                let original_body = prepared.request().body.to_vec();
                let (signature, verified) = custody
                    .fetch_and_sign_current_enrollment_request(
                        prepared,
                        self.startup.inventory.clone(),
                    )
                    .map_err(|_| Error::Rejected)?;
                let retained =
                    RetainedParticipantEnrollmentRequestV1::retain(verified, original_body)
                        .map_err(|_| Error::Rejected)?;
                self.startup.require_current()?;
                retained.original_body().map_err(|_| Error::Rejected)?;
                Ok::<_, Error>((signature, retained))
            })
            .map_err(|_| Error::Rejected)?;
        // A close/logout/account switch during actual I/O cannot publish this result.
        if !result.session_is_current {
            return Err(Error::Rejected);
        }
        let (signature, retained) = result.value?;
        self.startup.require_current()?;
        retained.original_body().map_err(|_| Error::Rejected)?;
        self.startup.require_current()?;
        Ok((signature, retained))
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
#[path = "ordinary_native_startup/account_publication.rs"]
mod account_publication;
use account_publication::{ProtectedAccountPublication, StableAccountSelectionIdentity};

static STARTUP: OnceLock<Arc<KagemushaNativeOrdinaryRuntimeStartupV1>> = OnceLock::new();
// These private state owners only order real construction/acquisition. Their scripted unit
// controls establish one-use control flow, never Native custody or financial qualification.
struct StartupRegistration {
    attempted: bool,
    original: Option<Arc<StartupRetirement>>,
}
impl StartupRegistration {
    const fn new() -> Self {
        Self {
            attempted: false,
            original: None,
        }
    }
    fn claim(&mut self) -> Result<NativeCompositionOriginal, Error> {
        if self.attempted {
            return Err(Error::Rejected);
        }
        self.attempted = true;
        let retirement = Arc::new(StartupRetirement::new());
        self.original = Some(Arc::clone(&retirement));
        Ok(NativeCompositionOriginal {
            retirement,
            generation: 0,
            completed: false,
        })
    }
}
static REGISTRATION: Mutex<StartupRegistration> = Mutex::new(StartupRegistration::new());

/// Sole private attempted composition, claimed before the producer performs source or key I/O.
/// Dropping an incomplete original retires it; no retry can claim another account owner.
pub(super) struct NativeCompositionOriginal {
    retirement: Arc<StartupRetirement>,
    generation: u64,
    completed: bool,
}
impl NativeCompositionOriginal {
    pub(super) fn require_current(&self) -> Result<(), Error> {
        self.retirement.require_original(self.generation)
    }
    fn install<T>(
        mut self,
        construct: impl FnOnce(Arc<StartupRetirement>) -> Result<Arc<T>, Error>,
        publish: impl FnOnce(Arc<T>) -> Result<(), Error>,
    ) -> Result<Arc<T>, Error> {
        self.require_current()?;
        // Neither the registration claim lock nor publication gate spans construction I/O.
        let startup = construct(Arc::clone(&self.retirement))?;
        self.require_current()?;
        self.retirement
            .publish_original(self.generation, || publish(Arc::clone(&startup)))?;
        self.require_current()?;
        self.completed = true;
        Ok(startup)
    }
}
impl Drop for NativeCompositionOriginal {
    fn drop(&mut self) {
        if !self.completed {
            // Failure, unwind or a lost publication result cannot restore the original.
            let _ = self.retirement.retire();
        }
    }
}
/// Claim and retain the one process-local original before any constructor or fixed-reader I/O.
/// This permit grants no account, source, release or financial authority.
pub(super) fn claim_native_composition_original() -> Result<NativeCompositionOriginal, Error> {
    REGISTRATION.lock().map_err(|_| Error::Rejected)?.claim()
}
fn retire_registered_composition(registration: &Mutex<StartupRegistration>) -> Result<(), Error> {
    let original = registration
        .lock()
        .map_err(|_| Error::Rejected)?
        .original
        .clone();
    match original {
        Some(original) => original.retire(),
        // Identity-only binding and normal onboarding do not consume a future first account.
        None => Ok(()),
    }
}
/// Deny the same pending or published original without I/O, Core close or registry revocation.
/// Cleanup of exact existing handles still runs separately in its original order.
pub(super) fn retire_pending_native_composition() -> Result<(), Error> {
    retire_registered_composition(&REGISTRATION)
}

// Generation zero is this retained once-only Native original. Retirement is permanent for
// this producer: neither a new registry permit nor an install retry can reopen that original.
// No managed frame supplies, resets or chooses the generation; no gate spans source I/O.
// This is process-local revocation, not a hardware wallet counter, trusted clock or replay proof.
struct StartupRetirement {
    generation: AtomicU64,
    publication: Mutex<bool>,
}
impl StartupRetirement {
    const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            publication: Mutex::new(false),
        }
    }
    fn capture_original(&self) -> Result<u64, Error> {
        let generation = self.generation.load(AtomicOrdering::Acquire);
        self.require_original(generation)?;
        Ok(generation)
    }
    fn require_original(&self, original: u64) -> Result<(), Error> {
        if original == 0 && self.generation.load(AtomicOrdering::Acquire) == original {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
    fn publish_original(
        &self,
        original: u64,
        publish: impl FnOnce() -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut published = self.publication.lock().map_err(|_| Error::Rejected)?;
        self.require_original(original)?;
        if *published {
            return Err(Error::Rejected);
        }
        // Production publication is only STARTUP.set: no I/O or callback crosses this gate.
        *published = true;
        publish()
    }
    fn retire(&self) -> Result<(), Error> {
        let gate = self.publication.lock();
        let poisoned = gate.is_err();
        // Poison is never a reason to leave the original usable. Deny under the retained gate
        // while still reporting the cleanup failure; do not clear poison or reset generation.
        let _gate = gate.unwrap_or_else(std::sync::PoisonError::into_inner);
        let retired = self
            .generation
            .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |current| {
                current.checked_add(1)
            })
            .map(|_| ())
            .map_err(|_| Error::Rejected);
        if poisoned {
            Err(Error::Rejected)
        } else {
            retired
        }
    }
}

struct InitialAcquisition {
    attempted: bool,
    complete: bool,
}
// Cleanup touches only the registry and must not wait behind an active initializer's mutex.
// Its revocation causes that initializer's existing current-owner rechecks to refuse publication.
fn require_startup_phase(acquisition: &Mutex<InitialAcquisition>, phase: u8) -> Result<(), Error> {
    if matches!(phase, 3..=5) {
        return Ok(());
    }
    acquisition
        .try_lock()
        .map_err(|_| Error::Rejected)?
        .require_phase(phase)
}
impl InitialAcquisition {
    const fn new() -> Self {
        Self {
            attempted: false,
            complete: false,
        }
    }
    fn require_callable(&self) -> Result<(), Error> {
        if self.attempted && !self.complete {
            Err(Error::Rejected)
        } else {
            Ok(())
        }
    }
    fn require_phase(&self, phase: u8) -> Result<(), Error> {
        match phase {
            1 | 2 | 6 => self.require_callable(),
            3..=5 => Ok(()),
            _ => Err(Error::Rejected),
        }
    }
    fn acquire_original(
        &mut self,
        retirement: &StartupRetirement,
        generation: u64,
        require_current: impl FnOnce() -> Result<(), Error>,
        original_acquisition: impl FnOnce() -> Result<(), Error>,
    ) -> Result<(), Error> {
        retirement.require_original(generation)?;
        let result = self.acquire(
            || {
                retirement.require_original(generation)?;
                require_current()?;
                retirement.require_original(generation)
            },
            || {
                retirement.require_original(generation)?;
                original_acquisition()?;
                retirement.require_original(generation)
            },
        );
        if retirement.require_original(generation).is_err() {
            // Retirement during final completion remains uncertain, never a callable retry.
            if self.attempted {
                self.complete = false;
            }
            return Err(Error::Rejected);
        }
        result
    }
    fn acquire(
        &mut self,
        require_current: impl FnOnce() -> Result<(), Error>,
        original_acquisition: impl FnOnce() -> Result<(), Error>,
    ) -> Result<(), Error> {
        if self.complete {
            return require_current();
        }
        if self.attempted {
            return Err(Error::Rejected);
        }
        // A transport/signing/fsync/publication error or lost result must not choose another
        // initial read or recreate the original Native enrollment/cash journals.
        self.attempted = true;
        original_acquisition()?;
        self.complete = true;
        Ok(())
    }
}

pub(super) fn has_registered_runtime() -> bool {
    STARTUP.get().is_some()
}
// Invoked by the real native installer before the managed bridge obtains its coordinator.
// Only the storage selector crosses C/JNI; the installed owner supplies every original.
pub(super) fn acquire_registered_runtime_before_install(path: &str) -> Result<(), Error> {
    STARTUP
        .get()
        .ok_or(Error::Unavailable)?
        .acquire_before_install(path)
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

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeStartupV1<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    phase: jni::sys::jint,
    id: jni::sys::jlong,
    original: jni::objects::JByteArray<'local>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<jni::sys::jobjectArray, ()> {
            let phase = u8::try_from(phase).map_err(|_| ())?;
            #[cfg(target_os = "android")]
            let context = if matches!(phase, 3..=5) {
                InstalledContext::require_early_retained_jni_class(&mut env, &_class)
                    .map_err(|_| ())?;
                None
            } else {
                let context = STARTUP
                    .get()
                    .and_then(|startup| startup.installed_context.as_ref())
                    .ok_or(())?;
                // Work retains complete installed-source admission; early cleanup identity
                // can only deny or release an exact existing original, never grant authority.
                context
                    .require_jni_owner(&mut env, &_class)
                    .map_err(|_| ())?;
                Some(context)
            };
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
            #[cfg(target_os = "android")]
            {
                if matches!(phase, 3..=5) {
                    InstalledContext::require_early_retained_jni_class(&mut env, &_class)
                } else {
                    context.ok_or(())?.require_jni_owner(&mut env, &_class)
                }
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

#[cfg(test)]
#[path = "ordinary_native_startup/tests.rs"]
mod tests;
