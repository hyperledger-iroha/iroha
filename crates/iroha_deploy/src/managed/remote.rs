//! Installed-profile attachment binding and the sole native worker's bounded relay loop.

use super::*;
use crate::{
    attachment::RelayProgress,
    bootstrap::{
        AuthenticatedBootstrap, BootstrapError, BootstrapStore, InstalledNetworkProfile,
        discover_native_bootstrap,
    },
    provisioning::{ProvisioningProgress, ProvisioningStage, RemoteProvisioning},
    sora_network::PrivateRootSpec,
};
use iroha_data_model::sns::{DATASPACE_ALIAS_SUFFIX_ID, NameSelectorV1};
#[cfg(test)]
use iroha_fs::OwnerDirectory;
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_model_base::topology::DataSpaceId;
use iroha_wallet::operations::OperationStatus;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const BINDING: &str = "binding.json";
const PROGRESS: &str = "progress.json";
const ACTIVATION: &str = "activation.json";
const MAX_ATTACH: Duration = Duration::from_secs(60);
const RELAY_TURN: Duration = Duration::from_secs(15);

#[derive(Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ProfileBinding {
    name: String,
    torii_root: String,
    chain_id: String,
    account_chain_discriminant: u16,
}
impl ProfileBinding {
    fn from_profile(profile: &InstalledNetworkProfile) -> Self {
        Self {
            name: profile.network_name().into(),
            torii_root: profile.torii_root().to_string(),
            chain_id: profile.chain_id().to_owned(),
            account_chain_discriminant: profile.account_chain_discriminant(),
        }
    }
    fn verify(&self, profile: &InstalledNetworkProfile) -> Result<()> {
        if self != &Self::from_profile(profile) {
            return Err(Error::Invalid(
                "retained attachment differs from the installed parent profile".into(),
            ));
        }
        Ok(())
    }
}

fn advance_profile(
    _directory: &PrivateDirectory,
    binding: &mut Binding,
    profile: &InstalledNetworkProfile,
) -> Result<()> {
    binding.profile.verify(profile)
}

fn bind_installed_profile(
    directory: &PrivateDirectory,
    profile: &InstalledNetworkProfile,
    deadline: Instant,
) -> Result<Binding> {
    let gate = directory.open_existing_lock("request.lock")?;
    loop {
        remaining(deadline)?;
        match gate.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => thread::sleep(POLL.min(remaining(deadline)?)),
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    let mut binding = read_binding(directory)?;
    advance_profile(directory, &mut binding, profile)?;
    Ok(binding)
}
#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Binding {
    profile: ProfileBinding,
    spec: PrivateRootSpec,
    account_alias: String,
    context: Option<ManagedContext>,
}
#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Activation {
    deadline_ms: u64,
}

#[cfg(test)]
fn outer_path(store: &ManagedStore, name: &str) -> PathBuf {
    store.root().join("attachments").join(name)
}
fn bootstrap_path(store: &ManagedStore, name: &str) -> PathBuf {
    store.root().join("parents").join(name)
}
fn existing_outer(store: &ManagedStore, name: &str) -> Result<Option<PrivateDirectory>> {
    validate_name(name)?;
    let Some(attachments) = store.attachments_directory()? else {
        return Ok(None);
    };
    Ok(attachments.open_child_optional(name)?)
}
fn read_binding(directory: &PrivateDirectory) -> Result<Binding> {
    let binding: Binding = decode(&directory.read(BINDING, MAX_METADATA)?)?;
    binding
        .spec
        .validate()
        .map_err(|_| Error::Invalid("invalid retained private identity".into()))?;
    iroha_wallet::namespace::resolve_private_owner_alias(
        &binding.spec.dataspace_alias,
        &binding.account_alias,
    )
    .map_err(|_| Error::Invalid("invalid retained private owner alias".into()))?;
    Ok(binding)
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or(Error::ParentDeadline)
}
fn now_ms() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .ok_or_else(|| Error::Invalid("local attachment clock is invalid".into()))
}
fn connecting(network: String) -> ManagedAttachmentStatus {
    ManagedAttachmentStatus {
        network,
        stage: ManagedAttachmentPhase::Connecting,
        wallet_status: None,
        local_successor: None,
        parent_confirmed: None,
        failure: None,
    }
}
fn attachment_deadline(status: &ManagedAttachmentStatus) -> Error {
    Error::ParentProgressDeadline {
        stage: status.stage,
        failure: status
            .failure
            .unwrap_or(ManagedAttachmentFailure::AwaitingCompletion),
    }
}

