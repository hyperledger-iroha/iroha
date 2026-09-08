// Publication-marker retirement for an exact obsolete ordinary body carrier.

/// Inert executor cut joined to the registry carrier before ledger publication.
#[must_use = "body marker retirement requires durable cancellation first"]
pub(in crate::sumeragi) struct PreparedLifecycleBodyMarkerRetirementV1 {
    material: crate::sumeragi::v2_lifecycle_coordinator::CertifiedBodyRetirementMaterialV1,
}

impl PreparedLifecycleBodyMarkerRetirementV1 {
    /// Check the complete comparison identity against the installed registry cut.
    pub(in crate::sumeragi) fn matches_material(
        &self,
        material: &crate::sumeragi::v2_lifecycle_coordinator::CertifiedBodyRetirementMaterialV1,
    ) -> bool {
        &self.material == material
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    /// Preflight retirement of only the exact immutable old Store publication.
    /// Fetch has no Store child to retire and preserves all newer markers.
    pub(in crate::sumeragi) fn prepare_lifecycle_body_marker_retirement(
        &self,
        material: &crate::sumeragi::v2_lifecycle_coordinator::CertifiedBodyRetirementMaterialV1,
    ) -> Result<PreparedLifecycleBodyMarkerRetirementV1, EffectExecutorError> {
        let receipt = material.durable_receipt();
        let key = (receipt.round(), receipt.subject());
        if self.durable_bodies.get(&key) != Some(receipt)
            || !self
                .recovered_bodies
                .get(&key)
                .is_some_and(|(manifest, retained)| {
                    retained == receipt
                        && HashOf::new(manifest) == receipt.manifest_hash()
                        && manifest.round == receipt.round()
                        && manifest.subject == receipt.subject()
                })
        {
            return Err(EffectExecutorError::Contract(
                "obsolete body retirement changed its durable body".to_owned(),
            ));
        }
        if let Some(expected) = material.store_publication() {
            let observed = self
                .published_lifecycle_store_retry_markers
                .get(&key)
                .and_then(PublishedLifecycleStoreTerminalRetrySealV1::publication_census_entry);
            if observed.as_ref() != Some(expected) {
                return Err(EffectExecutorError::Contract(
                    "obsolete Store retirement changed its immutable publication".to_owned(),
                ));
            }
        }
        Ok(PreparedLifecycleBodyMarkerRetirementV1 {
            material: material.clone(),
        })
    }

    /// Commit the preflighted marker removal after the exact ledger row is cancelled.
    pub(in crate::sumeragi) fn commit_lifecycle_body_marker_retirement(
        &mut self,
        cancelled: crate::sumeragi::v2_lifecycle_coordinator::CancelledCertifiedBodyWorkV1,
    ) {
        let prepared = cancelled.into_marker();
        if let Some(expected) = prepared.material.store_publication() {
            let key = expected.key();
            assert_eq!(
                self.published_lifecycle_store_retry_markers
                    .get(&key)
                    .and_then(PublishedLifecycleStoreTerminalRetrySealV1::publication_census_entry)
                    .as_ref(),
                Some(expected),
            );
            let removed = self
                .published_lifecycle_store_retry_markers
                .remove(&key)
                .expect("cancelled Store retains its exact published marker");
            assert_eq!(removed.publication_census_entry().as_ref(), Some(expected));
        }
    }
}
