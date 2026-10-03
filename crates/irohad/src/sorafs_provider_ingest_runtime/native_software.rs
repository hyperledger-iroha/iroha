//! Native software completion custody and durable local checkpoint CAS for publisher bootstrap.
//!
//! Filesystem durability protects process crashes and concurrent writers. Finalized ledger replay
//! remains the independent authority after restoration; this module claims no hardware rollback seal.

use super::*;
#[path = "native_source.rs"]
mod source;
use iroha_crypto::{ExposedPrivateKey, KeyPair};
use iroha_data_model::transaction::Executable;
use sorafs_node::ProviderIngestSealedCheckpointRecordV1;
use source::NativeAssignedSourceV1;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    sync::OnceLock,
};
use zeroize::Zeroizing;

/// Prepared credential custody; State is bound once after authenticated replay and before work starts.
pub(crate) struct NativeProducerV1 {
    resolver: Arc<NativeResolverV1>,
    checkpoint: Arc<NativeCheckpointV1>,
    source: Arc<NativeAssignedSourceV1>,
}
impl NativeProducerV1 {
    pub(crate) fn prepare(
        config: &SorafsProviderIngestRuntime,
        provider: ProviderId,
        data_dir: &Path,
    ) -> Result<Self> {
        let path = config
            .native_completion_credential
            .as_ref()
            .ok_or_else(|| eyre::eyre!("native completion credential is absent"))?;
        let bytes =
            crate::runtime_credential::load_bounded_runtime_credential_v1(path, 2, 16 * 1024 + 256)
                .map_err(|_| eyre::eyre!("native completion credential rejected"))?;
        let text = bytes
            .strip_suffix(b"\n")
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or_else(|| eyre::eyre!("native completion credential rejected"))?;
        let private: ExposedPrivateKey = text
            .parse()
            .map_err(|_| eyre::eyre!("native completion credential rejected"))?;
        let canonical = Zeroizing::new(
            private
                .try_to_multihash_string()
                .map_err(|_| eyre::eyre!("native completion credential rejected"))?,
        );
        let key = KeyPair::from_private_key(private.0)
            .map_err(|_| eyre::eyre!("native completion credential rejected"))?;
        if canonical.as_str() != text || key.public_key() != &config.completion_signer_public_key {
            bail!("native completion credential does not match the configured public binding");
        }
        if config.finalized_archive.retention_authority.is_some()
            || config.provider_attestation_journal.is_some()
        {
            bail!(
                "native completion custody does not provide external archive retention or Musubi attestation services"
            );
        }
        let resolver = Arc::new(NativeResolverV1 {
            config: config.clone(),
            provider,
            key: Arc::new(key),
            state: Arc::new(OnceLock::new()),
            owner: AccountId::new(config.completion_signer_public_key.clone()),
        });
        Ok(Self {
            resolver: Arc::clone(&resolver),
            checkpoint: Arc::new(NativeCheckpointV1 {
                handle: config.checkpoint_store_handle.clone(),
                qualification: configured_checkpoint_qualification(config),
                root: data_dir.join("provider-ingest-native-authority"),
                maximum: config.outbox.checkpoint_max_bytes.0,
                gate: Mutex::new(()),
                lock: OnceLock::new(),
            }),
            source: Arc::new(NativeAssignedSourceV1::new(resolver)?),
        })
    }
    pub(crate) fn bind_state(&self, state: Arc<State>) -> Result<()> {
        self.resolver
            .state
            .set(state)
            .map_err(|_| eyre::eyre!("native completion State was already bound"))
    }
    pub(crate) async fn preflight(
        &self,
        config: &SorafsProviderIngestRuntime,
        provider: ProviderId,
    ) -> Result<QualifiedProviderIngestRuntimeAdaptersV1> {
        preflight_runtime_adapters(
            config,
            provider,
            ProviderIngestRuntimeAdaptersV1::new(self.source.clone(), self.resolver.clone()),
            self.checkpoint.clone(),
        )
        .await
    }
}