fn terminal_operation_error(status: &ManagedAttachmentStatus) -> Option<Error> {
    status
        .failure
        .filter(|failure| failure.is_terminal_operation())
        .map(|failure| {
            Error::Invalid(format!(
                "parent attachment cannot complete during {}: {failure}; inspect `kagami dataspace status` and retain the original journals",
                status.stage,
            ))
        })
}

// A blocking status observation consumes the same foreground budget. Retain terminal
// operation failure precedence, then reject an otherwise complete reply received too late.
fn attachment_complete(status: &ManagedAttachmentStatus, deadline: Instant) -> Result<bool> {
    if let Some(error) = terminal_operation_error(status) {
        return Err(error);
    }
    let complete = status.stage == ManagedAttachmentPhase::Attached
        && status.failure.is_none()
        && status.parent_confirmed.is_some();
    if complete {
        remaining(deadline).map_err(|_| attachment_deadline(status))?;
    }
    Ok(complete)
}

fn record_wallet_status(
    public: &mut ManagedAttachmentStatus,
    status: Option<OperationStatus>,
    completed_turn: bool,
) {
    // Provision-only refreshes have no new transaction observation. They must not
    // erase a terminal result from the immutable registration or anchor journal.
    if status.is_none()
        && public
            .failure
            .is_some_and(ManagedAttachmentFailure::is_terminal_operation)
    {
        return;
    }
    public.wallet_status = status.map(|status| status.as_str().into());
    match status {
        Some(OperationStatus::Expired) => {
            public.failure = Some(ManagedAttachmentFailure::OperationExpired);
        }
        Some(OperationStatus::Rejected) => {
            public.failure = Some(ManagedAttachmentFailure::OperationRejected);
        }
        _ if completed_turn => public.failure = None,
        _ => {}
    }
}

fn record_provisioning_progress(
    public: &mut ManagedAttachmentStatus,
    progress: ProvisioningProgress,
    completed_turn: bool,
) {
    public.stage = match progress.stage {
        ProvisioningStage::Funding => ManagedAttachmentPhase::Funding,
        ProvisioningStage::Namespace => ManagedAttachmentPhase::Namespace,
        ProvisioningStage::Registering => ManagedAttachmentPhase::Registering,
        // A historical registration receipt cannot make the current parent observation ready.
        ProvisioningStage::Attached => ManagedAttachmentPhase::Anchoring,
    };
    record_wallet_status(public, progress.wallet_status, completed_turn);
    if let Some(confirmed) = progress.confirmed {
        public.parent_confirmed = Some(ManagedConfirmedAnchor {
            parent_height: confirmed.parent_height,
            child: confirmed.child,
        });
    }
}

fn record_relay_progress(public: &mut ManagedAttachmentStatus, relay: RelayProgress) {
    record_wallet_status(public, relay.parent.transaction_status, true);
    if let Some(cursor) = relay.local_successor {
        public.local_successor = Some(cursor);
    }
    if let Some(confirmed) = relay.parent.confirmed {
        public.parent_confirmed = Some(ManagedConfirmedAnchor {
            parent_height: confirmed.parent_height,
            child: confirmed.child,
        });
        if !public
            .failure
            .is_some_and(ManagedAttachmentFailure::is_terminal_operation)
        {
            public.stage = ManagedAttachmentPhase::Attached;
        }
    }
}
fn verify_context(binding: &Binding, prepared: &PreparedLocalnet) -> Result<()> {
    if binding.context.as_ref() != Some(&prepared.context)
        || binding.spec.dataspace_id.as_u64() != prepared.context.dataspace_id
        || binding.spec.dataspace_alias != prepared.context.dataspace_alias
    {
        return Err(Error::Invalid(
            "local reset or changed context cannot replace a retained parent attachment".into(),
        ));
    }
    Ok(())
}

fn verify_generation_binding(
    store: &ManagedStore,
    name: &str,
    binding: &Binding,
    prepared: &PreparedLocalnet,
) -> Result<()> {
    verify_context(binding, prepared)?;
    verify_private_generation(store, name, binding, prepared)
}

