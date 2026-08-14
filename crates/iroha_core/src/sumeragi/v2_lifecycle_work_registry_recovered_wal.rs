/// Detached, move-only registry authority for the exact recovered Validate
/// parent of one WAL-ahead phase vote.
///
/// Construction consumes only a fully prepared Ready/validated completion.
/// The concrete carrier remains private and is restored at its exact address
/// if this cut is dropped before the recovered vote is joined.
#[must_use = "a recovered WAL Validate cut must be joined or restored"]
pub(crate) struct RecoveredWalValidateRegistryCut<'registry> {
    registry: Option<&'registry mut ConcreteLifecycleWorkRegistry>,
    address: ConcreteWorkAddress,
    work: Option<ConcreteLifecycleWork>,
}

/// Opaque exact LedgerV1 store/frame retained by recovered-parent startup.
///
/// Neither the store nor decoded records can be extracted. The later fsync
/// transaction must consume this value together with the authenticated parent
/// repair, preserving the exact opened snapshot across the crash splice.
#[allow(dead_code)]
#[must_use = "an opened recovered WAL ledger must remain sealed through persistence"]
pub(crate) struct OpenedRecoveredWalValidateLedger {
    store: super::ledger::LifecycleLedgerStoreV1,
    opened: super::ledger::LifecycleLedgerV1,
}

/// Exact post-fsync LedgerV1 frame beside its uninstalled recovered Sign.
#[must_use = "the fsynced recovered WAL repair must install its Sign child"]
pub(crate) struct PersistedRecoveredWalValidateLedger<'registry> {
    store: super::ledger::LifecycleLedgerStoreV1,
    repaired: super::ledger::LifecycleLedgerV1,
    authority: PersistedRecoveredWalLifecycleAuthority<'registry>,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
enum PersistedRecoveredWalLifecycleAuthority<'registry> {
    Sign(DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>),
    SignedBroadcast(DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>),
    SignedBroadcastAndNextVote {
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        pair: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    },
}

/// Exact repaired storage and installed Sign retained under one registry borrow.
#[must_use = "the installed recovered WAL storage must complete lifecycle open"]
pub(crate) struct InstalledRecoveredWalSignStorage<'registry> {
    store: super::ledger::LifecycleLedgerStoreV1,
    repaired: super::ledger::LifecycleLedgerV1,
    installed: InstalledRecoveredWalSignRegistryCut<'registry>,
}

/// Typed fail-stop classification for the final exact-store recovery join.
#[derive(Debug, Error)]
#[error("{kind}")]
#[must_use = "failed recovered WAL storage completion requires restart"]
pub(crate) struct ProductionRecoveredWalStorageError {
    kind: ProductionRecoveredWalStorageErrorKind,
}

#[derive(Debug, Error)]
#[allow(variant_size_differences)]
enum ProductionRecoveredWalStorageErrorKind {
    #[error("repaired lifecycle ledger changed before unified open")]
    StaleLedger,
    #[error("durable Ready-Fetch recovery failed after WAL repair: {0}")]
    Fetch(#[source] super::ledger::DurableCertifiedFetchRecoveryError),
    #[error("unified lifecycle storage census is inconsistent: {0}")]
    Recovery(#[source] super::open::LifecycleRecoveryAssemblyError),
    #[error("recovered Sign and Ready-Fetch registry carriers conflict")]
    Registry,
    #[error("exact recovered lifecycle open failed: {0}")]
    Open(&'static str),
}

impl ProductionRecoveredWalStorageError {
    fn new(kind: ProductionRecoveredWalStorageErrorKind) -> Self {
        Self { kind }
    }

    /// Return a stable diagnostic without exposing retained startup authority.
    pub(crate) fn reason(&self) -> &'static str {
        match &self.kind {
            ProductionRecoveredWalStorageErrorKind::StaleLedger => {
                "repaired lifecycle ledger changed before unified open"
            }
            ProductionRecoveredWalStorageErrorKind::Fetch(_) => {
                "durable Ready-Fetch recovery failed after WAL repair"
            }
            ProductionRecoveredWalStorageErrorKind::Recovery(_) => {
                "unified lifecycle storage census is inconsistent"
            }
            ProductionRecoveredWalStorageErrorKind::Registry => {
                "recovered Sign and Ready-Fetch registry carriers conflict"
            }
            ProductionRecoveredWalStorageErrorKind::Open(reason) => reason,
        }
    }
}

/// Fail-stop error retaining the exact opened frame beside a failed fsync splice.
#[must_use = "failed exact-store recovered WAL persistence requires restart"]
pub(crate) struct ExactStoreRecoveredWalPersistError<'registry> {
    _ledger: OpenedRecoveredWalValidateLedger,
    error: RecoveredWalValidateLedgerPersistError<'registry>,
}

impl ExactStoreRecoveredWalPersistError<'_> {
    /// Return a stable diagnostic without exposing storage or repair authority.
    pub(crate) const fn reason(&self) -> &'static str {
        self.error.reason()
    }
}

/// Fail-stop error retaining repaired storage beside an uninstalled Sign.
#[must_use = "failed exact-store recovered Sign installation requires restart"]
pub(crate) struct ExactStoreRecoveredWalSignInstallError<'registry> {
    _store: super::ledger::LifecycleLedgerStoreV1,
    _repaired: super::ledger::LifecycleLedgerV1,
    error: RecoveredWalSignInstallError<'registry>,
}

impl ExactStoreRecoveredWalSignInstallError<'_> {
    /// Return a stable diagnostic without exposing storage or registry authority.
    pub(crate) const fn reason(&self) -> &'static str {
        self.error.reason()
    }
}

impl OpenedRecoveredWalValidateLedger {
    /// Fsync the authenticated repair only against this retained store/frame pair.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_recovered_wal_repair<'registry>(
        self,
        verified: &VerifiedHeightContext,
        repair: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    ) -> Result<
        PersistedRecoveredWalValidateLedger<'registry>,
        ExactStoreRecoveredWalPersistError<'registry>,
    > {
        let Self { store, opened } = self;
        if opened
            .recovered_phase_signed_broadcast_ordinals(&repair.repair)
            .is_some()
        {
            return match repair
                .authenticate_signed_broadcast_in_opened_ledger(verified, &store, &opened)
            {
                Ok(authority) => Ok(PersistedRecoveredWalValidateLedger {
                    store,
                    repaired: opened,
                    authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(authority),
                }),
                Err(error) => Err(ExactStoreRecoveredWalPersistError {
                    _ledger: Self { store, opened },
                    error,
                }),
            };
        }
        match repair.persist_in_opened_ledger(&store, &opened) {
            Ok((repaired, repair, _changed)) => Ok(PersistedRecoveredWalValidateLedger {
                store,
                repaired,
                authority: PersistedRecoveredWalLifecycleAuthority::Sign(repair),
            }),
            Err(error) => Err(ExactStoreRecoveredWalPersistError {
                _ledger: Self { store, opened },
                error,
            }),
        }
    }
}

impl<'registry> PersistedRecoveredWalValidateLedger<'registry> {
    /// Advance the cold adapter through either the exact single Broadcast or
    /// its adjacent WAL-backed Commit-Sign pair.
    ///
    /// Pair recognition is frame-bound and transaction-local; unrelated later
    /// rows do not change it. The body-store join and adapter replay happen
    /// before the authority variant changes, and the exact store is reloaded
    /// once more before this method releases the prepared startup.
    pub(in crate::sumeragi) fn prepare_cold_adapter_startup(
        self,
        verified: &VerifiedHeightContext,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        body_store: &V2BodyStore,
    ) -> Result<
        (
            crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
            Self,
        ),
        &'static str,
    > {
        let Self {
            store,
            repaired,
            authority,
        } = self;
        let repair = match authority {
            PersistedRecoveredWalLifecycleAuthority::Sign(repair) => {
                return Ok((
                    startup,
                    Self {
                        store,
                        repaired,
                        authority: PersistedRecoveredWalLifecycleAuthority::Sign(repair),
                    },
                ));
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair) => repair,
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote { .. } => {
                return Err("recovered phase cold adapter pair was prepared twice");
            }
        };
        let (observed_broadcast, validate_ordinal, sign_ordinal, broadcast_ordinal) = repaired
            .authenticate_recovered_phase_signed_broadcast(verified, &repair.repair)
            .map_err(|_| "recovered phase Broadcast changed before cold adapter preparation")?;
        if !observed_broadcast.exactly_matches(&repair.broadcast) {
            return Err("recovered phase Broadcast projection changed after ledger authentication");
        }
        let mut matching = repaired
            .recovered_lifecycle_signed_broadcast_and_sign_pairs()
            .map_err(|_| "recovered phase Broadcast-and-Sign pair classification failed")?
            .into_iter()
            .filter(|pair| {
                pair.parent()
                    == super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                        validate_ordinal,
                    }
                    && pair.parent_ordinal() == sign_ordinal
                    && pair.broadcast_ordinal() == broadcast_ordinal
            });
        let pair_hint = matching.next();
        if matching.next().is_some() {
            return Err("recovered phase Broadcast matched multiple durable successor pairs");
        }
        let Some(pair_hint) = pair_hint else {
            let adapter_authority = repair
                .repair
                .project_cold_adapter_authority(verified, &repair.broadcast)
                .ok_or("recovered phase Broadcast cannot replay the exact cold adapter")?;
            let startup = startup
                .advance_recovered_lifecycle_signed_broadcast(verified, adapter_authority)?;
            return Ok((
                startup,
                Self {
                    store,
                    repaired,
                    authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair),
                },
            ));
        };

        let mut preview = repair.repair.prepare_cold_signed_broadcast_and_sign(
            verified,
            startup,
            &repair.broadcast,
        )?;
        let body = body_store
            .authenticate_recovered_lifecycle_next_vote_body(&mut preview)
            .map_err(|_| "recovered phase next Vote lost its exact body-store authority")?;
        let seal = preview
            .seal_recovered_lifecycle_next_wal_vote(body)
            .map_err(|_| "recovered phase next Vote lost its WAL/body seal")?;
        let (startup, mut combined) = repair
            .repair
            .project_authenticated_cold_signed_broadcast_and_sign(verified, seal)
            .ok_or("recovered phase cold pair changed its WAL/body authority")?;
        let pair = repaired
            .authenticate_recovered_phase_signed_broadcast_and_sign(
                verified,
                &repair.repair,
                &combined,
            )
            .map_err(|_| "recovered phase cold pair changed its exact durable rows")?;
        if pair != pair_hint {
            return Err("recovered phase cold pair changed after executable projection");
        }
        let adapter_authority = combined
            .project_cold_adapter_replay_authority(verified)
            .ok_or("recovered phase cold pair cannot advance the exact adapter")?;
        let startup = startup
            .advance_recovered_lifecycle_signed_broadcast_and_sign(verified, adapter_authority)?;
        if !store.revalidates_recovered_phase_signed_broadcast_and_sign(
            verified,
            &repair.repair,
            &combined,
            &pair,
        ) {
            return Err("recovered phase cold pair changed after adapter advance");
        }
        Ok((
            startup,
            Self {
                store,
                repaired,
                authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote {
                    repair,
                    combined,
                    pair,
                },
            },
        ))
    }

    /// Install the exact recovered Sign without reopening or substituting storage.
    #[allow(clippy::result_large_err)]
    pub(crate) fn install_recovered_wal_sign(
        self,
    ) -> Result<
        InstalledRecoveredWalSignStorage<'registry>,
        ExactStoreRecoveredWalSignInstallError<'registry>,
    > {
        let Self {
            store,
            repaired,
            authority,
        } = self;
        let installed = match authority {
            PersistedRecoveredWalLifecycleAuthority::Sign(repair) => {
                repair.install_recovered_sign(&store)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair) => {
                repair.install_recovered_broadcast(&store)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote {
                repair,
                combined,
                pair,
            } => repair.install_recovered_broadcast_and_next_vote(&store, combined, pair),
        };
        match installed {
            Ok(installed) => Ok(InstalledRecoveredWalSignStorage {
                store,
                repaired,
                installed,
            }),
            Err(error) => Err(ExactStoreRecoveredWalSignInstallError {
                _store: store,
                _repaired: repaired,
                error,
            }),
        }
    }
}

impl<'registry> InstalledRecoveredWalSignStorage<'registry> {
    /// Complete the final-frame Fetch/Serve/Validate census and exact coordinator open.
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_production_lifecycle(
        self,
        verified: &VerifiedHeightContext,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        body_store: &mut V2BodyStore,
        payload_store: &mut CertifiedServePayloadStoreV1,
        serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<
        ProductionOpenedRecoveredWalSignLifecycleCut<'registry>,
        ProductionRecoveredWalStorageError,
    > {
        let body_store_identity = body_store.instance_identity();
        let payload_store_identity = payload_store.instance_identity();
        let Self {
            store,
            repaired,
            installed,
        } = self;
        if !store.load().is_ok_and(|loaded| loaded == repaired) {
            return Err(ProductionRecoveredWalStorageError::new(
                ProductionRecoveredWalStorageErrorKind::StaleLedger,
            ));
        }
        let projection = installed.authenticated_projection().ok_or_else(|| {
            ProductionRecoveredWalStorageError::new(
                ProductionRecoveredWalStorageErrorKind::Registry,
            )
        })?;
        let fetches = repaired
            .authenticate_durable_certified_fetch_startup(verified, body_store)
            .map_err(|error| {
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Fetch(error),
                )
            })?;
        let recovery = if let Some((broadcast, next_sign, pair)) =
            installed.phase_broadcast_and_next_vote_projection()
        {
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_durable_fetch_startup(
                repaired,
                serve_payloads,
                body_store,
                &projection,
                pair,
                broadcast,
                next_sign,
                fetches,
            )
        } else if let Some(broadcast) = installed.phase_broadcast_projection() {
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup(
                repaired,
                serve_payloads,
                body_store,
                &projection,
                broadcast,
                fetches,
            )
        } else {
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign_and_durable_fetch_startup(
                repaired,
                serve_payloads,
                body_store,
                &projection,
                fetches,
            )
        };
        let (recovery, fetches) = recovery.map_err(|error| {
            ProductionRecoveredWalStorageError::new(
                ProductionRecoveredWalStorageErrorKind::Recovery(error),
            )
        })?;
        fetches
            .install_alongside_recovered_wal_authority(&mut *installed.registry)
            .map_err(|_fetches| {
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Registry,
                )
            })?;
        let authority =
            authority::production_authority(verified, config, reply_route_source_capacity).ok_or(
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Open(
                        "verified height cannot derive recovered lifecycle authority",
                    ),
                ),
            )?;
        installed
            .open_with_exact_store_authority(authority, store, payload_store, recovery)
            .map_err(|error| {
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Open(error.reason()),
                )
            })
            .map(|opened| ProductionOpenedRecoveredWalSignLifecycleCut {
                opened,
                verified: verified.clone(),
                body_store_identity,
                payload_store_identity,
            })
    }
}

/// Opaque failure from storage-authenticated recovered-parent reconstruction.
///
/// Every variant owns the WAL or successor authority still in flight, the
/// exact opened ledger when one exists, and the detached body marker until it
/// has transferred into a sealed validation outcome. Dropping any pre-join
/// failure restores that marker to the same body-store instance.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered-parent reconstruction still owns startup authority"]
pub(crate) struct RecoveredWalParentFactoryError<'body> {
    failure: RecoveredWalParentFactoryFailure<'body>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalParentFactoryFailure<'body> {
    LedgerOpen {
        _error: super::ledger::LifecycleLedgerError,
        _recovered: RecoveredWalVoteSign,
    },
    BodyMarker {
        _error: RecoveredValidatedBodyCutError,
        _ledger: OpenedRecoveredWalValidateLedger,
        _recovered: RecoveredWalVoteSign,
    },
    LedgerParent {
        _ledger: OpenedRecoveredWalValidateLedger,
        _body: RecoveredValidatedBodyCut<'body>,
        _recovered: RecoveredWalVoteSign,
    },
    RuntimeParent {
        _ledger: OpenedRecoveredWalValidateLedger,
        _body: RecoveredValidatedBodyCut<'body>,
        _recovered: RecoveredWalVoteSign,
    },
    Lifecycle {
        _ledger: OpenedRecoveredWalValidateLedger,
        _body: RecoveredValidatedBodyCut<'body>,
        _error: RecoveredWalVoteLifecycleRepairError,
    },
    RegistryParent {
        _ledger: OpenedRecoveredWalValidateLedger,
        _repair: AuthenticatedWalVoteLifecycleRepair,
        _body: RecoveredValidatedBodyCut<'body>,
    },
}