#[derive(Clone)]
struct NativeResolverV1 {
    config: SorafsProviderIngestRuntime,
    provider: ProviderId,
    key: Arc<KeyPair>,
    owner: AccountId,
    state: Arc<OnceLock<Arc<State>>>,
}
impl NativeResolverV1 {
    fn state(&self) -> std::result::Result<&Arc<State>, ProviderIngestCompletionSignerErrorV1> {
        self.state
            .get()
            .ok_or(ProviderIngestCompletionSignerErrorV1::Unavailable)
    }
    fn eligible(&self, world: &impl iroha_core::state::WorldReadOnly) -> bool {
        let permission = |permission: &iroha_data_model::permission::Permission| {
            permission.name() == "CanCompleteSorafsReplicationOrder"
                && permission.payload().get().as_str() == "null"
        };
        let permitted = world
            .account_permissions()
            .get(&self.owner)
            .is_some_and(|permissions| permissions.iter().any(permission))
            || world
                .account_roles_iter(&self.owner)
                .filter_map(|id| world.roles().get(id))
                .any(|role| role.permissions().any(permission));
        permitted
            && world.provider_owners().get(&self.provider) == Some(&self.owner)
            && world
                .provider_ingest_completion_authorities()
                .get(&self.provider)
                .is_some_and(|authority| {
                    authority.provider_owner == self.owner
                        && authority.signer_policy == self.config.completion_signer_policy
                })
    }
    fn check_context(
        &self,
        context: &ProviderIngestCompletionSignerResolutionContextV1,
    ) -> std::result::Result<(), ProviderIngestCompletionSignerErrorV1> {
        let rejected = ProviderIngestCompletionSignerErrorV1::Rejected;
        if !context.is_valid()
            || context.provider_owner != self.owner
            || context.signer_policy != self.config.completion_signer_policy
        {
            return Err(rejected);
        }
        let view = self.state()?.view();
        if !self.eligible(view.world()) {
            return Err(rejected);
        }
        iroha_core::query::signer_finality::verify_signer_finality_v1(
            &view,
            context.finalized_cursor.height,
            context.finalized_cursor.block_hash,
        )
        .map_err(|_| rejected)?;
        let finalized_now = view
            .latest_block()
            .map_err(|error| match error {
                iroha_core::execution_attempt::ExecutionAttemptError::Deferred(_) => {
                    ProviderIngestCompletionSignerErrorV1::Unavailable
                }
                iroha_core::execution_attempt::ExecutionAttemptError::Rejected(_) => rejected,
            })?
            .ok_or(rejected)?
            .header()
            .creation_time()
            .as_secs();
        let now = finalized_now.max(source::now().map_err(|_| rejected)? / 1000);
        if iroha_core::query::provider_admission::read_finalized_provider_admission_v1(
            &view,
            self.provider,
            now,
        )
        .map_err(|_| rejected)?
        .is_none()
        {
            return Err(rejected);
        }
        Ok(())
    }
    fn sign_exact(
        &self,
        context: &ProviderIngestCompletionSignerResolutionContextV1,
        payload: TransactionPayload,
    ) -> std::result::Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1> {
        let rejected = ProviderIngestCompletionSignerErrorV1::Rejected;
        if !completion_payload_matches_resolution_context(&payload, context, self.provider) {
            return Err(rejected);
        }
        let state = self.state()?;
        let view = state.view();
        if !self.eligible(view.world()) || payload.network_id() != Some(state.network_id_ref()) {
            return Err(rejected);
        }
        iroha_core::query::signer_finality::verify_signer_finality_v1(
            &view,
            context.finalized_cursor.height,
            context.finalized_cursor.block_hash,
        )
        .map_err(|_| rejected)?;
        let finalized_now = view
            .latest_block()
            .map_err(|error| match error {
                iroha_core::execution_attempt::ExecutionAttemptError::Deferred(_) => {
                    ProviderIngestCompletionSignerErrorV1::Unavailable
                }
                iroha_core::execution_attempt::ExecutionAttemptError::Rejected(_) => rejected,
            })?
            .ok_or(rejected)?
            .header()
            .creation_time()
            .as_secs();
        let now = finalized_now.max(source::now().map_err(|_| rejected)? / 1000);
        if iroha_core::query::provider_admission::read_finalized_provider_admission_v1(
            &view,
            self.provider,
            now,
        )
        .map_err(|_| rejected)?
        .is_none()
        {
            return Err(rejected);
        }
        let Executable::Instructions(instructions) = payload.instructions() else {
            return Err(rejected);
        };
        let completion = instructions[0]
            .as_any()
            .downcast_ref::<CompleteReplicationOrder>()
            .ok_or(rejected)?;
        let order = view
            .world()
            .replication_orders()
            .get(completion.order_id())
            .ok_or(rejected)?;
        let pin = view
            .world()
            .pin_manifests()
            .get(&order.manifest_digest)
            .ok_or(rejected)?;
        let authority = view
            .world()
            .provider_ingest_completion_authorities()
            .get(&self.provider)
            .ok_or(rejected)?;
        if authority != completion.expected_authority() || pin.policy.retention_epoch <= now {
            return Err(rejected);
        }
        let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            context.finalized_cursor.height,
            context.finalized_cursor.block_hash,
            *self.provider.as_bytes(),
            *order.order_id.as_bytes(),
            *pin.digest.as_bytes(),
            pin.root_cid.as_bytes().to_vec(),
            pin.chunker.to_handle(),
            pin.chunk_digest_sha3_256,
            pin.por_root,
            pin.content_length,
        )
        .map_err(|_| rejected)?;
        let request = ProviderIngestCompletionPayloadRequestV1 {
            authorization,
            provider_owner: self.owner.clone(),
            expected_authority: authority.clone(),
            expected_assignment_revision: context.expected_assignment_revision,
            network_id: *state.network_id_ref(),
            completion_epoch: *completion.completion_epoch(),
            finalized_cursor: context.finalized_cursor,
        };
        if now > order.deadline_epoch {
            return Err(rejected);
        }
        validate_completion_order_binding(&request, self.provider, order, pin, finalized_now)
            .map_err(|_| rejected)?;
        // Retain this exact immutable State view until the exact payload is signed. The queue and
        // native completion instruction repeat current authority and assignment checks at commit.
        TransactionBuilder::from_payload(payload)
            .and_then(|builder| builder.try_sign(self.key.private_key()))
            .map_err(|_| rejected)
    }
}
impl ProviderIngestGovernedSignerResolverRuntimeV1 for NativeResolverV1 {
    fn runtime_handle(&self) -> &str {
        &self.config.completion_signer_resolver_handle
    }
    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestRuntimeProviderQualificationV1,
        ProviderIngestCompletionSignerResolverErrorV1,
    > {
        Ok(configured_completion_signer_resolver_qualification(
            &self.config,
        ))
    }
    fn signer_binding(
        &self,
    ) -> std::result::Result<
        ProviderIngestCompletionSignerBindingV1,
        ProviderIngestCompletionSignerResolverErrorV1,
    > {
        Ok(configured_completion_signer_binding(&self.config))
    }
    fn check_readiness(
        &self,
    ) -> std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1> {
        Ok(())
    }
    fn resolve(
        &self,
        context: ProviderIngestCompletionSignerResolutionContextV1,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<
            Option<Arc<dyn ProviderIngestCompletionSignerV1>>,
            ProviderIngestCompletionSignerResolverErrorV1,
        >,
    > {
        let resolver = self.clone();
        Box::pin(async move {
            crate::panic_recovery::join_recoverable(
                crate::panic_recovery::spawn_blocking_recoverable(move || {
                    resolver
                        .check_context(&context)
                        .map_err(|_| ProviderIngestCompletionSignerResolverErrorV1::Rejected)?;
                    let signer: Arc<dyn ProviderIngestCompletionSignerV1> =
                        Arc::new(NativeSignerV1 { resolver, context });
                    Ok(Some(signer))
                }),
            )
            .await
            .map_err(|_| ProviderIngestCompletionSignerResolverErrorV1::Unavailable)?
        })
    }
}
struct NativeSignerV1 {
    resolver: NativeResolverV1,
    context: ProviderIngestCompletionSignerResolutionContextV1,
}
impl ProviderIngestCompletionSignerV1 for NativeSignerV1 {
    fn runtime_handle(&self) -> &str {
        &self.resolver.config.completion_signer_handle
    }
    fn authority(&self) -> &AccountId {
        &self.resolver.owner
    }
    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestCompletionSignerQualificationV1,
        ProviderIngestCompletionSignerErrorV1,
    > {
        Ok(configured_completion_signer_binding(&self.resolver.config).qualification)
    }
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        self.resolver.config.completion_signer_policy
    }
    fn current_eligibility(
        &self,
    ) -> std::result::Result<
        ProviderIngestCompletionSignerPolicyV1,
        ProviderIngestCompletionSignerErrorV1,
    > {
        let view = self.resolver.state()?.view();
        if !self.resolver.eligible(view.world()) {
            return Err(ProviderIngestCompletionSignerErrorV1::Rejected);
        }
        Ok(self.signer_policy())
    }
    fn sign(
        &self,
        payload: TransactionPayload,
    ) -> ProviderIngestFutureV1<
        '_,
        std::result::Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>,
    > {
        let resolver = self.resolver.clone();
        let context = self.context.clone();
        Box::pin(async move {
            crate::panic_recovery::join_recoverable(
                crate::panic_recovery::spawn_blocking_recoverable(move || {
                    resolver.sign_exact(&context, payload)
                }),
            )
            .await
            .map_err(|_| ProviderIngestCompletionSignerErrorV1::Unavailable)?
        })
    }
}

