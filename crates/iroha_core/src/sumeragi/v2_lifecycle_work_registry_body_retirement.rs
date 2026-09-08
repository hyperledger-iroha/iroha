// Exact registry joins for obsolete certified body work. This file is included
// in the registry module so installed carrier fields never become public inputs.

/// Comparison-only body identity projected from an installed ordinary carrier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct CertifiedBodyRetirementMaterialV1 {
    stage: LifecycleWorkClass,
    tag: EventTag,
    durable_receipt: DurableBodyReceipt,
    store_publication: Option<PublishedLifecycleStoreRetryCensusEntryV1>,
}

impl CertifiedBodyRetirementMaterialV1 {
    /// Borrow the original durable body, which cancellation must retain.
    pub(in crate::sumeragi) fn durable_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_receipt
    }

    /// Borrow the immutable Store publication, excluding its mutable authority overlay.
    pub(in crate::sumeragi) fn store_publication(
        &self,
    ) -> Option<&PublishedLifecycleStoreRetryCensusEntryV1> {
        self.store_publication.as_ref()
    }

    fn matches_adapter(
        &self,
        adapter: &crate::sumeragi::v2::PreparedSupersededCertifiedBodyV1<'_>,
    ) -> bool {
        use crate::sumeragi::v2::SupersededCertifiedBodyStageV1;
        let stage = match self.stage {
            LifecycleWorkClass::Fetch => SupersededCertifiedBodyStageV1::Fetch,
            LifecycleWorkClass::Store => SupersededCertifiedBodyStageV1::Store,
            _ => return false,
        };
        adapter.stage() == stage
            && adapter.old_tag() == self.tag
            && adapter.current_tag().strictly_advances(self.tag)
            && adapter.context_id() == self.durable_receipt.context_id()
            && adapter.round() == self.durable_receipt.round()
            && adapter.subject() == self.durable_receipt.subject()
            && adapter.manifest_hash() == self.durable_receipt.manifest_hash()
    }
}

/// A retirement join did not retain the exact ordinary carrier or reducer owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CertifiedBodyRetirementErrorV1 {
    /// The installed carrier, stage, body, marker or lease is different.
    InvalidCarrier,
}

/// Registry, reducer and executor-marker cuts retained until cancellation is durable.
#[must_use = "obsolete body work has not been durably cancelled"]
pub(super) struct PreparedCertifiedBodyRetirementV1<'registry, 'adapter> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    material: CertifiedBodyRetirementMaterialV1,
    adapter: crate::sumeragi::v2::PreparedSupersededCertifiedBodyV1<'adapter>,
    marker: crate::sumeragi::v2_effects::PreparedLifecycleBodyMarkerRetirementV1,
}

/// The exact carrier has crossed durable cancellation and can retire its marker.
#[must_use = "the cancelled body carrier still owns its executor marker retirement"]
pub(in crate::sumeragi) struct CancelledCertifiedBodyWorkV1 {
    marker: crate::sumeragi::v2_effects::PreparedLifecycleBodyMarkerRetirementV1,
}

impl CancelledCertifiedBodyWorkV1 {
    /// Consume only the marker bound before the cancelled ledger publication.
    pub(in crate::sumeragi) fn into_marker(
        self,
    ) -> crate::sumeragi::v2_effects::PreparedLifecycleBodyMarkerRetirementV1 {
        self.marker
    }
}

impl<'registry> PreparedCertifiedFetchExecution<'registry> {
    /// Project comparison material without releasing the installed Fetch borrow.
    pub(super) fn retirement_material(&self) -> CertifiedBodyRetirementMaterialV1 {
        let (tag, _) = self.adapter_preview_inputs();
        CertifiedBodyRetirementMaterialV1 {
            stage: LifecycleWorkClass::Fetch,
            tag,
            durable_receipt: self.durable_body_receipt().clone(),
            store_publication: None,
        }
    }

