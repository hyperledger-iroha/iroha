#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedBodyPipelineReplayFamilyV1 {
    source: BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
}
/// Move-only launch proof that the exact configured genesis body was authenticated
/// before the height-one lifecycle opened.
///
/// The canonical bytes and subject may be borrowed by the executor, but a Fetch
/// replay seal can be minted only through this value. Raw block bytes or a raw
/// height context are therefore insufficient to manufacture local genesis
/// lifecycle authority.
#[derive(Debug)]
#[must_use = "installed authenticated genesis must remain with the height-one executor"]
pub(in crate::sumeragi) struct InstalledAuthenticatedGenesisReplayAuthorityV1 {
    context: wire::HeightContext,
    subject: wire::BlockSubject,
    canonical_wire: Arc<[u8]>,
}
/// Inert authenticated-genesis Fetch origin awaiting its exact Store successor.
#[derive(Debug)]
pub(in crate::sumeragi) struct AuthenticatedGenesisFetchReplayEvidenceV1 {
    coordinates: CertifiedBodyPipelineCoordinatesV1,
}
/// Inert authenticated-genesis Store origin awaiting one exact durable body frame.
#[derive(Debug)]
pub(in crate::sumeragi) struct AuthenticatedGenesisStoreReplayEvidenceV1 {
    coordinates: CertifiedBodyPipelineCoordinatesV1,
    store_pending: Arc<PendingRuntimeEffectBinding>,
}
/// Durable authenticated-genesis Store lineage awaiting its exact Validate successor.
#[derive(Debug)]
pub(in crate::sumeragi) struct AuthenticatedGenesisStoredReplayEvidenceV1 {
    family: CertifiedBodyPipelineReplayFamilyV1,
    store_pending: Arc<PendingRuntimeEffectBinding>,
}
/// Internal certified families admitted through the closed top-level `LocalBody` owner.
///
/// Both subtypes carry a complete authenticated QC, but neither represents a
/// certified Fetch response. Genesis bytes come from the opaque launch cut;
/// protected-lock bytes were already made durable by an earlier Proposal
/// pipeline whose live replay owner has since retired.
#[derive(Clone, Debug)]
enum AuthenticatedCertifiedLocalValidateFamilyV1 {
    Genesis(CertifiedBodyPipelineReplayFamilyV1),
    ProtectedLock(CertifiedBodyPipelineReplayFamilyV1),
}

impl AuthenticatedCertifiedLocalValidateFamilyV1 {
    const fn family(&self) -> &CertifiedBodyPipelineReplayFamilyV1 {
        match self {
            Self::Genesis(family) | Self::ProtectedLock(family) => family,
        }
    }

    fn family_mut(&mut self) -> &mut CertifiedBodyPipelineReplayFamilyV1 {
        match self {
            Self::Genesis(family) | Self::ProtectedLock(family) => family,
        }
    }

    fn authenticated_by_verified_height(&self, verified: &VerifiedHeightContext) -> bool {
        match self {
            Self::Genesis(family) => {
                authenticated_genesis_standalone_source(verified, &family.source)
            }
            Self::ProtectedLock(family) => {
                authenticated_refined_proposal_standalone_source(verified, &family.source)
            }
        }
    }
}

/// Internal families admitted through the closed top-level `LocalBody` owner.
#[derive(Clone, Debug)]
enum LocalValidateReplayFamilyV1 {
    Assembled(LocalBodyPipelineReplayFamilyV1),
    AuthenticatedCertified(AuthenticatedCertifiedLocalValidateFamilyV1),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedBodyPipelineCoordinatesV1 {
    tag: ReplayEventTagV1,
    certificate: wire::QuorumCertificate,
    manifest: wire::PayloadManifest,
    fetch_manifest_present: bool,
    certified_sources: Vec<PeerId>,
}

impl LifecycleReplayAuthorityV1 {
    /// Return whether this canonical authority retains a certified body origin.
    pub(super) fn is_certified_body_origin(&self) -> bool {
        matches!(
            &self.source,
            LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                origin: BodyPipelineOriginV1::Certified { .. },
                ..
            })
        )
    }
}

impl InstalledAuthenticatedGenesisReplayAuthorityV1 {
    /// Install the only process-local genesis replay authority from the opaque
    /// bootstrap seal and its exact frozen height-one context.
    pub(in crate::sumeragi) fn install(
        authenticated: &crate::sumeragi::v2_context::AuthenticatedGenesisBodyV1,
        context: &wire::HeightContext,
    ) -> Result<Self, &'static str> {
        Self::install_signed_block(authenticated.signed_block(), context)
    }
    /// Install an equivalent synthetic authority for executor fixtures.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        authenticated: &iroha_data_model::block::SignedBlock,
        context: &wire::HeightContext,
    ) -> Option<Self> {
        Self::install_signed_block(authenticated, context).ok()
    }
    fn install_signed_block(
        authenticated: &iroha_data_model::block::SignedBlock,
        context: &wire::HeightContext,
    ) -> Result<Self, &'static str> {
        if context.height != 1
            || context.parent_commit_qc.is_some()
            || context.snapshot_bootstrap.is_some()
        {
            return Err("authenticated genesis replay requires a fresh height-one context");
        }
        let proposal = authenticated.canonical_resultless_proposal();
        if proposal.header().height().get() != 1
            || proposal.header().view_change_index() != 0
            || proposal.header().prev_block_hash().is_some()
            || proposal.header().execution_context_hash().is_some()
            || proposal.execution_context().is_some()
            || !proposal.is_resultless_proposal()
        {
            return Err("authenticated genesis replay body is not canonical");
        }
        let canonical_wire: Arc<[u8]> = proposal
            .encode_wire()
            .map_err(|_| "authenticated genesis replay wire encoding failed")?
            .into();
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: proposal.hash(),
            payload_hash: Hash::new(canonical_wire.as_ref()),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        crate::sumeragi::v2_chunks::encode_payload(
            context,
            round,
            subject,
            canonical_wire.as_ref(),
        )
        .map_err(|_| "authenticated genesis replay body does not fit the frozen DA layout")?;
        Ok(Self {
            context: context.clone(),
            subject,
            canonical_wire,
        })
    }
    /// Borrow the exact canonical resultless genesis subject.
    pub(in crate::sumeragi) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }
    /// Borrow the exact canonical resultless genesis wire.
    pub(in crate::sumeragi) fn canonical_wire(&self) -> &Arc<[u8]> {
        &self.canonical_wire
    }
    /// Seal one exact certified Fetch under the installed local genesis body.
    pub(super) fn seal_exact_fetch(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        manifest: &wire::PayloadManifest,
    ) -> Option<AuthenticatedGenesisFetchReplayEvidenceV1> {
        let coordinates = exact_certified_fetch_coordinates_from_manifest(effect, manifest)?;
        let expected_sources = self
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let expected_manifest = crate::sumeragi::v2_chunks::encode_payload(
            &self.context,
            manifest.round,
            self.subject,
            self.canonical_wire.as_ref(),
        )
        .ok()?
        .manifest()
        .clone();
        if self.context.height != 1
            || self.context.parent_commit_qc.is_some()
            || self.context.snapshot_bootstrap.is_some()
            || coordinates.certificate.validate(&self.context).is_err()
            || coordinates.certificate.round.context_id != self.context.id()
            || coordinates.certificate.round.height != 1
            || coordinates.certificate.proposal_round != manifest.round
            || coordinates.certificate.subject != self.subject
            || manifest.subject.parent_block_hash.is_some()
            || manifest != &expected_manifest
            || coordinates.manifest != expected_manifest
            || coordinates.certified_sources != expected_sources
            || !pending.exactly_binds_adapter_effect(effect)
        {
            return None;
        }
        let source = BodyPipelineReplaySourceV1 {
            tag: coordinates.tag,
            origin: BodyPipelineOriginV1::Certified {
                certificate: coordinates.certificate.clone(),
                manifest: coordinates.manifest.clone(),
                fetch_manifest_present: coordinates.fetch_manifest_present,
                certified_sources: coordinates.certified_sources.clone(),
            },
        };
        canonical_replay_authority(
            replay_context(coordinates.certificate.round),
            LifecycleReplaySourceV1::BodyPipeline(source),
            LifecycleStageKind::FetchBody,
            ReplayPayloadBindingV1::None,
        )?;
        Some(AuthenticatedGenesisFetchReplayEvidenceV1 { coordinates })
    }
}

impl AuthenticatedGenesisFetchReplayEvidenceV1 {
    /// Recheck the exact certified Fetch coordinates without accepting a new body source.
    pub(super) fn exactly_matches_fetch(&self, effect: &AdapterEffect) -> bool {
        &certified_fetch_effect_from_coordinates(&self.coordinates) == effect
    }
    /// Recheck the exact Fetch together with its inherited runtime owner.
    pub(super) fn exactly_matches_fetch_pending(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.exactly_matches_fetch(effect) && pending.exactly_binds_adapter_effect(effect)
    }
    #[allow(clippy::result_large_err)]
    pub(super) fn project_store(
        self,
        fetch_effect: &AdapterEffect,
        fetch_pending: &PendingRuntimeEffectBinding,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
    ) -> Result<AuthenticatedGenesisStoreReplayEvidenceV1, Self> {
        if !self.exactly_matches_fetch_pending(fetch_effect, fetch_pending)
            || fetch_pending
                .project_certified_fetch_store_successor(fetch_effect, store_effect)
                .as_ref()
                != Some(store_pending)
        {
            return Err(self);
        }
        let projected = fetch_pending
            .project_certified_fetch_store_successor(fetch_effect, store_effect)
            .expect("an exact authenticated-genesis Fetch has one Store successor");
        Ok(AuthenticatedGenesisStoreReplayEvidenceV1 {
            coordinates: self.coordinates,
            store_pending: Arc::new(projected),
        })
    }
}

impl AuthenticatedGenesisStoreReplayEvidenceV1 {
    /// Recheck the original certified Fetch after it has projected Store.
    pub(super) fn exactly_matches_origin_fetch(&self, effect: &AdapterEffect) -> bool {
        &certified_fetch_effect_from_coordinates(&self.coordinates) == effect
    }
    /// Recheck the exact Store together with its inherited runtime owner.
    pub(super) fn exactly_matches_store_pending(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        &certified_store_effect_from_coordinates(&self.coordinates) == effect
            && pending.exactly_binds_adapter_effect(effect)
            && self.store_pending.as_ref() == pending
    }
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_durable_body(
        self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Result<AuthenticatedGenesisStoredReplayEvidenceV1, Self> {
        if !self.exactly_matches_store_pending(effect, pending) {
            return Err(self);
        }
        let Some(family) = exact_certified_body_pipeline_family(&self.coordinates, receipt) else {
            return Err(self);
        };
        Ok(AuthenticatedGenesisStoredReplayEvidenceV1 {
            family,
            store_pending: self.store_pending,
        })
    }
}

impl AuthenticatedGenesisStoredReplayEvidenceV1 {
    /// Recheck the original certified Fetch after the body became durable.
    pub(super) fn exactly_matches_origin_fetch(&self, effect: &AdapterEffect) -> bool {
        exact_certified_fetch_effect(&self.family).as_ref() == Some(effect)
    }
    /// Recheck the exact durable Store owner without releasing its replay family.
    pub(super) fn exactly_matches_store_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        certified_body_stage_matches(&self.family, effect, receipt, LifecycleStageKind::StoreBody)
            && pending.exactly_binds_adapter_effect(effect)
            && self.store_pending.as_ref() == pending
    }
    /// Preflight the only authority-monotone Store-to-Validate projection.
    pub(super) fn exactly_projects_validate(
        &self,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.exactly_matches_store_pending(store_effect, receipt, store_pending)
            && certified_body_stage_matches(
                &self.family,
                validate_effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
            && store_pending
                .project_store_validate_successor_with_authority_refinement(
                    store_effect,
                    validate_effect,
                    validate_pending,
                )
                .as_ref()
                == Some(validate_pending)
    }
    /// Consume the exact durable Store lineage into authenticated-genesis Validate evidence.
    #[allow(clippy::result_large_err)]
    pub(super) fn project_validate(
        self,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Result<LocalValidateReplayEvidenceV1, Self> {
        if !self.exactly_projects_validate(
            store_effect,
            store_pending,
            receipt,
            validate_effect,
            validate_pending,
        ) {
            return Err(self);
        }
        let projected = store_pending
            .project_store_validate_successor_with_authority_refinement(
                store_effect,
                validate_effect,
                validate_pending,
            )
            .expect("an exact authenticated-genesis Store has one Validate successor");
        Ok(LocalValidateReplayEvidenceV1 {
            family: LocalValidateReplayFamilyV1::AuthenticatedCertified(
                AuthenticatedCertifiedLocalValidateFamilyV1::Genesis(self.family),
            ),
            validate_pending: Arc::new(projected),
        })
    }
}

impl LocalValidateReplayFamilyV1 {
    fn authenticated_by_verified_height(&self, verified: &VerifiedHeightContext) -> bool {
        match self {
            Self::Assembled(_) => true,
            Self::AuthenticatedCertified(certified) => {
                certified.authenticated_by_verified_height(verified)
            }
        }
    }

    #[cfg(test)]
    fn assembled_body_frame_mut_for_test(&mut self) -> &mut BodyFrameBindingV1 {
        let Self::Assembled(family) = self else {
            panic!("assembled local-body fixture changed replay subtype")
        };
        &mut family.body_frame
    }
    #[cfg(test)]
    fn is_exact_for_stage_for_test(&self, stage: LifecycleStageKind) -> bool {
        match self {
            Self::Assembled(family) => family.is_exact_for_stage(stage),
            Self::AuthenticatedCertified(certified) => certified.family().is_exact_for_stage(stage),
        }
    }
    fn source_and_frame(&self) -> (&BodyPipelineReplaySourceV1, BodyFrameBindingV1) {
        match self {
            Self::Assembled(family) => (&family.source, family.body_frame),
            Self::AuthenticatedCertified(certified) => {
                let family = certified.family();
                (&family.source, family.body_frame)
            }
        }
    }
    fn source_mut(&mut self) -> &mut BodyPipelineReplaySourceV1 {
        match self {
            Self::Assembled(family) => &mut family.source,
            Self::AuthenticatedCertified(certified) => &mut certified.family_mut().source,
        }
    }
    fn exactly_matches_validate(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        match self {
            Self::Assembled(family) => {
                local_body_stage_matches(family, effect, receipt, LifecycleStageKind::ValidateBody)
            }
            Self::AuthenticatedCertified(certified) => certified_body_stage_matches(
                certified.family(),
                effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            ),
        }
    }
    fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        match self {
            Self::Assembled(family) => {
                exact_local_body_pipeline_family(&family.source, receipt)
                    .is_some_and(|expected| expected == *family)
                    && family.is_exact_for_stage(LifecycleStageKind::ValidateBody)
            }
            Self::AuthenticatedCertified(certified) => {
                let family = certified.family();
                exact_family_coordinates(family)
                    .and_then(|coordinates| certified_body_pipeline_family(&coordinates, receipt))
                    .is_some_and(|expected| {
                        expected == *family
                            && family.is_exact_for_stage(LifecycleStageKind::ValidateBody)
                    })
            }
        }
    }
    fn assembled_manifest(&self) -> Option<&wire::PayloadManifest> {
        let Self::Assembled(family) = self else {
            return None;
        };
        let BodyPipelineOriginV1::LocalBody(manifest) = &family.source.origin else {
            return None;
        };
        Some(manifest)
    }
    fn authenticated_certified_family(&self) -> Option<&CertifiedBodyPipelineReplayFamilyV1> {
        match self {
            Self::AuthenticatedCertified(certified) => Some(certified.family()),
            Self::Assembled(_) => None,
        }
    }
}

impl LocalValidateReplayEvidenceV1 {
    /// Reseal one historical protected-lock body whose original Proposal replay
    /// owner retired before this exact Validate turn.
    ///
    /// The runtime binding must already carry the same Prepare statement. The
    /// complete QC is retained in the certified replay source and is verified
    /// again at lifecycle admission (and again by cold standalone recovery).
    pub(in crate::sumeragi) fn from_exact_protected_lock_validate(
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
        certificate: &wire::QuorumCertificate,
        pending: PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = effect
        else {
            return None;
        };
        let statement = pending.candidate_statement()?;
        if !pending.exactly_binds_adapter_effect(effect)
            || certificate.phase != wire::GlobalPhase::Prepare
            || certificate.round != *round
            || certificate.proposal_round != manifest.round
            || certificate.subject != *subject
            || manifest.round != *round
            || manifest.subject != *subject
            || statement.context_id() != certificate.round.context_id
            || statement.round() != certificate.round
            || statement.proposal_round() != certificate.proposal_round
            || statement.subject() != Some(certificate.subject)
            || statement.phase() != Some(wire::GlobalPhase::Prepare)
            || statement.execution_commitment() != Some(certificate.execution_commitment)
        {
            return None;
        }
        let coordinates = CertifiedBodyPipelineCoordinatesV1 {
            tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
            certificate: certificate.clone(),
            manifest: manifest.clone(),
            fetch_manifest_present: true,
            certified_sources: Vec::new(),
        };
        let family = exact_certified_body_pipeline_family(&coordinates, receipt)?;
        if !certified_body_stage_matches(&family, effect, receipt, LifecycleStageKind::ValidateBody)
        {
            return None;
        }
        let evidence = Self {
            family: LocalValidateReplayFamilyV1::AuthenticatedCertified(
                AuthenticatedCertifiedLocalValidateFamilyV1::ProtectedLock(family),
            ),
            validate_pending: Arc::new(pending),
        };
        evidence
            .exactly_matches_validate_pending(effect, receipt, evidence.validate_pending.as_ref())
            .then_some(evidence)
    }

