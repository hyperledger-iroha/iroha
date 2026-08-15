impl AuthenticatedRecoveredAdapterStartup {
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
        body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let local_proposal_attempt =
            RecoveredLifecycleLocalProposalAttemptV1::from_control(&control);
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
    #[allow(
        clippy::result_large_err,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    #[inline(never)]
    fn open_recovered_decision_apply_branch(
        verified: VerifiedHeightContext,
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        mut body_store: super::v2_body_store::RevalidatedV2BodyStore,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        local_signer: &KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
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
            .prepare_replay_lineage(&verified, &fetch)
            .ok_or_else(|| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                        "Decision body replay lineage is inconsistent",
                    ),
                )
            })?;
        let preview = body
            .into_adapter_preview(adapter, &verified, fetch, lineage)
            .map_err(|error| {
                let _ = error.reason();
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                        "Decision body reducer fast-forward is inconsistent",
                    ),
                )
            })?;
        let storage = preview.into_storage_preview(&verified).map_err(|_error| {
            ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                    "Decision body storage lineage is inconsistent",
                ),
            )
        })?;
        if !storage.validates(&verified) {
            return Err(ProductionLifecycleOwnerStartupErrorV1::new(
                ProductionLifecycleOwnerStartupErrorKindV1::RecoveredDecisionApply(
                    "Decision body storage projection lost its same-store binding",
                ),
            ));
        }
        let restored = storage.restore_body();
        debug_assert!(restored.staged().validates(&verified));
        let staged = restored.into_staged();
        let body_store = body_store
            .into_lifecycle_owner_store(verified.context())
            .map_err(|error| {
                ProductionLifecycleOwnerStartupErrorV1::new(
                    ProductionLifecycleOwnerStartupErrorKindV1::BodyStore(error),
                )
            })?;
        let (payload_store, recovered_payloads) =
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                serve_payload_root,
                verified.context(),
            )
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
        let (payload_store, recovered_payloads) =
            super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                serve_payload_root,
                verified.context(),
            )
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
            ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
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
        Ok(Box::new(persisted))
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn prepare_recovered_phase_vote_cold_adapter_stage<'registry>(
        persisted: Box<PersistedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>>,
        body_store: &super::v2_body_store::V2BodyStore,
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
        let adapter_startup = ProductionLifecycleAdapterStartupV1::recovered(adapter, effects);
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
        let prepared =
            Self::prepare_recovered_phase_vote_cold_adapter_stage(persisted, &body_store)?;
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
}