#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredWalParentFactoryError<'_> {
    /// Return a stable diagnostic without exposing any retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalParentFactoryFailure::LedgerOpen { .. } => {
                "recovered WAL lifecycle ledger could not be opened"
            }
            RecoveredWalParentFactoryFailure::BodyMarker { .. } => {
                "recovered WAL vote has no exact revalidated body marker"
            }
            RecoveredWalParentFactoryFailure::LedgerParent { .. } => {
                "recovered WAL vote has no exact durable Validate parent"
            }
            RecoveredWalParentFactoryFailure::RuntimeParent { .. } => {
                "durable Validate parent could not reconstruct its runtime binding"
            }
            RecoveredWalParentFactoryFailure::Lifecycle { .. } => {
                "recovered Validate-to-Sign lifecycle projection failed"
            }
            RecoveredWalParentFactoryFailure::RegistryParent { .. } => {
                "recovered Validate parent conflicts with concrete registry state"
            }
        }
    }
}

/// Exclusive reservation for the detached Validate address and its projected
/// Sign successor address.
///
/// The parent address is vacant from the registry detach onward. The child
/// address is filled only by the pure LedgerV1 staging preflight and must also
/// be vacant before fsync. Retaining the exclusive registry borrow prevents a
/// concurrent concrete admission from invalidating either check.
struct RecoveredWalValidateRegistryReservation<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    parent_address: ConcreteWorkAddress,
    child: Option<(ConcreteWorkAddress, LifecycleDigest)>,
}

/// Fail-stop live use of the recovered-WAL detached-parent reservation.
///
/// The exact validated parent is retained but can no longer restore itself.
/// Its child address is bound before LedgerV1 fsync. After fsync the sole
/// operation inserts prechecked ordinary Sign work at that reserved address;
/// no fallible check or allocation-dependent staging remains.
#[must_use = "a live Validate-to-Sign registry reservation has not been published"]
pub(in crate::sumeragi) struct LiveValidateSignRegistryReservation<'registry> {
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
    _detached_parent: ConcreteLifecycleWork,
}

struct DetachedRecoveredValidateCompletion {
    address: ConcreteWorkAddress,
    installed_digest: LifecycleDigest,
    incumbent_address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    replay_evidence: DetachedValidateReplayEvidenceV1,
    outcome: DurableBodyValidationOutcome,
}

/// Exact provenance retained while a Validate completion is detached.
///
/// A live authenticated carrier moves its certified or remote-Proposal replay
/// family into this cut.
/// Cold WAL recovery instead consumes the separately authenticated body-store
/// marker after LedgerV1 and store equality have already been re-established;
/// it cannot manufacture the absent transport origin because this tranche
/// intentionally adds no replay field to LedgerV1.
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum DetachedValidateReplayEvidenceV1 {
    Retained(DurableValidateReplayEvidenceV1),
    RecoveredBodyMarker(DurableBodyReceipt),
}

impl DetachedValidateReplayEvidenceV1 {
    fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        match self {
            Self::Retained(evidence) => evidence.exactly_matches_durable_body(receipt),
            Self::RecoveredBodyMarker(recovered) => recovered == receipt,
        }
    }
}

/// Exact validated parent authority retained beside its authenticated WAL
/// lifecycle repair.
///
/// The validated-body outcome, durable receipt, original registry address,
/// both installed digests, and exclusive vacant-address reservation stay
/// opaque. The fsync/install composite must consume this value as a whole; it
/// cannot persist the logical repair while discarding the storage-authenticated
/// validation result. Any later logical or ledger failure is fail-stop/restart,
/// not an ordinary rollback which can restore the already-consumed binding.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a recovered validated WAL repair has not completed startup"]
pub(crate) struct AuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    repair: AuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl AuthenticatedRecoveredWalValidateLifecycleRepair<'_> {
    /// Revalidate the retained concrete pair and its exact durable validation.
    pub(crate) fn concrete_pair_and_validation_are_exact(&self) -> bool {
        recovered_validate_authority_is_exact(&self.repair, &self.validation, &self.reservation)
    }
}

fn recovered_validate_authority_is_exact(
    repair: &AuthenticatedWalVoteLifecycleRepair,
    validation: &DetachedRecoveredValidateCompletion,
    reservation: &RecoveredWalValidateRegistryReservation<'_>,
) -> bool {
    detached_recovered_validation_is_exact(repair, validation)
        && reservation.parent_address == validation.address
        && !reservation
            .registry
            .entries
            .contains_key(&validation.address)
}

fn detached_recovered_validation_is_exact(
    repair: &AuthenticatedWalVoteLifecycleRepair,
    validation: &DetachedRecoveredValidateCompletion,
) -> bool {
    let Some(validated) = validation.outcome.validated_receipt() else {
        return false;
    };
    let Ok((physical, universe, consumed)) = repair.parent().physical_geometry.normalized() else {
        return false;
    };
    ConcreteWorkAddress::new(
        validation.address.owner,
        validation.address.ordinal,
        validation.address.slot,
    ) == Some(validation.address)
        && validation.address == validation.incumbent_address
        && validation.address.owner.causal_root() == repair.parent().causal_root
        && physical.len() == 1
        && universe.len() == 1
        && consumed == universe
        && physical.get(&validation.address.slot) == Some(&validation.incumbent_digest)
        && &validation.durable_receipt == validated.durable()
        && validation.expected_manifest_hash == validated.durable().manifest_hash()
        && validation
            .replay_evidence
            .exactly_matches_durable_body(&validation.durable_receipt)
        && validation.installed_digest != validation.incumbent_digest
        && durable_validate_completion_digest(
            validation.incumbent_digest,
            validation.expected_manifest_hash,
            &validation.outcome,
        ) == Some(validation.installed_digest)
        && repair.concrete_pair_matches_validation(validated)
}

// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_BEGIN
/// Post-fsync authority for one exact recovered Validate-to-Sign splice.
///
/// This move-only token retains the exclusive registry reservation, the full
/// detached validation completion, and the frame-bound durable logical repair.
/// It exposes no parts or receipt extraction. The next startup tranche must
/// consume it directly when installing the projected Sign child.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a durable recovered WAL repair still reserves its concrete handoff"]
pub(crate) struct DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    repair: DurableAuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
}

/// Frame-bound recovered Validate→Sign→Broadcast authority.
///
/// The durable vote repair, body-revalidated Validate completion, exact child
/// projection, and exclusive registry reservation remain inseparable. Cold
/// recovery installs only the Broadcast address while retaining the complete
/// Validate/Sign authority underneath it.
#[must_use = "a durable recovered signed Broadcast still needs registry installation"]
struct DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry> {
    repair: DurableAuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
    broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    verified: VerifiedHeightContext,
    sign_address: ConcreteWorkAddress,
    broadcast_address: ConcreteWorkAddress,
}

// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_BEGIN
/// Exclusive post-install view of one exact recovered WAL Sign child.
///
/// The complete durable authority lives in the closed registry row. This
/// token retains the registry's exclusive borrow so no caller can replace,
/// take, or execute the child before the unified startup transaction commits
/// its remaining coordinator and adapter publications.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "an installed recovered WAL Sign child still seals startup"]
pub(crate) struct InstalledRecoveredWalSignRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    parent_address: ConcreteWorkAddress,
    child_address: ConcreteWorkAddress,
    child_digest: LifecycleDigest,
    next_sign: Option<(ConcreteWorkAddress, LifecycleDigest)>,
    pair: Option<super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1>,
}

/// Exclusive installed view of one standalone recovered control Sign.
///
/// The dedicated carrier remains in the registry while this cut is alive.
/// The cut exposes no address, ordinal, digest, projection, or registry parts;
/// its only production path installs the Fetch census and opens the exact
/// coordinator/store join.
#[must_use = "the installed recovered control Sign must complete startup"]
pub(super) struct InstalledRecoveredWalControlSignRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    next_sign: Option<(ConcreteWorkAddress, LifecycleDigest)>,
    pair: Option<super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1>,
}

/// Exclusive installed view of one recovered Decision Fetch.
///
/// The dedicated carrier stays in the registry while the complete durable
/// Fetch/Serve/Producer census is installed and joined to the coordinator.
/// No effect, pending binding, locator, or candidate can be extracted.
#[must_use = "the installed recovered Decision Fetch must complete startup"]
pub(super) struct InstalledRecoveredWalDecisionFetchRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}

/// Exclusive installed view of one recovered Decision Apply.
///
/// The closed carrier retains the original WAL Fetch and every body-backed
/// successor. This cut exposes no address, candidate, effect, pending binding,
/// or registry parts; it can only finish the exact prospective coordinator
/// publication assembled from the same four-row ledger lineage.
#[must_use = "the installed recovered Decision Apply must complete startup"]
pub(super) struct InstalledRecoveredDecisionApplyRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}

/// Fail-stop diagnostic for a rejected recovered control carrier install.
#[must_use = "failed recovered control installation requires restart"]
pub(super) struct RecoveredWalControlSignInstallError {
    failure: RecoveredWalControlSignInstallFailure,
}

#[allow(variant_size_differences)]
enum RecoveredWalControlSignInstallFailure {
    Projection {
        _projection: AuthenticatedRecoveredWalControlProjection,
    },
    Carrier {
        _carrier: DurableRecoveredWalControlSignCarrierV1,
    },
    BroadcastProjection {
        _projection: AuthenticatedRecoveredWalControlProjection,
        _broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    },
    BroadcastCarrier {
        _parent: DurableRecoveredWalControlSignCarrierV1,
        _broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    },
    BroadcastAndSignProjection {
        _projection: AuthenticatedRecoveredWalControlProjection,
        _combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    },
}

impl RecoveredWalControlSignInstallError {
    /// Return a stable diagnostic without exposing retained authority.
    pub(super) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalControlSignInstallFailure::Projection { .. } => {
                "recovered control Sign failed exact registry preflight"
            }
            RecoveredWalControlSignInstallFailure::Carrier { .. } => {
                "recovered control Sign carrier disagrees with durable storage"
            }
            RecoveredWalControlSignInstallFailure::BroadcastProjection { .. } => {
                "recovered control Broadcast failed exact registry preflight"
            }
            RecoveredWalControlSignInstallFailure::BroadcastCarrier { .. } => {
                "recovered control Broadcast carrier disagrees with durable storage"
            }
            RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection { .. } => {
                "recovered control Broadcast-and-Sign failed exact registry preflight"
            }
        }
    }
}

/// Fail-stop diagnostic from the installed control-carrier coordinator join.
#[must_use = "failed recovered control lifecycle open requires restart"]
pub(super) struct RecoveredWalControlSignLifecycleOpenError {
    reason: &'static str,
}

impl RecoveredWalControlSignLifecycleOpenError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Fail-stop diagnostic for a rejected recovered Decision Fetch install.
#[must_use = "failed recovered Decision Fetch installation requires restart"]
pub(super) struct RecoveredWalDecisionFetchInstallError {
    failure: RecoveredWalDecisionFetchInstallFailure,
}

#[allow(variant_size_differences)]
enum RecoveredWalDecisionFetchInstallFailure {
    Projection {
        _projection: AuthenticatedRecoveredWalDecisionFetchProjection,
    },
    Carrier {
        _carrier: DurableRecoveredWalDecisionFetchCarrierV1,
    },
    StoreProjection {
        _fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        _store: RecoveredDecisionFetchStoreProjectionV1,
    },
    StoreCarrier {
        _fetch: DurableRecoveredWalDecisionFetchCarrierV1,
        _store: RecoveredDecisionFetchStoreProjectionV1,
    },
}

impl RecoveredWalDecisionFetchInstallError {
    /// Return a stable diagnostic without exposing retained authority.
    pub(super) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalDecisionFetchInstallFailure::Projection { .. } => {
                "recovered Decision Fetch failed exact registry preflight"
            }
            RecoveredWalDecisionFetchInstallFailure::Carrier { .. } => {
                "recovered Decision Fetch carrier disagrees with durable storage"
            }
            RecoveredWalDecisionFetchInstallFailure::StoreProjection { .. } => {
                "recovered Decision Store failed exact registry preflight"
            }
            RecoveredWalDecisionFetchInstallFailure::StoreCarrier { .. } => {
                "recovered Decision Store carrier disagrees with durable storage"
            }
        }
    }
}

/// Fail-stop diagnostic from the installed Decision-Fetch coordinator join.
#[must_use = "failed recovered Decision Fetch lifecycle open requires restart"]
pub(super) struct RecoveredWalDecisionFetchLifecycleOpenError {
    reason: &'static str,
}

impl RecoveredWalDecisionFetchLifecycleOpenError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Fail-stop diagnostic for a rejected recovered Decision Apply install.
#[must_use = "failed recovered Decision Apply installation requires restart"]
pub(super) struct RecoveredDecisionApplyInstallError {
    reason: &'static str,
    _authority: RecoveredDecisionApplyInstallAuthority,
}

#[allow(variant_size_differences)]
enum RecoveredDecisionApplyInstallAuthority {
    Projection {
        _projection: RecoveredDecisionApplyStagedStorageV1,
        _effects: Vec<AdapterEffect>,
    },
    Carrier {
        _adapter: ProductionLifecycleAdapterStartupV1,
        _carrier: RecoveredDecisionApplyRegistryCarrierV1,
    },
}

impl RecoveredDecisionApplyInstallError {
    fn projection(
        reason: &'static str,
        projection: RecoveredDecisionApplyStagedStorageV1,
        effects: Vec<AdapterEffect>,
    ) -> Self {
        Self {
            reason,
            _authority: RecoveredDecisionApplyInstallAuthority::Projection {
                _projection: projection,
                _effects: effects,
            },
        }
    }

    fn carrier(
        reason: &'static str,
        adapter: ProductionLifecycleAdapterStartupV1,
        carrier: RecoveredDecisionApplyRegistryCarrierV1,
    ) -> Self {
        Self {
            reason,
            _authority: RecoveredDecisionApplyInstallAuthority::Carrier {
                _adapter: adapter,
                _carrier: carrier,
            },
        }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Fail-stop diagnostic from the installed Decision-Apply coordinator join.
#[must_use = "failed recovered Decision Apply lifecycle open requires restart"]
pub(super) struct RecoveredDecisionApplyLifecycleOpenError {
    reason: &'static str,
}

impl RecoveredDecisionApplyLifecycleOpenError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Opaque fail-stop error from post-fsync recovered Sign installation.
///
/// Every variant owns the complete uninstalled durable repair and exclusive
/// registry reservation. It exposes diagnostics only, so a failed store/frame
/// check cannot leak raw effect, pending, receipt, or retry authority.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered Sign installation still owns startup authority"]
pub(crate) struct RecoveredWalSignInstallError<'registry> {
    failure: RecoveredWalSignInstallFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalSignInstallFailure<'registry> {
    InvalidPreflight {
        _authority: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    #[cfg(test)]
    StoreOpen {
        _error: super::ledger::LifecycleLedgerError,
        _authority: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    SignedBroadcast {
        _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
    },
    SignedBroadcastAndNextVote {
        _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        _combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    },
}

impl RecoveredWalSignInstallError<'_> {
    /// Return a stable diagnostic without releasing retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalSignInstallFailure::InvalidPreflight { .. } => {
                "fsynced recovered WAL Sign child failed exact registry preflight"
            }
            #[cfg(test)]
            RecoveredWalSignInstallFailure::StoreOpen { .. } => {
                "recovered WAL ledger store could not be reopened for Sign installation"
            }
            RecoveredWalSignInstallFailure::SignedBroadcast { .. } => {
                "fsynced recovered phase Broadcast failed exact registry preflight"
            }
            RecoveredWalSignInstallFailure::SignedBroadcastAndNextVote { .. } => {
                "fsynced recovered phase Broadcast-and-Sign pair failed exact registry preflight"
            }
        }
    }
}