    /// Report whether this closed test-only carrier may project a local-proposal handoff.
    #[cfg(test)]
    pub(super) fn projects_local_proposal_handoff_for_test(&self) -> bool {
        matches!(self.family, LocalValidateReplayFamilyV1::Assembled(_))
    }
    /// Match only the replay authority canonically derived from this internal local subtype.
    pub(super) fn exactly_authorizes_local_admission_authority(
        &self,
        active_context: LifecycleContext,
        authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let (source, body_frame) = self.family.source_and_frame();
        let origin_class_is_exact = match &self.family {
            LocalValidateReplayFamilyV1::Assembled(_) => authority.is_local_body_origin(),
            LocalValidateReplayFamilyV1::AuthenticatedCertified(_) => {
                matches!(&source.origin, BodyPipelineOriginV1::Certified { .. })
            }
        };
        origin_class_is_exact
            && canonical_replay_authority(
                active_context,
                LifecycleReplaySourceV1::BodyPipeline(source.clone()),
                LifecycleStageKind::ValidateBody,
                ReplayPayloadBindingV1::BodyFrame(body_frame),
            )
            .as_ref()
                == Some(authority)
    }
}

fn remote_proposal_origin_matches_fetch(
    authenticated: &crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: &RuntimeIngressOwnershipEvidence,
    source: &BodyPipelineReplaySourceV1,
    effect: &AdapterEffect,
) -> bool {
    remote_proposal_fetch_effect(source).as_ref() == Some(effect)
        && exact_remote_proposal_fetch(authenticated, ingress, effect).is_some()
}

impl RemoteProposalStoreReplayEvidenceV1 {
    /// Recheck the exact ordinary Fetch which authenticated this later Store family.
    pub(in crate::sumeragi) fn exactly_matches_origin_fetch(&self, effect: &AdapterEffect) -> bool {
        remote_proposal_origin_matches_fetch(
            &self.authenticated,
            &self.ingress,
            &self.source,
            effect,
        )
    }
}

impl RemoteProposalStoredReplayEvidenceV1 {
    /// Recheck the exact ordinary Fetch which authenticated this durable family.
    pub(in crate::sumeragi) fn exactly_matches_origin_fetch(&self, effect: &AdapterEffect) -> bool {
        self.family.exactly_matches_origin_fetch(effect)
    }
}

impl RemoteProposalBodyPipelineReplayFamilyV1 {
    fn exactly_matches_origin_fetch(&self, effect: &AdapterEffect) -> bool {
        self.is_exact_for_stage(LifecycleStageKind::StoreBody)
            && remote_proposal_origin_matches_fetch(
                &self.authenticated,
                &self.ingress,
                &self.source,
                effect,
            )
    }
}

impl AuthenticatedCertifiedFetchReplayOriginV1 {
    /// Bind the exact selector-authenticated response to its pending Fetch.
    pub(super) fn from_completion_authority(
        authority: &CertifiedFetchCompletionAuthority<'_>,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        let response = authority.authenticated_response();
        if authority.request_hash() != response.request_hash
            || authority.response_hash() != HashOf::new(response)
            || !authority
                .candidate_pending()
                .exactly_binds_adapter_effect(effect)
        {
            return None;
        }
        Some(Self {
            coordinates: exact_certified_fetch_coordinates(effect, response)?,
            request_hash: authority.request_hash(),
            response_hash: authority.response_hash(),
        })
    }
    /// Consume the authenticated origin into one frame-bound canonical family.
    pub(super) fn bind_durable_body(
        self,
        receipt: &DurableCertifiedFetchBodyReceipt,
    ) -> Option<CertifiedFetchReplayEvidenceV1> {
        if receipt.request_hash() != self.request_hash
            || receipt.response_hash() != self.response_hash
        {
            return None;
        }
        Some(CertifiedFetchReplayEvidenceV1 {
            family: exact_certified_body_pipeline_family(
                &self.coordinates,
                receipt.durable_body(),
            )?,
        })
    }
}
impl CertifiedFetchReplayEvidenceV1 {
    /// Reauthenticate the persisted certificate and exact archive-source order
    /// against the immutable height context before restart authority is minted.
    fn authenticated_by_verified_height(&self, verified: &VerifiedHeightContext) -> bool {
        let BodyPipelineOriginV1::Certified {
            certificate,
            certified_sources,
            ..
        } = &self.family.source.origin
        else {
            return false;
        };
        let expected_sources = verified
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        replay_context(certificate.round)
            == super::projection::lifecycle_context(verified.context())
            && certified_sources == &expected_sources
            && verified.verify_quorum_certificate(certificate).is_ok()
    }
    fn exactly_matches_fetch_body(
        &self,
        effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        exact_certified_fetch_coordinates(effect, response)
            .and_then(|coordinates| certified_body_pipeline_family(&coordinates, receipt))
            .is_some_and(|expected| {
                expected == self.family
                    && self
                        .family
                        .is_exact_for_stage(LifecycleStageKind::FetchBody)
            })
    }
    /// Close this family over the exact incumbent runtime binding and durable frame.
    pub(super) fn project_durable_ready_fetch(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Option<DurableCertifiedFetchReplayProjectionV1> {
        if exact_certified_fetch_effect(&self.family).as_ref() != Some(effect)
            || !pending.exactly_binds_adapter_effect(effect)
            || !certified_body_pipeline_family(&exact_family_coordinates(&self.family)?, receipt)
                .is_some_and(|expected| expected == self.family)
        {
            return None;
        }
        durable_certified_fetch_projection(&self.family, effect, pending, receipt)
    }
    /// Reconstruct the exact Fetch effect and pending binding from a durable owner.
    ///
    /// Decoded replay data alone cannot invoke the pending constructor: the
    /// one-shot permit is minted only while this frame-bound evidence remains
    /// intact.
    fn reconstruct_exact_fetch(
        &self,
        causal_root: CausalRoot,
    ) -> Option<(AdapterEffect, PendingRuntimeEffectBinding)> {
        let effect = exact_certified_fetch_effect(&self.family)?;
        let pending = PendingRuntimeEffectBinding::from_durable_certified_fetch(
            DurableCertifiedFetchPendingMintPermit::new(),
            Hash::prehashed(*causal_root.digest().as_bytes()),
            &effect,
        )?;
        (digest_from_hash(pending.causal_lifecycle_key()) == causal_root.digest()
            && self
                .family
                .is_exact_for_stage(LifecycleStageKind::FetchBody))
        .then_some((effect, pending))
    }
    /// Authenticate one opened body-store seal against this exact replay family.
    pub(super) fn exactly_matches_recovered_body_frame(
        &self,
        reference: &DurableBodyFrameReference,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        let Some(coordinates) = exact_family_coordinates(&self.family) else {
            return false;
        };
        coordinates.manifest == *manifest
            && self.family.body_frame.durable_reference() == *reference
            && durable_body_frame_reference(replay_context(receipt.round()), receipt)
                == Some(*reference)
            && certified_body_pipeline_family(&coordinates, receipt)
                .is_some_and(|expected| expected == self.family)
    }
    /// Derive the direct adapter preview inputs from the sealed durable family.
    pub(super) fn adapter_preview_inputs<'a>(
        &'a self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Option<(EventTag, &'a wire::PayloadManifest)> {
        let _ready_projection = self.project_durable_ready_fetch(effect, pending, receipt)?;
        let BodyPipelineOriginV1::Certified { manifest, .. } = &self.family.source.origin else {
            return None;
        };
        Some((
            EventTag::new(
                self.family.source.tag.height,
                self.family.source.tag.view,
                crate::sumeragi::v2_core::Generation::new(self.family.source.tag.generation),
            ),
            manifest,
        ))
    }
    #[cfg(test)]
    fn exactly_matches_signed_response_for_test(
        &self,
        effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        signature_present(&response.signature)
            && self.exactly_matches_fetch_body(effect, response, receipt)
    }
    /// Project the fixed Store-stage evidence without exposing source parts.
    pub(super) fn project_store(
        &self,
        fetch_effect: &AdapterEffect,
        fetch_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        store_effect: &AdapterEffect,
    ) -> Option<CertifiedStoreReplayEvidenceV1> {
        (self
            .project_durable_ready_fetch(fetch_effect, fetch_pending, receipt)
            .is_some()
            && certified_body_stage_matches(
                &self.family,
                store_effect,
                receipt,
                LifecycleStageKind::StoreBody,
            ))
        .then(|| CertifiedStoreReplayEvidenceV1 {
            family: self.family.clone(),
        })
    }
    #[cfg(test)]
    pub(super) fn from_signed_response_for_test(
        fetch_effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
    ) -> Option<Self> {
        if !signature_present(&response.signature) {
            return None;
        }
        Some(Self {
            family: exact_certified_body_pipeline_family(
                &exact_certified_fetch_coordinates(fetch_effect, response)?,
                receipt,
            )?,
        })
    }
    #[cfg(test)]
    pub(super) fn project_store_for_test(
        &self,
        store_effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> Option<CertifiedStoreReplayEvidenceV1> {
        certified_body_stage_matches(
            &self.family,
            store_effect,
            receipt,
            LifecycleStageKind::StoreBody,
        )
        .then(|| CertifiedStoreReplayEvidenceV1 {
            family: self.family.clone(),
        })
    }
}
impl DurableCertifiedFetchReplayProjectionV1 {
    /// Compare the complete frame-bound projection with one logical restart row.
    pub(super) fn exactly_matches_recovered_candidate(
        &self,
        candidate: &CandidateAdmission,
        owner: OwnerId,
    ) -> bool {
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Fetch.capacity_class(), 0);
        candidate.key.phase() == LifecyclePhase::Fetch
            && candidate.causal_root == owner.causal_root()
            && candidate.work_class == LifecycleWorkClass::Fetch
            && candidate.stage
                == LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent)
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.reconstruction_source == owner.causal_root().digest()
            && candidate.payload == self.payload
            && candidate.replay_authority == self.authority
            && candidate.producer_turn.is_none()
            && self.causal_key == Hash::prehashed(*owner.causal_root().digest().as_bytes())
            && self.authority.structurally_matches_record(
                LifecycleContext::new(candidate.key.context(), candidate.key.round().height()),
                candidate.key,
                candidate.work_class,
                candidate.stage,
                candidate.payload,
            )
            && candidate.physical_geometry.normalized().is_ok_and(
                |(physical, universe, consumed)| {
                    physical.len() == 1
                        && physical.get(&slot) == Some(&self.completion_digest)
                        && universe == std::collections::BTreeSet::from([slot])
                        && consumed == universe
                },
            )
    }
    /// Canonical physical identity of the body-fsynced completion.
    pub(super) const fn completion_digest(&self) -> LifecycleDigest {
        self.completion_digest
    }
    /// Exact manifest hash retained independently by the body-store receipt.
    pub(super) const fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.expected_manifest_hash
    }
    /// Project the exact Ready recovery candidate named by one durable row.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn project_recovered_candidate(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        owner: OwnerId,
        stage: LifecycleStage,
        reconstruction_source: LifecycleDigest,
        payload: DurablePayloadReference,
        persisted_authority: &LifecycleReplayAuthorityV1,
    ) -> Option<CandidateAdmission> {
        if stage
            != LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent)
            || !self.exactly_matches_durable_record(
                active_context,
                key,
                owner.causal_root(),
                payload,
                reconstruction_source,
                persisted_authority,
            )
        {
            return None;
        }
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Fetch.capacity_class(), 0);
        let candidate = CandidateAdmission::new(
            key,
            owner.causal_root(),
            LifecycleWorkClass::Fetch,
            stage,
            InitialLifecycleState::Ready,
            reconstruction_source,
            self.payload,
            self.authority.clone(),
            PhysicalGeometry::new([PhysicalSlot::new(slot, self.completion_digest)], [slot]),
            None,
        );
        candidate
            .replay_authority_is_exact(active_context)
            .then_some(candidate)
    }
    /// Rebind only the durable fields of an exact Waiting Fetch row.
    pub(super) fn rebind_waiting_fetch_metadata(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        metadata: &mut DurableRecordMetadata,
    ) -> bool {
        if key.phase() != LifecyclePhase::Fetch
            || metadata.payload != DurablePayloadReference::None
            || metadata.reconstruction_source != digest_from_hash(&self.causal_key)
            || metadata.continuation != super::schema::DurableContinuation::None
            || !metadata
                .replay_authority
                .same_persisted_family(&self.authority)
            || !self.authority.structurally_matches_record(
                active_context,
                key,
                LifecycleWorkClass::Fetch,
                LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
                self.payload,
            )
        {
            return false;
        }
        metadata.payload = self.payload;
        metadata.replay_authority = self.authority.clone();
        true
    }
    /// Compare a recovered ledger row without exposing authority parts.
    pub(super) fn exactly_matches_durable_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        causal_root: CausalRoot,
        metadata_payload: DurablePayloadReference,
        reconstruction_source: LifecycleDigest,
        authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        self.payload == metadata_payload
            && self.authority == *authority
            && reconstruction_source == causal_root.digest()
            && self.causal_key == Hash::prehashed(*causal_root.digest().as_bytes())
            && self.authority.structurally_matches_record(
                active_context,
                key,
                LifecycleWorkClass::Fetch,
                LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
                self.payload,
            )
    }
}
/// Consume the sole opened-ledger/body-store join into restart authority.
#[allow(clippy::too_many_arguments)]
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn authenticate_recovered_durable_certified_fetch<
    F,
