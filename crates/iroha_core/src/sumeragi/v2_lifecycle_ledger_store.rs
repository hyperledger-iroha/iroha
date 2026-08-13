/// Typed LifecycleLedgerV1 load or persistence failure.
#[derive(Debug, Error)]
pub(in crate::sumeragi) enum LifecycleLedgerError {
    /// A filesystem operation failed.
    #[error("{0}")]
    Io(String),
    /// Frame bytes were malformed or noncanonical.
    #[error("invalid LifecycleLedgerV1 frame: {0}")]
    InvalidFrame(String),
    /// Decoded logical state violated a durable invariant.
    #[error("invalid LifecycleLedgerV1 state: {0}")]
    InvalidLedger(String),
}
/// Post-fsync receipt for one exact WAL-ahead Validate-to-Sign ledger repair.
///
/// Construction is private to [`LifecycleLedgerStoreV1`]. The receipt binds
/// both semantic keys, the typed edge and child ordinal, and the complete
/// framed ledger bytes which were published before it was returned.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DurableWalVoteLedgerRepairReceipt {
    store_path: PathBuf,
    context: LifecycleContext,
    parent_key: LifecycleKey,
    child_key: LifecycleKey,
    edge: DurableContinuationEdge,
    child_ordinal: u128,
    ledger_frame_hash: LifecycleDigest,
}
impl DurableWalVoteLedgerRepairReceipt {
    /// Return whether this receipt names one exact authenticated repair.
    pub(super) fn matches(&self, repair: &AuthenticatedWalVoteLifecycleRepair) -> bool {
        self.context.id() == repair.parent().key.context()
            && self.context.height() == repair.parent().key.round().height()
            && self.parent_key == repair.parent().key
            && self.child_key == repair.child().key
            && self.edge == repair.edge()
            && self.child_ordinal != 0
    }
    /// Return the durable child ordinal named by the published ledger.
    pub(super) const fn child_ordinal(&self) -> u128 {
        self.child_ordinal
    }
    /// Return the hash of the complete canonical ledger frame.
    pub(super) const fn ledger_frame_hash(&self) -> LifecycleDigest {
        self.ledger_frame_hash
    }
    /// Return whether the receipt belongs to this exact opened ledger store.
    pub(super) fn belongs_to(&self, store: &LifecycleLedgerStoreV1) -> bool {
        store
            .load()
            .ok()
            .is_some_and(|ledger| self.belongs_to_loaded(store, &ledger))
    }
    /// Validate this receipt against one already-loaded frame from its store.
    /// Keeping this comparison load-free lets the Sign-install preflight bind
    /// the frame hash and repaired-pair shape to the same read.
    pub(super) fn belongs_to_loaded(
        &self,
        store: &LifecycleLedgerStoreV1,
        ledger: &LifecycleLedgerV1,
    ) -> bool {
        self.store_path == store.path
            && self.context == store.context
            && ledger.context() == self.context
            && encode_frame(ledger, store.max_frame_bytes)
                .ok()
                .is_some_and(|frame| {
                    LifecycleDigest::new(Hash::new(frame).into()) == self.ledger_frame_hash
                })
    }
}
/// Crash-safe, bounded store for one height-local LifecycleLedgerV1.
#[derive(Clone, Debug)]
pub(in crate::sumeragi) struct LifecycleLedgerStoreV1 {
    path: PathBuf,
    context: LifecycleContext,
    max_records: usize,
    max_frame_bytes: u64,
}
impl LifecycleLedgerStoreV1 {
    fn is_authorized_complete_tip_predecessor_target(
        &self,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> bool {
        self.path.parent().is_some_and(|root| {
            complete_tip.authorizes_predecessor_lifecycle_root(root)
                && self.path == root.join(LEDGER_FILE)
        })
    }
    /// Compare the complete immutable publication target of two open handles.
    pub(super) fn same_publication_target(&self, other: &Self) -> bool {
        self.path == other.path
            && self.context == other.context
            && self.max_records == other.max_records
            && self.max_frame_bytes == other.max_frame_bytes
    }
    /// Open a height-local ledger under the coordinator's sealed size bounds.
    pub(in crate::sumeragi) fn open(
        root: &Path,
        context: LifecycleContext,
    ) -> Result<(Self, LifecycleLedgerV1), LifecycleLedgerError> {
        ensure_durable_ledger_directory(root)?;
        let store = Self {
            path: root.join(LEDGER_FILE),
            context,
            max_records: MAX_LIFECYCLE_RECORDS_PER_HEIGHT,
            max_frame_bytes: MAX_LEDGER_FRAME_BYTES,
        };
        let ledger = store.load()?;
        Ok((store, ledger))
    }
    pub(super) fn load(&self) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(LifecycleLedgerV1::empty(self.context));
            }
            Err(error) => {
                return Err(LifecycleLedgerError::Io(format!(
                    "failed to inspect lifecycle ledger {}: {error}",
                    self.path.display()
                )));
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LifecycleLedgerError::InvalidFrame(
                "ledger path is not a regular file".to_owned(),
            ));
        }
        if metadata.len() > self.max_frame_bytes {
            return Err(LifecycleLedgerError::InvalidFrame(
                "ledger exceeds its configured byte bound".to_owned(),
            ));
        }
        let read_limit = self.max_frame_bytes.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidFrame("ledger read bound overflowed".to_owned())
        })?;
        let mut bytes = Vec::new();
        File::open(&self.path)
            .and_then(|file| file.take(read_limit).read_to_end(&mut bytes))
            .map_err(|error| {
                LifecycleLedgerError::Io(format!(
                    "failed to read lifecycle ledger {}: {error}",
                    self.path.display()
                ))
            })?;
        let ledger = decode_frame(&bytes, self.max_frame_bytes)?;
        if ledger.context() != self.context {
            return Err(LifecycleLedgerError::InvalidLedger(
                "ledger belongs to another height context".to_owned(),
            ));
        }
        ledger.validate(self.max_records)?;
        Ok(ledger)
    }
    /// Persist one exact staged successor only while the attached frame still
    /// equals the coordinator state from which it was derived.
    ///
    /// The equality read happens before any atomic replacement begins. An
    /// exact stutter confirms the already-fsynced frame without rewriting it;
    /// otherwise a successful return means `successor` is the exact fsynced V1
    /// frame replacing `current`. Callers may perform only infallible in-memory
    /// publication after this method returns.
    pub(super) fn persist_exact_successor(
        &self,
        current: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
    ) -> Result<(), LifecycleLedgerError> {
        if self.load()? != *current {
            return Err(LifecycleLedgerError::InvalidLedger(
                "attached lifecycle ledger changed before successor publication".to_owned(),
            ));
        }
        if current == successor {
            return Ok(());
        }
        self.persist(successor)
    }
    /// Reload and authenticate one already-fsynced WAL repair as an exact
    /// repaired-pair stutter.
    ///
    /// This is a read-only post-fsync/install preflight. It deliberately does
    /// not expose the loaded ledger: callers learn only whether the complete
    /// current frame contains the exact authenticated parent/child pair and
    /// durable child ordinal they already own.
    pub(super) fn revalidates_durable_authenticated_wal_vote_repair(
        &self,
        durable: &DurableAuthenticatedWalVoteLifecycleRepair,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        if !durable.belongs_to_loaded(self, &loaded) {
            return false;
        }
        let Ok((staged, observed_child_ordinal, changed)) =
            loaded.stage_authenticated_wal_vote_repair(durable.repair())
        else {
            return false;
        };
        !changed && observed_child_ordinal == durable.child_ordinal() && staged == loaded
    }
    /// Reopen and compare the complete exact control-Sign row without exposing it.
    pub(super) fn revalidates_authenticated_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
        ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        let Ok((staged, observed_ordinal, changed)) =
            loaded.stage_authenticated_wal_control_sign(projection)
        else {
            return false;
        };
        !changed
            && observed_ordinal == ordinal
            && staged == loaded
            && projection.exactly_matches_ledger_at(&loaded, ordinal)
    }
    /// Reopen and authenticate one Advanced control Sign with its live Broadcast.
    pub(super) fn revalidates_recovered_control_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        control: &AuthenticatedRecoveredWalControlProjection,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        parent_ordinal: u128,
        child_ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        loaded
            .authenticate_recovered_control_signed_broadcast(verified, control)
            .is_ok_and(|(recovered, parent, child)| {
                parent == parent_ordinal
                    && child == child_ordinal
                    && recovered.exactly_matches(broadcast)
            })
    }
    /// Reload and reauthenticate one control-owned Broadcast-plus-Sign pair.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn revalidates_recovered_control_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        control: &AuthenticatedRecoveredWalControlProjection,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        expected: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> bool {
        self.load().is_ok_and(|loaded| {
            loaded
                .authenticate_recovered_control_signed_broadcast_and_sign(
                    verified, control, combined,
                )
                .is_ok_and(|observed| observed == *expected)
        })
    }
    /// Reload and reauthenticate one phase-owned Broadcast-plus-Sign pair.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn revalidates_recovered_phase_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        repair: &DurableAuthenticatedWalVoteLifecycleRepair,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        expected: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> bool {
        self.load().is_ok_and(|loaded| {
            loaded
                .authenticate_recovered_phase_signed_broadcast_and_sign(verified, repair, combined)
                .is_ok_and(|observed| observed == *expected)
        })
    }
    /// Reopen and compare one already-fsynced Decision Fetch row.
    pub(super) fn revalidates_authenticated_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        let Ok((staged, observed_ordinal, changed)) =
            loaded.stage_authenticated_wal_decision_fetch(projection)
        else {
            return false;
        };
        !changed
            && observed_ordinal == ordinal
            && staged == loaded
            && projection.exactly_matches_ledger_at(&loaded, ordinal)
    }
    /// Reopen and compare one already-fsynced advanced Fetch plus live Store cut.
    pub(super) fn revalidates_recovered_decision_fetch_store(
        &self,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
        fetch_ordinal: u128,
        store: &RecoveredDecisionFetchStoreProjectionV1,
    ) -> bool {
        self.load().is_ok_and(|loaded| {
            loaded
                .authenticate_recovered_decision_fetch_store(fetch, store)
                .is_ok_and(|(observed_fetch, _)| observed_fetch == fetch_ordinal)
        })
    }
    /// Atomically replace the ledger after validating all durable invariants.
    pub(super) fn persist(&self, ledger: &LifecycleLedgerV1) -> Result<(), LifecycleLedgerError> {
        if ledger.context() != self.context {
            return Err(LifecycleLedgerError::InvalidLedger(
                "cannot persist a foreign height context".to_owned(),
            ));
        }
        ledger.validate(self.max_records)?;
        let bytes = encode_frame(ledger, self.max_frame_bytes)?;
        let parent = self.path.parent().ok_or_else(|| {
            LifecycleLedgerError::Io("ledger path has no parent directory".to_owned())
        })?;
        let temporary = self.path.with_extension("norito.tmp");
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(LifecycleLedgerError::InvalidFrame(
                    "ledger temporary path is not a regular file".to_owned(),
                ));
            }
            Ok(_) => {
                fs::remove_file(&temporary).map_err(|error| {
                    LifecycleLedgerError::Io(format!(
                        "failed to discard lifecycle ledger temporary file {}: {error}",
                        temporary.display()
                    ))
                })?;
                sync_ledger_directory(parent)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(LifecycleLedgerError::Io(format!(
                    "failed to inspect lifecycle ledger temporary file {}: {error}",
                    temporary.display()
                )));
            }
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                LifecycleLedgerError::Io(format!(
                    "failed to create lifecycle ledger temporary file {}: {error}",
                    temporary.display()
                ))
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|error| {
                LifecycleLedgerError::Io(format!(
                    "failed to sync lifecycle ledger temporary file {}: {error}",
                    temporary.display()
                ))
            })?;
        fs::rename(&temporary, &self.path).map_err(|error| {
            LifecycleLedgerError::Io(format!(
                "failed to publish lifecycle ledger {}: {error}",
                self.path.display()
            ))
        })?;
        sync_ledger_directory(parent)?;
        Ok(())
    }
    /// Stage and fsync one authenticated WAL-ahead lifecycle repair.
    ///
    /// The receipt is minted only after the complete replacement frame and
    /// owning directory are synced. Exact repeats are persisted idempotently
    /// and receive the same frame-bound receipt.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_authenticated_wal_vote_repair(
        &self,
        ledger: &LifecycleLedgerV1,
        repair: AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        (
            LifecycleLedgerV1,
            DurableAuthenticatedWalVoteLifecycleRepair,
            bool,
        ),
        (LifecycleLedgerError, AuthenticatedWalVoteLifecycleRepair),
    > {
        let loaded = match self.load() {
            Ok(loaded) => loaded,
            Err(error) => return Err((error, repair)),
        };
        if &loaded != ledger {
            return Err((
                LifecycleLedgerError::InvalidLedger(
                    "WAL repair attempted to replace a stale ledger snapshot".to_owned(),
                ),
                repair,
            ));
        }
        let (staged, child_ordinal, changed) =
            match loaded.stage_authenticated_wal_vote_repair(&repair) {
                Ok(staged) => staged,
                Err(error) => return Err((error, repair)),
            };
        let frame = match encode_frame(&staged, self.max_frame_bytes) {
            Ok(frame) => frame,
            Err(error) => return Err((error, repair)),
        };
        if let Err(error) = self.persist(&staged) {
            return Err((error, repair));
        }
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: self.path.clone(),
            context: self.context,
            parent_key: repair.parent().key,
            child_key: repair.child().key,
            edge: repair.edge(),
            child_ordinal,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        debug_assert!(receipt.belongs_to(self));
        let durable = match repair.bind_durable_ledger_receipt(receipt) {
            Ok(durable) => durable,
            Err((repair, _receipt)) => {
                return Err((
                    LifecycleLedgerError::InvalidLedger(
                        "post-fsync WAL repair receipt did not bind its authority".to_owned(),
                    ),
                    repair,
                ));
            }
        };
        Ok((staged, durable, changed))
    }
    /// Bind an already-persisted Validate→Sign repair beneath a live Broadcast.
    ///
    /// This is a read-only crash-recovery counterpart to the repair fsync
    /// method. It mints the same frame-bound durable repair receipt only when
    /// the current canonical store contains the exact Advanced
    /// Validate→Advanced Sign→live Broadcast lineage. No ledger bytes are
    /// rewritten and no volatile dispatch identity is reconstructed.
    #[allow(clippy::result_large_err)]
    pub(super) fn authenticate_wal_vote_repair_for_signed_broadcast(
        &self,
        ledger: &LifecycleLedgerV1,
        repair: AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        DurableAuthenticatedWalVoteLifecycleRepair,
        (LifecycleLedgerError, AuthenticatedWalVoteLifecycleRepair),
    > {
        let loaded = match self.load() {
            Ok(loaded) => loaded,
            Err(error) => return Err((error, repair)),
        };
        if &loaded != ledger {
            return Err((
                LifecycleLedgerError::InvalidLedger(
                    "signed Broadcast recovery observed a stale ledger snapshot".to_owned(),
                ),
                repair,
            ));
        }
        let Some((_parent_ordinal, child_ordinal, _broadcast_ordinal)) =
            loaded.recovered_phase_signed_broadcast_ordinals(&repair)
        else {
            return Err((
                LifecycleLedgerError::InvalidLedger(
                    "signed Broadcast recovery lost its exact WAL vote lineage".to_owned(),
                ),
                repair,
            ));
        };
        let frame = match encode_frame(&loaded, self.max_frame_bytes) {
            Ok(frame) => frame,
            Err(error) => return Err((error, repair)),
        };
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: self.path.clone(),
            context: self.context,
            parent_key: repair.parent().key,
            child_key: repair.child().key,
            edge: repair.edge(),
            child_ordinal,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        match repair.bind_durable_ledger_receipt(receipt) {
            Ok(durable) if durable.belongs_to_loaded(self, &loaded) => Ok(durable),
            Ok(_durable) => unreachable!(
                "new signed Broadcast repair receipt must bind its unchanged loaded frame"
            ),
            Err((repair, _receipt)) => Err((
                LifecycleLedgerError::InvalidLedger(
                    "signed Broadcast repair receipt did not bind its WAL authority".to_owned(),
                ),
                repair,
            )),
        }
    }
}
fn sync_ledger_directory(directory: &Path) -> Result<(), LifecycleLedgerError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            LifecycleLedgerError::Io(format!(
                "failed to sync lifecycle ledger directory {}: {error}",
                directory.display()
            ))
        })
}
fn ensure_durable_ledger_directory(root: &Path) -> Result<(), LifecycleLedgerError> {
    ensure_durable_ledger_directory_with(root, &mut sync_ledger_directory)
}
fn ensure_durable_ledger_directory_with<Sync>(
    root: &Path,
    sync: &mut Sync,
) -> Result<(), LifecycleLedgerError>
where
    Sync: FnMut(&Path) -> Result<(), LifecycleLedgerError>,
{
    let parent = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(LifecycleLedgerError::InvalidFrame(
                    "ledger root is not a regular directory".to_owned(),
                ));
            }
            sync(root)?;
            if parent != root {
                sync(parent)?;
            }
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(LifecycleLedgerError::Io(format!(
                "failed to inspect lifecycle ledger root {}: {error}",
                root.display()
            )));
        }
    }
    ensure_durable_ledger_directory_with(parent, sync)?;
    match fs::create_dir(root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(LifecycleLedgerError::Io(format!(
                "failed to create lifecycle ledger root {}: {error}",
                root.display()
            )));
        }
    }
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        LifecycleLedgerError::Io(format!(
            "failed to inspect created lifecycle ledger root {}: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LifecycleLedgerError::InvalidFrame(
            "ledger root is not a regular directory".to_owned(),
        ));
    }
    sync(root)?;
    sync(parent)?;
    Ok(())
}
impl LifecycleCoordinator {
    pub(super) fn stage_durable_transaction(&self) -> Self {
        Self {
            episode_authority: self.episode_authority.clone(),
            active_context: self.active_context,
            records: self.records.clone(),
            key_index: self.key_index.clone(),
            owner_index: self.owner_index.clone(),
            ready_index: self.ready_index.clone(),
            admission_waits: self.admission_waits.clone(),
            active_lease: self.active_lease.clone(),
            high_water: self.high_water,
            next_lease: self.next_lease,
            durable_records: self.durable_records.clone(),
            capacity_geometry: self.capacity_geometry.clone(),
            capacity_used: self.capacity_used.clone(),
            capacity_generation: self.capacity_generation.clone(),
            observed_generation: self.observed_generation.clone(),
            producer_debts: self.producer_debts.clone(),
            ledger_store: self.ledger_store.clone(),
            fault: self.fault,
        }
    }
    pub(super) fn persist_durable_projection(&self) -> Result<(), LifecycleLedgerError> {
        let Some(store) = self.ledger_store.as_ref() else {
            return Ok(());
        };
        store.persist(&LifecycleLedgerV1::from_coordinator(self)?)
    }
    /// Fsync one staged successor against this coordinator's exact attached
    /// LedgerV1 frame.
    ///
    /// Unlike the generic durable helper, this first-release transaction never
    /// accepts an in-memory-only coordinator. The staged copy must retain the
    /// same store identity, and the on-disk frame must still equal the live
    /// coordinator projection before it can be replaced.
    pub(super) fn persist_exact_staged_successor(
        &self,
        staged: &Self,
    ) -> Result<(), LifecycleLedgerError> {
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "live lifecycle publication requires an attached LedgerV1 store".to_owned(),
            )
        })?;
        let staged_store = staged.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "staged lifecycle successor lost its attached LedgerV1 store".to_owned(),
            )
        })?;
        if !store.same_publication_target(staged_store) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged lifecycle successor changed its LedgerV1 store".to_owned(),
            ));
        }
        let current = LifecycleLedgerV1::from_coordinator(self)?;
        let successor = LifecycleLedgerV1::from_coordinator(staged)?;
        store.persist_exact_successor(&current, &successor)
    }
    /// Fsync one all-row finalized successor against this exact live owner.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn persist_exact_finalization_successor(
        self,
        staged: StagedFinalizationRetirementV1,
    ) -> Result<PublishedFinalizationRetirementV1, LifecycleLedgerError> {
        let StagedFinalizationRetirementV1 { current, retired } = staged;
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle retirement requires an attached LedgerV1 store".to_owned(),
            )
        })?;
        if LifecycleLedgerV1::from_coordinator(&self)? != current
            || current.context() != retired.context()
            || current.high_water() != retired.high_water()
            || current.records().len() != retired.records().len()
            || retired
                .records()
                .iter()
                .any(|record| record.terminal() == Some(None))
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle successor changed its exact live owner".to_owned(),
            ));
        }
        store.persist_exact_successor(&current, &retired)?;
        if store.load()? != retired {
            return Err(LifecycleLedgerError::InvalidLedger(
                "published finalization successor changed before owner commit".to_owned(),
            ));
        }
        Ok(PublishedFinalizationRetirementV1 {
            coordinator: self,
            current,
            retired,
        })
    }

    #[cfg(test)]
    pub(super) fn attach_empty_test_ledger(
        &mut self,
        root: &Path,
    ) -> Result<(), LifecycleLedgerError> {
        if self.ledger_store.is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "coordinator already owns a lifecycle ledger store".to_owned(),
            ));
        }
        let (store, existing) = LifecycleLedgerStoreV1::open(root, self.active_context)?;
        if existing.high_water != 0 || !existing.records.is_empty() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "test ledger attachment requires a new empty store".to_owned(),
            ));
        }
        store.persist(&LifecycleLedgerV1::from_coordinator(self)?)?;
        self.ledger_store = Some(store);
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn redirect_test_ledger_to_missing_parent(&mut self, root: &Path) {
        self.ledger_store
            .as_mut()
            .expect("test ledger is attached")
            .path = root.join("missing-parent").join(LEDGER_FILE);
    }
}
fn encode_frame(
    ledger: &LifecycleLedgerV1,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, LifecycleLedgerError> {
    let payload = ledger.encode();
    let payload_len = u64::try_from(payload.len()).map_err(|_| {
        LifecycleLedgerError::InvalidFrame("payload length is not representable".to_owned())
    })?;
    let frame_len = u64::try_from(HEADER_BYTES)
        .expect("header length fits u64")
        .checked_add(payload_len)
        .ok_or_else(|| LifecycleLedgerError::InvalidFrame("frame length overflowed".to_owned()))?;
    if frame_len > max_frame_bytes {
        return Err(LifecycleLedgerError::InvalidFrame(
            "frame exceeds its configured byte bound".to_owned(),
        ));
    }
    let digest = Hash::new(&payload);
    let mut frame =
        Vec::with_capacity(usize::try_from(frame_len).map_err(|_| {
            LifecycleLedgerError::InvalidFrame("frame is not addressable".to_owned())
        })?);
    frame.extend_from_slice(LEDGER_MAGIC);
    frame.extend_from_slice(&LEDGER_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(digest.as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}
fn decode_frame(
    bytes: &[u8],
    max_frame_bytes: u64,
) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_frame_bytes
        || bytes.len() < HEADER_BYTES
        || bytes.get(..LEDGER_MAGIC.len()) != Some(LEDGER_MAGIC.as_slice())
    {
        return Err(LifecycleLedgerError::InvalidFrame(
            "header or byte bound is invalid".to_owned(),
        ));
    }
    let version_offset = LEDGER_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| LifecycleLedgerError::InvalidFrame("version is truncated".to_owned()))?,
    );
    if version != LEDGER_VERSION {
        return Err(LifecycleLedgerError::InvalidFrame(format!(
            "unsupported frame version {version}"
        )));
    }
    let length_offset = version_offset + 2;
    let payload_len = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| LifecycleLedgerError::InvalidFrame("length is truncated".to_owned()))?,
    );
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| LifecycleLedgerError::InvalidFrame("payload is not addressable".to_owned()))?;
    let digest_offset = length_offset + 8;
    let payload_offset = digest_offset + HASH_BYTES;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err(LifecycleLedgerError::InvalidFrame(
            "frame length is inconsistent".to_owned(),
        ));
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
        return Err(LifecycleLedgerError::InvalidFrame(
            "checksum mismatch".to_owned(),
        ));
    }
    let mut cursor = payload;
    let ledger = LifecycleLedgerV1::decode_all(&mut cursor).map_err(|error| {
        LifecycleLedgerError::InvalidFrame(format!("Norito decode failed: {error}"))
    })?;
    if ledger.encode() != payload {
        return Err(LifecycleLedgerError::InvalidFrame(
            "payload is not canonically encoded".to_owned(),
        ));
    }
    Ok(ledger)
}
fn work_shape_is_valid(
    work_class: LifecycleWorkClass,
    key: LifecycleKey,
    stage: LifecycleStage,
) -> bool {
    work_class.accepts_stage(key.phase(), stage)
}
fn phase_code(phase: LifecyclePhase) -> u16 {
    match phase {
        LifecyclePhase::Proposal => 1,
        LifecyclePhase::Prepare => 2,
        LifecyclePhase::Commit => 3,
        LifecyclePhase::Timeout => 4,
        LifecyclePhase::Fetch => 5,
        LifecyclePhase::Store => 6,
        LifecyclePhase::Validate => 7,
        LifecyclePhase::Apply => 8,
        LifecyclePhase::BroadcastProposal => 9,
        LifecyclePhase::BroadcastPrepareVote => 10,
        LifecyclePhase::BroadcastCommitVote => 11,
        LifecyclePhase::BroadcastPrepareQc => 12,
        LifecyclePhase::BroadcastCommitQc => 13,
        LifecyclePhase::BroadcastTimeoutVote => 14,
        LifecyclePhase::BroadcastTc => 15,
        LifecyclePhase::EnterView => 16,
        LifecyclePhase::DiagnosticProposalEquivocation => 17,
        LifecyclePhase::DiagnosticVoteEquivocation => 18,
        LifecyclePhase::DiagnosticTimeoutEquivocation => 19,
        LifecyclePhase::DiagnosticInvalidBody => 20,
        LifecyclePhase::Serve => 21,
        LifecyclePhase::ProducerTurn => 22,
    }
}
fn decode_phase(code: u16) -> Option<LifecyclePhase> {
    Some(match code {
        1 => LifecyclePhase::Proposal,
        2 => LifecyclePhase::Prepare,
        3 => LifecyclePhase::Commit,
        4 => LifecyclePhase::Timeout,
        5 => LifecyclePhase::Fetch,
        6 => LifecyclePhase::Store,
        7 => LifecyclePhase::Validate,
        8 => LifecyclePhase::Apply,
        9 => LifecyclePhase::BroadcastProposal,
        10 => LifecyclePhase::BroadcastPrepareVote,
        11 => LifecyclePhase::BroadcastCommitVote,
        12 => LifecyclePhase::BroadcastPrepareQc,
        13 => LifecyclePhase::BroadcastCommitQc,
        14 => LifecyclePhase::BroadcastTimeoutVote,
        15 => LifecyclePhase::BroadcastTc,
        16 => LifecyclePhase::EnterView,
        17 => LifecyclePhase::DiagnosticProposalEquivocation,
        18 => LifecyclePhase::DiagnosticVoteEquivocation,
        19 => LifecyclePhase::DiagnosticTimeoutEquivocation,
        20 => LifecyclePhase::DiagnosticInvalidBody,
        21 => LifecyclePhase::Serve,
        22 => LifecyclePhase::ProducerTurn,
        _ => return None,
    })
}
fn work_class_code(work_class: LifecycleWorkClass) -> u16 {
    match work_class {
        LifecycleWorkClass::SignProposal => 1,
        LifecycleWorkClass::SignVote => 2,
        LifecycleWorkClass::SignTimeout => 3,
        LifecycleWorkClass::Fetch => 4,
        LifecycleWorkClass::Store => 5,
        LifecycleWorkClass::Validate => 6,
        LifecycleWorkClass::Apply => 7,
        LifecycleWorkClass::Broadcast => 8,
        LifecycleWorkClass::EnterView => 9,
        LifecycleWorkClass::EquivocationReport => 10,
        LifecycleWorkClass::InvalidBodyReport => 11,
        LifecycleWorkClass::CertifiedServe => 12,
        LifecycleWorkClass::ProducerTurn => 13,
    }
}
fn decode_work_class(code: u16) -> Option<LifecycleWorkClass> {
    Some(match code {
        1 => LifecycleWorkClass::SignProposal,
        2 => LifecycleWorkClass::SignVote,
        3 => LifecycleWorkClass::SignTimeout,
        4 => LifecycleWorkClass::Fetch,
        5 => LifecycleWorkClass::Store,
        6 => LifecycleWorkClass::Validate,
        7 => LifecycleWorkClass::Apply,
        8 => LifecycleWorkClass::Broadcast,
        9 => LifecycleWorkClass::EnterView,
        10 => LifecycleWorkClass::EquivocationReport,
        11 => LifecycleWorkClass::InvalidBodyReport,
        12 => LifecycleWorkClass::CertifiedServe,
        13 => LifecycleWorkClass::ProducerTurn,
        _ => return None,
    })
}
fn stage_kind_code(kind: LifecycleStageKind) -> u16 {
    match kind {
        LifecycleStageKind::SignProposal => 1,
        LifecycleStageKind::SignPrepareVote => 2,
        LifecycleStageKind::SignCommitVote => 3,
        LifecycleStageKind::SignTimeoutVote => 4,
        LifecycleStageKind::FetchBody => 5,
        LifecycleStageKind::StoreBody => 6,
        LifecycleStageKind::ValidateBody => 7,
        LifecycleStageKind::ApplyDecision => 8,
        LifecycleStageKind::BroadcastProposal => 9,
        LifecycleStageKind::BroadcastPrepareVote => 10,
        LifecycleStageKind::BroadcastCommitVote => 11,
        LifecycleStageKind::BroadcastPrepareQc => 12,
        LifecycleStageKind::BroadcastCommitQc => 13,
        LifecycleStageKind::BroadcastTimeoutVote => 14,
        LifecycleStageKind::BroadcastTc => 15,
        LifecycleStageKind::EnterView => 16,
        LifecycleStageKind::ReportProposalEquivocation => 17,
        LifecycleStageKind::ReportVoteEquivocation => 18,
        LifecycleStageKind::ReportTimeoutEquivocation => 19,
        LifecycleStageKind::ReportInvalidBody => 20,
        LifecycleStageKind::CertifiedServe => 21,
        LifecycleStageKind::ProducerTurn => 22,
    }
}
fn decode_stage_kind(code: u16) -> Option<LifecycleStageKind> {
    Some(match code {
        1 => LifecycleStageKind::SignProposal,
        2 => LifecycleStageKind::SignPrepareVote,
        3 => LifecycleStageKind::SignCommitVote,
        4 => LifecycleStageKind::SignTimeoutVote,
        5 => LifecycleStageKind::FetchBody,
        6 => LifecycleStageKind::StoreBody,
        7 => LifecycleStageKind::ValidateBody,
        8 => LifecycleStageKind::ApplyDecision,
        9 => LifecycleStageKind::BroadcastProposal,
        10 => LifecycleStageKind::BroadcastPrepareVote,
        11 => LifecycleStageKind::BroadcastCommitVote,
        12 => LifecycleStageKind::BroadcastPrepareQc,
        13 => LifecycleStageKind::BroadcastCommitQc,
        14 => LifecycleStageKind::BroadcastTimeoutVote,
        15 => LifecycleStageKind::BroadcastTc,
        16 => LifecycleStageKind::EnterView,
        17 => LifecycleStageKind::ReportProposalEquivocation,
        18 => LifecycleStageKind::ReportVoteEquivocation,
        19 => LifecycleStageKind::ReportTimeoutEquivocation,
        20 => LifecycleStageKind::ReportInvalidBody,
        21 => LifecycleStageKind::CertifiedServe,
        22 => LifecycleStageKind::ProducerTurn,
        _ => return None,
    })
}
const fn predecessor_code(scope: PredecessorScope) -> u8 {
    match scope {
        PredecessorScope::Independent => 0,
        PredecessorScope::ReadyOrdinalPrefix => 1,
        PredecessorScope::ProducerHandoffBarrier => 2,
    }
}
const fn decode_predecessor(code: u8) -> Option<PredecessorScope> {
    match code {
        0 => Some(PredecessorScope::Independent),
        1 => Some(PredecessorScope::ReadyOrdinalPrefix),
        2 => Some(PredecessorScope::ProducerHandoffBarrier),
        _ => None,
    }
}
/// Substitute one structurally valid but foreign control replay authority in a test frame.
#[cfg(test)]
pub(crate) fn substitute_recovered_control_replay_authority_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let controls = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = controls.as_slice() else {
        return false;
    };
    let Some(foreign) = ledger.records[*index]
        .replay_authority
        .with_foreign_origin_generation_for_test()
    else {
        return false;
    };
    ledger.records[*index].replay_authority = foreign;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