struct NativeCheckpointV1 {
    handle: String,
    qualification: ProviderIngestCheckpointProviderQualificationV1,
    root: PathBuf,
    maximum: u64,
    gate: Mutex<()>,
    lock: OnceLock<File>,
}
impl fmt::Debug for NativeCheckpointV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeCheckpointV1").finish_non_exhaustive()
    }
}
impl NativeCheckpointV1 {
    fn open(&self) -> std::result::Result<(), ProviderIngestCheckpointExternalErrorV1> {
        let rejected = ProviderIngestCheckpointExternalErrorV1::Rejected;
        if self.lock.get().is_none() {
            match fs::create_dir(&self.root) {
                Ok(()) => {
                    fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))
                        .map_err(|_| rejected)?;
                    File::open(self.root.parent().ok_or(rejected)?)
                        .and_then(|file| file.sync_all())
                        .map_err(|_| rejected)?;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(ProviderIngestCheckpointExternalErrorV1::Unavailable),
            }
            let metadata = fs::symlink_metadata(&self.root).map_err(|_| rejected)?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.mode() & 0o077 != 0
                || metadata.uid() != rustix::process::geteuid().as_raw()
            {
                return Err(rejected);
            }
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .mode(0o600)
                .custom_flags(
                    (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32,
                )
                .open(self.root.join("lock"))
                .map_err(|_| rejected)?;
            let metadata = lock.metadata().map_err(|_| rejected)?;
            if !metadata.is_file() || metadata.nlink() != 1 || metadata.mode() & 0o077 != 0 {
                return Err(rejected);
            }
            rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
                .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Unavailable)?;
            self.lock.set(lock).map_err(|_| rejected)?;
        }
        let lock = self
            .lock
            .get()
            .ok_or(rejected)?
            .metadata()
            .map_err(|_| rejected)?;
        let named = fs::symlink_metadata(self.root.join("lock")).map_err(|_| rejected)?;
        if lock.dev() != named.dev() || lock.ino() != named.ino() || named.nlink() != 1 {
            return Err(rejected);
        }
        Ok(())
    }
    fn read(
        &self,
    ) -> std::result::Result<
        Option<ProviderIngestSealedCheckpointRecordV1>,
        ProviderIngestCheckpointExternalErrorV1,
    > {
        let rejected = ProviderIngestCheckpointExternalErrorV1::Rejected;
        let mut file = match OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32,
            )
            .open(self.root.join("checkpoint.to"))
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(rejected),
        };
        let metadata = file.metadata().map_err(|_| rejected)?;
        let maximum = self.maximum.checked_add(4096).ok_or(rejected)?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.mode() & 0o077 != 0
            || metadata.len() > maximum
        {
            return Err(rejected);
        }
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(maximum + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| rejected)?;
        if bytes.len() as u64 > maximum {
            return Err(rejected);
        }
        ProviderIngestSealedCheckpointRecordV1::from_canonical_bytes(&bytes, self.maximum)
            .map(Some)
            .map_err(|_| rejected)
    }
}
impl ProviderIngestCheckpointRuntimeV1 for NativeCheckpointV1 {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestCheckpointProviderQualificationV1,
        ProviderIngestCheckpointExternalErrorV1,
    > {
        Ok(self.qualification)
    }
    fn load_latest(
        &self,
    ) -> std::result::Result<
        Option<ProviderIngestSealedCheckpointRecordV1>,
        ProviderIngestCheckpointExternalErrorV1,
    > {
        let _gate = self
            .gate
            .lock()
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Unavailable)?;
        self.open()?;
        self.read()
    }
    fn compare_and_swap_latest(
        &self,
        expected: Option<[u8; 32]>,
        next: &ProviderIngestSealedCheckpointRecordV1,
    ) -> std::result::Result<(), ProviderIngestCheckpointExternalErrorV1> {
        let rejected = ProviderIngestCheckpointExternalErrorV1::Rejected;
        let _gate = self
            .gate
            .lock()
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Unavailable)?;
        self.open()?;
        next.validate(self.maximum).map_err(|_| rejected)?;
        let current = self.read()?;
        if current.as_ref().map(|value| value.revision) != expected
            || next.predecessor_revision != expected
            || next.predecessor_checkpoint_digest
                != current.as_ref().map(|value| value.checkpoint_digest)
            || next.checkpoint_sequence
                != current
                    .as_ref()
                    .map_or(Some(1), |value| value.checkpoint_sequence.checked_add(1))
                    .ok_or(rejected)?
        {
            return Err(rejected);
        }
        let bytes = next
            .to_canonical_bytes(self.maximum)
            .map_err(|_| rejected)?;
        let temporary = self.root.join("checkpoint.pending");
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) => {
                if !metadata.is_file()
                    || metadata.file_type().is_symlink()
                    || metadata.nlink() != 1
                    || metadata.uid() != rustix::process::geteuid().as_raw()
                    || metadata.mode() & 0o077 != 0
                {
                    return Err(rejected);
                }
                fs::remove_file(&temporary).map_err(|_| rejected)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(rejected),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32,
            )
            .open(&temporary)
            .map_err(|_| rejected)?;
        if file.metadata().map_err(|_| rejected)?.nlink() != 1 {
            return Err(rejected);
        }
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Unavailable)?;
        fs::rename(&temporary, self.root.join("checkpoint.to"))
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Ambiguous)?;
        File::open(&self.root)
            .and_then(|file| file.sync_all())
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Ambiguous)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_software_tests.rs"]
mod tests;