/// Opaque fail-stop error from the recovered Validate LedgerV1 fsync splice.
///
/// Every variant owns either the complete pre-fsync authority or the complete
/// post-fsync authority. Even a preflight failure is restart-only: callers
/// cannot recover the consumed effect, pending binding, validation receipt, or
/// registry borrow and cannot present the failure as an ordinary rollback.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered WAL persistence still owns its registry reservation"]
pub(crate) struct RecoveredWalValidateLedgerPersistError<'registry> {
    failure: RecoveredWalValidateLedgerPersistFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalValidateLedgerPersistFailure<'registry> {
    InvalidAuthority {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    ParentLedgerMismatch {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    Stage {
        _error: super::ledger::LifecycleLedgerError,
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    InvalidChildAddress {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    OccupiedReservation {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    Persist {
        _error: super::ledger::LifecycleLedgerError,
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    PostFsync {
        _authority: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
}

impl RecoveredWalValidateLedgerPersistError<'_> {
    /// Return a stable diagnostic without releasing any retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalValidateLedgerPersistFailure::InvalidAuthority { .. } => {
                "recovered WAL validation authority is inconsistent"
            }
            RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch { .. } => {
                "recovered Validate address does not bind the exact opened ledger parent"
            }
            RecoveredWalValidateLedgerPersistFailure::Stage { .. } => {
                "recovered WAL ledger repair could not be staged"
            }
            RecoveredWalValidateLedgerPersistFailure::InvalidChildAddress { .. } => {
                "recovered WAL Sign child has no exact concrete address"
            }
            RecoveredWalValidateLedgerPersistFailure::OccupiedReservation { .. } => {
                "recovered WAL parent or Sign child registry address is occupied"
            }
            RecoveredWalValidateLedgerPersistFailure::Persist { .. } => {
                "recovered WAL ledger fsync did not complete authoritatively"
            }
            RecoveredWalValidateLedgerPersistFailure::PostFsync { .. } => {
                "fsynced recovered WAL repair failed its sealed postcondition"
            }
        }
    }
}

impl RecoveredWalValidateRegistryReservation<'_> {
    fn bind_child_if_vacant(
        &mut self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> bool {
        if self.registry.entries.contains_key(&self.parent_address)
            || self.registry.entries.contains_key(&address)
        {
            return false;
        }
        match self.child {
            Some(bound) => bound == (address, digest),
            None => {
                self.child = Some((address, digest));
                true
            }
        }
    }

    fn exact_vacant_pair(&self, validation: &DetachedRecoveredValidateCompletion) -> bool {
        let Some((child, _digest)) = self.child else {
            return false;
        };
        self.parent_address == validation.address
            && child != self.parent_address
            && !self.registry.entries.contains_key(&self.parent_address)
            && !self.registry.entries.contains_key(&child)
    }
}

impl LiveValidateSignRegistryReservation<'_> {
    fn bind_exact_child(&mut self, address: ConcreteWorkAddress, digest: LifecycleDigest) -> bool {
        self.reservation.bind_child_if_vacant(address, digest)
    }

    /// Install prechecked ordinary Sign work at the already-reserved child.
    ///
    /// This is called only after exact LedgerV1 fsync. All validation and
    /// vacancy checks happened while the same exclusive registry borrow was
    /// retained, so the remaining map publication is structurally infallible.
    fn install_live_sign(self, work: ConcreteLifecycleWork) {
        let Self {
            reservation,
            _detached_parent: _,
        } = self;
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child,
        } = reservation;
        let (child_address, child_digest) =
            child.expect("pre-fsync live Sign reservation binds one exact child");
        debug_assert_ne!(parent_address, child_address);
        debug_assert!(!registry.entries.contains_key(&parent_address));
        debug_assert!(!registry.entries.contains_key(&child_address));
        debug_assert_eq!(work.digest(), child_digest);
        debug_assert!(work.validates_at(child_address));
        let std::collections::btree_map::Entry::Vacant(entry) =
            registry.entries.entry(child_address)
        else {
            unreachable!("exclusive live Sign reservation kept its child address vacant")
        };
        entry.insert(work);
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry> AuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    /// Match immutable parent identity only; the immediately following typed
    /// ledger stage is the sole authority for accepting either the live parent
    /// or the exact already-repaired parent/child stutter.
    fn ledger_parent_core_identity_is_exact(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        let candidate = self.repair.parent();
        if ledger.context().id() != candidate.key.context()
            || ledger.context().height() != candidate.key.round().height()
        {
            return false;
        }
        let mut matching = ledger
            .records()
            .iter()
            .filter(|record| record.key() == Some(candidate.key));
        let Some(parent) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        parent.owner() == self.validation.address.owner
            && parent.ordinal() == self.validation.address.ordinal
            && parent.work_class() == Some(candidate.work_class)
            && parent.stage() == Some(candidate.stage)
            && parent.reconstruction_source() == candidate.reconstruction_source
            && parent.durable_payload() == Some(candidate.payload)
    }

    fn projected_child_address(
        &self,
        child_ordinal: u128,
    ) -> Option<(ConcreteWorkAddress, LifecycleDigest)> {
        let (physical, universe, consumed) =
            self.repair.child().physical_geometry.normalized().ok()?;
        if physical.len() != 1 || universe.len() != 1 || consumed != universe {
            return None;
        }
        let (&slot, &digest) = physical.first_key_value()?;
        let address = ConcreteWorkAddress::new(self.validation.address.owner, child_ordinal, slot)?;
        (address != self.validation.address).then_some((address, digest))
    }

    /// Bind the already-fsynced three-row vote lineage to its exact store.
    #[allow(clippy::result_large_err)]
    fn authenticate_signed_broadcast_in_opened_ledger(
        self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        opened: &super::ledger::LifecycleLedgerV1,
    ) -> Result<
        DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        if !self.concrete_pair_and_validation_are_exact()
            || !self.ledger_parent_core_identity_is_exact(opened)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        }
        let (broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal) = match opened
            .authenticate_recovered_phase_signed_broadcast_repair(verified, &self.repair)
        {
            Ok(projection) => projection,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        let (physical, universe, consumed) =
            match self.repair.child().physical_geometry.normalized() {
                Ok(geometry) => geometry,
                Err(error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                            _error: super::ledger::LifecycleLedgerError::InvalidLedger(format!(
                                "recovered signed Broadcast Sign geometry is invalid: {error:?}"
                            )),
                            _authority: self,
                        },
                    });
                }
            };
        let Some((&sign_slot, &sign_digest)) = physical.first_key_value() else {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        };
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let (Some(sign_address), Some(broadcast_address)) = (
            ConcreteWorkAddress::new(self.validation.address.owner, sign_ordinal, sign_slot),
            ConcreteWorkAddress::new(
                self.validation.address.owner,
                broadcast_ordinal,
                broadcast_slot,
            ),
        ) else {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        };
        if parent_ordinal != self.validation.address.ordinal
            || sign_slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            || physical.len() != 1
            || universe.len() != 1
            || consumed != universe
            || physical.get(&sign_slot) != Some(&sign_digest)
            || !broadcast.validates_at(verified, broadcast_address, broadcast.digest())
            || self
                .reservation
                .registry
                .entries
                .keys()
                .any(|address| address.owner == self.validation.address.owner)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        }
        let Self {
            repair,
            validation,
            reservation,
        } = self;
        let repair = match store.authenticate_wal_vote_repair_for_signed_broadcast(opened, repair) {
            Ok(repair) => repair,
            Err((error, repair)) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair {
                            repair,
                            validation,
                            reservation,
                        },
                    },
                });
            }
        };
        assert_eq!(repair.child_ordinal(), sign_ordinal);
        Ok(
            DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair {
                repair,
                validation,
                reservation,
                broadcast,
                verified: verified.clone(),
                sign_address,
                broadcast_address,
            },
        )
    }

    /// Stage against the exact opened ledger, reserve the projected child, and
    /// fsync the complete replacement without exposing inner authority.
    ///
    /// The store re-loads and compares `opened` immediately before staging, so
    /// a stale snapshot fails closed. Any returned error retains this exclusive
    /// registry borrow and is fail-stop/restart, regardless of whether bytes
    /// reached disk before the failure was observed.
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_in_opened_ledger(
        mut self,
        store: &super::ledger::LifecycleLedgerStoreV1,
        opened: &super::ledger::LifecycleLedgerV1,
    ) -> Result<
        (
            super::ledger::LifecycleLedgerV1,
            DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
            bool,
        ),
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        if !self.concrete_pair_and_validation_are_exact() {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        }
        if !self.ledger_parent_core_identity_is_exact(opened) {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch {
                    _authority: self,
                },
            });
        }
        let (expected, child_ordinal, expected_changed) =
            match opened.stage_authenticated_wal_vote_repair(&self.repair) {
                Ok(staged) => staged,
                Err(error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                            _error: error,
                            _authority: self,
                        },
                    });
                }
            };
        let Some((child_address, child_digest)) = self.projected_child_address(child_ordinal)
        else {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidChildAddress {
                    _authority: self,
                },
            });
        };
        if !self
            .reservation
            .bind_child_if_vacant(child_address, child_digest)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::OccupiedReservation {
                    _authority: self,
                },
            });
        }

        let Self {
            repair,
            validation,
            reservation,
        } = self;
        let (persisted, repair, changed) =
            match store.persist_authenticated_wal_vote_repair(opened, repair) {
                Ok(persisted) => persisted,
                Err((error, repair)) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                            _error: error,
                            _authority: AuthenticatedRecoveredWalValidateLifecycleRepair {
                                repair,
                                validation,
                                reservation,
                            },
                        },
                    });
                }
            };
        let durable = DurableAuthenticatedRecoveredWalValidateLifecycleRepair {
            repair,
            validation,
            reservation,
        };
        if persisted != expected
            || changed != expected_changed
            || durable.repair.child_ordinal() != child_ordinal
            || !durable.post_fsync_authority_is_exact(store)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                    _authority: durable,
                },
            });
        }
        Ok((persisted, durable, changed))
    }
}