>(
    _permit: DurableCertifiedFetchLedgerJoinPermit,
    verified: &VerifiedHeightContext,
    key: LifecycleKey,
    owner: OwnerId,
    ordinal: u128,
    stage: LifecycleStage,
    reconstruction_source: LifecycleDigest,
    payload: DurablePayloadReference,
    authority: &LifecycleReplayAuthorityV1,
    authenticate_body: F,
) -> Result<Option<AuthenticatedRecoveredDurableCertifiedFetchV1>, DurableBodyFrameRecoveryError>
where
    F: FnOnce() -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError>,
{
    let active_context = super::projection::lifecycle_context(verified.context());
    if ordinal == 0
        || owner.first_admission_ordinal() == 0
        || owner.first_admission_ordinal() > ordinal
        || reconstruction_source != owner.causal_root().digest()
    {
        return Ok(None);
    }
    let Some(evidence) =
        authority.recover_durable_certified_fetch(active_context, key, stage, payload)
    else {
        return Ok(None);
    };
    if !evidence.authenticated_by_verified_height(verified) {
        return Ok(None);
    }
    // The body-store seal is minted only after the retained source list and QC
    // have been authenticated by the immutable verified height context.
    let body = authenticate_body()?;
    let Some(durable_receipt) = body.into_certified_fetch_body(&evidence) else {
        return Ok(None);
    };
    let Some((effect, pending)) = evidence.reconstruct_exact_fetch(owner.causal_root()) else {
        return Ok(None);
    };
    let Some(ready_projection) =
        evidence.project_durable_ready_fetch(&effect, &pending, &durable_receipt)
    else {
        return Ok(None);
    };
    let Some(candidate) = ready_projection.project_recovered_candidate(
        active_context,
        key,
        owner,
        stage,
        reconstruction_source,
        payload,
        authority,
    ) else {
        return Ok(None);
    };
    let Ok(completion) = CertifiedFetchCompletion::from_recovered_durable_fetch(
        owner,
        ordinal,
        effect,
        pending,
        durable_receipt,
        evidence,
        &ready_projection,
    ) else {
        return Ok(None);
    };
    let recovered = AuthenticatedRecoveredDurableCertifiedFetchV1 {
        completion,
        candidate,
    };
    Ok(recovered.is_exact().then_some(recovered))
}
/// Consume the sole opened-ledger/body-store join for one standalone local or
/// signed-Proposal Validate row.
#[allow(clippy::too_many_arguments)]
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn authenticate_recovered_durable_standalone_validate<
    F,
>(
    _permit: DurableStandaloneValidateLedgerJoinPermit,
    verified: &VerifiedHeightContext,
    key: LifecycleKey,
    owner: OwnerId,
    ordinal: u128,
    stage: LifecycleStage,
    reconstruction_source: LifecycleDigest,
    payload: DurablePayloadReference,
    authority: &LifecycleReplayAuthorityV1,
    authenticate_body: F,
) -> Result<Option<AuthenticatedRecoveredDurableStandaloneValidateV1>, DurableBodyFrameRecoveryError>
where
    F: FnOnce() -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError>,
{
    let active_context = super::projection::lifecycle_context(verified.context());
    if ordinal == 0
        || owner.first_admission_ordinal() != ordinal
        || reconstruction_source != owner.causal_root().digest()
    {
        return Ok(None);
    }
    let Some(source) =
        authority.recover_durable_standalone_validate(active_context, key, stage, payload)
    else {
        return Ok(None);
    };
    match &source.source.origin {
        BodyPipelineOriginV1::Proposal(proposal) => {
            let message = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            ));
            if verified.verify_consensus_message(&message).is_err() {
                return Ok(None);
            }
        }
        BodyPipelineOriginV1::Certified { .. } => {
            if !authenticated_genesis_standalone_source(verified, &source.source)
                && !authenticated_refined_proposal_standalone_source(verified, &source.source)
            {
                return Ok(None);
            }
        }
        BodyPipelineOriginV1::LocalBody(_) => {}
        BodyPipelineOriginV1::RecoveredDecision { .. } => {
            return Ok(None);
        }
    }
    let body = authenticate_body()?;
    let Some(durable_receipt) = body.into_standalone_validate_body(&source) else {
        return Ok(None);
    };
    let Some(effect) = standalone_validate_effect(&source.source) else {
        return Ok(None);
    };
    let certified_predecessor = match &source.source.origin {
        BodyPipelineOriginV1::Certified { certificate, .. } => Some(certificate),
        BodyPipelineOriginV1::Proposal(_)
        | BodyPipelineOriginV1::LocalBody(_)
        | BodyPipelineOriginV1::RecoveredDecision { .. } => None,
    };
    let Some(pending) = PendingRuntimeEffectBinding::from_durable_standalone_validate(
        DurableStandaloneValidatePendingMintPermit::new(),
        Hash::prehashed(*owner.causal_root().digest().as_bytes()),
        &effect,
        certified_predecessor,
    ) else {
        return Ok(None);
    };
    let Some(replay_evidence) =
        RecoveredStandaloneValidateReplayEvidenceV1::from_authenticated_source(
            source,
            &effect,
            &durable_receipt,
            &pending,
        )
    else {
        return Ok(None);
    };
    let manifest = standalone_origin_manifest(&replay_evidence.source)
        .expect("standalone recovery accepted only local, Proposal, or genesis origins")
        .clone();
    let tag = match &effect {
        AdapterEffect::ValidateBody { tag, .. } => *tag,
        _ => unreachable!("standalone recovery reconstructed ValidateBody"),
    };
    let (carrier, candidate) = match DurableValidateBody::from_recovered_standalone_validate(
        owner,
        ordinal,
        effect.clone(),
        pending,
        durable_receipt.clone(),
        DurableValidateReplayEvidenceV1::recovered_standalone(replay_evidence),
        verified,
    ) {
        Ok(recovered) => recovered,
        Err(()) => return Ok(None),
    };
    if candidate.key != key
        || candidate.causal_root != owner.causal_root()
        || candidate.stage != stage
        || candidate.reconstruction_source != reconstruction_source
        || candidate.payload != payload
        || candidate.replay_authority != *authority
    {
        return Ok(None);
    }
    let mut replay_steps = Vec::with_capacity(2);
    if matches!(
        &authority.source,
        LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            origin: BodyPipelineOriginV1::Proposal(_) | BodyPipelineOriginV1::Certified { .. },
            ..
        })
    ) {
        let store = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let Some(step) = CertifiedBodyPipelineColdReplayStepV1::body_available(
            ordinal,
            tag,
            manifest.clone(),
            store,
        ) else {
            return Ok(None);
        };
        replay_steps.push(step);
    }
    let Some(step) =
        CertifiedBodyPipelineColdReplayStepV1::body_stored(ordinal, tag, durable_receipt, effect)
    else {
        return Ok(None);
    };
    replay_steps.push(step);
    let recovered = AuthenticatedRecoveredDurableStandaloneValidateV1 {
        candidate,
        carrier,
        replay_steps,
    };
    Ok(recovered.is_exact().then_some(recovered))
}
impl CertifiedStoreReplayEvidenceV1 {
    /// Compare one installed Store carrier with its complete reconstructed row.
    pub(super) fn exactly_matches_recovered_record(
        &self,
        active_context: LifecycleContext,
        record: &LifecycleRecord,
        metadata: &DurableRecordMetadata,
        installed_digest: LifecycleDigest,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        body_stage_matches_recovered_record(
            &self.family.source,
            self.family.body_frame,
            active_context,
            record,
            metadata,
            installed_digest,
            effect,
            receipt,
            pending,
            LifecycleWorkClass::Store,
            LifecycleStageKind::StoreBody,
        )
    }
    /// Compare this canonical family with one exact durable Store carrier.
    pub(super) fn exactly_matches_store(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        certified_body_stage_matches(&self.family, effect, receipt, LifecycleStageKind::StoreBody)
    }
    /// Project one installed Store carrier without exposing its replay family.
    ///
    /// The registry-only one-shot permit proves the evidence, durable frame,
    /// concrete effect, and pending binding still reside in one closed carrier.
    pub(in crate::sumeragi) fn project_installed_store_candidate(
        &self,
        _permit: InstalledBodyCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_store_candidate(verified, effect, receipt, pending)
    }
    /// Project one Store successor still sealed under its exact Fetch parent.
    pub(in crate::sumeragi) fn project_sealed_store_successor_candidate(
        &self,
        _permit: SealedBodySuccessorProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_store_candidate(verified, effect, receipt, pending)
    }
    fn project_exact_store_candidate(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches_store(effect, receipt) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = replay_context(receipt.round());
        let payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(active_context, receipt)
                .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?,
        );
        let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
        if payload_binding != ReplayPayloadBindingV1::BodyFrame(self.family.body_frame) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let authority = canonical_replay_authority(
            active_context,
            LifecycleReplaySourceV1::BodyPipeline(self.family.source.clone()),
            LifecycleStageKind::StoreBody,
            payload_binding,
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?;
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )?;
        candidate_from_authorized_projection(active_context, projected, payload, authority)
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }
    /// Project the canonical Store candidate for focused transition tests.
    #[cfg(test)]
    pub(super) fn project_candidate_for_test(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_store_candidate(verified, effect, receipt, pending)
    }
    /// Replace only the retained event origin in a negative test fixture.
    #[cfg(test)]
    pub(super) fn replace_with_foreign_origin_for_test(&mut self) -> bool {
        let previous = self.family.source.tag;
        self.family.source.tag.generation = previous.generation.wrapping_add(1);
        self.family.source.tag != previous
    }
    /// Project the fixed Validate-stage evidence without exposing source parts.
    pub(super) fn project_validate(
        &self,
        store_effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Option<CertifiedValidateReplayEvidenceV1> {
        if !self.exactly_matches_store(store_effect, receipt)
            || !certified_body_stage_matches(
                &self.family,
                validate_effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
        {
            return None;
        }
        Some(CertifiedValidateReplayEvidenceV1 {
            family: self.family.clone(),
            validate_pending: DirectSignedPendingBindingV1::from_exact_effect(
                validate_effect,
                validate_pending,
            )?,
        })
    }
}
impl CertifiedValidateReplayEvidenceV1 {
    /// Compare this canonical family and causal root with one exact Validate carrier.
    fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        certified_body_stage_matches(
            &self.family,
            effect,
            receipt,
            LifecycleStageKind::ValidateBody,
        ) && self.validate_pending.exactly_matches(effect, pending)
    }
    /// Revalidate the canonical family against its retained durable frame.
    pub(super) fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        exact_family_coordinates(&self.family)
            .and_then(|coordinates| certified_body_pipeline_family(&coordinates, receipt))
            .is_some_and(|expected| {
                expected == self.family
                    && self
                        .family
                        .is_exact_for_stage(LifecycleStageKind::ValidateBody)
            })
    }
}
impl RecoveredStandaloneValidateReplayEvidenceV1 {
    fn from_authenticated_source(
        recovered: RecoveredStandaloneValidateSourceV1,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let validate_pending = DirectSignedPendingBindingV1::from_exact_effect(effect, pending)?;
        let evidence = Self {
            source: recovered.source,
            body_frame: recovered.body_frame,
            validate_pending,
        };
        evidence
            .exactly_matches_validate_pending(effect, receipt, pending)
            .then_some(evidence)
    }
    fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.validate_pending.exactly_matches(effect, pending)
            && standalone_validate_stage_matches(&self.source, self.body_frame, effect, receipt)
    }
    fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        let effect = standalone_validate_effect(&self.source);
        effect.is_some_and(|effect| {
            standalone_validate_stage_matches(&self.source, self.body_frame, &effect, receipt)
        })
    }
}
impl RecoveredDecisionValidateReplayEvidenceV1 {
    fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.validate_pending.exactly_matches(effect, pending)
            && self
                .lineage
                .is_stage_closed(replay_context(receipt.round()))
            && recovered_decision_validate_stage_matches(
                &self.lineage.body.source,
                self.lineage.body.body_frame,
                effect,
                receipt,
            )
    }

    fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        recovered_decision_validate_effect(&self.lineage.body.source).is_some_and(|effect| {
            self.lineage
                .is_stage_closed(replay_context(receipt.round()))
                && recovered_decision_validate_stage_matches(
                    &self.lineage.body.source,
                    self.lineage.body.body_frame,
                    &effect,
                    receipt,
                )
        })
    }
}
impl RecoveredStandaloneValidateSourceV1 {
    /// Return whether this recovered source requires the private genesis body-store policy.
    pub(super) fn requires_genesis_authority_body_store(&self) -> bool {
        match &self.source.origin {
            BodyPipelineOriginV1::Certified {
                fetch_manifest_present,
                certified_sources,
                ..
            } => !*fetch_manifest_present || !certified_sources.is_empty(),
            BodyPipelineOriginV1::Proposal(_)
            | BodyPipelineOriginV1::LocalBody(_)
            | BodyPipelineOriginV1::RecoveredDecision { .. } => false,
        }
    }