    /// Join a strictly superseded reducer preview to this exact Fetch carrier.
    pub(super) fn seal_superseded_retirement<'adapter>(
        self,
        adapter: crate::sumeragi::v2::PreparedSupersededCertifiedBodyV1<'adapter>,
        marker: crate::sumeragi::v2_effects::PreparedLifecycleBodyMarkerRetirementV1,
    ) -> Result<
        PreparedCertifiedBodyRetirementV1<'registry, 'adapter>,
        CertifiedBodyRetirementErrorV1,
    > {
        let material = self.retirement_material();
        if !material.matches_adapter(&adapter) || !marker.matches_material(&material) {
            return Err(CertifiedBodyRetirementErrorV1::InvalidCarrier);
        }
        let work = &self.registry.entries[&self.address];
        if !work.validates_at(self.address) {
            return Err(CertifiedBodyRetirementErrorV1::InvalidCarrier);
        }
        let digest = work.digest;
        Ok(PreparedCertifiedBodyRetirementV1 {
            registry: self.registry,
            address: self.address,
            digest,
            material,
            adapter,
            marker,
        })
    }
}

impl<'registry> PreparedDurableStoreExecution<'registry> {
    /// Project only an ordinary certified Store; recovered Decision work has its own owner.
    pub(super) fn retirement_material(
        &self,
    ) -> Result<CertifiedBodyRetirementMaterialV1, CertifiedBodyRetirementErrorV1> {
        let (
            DurableStoreExecutionOriginV1::Certified,
            ConcreteLifecycleWorkKind::DurableStoreBody(store),
        ) = (&self.origin, &self.installed_work().kind)
        else {
            return Err(CertifiedBodyRetirementErrorV1::InvalidCarrier);
        };
        let publication = PublishedLifecycleStoreRetryCensusEntryV1::from_exact_published_store(
            &store.effect,
            &store.pending,
            &store.durable_receipt,
        )
        .ok_or(CertifiedBodyRetirementErrorV1::InvalidCarrier)?;
        let (tag, _, _) = self.adapter_preview_inputs();
        Ok(CertifiedBodyRetirementMaterialV1 {
            stage: LifecycleWorkClass::Store,
            tag,
            durable_receipt: store.durable_receipt.clone(),
            store_publication: Some(publication),
        })
    }

    /// Join exact ordinary Store publication and strict reducer supersession.
    pub(super) fn seal_superseded_retirement<'adapter>(
        self,
        adapter: crate::sumeragi::v2::PreparedSupersededCertifiedBodyV1<'adapter>,
        marker: crate::sumeragi::v2_effects::PreparedLifecycleBodyMarkerRetirementV1,
    ) -> Result<
        PreparedCertifiedBodyRetirementV1<'registry, 'adapter>,
        CertifiedBodyRetirementErrorV1,
    > {
        let material = self.retirement_material()?;
        if !material.matches_adapter(&adapter) || !marker.matches_material(&material) {
            return Err(CertifiedBodyRetirementErrorV1::InvalidCarrier);
        }
        let work = &self.registry.entries[&self.address];
        if !work.validates_at(self.address) {
            return Err(CertifiedBodyRetirementErrorV1::InvalidCarrier);
        }
        let digest = work.digest;
        Ok(PreparedCertifiedBodyRetirementV1 {
            registry: self.registry,
            address: self.address,
            digest,
            material,
            adapter,
            marker,
        })
    }
}

impl PreparedCertifiedBodyRetirementV1<'_, '_> {
    /// Rejoin the claimed registry address, context and `BodyFrame` before staging.
    pub(super) fn project_for_cancellation(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<DurablePayloadReference, CertifiedBodyRetirementErrorV1> {
        let work = sealed_successor_parent(self.registry, self.address, lease)
            .map_err(|_| CertifiedBodyRetirementErrorV1::InvalidCarrier)?;
        if work.digest != self.digest
            || lease.work_class() != self.material.stage
            || verified.context().id() != self.adapter.context_id()
            || lease.key().context()
                != super::projection::lifecycle_context(verified.context()).id()
            || !self.material.matches_adapter(&self.adapter)
            || !self.marker.matches_material(&self.material)
        {
            return Err(CertifiedBodyRetirementErrorV1::InvalidCarrier);
        }
        durable_validate_body_payload(&self.material.durable_receipt)
            .ok_or(CertifiedBodyRetirementErrorV1::InvalidCarrier)
    }

    /// Remove only the sealed carrier after its exact cancelled ledger row is durable.
    pub(super) fn commit_after_publication(self) -> CancelledCertifiedBodyWorkV1 {
        let Self {
            registry,
            address,
            digest,
            material: _,
            adapter,
            marker,
        } = self;
        let work = registry
            .entries
            .remove(&address)
            .expect("published cancellation retains its exact obsolete carrier");
        assert_eq!(work.digest, digest);
        assert!(work.validates_at(address));
        drop(adapter);
        CancelledCertifiedBodyWorkV1 { marker }
    }
}
