impl AuthenticatedRecoveredAdapterStartup {
    /// Borrow the exact marker frontier derived before the startup effect was sealed.
    ///
    /// The authority remains a bounded comparison capability. Neither the
    /// replay batch nor the adapter crosses this boundary.
    #[cfg(test)]
    const fn recovered_validation_authority(&self) -> &RecoveredValidationAuthority {
        &self.validation_authority
    }
    /// Largest producer ordinal authenticated by the replayed adjacent store.
    #[cfg(test)]
    const fn restored_producer_continuation_ordinal_high_watermark(&self) -> Option<u128> {
        self.adapter
            .restored_producer_continuation_ordinal_high_watermark()
    }
    /// Reconstruct the exact generic leader-wire recovery boundary.
    #[cfg(test)]
    fn leader_wire_recovery_authority(&self) -> Result<LeaderWireRecoveryAuthority, AdapterError> {
        self.adapter.leader_wire_recovery_authority()
    }
    /// Clone only typed durable producer terminals needed by the adjacent gate.
    #[cfg(test)]
    fn durable_producer_terminal_tokens(&self) -> Vec<ProducerContinuationTerminalToken> {
        self.adapter.durable_producer_terminal_tokens()
    }
    /// Consume recovered storage into one exact lifecycle execution-input seal.
    ///
    /// The State must own the supplied Kura Arc and the recovered adapter's
    /// network. The private runner permit supplies both the local signer and
    /// the cadence authenticated by signed-genesis or snapshot recovery. Fresh
    /// height-one startup must not derive cadence from the uncommitted State
    /// placeholder.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn bind_production_lifecycle_owner_factory_inputs_v1(
        &self,
        permit: super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1,
        storage: RecoveredLifecycleStorageAuthorityV1,
        state: Arc<crate::state::State>,
        queue: Arc<crate::queue::Queue>,
        kura: Arc<Kura>,
        provider_ingest_finalized_archive: Option<
            Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
        >,
        reputation_finalized_archive: Option<
            Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
        >,
        events_sender: crate::EventsSender,
    ) -> Result<RecoveredLifecycleOwnerFactoryInputsV1, ProductionLifecycleOwnerStartupErrorV1>
    {
        if !storage.kura_identity.matches(kura.as_ref())
            || !state.matches_kura_instance(&kura)
            || state.network_id_ref() != &self.adapter.wire_context.network_id
        {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::ExecutionIdentity,
            ));
        }
        let (local_signer, block_cadence) = permit.into_factory_dependencies();
        Ok(RecoveredLifecycleOwnerFactoryInputsV1 {
            adapter_owner: Arc::clone(&self.factory_owner),
            storage,
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            block_cadence,
            events_sender,
            local_signer,
        })
    }
    /// Consume all recovered adapter and storage authority into one V1 owner.
    ///
    /// The no-authority, phase-vote, control-Sign, and Decision-Fetch cases are
    /// private branches over this type's sealed authority enum. The lifecycle
    /// Ledger and Serve stores are derived internally from the exact Kura owner
    /// and frozen context; callers cannot substitute raw publication roots.
    /// The fresh quarantined body store must belong to that same Kura layout
    /// and the exact recovery-authenticated signature policy. Its sole
    /// consuming replay transition applies the finality and WAL marker filters,
    /// replays semantic validation with the exact service retained for live
    /// Apply, and only then seals the store so lifecycle storage may open. No
    /// body root, recovery snapshot, callback, effect, pending binding, ordinal,
    /// or branch selector crosses this boundary.
    #[allow(
        clippy::result_large_err,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1,
        body_store: super::v2_body_store::QuarantinedV2BodyStore,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        if !self.effects.is_empty() {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::ResidualEffects,
            ));
        }
        let RecoveredLifecycleOwnerFactoryInputsV1 {
            adapter_owner,
            storage,
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            block_cadence,
            events_sender,
            local_signer,
        } = factory_inputs;
        let context = self.adapter.wire_context.clone();
        if !Arc::ptr_eq(&adapter_owner, &self.factory_owner) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::ExecutionIdentity,
            ));
        }
        if storage.context_id != context.id()
            || storage.height != context.height
            || !body_store.matches_lifecycle_storage_root(
                &storage.body_store_root,
                &context,
                &storage.signature_policy,
            )
        {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(
                    super::v2_body_store::V2BodyStoreError::StoreRootMismatch,
                ),
            ));
        }
        if !self.adapter.wal.matches_path(&storage.wal_path) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::StorageLayout,
            ));
        }
        let validator_set_pops = self.adapter.proofs_of_possession.clone();
        let validation_authority = self.validation_authority.clone();
        let apply_service = super::v2_apply::V2ApplyService::new(
            Arc::clone(&state),
            queue,
            Arc::clone(&kura),
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            block_cadence,
            storage.genesis_account.clone(),
            events_sender,
            validator_set_pops.clone(),
        );
        if !apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::ExecutionIdentity,
            ));
        }
        let body_store = body_store
            .into_revalidated_lifecycle_startup(&apply_service, &context, validation_authority)
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::MarkerReplay(error),
                )
            })?;
        let RecoveredLifecycleStorageAuthorityV1 {
            kura_identity,
            wal_path,
            chunk_root,
            lifecycle_root,
            successor_floor,
            ..
        } = storage;
        let owner = self.open_production_lifecycle_owner_v1_at_authenticated_roots(
            config,
            reply_route_source_capacity,
            &lifecycle_root,
            &lifecycle_root,
            body_store,
            &local_signer,
        )?;
        let owner = match successor_floor {
            Some(floor) => owner
                .authenticate_recovered_successor_floor(floor)
                .map_err(|error| {
                    ProductionLifecycleOwnerStartupErrorV1::new(
                        ProductionLifecycleOwnerStartupErrorKindV1::SuccessorFloor(error),
                    )
                })?,
            None => owner,
        };
        let kura_binding = RecoveredLifecycleOwnerKuraBindingV1 {
            kura_identity,
            wal_path,
            chunk_root,
            local_signer: Some(local_signer.public_key().clone()),
        };
        Ok(owner.with_recovered_kura_binding_and_apply_service(kura_binding, apply_service))
    }
    /// Shared implementation after production or a test-only fixture has
    /// authenticated the complete lifecycle storage target.
    #[allow(
        clippy::result_large_err,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    #[inline(never)]
    fn open_production_lifecycle_owner_v1_at_authenticated_roots(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        if !self.effects.is_empty() {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::ResidualEffects,
            ));
        }
        let Self {
            adapter,
            effects,
            authority,
            validation_authority,
            factory_owner,
        } = self;
        let verified = VerifiedHeightContext {
            context: adapter.wire_context.clone(),
            proofs_of_possession: adapter.proofs_of_possession.clone(),
            parent_verification: adapter.parent_verification.clone(),
        };
        let local_proposal_attempt =
            RecoveredLifecycleLocalProposalAttemptV1::from_authenticated_durable_current_round(
                &adapter,
            )
            .map_err(|_| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredLocalProposal(
                        "current-round durable ProposalIntent projection is inconsistent",
                    ),
                )
            })?;
        match authority {
            RecoveredWalStartupAuthorityV1::None => Self::open_recovered_no_authority_branch(
                verified,
                adapter,
                effects,
                local_proposal_attempt,
                body_store,
                config,
                reply_route_source_capacity,
                ledger_root,
                serve_payload_root,
                local_signer,
            ),
            RecoveredWalStartupAuthorityV1::ControlSign(control) => {
                Self::open_recovered_control_authority_branch(
                    verified,
                    adapter,
                    effects,
                    control,
                    local_proposal_attempt,
                    body_store,
                    config,
                    reply_route_source_capacity,
                    ledger_root,
                    serve_payload_root,
                    local_signer,
                )
            }
            RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) => {
                // A Decision terminally suppresses proposal work through the
                // reducer directive. Its startup branch intentionally accepts
                // no runner-local proposal-attempt owner.
                drop(local_proposal_attempt);
                Self::open_recovered_decision_authority_branch(
                    verified,
                    adapter,
                    effects,
                    fetch,
                    body_store,
                    config,
                    reply_route_source_capacity,
                    ledger_root,
                    serve_payload_root,
                    local_signer,
                )
            }
            RecoveredWalStartupAuthorityV1::PhaseVote(vote) => {
                let phase_startup = Self {
                    adapter,
                    effects,
                    authority: RecoveredWalStartupAuthorityV1::PhaseVote(vote),
                    validation_authority,
                    factory_owner,
                };
                Self::open_recovered_phase_vote_branch(
                    phase_startup,
                    verified,
                    local_proposal_attempt,
                    body_store,
                    config,
                    reply_route_source_capacity,
                    ledger_root,
                    serve_payload_root,
                    local_signer,
                )
            }
        }
    }
    #[allow(clippy::result_large_err)]
    fn ensure_recovered_body_store_context(
        body_store: &super::v2_body_store::RevalidatedV2BodyStore,
        verified: &VerifiedHeightContext,
    ) -> Result<(), ProductionLifecycleOwnerStartupErrorV1> {
        if body_store.matches_context(verified.context()) {
            return Ok(());
        }
        Err(ProductionLifecycleOwnerStartupErrorV1::new(
            ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(
                super::v2_body_store::V2BodyStoreError::ContextMismatch,
            ),
        ))
    }
    // Standalone WAL authority is projected before the already-revalidated
    // body cut is unsealed and before Serve/Ledger open. A malformed token
    // therefore cannot mutate or observe any newly opened storage owner.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_control_authority_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        control: RecoveredWalControlSign,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        if let Some(control_attempt) =
            RecoveredLifecycleLocalProposalAttemptV1::from_control(&control)
            && local_proposal_attempt.as_ref().is_none_or(|durable| {
                durable.tag != control_attempt.tag
                    || durable.round != control_attempt.round
                    || durable.subject != control_attempt.subject
            })
        {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredLocalProposal(
                    "Proposal Sign differs from its durable current-round ProposalIntent",
                ),
            ));
        }
        let projected =
            crate::sumeragi::v2_runtime::project_recovered_wal_control_sign(&verified, control)
                .map_err(|_control| {
                    ProductionLifecycleOwnerStartupErrorV1::new(
                        ProductionLifecycleOwnerStartupErrorKindV1::RecoveredControl(
                            "recovered control Sign projection is inconsistent",
                        ),
                    )
                })?;
        Self::ensure_recovered_body_store_context(&body_store, &verified)?;
        Self::open_recovered_control_projection_branch(
            verified,
            adapter,
            effects,
            projected,
            local_proposal_attempt,
            body_store,
            config,
            reply_route_source_capacity,
            ledger_root,
            serve_payload_root,
            local_signer,
        )
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_decision_authority_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        fetch: RecoveredWalDecisionFetch,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let fetch =
            crate::sumeragi::v2_runtime::project_recovered_wal_decision_fetch(&verified, fetch)
                .map_err(|_fetch| {
                    ProductionLifecycleOwnerStartupErrorV1::new(
                        ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionFetch(
                            "recovered Decision Fetch projection is inconsistent",
                        ),
                    )
                })?;
        Self::ensure_recovered_body_store_context(&body_store, &verified)?;
        if body_store.has_rejected_recovered_decision_body(&fetch) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionBody(
                    super::v2_body_store::RecoveredDecisionApplyBodyCutError::DeterministicRejection,
                ),
            ));
        }
        if body_store.has_exact_recovered_decision_fetch_parent(&fetch) {
            return Self::open_recovered_decision_apply_branch(
                verified,
                adapter,
                effects,
                fetch,
                body_store,
                config,
                reply_route_source_capacity,
                ledger_root,
                serve_payload_root,
                local_signer,
            );
        }
        Self::open_recovered_decision_fetch_projection_branch(
            verified,
            adapter,
            effects,
            fetch,
            body_store,
            config,
            reply_route_source_capacity,
            ledger_root,
            serve_payload_root,
            local_signer,
        )
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn prepare_recovered_decision_apply_branch(
        verified: &VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        mut body_store: super::v2_body_store::RevalidatedV2BodyStore,
    ) -> Result<
        Box<PreparedRecoveredDecisionApplyOwnerOpenV1>,
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        // Establish marker presence before taking the move-only body cut so
        // the cut's borrow ends before the store is consumed.
        let body = body_store
            .detach_recovered_decision_apply_body(&fetch)
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionBody(error),
                )
            })?;
        if !body.exactly_matches_decision(&fetch) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                    "detached Decision body lost its exact same-store binding",
                ),
            ));
        }
        let lineage = body
            .prepare_replay_lineage(verified, &fetch)
            .ok_or_else(|| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                        "Decision body replay lineage is inconsistent",
                    ),
                )
            })?;
        let preview = body
            .into_adapter_preview(adapter, verified, fetch, lineage)
            .map_err(|error| {
                let _ = error.reason();
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                        "Decision body reducer fast-forward is inconsistent",
                    ),
                )
            })?;
        let storage = preview.into_storage_preview(verified).map_err(|_error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                    "Decision body storage lineage is inconsistent",
                ),
            )
        })?;
        if !storage.validates(verified) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                    "Decision body storage projection lost its same-store binding",
                ),
            ));
        }
        let restored = storage.restore_body();
        debug_assert!(restored.staged().validates(verified));
        Ok(Box::new(PreparedRecoveredDecisionApplyOwnerOpenV1 {
            staged: restored.into_staged(),
            body_store,
        }))
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_prepared_recovered_decision_apply_branch(
        verified: VerifiedHeightContext,
        effects: Vec<AdapterEffect>,
        prepared: Box<PreparedRecoveredDecisionApplyOwnerOpenV1>,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let PreparedRecoveredDecisionApplyOwnerOpenV1 { staged, body_store } = *prepared;
        let body_store = body_store
            .into_lifecycle_owner_store(verified.context())
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
                )
            })?;
        let payload_store_open = if body_store.emergency_read_only() {
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open_emergency_fast_read_only(
                serve_payload_root,
                verified.context(),
            )
        } else {
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                serve_payload_root,
                verified.context(),
            )
        };
        let (payload_store, recovered_payloads) = payload_store_open
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::ServeStore(error),
                )
            })?;
        let serve_payloads = recovered_payloads
            .authenticate(&verified, local_signer, &body_store)
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::ServeRecovery(error),
                )
            })?;
        ProductionLifecycleOwnerV1::open_recovered_decision_apply_startup(
            verified,
            staged,
            effects,
            ledger_root,
            body_store,
            config,
            reply_route_source_capacity,
            payload_store,
            serve_payloads,
        )
        .map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(error.reason()),
            )
        })
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_decision_apply_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let prepared =
            Self::prepare_recovered_decision_apply_branch(&verified, adapter, fetch, body_store)?;
        Self::open_prepared_recovered_decision_apply_branch(
            verified,
            effects,
            prepared,
            config,
            reply_route_source_capacity,
            ledger_root,
            serve_payload_root,
            local_signer,
        )
    }
    #[allow(clippy::result_large_err, clippy::type_complexity)]
    #[inline(never)]
    fn open_recovered_non_apply_stores(
        verified: &VerifiedHeightContext,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<
        (
            super::v2_body_store::V2BodyStore,
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
            super::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
        ),
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        let body_store = body_store
            .into_lifecycle_owner_store(verified.context())
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
                )
            })?;
        let payload_store_open = if body_store.emergency_read_only() {
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open_emergency_fast_read_only(
                serve_payload_root,
                verified.context(),
            )
        } else {
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                serve_payload_root,
                verified.context(),
            )
        };
        let (payload_store, recovered_payloads) = payload_store_open
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::ServeStore(error),
                )
            })?;
        let serve_payloads = recovered_payloads
            .authenticate(verified, local_signer, &body_store)
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::ServeRecovery(error),
                )
            })?;
        Ok((body_store, payload_store, serve_payloads))
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_no_authority_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        Self::ensure_recovered_body_store_context(&body_store, &verified)?;
        let (body_store, payload_store, serve_payloads) = Self::open_recovered_non_apply_stores(
            &verified,
            body_store,
            serve_payload_root,
            local_signer,
        )?;
        ProductionLifecycleOwnerV1::open_storage_only_recovered_startup(
            verified,
            ledger_root,
            body_store,
            config,
            reply_route_source_capacity,
            payload_store,
            serve_payloads,
            ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(
                adapter,
                effects,
                local_proposal_attempt,
            ),
        )
        .map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::StorageOnly(error),
            )
        })
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_control_projection_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        control: AuthenticatedRecoveredWalControlProjection,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let (body_store, payload_store, serve_payloads) = Self::open_recovered_non_apply_stores(
            &verified,
            body_store,
            serve_payload_root,
            local_signer,
        )?;
        ProductionLifecycleOwnerV1::open_recovered_control_startup(
            verified,
            control,
            ledger_root,
            body_store,
            config,
            reply_route_source_capacity,
            payload_store,
            serve_payloads,
            ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(
                adapter,
                effects,
                local_proposal_attempt,
            ),
        )
        .map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredControl(error.reason()),
            )
        })
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_decision_fetch_projection_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let (body_store, payload_store, serve_payloads) = Self::open_recovered_non_apply_stores(
            &verified,
            body_store,
            serve_payload_root,
            local_signer,
        )?;
        ProductionLifecycleOwnerV1::open_recovered_decision_fetch_startup(
            verified,
            fetch,
            ledger_root,
            body_store,
            config,
            reply_route_source_capacity,
            payload_store,
            serve_payloads,
            ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
        )
        .map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionFetch(error.reason()),
            )
        })
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn authenticate_recovered_phase_vote_stage<'registry>(
        phase_startup: AuthenticatedRecoveredAdapterStartup,
        registry: &'registry mut LifecycleWorkRegistryHolder,
        body_store: &mut super::v2_body_store::V2BodyStore,
        ledger_root: &std::path::Path,
    ) -> Result<
        Box<StorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        let authenticated = phase_startup
            .authenticate_recovered_parent_from_storage(registry, body_store, ledger_root)
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredParent(error.reason()),
                )
            })?;
        Ok(Box::new(authenticated))
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn persist_recovered_phase_vote_stage<'registry>(
        authenticated: Box<StorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
    ) -> Result<
        Box<PersistedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        let persisted = (*authenticated).persist_repair().map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::Persist(error.reason()),
            )
        })?;
        Ok(persisted)
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn prepare_recovered_phase_vote_cold_adapter_stage<'registry>(
        persisted: Box<PersistedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        body_store: &super::v2_body_store::V2BodyStore,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
    ) -> Result<
        Box<ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        let PersistedStorageAuthenticatedRecoveredWalLifecycleStartup {
            adapter,
            effects,
            persisted,
        } = *persisted;
        let verified = VerifiedHeightContext {
            context: adapter.wire_context.clone(),
            proofs_of_possession: adapter.proofs_of_possession.clone(),
            parent_verification: adapter.parent_verification.clone(),
        };
        let adapter_startup =
            ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(
                adapter,
                effects,
                local_proposal_attempt,
            );
        let (adapter_startup, persisted) = persisted
            .prepare_cold_adapter_startup(&verified, adapter_startup, body_store)
            .map_err(|reason| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::SignInstall(reason),
                )
            })?;
        Ok(Box::new(
            ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup {
                adapter_startup,
                verified,
                persisted,
            },
        ))
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn install_recovered_phase_vote_sign_stage<'registry>(
        prepared: Box<ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
    ) -> Result<
        Box<InstalledStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        let installed = (*prepared).install_recovered_sign().map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::SignInstall(error.reason()),
            )
        })?;
        Ok(Box::new(installed))
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_phase_vote_seals_stage(
        installed: Box<InstalledStorageAuthenticatedRecoveredWalLifecycleStartup<'_>>,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        body_store: &mut super::v2_body_store::V2BodyStore,
        payload_store: &mut super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        serve_payloads: super::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<
        Box<ProductionRecoveredLifecycleOwnerStartupV1>,
        ProductionLifecycleOwnerStartupErrorV1,
    > {
        let paired = (*installed)
            .open_production_owner_seals(
                config,
                reply_route_source_capacity,
                body_store,
                payload_store,
                serve_payloads,
            )
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredOpen(error.reason()),
                )
            })?;
        Ok(Box::new(paired))
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn finish_recovered_phase_vote_owner_stage(
        paired: Box<ProductionRecoveredLifecycleOwnerStartupV1>,
        registry: Box<LifecycleWorkRegistryHolder>,
        payload_store: super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        body_store: super::v2_body_store::V2BodyStore,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let registry = *registry;
        (*paired)
            .into_owner(registry, payload_store, body_store)
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredOwner(error),
                )
            })
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    #[inline(never)]
    fn open_recovered_phase_vote_branch(
        phase_startup: AuthenticatedRecoveredAdapterStartup,
        verified: VerifiedHeightContext,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        Self::ensure_recovered_body_store_context(&body_store, &verified)?;
        let (mut body_store, mut payload_store, serve_payloads) =
            Self::open_recovered_non_apply_stores(
                &verified,
                body_store,
                serve_payload_root,
                local_signer,
            )?;
        let mut registry = Box::new(LifecycleWorkRegistryHolder::empty());
        let authenticated = Self::authenticate_recovered_phase_vote_stage(
            phase_startup,
            registry.as_mut(),
            &mut body_store,
            ledger_root,
        )?;
        let persisted = Self::persist_recovered_phase_vote_stage(authenticated)?;
        let prepared = Self::prepare_recovered_phase_vote_cold_adapter_stage(
            persisted,
            &body_store,
            local_proposal_attempt,
        )?;
        let installed = Self::install_recovered_phase_vote_sign_stage(prepared)?;
        let paired = Self::open_recovered_phase_vote_seals_stage(
            installed,
            config,
            reply_route_source_capacity,
            &mut body_store,
            &mut payload_store,
            serve_payloads,
        )?;
        Self::finish_recovered_phase_vote_owner_stage(paired, registry, payload_store, body_store)
    }
    /// Open an empty-marker test body store and enter the exact production handoff.
    ///
    /// Production has no root-reopen counterpart. Fixtures which persist a
    /// terminal marker must explicitly replay it and pass the resulting sealed
    /// store to [`Self::open_production_lifecycle_owner_v1`].
    #[cfg(test)]
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    fn open_production_lifecycle_owner_v1_from_roots_for_test(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        body_root: &std::path::Path,
        body_signature_policy: super::v2_body_store::BlockSignaturePolicy,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        if !self.effects.is_empty() {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::ResidualEffects,
            ));
        }
        let mut body_store = super::v2_body_store::V2BodyStore::open_with_policy(
            body_root,
            self.adapter.wire_context.clone(),
            body_signature_policy,
        )
        .map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
            )
        })?;
        body_store
            .ensure_recovered_markers_revalidated()
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
                )
            })?;
        body_store
            .revalidate_recovered_markers(|_| -> Result<wire::ExecutionCommitment, String> {
                unreachable!("the root-only fixture admits no recovered markers")
            })
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
                )
            })?;
        let body_store = body_store.into_revalidated_startup().map_err(|error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
            )
        })?;
        self.open_production_lifecycle_owner_v1_at_authenticated_roots(
            config,
            reply_route_source_capacity,
            ledger_root,
            serve_payload_root,
            body_store,
            local_signer,
        )
    }
    /// Enter the shared owner implementation with an already-revalidated test store.
    #[cfg(test)]
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    fn open_production_lifecycle_owner_v1_with_store_for_test(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        self.open_production_lifecycle_owner_v1_at_authenticated_roots(
            config,
            reply_route_source_capacity,
            ledger_root,
            serve_payload_root,
            body_store,
            local_signer,
        )
    }
    #[cfg(test)]
    fn recovered_phase_vote_for_test(&self) -> Option<&RecoveredWalVoteSign> {
        match &self.authority {
            RecoveredWalStartupAuthorityV1::PhaseVote(vote) => Some(vote),
            RecoveredWalStartupAuthorityV1::None
            | RecoveredWalStartupAuthorityV1::ControlSign(_)
            | RecoveredWalStartupAuthorityV1::DecisionFetch(_) => None,
        }
    }
    #[cfg(test)]
    fn has_no_recovered_wal_authority_for_test(&self) -> bool {
        matches!(self.authority, RecoveredWalStartupAuthorityV1::None)
    }
    #[cfg(test)]
    fn has_recovered_control_sign_for_test(&self) -> bool {
        matches!(
            self.authority,
            RecoveredWalStartupAuthorityV1::ControlSign(_)
        )
    }
    /// Finish a startup whose current reducer state owns no WAL continuation.
    ///
    /// This extraction is retained only for focused adapter tests. Production
    /// startup always consumes the wrapper through
    /// [`Self::open_production_lifecycle_owner_v1`], including the no-vote case.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    fn finish_without_wal_vote(
        mut self,
    ) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), (AdapterError, Self)> {
        if !matches!(self.authority, RecoveredWalStartupAuthorityV1::None) {
            return Err((AdapterError::RecoveredVoteSignMismatch, self));
        }
        if let Err(error) = self.adapter.publish_status() {
            return Err((error, self));
        }
        Ok((self.adapter, self.effects))
    }
    /// Reconstruct the recovered Validate parent from exact durable storage.
    ///
    /// This production factory consumes no scheduler lease, runtime ordinal
    /// source, or caller-minted effect/pending pair. Success retains the
    /// authenticated WAL repair and exact opened LedgerV1 store/frame beside
    /// the adapter. The repair keeps its registry borrow exclusive. Every
    /// failure returns one opaque owner of the complete remaining startup
    /// authority.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    fn authenticate_recovered_parent_from_storage<'registry, 'body>(
        mut self,
        registry: &'registry mut LifecycleWorkRegistryHolder,
        body_store: &'body mut super::v2_body_store::V2BodyStore,
        ledger_root: &std::path::Path,
    ) -> Result<
        StorageAuthenticatedRecoveredWalLifecycleStartup<'registry>,
        StorageAuthenticatedRecoveredWalLifecycleStartupError<'body>,
    > {
        let verified = VerifiedHeightContext {
            context: self.adapter.wire_context.clone(),
            proofs_of_possession: self.adapter.proofs_of_possession.clone(),
            parent_verification: self.adapter.parent_verification.clone(),
        };
        if !matches!(self.authority, RecoveredWalStartupAuthorityV1::PhaseVote(_)) {
            return Err(StorageAuthenticatedRecoveredWalLifecycleStartupError {
                failure: StorageAuthenticatedRecoveredWalLifecycleStartupFailure::MissingVote {
                    _startup: self,
                },
            });
        }
        let RecoveredWalStartupAuthorityV1::PhaseVote(recovered) =
            core::mem::replace(&mut self.authority, RecoveredWalStartupAuthorityV1::None)
        else {
            unreachable!("phase-vote shape checked before replacement")
        };
        match registry.reconstruct_recovered_wal_validate_parent(
            &verified,
            body_store,
            ledger_root,
            recovered,
        ) {
            Ok((ledger, repair)) => Ok(StorageAuthenticatedRecoveredWalLifecycleStartup {
                adapter: self.adapter,
                effects: self.effects,
                ledger,
                repair,
            }),
            Err(error) => Err(StorageAuthenticatedRecoveredWalLifecycleStartupError {
                failure: StorageAuthenticatedRecoveredWalLifecycleStartupFailure::Factory {
                    _adapter: self.adapter,
                    _effects: self.effects,
                    _error: error,
                },
            }),
        }
    }
    /// Join the recovered phase vote to one opaque exact-Validate registry cut.
    ///
    /// The cut owns the closed validated completion; no raw effect or pending
    /// binding crosses this boundary. Success keeps the adapter, remaining
    /// startup batch, and authenticated lifecycle repair in one non-clone
    /// wrapper. Every failure owns all inputs and exposes diagnostics only.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    fn authenticate_recovered_validate<'registry>(
        mut self,
        validate: RecoveredWalValidateRegistryCut<'registry>,
    ) -> Result<
        AuthenticatedRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleStartupError<'registry>,
    > {
        let verified = VerifiedHeightContext {
            context: self.adapter.wire_context.clone(),
            proofs_of_possession: self.adapter.proofs_of_possession.clone(),
            parent_verification: self.adapter.parent_verification.clone(),
        };
        if !matches!(self.authority, RecoveredWalStartupAuthorityV1::PhaseVote(_)) {
            return Err(RecoveredWalLifecycleStartupError {
                failure: Box::new(RecoveredWalLifecycleStartupFailure::MissingVote {
                    startup: self,
                    validate,
                }),
            });
        }
        let RecoveredWalStartupAuthorityV1::PhaseVote(recovered_vote) =
            core::mem::replace(&mut self.authority, RecoveredWalStartupAuthorityV1::None)
        else {
            unreachable!("phase-vote shape checked before replacement")
        };
        match validate.join_recovered_vote(&verified, recovered_vote) {
            Ok(repair) => Ok(AuthenticatedRecoveredWalLifecycleStartup {
                adapter: self.adapter,
                effects: self.effects,
                repair,
            }),
            Err(error) => Err(RecoveredWalLifecycleStartupError {
                failure: Box::new(RecoveredWalLifecycleStartupFailure::RegistryJoin {
                    adapter: self.adapter,
                    effects: self.effects,
                    error,
                }),
            }),
        }
    }
}
impl<'registry> StorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    /// Fsync the repaired frame against the store retained since reconstruction.
    #[allow(clippy::result_large_err)]
    fn persist_repair(
        self,
    ) -> Result<
        Box<PersistedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        Box<StorageRecoveredWalPersistError<'registry>>,
    > {
        let Self {
            adapter,
            effects,
            ledger,
            repair,
        } = self;
        let verified = VerifiedHeightContext {
            context: adapter.wire_context.clone(),
            proofs_of_possession: adapter.proofs_of_possession.clone(),
            parent_verification: adapter.parent_verification.clone(),
        };
        match ledger.persist_recovered_wal_repair(&verified, repair) {
            Ok(persisted) => Ok(Box::new(
                PersistedStorageAuthenticatedRecoveredWalLifecycleStartup {
                    adapter,
                    effects,
                    persisted,
                },
            )),
            Err(error) => Err(Box::new(StorageRecoveredWalPersistError {
                _adapter: adapter,
                _effects: effects,
                error,
            })),
        }
    }
}
impl<'registry> ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    /// Install the repaired Sign while retaining the exact post-fsync store.
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn install_recovered_sign(
        self,
    ) -> Result<
        InstalledStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>,
        StorageRecoveredWalSignInstallError<'registry>,
    > {
        let Self {
            adapter_startup,
            verified,
            persisted,
        } = self;
        match persisted.install_recovered_wal_sign() {
            Ok(installed) => Ok(InstalledStorageAuthenticatedRecoveredWalLifecycleStartup {
                adapter_startup,
                verified,
                installed,
            }),
            Err(error) => Err(StorageRecoveredWalSignInstallError {
                _startup: adapter_startup,
                error,
            }),
        }
    }
}
impl<'registry> InstalledStorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    /// Complete final-frame recovery and release only no-lifetime owner seals.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    fn open_production_owner_seals(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        body_store: &mut super::v2_body_store::V2BodyStore,
        payload_store: &mut super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        serve_payloads: super::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<ProductionRecoveredLifecycleOwnerStartupV1, StorageRecoveredWalOpenError<'registry>>
    {
        let Self {
            adapter_startup,
            verified,
            installed,
        } = self;
        let opened = match installed.open_production_lifecycle(
            adapter_startup,
            &verified,
            config,
            reply_route_source_capacity,
            body_store,
            payload_store,
            serve_payloads,
        ) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(StorageRecoveredWalOpenError {
                    failure: StorageRecoveredWalOpenFailure::Storage { error },
                });
            }
        };
        let (adapter_startup, opened) = match opened.into_production_owner_open() {
            Ok(parts) => parts,
            Err(opened) => {
                return Err(StorageRecoveredWalOpenError {
                    failure: StorageRecoveredWalOpenFailure::OwnerSeal { _opened: opened },
                });
            }
        };
        Ok(ProductionRecoveredLifecycleOwnerStartupV1 {
            adapter_startup,
            opened,
        })
    }
}
