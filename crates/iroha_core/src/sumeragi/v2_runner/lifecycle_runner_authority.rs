//! Move-only runner authorities for production lifecycle startup and activation.

use super::*;

/// Move-only authority for binding runner-owned lifecycle execution dependencies.
///
/// The non-PendingKura runner mints this private seal immediately before it
/// moves Queue, archive, and event ownership into recovered startup. Sumeragi
/// siblings may name the consumed type but cannot manufacture production
/// authority for caller-selected dependencies.
#[must_use = "the runner dependency permit must enter recovered lifecycle startup"]
pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1 {
    _seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1,
    local_signer: KeyPair,
    block_cadence: Duration,
}

struct RecoveredLifecycleOwnerFactoryDependencyPermitSealV1;

impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1 {
    fn drop(&mut self) {}
}

impl RecoveredLifecycleOwnerFactoryDependencyPermitV1 {
    pub(super) fn mint_for_recovered_runner(
        local_signer: KeyPair,
        block_cadence: Duration,
    ) -> Self {
        Self {
            _seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1,
            local_signer,
            block_cadence,
        }
    }

    #[cfg(test)]
    /// Mint the same sealed dependency permit for production-shaped unit tests.
    pub(in crate::sumeragi) fn for_test(local_signer: KeyPair, block_cadence: Duration) -> Self {
        Self::mint_for_recovered_runner(local_signer, block_cadence)
    }

    /// Consume the runner seal into its authenticated factory dependencies.
    pub(in crate::sumeragi) fn into_factory_dependencies(self) -> (KeyPair, Duration) {
        (self.local_signer, self.block_cadence)
    }
}

/// Runner-private one-shot authority for activating a launched lifecycle height.
///
/// The permit retains the exact process readiness flag and fair-ingress Arc.
/// Its status authority is either the currently recovered height, an applied
/// predecessor handoff, or audited-snapshot bootstrap. CompleteTip uses the
/// separate authority below because its retired predecessor must remain joined
/// to the launched H+1 owner until this exact publication boundary.
#[must_use = "runner activation authority must be consumed by the launched lifecycle"]
pub(in crate::sumeragi) struct ProductionLifecycleRunnerActivationV1 {
    _seal: ProductionLifecycleRunnerActivationSealV1,
    pub(super) ingress_ready: Arc<AtomicBool>,
    pub(super) block_ingress: Arc<FairV2Ingress>,
    status: ProductionLifecycleRunnerStatusAuthorityV1,
}

struct ProductionLifecycleRunnerActivationSealV1;

impl Drop for ProductionLifecycleRunnerActivationSealV1 {
    fn drop(&mut self) {}
}

enum ProductionLifecycleRunnerStatusAuthorityV1 {
    CurrentHeight,
    Applied {
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    },
    SnapshotBootstrap {
        authority: SnapshotSuccessorActivationAuthority,
    },
}

impl ProductionLifecycleRunnerActivationV1 {
    /// Mint the current-height activation for the lifecycle runner.
    pub(super) fn current_height(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
            status: ProductionLifecycleRunnerStatusAuthorityV1::CurrentHeight,
        }
    }

    /// Mint an applied-predecessor successor activation without exposing parts.
    pub(super) fn applied(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
            status: ProductionLifecycleRunnerStatusAuthorityV1::Applied {
                expected_predecessor,
                authority,
            },
        }
    }

    /// Mint an audited-snapshot successor activation without exposing parts.
    pub(super) fn snapshot_bootstrap(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        authority: SnapshotSuccessorActivationAuthority,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
            status: ProductionLifecycleRunnerStatusAuthorityV1::SnapshotBootstrap { authority },
        }
    }

    /// Open the exact retained ingress, publish status, then release readiness.
    pub(in crate::sumeragi) fn open_and_publish(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
        successor: wire::SumeragiV2Status,
    ) -> Result<ProductionLifecycleActivatedRunnerAuthorityV1, V2RunnerError> {
        self.ingress_ready.store(false, Ordering::Release);
        if !Arc::ptr_eq(&self.block_ingress, launched_ingress) {
            self.block_ingress.close();
            return Err(V2RunnerError::LifecycleActivationIngressMismatch);
        }
        self.block_ingress.open().map_err(ingress_capacity_error)?;
        let publication = match self.status {
            ProductionLifecycleRunnerStatusAuthorityV1::CurrentHeight => {
                super::super::status::set_v2_status(successor);
                Ok(())
            }
            ProductionLifecycleRunnerStatusAuthorityV1::Applied {
                expected_predecessor,
                authority,
            } => super::super::status::activate_v2_successor_height(
                expected_predecessor,
                authority,
                successor,
            )
            .map_err(V2RunnerError::from),
            ProductionLifecycleRunnerStatusAuthorityV1::SnapshotBootstrap { authority } => {
                super::super::status::activate_snapshot_bootstrap_v2_height(authority, successor)
                    .map_err(V2RunnerError::from)
            }
        };
        if let Err(error) = publication {
            self.block_ingress.close();
            return Err(error);
        }
        self.ingress_ready.store(true, Ordering::Release);
        Ok(ProductionLifecycleActivatedRunnerAuthorityV1 {
            _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
            ingress_ready: self.ingress_ready,
            block_ingress: self.block_ingress,
        })
    }

    /// Consume an unpublished activation during an orderly operator shutdown.
    pub(in crate::sumeragi) fn retire_unpublished(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn current_height_for_test(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self::current_height(ingress_ready, block_ingress)
    }
}