fn verify_private_generation(
    store: &ManagedStore,
    name: &str,
    binding: &Binding,
    prepared: &PreparedLocalnet,
) -> Result<()> {
    let directory = store.directory(name)?;
    let retained = generation::read(&directory)?;
    if retained.prepared != *prepared
        || retained.root_kind
            != (RootKind::Private {
                spec: binding.spec.clone(),
            })
    {
        return Err(Error::Invalid(
            "attachment differs from the signed private root scope".into(),
        ));
    }
    Ok(())
}

// Capture the exact published generation even when its first startup failed. This does not
// activate parent work or attest readiness; it preserves identity and diagnostic custody.
fn retain_prepared_context(
    store: &ManagedStore,
    name: &str,
    directory: &PrivateDirectory,
    binding: &mut Binding,
    prepared: &PreparedLocalnet,
) -> Result<()> {
    verify_private_generation(store, name, binding, prepared)?;
    if binding.context.is_some() {
        verify_context(binding, prepared)?;
    } else {
        let mut retained = binding.clone();
        retained.context = Some(prepared.context.clone());
        verify_context(&retained, prepared)?;
        directory.write_atomic(BINDING, &encode(&retained)?, PublishMode::Replace)?;
        *binding = retained;
    }
    Ok(())
}

// Publish the foreground's original deadline alongside its exact generation binding.
// A worker may begin its Ready-triggered turn before the caller observes Ready; it
// must already see this budget rather than freeze a background turn's shorter TTL.
fn retain_prepared_activation(
    store: &ManagedStore,
    name: &str,
    directory: &PrivateDirectory,
    binding: &mut Binding,
    prepared: &PreparedLocalnet,
    deadline: Instant,
) -> Result<()> {
    retain_prepared_context(store, name, directory, binding, prepared)?;
    let activation = Activation {
        deadline_ms: now_ms()?
            .checked_add(
                u64::try_from(remaining(deadline)?.as_millis())
                    .map_err(|_| Error::ParentDeadline)?,
            )
            .ok_or(Error::ParentDeadline)?,
    };
    directory.write_atomic(ACTIVATION, &encode(&activation)?, PublishMode::Replace)?;
    Ok(())
}

fn attachment_turn_deadline(
    directory: &PrivateDirectory,
    parent_confirmed: bool,
) -> Result<Instant> {
    match directory.read_optional(ACTIVATION, MAX_METADATA)? {
        Some(bytes) => {
            let activation: Activation = decode(&bytes)?;
            let milliseconds = activation.deadline_ms.saturating_sub(now_ms()?);
            // Registration consumes the original foreground budget. Once this live owner
            // has independently confirmed it, new anchors get their own bounded turn;
            // they must not inherit the last milliseconds of startup's budget.
            // Still decode the retained activation above to preserve custody validation.
            if !parent_confirmed && milliseconds > 0 {
                Ok(Instant::now() + Duration::from_millis(milliseconds).min(MAX_ATTACH))
            } else {
                Ok(Instant::now() + RELAY_TURN)
            }
        }
        None => Ok(Instant::now() + RELAY_TURN),
    }
}

/// Explicit public-parent inputs for an independent cold-build finality owner.
pub(super) struct BuildRegistryContext {
    pub(super) parent: iroha::config::Config,
    pub(super) bootstrap: AuthenticatedBootstrap,
    pub(super) finality_path: PathBuf,
}