#[cfg(test)]
impl<'registry> AuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    fn parent_ledger_for_test(
        &self,
        owner: OwnerId,
        ordinal: u128,
    ) -> Result<super::ledger::LifecycleLedgerV1, super::ledger::LifecycleLedgerError> {
        let parent = self.repair.parent();
        super::ledger::LifecycleLedgerV1::new(
            LifecycleContext::new(parent.key.context(), parent.key.round().height()),
            ordinal,
            vec![super::ledger::LifecycleLedgerRecordV1::new(
                parent.key,
                owner,
                ordinal,
                parent.work_class,
                parent.stage,
                None,
                parent.reconstruction_source,
                parent.payload,
                parent.replay_authority.clone(),
                super::schema::DurableContinuation::None,
            )?],
            BTreeMap::new(),
        )
    }

    /// Verify that a row with the right semantic projection but the wrong
    /// durable ordinal, owner identity, or row inventory cannot pass the outer
    /// address-to-ledger binding.
    pub(crate) fn rejects_wrong_ledger_parent_bindings_for_test(&self) -> bool {
        let parent = self.repair.parent();
        let address = self.validation.address;
        let Some(other_ordinal) = address.ordinal.checked_add(1) else {
            return false;
        };
        let Ok(exact) = self.parent_ledger_for_test(address.owner, address.ordinal) else {
            return false;
        };
        let child = self.repair.child();
        let first_ordinal = address.owner.first_admission_ordinal();
        let Ok(preceding_child) = super::ledger::LifecycleLedgerRecordV1::new(
            child.key,
            address.owner,
            first_ordinal,
            child.work_class,
            child.stage,
            None,
            child.reconstruction_source,
            child.payload,
            child.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        ) else {
            return false;
        };
        let Ok(displaced_parent) = super::ledger::LifecycleLedgerRecordV1::new(
            parent.key,
            address.owner,
            other_ordinal,
            parent.work_class,
            parent.stage,
            None,
            parent.reconstruction_source,
            parent.payload,
            parent.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        ) else {
            return false;
        };
        let Ok(wrong_ordinal) = super::ledger::LifecycleLedgerV1::new(
            exact.context(),
            other_ordinal,
            vec![preceding_child, displaced_parent],
            BTreeMap::new(),
        ) else {
            return false;
        };
        let wrong_owner = OwnerId::new(parent.causal_root, other_ordinal);
        let Ok(wrong_owner) = self.parent_ledger_for_test(wrong_owner, other_ordinal) else {
            return false;
        };
        let wrong_row = super::ledger::LifecycleLedgerV1::empty(exact.context());
        self.ledger_parent_core_identity_is_exact(&exact)
            && !self.ledger_parent_core_identity_is_exact(&wrong_ordinal)
            && !self.ledger_parent_core_identity_is_exact(&wrong_owner)
            && !self.ledger_parent_core_identity_is_exact(&wrong_row)
    }

    /// Prove structurally valid replay-origin substitutions fail on both rows.
    pub(crate) fn rejects_foreign_replay_authorities_for_test(&self) -> bool {
        let address = self.validation.address;
        let Ok(seed) = self.parent_ledger_for_test(address.owner, address.ordinal) else {
            return false;
        };
        let Ok((repaired, child_ordinal, changed)) =
            seed.stage_authenticated_wal_vote_repair(&self.repair)
        else {
            return false;
        };
        let Ok((physical, _, _)) = self.repair.child().physical_geometry.normalized() else {
            return false;
        };
        let Some((&child_slot, _)) = physical.first_key_value() else {
            return false;
        };
        let Some(child_address) =
            ConcreteWorkAddress::new(address.owner, child_ordinal, child_slot)
        else {
            return false;
        };
        let projection = AuthenticatedRecoveredWalSignProjection {
            parent: self.repair.parent().clone(),
            child: self.repair.child().clone(),
            parent_address: address,
            child_address,
        };
        let context = repaired.context();
        let Some(foreign_parent) = repaired.with_foreign_replay_authority_for_test(address.ordinal)
        else {
            return false;
        };
        let Some(foreign_child) = repaired.with_foreign_replay_authority_for_test(child_ordinal)
        else {
            return false;
        };
        changed
            && projection.repaired_pair_is_exact(context, repaired.records())
            && !projection.repaired_pair_is_exact(context, foreign_parent.records())
            && !projection.repaired_pair_is_exact(context, foreign_child.records())
    }

    /// Consume the complete outer authority through one real fsync, reopen the
    /// frame, and prove that the authenticated repeat stutters exactly.
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    pub(crate) fn persist_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        (
            super::ledger::WalVoteLedgerRepairTestSummary,
            DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        ),
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        let context = LifecycleContext::new(
            self.repair.parent().key.context(),
            self.repair.parent().key.round().height(),
        );
        let seed = match self.parent_ledger_for_test(
            self.validation.address.owner,
            self.validation.address.ordinal,
        ) {
            Ok(seed) => seed,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        if !opened.records().is_empty() || opened.high_water() != 0 {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch {
                    _authority: self,
                },
            });
        }
        if let Err(error) = store.persist(&seed) {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                    _error: error,
                    _authority: self,
                },
            });
        }
        let (repaired, durable, first_changed) = self.persist_in_opened_ledger(&store, &seed)?;
        let (reopened_store, reopened) =
            match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
                Ok(reopened) => reopened,
                Err(_error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                            _authority: durable,
                        },
                    });
                }
            };
        let reopened_exact =
            reopened == repaired && durable.post_fsync_authority_is_exact(&reopened_store);
        let (repeated, child_ordinal, repeat_changed) =
            match durable.stage_repeat_for_test(&reopened) {
                Ok(repeated) => repeated,
                Err(_error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                            _authority: durable,
                        },
                    });
                }
            };
        if repeated != repaired || child_ordinal != durable.repair.child_ordinal() {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                    _authority: durable,
                },
            });
        }
        let parent_ordinal = durable.validation.address.ordinal;
        let parent = repeated
            .records()
            .iter()
            .find(|record| record.ordinal() == parent_ordinal);
        let child = repeated
            .records()
            .iter()
            .find(|record| record.ordinal() == child_ordinal);
        let repair = durable.repair.repair();
        let edge = repair.edge();
        let parent_advanced = parent.is_some_and(|record| {
            record.key() == Some(repair.parent().key)
                && record.owner() == durable.validation.address.owner
                && record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
                && record.continuation()
                    == Some(super::schema::DurableContinuation::successor(
                        edge,
                        child_ordinal,
                    ))
        });
        let child_live = child.is_some_and(|record| {
            let candidate = repair.child();
            candidate.initial_state == InitialLifecycleState::Ready
                && record.key() == Some(candidate.key)
                && record.owner() == durable.validation.address.owner
                && record.work_class() == Some(candidate.work_class)
                && record.stage() == Some(candidate.stage)
                && record.reconstruction_source() == candidate.reconstruction_source
                && record.durable_payload() == Some(candidate.payload)
                && record.terminal() == Some(None)
                && record.continuation() == Some(super::schema::DurableContinuation::None)
        });
        let summary = super::ledger::WalVoteLedgerRepairTestSummary::new(
            child_ordinal,
            edge,
            first_changed,
            repeat_changed,
            parent_advanced,
            child_live,
            repeated.high_water(),
            durable.repair.ledger_frame_hash() != LifecycleDigest::new([0_u8; 32]),
            reopened_exact,
        );
        Ok((summary, durable))
    }

    /// Exercise the outer stale-snapshot guard without releasing its sealed
    /// error authority. The exact parent snapshot is intentionally not written
    /// to the opened store before the consuming persistence call.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_stale_snapshot_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        let context = LifecycleContext::new(
            self.repair.parent().key.context(),
            self.repair.parent().key.round().height(),
        );
        let seed = match self.parent_ledger_for_test(
            self.validation.address.owner,
            self.validation.address.ordinal,
        ) {
            Ok(seed) => seed,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        if !opened.records().is_empty() || opened.high_water() != 0 {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch {
                    _authority: self,
                },
            });
        }
        self.persist_in_opened_ledger(&store, &seed)
            .map(|(_ledger, durable, _changed)| durable)
    }

    /// Reopen an existing ledger frame and consume this fresh startup
    /// authority through the same idempotent fsync seam.
    ///
    /// This models a crash after ledger publication but before registry
    /// installation. Only the exact already-repaired pair may return
    /// `changed == false`; the production staging function rejects every
    /// third parent/child shape.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_reopened_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        (
            bool,
            DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        ),
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        let context = LifecycleContext::new(
            self.repair.parent().key.context(),
            self.repair.parent().key.round().height(),
        );
        let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        self.persist_in_opened_ledger(&store, &opened)
            .map(|(_ledger, durable, changed)| (changed, durable))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry> DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    fn post_fsync_authority_is_exact(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        let repair = self.repair.repair();
        let Some((child, child_digest)) = self.reservation.child else {
            return false;
        };
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        recovered_validate_authority_is_exact(repair, &self.validation, &self.reservation)
            && self.reservation.exact_vacant_pair(&self.validation)
            && child.owner == self.validation.address.owner
            && child.owner.causal_root() == repair.child().causal_root
            && child.ordinal == self.repair.child_ordinal()
            && child.slot == effect_slot
            && self
                .reservation
                .registry
                .entries
                .keys()
                .all(|address| address.owner != child.owner)
            && repair
                .child()
                .physical_geometry
                .normalized()
                .ok()
                .is_some_and(|(physical, universe, consumed)| {
                    physical.len() == 1
                        && universe.len() == 1
                        && consumed == universe
                        && physical.get(&child.slot) == Some(&child_digest)
                })
            && store.revalidates_durable_authenticated_wal_vote_repair(&self.repair)
    }

    /// Consume the complete post-fsync authority into one exact closed Sign
    /// registry row.
    ///
    /// The current store frame, idempotent repaired-pair shape, parent/child
    /// vacancies, empty causal owner, receipt ordinal, sole Effect slot, and
    /// child digest are all checked before the single insertion. An error
    /// therefore retains the complete uninstalled authority. After insertion
    /// no fallible operation runs; the returned opaque cut keeps the registry
    /// exclusively borrowed and revalidates the exact row without exposing it.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_sign(
        self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        if !self.post_fsync_authority_is_exact(store) {
            return Err(RecoveredWalSignInstallError {
                failure: RecoveredWalSignInstallFailure::InvalidPreflight { _authority: self },
            });
        }
        let (child_address, child_digest) = self
            .reservation
            .child
            .expect("exact post-fsync authority reserves one Sign child");
        let Self {
            repair,
            validation,
            reservation,
        } = self;
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child: _,
        } = reservation;
        let work = ConcreteLifecycleWork {
            digest: child_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalSign(DurableRecoveredWalSignWork {
                repair,
                validation,
                dispatch_key: None,
            }),
        };
        debug_assert!(work.validates_at(child_address));
        let std::collections::btree_map::Entry::Vacant(entry) =
            registry.entries.entry(child_address)
        else {
            unreachable!("exclusive preflight proved the recovered Sign address vacant")
        };
        entry.insert(work);
        Ok(InstalledRecoveredWalSignRegistryCut {
            registry,
            parent_address,
            child_address,
            child_digest,
            next_sign: None,
            pair: None,
        })
    }

    #[cfg(test)]
    fn stage_repeat_for_test(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> Result<(super::ledger::LifecycleLedgerV1, u128, bool), super::ledger::LifecycleLedgerError>
    {
        ledger.stage_authenticated_wal_vote_repair(self.repair.repair())
    }

    /// Reopen the focused store and revalidate the frame-bound receipt plus
    /// both still-vacant registry reservations without exposing either one.
    #[cfg(test)]
    pub(crate) fn remains_exact_for_test(&self, root: &std::path::Path) -> bool {
        let repair = self.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        super::ledger::LifecycleLedgerStoreV1::open(root, context)
            .ok()
            .is_some_and(|(store, ledger)| {
                self.post_fsync_authority_is_exact(&store)
                    && self.stage_repeat_for_test(&ledger).ok().is_some_and(
                        |(repeated, ordinal, changed)| {
                            repeated == ledger && ordinal == self.repair.child_ordinal() && !changed
                        },
                    )
            })
    }

    /// Reopen the supplied ledger root and consume this durable authority into
    /// its exact recovered Sign row.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn install_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        let repair = self.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        let store = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok((store, _opened)) => store,
            Err(error) => {
                return Err(RecoveredWalSignInstallError {
                    failure: RecoveredWalSignInstallFailure::StoreOpen {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        self.install_recovered_sign(&store)
    }
}

impl<'registry> DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry> {
    /// Install the exact live Broadcast while retaining its complete phase-vote parent.
    #[allow(clippy::result_large_err)]
    fn install_recovered_broadcast(
        self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        let Self {
            repair,
            validation,
            reservation,
            broadcast,
            verified,
            sign_address,
            broadcast_address,
        } = self;
        let Ok(ledger) = store.load() else {
            return Err(RecoveredWalSignInstallError {
                failure: RecoveredWalSignInstallFailure::SignedBroadcast {
                    _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair {
                        repair,
                        validation,
                        reservation,
                        broadcast,
                        verified,
                        sign_address,
                        broadcast_address,
                    },
                },
            });
        };
        let exact = detached_recovered_validation_is_exact(repair.repair(), &validation)
            && repair.belongs_to_loaded(store, &ledger)
            && ledger
                .authenticate_recovered_phase_signed_broadcast(&verified, &repair)
                .is_ok_and(|(recovered, parent, sign, child)| {
                    parent == validation.address.ordinal
                        && sign == sign_address.ordinal
                        && child == broadcast_address.ordinal
                        && recovered.exactly_matches(&broadcast)
                })
            && reservation.parent_address == validation.address
            && reservation
                .registry
                .entries
                .keys()
                .all(|address| address.owner != broadcast_address.owner);
        if !exact {
            return Err(RecoveredWalSignInstallError {
                failure: RecoveredWalSignInstallFailure::SignedBroadcast {
                    _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair {
                        repair,
                        validation,
                        reservation,
                        broadcast,
                        verified,
                        sign_address,
                        broadcast_address,
                    },
                },
            });
        }
        let digest = broadcast.digest();
        let parent = DurableRecoveredWalSignWork {
            repair,
            validation,
            dispatch_key: None,
        };
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::PhaseVote(parent),
                    broadcast,
                    verified,
                    address: broadcast_address,
                    paired_next_sign: None,
                },
            ),
        };
        assert!(work.validates_at(broadcast_address));
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child: _,
        } = reservation;
        let previous = registry.entries.insert(broadcast_address, work);
        assert!(previous.is_none());
        Ok(InstalledRecoveredWalSignRegistryCut {
            registry,
            parent_address,
            child_address: broadcast_address,
            child_digest: digest,
            next_sign: None,
            pair: None,
        })
    }

    /// Install one exact phase Prepare-Broadcast plus its independent Commit Sign.
    ///
    /// The loaded LedgerV1 frame, detached Validate completion, WAL repair,
    /// combined executable projection, and both fresh child rows are joined
    /// before either process-local carrier is inserted. The Broadcast retains
    /// the historical phase parent and an explicit link to the independently
    /// owned next Sign, which remains undispatched.
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    fn install_recovered_broadcast_and_next_vote(
        self,
        store: &super::ledger::LifecycleLedgerStoreV1,
        combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        pair: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        let fail = |repair, combined| RecoveredWalSignInstallError {
            failure: RecoveredWalSignInstallFailure::SignedBroadcastAndNextVote {
                _repair: repair,
                _combined: combined,
            },
        };
        let Ok(ledger) = store.load() else {
            return Err(fail(self, combined));
        };
        let record_at = |ordinal| {
            ledger
                .records()
                .binary_search_by_key(&ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index))
        };
        let Some(broadcast_record) = record_at(pair.broadcast_ordinal()) else {
            return Err(fail(self, combined));
        };
        let Some(next_sign_record) = record_at(pair.next_sign_ordinal()) else {
            return Err(fail(self, combined));
        };
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let next_sign_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(broadcast_address) = ConcreteWorkAddress::new(
            broadcast_record.owner(),
            broadcast_record.ordinal(),
            broadcast_slot,
        ) else {
            return Err(fail(self, combined));
        };
        let Some(next_sign_address) = ConcreteWorkAddress::new(
            next_sign_record.owner(),
            next_sign_record.ordinal(),
            next_sign_slot,
        ) else {
            return Err(fail(self, combined));
        };
        let exact = pair.parent()
            == super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                validate_ordinal: self.validation.address.ordinal,
            }
            && pair.parent_ordinal() == self.sign_address.ordinal
            && pair.broadcast_ordinal() == self.broadcast_address.ordinal
            && detached_recovered_validation_is_exact(self.repair.repair(), &self.validation)
            && self.repair.belongs_to_loaded(store, &ledger)
            && ledger
                .authenticate_recovered_phase_signed_broadcast_and_sign(
                    &self.verified,
                    &self.repair,
                    &combined,
                )
                .is_ok_and(|observed| observed == pair)
            && self.reservation.parent_address == self.validation.address
            && self.reservation.registry.entries.is_empty()
            && combined.broadcast_exactly_matches(&self.broadcast)
            && combined.exactly_matches_fresh_records(
                ledger.context(),
                broadcast_record,
                next_sign_record,
            )
            && broadcast_address == self.broadcast_address
            && broadcast_address.owner == self.sign_address.owner
            && next_sign_address.owner != self.sign_address.owner;
        if !exact {
            return Err(fail(self, combined));
        }

        let Self {
            repair,
            validation,
            reservation,
            broadcast: _,
            verified,
            sign_address: _,
            broadcast_address,
        } = self;
        let (broadcast, next_sign) = combined.into_registry_children(
            RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new(),
        );
        let broadcast_digest = broadcast.digest();
        let next_sign_digest = next_sign.digest();
        let parent = DurableRecoveredWalSignWork {
            repair,
            validation,
            dispatch_key: None,
        };
        let broadcast_work = ConcreteLifecycleWork {
            digest: broadcast_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::PhaseVote(parent),
                    broadcast,
                    verified: verified.clone(),
                    address: broadcast_address,
                    paired_next_sign: Some((next_sign_address, next_sign_digest)),
                },
            ),
        };
        let next_sign_work = ConcreteLifecycleWork {
            digest: next_sign_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: next_sign,
                    verified,
                    address: next_sign_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(broadcast_work.validates_at(broadcast_address));
        assert!(next_sign_work.validates_at(next_sign_address));
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child: _,
        } = reservation;
        assert!(
            registry
                .entries
                .insert(broadcast_address, broadcast_work)
                .is_none()
        );
        assert!(
            registry
                .entries
                .insert(next_sign_address, next_sign_work)
                .is_none()
        );
        Ok(InstalledRecoveredWalSignRegistryCut {
            registry,
            parent_address,
            child_address: broadcast_address,
            child_digest: broadcast_digest,
            next_sign: Some((next_sign_address, next_sign_digest)),
            pair: Some(pair),
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl InstalledRecoveredWalSignRegistryCut<'_> {
    fn phase_broadcast_projection(&self) -> Option<&RecoveredLifecycleSignedBroadcastProjectionV1> {
        if self.next_sign.is_some() || self.pair.is_some() {
            return None;
        }
        let work = self.registry.entries.get(&self.child_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        matches!(
            &broadcast.parent,
            DurableRecoveredLifecycleSignParentV1::PhaseVote(_)
        )
        .then_some(&broadcast.broadcast)
    }

    fn phase_broadcast_and_next_vote_projection(
        &self,
    ) -> Option<(
        &RecoveredLifecycleSignedBroadcastProjectionV1,
        &RecoveredLifecycleNextWalVoteCandidateProjectionV1,
        &super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    )> {
        let (next_sign_address, next_sign_digest) = self.next_sign?;
        let pair = self.pair.as_ref()?;
        let broadcast_work = self.registry.entries.get(&self.child_address)?;
        let next_sign_work = self.registry.entries.get(&next_sign_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &broadcast_work.kind
        else {
            return None;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_sign_work.kind
        else {
            return None;
        };
        (broadcast_work.digest == self.child_digest
            && next_sign_work.digest == next_sign_digest
            && pair.broadcast_ordinal() == self.child_address.ordinal
            && pair.next_sign_ordinal() == next_sign_address.ordinal
            && matches!(
                pair.parent(),
                super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                    validate_ordinal
                } if validate_ordinal == self.parent_address.ordinal
            )
            && broadcast.paired_next_sign == Some((next_sign_address, next_sign_digest))
            && matches!(
                &broadcast.parent,
                DurableRecoveredLifecycleSignParentV1::PhaseVote(_)
            ))
        .then_some((&broadcast.broadcast, &next_sign.projection, pair))
    }

    fn installed_entry_is_exact(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self.parent_address == self.child_address
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
        {
            return false;
        }
        self.registry
            .entries
            .get(&self.child_address)
            .is_some_and(|work| {
                work.digest == self.child_digest
                    && work.validates_at(self.child_address)
                    && matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                            if sign.validates_in_store(
                                self.child_address,
                                self.child_digest,
                                store,
                            )
                    )
            })
    }

    /// Reopen the receipt's height-local store and prove the installed parent,
    /// child, owner-count, ordinal, sole Effect slot, digest, and frame binding.
    #[cfg(test)]
    pub(crate) fn exact_installed_shape_for_test(&self, root: &std::path::Path) -> bool {
        let Some(work) = self.registry.entries.get(&self.child_address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) = &work.kind else {
            return false;
        };
        let repair = sign.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        super::ledger::LifecycleLedgerStoreV1::open(root, context)
            .ok()
            .is_some_and(|(store, _opened)| self.installed_entry_is_exact(&store))
    }
}

#[cfg(test)]
impl RecoveredWalSignInstallError<'_> {
    /// Prove that this opaque error still owns the complete exact authority and
    /// both registry vacancies when checked against the original store.
    pub(crate) fn retains_exact_vacancies_for_test(&self, root: &std::path::Path) -> bool {
        let authority = match &self.failure {
            RecoveredWalSignInstallFailure::InvalidPreflight {
                _authority: authority,
            }
            | RecoveredWalSignInstallFailure::StoreOpen {
                _authority: authority,
                ..
            } => authority,
            RecoveredWalSignInstallFailure::SignedBroadcast { .. }
            | RecoveredWalSignInstallFailure::SignedBroadcastAndNextVote { .. } => return false,
        };
        let repair = authority.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        super::ledger::LifecycleLedgerStoreV1::open(root, context)
            .ok()
            .is_some_and(|(store, _opened)| authority.post_fsync_authority_is_exact(&store))
    }
}
// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_END
// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_END

// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_BEGIN
/// Opaque logical projection minted only from one exact installed Sign row.
///
/// Its fields are private, it has no constructor or parts API, and it carries
/// no effect, pending binding, body receipt, or ledger receipt. The durable
/// open module may query or splice it, but callers cannot supply substitute
/// parent/child candidates to the authenticated recovery cut.
pub(super) struct AuthenticatedRecoveredWalSignProjection {
    parent: CandidateAdmission,
    child: CandidateAdmission,
    parent_address: ConcreteWorkAddress,
    child_address: ConcreteWorkAddress,
}

impl AuthenticatedRecoveredWalSignProjection {
    /// Return whether both sealed candidates belong to one exact context.
    pub(super) fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        let Ok((physical, universe, consumed)) = self.child.physical_geometry.normalized() else {
            return false;
        };
        context.id() == self.parent.key.context()
            && context.height() == self.parent.key.round().height()
            && context.id() == self.child.key.context()
            && context.height() == self.child.key.round().height()
            && self.parent.work_class == LifecycleWorkClass::Validate
            && self.child.work_class == LifecycleWorkClass::SignVote
            && self.parent.causal_root == self.child.causal_root
            && self.parent_address.owner.causal_root() == self.parent.causal_root
            && self.child_address.owner == self.parent_address.owner
            && self.child_address.owner.causal_root() == self.child.causal_root
            && self.parent_address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            && self.parent_address.ordinal < self.child_address.ordinal
            && self.child_address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            && physical.len() == 1
            && universe.len() == 1
            && consumed == universe
            && physical.contains_key(&self.child_address.slot)
    }

    /// Return the sealed recovered Validate semantic key.
    pub(super) const fn parent_key(&self) -> LifecycleKey {
        self.parent.key
    }

    /// Return the sealed recovered Sign semantic key.
    pub(super) const fn child_key(&self) -> LifecycleKey {
        self.child.key
    }

    fn continuation_edge(&self) -> Option<super::schema::DurableContinuationEdge> {
        match (self.child.key.phase(), self.child.stage.kind()) {
            (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                Some(super::schema::DurableContinuationEdge::ValidateToSignPrepare)
            }
            (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                Some(super::schema::DurableContinuationEdge::ValidateToSignCommit)
            }
            _ => None,
        }
    }

    fn repaired_child_record_is_exact(
        &self,
        context: LifecycleContext,
        record: &super::ledger::LifecycleLedgerRecordV1,
    ) -> bool {
        self.belongs_to_context(context)
            && record.key() == Some(self.child.key)
            && record.owner() == self.child_address.owner
            && record.ordinal() == self.child_address.ordinal
            && record.work_class() == Some(self.child.work_class)
            && record.stage() == Some(self.child.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.child.reconstruction_source
            && record.durable_payload() == Some(self.child.payload)
            && record.replay_matches_candidate(&self.child)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && self.child.initial_state == InitialLifecycleState::Ready
            && self.child.producer_turn.is_none()
    }

    /// Prove that one repaired LedgerV1 frame retains both exact sides and its
    /// typed Validate→Sign edge at the installed concrete addresses.
    pub(super) fn repaired_pair_is_exact(
        &self,
        context: LifecycleContext,
        records: &[super::ledger::LifecycleLedgerRecordV1],
    ) -> bool {
        let Some(edge) = self.continuation_edge() else {
            return false;
        };
        let Some(parent) = records
            .iter()
            .find(|record| record.ordinal() == self.parent_address.ordinal)
        else {
            return false;
        };
        let Some(child) = records
            .iter()
            .find(|record| record.ordinal() == self.child_address.ordinal)
        else {
            return false;
        };
        self.repaired_child_record_is_exact(context, child)
            && parent.key() == Some(self.parent.key)
            && parent.owner() == self.parent_address.owner
            && parent.ordinal() == self.parent_address.ordinal
            && parent.work_class() == Some(self.parent.work_class)
            && parent.stage() == Some(self.parent.stage)
            && parent.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && parent.reconstruction_source() == self.parent.reconstruction_source
            && parent.durable_payload() == Some(self.parent.payload)
            && parent.replay_matches_candidate(&self.parent)
            && parent.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    edge,
                    self.child_address.ordinal,
                ))
            && self.parent.initial_state == InitialLifecycleState::Ready
            && self.parent.producer_turn.is_none()
    }

    /// Prove the same repaired parent with an Advanced Sign and live Broadcast.
    pub(super) fn signed_broadcast_chain_is_exact(
        &self,
        context: LifecycleContext,
        records: &[super::ledger::LifecycleLedgerRecordV1],
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        let Some(validate_edge) = self.continuation_edge() else {
            return false;
        };
        let Some(parent) = records
            .iter()
            .find(|record| record.ordinal() == self.parent_address.ordinal)
        else {
            return false;
        };
        let Some(sign) = records
            .iter()
            .find(|record| record.ordinal() == self.child_address.ordinal)
        else {
            return false;
        };
        let expected_broadcast_edge = match (self.child.key.phase(), self.child.stage.kind()) {
            (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                super::schema::DurableContinuationEdge::SignPrepareToBroadcast
            }
            (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                super::schema::DurableContinuationEdge::SignCommitToBroadcast
            }
            _ => return false,
        };
        let Some((observed_edge, broadcast_ordinal)) = sign
            .continuation()
            .and_then(|continuation| continuation.successor_parts())
        else {
            return false;
        };
        let Some(broadcast_record) = records
            .iter()
            .find(|record| record.ordinal() == broadcast_ordinal)
        else {
            return false;
        };
        self.belongs_to_context(context)
            && observed_edge == expected_broadcast_edge
            && parent.key() == Some(self.parent.key)
            && parent.owner() == self.parent_address.owner
            && parent.work_class() == Some(self.parent.work_class)
            && parent.stage() == Some(self.parent.stage)
            && parent.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && parent.reconstruction_source() == self.parent.reconstruction_source
            && parent.durable_payload() == Some(self.parent.payload)
            && parent.replay_matches_candidate(&self.parent)
            && parent.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    validate_edge,
                    self.child_address.ordinal,
                ))
            && sign.key() == Some(self.child.key)
            && sign.owner() == self.child_address.owner
            && sign.work_class() == Some(self.child.work_class)
            && sign.stage() == Some(self.child.stage)
            && sign.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && sign.reconstruction_source() == self.child.reconstruction_source
            && sign.durable_payload() == Some(self.child.payload)
            && sign.replay_matches_candidate(&self.child)
            && broadcast.exactly_matches_record(broadcast_record, sign.owner())
    }

    /// Install the exact opaque Sign child only when one live repaired ledger
    /// row retains its complete installed address and logical identity.
    ///
    /// This is the production reconstruction surface for the post-fsync crash
    /// cut. It accepts no caller-supplied candidate and mutates the destination
    /// only after every durable field has matched.
    pub(super) fn insert_repaired_child_from_record(
        &self,
        context: LifecycleContext,
        record: &super::ledger::LifecycleLedgerRecordV1,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if !self.repaired_child_record_is_exact(context, record)
            || candidates.contains_key(&self.parent.key)
            || candidates.contains_key(&self.child.key)
        {
            return false;
        }
        candidates.insert(self.child.key, self.child.clone());
        true
    }

    /// Atomically replace the exact parent or stutter on the exact child.
    pub(super) fn splice_candidates(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        match (
            candidates.get(&self.parent.key),
            candidates.get(&self.child.key),
        ) {
            (Some(parent), None) if parent == &self.parent => {
                let removed = candidates
                    .remove(&self.parent.key)
                    .expect("exact recovered Validate parent was preflighted");
                debug_assert_eq!(&removed, &self.parent);
                let displaced = candidates.insert(self.child.key, self.child.clone());
                debug_assert!(displaced.is_none());
                true
            }
            // A fresh startup after the ledger fsync reconstructs the already
            // live Sign child and must stutter at this logical splice.
            (None, Some(child)) if child == &self.child => true,
            // Any foreign value occupying either semantic key, both exact
            // sides at once, or neither side fails before mutation.
            _ => false,
        }
    }

    /// Prove the parent is absent and the exact child is retained.
    pub(super) fn owns_spliced_candidates(
        &self,
        candidates: &BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        !candidates.contains_key(&self.parent.key)
            && candidates.get(&self.child.key) == Some(&self.child)
    }

    /// Build one closed repaired-pair fixture without exposing either raw
    /// candidate to sibling lifecycle tests.
    #[cfg(all(test, feature = "bls"))]
    pub(super) fn repaired_ledger_fixture_for_test(
        context: LifecycleContext,
        marker: u8,
    ) -> Option<(Self, super::ledger::LifecycleLedgerV1)> {
        let root = super::CausalRoot::new(LifecycleDigest::new([marker.wrapping_add(3); 32]));
        let parent_replay = super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::ValidateBody,
            marker,
        );
        let child_replay = super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::SignPrepareVote,
            marker,
        );
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let parent = CandidateAdmission::new(
            parent_replay.key,
            root,
            LifecycleWorkClass::Validate,
            LifecycleStage::new(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            InitialLifecycleState::Ready,
            root.digest(),
            parent_replay.payload,
            parent_replay.authority,
            super::PhysicalGeometry::new(
                [PhysicalSlot::new(
                    effect_slot,
                    LifecycleDigest::new([marker.wrapping_add(4); 32]),
                )],
                [effect_slot],
            ),
            None,
        );
        let child = CandidateAdmission::new(
            child_replay.key,
            root,
            LifecycleWorkClass::SignVote,
            LifecycleStage::new(
                LifecycleStageKind::SignPrepareVote,
                PredecessorScope::Independent,
            ),
            InitialLifecycleState::Ready,
            root.digest(),
            DurablePayloadReference::None,
            child_replay.authority,
            super::PhysicalGeometry::new(
                [PhysicalSlot::new(
                    effect_slot,
                    LifecycleDigest::new([marker.wrapping_add(5); 32]),
                )],
                [effect_slot],
            ),
            None,
        );
        let owner = OwnerId::new(root, 1);
        let parent_address = ConcreteWorkAddress::new(owner, 1, effect_slot)?;
        let child_address = ConcreteWorkAddress::new(owner, 2, effect_slot)?;
        let parent_record = super::ledger::LifecycleLedgerRecordV1::new(
            parent.key,
            owner,
            parent_address.ordinal,
            parent.work_class,
            parent.stage,
            Some(super::TerminalOutcome::Advanced),
            parent.reconstruction_source,
            parent.payload,
            parent.replay_authority.clone(),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::ValidateToSignPrepare,
                child_address.ordinal,
            ),
        )
        .ok()?;
        let child_record = super::ledger::LifecycleLedgerRecordV1::new(
            child.key,
            owner,
            child_address.ordinal,
            child.work_class,
            child.stage,
            None,
            child.reconstruction_source,
            child.payload,
            child.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
        .ok()?;
        let ledger = super::ledger::LifecycleLedgerV1::new(
            context,
            child_address.ordinal,
            vec![parent_record, child_record],
            BTreeMap::new(),
        )
        .ok()?;
        Some((
            Self {
                parent,
                child,
                parent_address,
                child_address,
            },
            ledger,
        ))
    }

    /// Seed only the opaque projection's parent in a focused recovery fixture.
    #[cfg(test)]
    pub(super) fn seed_parent_candidate_for_test(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates
            .insert(self.parent.key, self.parent.clone())
            .is_none()
    }

    /// Seed only the opaque projection's child in a focused recovery fixture.
    #[cfg(test)]
    pub(super) fn seed_child_candidate_for_test(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates
            .insert(self.child.key, self.child.clone())
            .is_none()
    }

    /// Seed both exact sides to prove the production splice rejects ambiguity.
    #[cfg(test)]
    pub(super) fn seed_both_candidates_for_test(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if !candidates.is_empty() {
            return false;
        }
        candidates.insert(self.parent.key, self.parent.clone());
        candidates.insert(self.child.key, self.child.clone());
        true
    }
}