/// Runner-private activation half for an exact launched CompleteTip successor.
#[must_use = "CompleteTip runner activation must consume its launched retirement join"]
pub(in crate::sumeragi) struct ProductionLifecycleCompleteTipRunnerActivationV1 {
    _seal: ProductionLifecycleCompleteTipRunnerActivationSealV1,
    pub(super) ingress_ready: Arc<AtomicBool>,
    pub(super) block_ingress: Arc<FairV2Ingress>,
}

struct ProductionLifecycleCompleteTipRunnerActivationSealV1;

impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleCompleteTipRunnerActivationV1 {
    /// Mint only after retired H is bound to the launched H+1 lifecycle owner.
    pub(super) fn mint_for_recovered_runner(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleCompleteTipRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
        }
    }

    /// Publish only through the still-sealed retired CompleteTip authority.
    pub(in crate::sumeragi) fn open_and_publish(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
        retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,
        successor: wire::SumeragiV2Status,
    ) -> Result<ProductionLifecycleActivatedRunnerAuthorityV1, V2RunnerError> {
        self.ingress_ready.store(false, Ordering::Release);
        if !Arc::ptr_eq(&self.block_ingress, launched_ingress) {
            self.block_ingress.close();
            return Err(V2RunnerError::LifecycleActivationIngressMismatch);
        }
        if !retirement.authorizes_successor_status(&successor) {
            self.block_ingress.close();
            return Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid {
                predecessor: retirement.predecessor(),
            });
        }
        self.block_ingress.open().map_err(ingress_capacity_error)?;
        if let Err(error) =
            super::super::status::activate_recovered_complete_tip_v2_height(retirement, successor)
        {
            self.block_ingress.close();
            return Err(error.into());
        }
        self.ingress_ready.store(true, Ordering::Release);
        Ok(ProductionLifecycleActivatedRunnerAuthorityV1 {
            _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
            ingress_ready: self.ingress_ready,
            block_ingress: self.block_ingress,
        })
    }

    /// Consume an unpublished CompleteTip activation during orderly shutdown.
    pub(in crate::sumeragi) fn retire_unpublished(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self::mint_for_recovered_runner(ingress_ready, block_ingress)
    }
}

/// Runner-private activation for one interrupted canonical Kura tip.
///
/// This authority owns the same exact readiness flag and fair-ingress Arc as
/// ordinary activation, but carries no successor-publication authority. Its
/// sole live transition publishes the recovered current-height snapshot and
/// opens transport only after the local Decision Apply is durable, without
/// arming pacemaker clocks.
#[must_use = "pending Kura activation must enter its no-clock lifecycle state"]
pub(in crate::sumeragi) struct ProductionLifecyclePendingKuraRunnerActivationV1 {
    _seal: ProductionLifecyclePendingKuraRunnerActivationSealV1,
    pub(super) ingress_ready: Arc<AtomicBool>,
    pub(super) block_ingress: Arc<FairV2Ingress>,
}

struct ProductionLifecyclePendingKuraRunnerActivationSealV1;

impl Drop for ProductionLifecyclePendingKuraRunnerActivationSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecyclePendingKuraRunnerActivationV1 {
    /// Mint beside the exact interrupted-tip lifecycle owner.
    pub(super) fn mint_for_recovered_runner(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self {
            _seal: ProductionLifecyclePendingKuraRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
        }
    }

    /// Publish the recovered current-height status and open the exact ingress.
    pub(in crate::sumeragi) fn open_and_publish_recovered_height(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
        status: wire::SumeragiV2Status,
    ) -> Result<ProductionLifecycleActivatedRunnerAuthorityV1, V2RunnerError> {
        self.ingress_ready.store(false, Ordering::Release);
        if !Arc::ptr_eq(&self.block_ingress, launched_ingress) {
            self.block_ingress.close();
            return Err(V2RunnerError::LifecycleActivationIngressMismatch);
        }
        self.block_ingress.open().map_err(ingress_capacity_error)?;
        super::super::status::set_v2_status(status);
        self.ingress_ready.store(true, Ordering::Release);
        Ok(ProductionLifecycleActivatedRunnerAuthorityV1 {
            _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
            ingress_ready: self.ingress_ready,
            block_ingress: self.block_ingress,
        })
    }

    /// Consume an unpublished interrupted-tip activation during shutdown.
    pub(in crate::sumeragi) fn retire_unpublished(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self::mint_for_recovered_runner(ingress_ready, block_ingress)
    }
}