    pub(super) fn exactly_matches_recovered_body_frame(
        &self,
        reference: &DurableBodyFrameReference,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        standalone_origin_manifest(&self.source).is_some_and(|retained| retained == manifest)
            && self.body_frame.durable_reference() == *reference
            && durable_body_frame_reference(replay_context(receipt.round()), receipt)
                == Some(*reference)
            && standalone_validate_effect(&self.source).is_some_and(|effect| {
                standalone_validate_stage_matches(&self.source, self.body_frame, &effect, receipt)
            })
    }
}
fn standalone_origin_manifest(
    source: &BodyPipelineReplaySourceV1,
) -> Option<&wire::PayloadManifest> {
    match &source.origin {
        BodyPipelineOriginV1::Proposal(proposal) => Some(&proposal.manifest),
        BodyPipelineOriginV1::LocalBody(manifest) => Some(manifest),
        BodyPipelineOriginV1::Certified { manifest, .. } => Some(manifest),
        BodyPipelineOriginV1::RecoveredDecision { .. } => None,
    }
}

fn recovered_decision_origin_manifest(
    source: &BodyPipelineReplaySourceV1,
) -> Option<&wire::PayloadManifest> {
    let BodyPipelineOriginV1::RecoveredDecision { manifest, .. } = &source.origin else {
        return None;
    };
    Some(manifest)
}

fn authenticated_genesis_standalone_source(
    verified: &VerifiedHeightContext,
    source: &BodyPipelineReplaySourceV1,
) -> bool {
    let BodyPipelineOriginV1::Certified {
        certificate,
        manifest,
        certified_sources,
        ..
    } = &source.origin
    else {
        return false;
    };
    let context = verified.context();
    let expected_sources = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    context.height == 1
        && context.parent_commit_qc.is_none()
        && context.snapshot_bootstrap.is_none()
        && certificate.round.context_id == context.id()
        && certificate.round.height == 1
        && certificate.proposal_round == manifest.round
        && certificate.subject == manifest.subject
        && manifest.subject.parent_block_hash.is_none()
        && certificate.validate(context).is_ok()
        && manifest.validate(context).is_ok()
        && certified_sources == &expected_sources
        && verified.verify_quorum_certificate(certificate).is_ok()
}