/// Sealed coordinator-open result for one installed recovered Sign.
///
/// The registry remains exclusively borrowed and the authenticated recovery
/// cut stays beside the opened coordinator. No ordinary coordinator, concrete
/// row, candidate, or receipt extraction surface exists.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "opened recovered WAL Sign startup has not published adapter status"]
pub(crate) struct OpenedRecoveredWalSignLifecycleCut<'registry> {
    installed: InstalledRecoveredWalSignRegistryCut<'registry>,
    recovery: AuthenticatedLifecycleRecoveryCut,
    coordinator: LifecycleCoordinator,
}

/// Production-only recovered open with the exact stores used by authentication.
///
/// The comparison seals are captured inside the storage-authenticated open,
/// before any borrow is released. A later owner constructor therefore cannot
/// relabel the opened coordinator with same-context foreign store instances.
#[must_use = "the production recovered open must enter its exact lifecycle owner"]
pub(crate) struct ProductionOpenedRecoveredWalSignLifecycleCut<'registry> {
    opened: OpenedRecoveredWalSignLifecycleCut<'registry>,
    verified: VerifiedHeightContext,
    body_store_identity: crate::sumeragi::v2_body_store::V2BodyStoreInstanceIdentity,
    payload_store_identity:
        crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreInstanceIdentity,
}

/// No-lifetime exact-open seal used only to construct the owning production service.
#[must_use = "the exact recovered WAL open must enter its production owner"]
pub(crate) struct RecoveredWalProductionOwnerOpenV1 {
    pub(super) coordinator: LifecycleCoordinator,
    pub(super) verified: VerifiedHeightContext,
    pub(super) serve_payloads:
        crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
    pub(super) registry_identity: ConcreteLifecycleWorkRegistryInstanceIdentity,
    pub(super) body_store_identity:
        crate::sumeragi::v2_body_store::V2BodyStoreInstanceIdentity,
    pub(super) payload_store_identity:
        crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreInstanceIdentity,
}