pub(super) fn build_registry_context(
    store: &ManagedStore,
    runtime: &InstalledRuntime,
    prepared: &PreparedLocalnet,
    deadline: Instant,
) -> Result<Option<BuildRegistryContext>> {
    let name = &prepared.context.name;
    let Some(directory) = existing_outer(store, name)? else {
        return Ok(None);
    };
    let binding = read_binding(&directory)?;
    remaining(deadline)?;
    let profiles = runtime.network_profiles()?;
    let profile = profiles
        .select(&binding.profile.name)
        .map_err(|error| Error::Invalid(error.to_string()))?;
    let binding = bind_installed_profile(&directory, profile, deadline)?;
    verify_generation_binding(store, name, &binding, prepared)?;
    let bootstrap = authenticate_parent(&bootstrap_path(store, name), profile, deadline, false)?;
    if bootstrap.metadata().network_id != binding.spec.parent_network_id {
        return Err(Error::Invalid(
            "parent reset cannot replace the selected build registry".into(),
        ));
    }
    let Some(registry) = bootstrap.metadata().build_registry.as_ref() else {
        return Ok(None);
    };
    let mut parent = RemoteProvisioning::load_parent_config(
        &directory.path().join("provisioning"),
        &bootstrap,
        prepared,
        &binding.account_alias,
    )
    .map_err(|error| Error::Invalid(error.to_string()))?;
    if !registry
        .torii_roots
        .iter()
        .any(|root| root == parent.torii_api_url.as_str())
    {
        parent.torii_api_url = registry
            .torii_roots
            .first()
            .ok_or_else(|| Error::Invalid("signed build registry has no endpoint".into()))?
            .parse()
            .map_err(|_| Error::Invalid("signed build registry endpoint is invalid".into()))?;
    }
    remaining(deadline)?;
    Ok(Some(BuildRegistryContext {
        parent,
        bootstrap,
        finality_path: directory.path().join("registry_finality"),
    }))
}

fn authenticate_parent(
    path: &Path,
    profile: &InstalledNetworkProfile,
    deadline: Instant,
    refresh: bool,
) -> Result<AuthenticatedBootstrap> {
    remaining(deadline)?;
    let store = loop {
        match BootstrapStore::open(path) {
            Ok(store) => break store,
            Err(BootstrapError::Busy) => thread::sleep(POLL.min(remaining(deadline)?)),
            Err(error) => return Err(Error::Invalid(error.to_string())),
        }
    };
    remaining(deadline)?;
    if !refresh
        && let Some(retained) = store
            .load(profile)
            .map_err(|error| Error::Invalid(error.to_string()))?
    {
        return Ok(retained);
    }
    remaining(deadline)?;
    let native =
        discover_native_bootstrap(profile, deadline).map_err(|source| Error::ParentCheckpoint {
            profile: Box::new(profile.clone()),
            source: Box::new(source.into()),
        })?;
    store
        .accept(profile, native)
        .map_err(|source| Error::ParentCheckpoint {
            profile: Box::new(profile.clone()),
            source: Box::new(source.into()),
        })
}

/// Explicit administrative authorization, separate from config-free SNS attachment.
#[derive(Clone, Debug)]
pub struct ManagedAmxRegistrationOptions {
    /// Exact parent fee asset selected by the administrator.
    pub fee_asset: iroha_data_model::asset::AssetDefinitionId,
    /// Positive aggregate ceiling for this sole retained transaction.
    pub max_fee: iroha_primitives::numeric::Quantity,
    /// Original finite exclusive UTC signing authorization, unchanged on recovery.
    pub deadline_unix_ms: u64,
    /// This turn's positive I/O budget, at most sixty seconds; never renews signing authority.
    pub timeout: Duration,
}