/// Authenticate the durable marker used when a signed-Proposal Validate
/// acquires Prepare/Commit authority after its physical body pipeline began.
///
/// An empty source list is not a valid certified Fetch (which must name the
/// complete roster); together with an already-present Proposal manifest it is
/// the closed persisted discriminator minted by
/// `exact_remote_proposal_validate_source`. The complete QC is sufficient
/// restart authority, but must be verified again against the frozen context.
fn authenticated_refined_proposal_standalone_source(
    verified: &VerifiedHeightContext,
    source: &BodyPipelineReplaySourceV1,
) -> bool {
    let BodyPipelineOriginV1::Certified {
        certificate,
        manifest,
        fetch_manifest_present,
        certified_sources,
    } = &source.origin
    else {
        return false;
    };
    let context = verified.context();
    *fetch_manifest_present
        && certified_sources.is_empty()
        && certificate.round.context_id == context.id()
        && certificate.round.height == context.height
        && certificate.proposal_round == manifest.round
        && certificate.subject == manifest.subject
        && matches!(
            certificate.phase,
            wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit
        )
        && certificate.validate(context).is_ok()
        && manifest.validate(context).is_ok()
        && verified.verify_quorum_certificate(certificate).is_ok()
}
fn standalone_validate_effect(source: &BodyPipelineReplaySourceV1) -> Option<AdapterEffect> {
    let manifest = standalone_origin_manifest(source)?;
    Some(AdapterEffect::ValidateBody {
        tag: EventTag::new(
            source.tag.height,
            source.tag.view,
            crate::sumeragi::v2_core::Generation::new(source.tag.generation),
        ),
        round: manifest.round,
        subject: manifest.subject,
    })
}
fn recovered_decision_validate_effect(
    source: &BodyPipelineReplaySourceV1,
) -> Option<AdapterEffect> {
    let manifest = recovered_decision_origin_manifest(source)?;
    Some(AdapterEffect::ValidateBody {
        tag: EventTag::new(
            source.tag.height,
            source.tag.view,
            crate::sumeragi::v2_core::Generation::new(source.tag.generation),
        ),
        round: manifest.round,
        subject: manifest.subject,
    })
}
fn standalone_validate_stage_matches(
    source: &BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
) -> bool {
    let Some(manifest) = standalone_origin_manifest(source) else {
        return false;
    };
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = effect
    else {
        return false;
    };
    let context = replay_context(*round);
    source.tag == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
        && *round == manifest.round
        && *subject == manifest.subject
        && receipt.context_id() == round.context_id
        && receipt.round() == *round
        && receipt.subject() == *subject
        && receipt.manifest_hash() == HashOf::new(manifest)
        && durable_body_frame_reference(context, receipt) == Some(body_frame.durable_reference())
        && canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::BodyPipeline(source.clone()),
            LifecycleStageKind::ValidateBody,
            ReplayPayloadBindingV1::BodyFrame(body_frame),
        )
        .is_some()
}
fn recovered_decision_validate_stage_matches(
    source: &BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
) -> bool {
    let Some(manifest) = recovered_decision_origin_manifest(source) else {
        return false;
    };
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = effect
    else {
        return false;
    };
    let context = replay_context(*round);
    source.tag == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
        && *round == manifest.round
        && *subject == manifest.subject
        && receipt.context_id() == round.context_id
        && receipt.round() == *round
        && receipt.subject() == *subject
        && receipt.manifest_hash() == HashOf::new(manifest)
        && durable_body_frame_reference(context, receipt) == Some(body_frame.durable_reference())
        && canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::BodyPipeline(source.clone()),
            LifecycleStageKind::ValidateBody,
            ReplayPayloadBindingV1::BodyFrame(body_frame),
        )
        .is_some()
}
impl DurableValidateReplayEvidenceV1 {
    /// Compare one installed Validate carrier with its complete reconstructed row.
    pub(super) fn exactly_matches_recovered_record(
        &self,
        active_context: LifecycleContext,
        record: &LifecycleRecord,
        metadata: &DurableRecordMetadata,
        installed_digest: LifecycleDigest,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        let (source, body_frame) = match self {
            Self::Certified(evidence) => (&evidence.family.source, evidence.family.body_frame),
            Self::RemoteProposal(evidence) => (&evidence.family.source, evidence.family.body_frame),
            Self::LocalBody(evidence) => evidence.family.source_and_frame(),
            Self::RecoveredStandalone(evidence) => (&evidence.source, evidence.body_frame),
            Self::RecoveredDecision(evidence) => (
                &evidence.lineage.body.source,
                evidence.lineage.body.body_frame,
            ),
        };
        body_stage_matches_recovered_record(
            source,
            body_frame,
            active_context,
            record,
            metadata,
            installed_digest,
            effect,
            receipt,
            pending,
            LifecycleWorkClass::Validate,
            LifecycleStageKind::ValidateBody,
        )
    }
    /// Wrap one exact certified Validate family without exposing its source.
    pub(super) const fn certified(evidence: CertifiedValidateReplayEvidenceV1) -> Self {
        Self::Certified(evidence)
    }
    /// Wrap one exact ordinary remote-Proposal Validate family.
    pub(super) const fn remote_proposal(evidence: RemoteProposalValidateReplayEvidenceV1) -> Self {
        Self::RemoteProposal(evidence)
    }
    /// Wrap one exact locally assembled Validate family.
    pub(in crate::sumeragi) const fn local_body(evidence: LocalValidateReplayEvidenceV1) -> Self {
        Self::LocalBody(evidence)
    }
    fn recovered_standalone(evidence: RecoveredStandaloneValidateReplayEvidenceV1) -> Self {
        Self::RecoveredStandalone(evidence)
    }
    /// Retain only the inert recovered-Decision replay identity for a registry seal.
    pub(super) fn seal_recovered_decision_registry_evidence(
        &self,
    ) -> Option<RecoveredDecisionValidateReplayEvidenceV1> {
        match self {
            Self::RecoveredDecision(evidence) => Some(evidence.clone()),
            Self::Certified(_)
            | Self::RemoteProposal(_)
            | Self::LocalBody(_)
            | Self::RecoveredStandalone(_) => None,
        }
    }
    /// Compare two Validate replay carriers only within the recovered-Decision family.
    pub(super) fn same_recovered_decision_registry_evidence(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::RecoveredDecision(left), Self::RecoveredDecision(right)) if left == right
        )
    }
    /// Compare an installed Validate carrier with one inert recovered-Decision seal.
    pub(super) fn matches_recovered_decision_registry_evidence(
        &self,
        expected: &RecoveredDecisionValidateReplayEvidenceV1,
    ) -> bool {
        matches!(self, Self::RecoveredDecision(evidence) if evidence == expected)
    }
    /// Reproject and compare the exact recovered-Decision Validate candidate.
    ///
    /// This comparison-only oracle releases no candidate or runtime owner and
    /// is used before the linear startup projection is consumed.
    pub(super) fn exactly_projects_recovered_decision_validate_candidate(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
        expected: &CandidateAdmission,
    ) -> bool {
        matches!(self, Self::RecoveredDecision(_))
            && self
                .project_exact_validate_candidate(verified, effect, receipt, pending)
                .is_ok_and(|candidate| candidate == *expected)
    }
    /// Project local-proposal completion authority without exposing a generic
    /// replay-source or pending-binding constructor.
    pub(in crate::sumeragi) fn project_local_completion_evidence(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<(LocalValidateReplayEvidenceV1, wire::PayloadManifest)> {
        match self {
            Self::LocalBody(evidence) => {
                if !evidence.exactly_matches_validate_pending(effect, receipt, pending) {
                    return None;
                }
                let manifest = evidence.family.assembled_manifest()?;
                Some((evidence.clone(), manifest.clone()))
            }
            Self::RecoveredStandalone(evidence) => {
                let BodyPipelineOriginV1::LocalBody(manifest) = &evidence.source.origin else {
                    return None;
                };
                if !evidence.exactly_matches_validate_pending(effect, receipt, pending) {
                    return None;
                }
                let reconstructed = PendingRuntimeEffectBinding::from_durable_standalone_validate(
                    DurableStandaloneValidatePendingMintPermit::new(),
                    *pending.causal_lifecycle_key(),
                    effect,
                    None,
                )?;
                if &reconstructed != pending {
                    return None;
                }
                Some((
                    LocalValidateReplayEvidenceV1 {
                        family: LocalValidateReplayFamilyV1::Assembled(
                            LocalBodyPipelineReplayFamilyV1 {
                                source: evidence.source.clone(),
                                body_frame: evidence.body_frame,
                            },
                        ),
                        validate_pending: Arc::new(reconstructed),
                    },
                    manifest.clone(),
                ))
            }
            Self::Certified(_) | Self::RemoteProposal(_) | Self::RecoveredDecision(_) => None,
        }
    }
    /// Compare the closed family with one exact Validate effect, body frame,
    /// and causal pending binding.
    pub(in crate::sumeragi) fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        match self {
            Self::Certified(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
            Self::RemoteProposal(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
            Self::LocalBody(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
            Self::RecoveredStandalone(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
            Self::RecoveredDecision(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
        }
    }
    /// Revalidate the closed family against its retained durable body frame.
    pub(super) fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        match self {
            Self::Certified(evidence) => evidence.exactly_matches_durable_body(receipt),
            Self::RemoteProposal(evidence) => {
                remote_proposal_validate_matches_durable_body(evidence, receipt)
            }
            Self::LocalBody(evidence) => evidence.exactly_matches_durable_body(receipt),
            Self::RecoveredStandalone(evidence) => evidence.exactly_matches_durable_body(receipt),
            Self::RecoveredDecision(evidence) => evidence.exactly_matches_durable_body(receipt),
        }
    }
    /// Project one installed Validate carrier without exposing its replay family.
    ///
    /// The registry-only one-shot permit proves the evidence, durable frame,
    /// concrete effect, and pending binding still reside in one closed carrier.
    pub(in crate::sumeragi) fn project_installed_validate_candidate(
        &self,
        _permit: InstalledBodyCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
    }
    /// Project one Validate successor still sealed under its exact Store parent.
    pub(in crate::sumeragi) fn project_sealed_validate_successor_candidate(
        &self,
        _permit: SealedBodySuccessorProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
    }
    /// Replace only the retained event origin in a negative test fixture.
    #[cfg(test)]
    pub(super) fn replace_with_foreign_origin_for_test(&mut self) -> bool {
        let source = match self {
            Self::Certified(evidence) => &mut evidence.family.source,
            Self::RemoteProposal(evidence) => &mut evidence.family.source,
            Self::LocalBody(evidence) => evidence.family.source_mut(),
            Self::RecoveredStandalone(evidence) => &mut evidence.source,
            Self::RecoveredDecision(evidence) => &mut evidence.lineage.body.source,
        };
        let previous = source.tag;
        source.tag.generation = previous.generation.wrapping_add(1);
        source.tag != previous
    }
    /// Join this retained Validate origin to the exact body frame and runtime owner.
    ///
    /// The canonical body-pipeline authority remains private and is attached
    /// only after the runtime projection, durable receipt, and retained
    /// pending fingerprint all agree exactly.
    pub(in crate::sumeragi) fn project_recovered_validate_candidate(
        &self,
        _permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
            .ok()
    }
    fn project_exact_validate_candidate(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches_validate_pending(effect, receipt, pending) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = replay_context(receipt.round());
        let payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(active_context, receipt)
                .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?,
        );
        let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
        let source = match self {
            Self::Certified(evidence) => {
                if payload_binding != ReplayPayloadBindingV1::BodyFrame(evidence.family.body_frame)
                {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                evidence.family.source.clone()
            }
            Self::RemoteProposal(evidence) => {
                if payload_binding != ReplayPayloadBindingV1::BodyFrame(evidence.family.body_frame)
                {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                evidence.validate_source.clone()
            }
            Self::LocalBody(evidence) => {
                if !evidence.family.authenticated_by_verified_height(verified) {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                let (source, body_frame) = evidence.family.source_and_frame();
                if payload_binding != ReplayPayloadBindingV1::BodyFrame(body_frame) {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                source.clone()
            }
            Self::RecoveredStandalone(evidence) => {
                if payload_binding != ReplayPayloadBindingV1::BodyFrame(evidence.body_frame) {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                evidence.source.clone()
            }
            Self::RecoveredDecision(evidence) => {
                if payload_binding
                    != ReplayPayloadBindingV1::BodyFrame(evidence.lineage.body.body_frame)
                {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                evidence.lineage.body.source.clone()
            }
        };
        let authority = canonical_replay_authority(
            active_context,
            LifecycleReplaySourceV1::BodyPipeline(source),
            LifecycleStageKind::ValidateBody,
            payload_binding,
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?;
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )?;
        candidate_from_authorized_projection(active_context, projected, payload, authority)
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }
    /// Project the canonical Validate candidate for focused transition tests.
    #[cfg(test)]
    pub(super) fn project_candidate_for_test(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
    }
    /// Consume the adapter's move-only registered-Prepare proof into the exact
    /// canonical invalid-body report evidence.
    ///
    /// The capability is minted only by the fixed Ready/rejected adapter
    /// preview. Callers cannot substitute the report certificate or child
    /// pending binding, and decoded V1 data never reaches this constructor.
    pub(in crate::sumeragi) fn seal_invalid_body_report(
        capability: RegisteredPrepareInvalidBodyReportCapability,
        validate_origin: Self,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        report_effect: &AdapterEffect,
        report_pending: &PendingRuntimeEffectBinding,
    ) -> Option<InvalidBodyReportReplayEvidenceV1> {
        if !capability.exactly_matches_report(report_effect)
            || !validate_origin.exactly_matches_validate_pending(
                validate_effect,
                receipt,
                validate_pending,
            )
        {
            return None;
        }
        let projected_report_pending = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                validate_effect,
                report_effect,
            )
            .or_else(|| {
                validate_pending
                    .project_validate_report_invalid_certified_body_with_registered_prepare(
                        validate_effect,
                        report_effect,
                        &capability,
                    )
            })?;
        if &projected_report_pending != report_pending {
            return None;
        }
        let authority = exact_invalid_body_report_authority(
            &validate_origin,
            validate_effect,
            receipt,
            report_effect,
        )?;
        let pending_fingerprint =
            DirectSignedPendingBindingV1::from_exact_effect(report_effect, report_pending)?;
        let evidence = InvalidBodyReportReplayEvidenceV1 {
            authority,
            validate_origin,
            report_pending: pending_fingerprint,
        };
        evidence
            .exactly_matches(
                validate_effect,
                validate_pending,
                receipt,
                report_effect,
                report_pending,
            )
            .then_some(evidence)
    }
}
#[allow(clippy::too_many_arguments)]
fn body_stage_matches_recovered_record(
    source: &BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
    active_context: LifecycleContext,
    record: &LifecycleRecord,
    metadata: &DurableRecordMetadata,
    installed_digest: LifecycleDigest,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    pending: &PendingRuntimeEffectBinding,
    work_class: LifecycleWorkClass,
    stage_kind: LifecycleStageKind,
) -> bool {
    let (tag, round, subject) = match effect {
        AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } if stage_kind == LifecycleStageKind::StoreBody => (*tag, *round, *subject),
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } if stage_kind == LifecycleStageKind::ValidateBody => (*tag, *round, *subject),
        _ => return false,
    };
    let Some(statement) = pending.candidate_statement() else {
        return false;
    };
    let expected_payload = DurablePayloadReference::BodyFrame(body_frame.durable_reference());
    let replay_source = LifecycleReplaySourceV1::BodyPipeline(source.clone());
    let Ok(shape) = replay_source.project(
        active_context,
        stage_kind,
        &ReplayPayloadBindingV1::BodyFrame(body_frame),
    ) else {
        return false;
    };
    let expected_authority = canonical_replay_authority(
        active_context,
        replay_source,
        stage_kind,
        ReplayPayloadBindingV1::BodyFrame(body_frame),
    );
    let slot = PhysicalSlotId::for_capacity(work_class.capacity_class(), 0);
    pending.exactly_binds_adapter_effect(effect)
        && statement.context_id() == round.context_id
        && statement.round().context_id == round.context_id
        && statement.proposal_round() == round
        && statement.subject() == Some(subject)
        && shape.key.round()
            == LifecycleRound::new(statement.round().height, statement.round().view)
        && shape.key.proposal_round()
            == Some(LifecycleRound::new(
                statement.proposal_round().height,
                statement.proposal_round().view,
            ))
        && shape.key.subject() == Some(block_subject(subject))
        && shape.key.execution_commitment()
            == statement.execution_commitment().map(execution_commitment)
        && tag.height() == active_context.height()
        && tag.view() >= statement.round().view
        && digest_from_hash(pending.causal_lifecycle_key()) == record.owner.causal_root().digest()
        && record.key == shape.key
        && record.work_class == work_class
        && record.stage == LifecycleStage::new(stage_kind, PredecessorScope::Independent)
        && record.state == super::LifecycleState::Ready
        && record.physical_slots == std::collections::BTreeMap::from([(slot, installed_digest)])
        && record.episode.slot_universe == std::collections::BTreeSet::from([slot])
        && record.episode.consumed_slots == record.episode.slot_universe
        && metadata.reconstruction_source == record.owner.causal_root().digest()
        && metadata.payload == expected_payload
        && expected_authority.as_ref() == Some(&metadata.replay_authority)
        && durable_body_frame_reference(active_context, receipt)
            == Some(body_frame.durable_reference())
        && installed_digest == digest_from_hash(pending.exact_effect_identity())
}
impl InvalidBodyReportReplayEvidenceV1 {
    /// Compare the complete body origin, rejection envelope, report effect,
    /// and causal binding without exposing any retained part.
    pub(in crate::sumeragi) fn exactly_matches(
        &self,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        report_effect: &AdapterEffect,
        report_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.validate_origin.exactly_matches_validate_pending(
            validate_effect,
            receipt,
            validate_pending,
        ) && self
            .report_pending
            .exactly_matches(report_effect, report_pending)
            && exact_invalid_body_report_authority(
                &self.validate_origin,
                validate_effect,
                receipt,
                report_effect,
            )
            .is_some_and(|expected| expected == self.authority)
    }
    /// Attach the retained invalid-body authority to its exact report shape.
    ///
    /// The private transition permit is borrowed across projection and remains
    /// owned by the registry join. No decoded or caller-supplied report can
    /// invoke this path.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn project_sealed_invalid_body_report_candidate(
        &self,
        _permit: &SealedInvalidBodyReportProjectionPermit,
        verified: &VerifiedHeightContext,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        report_effect: &AdapterEffect,
        report_pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches(
            validate_effect,
            validate_pending,
            receipt,
            report_effect,
            report_pending,
        ) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = replay_context(receipt.round());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            report_effect,
            report_pending,
        )?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            self.authority.clone(),
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }

    /// Consume the canonical rejection envelope into one mandatory-bound,
    /// closed report carrier for the fixed Validate publication transaction.
    ///
    /// The replay authority never escapes as a caller-supplied part. Failure
    /// returns the complete evidence, effect, and pending owner unchanged.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_live_validate_report_work(
        self,
        permit: LiveValidateReportWorkProjectionPermit,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<
        PreparedLiveValidateReportRegistryWork,
        (Self, AdapterEffect, PendingRuntimeEffectBinding),
    > {
        if !self.report_pending.exactly_matches(&effect, &pending) {
            return Err((self, effect, pending));
        }
        let Self {
            authority,
            validate_origin,
            report_pending,
        } = self;
        let bound = match BoundAdapterEffectV1::bind_invalid_body_report(
            InvalidBodyReportBoundEffectPermit::new(),
            effect,
            pending,
            authority,
        ) {
            Ok(bound) => bound,
            Err((effect, pending, authority)) => {
                return Err((
                    Self {
                        authority,
                        validate_origin,
                        report_pending,
                    },
                    effect,
                    pending,
                ));
            }
        };
        drop(validate_origin);
        drop(report_pending);
        Ok(PreparedLiveValidateReportRegistryWork::from_bound(
            permit, bound,
        ))
    }
}
fn remote_proposal_validate_matches_durable_body(
    evidence: &RemoteProposalValidateReplayEvidenceV1,
    receipt: &DurableBodyReceipt,
) -> bool {
    let BodyPipelineOriginV1::Proposal(proposal) = &evidence.family.source.origin else {
        return false;
    };
    let effect = AdapterEffect::ValidateBody {
        tag: EventTag::new(
            evidence.family.source.tag.height,
            evidence.family.source.tag.view,
            crate::sumeragi::v2_core::Generation::new(evidence.family.source.tag.generation),
        ),
        round: proposal.round,
        subject: proposal.subject,
    };
    evidence.exactly_matches_validate(&effect, receipt)
}
fn exact_invalid_body_report_authority(
    validate_origin: &DurableValidateReplayEvidenceV1,
    validate_effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    report_effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    const CANONICAL_REJECTION_CODE: u8 = 0;
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = validate_effect
    else {
        return None;
    };
    let AdapterEffect::ReportInvalidCertifiedBody {
        subject: report_subject,
        certificate,
    } = report_effect
    else {
        return None;
    };
    if certificate.phase != wire::GlobalPhase::Prepare
        || certificate.round != *round
        || certificate.proposal_round != *round
        || certificate.subject != *subject
        || *report_subject != *subject
        || tag.height() != certificate.round.height
        || tag.view() != certificate.round.view
        || receipt.context_id() != round.context_id
        || receipt.round() != *round
        || receipt.subject() != *subject
    {
        return None;
    }
    let (validation_origin, manifest) = match validate_origin {
        DurableValidateReplayEvidenceV1::Certified(evidence) => {
            let coordinates = exact_family_coordinates(&evidence.family)?;
            if coordinates.certificate != *certificate {
                return None;
            }
            (evidence.family.source.clone(), coordinates.manifest)
        }
        DurableValidateReplayEvidenceV1::RemoteProposal(evidence) => {
            let BodyPipelineOriginV1::Proposal(proposal) = &evidence.family.source.origin else {
                return None;
            };
            if proposal.round != *round || proposal.subject != *subject {
                return None;
            }
            (evidence.family.source.clone(), proposal.manifest.clone())
        }
        DurableValidateReplayEvidenceV1::LocalBody(evidence) => {
            let family = evidence.family.authenticated_certified_family()?;
            let coordinates = exact_family_coordinates(family)?;
            if coordinates.certificate != *certificate {
                return None;
            }
            (family.source.clone(), coordinates.manifest)
        }
        DurableValidateReplayEvidenceV1::RecoveredStandalone(evidence) => {
            match &evidence.source.origin {
                BodyPipelineOriginV1::Proposal(proposal) => {
                    if proposal.round != *round || proposal.subject != *subject {
                        return None;
                    }
                    (evidence.source.clone(), proposal.manifest.clone())
                }
                BodyPipelineOriginV1::Certified {
                    certificate: origin_certificate,
                    manifest,
                    ..
                } => {
                    if origin_certificate != certificate {
                        return None;
                    }
                    (evidence.source.clone(), manifest.clone())
                }
                BodyPipelineOriginV1::LocalBody(_)
                | BodyPipelineOriginV1::RecoveredDecision { .. } => return None,
            }
        }
        DurableValidateReplayEvidenceV1::RecoveredDecision(_) => return None,
    };
    if receipt.manifest_hash() != HashOf::new(&manifest) {
        return None;
    }
    let context = replay_context(certificate.round);
    canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::InvalidCertifiedBody(InvalidBodyReplaySourceV1 {
            validation_origin,
            certificate: certificate.clone(),
            outcome: RejectedBodyOutcomeBindingV1 {
                manifest,
                body_frame_hash: *receipt.frame_hash().as_ref(),
                rejection_code: CANONICAL_REJECTION_CODE,
            },
        }),
        LifecycleStageKind::ReportInvalidBody,
        ReplayPayloadBindingV1::None,
    )
}
fn exact_certified_fetch_coordinates(
    effect: &AdapterEffect,
    response: &wire::CertifiedBodyResponse,
) -> Option<CertifiedBodyPipelineCoordinatesV1> {
    exact_certified_fetch_coordinates_from_manifest(effect, &response.manifest)
}
fn exact_certified_fetch_coordinates_from_manifest(
    effect: &AdapterEffect,
    response_manifest: &wire::PayloadManifest,
) -> Option<CertifiedBodyPipelineCoordinatesV1> {
    let AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest,
        certificate,
        ..
    } = effect
    else {
        return None;
    };
    let certificate = certificate.as_ref()?;
    let context = replay_context(*round);
    let replay_tag = ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get());
    let tag_matches_origin = if certificate.phase == wire::GlobalPhase::Commit {
        replay_tag.matches_decision_round(context, *round)
    } else {
        replay_tag.matches_round(context, *round)
    };
    if response_manifest.round != *round
        || response_manifest.subject != *subject
        || manifest
            .as_ref()
            .is_some_and(|expected| expected != response_manifest)
        || certificate.proposal_round != *round
        || certificate.subject != *subject
        || !tag_matches_origin
    {
        return None;
    }
    Some(CertifiedBodyPipelineCoordinatesV1 {
        tag: replay_tag,
        certificate: certificate.clone(),
        manifest: response_manifest.clone(),
        fetch_manifest_present: manifest.is_some(),
        certified_sources: match effect {
            AdapterEffect::FetchBody {
                certified_sources, ..
            } => certified_sources.clone(),
            _ => unreachable!("Fetch shape was checked above"),
        },
    })
}
/// Derive the canonical payload-free replay authority for one live certified Fetch.
///
/// The response manifest supplies the immutable body coordinates when the
/// certificate-backed `FetchBody` was admitted without a proposal manifest.
/// Consensus authentication remains the responsibility of the adjacent
/// verified-context projection; this helper only seals the exact effect,
/// pending runtime binding, and canonical replay envelope together.
pub(super) fn exact_pending_certified_fetch_admission_authority(
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
    response_manifest: &wire::PayloadManifest,
) -> Option<LifecycleReplayAuthorityV1> {
    if !pending.exactly_binds_adapter_effect(effect) {
        return None;
    }
    let coordinates = exact_certified_fetch_coordinates_from_manifest(effect, response_manifest)?;
    let context = replay_context(coordinates.certificate.round);
    let source = BodyPipelineReplaySourceV1 {
        tag: coordinates.tag,
        origin: BodyPipelineOriginV1::Certified {
            certificate: coordinates.certificate,
            manifest: coordinates.manifest,
            fetch_manifest_present: coordinates.fetch_manifest_present,
            certified_sources: coordinates.certified_sources,
        },
    };
    canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::BodyPipeline(source),
        LifecycleStageKind::FetchBody,
        ReplayPayloadBindingV1::None,
    )
}
fn exact_certified_body_pipeline_family(
    coordinates: &CertifiedBodyPipelineCoordinatesV1,
    receipt: &DurableBodyReceipt,
) -> Option<CertifiedBodyPipelineReplayFamilyV1> {
    let family = certified_body_pipeline_family(coordinates, receipt)?;
    family.is_exact_all_stages().then_some(family)
}
fn certified_body_pipeline_family(
    coordinates: &CertifiedBodyPipelineCoordinatesV1,
    receipt: &DurableBodyReceipt,
) -> Option<CertifiedBodyPipelineReplayFamilyV1> {
    let certificate = &coordinates.certificate;
    let manifest = &coordinates.manifest;
    if receipt.context_id() != certificate.round.context_id
        || receipt.round() != manifest.round
        || receipt.subject() != manifest.subject
        || receipt.manifest_hash() != HashOf::new(manifest)
    {
        return None;
    }
    let context = LifecycleContext::new(
        digest_from_bytes(certificate.round.context_id.0.as_ref()),
        certificate.round.height,
    );
    let frame = durable_body_frame_reference(context, receipt)?;
    let source = BodyPipelineReplaySourceV1 {
        tag: coordinates.tag,
        origin: BodyPipelineOriginV1::Certified {
            certificate: certificate.clone(),
            manifest: manifest.clone(),
            fetch_manifest_present: coordinates.fetch_manifest_present,
            certified_sources: coordinates.certified_sources.clone(),
        },
    };
    let ReplayPayloadBindingV1::BodyFrame(body_frame) =
        ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame))
    else {
        unreachable!("a durable body frame projects one body-frame binding")
    };
    Some(CertifiedBodyPipelineReplayFamilyV1 { source, body_frame })
}
fn canonical_replay_authority(
    context: LifecycleContext,
    source: LifecycleReplaySourceV1,
    stage_kind: LifecycleStageKind,
    payload: ReplayPayloadBindingV1,
) -> Option<LifecycleReplayAuthorityV1> {
    let shape = source.project(context, stage_kind, &payload).ok()?;
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            shape.work_class,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            match &authority.payload {
                ReplayPayloadBindingV1::None => DurablePayloadReference::None,
                ReplayPayloadBindingV1::BodyFrame(frame) => {
                    DurablePayloadReference::BodyFrame(frame.durable_reference())
                }
                ReplayPayloadBindingV1::CertifiedServePending { .. }
                | ReplayPayloadBindingV1::CertifiedServeCompleted { .. }
                | ReplayPayloadBindingV1::CertifiedServeNegative { .. } => return None,
            },
        )
        .ok()?;
    let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()).ok()?;
    (canonical == authority).then_some(canonical)
}
/// Classify the private recovered-Decision body continuation family.
///
/// `None` means neither side belongs to this family and the ordinary body-edge
/// rules apply. `Some(false)` is a hard mismatch: once the payload-free
/// `FetchDecision` or a recovered-Decision body source appears, it cannot be
/// spliced to a generic body family or skip an intermediate stage.
pub(super) fn recovered_decision_body_continuation_is_exact(
    edge: super::schema::DurableContinuationEdge,
    parent: &LifecycleReplayAuthorityV1,
    parent_payload: DurablePayloadReference,
    child: &LifecycleReplayAuthorityV1,
    child_payload: DurablePayloadReference,
) -> Option<bool> {
    let fetch = recovered_decision_fetch_parts(parent);
    let parent_body = recovered_decision_body_parts(parent);
    let child_body = recovered_decision_body_parts(child);
    let family_present = fetch.is_some() || parent_body.is_some() || child_body.is_some();
    if !family_present {
        return None;
    }
    let canonical = |authority: &LifecycleReplayAuthorityV1, payload: DurablePayloadReference| {
        authority.payload.matches(payload)
            && LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
                .is_ok_and(|decoded| decoded == *authority)
    };
    if !canonical(parent, parent_payload) || !canonical(child, child_payload) {
        return Some(false);
    }
    Some(match edge {
        super::schema::DurableContinuationEdge::FetchToStore => {
            let (fetch_locator, fetch_tag, fetch_certificate) = match fetch {
                Some(parts) => parts,
                None => return Some(false),
            };
            let (body_source, body_frame) = match child_body {
                Some(parts) => parts,
                None => return Some(false),
            };
            parent_payload == DurablePayloadReference::None
                && child_payload
                    == DurablePayloadReference::BodyFrame(body_frame.durable_reference())
                && body_source.locator == fetch_locator
                && body_source.tag == fetch_tag
                && body_source.certificate == &fetch_certificate
        }
        super::schema::DurableContinuationEdge::StoreToValidate => {
            parent_body.is_some()
                && child_body.is_some()
                && parent == child
                && parent_payload == child_payload
                && matches!(parent_payload, DurablePayloadReference::BodyFrame(_))
        }
        super::schema::DurableContinuationEdge::ValidateToApply => {
            let (body_source, body_frame) = match parent_body {
                Some(parts) => parts,
                None => return Some(false),
            };
            let (apply_locator, apply_tag, apply_certificate, apply_frame) =
                match recovered_decision_apply_parts(child) {
                    Some(parts) => parts,
                    None => return Some(false),
                };
            parent_payload == DurablePayloadReference::BodyFrame(body_frame.durable_reference())
                && child_payload
                    == DurablePayloadReference::BodyFrame(apply_frame.durable_reference())
                && body_frame == apply_frame
                && body_source.locator == apply_locator
                && body_source.tag == apply_tag
                && body_source.certificate == apply_certificate
        }
        super::schema::DurableContinuationEdge::ValidateToInvalidBodyReport
        | super::schema::DurableContinuationEdge::ValidateToSignPrepare
        | super::schema::DurableContinuationEdge::ValidateToSignCommit
        | super::schema::DurableContinuationEdge::SignProposalToBroadcast
        | super::schema::DurableContinuationEdge::SignPrepareToBroadcast
        | super::schema::DurableContinuationEdge::SignCommitToBroadcast
        | super::schema::DurableContinuationEdge::SignTimeoutToBroadcast => false,
    })
}
fn recovered_decision_fetch_parts(
    authority: &LifecycleReplayAuthorityV1,
) -> Option<(
    PersistedWalFrameLocatorV1,
    ReplayEventTagV1,
    wire::QuorumCertificate,
)> {
    let (
        ReplayPayloadBindingV1::None,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role,
            tag,
            action:
                WalReplayActionV1::FetchDecision {
                    certificate,
                    certified_sources,
                },
        }),
    ) = (&authority.payload, &authority.source)
    else {
        return None;
    };
    (authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
        && locator.is_exact()
        && role.matches(ReplayWalRoleV1::DECISION)
        && certificate.phase == wire::GlobalPhase::Commit
        && certified_sources_are_bounded_unique(certified_sources)
        && !certified_sources.is_empty())
    .then_some((*locator, *tag, certificate.clone()))
}
struct RecoveredDecisionBodyReplayParts<'authority> {
    locator: PersistedWalFrameLocatorV1,
    tag: ReplayEventTagV1,
    certificate: &'authority wire::QuorumCertificate,
}
fn recovered_decision_body_parts(
    authority: &LifecycleReplayAuthorityV1,
) -> Option<(RecoveredDecisionBodyReplayParts<'_>, BodyFrameBindingV1)> {
    let (
        ReplayPayloadBindingV1::BodyFrame(body_frame),
        LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            tag,
            origin:
                BodyPipelineOriginV1::RecoveredDecision {
                    locator,
                    certificate,
                    manifest,
                },
        }),
    ) = (&authority.payload, &authority.source)
    else {
        return None;
    };
    (authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
        && locator.is_exact()
        && certificate.phase == wire::GlobalPhase::Commit
        && body_frame.matches_origin(
            replay_context(certificate.round),
            certificate.proposal_round,
            certificate.subject,
        )
        && body_frame.manifest == *HashOf::new(manifest).as_ref())
    .then_some((
        RecoveredDecisionBodyReplayParts {
            locator: *locator,
            tag: *tag,
            certificate,
        },
        *body_frame,
    ))
}
fn recovered_decision_apply_parts(
    authority: &LifecycleReplayAuthorityV1,
) -> Option<(
    PersistedWalFrameLocatorV1,
    ReplayEventTagV1,
    &wire::QuorumCertificate,
    BodyFrameBindingV1,
)> {
    let (
        ReplayPayloadBindingV1::BodyFrame(body_frame),
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role,
            tag,
            action: WalReplayActionV1::ApplyDecision(certificate),
        }),
    ) = (&authority.payload, &authority.source)
    else {
        return None;
    };
    (authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
        && locator.is_exact()
        && role.matches(ReplayWalRoleV1::DECISION)
        && certificate.phase == wire::GlobalPhase::Commit
        && body_frame.matches_origin(
            replay_context(certificate.round),
            certificate.proposal_round,
            certificate.subject,
        ))
    .then_some((*locator, *tag, certificate, *body_frame))
}
impl CertifiedBodyPipelineReplayFamilyV1 {
    fn is_exact_all_stages(&self) -> bool {
        self.is_exact_for_stage(LifecycleStageKind::FetchBody)
            && self.is_exact_for_stage(LifecycleStageKind::StoreBody)
            && self.is_exact_for_stage(LifecycleStageKind::ValidateBody)
    }
    fn is_exact_for_stage(&self, stage: LifecycleStageKind) -> bool {
        let Some(coordinates) = exact_family_coordinates(self) else {
            return false;
        };
        let context = LifecycleContext::new(
            digest_from_bytes(coordinates.certificate.round.context_id.0.as_ref()),
            coordinates.certificate.round.height,
        );
        let source = LifecycleReplaySourceV1::BodyPipeline(self.source.clone());
        let payload = match stage {
            LifecycleStageKind::FetchBody
            | LifecycleStageKind::StoreBody
            | LifecycleStageKind::ValidateBody => {
                ReplayPayloadBindingV1::BodyFrame(self.body_frame)
            }
            _ => return false,
        };
        canonical_replay_authority(context, source, stage, payload).is_some()
    }
}
fn exact_family_coordinates(
    family: &CertifiedBodyPipelineReplayFamilyV1,
) -> Option<CertifiedBodyPipelineCoordinatesV1> {
    let BodyPipelineOriginV1::Certified {
        certificate,
        manifest,
        fetch_manifest_present,
        certified_sources,
    } = &family.source.origin
    else {
        return None;
    };
    Some(CertifiedBodyPipelineCoordinatesV1 {
        tag: family.source.tag,
        certificate: certificate.clone(),
        manifest: manifest.clone(),
        fetch_manifest_present: *fetch_manifest_present,
        certified_sources: certified_sources.clone(),
    })
}
fn exact_certified_fetch_effect(
    family: &CertifiedBodyPipelineReplayFamilyV1,
) -> Option<AdapterEffect> {
    let coordinates = exact_family_coordinates(family)?;
    Some(certified_fetch_effect_from_coordinates(&coordinates))
}
fn certified_fetch_effect_from_coordinates(
    coordinates: &CertifiedBodyPipelineCoordinatesV1,
) -> AdapterEffect {
    AdapterEffect::FetchBody {
        tag: EventTag::new(
            coordinates.tag.height,
            coordinates.tag.view,
            crate::sumeragi::v2_core::Generation::new(coordinates.tag.generation),
        ),
        round: coordinates.certificate.proposal_round,
        subject: coordinates.certificate.subject,
        manifest: coordinates
            .fetch_manifest_present
            .then_some(coordinates.manifest.clone()),
        certified_sources: coordinates.certified_sources.clone(),
        certificate: Some(coordinates.certificate.clone()),
    }
}
fn certified_store_effect_from_coordinates(
    coordinates: &CertifiedBodyPipelineCoordinatesV1,
) -> AdapterEffect {
    AdapterEffect::StoreBody {
        tag: EventTag::new(
            coordinates.tag.height,
            coordinates.tag.view,
            crate::sumeragi::v2_core::Generation::new(coordinates.tag.generation),
        ),
        round: coordinates.manifest.round,
        subject: coordinates.manifest.subject,
    }
}
fn durable_certified_fetch_projection(
    family: &CertifiedBodyPipelineReplayFamilyV1,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
    receipt: &DurableBodyReceipt,
) -> Option<DurableCertifiedFetchReplayProjectionV1> {
    if exact_certified_fetch_effect(family).as_ref() != Some(effect)
        || !pending.exactly_binds_adapter_effect(effect)
        || certified_body_pipeline_family(&exact_family_coordinates(family)?, receipt).as_ref()
            != Some(family)
    {
        return None;
    }
    let context = replay_context(receipt.round());
    let payload =
        DurablePayloadReference::BodyFrame(durable_body_frame_reference(context, receipt)?);
    let authority = canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::BodyPipeline(family.source.clone()),
        LifecycleStageKind::FetchBody,
        ReplayPayloadBindingV1::from_payload(payload),
    )?;
    let causal_key = *pending.causal_lifecycle_key();
    let effect_identity = *pending.exact_effect_identity();
    let completion_digest = canonical_durable_certified_fetch_completion_digest(
        causal_key,
        effect_identity,
        &authority,
    );
    Some(DurableCertifiedFetchReplayProjectionV1 {
        payload,
        authority,
        causal_key,
        effect_identity,
        completion_digest,
        expected_manifest_hash: receipt.manifest_hash(),
    })
}
fn canonical_durable_certified_fetch_completion_digest(
    causal_key: Hash,
    effect_identity: Hash,
    authority: &LifecycleReplayAuthorityV1,
) -> LifecycleDigest {
    const DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:durable-certified-fetch:v1";
    let encoded_authority = authority.encode();
    let mut preimage =
        Vec::with_capacity(DOMAIN.len() + 1 + Hash::LENGTH * 2 + 8 + encoded_authority.len());
    preimage.extend_from_slice(DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(causal_key.as_ref());
    preimage.extend_from_slice(effect_identity.as_ref());
    preimage.extend_from_slice(
        &u64::try_from(encoded_authority.len())
            .expect("bounded replay authority encoding fits u64")
            .to_le_bytes(),
    );
    preimage.extend_from_slice(&encoded_authority);
    digest_from_hash(&Hash::new(preimage))
}
fn certified_body_stage_matches(
    family: &CertifiedBodyPipelineReplayFamilyV1,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    stage: LifecycleStageKind,
) -> bool {
    let Some(coordinates) = exact_family_coordinates(family) else {
        return false;
    };
    let exact_effect = match (stage, effect) {
        (
            LifecycleStageKind::StoreBody,
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            },
        )
        | (
            LifecycleStageKind::ValidateBody,
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            },
        ) => {
            coordinates.tag
                == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
                && *round == coordinates.certificate.round
                && *subject == coordinates.certificate.subject
        }
        _ => false,
    };
    exact_effect
        && certified_body_pipeline_family(&coordinates, receipt)
            .is_some_and(|expected| expected == *family && family.is_exact_for_stage(stage))
}
fn exact_live_wal_replay_projection(
    wal_identity: &LiveWalFrameIdentity,
    effect: &AdapterEffect,
) -> Option<LiveWalReplayProjectionV1> {
    if !wal_identity.is_exact() {
        return None;
    }
    let (tag, round, role, stage, action) = match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } => (
            *tag,
            proposal.round,
            ReplayWalRoleV1::PROPOSAL_INTENT,
            LifecycleStageKind::SignProposal,
            WalReplayActionV1::SignProposal(proposal.clone()),
        ),
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(vote),
        } => {
            let (role, stage) = match vote.phase {
                wire::GlobalPhase::Prepare => (
                    ReplayWalRoleV1::PREPARE_INTENT,
                    LifecycleStageKind::SignPrepareVote,
                ),
                wire::GlobalPhase::Commit => (
                    ReplayWalRoleV1::LOCK_AND_COMMIT,
                    LifecycleStageKind::SignCommitVote,
                ),
            };
            (
                *tag,
                vote.round,
                role,
                stage,
                WalReplayActionV1::SignVote(vote.clone()),
            )
        }
        AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(vote),
        } => (
            *tag,
            vote.round,
            ReplayWalRoleV1::TIMEOUT_INTENT,
            LifecycleStageKind::SignTimeoutVote,
            WalReplayActionV1::SignTimeoutVote(vote.clone()),
        ),
        AdapterEffect::Apply {
            tag, certificate, ..
        } => (
            *tag,
            certificate.round,
            ReplayWalRoleV1::DECISION,
            LifecycleStageKind::ApplyDecision,
            WalReplayActionV1::ApplyDecision(certificate.clone()),
        ),
        AdapterEffect::EnterView {
            tag,
            certificate,
            protected_lock,
        } => (
            *tag,
            certificate.round,
            ReplayWalRoleV1::INSTALL_TIMEOUT,
            LifecycleStageKind::EnterView,
            WalReplayActionV1::EnterView {
                certificate: certificate.clone(),
                protected_lock: protected_lock.clone(),
            },
        ),
        AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => return None,
    };
    let context = replay_context(round);
    let replay_tag = ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get());
    let source = WalReplaySourceV1 {
        locator: wal_identity.persisted_locator(),
        role,
        tag: replay_tag,
        action,
    };
    if stage == LifecycleStageKind::ApplyDecision {
        let WalReplayActionV1::ApplyDecision(certificate) = &source.action else {
            unreachable!("Apply stage is constructed from one Decision action")
        };
        if !source.locator.is_exact()
            || !source.role.matches(ReplayWalRoleV1::DECISION)
            || !qc_shape(context, certificate)
            || certificate.phase != wire::GlobalPhase::Commit
            || !source
                .tag
                .matches_decision_round(context, certificate.round)
        {
            return None;
        }
    }
    Some(LiveWalReplayProjectionV1 {
        context,
        stage,
        source,
    })
}
fn canonical_wal_source(source: &WalReplaySourceV1) -> bool {
    let encoded = source.encode();
    if encoded.is_empty() || encoded.len() > MAX_REPLAY_AUTHORITY_BYTES {
        return false;
    }
    let mut cursor = encoded.as_slice();
    WalReplaySourceV1::decode_all(&mut cursor).is_ok_and(|canonical| {
        cursor.is_empty() && canonical == *source && canonical.encode() == encoded
    })
}
fn exact_recovered_wal_control_authority(
    locator: RecoveredWalFrameIdentity,
    effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    if !locator.is_exact() {
        return None;
    }
    let (
        tag,
        round,
        role,
        work_class,
        phase,
        stage_kind,
        action,
        proposal_round,
        subject,
        execution,
    ) = match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } => (
            *tag,
            proposal.round,
            ReplayWalRoleV1::PROPOSAL_INTENT,
            LifecycleWorkClass::SignProposal,
            LifecyclePhase::Proposal,
            LifecycleStageKind::SignProposal,
            WalReplayActionV1::SignProposal(proposal.clone()),
            Some(proposal.round),
            Some(block_subject(proposal.subject)),
            None,
        ),
        AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(vote),
        } => (
            *tag,
            vote.round,
            ReplayWalRoleV1::TIMEOUT_INTENT,
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            LifecycleStageKind::SignTimeoutVote,
            WalReplayActionV1::SignTimeoutVote(vote.clone()),
            vote.highest_prepare_qc
                .as_ref()
                .map(|certificate| certificate.proposal_round),
            vote.highest_prepare_qc
                .as_ref()
                .map(|certificate| block_subject(certificate.subject)),
            vote.highest_prepare_qc
                .as_ref()
                .map(|certificate| execution_commitment(certificate.execution_commitment)),
        ),
        AdapterEffect::Sign {
            request: SignRequest::Vote(_),
            ..
        }
        | AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::Apply { .. }
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => return None,
    };
    if tag.height() != round.height || tag.view() != round.view {
        return None;
    }
    let context =
        LifecycleContext::new(digest_from_bytes(round.context_id.0.as_ref()), round.height);
    let source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role,
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        action,
    });
    let payload = ReplayPayloadBindingV1::None;
    let shape = source.project(context, stage_kind, &payload).ok()?;
    if shape.work_class != work_class
        || shape.stage_kind != stage_kind
        || shape.key != lifecycle_key(context, round, proposal_round, subject, phase, execution)
    {
        return None;
    }
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            work_class,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .ok()
        .map(|_| authority)
}
fn exact_recovered_wal_decision_fetch_authority(
    verified: &VerifiedHeightContext,
    locator: RecoveredWalFrameIdentity,
    effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    if !locator.is_exact() {
        return None;
    }
    let AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: None,
        certified_sources,
        certificate: Some(certificate),
    } = effect
    else {
        return None;
    };
    let expected_sources = verified
        .context()
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let context = super::projection::lifecycle_context(verified.context());
    let replay_tag = ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get());
    if certificate.phase != wire::GlobalPhase::Commit
        || certificate.proposal_round != *round
        || certificate.subject != *subject
        || certified_sources != &expected_sources
        || !replay_tag.matches_decision_round(context, certificate.round)
        || verified.verify_quorum_certificate(certificate).is_err()
    {
        return None;
    }
    let source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role: ReplayWalRoleV1::DECISION,
        tag: replay_tag,
        action: WalReplayActionV1::FetchDecision {
            certificate: certificate.clone(),
            certified_sources: certified_sources.clone(),
        },
    });
    let payload = ReplayPayloadBindingV1::None;
    let shape = source
        .project(context, LifecycleStageKind::FetchBody, &payload)
        .ok()?;
    if shape.work_class != LifecycleWorkClass::Fetch
        || shape.stage_kind != LifecycleStageKind::FetchBody
        || shape.key
            != lifecycle_key(
                context,
                certificate.round,
                Some(certificate.proposal_round),
                Some(block_subject(certificate.subject)),
                LifecyclePhase::Fetch,
                Some(execution_commitment(certificate.execution_commitment)),
            )
    {
        return None;
    }
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .ok()?;
    Some(authority)
}
fn exact_recovered_wal_vote_authority(
    locator: RecoveredWalFrameIdentity,
    tag: EventTag,
    vote: &wire::Vote,
) -> Option<LifecycleReplayAuthorityV1> {
    let tag_matches_vote = tag.height() == vote.round.height
        && match vote.phase {
            wire::GlobalPhase::Prepare => tag.view() == vote.round.view,
            wire::GlobalPhase::Commit => tag.view() >= vote.round.view,
        };
    if !locator.is_exact() || !tag_matches_vote {
        return None;
    }
    let (role, phase, stage_kind) = match vote.phase {
        wire::GlobalPhase::Prepare => (
            ReplayWalRoleV1::PREPARE_INTENT,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ),
        wire::GlobalPhase::Commit => (
            ReplayWalRoleV1::LOCK_AND_COMMIT,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        ),
    };
    let context = LifecycleContext::new(
        digest_from_bytes(vote.round.context_id.0.as_ref()),
        vote.round.height,
    );
    let payload = ReplayPayloadBindingV1::None;
    let source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role,
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        action: WalReplayActionV1::SignVote(vote.clone()),
    });
    let shape = source.project(context, stage_kind, &payload).ok()?;
    if shape.work_class != LifecycleWorkClass::SignVote
        || shape.stage_kind != stage_kind
        || shape.key
            != lifecycle_key(
                context,
                vote.round,
                Some(vote.proposal_round),
                Some(block_subject(vote.subject)),
                phase,
                Some(execution_commitment(vote.execution_commitment)),
            )
    {
        return None;
    }
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            LifecycleWorkClass::SignVote,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .ok()?;
    Some(authority)
}
impl WalReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        if !self.locator.is_exact() {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let shape = match &self.action {
            WalReplayActionV1::SignProposal(proposal) => {
                if !self.role.matches(ReplayWalRoleV1::PROPOSAL_INTENT)
                    || !proposal_shape(context, proposal, false)
                    || !self.tag.matches_round(context, proposal.round)
                    || self.tag.view != proposal.round.view
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        proposal.round,
                        Some(proposal.round),
                        Some(block_subject(proposal.subject)),
                        LifecyclePhase::Proposal,
                        None,
                    ),
                    LifecycleWorkClass::SignProposal,
                    LifecycleStageKind::SignProposal,
                )
            }
            WalReplayActionV1::SignVote(vote) => {
                let (role, phase, stage_kind) = match vote.phase {
                    wire::GlobalPhase::Prepare => (
                        ReplayWalRoleV1::PREPARE_INTENT,
                        LifecyclePhase::Prepare,
                        LifecycleStageKind::SignPrepareVote,
                    ),
                    wire::GlobalPhase::Commit => (
                        ReplayWalRoleV1::LOCK_AND_COMMIT,
                        LifecyclePhase::Commit,
                        LifecycleStageKind::SignCommitVote,
                    ),
                };
                let tag_matches_vote = self.tag.matches_round(context, vote.round)
                    && match vote.phase {
                        wire::GlobalPhase::Prepare => self.tag.view == vote.round.view,
                        wire::GlobalPhase::Commit => true,
                    };
                if !self.role.matches(role)
                    || !vote_shape(context, vote, false)
                    || !tag_matches_vote
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        vote.round,
                        Some(vote.proposal_round),
                        Some(block_subject(vote.subject)),
                        phase,
                        Some(execution_commitment(vote.execution_commitment)),
                    ),
                    LifecycleWorkClass::SignVote,
                    stage_kind,
                )
            }
            WalReplayActionV1::SignTimeoutVote(vote) => {
                if !self.role.matches(ReplayWalRoleV1::TIMEOUT_INTENT)
                    || !timeout_vote_shape(context, vote, false)
                    || !self.tag.matches_round(context, vote.round)
                    || self.tag.view != vote.round.view
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                let highest = vote.highest_prepare_qc.as_ref();
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        vote.round,
                        highest.map(|qc| qc.proposal_round),
                        highest.map(|qc| block_subject(qc.subject)),
                        LifecyclePhase::Timeout,
                        highest.map(|qc| execution_commitment(qc.execution_commitment)),
                    ),
                    LifecycleWorkClass::SignTimeout,
                    LifecycleStageKind::SignTimeoutVote,
                )
            }
            WalReplayActionV1::ApplyDecision(certificate) => {
                if !self.role.matches(ReplayWalRoleV1::DECISION)
                    || !qc_shape(context, certificate)
                    || certificate.phase != wire::GlobalPhase::Commit
                    || !self.tag.matches_decision_round(context, certificate.round)
                    || !payload.matches_body_origin(
                        context,
                        certificate.proposal_round,
                        certificate.subject,
                    )
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        certificate.round,
                        Some(certificate.proposal_round),
                        Some(block_subject(certificate.subject)),
                        LifecyclePhase::Apply,
                        Some(execution_commitment(certificate.execution_commitment)),
                    ),
                    LifecycleWorkClass::Apply,
                    LifecycleStageKind::ApplyDecision,
                )
            }
            WalReplayActionV1::FetchDecision {
                certificate,
                certified_sources,
            } => {
                if !self.role.matches(ReplayWalRoleV1::DECISION)
                    || !qc_shape(context, certificate)
                    || certificate.phase != wire::GlobalPhase::Commit
                    || !self.tag.matches_decision_round(context, certificate.round)
                    || certified_sources.is_empty()
                    || certified_sources.len() > wire::MAX_VALIDATORS_PER_HEIGHT
                    || certified_sources
                        .iter()
                        .enumerate()
                        .any(|(index, source)| certified_sources[..index].contains(source))
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        certificate.round,
                        Some(certificate.proposal_round),
                        Some(block_subject(certificate.subject)),
                        LifecyclePhase::Fetch,
                        Some(execution_commitment(certificate.execution_commitment)),
                    ),
                    LifecycleWorkClass::Fetch,
                    LifecycleStageKind::FetchBody,
                )
            }
            WalReplayActionV1::EnterView {
                certificate,
                protected_lock,
            } => {
                if !self.role.matches(ReplayWalRoleV1::INSTALL_TIMEOUT)
                    || !timeout_certificate_shape(context, certificate)
                    || !enter_view_shape(context, self.tag, certificate, protected_lock.as_ref())
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                let execution_round = wire::ConsensusRound {
                    context_id: certificate.round.context_id,
                    height: certificate.round.height,
                    view: self.tag.view,
                };
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        execution_round,
                        protected_lock.as_ref().map(|lock| lock.proposal_round),
                        protected_lock
                            .as_ref()
                            .map(|lock| block_subject(lock.subject)),
                        LifecyclePhase::EnterView,
                        protected_lock
                            .as_ref()
                            .map(|lock| execution_commitment(lock.execution_commitment)),
                    ),
                    LifecycleWorkClass::EnterView,
                    LifecycleStageKind::EnterView,
                )
            }
        };
        (shape.stage_kind == requested_stage)
            .then_some(shape)
            .ok_or(ReplayAuthorityValidationError::RecordMismatch)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct BodyPipelineReplaySourceV1 {
    tag: ReplayEventTagV1,
    origin: BodyPipelineOriginV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum BodyPipelineOriginV1 {
    #[codec(index = 0)]
    Proposal(wire::Proposal),
    #[codec(index = 1)]
    Certified {
        certificate: wire::QuorumCertificate,
        manifest: wire::PayloadManifest,
        fetch_manifest_present: bool,
        certified_sources: Vec<PeerId>,
    },
    #[codec(index = 2)]
    LocalBody(wire::PayloadManifest),
    #[codec(index = 3)]
    RecoveredDecision {
        locator: PersistedWalFrameLocatorV1,
        certificate: wire::QuorumCertificate,
        manifest: wire::PayloadManifest,
    },
}
impl BodyPipelineReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let (
            round,
            proposal_round,
            subject,
            commitment,
            manifest,
            local_body,
            recovered_decision,
            decision_owned,
        ) = match &self.origin {
            BodyPipelineOriginV1::Proposal(proposal) => {
                if !proposal_shape(context, proposal, true) {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                (
                    proposal.round,
                    proposal.round,
                    proposal.subject,
                    None,
                    Some(&proposal.manifest),
                    false,
                    false,
                    false,
                )
            }
            BodyPipelineOriginV1::Certified {
                certificate,
                manifest,
                fetch_manifest_present: _,
                certified_sources,
            } => {
                if !qc_shape(context, certificate)
                    || !manifest_matches_origin(
                        context,
                        manifest,
                        certificate.proposal_round,
                        certificate.subject,
                    )
                    || !certified_sources_are_bounded_unique(certified_sources)
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                (
                    certificate.round,
                    certificate.proposal_round,
                    certificate.subject,
                    Some(execution_commitment(certificate.execution_commitment)),
                    Some(manifest),
                    false,
                    false,
                    certificate.phase == wire::GlobalPhase::Commit,
                )
            }
            BodyPipelineOriginV1::LocalBody(manifest) => {
                if !round_matches_context(context, manifest.round) {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                (
                    manifest.round,
                    manifest.round,
                    manifest.subject,
                    None,
                    Some(manifest),
                    true,
                    false,
                    false,
                )
            }
            BodyPipelineOriginV1::RecoveredDecision {
                locator,
                certificate,
                manifest,
            } => {
                if !locator.is_exact()
                    || !qc_shape(context, certificate)
                    || certificate.phase != wire::GlobalPhase::Commit
                    || !manifest_matches_origin(
                        context,
                        manifest,
                        certificate.proposal_round,
                        certificate.subject,
                    )
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                (
                    certificate.round,
                    certificate.proposal_round,
                    certificate.subject,
                    Some(execution_commitment(certificate.execution_commitment)),
                    Some(manifest),
                    false,
                    true,
                    true,
                )
            }
        };
        let tag_matches_origin = if decision_owned {
            self.tag.matches_decision_round(context, round)
        } else {
            self.tag.matches_round(context, round)
        };
        if !tag_matches_origin {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let (phase, work_class) = match requested_stage {
            LifecycleStageKind::FetchBody if !recovered_decision => {
                (LifecyclePhase::Fetch, LifecycleWorkClass::Fetch)
            }
            LifecycleStageKind::StoreBody => (LifecyclePhase::Store, LifecycleWorkClass::Store),
            LifecycleStageKind::ValidateBody => {
                (LifecyclePhase::Validate, LifecycleWorkClass::Validate)
            }
            _ => return Err(ReplayAuthorityValidationError::RecordMismatch),
        };
        if local_body && requested_stage == LifecycleStageKind::FetchBody {
            return Err(ReplayAuthorityValidationError::RecordMismatch);
        }
        let key = lifecycle_key(
            context,
            round,
            Some(proposal_round),
            Some(block_subject(subject)),
            phase,
            commitment,
        );
        match requested_stage {
            LifecycleStageKind::FetchBody
                if payload.is_none()
                    || (!local_body
                        && manifest.is_some_and(|manifest| {
                            payload.matches_exact_body(context, proposal_round, subject, manifest)
                        })) => {}
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody
                if manifest.is_some_and(|manifest| {
                    payload.matches_exact_body(context, proposal_round, subject, manifest)
                }) => {}
            _ => return Err(ReplayAuthorityValidationError::PayloadMismatch),
        }
        Ok(ReplayShape::new(key, work_class, requested_stage))
    }
}
fn certified_sources_are_bounded_unique(certified_sources: &[PeerId]) -> bool {
    certified_sources.len() <= wire::MAX_VALIDATORS_PER_HEIGHT
        && certified_sources
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == certified_sources.len()
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct InvalidBodyReplaySourceV1 {
    validation_origin: BodyPipelineReplaySourceV1,
    certificate: wire::QuorumCertificate,
    outcome: RejectedBodyOutcomeBindingV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct RejectedBodyOutcomeBindingV1 {
    manifest: wire::PayloadManifest,
    body_frame_hash: [u8; 32],
    rejection_code: u8,
}
impl InvalidBodyReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let origin_payload = ReplayPayloadBindingV1::BodyFrame(BodyFrameBindingV1 {
            context: *context.id().as_bytes(),
            round_height: self.outcome.manifest.round.height,
            round_view: self.outcome.manifest.round.view,
            subject: *block_subject(self.outcome.manifest.subject).as_bytes(),
            manifest: *HashOf::new(&self.outcome.manifest).as_ref(),
            frame: self.outcome.body_frame_hash,
        });
        let origin_shape = self.validation_origin.project(
            context,
            LifecycleStageKind::ValidateBody,
            &origin_payload,
        )?;
        if requested_stage != LifecycleStageKind::ReportInvalidBody
            || !payload.is_none()
            || !qc_shape(context, &self.certificate)
            || self.certificate.phase != wire::GlobalPhase::Prepare
            || self.certificate.round != self.certificate.proposal_round
            || self.outcome.rejection_code != 0
            || !manifest_matches_origin(
                context,
                &self.outcome.manifest,
                self.certificate.proposal_round,
                self.certificate.subject,
            )
        {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        match &self.validation_origin.origin {
            BodyPipelineOriginV1::Proposal(proposal)
                if proposal.round == self.certificate.proposal_round
                    && proposal.subject == self.certificate.subject
                    && proposal.manifest == self.outcome.manifest => {}
            BodyPipelineOriginV1::Certified {
                certificate,
                manifest,
                ..
            } if certificate == &self.certificate && manifest == &self.outcome.manifest => {}
            BodyPipelineOriginV1::Proposal(_)
            | BodyPipelineOriginV1::Certified { .. }
            | BodyPipelineOriginV1::LocalBody(_)
            | BodyPipelineOriginV1::RecoveredDecision { .. } => {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
        }
        if origin_shape.work_class != LifecycleWorkClass::Validate
            || origin_shape.stage_kind != LifecycleStageKind::ValidateBody
            || origin_shape.key.context() != context.id()
            || origin_shape.key.round()
                != LifecycleRound::new(
                    self.certificate.proposal_round.height,
                    self.certificate.proposal_round.view,
                )
            || origin_shape.key.proposal_round()
                != Some(LifecycleRound::new(
                    self.certificate.proposal_round.height,
                    self.certificate.proposal_round.view,
                ))
            || origin_shape.key.subject() != Some(block_subject(self.certificate.subject))
        {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        Ok(ReplayShape::new(
            lifecycle_key(
                context,
                self.certificate.round,
                Some(self.certificate.proposal_round),
                Some(block_subject(self.certificate.subject)),
                LifecyclePhase::DiagnosticInvalidBody,
                Some(execution_commitment(self.certificate.execution_commitment)),
            ),
            LifecycleWorkClass::InvalidBodyReport,
            LifecycleStageKind::ReportInvalidBody,
        ))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct CertifiedServeStorageSourceV1 {
    request: wire::CertifiedBodyRequest,
    payload_hash: [u8; 32],
    local_retainer: wire::ValidatorIndex,
}
impl CertifiedServeStorageSourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let certificate = &self.request.certificate;
        let local_retainer = usize::try_from(self.local_retainer)
            .map_err(|_| ReplayAuthorityValidationError::InvalidSource)?;
        if local_retainer >= wire::MAX_VALIDATORS_PER_HEIGHT
            || !signature_present(&self.request.signature)
            || !round_matches_context(context, self.request.round)
            || !qc_shape(context, certificate)
            || certificate.proposal_round != self.request.round
            || certificate.subject != self.request.subject
            || certificate
                .signers
                .binary_search(&self.local_retainer)
                .is_err()
        {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let request_hash = HashOf::new(&self.request);
        let request_digest = digest_from_bytes(request_hash.as_ref());
        let certificate_digest = digest_from_bytes(HashOf::new(certificate).as_ref());
        let phase = match requested_stage {
            LifecycleStageKind::CertifiedServe => LifecyclePhase::Serve,
            LifecycleStageKind::ProducerTurn => LifecyclePhase::ProducerTurn,
            _ => return Err(ReplayAuthorityValidationError::RecordMismatch),
        };
        let key = lifecycle_key(
            context,
            certificate.round,
            Some(self.request.round),
            Some(certified_serve_key_subject(
                self.request.subject,
                request_hash,
            )),
            phase,
            Some(execution_commitment(certificate.execution_commitment)),
        );
        let work_class = match requested_stage {
            LifecycleStageKind::CertifiedServe => {
                if !payload.matches_certified_serve(request_digest, certificate_digest) {
                    return Err(ReplayAuthorityValidationError::PayloadMismatch);
                }
                LifecycleWorkClass::CertifiedServe
            }
            LifecycleStageKind::ProducerTurn => {
                if !payload.is_none() {
                    return Err(ReplayAuthorityValidationError::PayloadMismatch);
                }
                LifecycleWorkClass::ProducerTurn
            }
            _ => unreachable!("stage was checked above"),
        };
        Ok(ReplayShape::new(key, work_class, requested_stage))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences)]
enum ReplayPayloadBindingV1 {
    #[codec(index = 0)]
    None,
    #[codec(index = 1)]
    BodyFrame(BodyFrameBindingV1),
    #[codec(index = 2)]
    CertifiedServePending {
        request: [u8; 32],
        certificate: [u8; 32],
    },
    #[codec(index = 3)]
    CertifiedServeCompleted {
        request: [u8; 32],
        certificate: [u8; 32],
        response: [u8; 32],
    },
    #[codec(index = 4)]
    CertifiedServeNegative {
        request: [u8; 32],
        certificate: [u8; 32],
        outcome_kind: u8,
        outcome_code: Option<u16>,
    },
}
impl ReplayPayloadBindingV1 {
    fn from_payload(payload: DurablePayloadReference) -> Self {
        match payload {
            DurablePayloadReference::None => Self::None,
            DurablePayloadReference::BodyFrame(frame) => Self::BodyFrame(BodyFrameBindingV1 {
                context: *frame.context.as_bytes(),
                round_height: frame.round.height(),
                round_view: frame.round.view(),
                subject: *frame.subject.as_bytes(),
                manifest: *frame.manifest.as_bytes(),
                frame: *frame.frame.as_bytes(),
            }),
            DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            } => Self::CertifiedServePending {
                request: *request.as_bytes(),
                certificate: *certificate.as_bytes(),
            },
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response,
            } => Self::CertifiedServeCompleted {
                request: *request.as_bytes(),
                certificate: *certificate.as_bytes(),
                response: *response.as_bytes(),
            },
            DurablePayloadReference::CertifiedServeNegative {
                request,
                certificate,
                outcome,
            } => {
                let (outcome_kind, outcome_code) = match outcome {
                    DurableServeNegativeOutcome::Cancelled => (0, None),
                    DurableServeNegativeOutcome::Rejected(code) => (1, Some(code)),
                    DurableServeNegativeOutcome::Failed(code) => (2, Some(code)),
                };
                Self::CertifiedServeNegative {
                    request: *request.as_bytes(),
                    certificate: *certificate.as_bytes(),
                    outcome_kind,
                    outcome_code,
                }
            }
        }
    }
    fn matches(&self, payload: DurablePayloadReference) -> bool {
        *self == Self::from_payload(payload)
    }
    fn durable_payload(&self) -> Option<DurablePayloadReference> {
        Some(match self {
            Self::None => DurablePayloadReference::None,
            Self::BodyFrame(frame) => DurablePayloadReference::BodyFrame(frame.durable_reference()),
            Self::CertifiedServePending {
                request,
                certificate,
            } => DurablePayloadReference::CertifiedServePending {
                request: LifecycleDigest::new(*request),
                certificate: LifecycleDigest::new(*certificate),
            },
            Self::CertifiedServeCompleted {
                request,
                certificate,
                response,
            } => DurablePayloadReference::CertifiedServeCompleted {
                request: LifecycleDigest::new(*request),
                certificate: LifecycleDigest::new(*certificate),
                response: LifecycleDigest::new(*response),
            },
            Self::CertifiedServeNegative {
                request,
                certificate,
                outcome_kind,
                outcome_code,
            } => DurablePayloadReference::CertifiedServeNegative {
                request: LifecycleDigest::new(*request),
                certificate: LifecycleDigest::new(*certificate),
                outcome: match (*outcome_kind, *outcome_code) {
                    (0, None) => DurableServeNegativeOutcome::Cancelled,
                    (1, Some(code)) => DurableServeNegativeOutcome::Rejected(code),
                    (2, Some(code)) => DurableServeNegativeOutcome::Failed(code),
                    _ => return None,
                },
            },
        })
    }
    const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    fn matches_exact_body(
        &self,
        context: LifecycleContext,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        manifest: &wire::PayloadManifest,
    ) -> bool {
        let Self::BodyFrame(frame) = self else {
            return false;
        };
        frame.matches_origin(context, proposal_round, subject)
            && frame.manifest == *digest_from_bytes(HashOf::new(manifest).as_ref()).as_bytes()
    }
    fn matches_body_origin(
        &self,
        context: LifecycleContext,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        match self {
            Self::BodyFrame(frame) => frame.matches_origin(context, proposal_round, subject),
            Self::None
            | Self::CertifiedServePending { .. }
            | Self::CertifiedServeCompleted { .. }
            | Self::CertifiedServeNegative { .. } => false,
        }
    }
    fn matches_certified_serve(
        &self,
        expected_request: LifecycleDigest,
        expected_certificate: LifecycleDigest,
    ) -> bool {
        let (request, certificate) = match self {
            Self::CertifiedServePending {
                request,
                certificate,
            }
            | Self::CertifiedServeCompleted {
                request,
                certificate,
                ..
            }
            | Self::CertifiedServeNegative {
                request,
                certificate,
                ..
            } => (request, certificate),
            Self::None | Self::BodyFrame(_) => return false,
        };
        request == expected_request.as_bytes() && certificate == expected_certificate.as_bytes()
    }
}

/// Published local-proposal command plus the replay authority retained beside it.
#[must_use = "published local-proposal replay authority must be reconciled by the executor"]
pub(in crate::sumeragi) struct PublishedLifecycleLocalProposalReadyV1 {
    command_identity: LocalProposalReadyCommandIdentity,
    command_was_coalesced: bool,
    replay: LocalProposalReadyReplayEvidenceV1,
}

impl PublishedLifecycleLocalProposalReadyV1 {
    /// Consume the published bundle into the executor's replay index entry.
    pub(in crate::sumeragi) fn into_entry(
        self,
    ) -> (
        LocalProposalReadyCommandIdentity,
        LocalProposalReadyReplayEvidenceV1,
    ) {
        (self.command_identity, self.replay)
    }

    /// Borrow the inert command identity for exact duplicate lookup.
    pub(in crate::sumeragi) const fn command_identity(&self) -> LocalProposalReadyCommandIdentity {
        self.command_identity
    }

    /// Return whether runtime admission consumed no new FIFO owner.
    pub(in crate::sumeragi) const fn command_was_coalesced(&self) -> bool {
        self.command_was_coalesced
    }
}