/// Opaque fail-stop coordinator-open error retaining every volatile input.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered WAL coordinator open still owns all startup authority"]
pub(crate) struct RecoveredWalSignLifecycleOpenError<'registry> {
    failure: RecoveredWalSignLifecycleOpenFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalSignLifecycleOpenFailure<'registry> {
    InvalidAuthority {
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    InvalidRegistry {
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    InvalidRecovery {
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    Prepare {
        _error: LifecycleOpenError,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    PreCommitMismatch {
        _prepared: PreparedLifecycleCoordinatorOpen,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    Commit {
        _error: LifecycleOpenCommitError,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    PostCommitMismatch {
        _coordinator: LifecycleCoordinator,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
}

impl RecoveredWalSignLifecycleOpenError<'_> {
    /// Stable diagnostic which exposes none of the retained recovery parts.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalSignLifecycleOpenFailure::InvalidAuthority { .. } => {
                "verified height cannot derive recovered lifecycle authority"
            }
            RecoveredWalSignLifecycleOpenFailure::InvalidRegistry { .. } => {
                "installed recovered Sign registry seal is inconsistent"
            }
            RecoveredWalSignLifecycleOpenFailure::InvalidRecovery { .. } => {
                "authenticated recovery lacks the exact recovered WAL handoff"
            }
            RecoveredWalSignLifecycleOpenFailure::Prepare { .. } => {
                "repaired lifecycle ledger could not prepare an exact coordinator open"
            }
            RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch { .. } => {
                "prepared coordinator disagrees with the installed recovered Sign"
            }
            RecoveredWalSignLifecycleOpenFailure::Commit { .. } => {
                "exact recovered coordinator stores could not be published"
            }
            RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch { .. } => {
                "published coordinator disagrees with the installed recovered Sign"
            }
        }
    }
}

impl InstalledRecoveredWalSignRegistryCut<'_> {
    fn structurally_exact_sign(&self) -> Option<&DurableRecoveredWalSignWork> {
        if self.parent_address == self.child_address
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.child_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) = &work.kind else {
            return None;
        };
        (work.digest == self.child_digest
            && work.validates_at(self.child_address)
            && sign.validates_at(self.child_address, self.child_digest))
        .then_some(sign)
    }

    fn structurally_exact_phase_broadcast(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredLifecycleSignedBroadcastWork> {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self.parent_address == self.child_address
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.child_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        (work.digest == self.child_digest
            && work.validates_at(self.child_address)
            && broadcast.validates_phase_in_store(store))
        .then_some(broadcast)
    }

    fn structurally_exact_phase_broadcast_and_next_vote(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<(
        &DurableRecoveredLifecycleSignedBroadcastWork,
        &DurableRecoveredLifecycleNextWalVoteSignWork,
        &super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    )> {
        let (next_sign_address, next_sign_digest) = self.next_sign?;
        let pair = self.pair.as_ref()?;
        if self.parent_address == self.child_address
            || self.child_address.owner == next_sign_address.owner
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == next_sign_address.owner)
                .count()
                != 1
            || !store
                .load()
                .is_ok_and(|ledger| pair.exactly_matches_ledger(&ledger))
        {
            return None;
        }
        let broadcast_work = self.registry.entries.get(&self.child_address)?;
        let next_sign_work = self.registry.entries.get(&next_sign_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &broadcast_work.kind
        else {
            return None;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_sign_work.kind
        else {
            return None;
        };
        (broadcast_work.digest == self.child_digest
            && next_sign_work.digest == next_sign_digest
            && broadcast_work.validates_at(self.child_address)
            && next_sign_work.validates_at(next_sign_address)
            && broadcast.paired_next_sign == Some((next_sign_address, next_sign_digest))
            && broadcast.validates_phase_in_store(store)
            && matches!(
                pair.parent(),
                super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                    validate_ordinal
                } if validate_ordinal == self.parent_address.ordinal
            )
            && pair.broadcast_ordinal() == self.child_address.ordinal
            && pair.next_sign_ordinal() == next_sign_address.ordinal)
            .then_some((broadcast, next_sign, pair))
    }

    fn authenticated_projection(&self) -> Option<AuthenticatedRecoveredWalSignProjection> {
        let (repair, parent_address, sign_ordinal) = if let Some(sign) =
            self.structurally_exact_sign()
        {
            (
                sign.repair.repair(),
                self.parent_address,
                self.child_address.ordinal,
            )
        } else {
            let work = self.registry.entries.get(&self.child_address)?;
            let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
                &work.kind
            else {
                return None;
            };
            let DurableRecoveredLifecycleSignParentV1::PhaseVote(parent) = &broadcast.parent else {
                return None;
            };
            (
                parent.repair.repair(),
                parent.validation.address,
                parent.repair.child_ordinal(),
            )
        };
        let child_address = ConcreteWorkAddress::new(
            self.child_address.owner,
            sign_ordinal,
            PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
        )?;
        Some(AuthenticatedRecoveredWalSignProjection {
            parent: repair.parent().clone(),
            child: repair.child().clone(),
            parent_address,
            child_address,
        })
    }

    fn coordinator_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        let candidate = &projection.child;
        let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
            return false;
        };
        let Some(record) = coordinator.records.get(&self.child_address.ordinal) else {
            return false;
        };
        let Some(durable) = coordinator.durable_records.get(&self.child_address.ordinal) else {
            return false;
        };
        coordinator.fault.is_none()
            && coordinator.active_context.id() == candidate.key.context()
            && coordinator.active_context.height() == candidate.key.round().height()
            && coordinator.high_water >= self.child_address.ordinal
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.producer_turn.is_none()
            && record.key == candidate.key
            && record.owner == self.child_address.owner
            && record.owner.causal_root() == candidate.causal_root
            && record.ordinal == self.child_address.ordinal
            && record.work_class == LifecycleWorkClass::SignVote
            && record.stage == candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && durable.matches_admission(candidate)
            && coordinator.key_index.get(&candidate.key) == Some(&self.child_address.ordinal)
            && coordinator.owner_index.get(&candidate.causal_root) == Some(&record.owner)
            && coordinator
                .ready_index
                .contains(&self.child_address.ordinal)
    }

    fn coordinator_broadcast_is_exact(&self, coordinator: &LifecycleCoordinator) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        self.structurally_exact_phase_broadcast(store)
            .is_some_and(|broadcast| {
                broadcast.matches_current_ready_record(
                    self.child_address,
                    self.child_digest,
                    coordinator,
                )
            })
    }

    fn coordinator_broadcast_and_next_vote_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        self.structurally_exact_phase_broadcast_and_next_vote(store)
            .is_some_and(|(broadcast, next_sign, _pair)| {
                broadcast.matches_current_ready_record(
                    self.child_address,
                    self.child_digest,
                    coordinator,
                ) && self.next_sign.is_some_and(|(address, digest)| {
                    next_sign.matches_current_ready_record(address, digest, coordinator)
                })
            })
    }

    fn recovery_is_exact(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        if let Some((broadcast, next_sign, pair)) = self.phase_broadcast_and_next_vote_projection()
        {
            recovery.owns_recovered_phase_broadcast_and_next_sign(pair, broadcast, next_sign)
        } else if let Some(broadcast) = self.phase_broadcast_projection() {
            recovery.owns_recovered_phase_broadcast(projection, broadcast)
        } else {
            recovery.splice_recovered_wal_sign(projection)
                && recovery.owns_recovered_wal_sign(projection)
        }
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        let sign_is_exact = recovery.owns_recovered_wal_sign(projection)
            && self.installed_entry_is_exact(prepared.store())
            && self.coordinator_is_exact(prepared.coordinator(), projection);
        let broadcast_is_exact = self
            .structurally_exact_phase_broadcast(prepared.store())
            .is_some_and(|broadcast| {
                recovery.owns_recovered_phase_broadcast(projection, &broadcast.broadcast)
                    && broadcast.matches_current_ready_record(
                        self.child_address,
                        self.child_digest,
                        prepared.coordinator(),
                    )
            });
        let pair_is_exact = self
            .structurally_exact_phase_broadcast_and_next_vote(prepared.store())
            .is_some_and(|(broadcast, next_sign, pair)| {
                recovery.owns_recovered_phase_broadcast_and_next_sign(
                    pair,
                    &broadcast.broadcast,
                    &next_sign.projection,
                ) && broadcast.matches_current_ready_record(
                    self.child_address,
                    self.child_digest,
                    prepared.coordinator(),
                ) && self.next_sign.is_some_and(|(address, digest)| {
                    next_sign.matches_current_ready_record(address, digest, prepared.coordinator())
                })
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact phase pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.child_address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.child_address)
                    } else {
                        RecoveredWalRegistrySlotV1::PhaseVote(self.child_address)
                    },
                )
    }

    fn opened_join_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        recovery: &AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        let sign_is_exact = recovery.owns_recovered_wal_sign(projection)
            && self.installed_entry_is_exact(store)
            && self.coordinator_is_exact(coordinator, projection);
        let broadcast_is_exact =
            self.structurally_exact_phase_broadcast(store)
                .is_some_and(|broadcast| {
                    recovery.owns_recovered_phase_broadcast(projection, &broadcast.broadcast)
                        && self.coordinator_broadcast_is_exact(coordinator)
                });
        let pair_is_exact = self
            .structurally_exact_phase_broadcast_and_next_vote(store)
            .is_some_and(|(broadcast, next_sign, pair)| {
                recovery.owns_recovered_phase_broadcast_and_next_sign(
                    pair,
                    &broadcast.broadcast,
                    &next_sign.projection,
                ) && self.coordinator_broadcast_and_next_vote_is_exact(coordinator)
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact phase pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.child_address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.child_address)
                    } else {
                        RecoveredWalRegistrySlotV1::PhaseVote(self.child_address)
                    },
                )
    }
}