impl ManagedStore {
    /// Register the original private AMX instance with an explicitly selected parent administrator.
    /// This action needs exclusive original provisioning custody (for example after an explicit
    /// stop); it never stops the running supervisor, grants permissions or replaces a generation.
    /// Only independently certified original Network execution establishes a final result.
    ///
    /// # Errors
    /// Rejects missing/incomplete original sources, competing ownership, changed installed parent,
    /// unsafe signing context, fee/time changes, exhausted deadline or invalid native inclusion.
    pub fn register_amx_dataspace(
        &self,
        runtime: &InstalledRuntime,
        name: &str,
        administrator: &iroha::config::Config,
        options: &ManagedAmxRegistrationOptions,
    ) -> Result<crate::attachment::AmxRegistrationProgress> {
        validate_name(name)?;
        if options.timeout.is_zero() || options.timeout > MAX_ATTACH || options.max_fee.is_zero() {
            return Err(Error::Invalid(
                "AMX registration needs a positive fee ceiling and at most sixty seconds".into(),
            ));
        }
        let deadline = Instant::now()
            .checked_add(options.timeout)
            .ok_or_else(|| Error::Invalid("AMX registration I/O deadline overflow".into()))?;
        let directory = existing_outer(self, name)?
            .ok_or_else(|| Error::Invalid("original private attachment is missing".into()))?;
        let gate = directory.open_existing_lock("request.lock")?;
        gate.try_lock().map_err(|_| Error::Busy(name.into()))?;
        let network = self.directory(name)?;
        let _generation = store::acquire(&network, "operation.lock", name)?;
        let _inactive = store::acquire(&network, "runtime.lock", name)?;
        let mut binding = read_binding(&directory)?;
        let profiles = runtime.network_profiles()?;
        let profile = profiles
            .select(&binding.profile.name)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        advance_profile(&directory, &mut binding, profile)?;
        let prepared = self.prepared(name)?;
        verify_generation_binding(self, name, &binding, &prepared)?;
        if directory.open_child_optional("provisioning")?.is_none() {
            return Err(Error::Invalid(
                "original namespace provisioning is missing".into(),
            ));
        }
        let bootstrap = authenticate_parent(&bootstrap_path(self, name), profile, deadline, false)?;
        // Published source capsules are existing-only here: no HTTP repair or new G1/H2 pair.
        let selection = crate::bootstrap::AmxSourceSelection {
            bootstrap: &bootstrap,
            attachment: &directory,
            deadline,
        };
        let sources = selection
            .retain(&binding.spec, false)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        sources
            .require_signed_generation(&prepared)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let mut provisioning = RemoteProvisioning::open(
            &directory.path().join("provisioning"),
            &bootstrap,
            &prepared,
            &binding.account_alias,
        )
        .map_err(|error| Error::Invalid(error.to_string()))?;
        let spending = iroha_wallet::operations::BoundedTransactionOptions {
            fee_payment: iroha_data_model::transaction::FeePaymentIntent::authority(
                Vec::new(),
                None,
            ),
            max_total_fees: std::collections::BTreeMap::from([(
                options.fee_asset.clone(),
                options.max_fee.clone(),
            )]),
            deadline,
        };
        provisioning
            .register_amx_once(
                &bootstrap,
                administrator,
                options.deadline_unix_ms,
                &spending,
            )
            .map_err(|error| Error::Invalid(error.to_string()))
    }
}

