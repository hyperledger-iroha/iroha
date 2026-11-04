// Read-only rollout identity from one committed view. Node configuration has no authority.
impl StateView<'_> {
    /// Return the original governed lifecycle boundary selected by this committed view.
    /// # Errors
    /// The selected runtime has no consistent committed activation lineage.
    pub fn lifecycle_activation_policy(
        &self,
    ) -> Result<crate::runtime::lifecycle_activation::CommittedLifecyclePolicy, String> {
        crate::runtime::lifecycle_activation::committed_policy(
            self.world(),
            *self.network_id(),
            self.height() as u64,
        )
    }

    /// Fingerprint original signed genesis and committed runtime activation provenance.
    /// The block-hash authority and runtime selection come from this same view.
    /// # Errors
    /// Original signed genesis is absent, unreadable, or inconsistent with the
    /// view, or the committed activation lineage is incomplete.
    pub fn lifecycle_consensus_config_fingerprint(&self) -> Result<iroha_crypto::Hash, String> {
        let expected = self.block_hashes.get(0).ok_or("genesis is not committed")?;
        let genesis = committed_block_from_kura(
            self.kura,
            std::num::NonZeroUsize::new(1).expect("one"),
            *expected,
            &self.execution_budget(),
        )
        .map_err(|error| format!("cannot read committed genesis: {error:?}"))?
        .ok_or("genesis is not committed")?;
        let genesis_fingerprint =
            iroha_data_model::sumeragi_finality::consensus_configuration_fingerprint(&genesis)
                .map_err(|error| error.to_string())?;
        Ok(
            crate::runtime::lifecycle_activation::configuration_fingerprint(
                genesis_fingerprint,
                &self.lifecycle_activation_policy()?,
            ),
        )
    }
}