impl InstalledRecoveredWalControlSignRegistryCut<'_> {
    fn exact_control_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredWalControlSignWork> {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.address.owner)
                .count()
                != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && sign.validates_in_store(self.address, self.digest, store))
        .then_some(sign)
    }

    fn exact_control_broadcast_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredLifecycleSignedBroadcastWork> {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.address.owner)
                .count()
                != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && broadcast.validates_control_in_store(store))
        .then_some(broadcast)
    }

    fn exact_control_broadcast_and_next_vote_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<(
        &DurableRecoveredLifecycleSignedBroadcastWork,
        &DurableRecoveredLifecycleNextWalVoteSignWork,
        &super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    )> {
        let (next_sign_address, next_sign_digest) = self.next_sign?;
        let pair = self.pair.as_ref()?;
        if pair.parent()
            != super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
            || pair.broadcast_ordinal() != self.address.ordinal
            || pair.next_sign_ordinal() != next_sign_address.ordinal
            || self.address.owner == next_sign_address.owner
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.address.owner)
                .count()
                != 1
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == next_sign_address.owner)
                .count()
                != 1
            || !store
                .load()
                .is_ok_and(|ledger| pair.exactly_matches_ledger(&ledger))
        {
            return None;
        }
        let broadcast_work = self.registry.entries.get(&self.address)?;
        let next_sign_work = self.registry.entries.get(&next_sign_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &broadcast_work.kind
        else {
            return None;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_sign_work.kind
        else {
            return None;
        };
        (broadcast_work.digest == self.digest
            && next_sign_work.digest == next_sign_digest
            && broadcast_work.validates_at(self.address)
            && next_sign_work.validates_at(next_sign_address)
            && broadcast.validates_control_in_store(store))
        .then_some((broadcast, next_sign, pair))
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let sign_is_exact = self
            .exact_control_work(prepared.store())
            .is_some_and(|sign| {
                sign.carrier.owns_recovery(recovery)
                    && sign.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        let broadcast_is_exact = self
            .exact_control_broadcast_work(prepared.store())
            .is_some_and(|broadcast| {
                broadcast.owns_control_recovery(recovery)
                    && broadcast.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        let pair_is_exact = self
            .exact_control_broadcast_and_next_vote_work(prepared.store())
            .is_some_and(|(broadcast, next_sign, pair)| {
                broadcast.owns_control_recovery(recovery)
                    && recovery.owns_recovered_control_broadcast_and_next_sign(
                        pair,
                        &broadcast.broadcast,
                        &next_sign.projection,
                    )
                    && broadcast.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
                    && self.next_sign.is_some_and(|(address, digest)| {
                        next_sign.matches_current_ready_record(
                            address,
                            digest,
                            prepared.coordinator(),
                        )
                    })
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::ControlSign(self.address)
                    },
                )
    }

    fn opened_join_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        let sign_is_exact = self.exact_control_work(store).is_some_and(|sign| {
            sign.carrier.owns_recovery(recovery)
                && sign.matches_current_ready_record(self.address, self.digest, coordinator)
        });
        let broadcast_is_exact =
            self.exact_control_broadcast_work(store)
                .is_some_and(|broadcast| {
                    broadcast.owns_control_recovery(recovery)
                        && broadcast.matches_current_ready_record(
                            self.address,
                            self.digest,
                            coordinator,
                        )
                });
        let pair_is_exact = self
            .exact_control_broadcast_and_next_vote_work(store)
            .is_some_and(|(broadcast, next_sign, pair)| {
                broadcast.owns_control_recovery(recovery)
                    && recovery.owns_recovered_control_broadcast_and_next_sign(
                        pair,
                        &broadcast.broadcast,
                        &next_sign.projection,
                    )
                    && broadcast.matches_current_ready_record(
                        self.address,
                        self.digest,
                        coordinator,
                    )
                    && self.next_sign.is_some_and(|(address, digest)| {
                        next_sign.matches_current_ready_record(address, digest, coordinator)
                    })
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::ControlSign(self.address)
                    },
                )
    }

    /// Install the complete durable Fetch census beside this sole WAL authority.
    pub(super) fn install_fetches(
        &mut self,
        fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(), RecoveredWalControlSignLifecycleOpenError> {
        fetches
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_fetches| {
                RecoveredWalControlSignLifecycleOpenError::new(
                    "recovered control Sign and Fetch carriers conflict",
                )
            })
    }
}

impl<'registry> InstalledRecoveredWalControlSignRegistryCut<'registry> {
    /// Open and commit the exact control/Fetch/Serve/Producer recovery census.
    #[allow(clippy::result_large_err)]
    pub(super) fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        (LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut),
        RecoveredWalControlSignLifecycleOpenError,
    > {
        let exact_carriers = usize::from(self.exact_control_work(&store).is_some())
            + usize::from(self.exact_control_broadcast_work(&store).is_some())
            + usize::from(
                self.exact_control_broadcast_and_next_vote_work(&store)
                    .is_some(),
            );
        if exact_carriers != 1 {
            return Err(RecoveredWalControlSignLifecycleOpenError::new(
                "installed recovered control carrier is not exact",
            ));
        }
        let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
            payload_store,
            &recovery,
        )
        .map_err(|_error| {
            RecoveredWalControlSignLifecycleOpenError::new(
                "recovered control coordinator preparation failed",
            )
        })?;
        if !self.prepared_join_is_exact(&prepared, &recovery) {
            return Err(RecoveredWalControlSignLifecycleOpenError::new(
                "prepared recovered control registry/coordinator census changed",
            ));
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
            .map_err(|_error| {
                RecoveredWalControlSignLifecycleOpenError::new(
                    "recovered control coordinator commit failed",
                )
            })?;
        if !self.opened_join_is_exact(&coordinator, &recovery) {
            return Err(RecoveredWalControlSignLifecycleOpenError::new(
                "opened recovered control registry/coordinator census changed",
            ));
        }
        Ok((coordinator, recovery))
    }
}

impl InstalledRecoveredWalDecisionFetchRegistryCut<'_> {
    fn exact_decision_fetch_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredWalDecisionFetchWork> {
        if self
            .registry
            .entries
            .keys()
            .filter(|address| address.owner == self.address.owner)
            .count()
            != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && fetch.validates_in_store(self.address, self.digest, store))
        .then_some(fetch)
    }

    fn exact_decision_store_work(
        &self,
        ledger: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredDecisionStoreWork> {
        if self
            .registry
            .entries
            .keys()
            .filter(|address| address.owner == self.address.owner)
            .count()
            != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && store.validates_in_store(self.address, self.digest, ledger))
        .then_some(store)
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let fetch_is_exact = self
            .exact_decision_fetch_work(prepared.store())
            .is_some_and(|fetch| {
                fetch.carrier.owns_recovery(recovery)
                    && fetch.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        let store_is_exact = self
            .exact_decision_store_work(prepared.store())
            .is_some_and(|store| {
                store.owns_recovery(recovery)
                    && store.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        (fetch_is_exact ^ store_is_exact)
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    if store_is_exact {
                        RecoveredWalRegistrySlotV1::DecisionStore(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::DecisionFetch(self.address)
                    },
                )
    }

    fn opened_join_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        let fetch_is_exact = self.exact_decision_fetch_work(store).is_some_and(|fetch| {
            fetch.carrier.owns_recovery(recovery)
                && fetch.matches_current_ready_record(self.address, self.digest, coordinator)
        });
        let store_is_exact = self.exact_decision_store_work(store).is_some_and(|store| {
            store.owns_recovery(recovery)
                && store.matches_current_ready_record(self.address, self.digest, coordinator)
        });
        (fetch_is_exact ^ store_is_exact)
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    if store_is_exact {
                        RecoveredWalRegistrySlotV1::DecisionStore(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::DecisionFetch(self.address)
                    },
                )
    }

    /// Install the complete body-backed Fetch census beside the WAL Fetch.
    pub(super) fn install_fetches(
        &mut self,
        fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(), RecoveredWalDecisionFetchLifecycleOpenError> {
        fetches
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_fetches| {
                RecoveredWalDecisionFetchLifecycleOpenError::new(
                    "recovered Decision Fetch and body-backed Fetch carriers conflict",
                )
            })
    }
}

impl<'registry> InstalledRecoveredWalDecisionFetchRegistryCut<'registry> {
    /// Open and commit the exact Decision-Fetch/Fetch/Serve/Producer census.
    #[allow(clippy::result_large_err)]
    pub(super) fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        (LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut),
        RecoveredWalDecisionFetchLifecycleOpenError,
    > {
        if self.exact_decision_fetch_work(&store).is_none()
            && self.exact_decision_store_work(&store).is_none()
        {
            return Err(RecoveredWalDecisionFetchLifecycleOpenError::new(
                "installed recovered Decision Fetch carrier is not exact",
            ));
        }
        let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
            payload_store,
            &recovery,
        )
        .map_err(|_error| {
            RecoveredWalDecisionFetchLifecycleOpenError::new(
                "recovered Decision Fetch coordinator preparation failed",
            )
        })?;
        if !self.prepared_join_is_exact(&prepared, &recovery) {
            return Err(RecoveredWalDecisionFetchLifecycleOpenError::new(
                "prepared recovered Decision Fetch registry/coordinator census changed",
            ));
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
            .map_err(|_error| {
                RecoveredWalDecisionFetchLifecycleOpenError::new(
                    "recovered Decision Fetch coordinator commit failed",
                )
            })?;
        if !self.opened_join_is_exact(&coordinator, &recovery) {
            return Err(RecoveredWalDecisionFetchLifecycleOpenError::new(
                "opened recovered Decision Fetch registry/coordinator census changed",
            ));
        }
        Ok((coordinator, recovery))
    }
}

impl InstalledRecoveredDecisionApplyRegistryCut<'_> {
    fn exact_apply_work(&self) -> Option<&DurableRecoveredDecisionApplyWork> {
        if self
            .registry
            .entries
            .keys()
            .filter(|address| address.owner == self.address.owner)
            .count()
            != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && apply.validates_at(self.address, self.digest))
        .then_some(apply)
    }

    fn prepared_join_is_exact(&self, prepared: &PreparedLifecycleCoordinatorOpen) -> bool {
        self.exact_apply_work().is_some_and(|apply| {
            apply.matches_current_ready_record(self.address, self.digest, prepared.coordinator())
        }) && self
            .registry
            .exactly_covers_recovered_ready_fetches_with_extra(
                prepared.coordinator(),
                RecoveredWalRegistrySlotV1::DecisionApply(self.address),
            )
    }

    fn opened_join_is_exact(&self, coordinator: &LifecycleCoordinator) -> bool {
        coordinator.ledger_store.is_some()
            && self.exact_apply_work().is_some_and(|apply| {
                apply.matches_current_ready_record(self.address, self.digest, coordinator)
            })
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    RecoveredWalRegistrySlotV1::DecisionApply(self.address),
                )
    }

    /// Install every unrelated durable Ready-Fetch beside the sole Apply carrier.
    pub(super) fn install_fetches(
        &mut self,
        fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(), RecoveredDecisionApplyLifecycleOpenError> {
        fetches
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_fetches| {
                RecoveredDecisionApplyLifecycleOpenError::new(
                    "recovered Decision Apply and Ready-Fetch carriers conflict",
                )
            })
    }
}

impl<'registry> InstalledRecoveredDecisionApplyRegistryCut<'registry> {
    /// Publish the exact prospective four-row successor and finish startup.
    ///
    /// Coordinator reconstruction, payload-store authentication, the complete
    /// prepublication registry census, and the exact predecessor/successor
    /// ledger pair are already retained by `prepared`. After its single exact
    /// successor fsync, only infallible coordinator/registry ownership moves
    /// remain.
    #[allow(clippy::result_large_err)]
    pub(super) fn open_with_prepared_successor(
        self,
        prepared: PreparedLifecycleCoordinatorOpen,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        (LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut),
        RecoveredDecisionApplyLifecycleOpenError,
    > {
        if !self.prepared_join_is_exact(&prepared) {
            return Err(RecoveredDecisionApplyLifecycleOpenError::new(
                "prepared recovered Decision Apply registry/coordinator census changed",
            ));
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
            .map_err(|_error| {
                RecoveredDecisionApplyLifecycleOpenError::new(
                    "recovered Decision Apply exact successor publication failed",
                )
            })?;
        assert!(
            self.opened_join_is_exact(&coordinator),
            "preflighted recovered Decision Apply publication must finish with the exact opened census"
        );
        Ok((coordinator, recovery))
    }
}

impl<'registry> InstalledRecoveredWalSignRegistryCut<'registry> {
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    fn open_with_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(projection) = self.authenticated_projection() else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidRegistry {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        let recovery_is_exact = self.recovery_is_exact(&mut recovery, &projection);
        if !recovery_is_exact {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidRecovery {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        let prepared = match LifecycleCoordinator::prepare_with_authority_borrowed(
            authority,
            ledger_root,
            payload_store,
            &recovery,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(RecoveredWalSignLifecycleOpenError {
                    failure: RecoveredWalSignLifecycleOpenFailure::Prepare {
                        _error: error,
                        _installed: self,
                        _recovery: recovery,
                    },
                });
            }
        };
        if !self.prepared_join_is_exact(&prepared, &recovery, &projection) {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch {
                    _prepared: prepared,
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        let coordinator = match prepared.commit_with_registry(
            &mut *self.registry,
            payload_store,
            &mut recovery,
        ) {
            Ok(coordinator) => coordinator,
            Err(error) => {
                return Err(RecoveredWalSignLifecycleOpenError {
                    failure: RecoveredWalSignLifecycleOpenFailure::Commit {
                        _error: error,
                        _installed: self,
                        _recovery: recovery,
                    },
                });
            }
        };
        if !self.opened_join_is_exact(&coordinator, &recovery, &projection) {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch {
                    _coordinator: coordinator,
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        Ok(OpenedRecoveredWalSignLifecycleCut {
            installed: self,
            recovery,
            coordinator,
        })
    }

    /// Open against the exact store retained continuously since parent reconstruction.
    #[allow(clippy::result_large_err)]
    fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(projection) = self.authenticated_projection() else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidRegistry {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        let recovery_is_exact = self.recovery_is_exact(&mut recovery, &projection);
        if !recovery_is_exact {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidRecovery {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        let prepared = match LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
            payload_store,
            &recovery,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(RecoveredWalSignLifecycleOpenError {
                    failure: RecoveredWalSignLifecycleOpenFailure::Prepare {
                        _error: error,
                        _installed: self,
                        _recovery: recovery,
                    },
                });
            }
        };
        if !self.prepared_join_is_exact(&prepared, &recovery, &projection) {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch {
                    _prepared: prepared,
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        let coordinator = match prepared.commit_with_registry(
            &mut *self.registry,
            payload_store,
            &mut recovery,
        ) {
            Ok(coordinator) => coordinator,
            Err(error) => {
                return Err(RecoveredWalSignLifecycleOpenError {
                    failure: RecoveredWalSignLifecycleOpenFailure::Commit {
                        _error: error,
                        _installed: self,
                        _recovery: recovery,
                    },
                });
            }
        };
        if !self.opened_join_is_exact(&coordinator, &recovery, &projection) {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch {
                    _coordinator: coordinator,
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        Ok(OpenedRecoveredWalSignLifecycleCut {
            installed: self,
            recovery,
            coordinator,
        })
    }

    /// Prepare, exact-join, and durably publish the recovered coordinator from
    /// production verified/configured authority without releasing the registry
    /// borrow.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_coordinator_from_verified(
        self,
        verified: &VerifiedHeightContext,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(authority) =
            authority::production_authority(verified, config, reply_route_source_capacity)
        else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidAuthority {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        self.open_with_authority(authority, ledger_root, payload_store, recovery)
    }

    /// Open with the minimal exact test authority while retaining all seals.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_coordinator_for_test(
        self,
        verified: &VerifiedHeightContext,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(authority) = authority::recovered_wal_test_authority(verified) else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidAuthority {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        self.open_with_authority(authority, ledger_root, payload_store, recovery)
    }

    /// Corrupt only the opaque installed-token digest for a focused negative
    /// test. The closed registry row and its complete durable authority remain
    /// present and exclusively borrowed.
    #[cfg(test)]
    pub(crate) fn corrupt_registry_seal_for_test(&mut self) {
        self.child_digest = LifecycleDigest::new([0xFF; 32]);
    }

    /// Seed the exact opaque recovered Validate parent for a focused fixture.
    #[cfg(test)]
    pub(crate) fn seed_parent_recovery_for_test(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.authenticated_projection()
            .is_some_and(|projection| recovery.seed_recovered_wal_parent_for_test(&projection))
    }

    /// Seed the exact opaque recovered Sign child for a re-entry fixture.
    #[cfg(test)]
    pub(crate) fn seed_child_recovery_for_test(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.authenticated_projection()
            .is_some_and(|projection| recovery.seed_recovered_wal_child_for_test(&projection))
    }

    /// Seed both opaque projection sides for an ambiguous-recovery negative.
    #[cfg(test)]
    pub(crate) fn seed_both_recovery_for_test(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.authenticated_projection().is_some_and(|projection| {
            recovery.seed_both_recovered_wal_candidates_for_test(&projection)
        })
    }
}

impl<'registry> ProductionOpenedRecoveredWalSignLifecycleCut<'registry> {
    /// Consume the exclusive registry borrow into a no-lifetime owner-open seal.
    pub(crate) fn into_production_owner_open(
        self,
    ) -> Result<RecoveredWalProductionOwnerOpenV1, Self> {
        let Self {
            opened,
            verified,
            body_store_identity,
            payload_store_identity,
        } = self;
        let Some(projection) = opened.installed.authenticated_projection() else {
            return Err(Self {
                opened,
                verified,
                body_store_identity,
                payload_store_identity,
            });
        };
        if !opened.installed.opened_join_is_exact(
            &opened.coordinator,
            &opened.recovery,
            &projection,
        ) || opened.coordinator.active_context()
            != projection::lifecycle_context(verified.context())
        {
            return Err(Self {
                opened,
                verified,
                body_store_identity,
                payload_store_identity,
            });
        }
        let OpenedRecoveredWalSignLifecycleCut {
            installed,
            recovery,
            coordinator,
        } = opened;
        let registry_identity = installed.registry.instance_identity();
        drop(installed);
        Ok(RecoveredWalProductionOwnerOpenV1 {
            coordinator,
            verified,
            serve_payloads: recovery.into_serve_payloads(),
            registry_identity,
            body_store_identity,
            payload_store_identity,
        })
    }

    /// Seal a focused opened-cut fixture with the exact stores it used.
    #[cfg(test)]
    pub(crate) fn from_opened_for_test(
        opened: OpenedRecoveredWalSignLifecycleCut<'registry>,
        verified: VerifiedHeightContext,
        body_store: &V2BodyStore,
        payload_store: &CertifiedServePayloadStoreV1,
    ) -> Self {
        Self {
            opened,
            verified,
            body_store_identity: body_store.instance_identity(),
            payload_store_identity: payload_store.instance_identity(),
        }
    }
}

#[cfg(test)]
impl OpenedRecoveredWalSignLifecycleCut<'_> {
    /// Revalidate the complete installed/recovery/coordinator/store join.
    pub(crate) fn exact_join_for_test(&self) -> bool {
        let Some(projection) = self.installed.authenticated_projection() else {
            return false;
        };
        self.installed
            .opened_join_is_exact(&self.coordinator, &self.recovery, &projection)
            && self.recovered_wal_sign_census_rejects_mutations_for_test()
    }

    fn recovered_wal_sign_census_rejects_mutations_for_test(&self) -> bool {
        let address = self.installed.child_address;
        let registry = &*self.installed.registry;
        if !registry.exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator) {
            return false;
        }

        let mut missing = self.coordinator.clone();
        missing.records.remove(&address.ordinal);

        let mut terminal = self.coordinator.clone();
        let Some(terminal_record) = terminal.records.get_mut(&address.ordinal) else {
            return false;
        };
        terminal_record.state = super::LifecycleState::Terminal(super::TerminalOutcome::Cancelled);

        let mut stale = self.coordinator.clone();
        stale.ready_index.remove(&address.ordinal);

        let mut mutated = self.coordinator.clone();
        let Some(metadata) = mutated.durable_records.get_mut(&address.ordinal) else {
            return false;
        };
        let mut foreign_source = *metadata.reconstruction_source.as_bytes();
        foreign_source[0] ^= 1;
        metadata.reconstruction_source = LifecycleDigest::new(foreign_source);

        [&missing, &terminal, &stale, &mutated]
            .into_iter()
            .all(|coordinator| {
                !registry.exactly_covers_recovered_ready_work_and_wal_authority(coordinator)
            })
    }
}

#[cfg(test)]
impl RecoveredWalSignLifecycleOpenError<'_> {
    fn installed(&self) -> &InstalledRecoveredWalSignRegistryCut<'_> {
        match &self.failure {
            RecoveredWalSignLifecycleOpenFailure::InvalidAuthority { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::InvalidRegistry { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::InvalidRecovery { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::Prepare { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::Commit { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch { _installed, .. } => {
                _installed
            }
        }
    }

    /// Prove the error retains one exact installed row against the ledger.
    pub(crate) fn retains_exact_installed_for_test(&self, ledger_root: &Path) -> bool {
        self.installed().exact_installed_shape_for_test(ledger_root)
    }

    /// Prove the error still exclusively owns a closed recovered Sign row.
    pub(crate) fn retains_closed_registry_row_for_test(&self) -> bool {
        let installed = self.installed();
        installed
            .registry
            .entries
            .get(&installed.child_address)
            .is_some_and(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
                )
            })
    }
}
// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_END

impl DetachedRecoveredValidateCompletion {
    fn restore(
        self,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> ConcreteLifecycleWork {
        let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) = self.replay_evidence
        else {
            panic!("a cold recovered body marker cannot reconstruct live certified Fetch origin")
        };
        ConcreteLifecycleWork {
            digest: self.installed_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(DurableValidateCompletion {
                address: self.address,
                incumbent: DurableValidateBody {
                    address: self.incumbent_address,
                    effect,
                    pending,
                    durable_receipt: self.durable_receipt,
                    expected_manifest_hash: self.expected_manifest_hash,
                    replay_evidence,
                },
                incumbent_digest: self.incumbent_digest,
                outcome: self.outcome,
            }),
        }
    }
}

/// Ownership-preserving failure from the fixed recovered-WAL parent join.
///
/// No adapter effect, pending binding, recovered vote, or registry entry is
/// exposed. Before projection, dropping this value restores the exact detached
/// carrier; a lifecycle-authentication failure retains all linear authority
/// and requires restart rather than falling back to ordinary execution.
#[must_use = "failed recovered WAL validation still owns its sealed authority"]
pub(crate) struct RecoveredWalValidateRegistryJoinError<'registry> {
    failure: RecoveredWalValidateRegistryJoinFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalValidateRegistryJoinFailure<'registry> {
    InvalidCarrier {
        _cut: RecoveredWalValidateRegistryCut<'registry>,
        _recovered: RecoveredWalVoteSign,
    },
    Projection {
        _cut: RecoveredWalValidateRegistryCut<'registry>,
        _recovered: RecoveredWalVoteSign,
    },
    Lifecycle {
        _cut: RecoveredWalValidateRegistryCut<'registry>,
        _error: RecoveredWalVoteLifecycleRepairError,
        _completion: DetachedRecoveredValidateCompletion,
    },
}

impl RecoveredWalValidateRegistryJoinError<'_> {
    /// Return a stable diagnostic without exposing retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalValidateRegistryJoinFailure::InvalidCarrier { .. } => {
                "recovered Validate registry carrier is invalid"
            }
            RecoveredWalValidateRegistryJoinFailure::Projection { .. } => {
                "recovered vote does not project from the exact Validate registry carrier"
            }
            RecoveredWalValidateRegistryJoinFailure::Lifecycle { _error, .. } => _error.reason(),
        }
    }
}

impl Drop for RecoveredWalValidateRegistryCut<'_> {
    fn drop(&mut self) {
        let Some(work) = self.work.take() else {
            return;
        };
        let Some(registry) = self.registry.as_deref_mut() else {
            debug_assert!(
                false,
                "detached recovered Validate lost its registry borrow"
            );
            return;
        };
        match registry.entries.entry(self.address) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(work);
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                debug_assert!(false, "detached recovered Validate address was replaced");
            }
        }
    }
}

impl ConcreteLifecycleWorkRegistry {
    /// Bind one fsynced recovered body to the exact claimed WAL Fetch carrier.
    pub(super) fn prepare_recovered_decision_fetch_store_adapter_authority(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        key: RecoveredDecisionFetchDispatchKeyV1,
        body: crate::sumeragi::v2_body_store::RecoveredDecisionFetchStoreBodyAuthorityV1,
    ) -> Result<
        super::RecoveredDecisionFetchStoreAdapterAuthorityV1,
        RecoveredDecisionFetchStorePreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        if lease.physical_slots().len() != 1 {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let work = self
            .entries
            .get(&address)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        };
        if work.digest != digest
            || !work.validates_at(address)
            || fetch.dispatch_key != Some(key)
            || !key.matches(coordinator.active_context, address, digest)
            || !fetch.matches_claimed_record(address, digest, coordinator, lease)
        {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        fetch
            .carrier
            .project_store_adapter_authority(body)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidBody)
    }

    /// Seal the reducer-derived recovered Store child under the claimed Fetch.
    pub(super) fn prepare_recovered_decision_fetch_store_successor<'registry, 'adapter>(
        &'registry mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        key: RecoveredDecisionFetchDispatchKeyV1,
        adapter: crate::sumeragi::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'adapter>,
    ) -> Result<
        PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>,
        RecoveredDecisionFetchStorePreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        if lease.physical_slots().len() != 1 {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        let fetch_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let fetch = self
            .entries
            .get(&fetch_address)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &fetch.kind else {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        };
        if fetch.dispatch_key != Some(key)
            || !key.matches(coordinator.active_context, fetch_address, digest)
            || !fetch.matches_claimed_record(fetch_address, digest, coordinator, lease)
        {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        let store = fetch
            .carrier
            .project_store_successor(verified, adapter.body_authority(), adapter.store_effect())
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidStoreProjection)?;
        let store_ordinal = coordinator
            .high_water
            .checked_add(1)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::ChildCollision)?;
        let store_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let store_address = ConcreteWorkAddress::new(lease.owner(), store_ordinal, store_slot)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::ChildCollision)?;
        if self.entries.contains_key(&store_address)
            || self
                .entries
                .keys()
                .filter(|address| address.owner == lease.owner())
                .count()
                != 1
            || !store.validates_at(coordinator.active_context, store_address, store.digest())
        {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::ChildCollision);
        }
        Ok(PreparedRecoveredDecisionFetchStoreSuccessor {
            registry: self,
            fetch_address,
            store_address,
            store,
            adapter,
        })
    }
}