impl ManagedStore {
    /// Prepare four private validators and activate their sole outbound attachment owner.
    ///
    /// Uses only an independently installed profile and retained native operation journals.
    /// The complete foreground operation shares at most sixty seconds. Timeout retains exact
    /// work for authenticated status and retry; it never substitutes a new child or signer.
    ///
    /// # Errors
    /// Unknown profiles, changed custody/identity, failed startup or unconfirmed attachment.
    pub fn up_dataspace(
        &self,
        runtime: &InstalledRuntime,
        request: &DataspaceRequest,
    ) -> Result<ManagedDataspaceStatus> {
        validate_name(&request.name)?;
        if request.timeout.is_zero() || request.timeout > MAX_ATTACH {
            return Err(Error::Invalid(
                "dataspace startup budget must be positive and at most sixty seconds".into(),
            ));
        }
        iroha_wallet::namespace::resolve_private_owner_alias(
            &request.alias,
            &request.account_alias,
        )
        .map_err(|_| Error::Invalid("invalid private owner alias".into()))?;
        let deadline = Instant::now() + request.timeout;
        let profiles = runtime.network_profiles()?;
        let profile = profiles
            .select(&request.network)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, &request.alias)
            .map_err(|_| Error::Invalid("invalid dataspace alias".into()))?;
        if selector.normalized_label() != request.alias || request.alias == "universal" {
            return Err(Error::Invalid(
                "dataspace alias must be canonical and private".into(),
            ));
        }
        let mut discovered = None;
        let directory = if let Some(directory) = existing_outer(self, &request.name)? {
            directory
        } else {
            let bootstrap = authenticate_parent(
                &bootstrap_path(self, &request.name),
                profile,
                deadline,
                true,
            )?;
            let spec = PrivateRootSpec {
                parent_network_id: bootstrap.metadata().network_id,
                dataspace_id: DataSpaceId::from_hash(&selector.name_hash()),
                dataspace_alias: request.alias.clone(),
            };
            spec.validate().map_err(|_| {
                Error::Invalid("invalid private scope derived from installed parent".into())
            })?;
            let binding = Binding {
                profile: ProfileBinding::from_profile(profile),
                spec,
                account_alias: request.account_alias.clone(),
                context: None,
            };
            discovered = Some(bootstrap);
            self.publish_attachment(
                &request.name,
                &[("request.lock", b""), (BINDING, &encode(&binding)?)],
            )?
        };
        let gate = directory.open_existing_lock("request.lock")?;
        gate.try_lock()
            .map_err(|_| Error::Busy(request.name.clone()))?;
        let mut binding = read_binding(&directory)?;
        if binding.spec.dataspace_alias != request.alias
            || binding.account_alias != request.account_alias
        {
            return Err(Error::Invalid(
                "context already selects a different dataspace or owner alias".into(),
            ));
        }
        advance_profile(&directory, &mut binding, profile)?;
        // A completed binding permanently remembers the child even if its local generation
        // is reset. Do not generate replacement keys before detecting that mismatch.
        if binding.context.is_some() {
            verify_generation_binding(
                self,
                &request.name,
                &binding,
                &self.prepared(&request.name)?,
            )?;
        }
        let spec = binding.spec.clone();
        // Explicit startup refreshes an old context through compact native certificates.
        // A new context reuses the same discovery already performed above.
        let bootstrap = match discovered {
            Some(bootstrap) => bootstrap,
            None => authenticate_parent(
                &bootstrap_path(self, &request.name),
                profile,
                deadline,
                true,
            )?,
        };
        if bootstrap.metadata().network_id != spec.parent_network_id {
            return Err(Error::Invalid(
                "parent reset cannot replace the selected AMX private root".into(),
            ));
        }
        let sources = crate::bootstrap::AmxSourceSelection {
            bootstrap: &bootstrap,
            attachment: &directory,
            deadline,
        };
        let local = self.up_private_root_with_amx_bound(
            &runtime.private_root_request(
                &request.name,
                remaining(deadline)?.min(Duration::from_secs(30)),
            ),
            &spec,
            &sources,
            |prepared| {
                retain_prepared_activation(
                    self,
                    &request.name,
                    &directory,
                    &mut binding,
                    prepared,
                    deadline,
                )
            },
        )?;
        if local.phase != ManagedPhase::Ready {
            return Err(Error::Invalid(local.failure.unwrap_or_else(|| {
                "private validators did not establish readiness".into()
            })));
        }
        let prepared = self.prepared(&request.name)?;
        verify_generation_binding(self, &request.name, &binding, &prepared)?;
        drop(gate);
        remaining(deadline)?;
        let network = self.directory(&request.name)?;
        store::exchange_until(&network, ControlAction::AttachmentStart, deadline)?;
        let mut last = connecting(binding.profile.name.clone());
        loop {
            remaining(deadline).map_err(|_| attachment_deadline(&last))?;
            let status = self
                .dataspace_status_until(&request.name, deadline)
                .map_err(|error| {
                    if Instant::now() >= deadline {
                        attachment_deadline(&last)
                    } else {
                        error
                    }
                })?
                .ok_or_else(|| Error::Invalid("worker has no bound attachment".into()))?;
            if attachment_complete(&status.attachment, deadline)? {
                return Ok(status);
            }
            if status.local.phase != ManagedPhase::Ready {
                return Err(Error::Invalid(
                    "private validators stopped during attachment".into(),
                ));
            }
            last = status.attachment;
            thread::sleep(POLL.min(remaining(deadline).map_err(|_| attachment_deadline(&last))?));
        }
    }

    /// Observe an explicitly configured remote attachment separately from local readiness.
    ///
    /// `None` means this context has no remote binding. Stopped contexts expose only retained
    /// historical progress and mark the parent unavailable; live status requires native IPC.
    ///
    /// # Errors
    /// Missing generation, unsafe custody, changed child identity or unavailable live owner.
    pub fn dataspace_status(&self, name: &str) -> Result<Option<ManagedDataspaceStatus>> {
        // Standalone observation retains two ordinary I/O allowances, shared across both reads.
        self.dataspace_status_until(name, Instant::now() + transport::IO_TIMEOUT * 2)
    }

    fn dataspace_status_until(
        &self,
        name: &str,
        deadline: Instant,
    ) -> Result<Option<ManagedDataspaceStatus>> {
        let local = self.status_until(name, deadline)?;
        let Some(directory) = existing_outer(self, name)? else {
            return Ok(None);
        };
        let binding = read_binding(&directory)?;
        let prepared = self.prepared(name)?;
        verify_generation_binding(self, name, &binding, &prepared)?;
        if matches!(local.phase, ManagedPhase::Ready | ManagedPhase::Starting) {
            let network = self.directory(name)?;
            let worker: WorkerRecord = decode(&network.read(WORKER, MAX_METADATA)?)?;
            let response: ManagedDataspaceStatus = transport::request_as_until(
                &network,
                &ControlRequest {
                    token: worker.token,
                    action: ControlAction::AttachmentStatus,
                },
                deadline,
            )?;
            if response.local.context != local.context
                || response.attachment.network != binding.profile.name
            {
                return Err(Error::Invalid(
                    "worker attachment belongs to another context".into(),
                ));
            }
            return Ok(Some(response));
        }
        let mut attachment = match directory.read_optional(PROGRESS, MAX_METADATA)? {
            Some(bytes) => decode(&bytes)?,
            None => connecting(binding.profile.name.clone()),
        };
        if attachment.network != binding.profile.name {
            return Err(Error::Invalid(
                "retained attachment status selects another parent".into(),
            ));
        }
        attachment.stage = ManagedAttachmentPhase::Unavailable;
        attachment.failure = Some(ManagedAttachmentFailure::SupervisorStopped);
        Ok(Some(ManagedDataspaceStatus { local, attachment }))
    }
}

