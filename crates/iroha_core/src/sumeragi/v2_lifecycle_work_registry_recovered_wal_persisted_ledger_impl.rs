impl<'registry> PersistedRecoveredWalValidateLedger<'registry> {
    /// Advance the cold adapter through either the exact single Broadcast or
    /// its adjacent WAL-backed Commit-Sign pair.
    ///
    /// Pair recognition is frame-bound and transaction-local; unrelated later
    /// rows do not change it. The body-store join and adapter replay happen
    /// before the authority variant changes, and the exact store is reloaded
    /// once more before this method releases the prepared startup.
    #[inline(never)]
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
        match authority {
            PersistedRecoveredWalLifecycleAuthority::Sign(repair) => {
                Self::prepare_cold_sign_branch(store, repaired, repair, startup)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair) => {
                Self::prepare_cold_signed_broadcast_branch(
                    store, repaired, repair, verified, startup, body_store,
                )
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote { .. } => {
                Err("recovered phase cold adapter pair was prepared twice")
            }
        }
    }
    #[inline(never)]
    fn prepare_cold_sign_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: Box<DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>>,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
    ) -> Result<
        (
            crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
            Self,
        ),
        &'static str,
    > {
        Ok((
            startup,
            Self {
                store,
                repaired,
                authority: PersistedRecoveredWalLifecycleAuthority::Sign(repair),
            },
        ))
    }
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn prepare_cold_signed_broadcast_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
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
        drop(matching);
        match pair_hint {
            Some(pair_hint) => Self::prepare_cold_signed_broadcast_and_next_vote_branch(
                store, repaired, repair, pair_hint, verified, startup, body_store,
            ),
            None => Self::prepare_cold_single_signed_broadcast_branch(
                store, repaired, repair, verified, startup,
            ),
        }
    }
    #[inline(never)]
    fn prepare_cold_single_signed_broadcast_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        verified: &VerifiedHeightContext,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
    ) -> Result<
        (
            crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
            Self,
        ),
        &'static str,
    > {
        let adapter_authority = repair
            .repair
            .project_cold_adapter_authority(verified, &repair.broadcast)
            .ok_or("recovered phase Broadcast cannot replay the exact cold adapter")?;
        let startup =
            startup.advance_recovered_lifecycle_signed_broadcast(verified, adapter_authority)?;
        Ok((
            startup,
            Self {
                store,
                repaired,
                authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair),
            },
        ))
    }
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn prepare_cold_signed_broadcast_and_next_vote_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        pair_hint: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
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
    #[inline(never)]
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
        match authority {
            PersistedRecoveredWalLifecycleAuthority::Sign(repair) => {
                Self::install_recovered_sign_branch(store, repaired, repair)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair) => {
                Self::install_recovered_broadcast_branch(store, repaired, repair)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote {
                repair,
                combined,
                pair,
            } => Self::install_recovered_broadcast_and_next_vote_branch(
                store, repaired, repair, combined, pair,
            ),
        }
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn install_recovered_sign_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: Box<DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>>,
    ) -> Result<
        InstalledRecoveredWalSignStorage<'registry>,
        ExactStoreRecoveredWalSignInstallError<'registry>,
    > {
        match repair.install_recovered_sign(&store) {
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
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn install_recovered_broadcast_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
    ) -> Result<
        InstalledRecoveredWalSignStorage<'registry>,
        ExactStoreRecoveredWalSignInstallError<'registry>,
    > {
        match repair.install_recovered_broadcast(&store) {
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
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn install_recovered_broadcast_and_next_vote_branch(
        store: super::ledger::LifecycleLedgerStoreV1,
        repaired: super::ledger::LifecycleLedgerV1,
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        pair: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> Result<
        InstalledRecoveredWalSignStorage<'registry>,
        ExactStoreRecoveredWalSignInstallError<'registry>,
    > {
        match repair.install_recovered_broadcast_and_next_vote(&store, combined, pair) {
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