impl<'registry, 'adapter> PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter> {
    /// Project the exact child while retaining registry and adapter borrows.
    pub(super) fn project_for_body_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        if self.fetch_address.owner != lease.owner()
            || self.fetch_address.ordinal != lease.ordinal()
            || self.store_address.owner != lease.owner()
        {
            return Err(SealedBodySuccessorProjectionError::ForeignParent);
        }
        self.store
            .candidate_for_transition(verified)
            .ok_or(SealedBodySuccessorProjectionError::InvalidCarrier)
    }

    /// Replace the exact WAL Fetch carrier with its dedicated Store carrier.
    pub(super) fn commit_after_publication(
        self,
    ) -> crate::sumeragi::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'adapter> {
        let Self {
            registry,
            fetch_address,
            store_address,
            store,
            adapter,
        } = self;
        let fetch = registry
            .entries
            .remove(&fetch_address)
            .expect("published recovered Store retains its exact Fetch carrier");
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = fetch.kind else {
            panic!("published recovered Store cannot replace another carrier class")
        };
        assert!(fetch.dispatch_key.is_some());
        let context = store.context();
        let digest = store.digest();
        let replacement = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(
                DurableRecoveredDecisionStoreWork {
                    fetch: fetch.carrier,
                    store,
                    context,
                    address: store_address,
                },
            ),
        };
        assert!(replacement.validates_at(store_address));
        assert!(
            registry
                .entries
                .insert(store_address, replacement)
                .is_none()
        );
        adapter
    }
}

impl ConcreteLifecycleWorkRegistry {
    /// Seal the adapter-authenticated Broadcast child under one claimed recovered Sign.
    pub(super) fn prepare_recovered_lifecycle_sign_broadcast_successor<'registry, 'adapter>(
        &'registry mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        key: RecoveredLifecycleSignDispatchKeyV1,
        adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter>,
    ) -> Result<
        PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>,
        RecoveredLifecycleSignBroadcastPreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier)?;
        if lease.physical_slots().len() != 1
            || adapter.dispatch_key() != key
            || adapter.shape()
                != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast
        {
            return Err(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier);
        }
        let sign_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier)?;
        let projection_authority = adapter.project_registry_broadcast_authority();
        let sign = self
            .entries
            .get(&sign_address)
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier)?;
        if sign.digest != digest || !sign.validates_at(sign_address) {
            return Err(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier);
        }
        let (projected_key, broadcast) = match &sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                if sign.dispatch_key == Some(key)
                    && sign.matches_claimed_record(sign_address, digest, coordinator, lease) =>
            {
                sign.repair
                    .project_authenticated_signed_broadcast(verified, projection_authority)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign)
                if sign.dispatch_key == Some(key)
                    && sign.matches_current_claimed_record(
                        sign_address,
                        digest,
                        coordinator,
                        lease,
                    ) =>
            {
                super::wal_recovery::project_recovered_next_wal_vote_signed_broadcast(
                    &sign.projection,
                    verified,
                    projection_authority,
                )
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                if sign.dispatch_key == Some(key)
                    && sign.carrier.matches_claimed_record(coordinator, lease) =>
            {
                sign.carrier
                    .project_authenticated_signed_broadcast(verified, projection_authority)
            }
            _ => None,
        }
        .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidBroadcastProjection)?;
        if projected_key != key {
            return Err(
                RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidBroadcastProjection,
            );
        }
        let broadcast_ordinal = coordinator
            .high_water
            .checked_add(1)
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::ChildCollision)?;
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let broadcast_address =
            ConcreteWorkAddress::new(lease.owner(), broadcast_ordinal, broadcast_slot)
                .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::ChildCollision)?;
        if self.entries.contains_key(&broadcast_address)
            || self
                .entries
                .keys()
                .filter(|address| address.owner == lease.owner())
                .count()
                != 1
            || !broadcast.validates_at(verified, broadcast_address, broadcast.digest())
        {
            return Err(RecoveredLifecycleSignBroadcastPreparationErrorV1::ChildCollision);
        }
        Ok(PreparedRecoveredLifecycleSignBroadcastSuccessor {
            registry: self,
            sign_address,
            broadcast_address,
            broadcast,
            verified: verified.clone(),
            adapter,
        })
    }

    /// Seal the exact Broadcast-and-next-WAL-Sign pair under one claimed Sign.
    ///
    /// Body authority has already crossed the launched service/executor census,
    /// but remains opaque here. The adapter consumes it only after the exact
    /// installed parent, dispatch key, lease, and two-child reducer shape have
    /// all rejoined; no registry entry changes before LedgerV1 publication.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<
        'registry,
        'adapter,
    >(
        &'registry mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        key: RecoveredLifecycleSignDispatchKeyV1,
        mut adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<
            'adapter,
        >,
        body: crate::sumeragi::v2::RecoveredLifecycleNextVoteBodyAuthorityV1,
    ) -> Result<
        PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>,
        RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier)?;
        if lease.physical_slots().len() != 1
            || adapter.dispatch_key() != key
            || adapter.shape()
                != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
            || coordinator.high_water.checked_add(2).is_none()
        {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier,
            );
        }
        let sign_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier)?;
        let sign = self
            .entries
            .get(&sign_address)
            .ok_or(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier)?;
        if sign.digest != digest
            || !sign.validates_at(sign_address)
            || self
                .entries
                .keys()
                .filter(|address| address.owner == lease.owner())
                .count()
                != 1
        {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier,
            );
        }
        let parent_is_exact = match &sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.dispatch_key == Some(key)
                    && sign.matches_claimed_record(sign_address, digest, coordinator, lease)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                sign.dispatch_key == Some(key)
                    && sign.matches_current_claimed_record(sign_address, digest, coordinator, lease)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.dispatch_key == Some(key)
                    && sign.carrier.matches_claimed_record(coordinator, lease)
            }
            _ => false,
        };
        if !parent_is_exact {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier,
            );
        }
        let projection_authority = adapter.project_broadcast_and_sign_authority(body).map_err(
            |_| RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
        )?;
        let (projected_key, successor) = match &sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => sign
                .repair
                .project_authenticated_signed_broadcast_and_sign(verified, projection_authority),
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                super::wal_recovery::project_recovered_next_wal_vote_signed_broadcast_and_sign(
                    &sign.projection,
                    verified,
                    projection_authority,
                )
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => sign
                .carrier
                .project_authenticated_signed_broadcast_and_sign(verified, projection_authority),
            _ => None,
        }
        .ok_or(
            RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
        )?;
        if projected_key != key {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        }
        Ok(PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor {
            registry: self,
            sign_address,
            successor,
            verified: verified.clone(),
            adapter,
        })
    }
}

impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter> {
    /// Project the exact Broadcast candidate while retaining every owner.
    pub(super) fn project_for_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        if self.sign_address.owner != lease.owner()
            || self.sign_address.ordinal != lease.ordinal()
            || self.broadcast_address.owner != lease.owner()
            || self.verified.context() != verified.context()
            || self.verified.proofs_of_possession() != verified.proofs_of_possession()
            || !self.broadcast.validates_at(
                verified,
                self.broadcast_address,
                self.broadcast.digest(),
            )
        {
            return Err(SealedBodySuccessorProjectionError::ForeignParent);
        }
        Ok(self.broadcast.candidate().clone())
    }

    /// Replace the exact recovered Sign carrier with its durable Broadcast child.
    pub(super) fn commit_after_publication(
        self,
    ) -> crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter> {
        let Self {
            registry,
            sign_address,
            broadcast_address,
            broadcast,
            verified,
            adapter,
        } = self;
        let sign = registry
            .entries
            .remove(&sign_address)
            .expect("published recovered Broadcast retains its exact Sign carrier");
        let parent = match sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::PhaseVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::NextWalVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::Control(sign)
            }
            _ => panic!("published recovered Broadcast cannot replace another carrier class"),
        };
        assert!(parent.dispatch_key().is_some());
        let digest = broadcast.digest();
        let replacement = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent,
                    broadcast,
                    verified,
                    address: broadcast_address,
                    paired_next_sign: None,
                },
            ),
        };
        assert!(replacement.validates_at(broadcast_address));
        assert!(
            registry
                .entries
                .insert(broadcast_address, replacement)
                .is_none()
        );
        adapter
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry, 'adapter>
    PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>
{
    /// Classify the sole assertion-only adapter commit mode before fsync.
    pub(super) fn publication_is_vote(&self) -> Option<bool> {
        match (
            self.adapter.is_vote_broadcast_and_sign(),
            self.adapter.is_authorized_proposal_broadcast_and_sign(),
        ) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            (true, true) | (false, false) => None,
        }
    }

    /// Clone the two inert admissions only under the transition-private permit.
    pub(super) fn project_transition_candidates(
        &self,
        permit: super::body_pipeline_transition::RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1,
    ) -> (CandidateAdmission, CandidateAdmission) {
        self.successor.project_transition_candidates(permit)
    }

    /// Bind both process-local child addresses to the exact staged rows.
    pub(super) fn bind_staged_children(
        self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
        next_sign_ordinal: u128,
    ) -> Result<
        BoundRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>,
        RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1,
    > {
        if !self.successor.matches_staged_ready_children(
            &self.verified,
            coordinator,
            broadcast_ordinal,
            next_sign_ordinal,
        ) {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        }
        let (Some(broadcast_record), Some(next_sign_record)) = (
            coordinator.records.get(&broadcast_ordinal),
            coordinator.records.get(&next_sign_ordinal),
        ) else {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        };
        let (Some((&broadcast_slot, _)), Some((&next_sign_slot, _))) = (
            broadcast_record.physical_slots.first_key_value(),
            next_sign_record.physical_slots.first_key_value(),
        ) else {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        };
        let (Some(broadcast_address), Some(next_sign_address)) = (
            ConcreteWorkAddress::new(broadcast_record.owner, broadcast_ordinal, broadcast_slot),
            ConcreteWorkAddress::new(next_sign_record.owner, next_sign_ordinal, next_sign_slot),
        ) else {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        };
        if broadcast_address.owner != self.sign_address.owner
            || next_sign_address.owner == self.sign_address.owner
            || self.registry.entries.contains_key(&broadcast_address)
            || self.registry.entries.contains_key(&next_sign_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.sign_address.owner)
                .count()
                != 1
            || self
                .registry
                .entries
                .keys()
                .any(|address| address.owner == next_sign_address.owner)
        {
            return Err(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::ChildCollision);
        }
        Ok(BoundRecoveredLifecycleSignBroadcastAndSignSuccessor {
            registry: self.registry,
            sign_address: self.sign_address,
            broadcast_address,
            next_sign_address,
            successor: self.successor,
            verified: self.verified,
            adapter: self.adapter,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry, 'adapter>
    BoundRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>
{
    /// Replace the exact recovered Sign with both durably published children.
    ///
    /// The combined projection separates only in this assertion-only tail.
    /// Broadcast retains the original claimed Sign carrier; the follow-on WAL
    /// Vote becomes a distinct undispatched Sign carrier at its staged owner.
    pub(super) fn commit_after_publication(
        self,
    ) -> crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter> {
        let Self {
            registry,
            sign_address,
            broadcast_address,
            next_sign_address,
            successor,
            verified,
            adapter,
        } = self;
        let sign = registry
            .entries
            .remove(&sign_address)
            .expect("published combined successor retains its exact Sign parent");
        let parent = match sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::PhaseVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::NextWalVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::Control(sign)
            }
            _ => panic!("published combined successor cannot replace another carrier class"),
        };
        assert!(parent.dispatch_key().is_some());
        let (broadcast, next_sign) = successor.into_registry_children(
            RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new(),
        );
        let broadcast_digest = broadcast.digest();
        let next_sign_digest = next_sign.digest();
        let broadcast_work = ConcreteLifecycleWork {
            digest: broadcast_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent,
                    broadcast,
                    verified: verified.clone(),
                    address: broadcast_address,
                    paired_next_sign: Some((next_sign_address, next_sign_digest)),
                },
            ),
        };
        let next_sign_work = ConcreteLifecycleWork {
            digest: next_sign_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: next_sign,
                    verified,
                    address: next_sign_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(broadcast_work.validates_at(broadcast_address));
        assert!(next_sign_work.validates_at(next_sign_address));
        assert!(
            registry
                .entries
                .insert(broadcast_address, broadcast_work)
                .is_none()
        );
        assert!(
            registry
                .entries
                .insert(next_sign_address, next_sign_work)
                .is_none()
        );
        adapter
    }
}