/// A worker-owned public observation; all network clients and journals live in its thread.
pub(super) struct AttachmentWorker {
    status: Arc<Mutex<ManagedAttachmentStatus>>,
    thread: thread::JoinHandle<()>,
}
impl AttachmentWorker {
    pub(super) fn start(
        store: &ManagedStore,
        name: &str,
        prepared: &PreparedLocalnet,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Option<Self>> {
        let Some(directory) = existing_outer(store, name)? else {
            return Ok(None);
        };
        let binding = read_binding(&directory)?;
        if binding.context.is_none() {
            return Ok(None);
        }
        verify_generation_binding(store, name, &binding, prepared)?;
        let status = Arc::new(Mutex::new(connecting(binding.profile.name.clone())));
        let shared = Arc::clone(&status);
        let prepared = prepared.clone();
        let bootstrap = bootstrap_path(store, name);
        let thread = thread::Builder::new()
            .name("private-parent-relay".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                relay_loop(
                    &directory, &bootstrap, &binding, &prepared, &shared, &cancelled,
                );
            })?;
        Ok(Some(Self { status, thread }))
    }
    pub(super) fn status(&self) -> ManagedAttachmentStatus {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if self.thread.is_finished() {
            status.stage = ManagedAttachmentPhase::Unavailable;
            status
                .failure
                .get_or_insert(ManagedAttachmentFailure::WorkerUnavailable);
        }
        status
    }
    pub(super) fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }
    /// Consume the actual relay task after its caller has cancelled the shared owner.
    /// No detached timeout can release ownership while a finite relay turn still mutates.
    pub(super) fn join(self) -> Result<()> {
        self.thread
            .join()
            .map_err(|_| Error::Invalid("owned parent attachment task panicked".into()))
    }
}

/// Report a bound but inactive owner without exposing its internal filesystem error.
pub(super) fn inactive_status(
    store: &ManagedStore,
    name: &str,
    prepared: &PreparedLocalnet,
) -> Result<Option<ManagedAttachmentStatus>> {
    let Some(directory) = existing_outer(store, name)? else {
        return Ok(None);
    };
    let binding = read_binding(&directory)?;
    if binding.context.is_none() {
        return Ok(None);
    }
    verify_generation_binding(store, name, &binding, prepared)?;
    let mut status = connecting(binding.profile.name);
    status.stage = ManagedAttachmentPhase::Unavailable;
    status.failure = Some(ManagedAttachmentFailure::WorkerUnavailable);
    Ok(Some(status))
}