/// Substitute a structurally valid foreign replay origin on the WAL Decision Fetch row.
#[cfg(test)]
pub(crate) fn substitute_recovered_decision_fetch_replay_authority_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let fetches = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.durable_payload() == Some(DurablePayloadReference::None))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = fetches.as_slice() else {
        return false;
    };
    let Some(foreign) = ledger.records[*index]
        .replay_authority
        .with_foreign_origin_generation_for_test()
    else {
        return false;
    };
    ledger.records[*index].replay_authority = foreign;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
/// Substitute a valid foreign owner while retaining the exact Decision Fetch key.
#[cfg(test)]
pub(crate) fn substitute_recovered_decision_fetch_owner_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let fetches = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.durable_payload() == Some(DurablePayloadReference::None))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = fetches.as_slice() else {
        return false;
    };
    let ordinal = ledger.records[*index].ordinal;
    let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xDF; 32])), ordinal);
    ledger.records[*index].causal_root = *owner.causal_root().digest().as_bytes();
    ledger.records[*index].owner_first_ordinal = owner.first_admission_ordinal();
    ledger.records[*index].reconstruction_source = *owner.causal_root().digest().as_bytes();
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
/// Append a valid foreign terminal row which aliases the control row's owner.
#[cfg(test)]
pub(crate) fn append_same_owner_foreign_terminal_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let controls = ledger
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            )
        })
        .collect::<Vec<_>>();
    let [control] = controls.as_slice() else {
        return false;
    };
    let owner = control.owner();
    let Some(ordinal) = ledger.high_water.checked_add(1) else {
        return false;
    };
    let foreign = super::replay_authority::exact_record_fixture(
        context,
        LifecycleStageKind::ReportProposalEquivocation,
        0x7F,
    );
    let Ok(terminal) = LifecycleLedgerRecordV1::new(
        foreign.key,
        owner,
        ordinal,
        foreign.work_class,
        foreign.stage,
        Some(TerminalOutcome::Cancelled),
        owner.causal_root().digest(),
        foreign.payload,
        foreign.authority,
        DurableContinuation::None,
    ) else {
        return false;
    };
    ledger.records.push(terminal);
    ledger.high_water = ordinal;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
