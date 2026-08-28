impl State {
    /// Update pipeline preferences using a loaded configuration.
    pub fn set_pipeline(&mut self, pipeline: iroha_config::parameters::actual::Pipeline) {
        self.pipeline = pipeline;
        self.pipeline_parallelism = PipelineParallelism::new(&self.pipeline);
        self.stateless_validation_cache
            .lock()
            .set_cap(self.pipeline.stateless_cache_cap);
        *self.trigger_ivm_cache.lock() = IvmCache::with_capacity(self.pipeline.cache_size);
        *self.contract_query_ivm_cache.lock() = IvmCache::with_capacity(self.pipeline.cache_size);
        *self.pipeline_ivm_prepared_cache.write() =
            PreparedContractCache::with_capacity(self.pipeline.cache_size);
        // Configure the IVM global pre-decode cache from pipeline settings.
        ivm::ivm_cache::configure_limits(ivm::ivm_cache::CacheLimits {
            capacity: self.pipeline.cache_size,
            max_bytes: self.pipeline.ivm_cache_max_bytes,
            max_decoded_ops: self.pipeline.ivm_cache_max_decoded_ops,
        });
        ivm::zk::set_prover_threads(self.pipeline.ivm_prover_threads);
    }

    #[inline]
    pub(crate) fn stateless_validation_cache(
        &self,
    ) -> &parking_lot::Mutex<StatelessValidationCache> {
        &self.stateless_validation_cache
    }

    /// Update oracle aggregation preferences.
    pub fn set_oracle(&mut self, oracle: iroha_config::parameters::actual::Oracle) {
        self.oracle = oracle;
    }

    /// Update settlement configuration snapshot and rebuild the router engine.
    pub fn set_settlement(&mut self, settlement: iroha_config::parameters::actual::Settlement) {
        self.settlement = settlement;
        self.settlement_engine = SettlementEngine::from_router_config(&self.settlement.router);
    }

    /// Install the fully authenticated immutable Kagemusha V4 release catalog.
    ///
    /// Startup calls this before Kura replay; transaction execution receives an
    /// `Arc` snapshot and never performs release filesystem access.
    pub fn set_kagemusha_release_catalog(
        &mut self,
        catalog: crate::smartcontracts::isi::offline::KagemushaReleaseCatalogV4,
    ) {
        self.kagemusha_release_catalog = Arc::new(catalog);
    }

    /// Install the immutable, startup-authenticated Kagemusha runtime projection identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an all-zero identity or an attempt to replace a
    /// previously or concurrently installed identity with different bytes.
    pub fn install_kagemusha_runtime_effective_config_sha256(
        &self,
        digest: [u8; 32],
    ) -> Result<(), String> {
        if digest == [0; 32] {
            return Err("Kagemusha runtime-effective config digest must be nonzero".to_owned());
        }
        match self.kagemusha_runtime_effective_config_sha256.set(digest) {
            Ok(()) => Ok(()),
            Err(digest)
                if self.kagemusha_runtime_effective_config_sha256.get() == Some(&digest) =>
            {
                Ok(())
            }
            Err(_) => {
                Err("Kagemusha runtime-effective config digest is already installed".to_owned())
            }
        }
    }

    /// Check one committed or prospective world against the immutable local projection.
    pub(crate) fn require_kagemusha_runtime_effective_config_for_world(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<(), String> {
        crate::smartcontracts::isi::offline::require_local_runtime_effective_config(
            world,
            self.kagemusha_runtime_effective_config_sha256
                .get()
                .copied(),
        )
    }

    /// Check the reconstructed committed world before opening consensus output.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or ambiguous active lifecycle state, a
    /// missing local projection, or any projection mismatch.
    pub fn require_committed_kagemusha_runtime_effective_config(&self) -> Result<(), String> {
        let view = self.view();
        self.require_kagemusha_runtime_effective_config_for_world(view.world())
    }

    /// Current settlement configuration snapshot.
    #[must_use]
    pub fn settlement(&self) -> &iroha_config::parameters::actual::Settlement {
        &self.settlement
    }

    /// Update Nexus configuration snapshot.
    ///
    /// # Errors
    ///
    /// Returns a `LaneLifecycleError` if lanes reference unknown dataspaces,
    /// routing policy targets cannot resolve, or geometry updates cannot be
    /// applied to the current state. A textual dataspace namespace also cannot
    /// be retired until all asset-definition alias bindings in that namespace
    /// are explicitly cleared.
    pub fn set_nexus(
        &mut self,
        nexus: iroha_config::parameters::actual::Nexus,
    ) -> Result<(), LaneLifecycleError> {
        self.ensure_config_catalog_mutation_is_pre_genesis(&nexus.lane_catalog, false)?;
        let configured_lane_catalog = self.nexus.read().configured_lane_catalog.clone();
        self.set_nexus_with_configured_lane_catalog(nexus, configured_lane_catalog, None)
    }
}