fn relay_loop(
    directory: &PrivateDirectory,
    bootstrap_path: &Path,
    binding: &Binding,
    prepared: &PreparedLocalnet,
    status: &Mutex<ManagedAttachmentStatus>,
    cancelled: &Arc<AtomicBool>,
) {
    let mut service = None;
    // Capture a current compact checkpoint after private validators finish starting.
    let mut refresh = true;
    while !cancelled.load(Ordering::Acquire) {
        let result = relay_turn(
            directory,
            bootstrap_path,
            binding,
            prepared,
            &mut service,
            status,
            RelayControl { refresh, cancelled },
        );
        // Refresh native certificates after a failed observation without clearing the pinned
        // network, original private identity or transaction journals.
        refresh = result.is_err();
        if let Err(failure) = result {
            // An uncertain publication poisons native stores until reopen. Reopen through their
            // canonical owners on the next bounded turn; never replace or rewind any journal.
            service = None;
            let mut public = status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !public
                .failure
                .is_some_and(ManagedAttachmentFailure::is_terminal_operation)
            {
                public.failure = Some(failure);
            }
        }
        let public = status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let published = encode(&public).and_then(|bytes| {
            directory
                .write_atomic(PROGRESS, &bytes, PublishMode::Replace)
                .map_err(Error::from)
        });
        if published.is_err() {
            let mut public = status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            public.failure = Some(ManagedAttachmentFailure::CustodyUnavailable);
            return;
        }
        for _ in 0..20 {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(POLL);
        }
    }
}
#[derive(Clone, Copy)]
struct RelayControl<'a> {
    refresh: bool,
    cancelled: &'a Arc<AtomicBool>,
}

fn can_relay_in_turn(progress: ProvisioningProgress, parent_confirmed: bool) -> bool {
    // Reopening may first reveal an independently confirmed registration after this
    // turn selected its startup budget. Record that fact, then let the next turn use
    // its full background budget before preparing any new anchor.
    (parent_confirmed || progress.confirmed.is_none())
        && matches!(
            progress.stage,
            ProvisioningStage::Registering | ProvisioningStage::Attached
        )
}

fn relay_turn(
    directory: &PrivateDirectory,
    bootstrap_path: &Path,
    binding: &Binding,
    prepared: &PreparedLocalnet,
    service: &mut Option<RemoteProvisioning>,
    status: &Mutex<ManagedAttachmentStatus>,
    control: RelayControl<'_>,
) -> std::result::Result<(), ManagedAttachmentFailure> {
    let RelayControl { refresh, cancelled } = control;
    if cancelled.load(Ordering::Acquire) {
        return Err(ManagedAttachmentFailure::SupervisorStopped);
    }
    let runtime = InstalledRuntime::discover()?;
    let profiles = runtime.network_profiles()?;
    let profile = profiles
        .select(&binding.profile.name)
        .map_err(ManagedAttachmentFailure::from)?;
    let parent_confirmed = status
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .parent_confirmed
        .is_some();
    let deadline = attachment_turn_deadline(directory, parent_confirmed)?;
    let retained = bind_installed_profile(directory, profile, deadline)?;
    if retained.spec != binding.spec
        || retained.account_alias != binding.account_alias
        || retained.context != binding.context
    {
        return Err(ManagedAttachmentFailure::ContextRejected);
    }
    let bootstrap = authenticate_parent(bootstrap_path, profile, deadline, refresh)
        .map_err(|_| ManagedAttachmentFailure::ParentAuthenticationFailed)?;
    if bootstrap.metadata().network_id != binding.spec.parent_network_id {
        return Err(ManagedAttachmentFailure::ContextRejected);
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(ManagedAttachmentFailure::SupervisorStopped);
    }
    if service.is_none() {
        *service = Some(
            RemoteProvisioning::open(
                &directory.path().join("provisioning"),
                &bootstrap,
                prepared,
                &binding.account_alias,
            )
            .and_then(|service| service.with_cancellation(Arc::clone(cancelled)))
            .map_err(ManagedAttachmentFailure::from)?,
        );
    }
    let service = service
        .as_mut()
        .ok_or_else(|| Error::Invalid("attachment owner is unavailable".into()))?;
    {
        let mut public = status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        record_provisioning_progress(&mut public, service.progress(), false);
    }
    let result = service.provision_once(&bootstrap, deadline);
    let progress = {
        let mut public = status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match result {
            Ok(progress) => {
                record_provisioning_progress(&mut public, progress, true);
                progress
            }
            Err(error) => {
                // Funding may have completed before a later namespace step failed. Read the
                // owner's retained stage; never infer it from a request or response message.
                record_provisioning_progress(&mut public, service.progress(), false);
                return Err(error.into());
            }
        }
    };
    if can_relay_in_turn(progress, parent_confirmed) {
        let relay = service
            .relay_once(&bootstrap, deadline)
            .map_err(ManagedAttachmentFailure::from)?;
        let mut public = status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        record_relay_progress(&mut public, relay);
    }
    Ok(())
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
