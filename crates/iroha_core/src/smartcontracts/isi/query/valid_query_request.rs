impl ValidQueryRequest {
    /// Validate a query for an API client by calling the executor.
    ///
    /// # Errors
    ///
    /// Returns an error if the query validation fails or request limits are exceeded.
    pub(crate) fn validate_for_client_parts(
        request: QueryRequest,
        authority: &AccountId,
        state_ro: &impl StateReadOnly,
        limits: QueryLimits,
    ) -> Result<Self, ValidationFail> {
        let latest_block = state_ro.latest_block().map(|block| block.header());
        Self::validate_for_client_world_parts(
            request,
            authority,
            state_ro.world(),
            latest_block,
            limits,
        )
    }

    /// Validate a query for an API client using world-state and latest committed block header.
    ///
    /// # Errors
    ///
    /// Returns an error if the query validation fails or request limits are exceeded.
    pub(crate) fn validate_for_client_world_parts(
        request: QueryRequest,
        authority: &AccountId,
        world_ro: &impl WorldReadOnly,
        latest_block: Option<BlockHeader>,
        limits: QueryLimits,
    ) -> Result<Self, ValidationFail> {
        ensure_query_registry_initialized();
        validate_query_request_limits(&request, limits)?;
        world_ro.executor().validate_query_with_world_parts(
            world_ro,
            latest_block,
            authority,
            &request,
        )?;
        Ok(Self { request, limits })
    }

    /// Validate a query for an IVM program.
    ///
    /// NOTE: The previous API used `ivm::state` types directly which are no longer exposed.
    /// This shim keeps the public surface while decoupling from IVM internals.
    /// Provide a state object that can validate a query via this trait.
    ///
    /// # Errors
    /// Returns a validation error if the request is rejected by the IVM validator.
    pub(crate) fn validate_for_ivm(
        query: QueryRequest,
        state: &mut impl IvmQueryValidator,
        limits: QueryLimits,
    ) -> Result<Self, ValidationFail> {
        ensure_query_registry_initialized();
        if matches!(&query, QueryRequest::Continue(_)) {
            return Err(ValidationFail::NotPermitted(
                "QueryRequest::Continue is not supported in IVM".to_string(),
            ));
        }
        validate_query_request_limits(&query, limits)?;
        let authority = state.authority().clone();
        state.validate_query(&authority, &query)?;
        Ok(Self {
            request: query,
            limits,
        })
    }

    /// Execute a validated query request.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    pub(crate) fn execute(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
    ) -> Result<QueryResponse, Error> {
        self.execute_stored_and_bind_revalidation(live_query_store, state, authority, None, None)
    }

    /// Execute a validated query request with an optional state handle for
    /// bounded stored cursors that can replay one continuation page at a time.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    #[cfg(test)]
    pub(crate) fn execute_with_replay_state(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Weak<State>,
    ) -> Result<QueryResponse, Error> {
        self.execute_stored_and_bind_revalidation(
            live_query_store,
            state,
            authority,
            Some(replay_state),
            None,
        )
    }

    /// Execute a validated stored query with an owning replay state and the
    /// client-provided budget for the initial `Start` request.
    ///
    /// # Errors
    ///
    /// Returns an error if query execution or budgeted projection fails.
    pub(crate) fn execute_with_replay_state_and_start_budget(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Weak<State>,
        stored_start_budget: Option<u64>,
    ) -> Result<QueryResponse, Error> {
        self.execute_stored_and_bind_revalidation(
            live_query_store,
            state,
            authority,
            Some(replay_state),
            stored_start_budget,
        )
    }

    fn execute_stored_and_bind_revalidation(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Option<Weak<State>>,
        stored_start_budget: Option<u64>,
    ) -> Result<QueryResponse, Error> {
        if let Some(ordinary_limits) = self.limits.ordinary_execution_limits {
            ordinary_memory::ensure_request_admitted(
                &self.request,
                ordinary_memory::OrdinaryCursorMode::Stored,
                self.limits,
                ordinary_limits,
            )?;
        }
        let revalidation_limit = self
            .limits
            .ordinary_execution_limits
            .map(OrdinaryQueryExecutionLimits::max_revalidation_archive_bytes);
        let revalidation_archive = matches!(&self.request, QueryRequest::Start(_))
            .then(|| encode_stored_query_revalidation_request(&self.request, revalidation_limit))
            .transpose()?;
        let response = self.execute_stored_inner(
            live_query_store,
            state,
            authority,
            replay_state,
            stored_start_budget,
        )?;

        if let (
            Some(archive),
            QueryResponse::Iterable(QueryOutput {
                continue_cursor: Some(cursor),
                ..
            }),
        ) = (revalidation_archive, &response)
            && let Err(error) =
                live_query_store.bind_revalidation_request(cursor, authority, archive)
        {
            live_query_store.drop_query(&cursor.query);
            return Err(error);
        }

        Ok(response)
    }

    #[allow(clippy::too_many_lines)] // not much we can do, we _need_ to list all the box types here
    fn execute_stored_inner(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Option<Weak<State>>,
        stored_start_budget: Option<u64>,
    ) -> Result<QueryResponse, Error> {
        let Self { request, limits } = self;
        match request {
            QueryRequest::Singular(singular_query) => {
                let output = singular_query.execute(state)?;
                Ok(QueryResponse::Singular(output))
            }
            QueryRequest::Start(iter_query) => {
                use iroha_data_model::query;

                fn try_decode_query<Q>(
                    erased: &query::ErasedIterQuery<
                        impl HasProjection<PredicateMarker>
                        + HasProjection<SelectorMarker, AtomType = ()>
                        + Send
                        + Sync,
                    >,
                ) -> Option<Q>
                where
                    Q: norito::codec::Decode + norito::codec::Encode,
                {
                    decode_iter_query_payload_exact(erased.payload())
                }

                #[allow(clippy::too_many_arguments)]
                fn run_dispatch<T, Q, F>(
                    qbox: &query::QueryBox<query::QueryOutputBatchBox>,
                    params: &query::parameters::QueryParams,
                    limits: QueryLimits,
                    state: &impl StateReadOnly,
                    live_query_store: &LiveQueryStoreHandle,
                    authority: &AccountId,
                    gas_budget: Option<u64>,
                    replay_state: Option<Weak<State>>,
                    _decode: F,
                ) -> Result<Option<QueryResponse>, Error>
                where
                    T: Send + Sync + 'static,
                    Q: super::super::ValidQuery<Item = T>
                        + NoritoSerialize
                        + for<'de> norito::core::NoritoDeserialize<'de>
                        + Send
                        + Sync
                        + 'static,
                    T: HasProjection<SelectorMarker, AtomType = ()>
                        + HasProjection<PredicateMarker>
                        + crate::smartcontracts::isi::query::SortableQueryOutput
                        + NoritoSerialize
                        + for<'de> norito::core::NoritoDeserialize<'de>
                        + norito::json::JsonSerialize
                        + Send
                        + Sync
                        + 'static,
                    for<'de> <T as crate::smartcontracts::isi::query::SortableQueryOutput>::TiebreakKey:
                        norito::core::NoritoDeserialize<'de>,
                    <T as HasProjection<SelectorMarker>>::Projection:
                        EvaluateSelector<T> + Send + Sync,
                    query::QueryOutputBatchBox: From<Vec<T>>,
                    F: Fn(&query::ErasedIterQuery<T>) -> Option<Q>,
                {
                    if let Some(erased) = query::iter_query_inner::<T>(qbox) {
                        let mut decoder =
                            FastIterComponentDecoder::new(limits, [erased.payload(), &[], &[]])?;
                        let Some(concrete) = decoder.try_decode::<Q>(erased.payload())? else {
                            return Ok(None);
                        };
                        // Execute the concrete ValidQuery with provided predicate
                        let predicate = erased.predicate_cloned();
                        let iter = execute_iterable_source(
                            concrete,
                            predicate,
                            params,
                            limits,
                            ordinary_memory::OrdinaryCursorMode::Stored,
                            state,
                        )?;

                        // Postprocess and register a live iterator (or prepared fast-start).
                        let output = handle_iter_start_stored_replayable(
                            iter,
                            erased.selector_cloned(),
                            params,
                            limits,
                            live_query_store,
                            authority,
                            gas_budget,
                            replay_state,
                        )?;
                        return Ok(Some(QueryResponse::Iterable(output)));
                    }
                    Ok(None)
                }

                let params = &iter_query.params;
                #[cfg_attr(not(feature = "fast_dsl"), allow(unused_variables))]
                let stored_cursor_budget = {
                    let min = state.pipeline().query_stored_min_gas_units;
                    stored_start_budget.or_else(|| (min > 0).then_some(min))
                };
                // Fast-DSL path: when the boxed query payload is not present, reconstruct
                // from item kind and encoded predicate/selector.
                if legacy_query_box(&iter_query).is_none() {
                    {
                        use iroha_data_model::query::QueryItemKind;
                        let mut decoder = FastIterComponentDecoder::new(
                            limits,
                            [
                                &iter_query.query_payload,
                                &iter_query.predicate_bytes,
                                &iter_query.selector_bytes,
                            ],
                        )?;
                        // Helper to run an iterable query using the encoded predicate/selector.
                        macro_rules! run_payload_or_default {
                            // Unit queries have an empty canonical payload. Reject any other bytes so
                            // parameterized or malformed payloads cannot become global queries.
                            ($itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    decoder.decode(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    decoder.decode(&iter_query.selector_bytes)?;
                                let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Stored,
                                    state,
                                )?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                            // For parameterized queries that require payload: fail if missing
                            (require_payload $itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    decoder.decode(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    decoder.decode(&iter_query.selector_bytes)?;
                                let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Stored,
                                    state,
                                )?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                        }
                        macro_rules! run_fast {
                            ($itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    decoder.decode(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    decoder.decode(&iter_query.selector_bytes)?;
                                let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Stored,
                                    state,
                                )?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                        }
                        match iter_query.item {
                            QueryItemKind::Domain => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::domain::Domain,
                                        iroha_data_model::query::domain::prelude::FindDomainsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::domain::Domain,
                                    iroha_data_model::query::domain::prelude::FindDomains
                                )
                            }
                            QueryItemKind::Account => {
                                // Prefer parameterized query when payload is present; otherwise default.
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::account::Account, iroha_data_model::query::account::prelude::FindAccountsWithAsset)
                                }
                                run_fast!(
                                    iroha_data_model::account::Account,
                                    iroha_data_model::query::account::prelude::FindAccounts
                                )
                            }
                            QueryItemKind::AccountId => run_payload_or_default!(
                                iroha_data_model::account::AccountId,
                                iroha_data_model::query::account::prelude::FindAccountIds
                            ),
                            QueryItemKind::Asset => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::asset::value::Asset,
                                        iroha_data_model::query::asset::prelude::FindAssetsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::asset::value::Asset,
                                    iroha_data_model::query::asset::prelude::FindAssets
                                )
                            }
                            QueryItemKind::AssetDefinition => run_payload_or_default!(
                                iroha_data_model::asset::definition::AssetDefinition,
                                iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                            ),
                            QueryItemKind::RepoAgreement => run_payload_or_default!(
                                iroha_data_model::repo::RepoAgreement,
                                iroha_data_model::query::repo::prelude::FindRepoAgreements
                            ),
                            QueryItemKind::Nft => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::nft::Nft,
                                        iroha_data_model::query::nft::prelude::FindNftsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nft::Nft,
                                    iroha_data_model::query::nft::prelude::FindNfts
                                )
                            }
                            QueryItemKind::Rwa => run_payload_or_default!(
                                iroha_data_model::rwa::Rwa,
                                iroha_data_model::query::rwa::prelude::FindRwas
                            ),
                            QueryItemKind::Role => run_payload_or_default!(
                                iroha_data_model::role::Role,
                                iroha_data_model::query::role::prelude::FindRoles
                            ),
                            QueryItemKind::RoleId => {
                                // If payload present, it's a parameterized FindRolesByAccountId; otherwise use FindRoleIds.
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::role::RoleId, iroha_data_model::query::role::prelude::FindRolesByAccountId)
                                }
                                run_fast!(
                                    iroha_data_model::role::RoleId,
                                    iroha_data_model::query::role::prelude::FindRoleIds
                                )
                            }
                            QueryItemKind::PeerId => run_payload_or_default!(
                                iroha_data_model::peer::PeerId,
                                iroha_data_model::query::peer::prelude::FindPeers
                            ),
                            QueryItemKind::TriggerId => run_payload_or_default!(
                                iroha_data_model::trigger::TriggerId,
                                iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                            ),
                            QueryItemKind::Trigger => run_payload_or_default!(
                                iroha_data_model::trigger::Trigger,
                                iroha_data_model::query::trigger::prelude::FindTriggers
                            ),
                            QueryItemKind::CommittedTransaction => {
                                let _concrete = decoder.decode::<
                                    iroha_data_model::query::transaction::prelude::FindTransactions,
                                >(&iter_query.query_payload)
                                ?;
                                let pred = decoder
                                    .decode::<CompoundPredicate<CommittedTransaction>>(
                                        &iter_query.predicate_bytes,
                                    )?;
                                let sel = decoder.decode::<SelectorTuple<CommittedTransaction>>(
                                    &iter_query.selector_bytes,
                                )?;
                                let output = handle_find_transactions_stored(
                                    state,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }
                            QueryItemKind::SignedBlock => run_payload_or_default!(
                                iroha_data_model::block::SignedBlock,
                                iroha_data_model::query::block::prelude::FindBlocks
                            ),
                            QueryItemKind::BlockHeader => run_payload_or_default!(
                                iroha_data_model::block::BlockHeader,
                                iroha_data_model::query::block::prelude::FindBlockHeaders
                            ),
                            QueryItemKind::ProofRecord => {
                                if limits.canonical_output_limits.is_some() {
                                    return Err(Error::Conversion(
                                        "canonical fanout rejects proof queries before source execution because their implementations are not protocol-bounded"
                                            .to_owned(),
                                    ));
                                }
                                let pred = decoder
                                    .decode::<iroha_data_model::query::dsl::CompoundPredicate<
                                    iroha_data_model::proof::ProofRecord,
                                >>(
                                    &iter_query.predicate_bytes
                                )?;
                                let sel = decoder
                                    .decode::<iroha_data_model::query::dsl::SelectorTuple<
                                        iroha_data_model::proof::ProofRecord,
                                    >>(
                                        &iter_query.selector_bytes
                                    )?;
                                macro_rules! try_proof_query {
                                    ($find:ty) => {{
                                        if let Some(concrete) = decoder
                                            .try_decode::<$find>(&iter_query.query_payload)?
                                        {
                                            let iter = execute_iterable_source(
                                                concrete,
                                                pred,
                                                params,
                                                limits,
                                                ordinary_memory::OrdinaryCursorMode::Stored,
                                                state,
                                            )?;
                                            let output = handle_iter_start_stored_replayable(
                                                iter,
                                                sel,
                                                params,
                                                limits,
                                                live_query_store,
                                                authority,
                                                stored_cursor_budget,
                                                replay_state.clone(),
                                            )?;
                                            return Ok(QueryResponse::Iterable(output));
                                        }
                                    }};
                                }
                                if !iter_query.query_payload.is_empty() {
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByBackend
                                    );
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByStatus
                                    );
                                    return Err(Error::Conversion(
                                        "failed to decode proof query payload".into(),
                                    ));
                                }
                                let concrete = decoder.decode::<
                                    iroha_data_model::query::proof::prelude::FindProofRecords,
                                >(&iter_query.query_payload)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Stored,
                                    state,
                                )?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }
                            QueryItemKind::AssetEscrowRecord => run_payload_or_default!(
                                iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrows
                            ),
                            QueryItemKind::AssetEscrowsBySeller => run_payload_or_default!(
                                require_payload iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrowsBySeller
                            ),
                            QueryItemKind::AssetEscrowsByBuyer => run_payload_or_default!(
                                require_payload iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrowsByBuyer
                            ),
                            QueryItemKind::AssetEscrowsByStatus => run_payload_or_default!(
                                require_payload iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrowsByStatus
                            ),
                            QueryItemKind::OracleFeedConfig => run_payload_or_default!(
                                iroha_data_model::oracle::FeedConfig,
                                iroha_data_model::query::oracle::prelude::FindOracleFeeds
                            ),
                            QueryItemKind::OracleFeedEventRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::events::data::oracle::FeedEventRecord, iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId)
                            }
                            QueryItemKind::OracleProviderStatsRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::OracleProviderStatsRecord, iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId)
                            }
                            QueryItemKind::OracleDispute => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::oracle::OracleDispute,
                                        iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::oracle::OracleDispute,
                                    iroha_data_model::query::oracle::prelude::FindOracleDisputes
                                )
                            }
                            QueryItemKind::OracleChangeProposal => run_payload_or_default!(
                                iroha_data_model::oracle::OracleChangeProposal,
                                iroha_data_model::query::oracle::prelude::FindOracleChanges
                            ),
                            QueryItemKind::TwitterBindingRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::TwitterBindingRecord, iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid)
                            }
                            QueryItemKind::DefiOracleAttestation => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::DefiOracleAttestation, iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey)
                            }
                            QueryItemKind::Permission => {
                                run_payload_or_default!(require_payload iroha_data_model::permission::Permission, iroha_data_model::query::permission::prelude::FindPermissionsByAccountId)
                            }
                            QueryItemKind::FeeSponsorProgram => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::nexus::FeeSponsorProgram, iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramsBySponsor)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nexus::FeeSponsorProgram,
                                    iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                                )
                            }
                            QueryItemKind::FeeSponsorProgramId => run_payload_or_default!(
                                iroha_data_model::nexus::FeeSponsorProgramId,
                                iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                            ),
                        }
                    }
                    #[cfg(any())]
                    {
                        // unreachable: iroha_core is built with std; fast_dsl iterable path requires std in data_model.
                        return Err(Error::Conversion(
                            "fast_dsl iterable path requires std".into(),
                        ));
                    }
                }
                // Fallback for fast_dsl-enabled callers: if the boxed query is absent,
                // reconstruct a default iterable query from the item kind.
                if legacy_query_box(&iter_query).is_none() {
                    use iroha_data_model::query::QueryItemKind;
                    let mut decoder = FastIterComponentDecoder::new(
                        limits,
                        [
                            &iter_query.query_payload,
                            &iter_query.predicate_bytes,
                            &iter_query.selector_bytes,
                        ],
                    )?;
                    macro_rules! run_unit {
                        ($itemty:ty, $find:ty) => {{
                            let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                decoder.decode(&iter_query.predicate_bytes)?;
                            let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                decoder.decode(&iter_query.selector_bytes)?;
                            let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                            let iter = execute_iterable_source(
                                concrete,
                                pred,
                                params,
                                limits,
                                ordinary_memory::OrdinaryCursorMode::Stored,
                                state,
                            )?;
                            let output = handle_iter_start_stored_replayable(
                                iter,
                                sel,
                                params,
                                limits,
                                live_query_store,
                                authority,
                                stored_cursor_budget,
                                replay_state.clone(),
                            )?;
                            return Ok(QueryResponse::Iterable(output));
                        }};
                    }
                    match iter_query.item {
                        QueryItemKind::Domain => run_unit!(
                            iroha_data_model::domain::Domain,
                            iroha_data_model::query::domain::prelude::FindDomains
                        ),
                        QueryItemKind::Account => run_unit!(
                            iroha_data_model::account::Account,
                            iroha_data_model::query::account::prelude::FindAccounts
                        ),
                        QueryItemKind::AccountId => run_unit!(
                            iroha_data_model::account::AccountId,
                            iroha_data_model::query::account::prelude::FindAccountIds
                        ),
                        QueryItemKind::Asset => run_unit!(
                            iroha_data_model::asset::value::Asset,
                            iroha_data_model::query::asset::prelude::FindAssets
                        ),
                        QueryItemKind::AssetDefinition => run_unit!(
                            iroha_data_model::asset::definition::AssetDefinition,
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                        ),
                        QueryItemKind::RepoAgreement => run_unit!(
                            iroha_data_model::repo::RepoAgreement,
                            iroha_data_model::query::repo::prelude::FindRepoAgreements
                        ),
                        QueryItemKind::Nft => run_unit!(
                            iroha_data_model::nft::Nft,
                            iroha_data_model::query::nft::prelude::FindNfts
                        ),
                        QueryItemKind::Rwa => run_unit!(
                            iroha_data_model::rwa::Rwa,
                            iroha_data_model::query::rwa::prelude::FindRwas
                        ),
                        QueryItemKind::Role => run_unit!(
                            iroha_data_model::role::Role,
                            iroha_data_model::query::role::prelude::FindRoles
                        ),
                        QueryItemKind::RoleId => run_unit!(
                            iroha_data_model::role::RoleId,
                            iroha_data_model::query::role::prelude::FindRoleIds
                        ),
                        QueryItemKind::PeerId => run_unit!(
                            iroha_data_model::peer::PeerId,
                            iroha_data_model::query::peer::prelude::FindPeers
                        ),
                        QueryItemKind::TriggerId => run_unit!(
                            iroha_data_model::trigger::TriggerId,
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                        ),
                        QueryItemKind::Trigger => run_unit!(
                            iroha_data_model::trigger::Trigger,
                            iroha_data_model::query::trigger::prelude::FindTriggers
                        ),
                        QueryItemKind::CommittedTransaction => run_unit!(
                            iroha_data_model::query::CommittedTransaction,
                            iroha_data_model::query::transaction::prelude::FindTransactions
                        ),
                        QueryItemKind::SignedBlock => run_unit!(
                            iroha_data_model::block::SignedBlock,
                            iroha_data_model::query::block::prelude::FindBlocks
                        ),
                        QueryItemKind::BlockHeader => run_unit!(
                            iroha_data_model::block::BlockHeader,
                            iroha_data_model::query::block::prelude::FindBlockHeaders
                        ),
                        QueryItemKind::ProofRecord => run_unit!(
                            iroha_data_model::proof::ProofRecord,
                            iroha_data_model::query::proof::prelude::FindProofRecords
                        ),
                        QueryItemKind::AssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAssetEscrows
                        ),
                        QueryItemKind::AssetEscrowsBySeller
                        | QueryItemKind::AssetEscrowsByBuyer
                        | QueryItemKind::AssetEscrowsByStatus => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleFeedConfig => run_unit!(
                            iroha_data_model::oracle::FeedConfig,
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds
                        ),
                        QueryItemKind::OracleFeedEventRecord
                        | QueryItemKind::OracleProviderStatsRecord
                        | QueryItemKind::TwitterBindingRecord
                        | QueryItemKind::DefiOracleAttestation => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleDispute => run_unit!(
                            iroha_data_model::oracle::OracleDispute,
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes
                        ),
                        QueryItemKind::OracleChangeProposal => run_unit!(
                            iroha_data_model::oracle::OracleChangeProposal,
                            iroha_data_model::query::oracle::prelude::FindOracleChanges
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::FeeSponsorProgram => run_unit!(
                            iroha_data_model::nexus::FeeSponsorProgram,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                        ),
                        QueryItemKind::FeeSponsorProgramId => run_unit!(
                            iroha_data_model::nexus::FeeSponsorProgramId,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                        ),
                    }
                }
                if legacy_query_box(&iter_query).is_none() {
                    use iroha_data_model::query::QueryItemKind;
                    let mut decoder = FastIterComponentDecoder::new(
                        limits,
                        [
                            &iter_query.query_payload,
                            &iter_query.predicate_bytes,
                            &iter_query.selector_bytes,
                        ],
                    )?;
                    macro_rules! run_unit {
                        ($itemty:ty, $find:ty) => {{
                            let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                decoder.decode(&iter_query.predicate_bytes)?;
                            let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                decoder.decode(&iter_query.selector_bytes)?;
                            let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                            let iter = execute_iterable_source(
                                concrete,
                                pred,
                                params,
                                limits,
                                ordinary_memory::OrdinaryCursorMode::Stored,
                                state,
                            )?;
                            let (output, _processed_items) =
                                apply_query_postprocessing_ephemeral_with_budget(
                                    iter, sel, params, limits, None,
                                )?;
                            return Ok(QueryResponse::Iterable(output));
                        }};
                    }
                    match iter_query.item {
                        QueryItemKind::Domain => run_unit!(
                            iroha_data_model::domain::Domain,
                            iroha_data_model::query::domain::prelude::FindDomains
                        ),
                        QueryItemKind::Account => run_unit!(
                            iroha_data_model::account::Account,
                            iroha_data_model::query::account::prelude::FindAccounts
                        ),
                        QueryItemKind::AccountId => run_unit!(
                            iroha_data_model::account::AccountId,
                            iroha_data_model::query::account::prelude::FindAccountIds
                        ),
                        QueryItemKind::Asset => run_unit!(
                            iroha_data_model::asset::value::Asset,
                            iroha_data_model::query::asset::prelude::FindAssets
                        ),
                        QueryItemKind::AssetDefinition => run_unit!(
                            iroha_data_model::asset::definition::AssetDefinition,
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                        ),
                        QueryItemKind::RepoAgreement => run_unit!(
                            iroha_data_model::repo::RepoAgreement,
                            iroha_data_model::query::repo::prelude::FindRepoAgreements
                        ),
                        QueryItemKind::Nft => run_unit!(
                            iroha_data_model::nft::Nft,
                            iroha_data_model::query::nft::prelude::FindNfts
                        ),
                        QueryItemKind::Rwa => run_unit!(
                            iroha_data_model::rwa::Rwa,
                            iroha_data_model::query::rwa::prelude::FindRwas
                        ),
                        QueryItemKind::Role => run_unit!(
                            iroha_data_model::role::Role,
                            iroha_data_model::query::role::prelude::FindRoles
                        ),
                        QueryItemKind::RoleId => run_unit!(
                            iroha_data_model::role::RoleId,
                            iroha_data_model::query::role::prelude::FindRoleIds
                        ),
                        QueryItemKind::PeerId => run_unit!(
                            iroha_data_model::peer::PeerId,
                            iroha_data_model::query::peer::prelude::FindPeers
                        ),
                        QueryItemKind::TriggerId => run_unit!(
                            iroha_data_model::trigger::TriggerId,
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                        ),
                        QueryItemKind::Trigger => run_unit!(
                            iroha_data_model::trigger::Trigger,
                            iroha_data_model::query::trigger::prelude::FindTriggers
                        ),
                        QueryItemKind::CommittedTransaction => run_unit!(
                            iroha_data_model::query::CommittedTransaction,
                            iroha_data_model::query::transaction::prelude::FindTransactions
                        ),
                        QueryItemKind::SignedBlock => run_unit!(
                            iroha_data_model::block::SignedBlock,
                            iroha_data_model::query::block::prelude::FindBlocks
                        ),
                        QueryItemKind::BlockHeader => run_unit!(
                            iroha_data_model::block::BlockHeader,
                            iroha_data_model::query::block::prelude::FindBlockHeaders
                        ),
                        QueryItemKind::ProofRecord => run_unit!(
                            iroha_data_model::proof::ProofRecord,
                            iroha_data_model::query::proof::prelude::FindProofRecords
                        ),
                        QueryItemKind::AssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAssetEscrows
                        ),
                        QueryItemKind::AssetEscrowsBySeller
                        | QueryItemKind::AssetEscrowsByBuyer
                        | QueryItemKind::AssetEscrowsByStatus => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleFeedConfig => run_unit!(
                            iroha_data_model::oracle::FeedConfig,
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds
                        ),
                        QueryItemKind::OracleFeedEventRecord
                        | QueryItemKind::OracleProviderStatsRecord
                        | QueryItemKind::TwitterBindingRecord
                        | QueryItemKind::DefiOracleAttestation => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleDispute => run_unit!(
                            iroha_data_model::oracle::OracleDispute,
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes
                        ),
                        QueryItemKind::OracleChangeProposal => run_unit!(
                            iroha_data_model::oracle::OracleChangeProposal,
                            iroha_data_model::query::oracle::prelude::FindOracleChanges
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::FeeSponsorProgram => run_unit!(
                            iroha_data_model::nexus::FeeSponsorProgram,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                        ),
                        QueryItemKind::FeeSponsorProgramId => run_unit!(
                            iroha_data_model::nexus::FeeSponsorProgramId,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                        ),
                    }
                }
                let Some(qbox) = legacy_query_box(&iter_query) else {
                    // Final fallback: default unit iterable by item kind
                    use iroha_data_model::query::QueryItemKind;
                    let mut decoder = FastIterComponentDecoder::new(
                        limits,
                        [
                            &iter_query.query_payload,
                            &iter_query.predicate_bytes,
                            &iter_query.selector_bytes,
                        ],
                    )?;
                    macro_rules! run_unit {
                        ($itemty:ty, $find:ty) => {{
                            let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                decoder.decode(&iter_query.predicate_bytes)?;
                            let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                decoder.decode(&iter_query.selector_bytes)?;
                            let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                            let iter = execute_iterable_source(
                                concrete,
                                pred,
                                params,
                                limits,
                                ordinary_memory::OrdinaryCursorMode::Stored,
                                state,
                            )?;
                            let output = handle_iter_start_stored_replayable(
                                iter,
                                sel,
                                params,
                                limits,
                                live_query_store,
                                authority,
                                stored_cursor_budget,
                                replay_state.clone(),
                            )?;
                            return Ok(QueryResponse::Iterable(output));
                        }};
                    }
                    match iter_query.item {
                        QueryItemKind::Domain => run_unit!(
                            iroha_data_model::domain::Domain,
                            iroha_data_model::query::domain::prelude::FindDomains
                        ),
                        QueryItemKind::Account => run_unit!(
                            iroha_data_model::account::Account,
                            iroha_data_model::query::account::prelude::FindAccounts
                        ),
                        QueryItemKind::AccountId => run_unit!(
                            iroha_data_model::account::AccountId,
                            iroha_data_model::query::account::prelude::FindAccountIds
                        ),
                        QueryItemKind::Asset => run_unit!(
                            iroha_data_model::asset::value::Asset,
                            iroha_data_model::query::asset::prelude::FindAssets
                        ),
                        QueryItemKind::AssetDefinition => run_unit!(
                            iroha_data_model::asset::definition::AssetDefinition,
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                        ),
                        QueryItemKind::RepoAgreement => run_unit!(
                            iroha_data_model::repo::RepoAgreement,
                            iroha_data_model::query::repo::prelude::FindRepoAgreements
                        ),
                        QueryItemKind::Nft => run_unit!(
                            iroha_data_model::nft::Nft,
                            iroha_data_model::query::nft::prelude::FindNfts
                        ),
                        QueryItemKind::Rwa => run_unit!(
                            iroha_data_model::rwa::Rwa,
                            iroha_data_model::query::rwa::prelude::FindRwas
                        ),
                        QueryItemKind::Role => run_unit!(
                            iroha_data_model::role::Role,
                            iroha_data_model::query::role::prelude::FindRoles
                        ),
                        QueryItemKind::RoleId => run_unit!(
                            iroha_data_model::role::RoleId,
                            iroha_data_model::query::role::prelude::FindRoleIds
                        ),
                        QueryItemKind::PeerId => run_unit!(
                            iroha_data_model::peer::PeerId,
                            iroha_data_model::query::peer::prelude::FindPeers
                        ),
                        QueryItemKind::TriggerId => run_unit!(
                            iroha_data_model::trigger::TriggerId,
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                        ),
                        QueryItemKind::Trigger => run_unit!(
                            iroha_data_model::trigger::Trigger,
                            iroha_data_model::query::trigger::prelude::FindTriggers
                        ),
                        QueryItemKind::CommittedTransaction => run_unit!(
                            iroha_data_model::query::CommittedTransaction,
                            iroha_data_model::query::transaction::prelude::FindTransactions
                        ),
                        QueryItemKind::SignedBlock => run_unit!(
                            iroha_data_model::block::SignedBlock,
                            iroha_data_model::query::block::prelude::FindBlocks
                        ),
                        QueryItemKind::BlockHeader => run_unit!(
                            iroha_data_model::block::BlockHeader,
                            iroha_data_model::query::block::prelude::FindBlockHeaders
                        ),
                        QueryItemKind::ProofRecord => run_unit!(
                            iroha_data_model::proof::ProofRecord,
                            iroha_data_model::query::proof::prelude::FindProofRecords
                        ),
                        QueryItemKind::AssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAssetEscrows
                        ),
                        QueryItemKind::AssetEscrowsBySeller
                        | QueryItemKind::AssetEscrowsByBuyer
                        | QueryItemKind::AssetEscrowsByStatus => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleFeedConfig => run_unit!(
                            iroha_data_model::oracle::FeedConfig,
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds
                        ),
                        QueryItemKind::OracleFeedEventRecord
                        | QueryItemKind::OracleProviderStatsRecord
                        | QueryItemKind::TwitterBindingRecord
                        | QueryItemKind::DefiOracleAttestation => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleDispute => run_unit!(
                            iroha_data_model::oracle::OracleDispute,
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes
                        ),
                        QueryItemKind::OracleChangeProposal => run_unit!(
                            iroha_data_model::oracle::OracleChangeProposal,
                            iroha_data_model::query::oracle::prelude::FindOracleChanges
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::FeeSponsorProgram => run_unit!(
                            iroha_data_model::nexus::FeeSponsorProgram,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                        ),
                        QueryItemKind::FeeSponsorProgramId => run_unit!(
                            iroha_data_model::nexus::FeeSponsorProgramId,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                        ),
                    }
                };

                // Try dispatch for all supported iterable queries, keyed by their item type.
                // For item types that have multiple concrete query variants (e.g., Account),
                // attempt decodes in priority order.
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomains,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::domain::prelude::FindDomains>(e)
                    },
                )? {
                    return Ok(resp);
                }
                // Accounts: support both `FindAccounts` and `FindAccountsWithAsset`
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccounts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::account::prelude::FindAccounts>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssets,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| try_decode_query::<iroha_data_model::query::asset::prelude::FindAssets>(e),
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::asset::definition::AssetDefinition,
                    iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::repo::RepoAgreement,
                    iroha_data_model::query::repo::prelude::FindRepoAgreements,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::repo::prelude::FindRepoAgreements>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNftsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::nft::prelude::FindNftsByAccountId>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNfts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| try_decode_query::<iroha_data_model::query::nft::prelude::FindNfts>(e),
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::role::Role,
                    iroha_data_model::query::role::prelude::FindRoles,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| try_decode_query::<iroha_data_model::query::role::prelude::FindRoles>(e),
                )? {
                    return Ok(resp);
                }
                // RoleId: support both `FindRoleIds` and `FindRolesByAccountId`.
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRoleIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        e.payload()
                            .is_empty()
                            .then_some(iroha_data_model::query::role::prelude::FindRoleIds)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRolesByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::role::prelude::FindRolesByAccountId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecords,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::proof::prelude::FindProofRecords>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::peer::PeerId,
                    iroha_data_model::query::peer::prelude::FindPeers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| try_decode_query::<iroha_data_model::query::peer::prelude::FindPeers>(e),
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::trigger::TriggerId,
                    iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        e.payload().is_empty().then_some(
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::trigger::Trigger,
                    iroha_data_model::query::trigger::prelude::FindTriggers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::trigger::prelude::FindTriggers>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(erased) = query::iter_query_inner::<CommittedTransaction>(qbox) {
                    if limits.canonical_output_limits.is_some() {
                        return Err(Error::Conversion(
                            "canonical fanout rejects `FindTransactions` before payload decoding or source execution"
                                .into(),
                        ));
                    }
                    let mut decoder =
                        FastIterComponentDecoder::new(limits, [erased.payload(), &[], &[]])?;
                    if decoder
                        .try_decode::<
                            iroha_data_model::query::transaction::prelude::FindTransactions,
                        >(erased.payload())?
                        .is_none()
                    {
                        return Err(Error::Conversion(
                            "malformed payload for transaction iterable query".into(),
                        ));
                    }
                    let output = handle_find_transactions_stored(
                        state,
                        erased.predicate_cloned(),
                        erased.selector_cloned(),
                        params,
                        limits,
                        live_query_store,
                        authority,
                        stored_cursor_budget,
                        replay_state.clone(),
                    )?;
                    return Ok(QueryResponse::Iterable(output));
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::block::SignedBlock,
                    iroha_data_model::query::block::prelude::FindBlocks,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| try_decode_query::<iroha_data_model::query::block::prelude::FindBlocks>(e),
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::block::BlockHeader,
                    iroha_data_model::query::block::prelude::FindBlockHeaders,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::block::prelude::FindBlockHeaders>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::FeedConfig,
                    iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::oracle::prelude::FindOracleFeeds>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::events::data::oracle::FeedEventRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleProviderStatsRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleChangeProposal,
                    iroha_data_model::query::oracle::prelude::FindOracleChanges,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleChanges,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::TwitterBindingRecord,
                    iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::DefiOracleAttestation,
                    iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }

                // Boxed registry parity for the variants which are also
                // available through the fast-DSL item-kind path.
                macro_rules! try_remaining_stored_dispatch {
                    ($item:ty, $query:ty) => {
                        if let Some(response) = run_dispatch::<$item, $query, _>(
                            qbox,
                            params,
                            limits,
                            state,
                            live_query_store,
                            authority,
                            stored_cursor_budget,
                            replay_state.clone(),
                            |erased| try_decode_query::<$query>(erased),
                        )? {
                            return Ok(response);
                        }
                    };
                }
                try_remaining_stored_dispatch!(
                    iroha_data_model::account::AccountId,
                    iroha_data_model::query::account::prelude::FindAccountIds
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::rwa::Rwa,
                    iroha_data_model::query::rwa::prelude::FindRwas
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::permission::Permission,
                    iroha_data_model::query::permission::prelude::FindPermissionsByAccountId
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrowsBySeller
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrowsByBuyer
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrowsByStatus
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrows
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::nexus::FeeSponsorProgram,
                    iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramsBySponsor
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::nexus::FeeSponsorProgram,
                    iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                );
                try_remaining_stored_dispatch!(
                    iroha_data_model::nexus::FeeSponsorProgramId,
                    iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                );

                Err(Error::Conversion(
                    "unsupported iterable query type".to_string(),
                ))
            }
            QueryRequest::Continue(cursor) => Ok(QueryResponse::Iterable(
                live_query_store.handle_iter_continue(cursor, authority)?,
            )),
        }
    }

    /// Execute a validated query request using an ephemeral iterator for iterable queries.
    ///
    /// Iterable queries return only the first batch and do not allocate a
    /// reusable cursor in the [`LiveQueryStore`]. Suitable for snapshot-bound
    /// contexts where queries must not outlive the captured view.
    ///
    /// # Errors
    /// Returns an error if the query execution fails.
    pub(crate) fn execute_ephemeral(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
    ) -> Result<QueryResponse, Error> {
        self.execute_ephemeral_with_stats(live_query_store, state, authority, None)
            .map(|(response, _)| response)
    }

    pub(crate) fn execute_ephemeral_with_stats(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(QueryResponse, QueryExecutionStats), Error> {
        let (response, mut stats) =
            self.execute_ephemeral_inner_with_stats(live_query_store, state, authority, budget)?;
        stats.record_response(&response, budget)?;
        Ok((response, stats))
    }

    #[allow(clippy::too_many_lines)]
    fn execute_ephemeral_inner_with_stats(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(QueryResponse, QueryExecutionStats), Error> {
        let Self { request, limits } = self;
        if let Some(ordinary_limits) = limits.ordinary_execution_limits {
            ordinary_memory::ensure_request_admitted(
                &request,
                ordinary_memory::OrdinaryCursorMode::Ephemeral,
                limits,
                ordinary_limits,
            )?;
        }
        let budget = limits
            .ordinary_execution_limits
            .map(OrdinaryQueryExecutionLimits::execution_budget)
            .or(budget);
        let budget_items = budget;
        match request {
            QueryRequest::Singular(singular_query) => {
                let mut stats = QueryExecutionStats::default();
                let output =
                    singular_memory::execute_with_limits(limits.singular_output_limits, || {
                        // Install the singular allocation guard before source
                        // preflight. Some source adapters validate persisted
                        // records while measuring them, and those decodes must
                        // receive the same resident D ceiling as execution.
                        if limits.server_memory_budget
                            && let Some(server_budget) = budget
                        {
                            let source_bytes =
                                ordinary_memory::preflight_server_singular_source_materialization(
                                    &singular_query,
                                    state,
                                    server_budget,
                                    limits.singular_output_limits.is_some(),
                                )?;
                            stats.record_preflighted_item(source_bytes, Some(server_budget))?;
                        }
                        singular_query.execute(state)
                    })?;
                stats.record_value_bytes(&output, budget)?;
                Ok((QueryResponse::Singular(output), stats))
            }
            QueryRequest::Start(iter_query) => {
                use iroha_data_model::query;

                fn try_decode_query<Q>(
                    erased: &query::ErasedIterQuery<
                        impl HasProjection<PredicateMarker>
                        + HasProjection<SelectorMarker, AtomType = ()>
                        + Send
                        + Sync,
                    >,
                ) -> Option<Q>
                where
                    Q: norito::codec::Decode + norito::codec::Encode,
                {
                    decode_iter_query_payload_exact(erased.payload())
                }

                #[allow(clippy::too_many_arguments)]
                fn run_dispatch<T, Q, F>(
                    qbox: &query::QueryBox<query::QueryOutputBatchBox>,
                    params: &query::parameters::QueryParams,
                    limits: QueryLimits,
                    budget: Option<QueryExecutionBudget>,
                    state: &impl StateReadOnly,
                    _live_query_store: &LiveQueryStoreHandle,
                    _authority: &AccountId,
                    __stored_cursor_budget: Option<u64>,
                    _decode: F,
                ) -> Result<Option<(QueryResponse, QueryExecutionStats)>, Error>
                where
                    T: Send + Sync + 'static,
                    Q: super::super::ValidQuery<Item = T>
                        + NoritoSerialize
                        + for<'de> norito::core::NoritoDeserialize<'de>
                        + 'static,
                    T: HasProjection<SelectorMarker, AtomType = ()>
                        + HasProjection<PredicateMarker>
                        + crate::smartcontracts::isi::query::SortableQueryOutput
                        + NoritoSerialize
                        + for<'de> norito::core::NoritoDeserialize<'de>
                        + norito::json::JsonSerialize
                        + Send
                        + Sync
                        + 'static,
                    for<'de> <T as crate::smartcontracts::isi::query::SortableQueryOutput>::TiebreakKey:
                        norito::core::NoritoDeserialize<'de>,
                    <T as HasProjection<SelectorMarker>>::Projection:
                        EvaluateSelector<T> + Send + Sync,
                    query::QueryOutputBatchBox: From<Vec<T>>,
                    F: Fn(&query::ErasedIterQuery<T>) -> Option<Q>,
                {
                    if let Some(erased) = query::iter_query_inner::<T>(qbox) {
                        let mut decoder =
                            FastIterComponentDecoder::new(limits, [erased.payload(), &[], &[]])?;
                        if let Some(output_limits) = limits.canonical_output_limits {
                            canonical_topk::ensure_canonical_query_source_admitted::<T, Q>(
                                erased.predicate(),
                                erased.selector(),
                                params,
                                output_limits,
                            )?;
                            let Some(concrete) = decoder.try_decode::<Q>(erased.payload())? else {
                                return Ok(None);
                            };
                            let (output, stats) = canonical_topk::execute_canonical_query(
                                concrete,
                                erased.predicate_cloned(),
                                erased.selector_cloned(),
                                state,
                                params,
                                limits,
                                output_limits,
                                budget,
                            )?;
                            return Ok(Some((QueryResponse::Iterable(output), stats)));
                        }
                        // Decode the concrete query variant from the payload.
                        let Some(concrete) = decoder.try_decode::<Q>(erased.payload())? else {
                            return Ok(None);
                        };
                        // Execute the concrete ValidQuery with provided predicate
                        let iter = execute_iterable_source(
                            concrete,
                            erased.predicate_cloned(),
                            params,
                            limits,
                            ordinary_memory::OrdinaryCursorMode::Ephemeral,
                            state,
                        )?;

                        // Postprocess: sort/paginate/project and return only the first batch (no cursor)
                        let (output, stats) = apply_query_postprocessing_ephemeral_with_budget(
                            iter,
                            erased.selector_cloned(),
                            params,
                            limits,
                            budget,
                        )?;
                        return Ok(Some((QueryResponse::Iterable(output), stats)));
                    }
                    Ok(None)
                }

                let params = &iter_query.params;
                // Fast-DSL path: when the boxed query payload is not present, reconstruct
                // from item kind and encoded predicate/selector.
                if legacy_query_box(&iter_query).is_none() {
                    if limits.canonical_output_limits.is_some() {
                        return Err(Error::Conversion(
                            "canonical fanout rejects opaque fast-DSL starts before nested payload, predicate, or selector decoding"
                                .to_owned(),
                        ));
                    }
                    #[cfg(feature = "fast_dsl")]
                    {
                        use iroha_data_model::query::QueryItemKind;
                        let mut decoder = FastIterComponentDecoder::new(
                            limits,
                            [
                                &iter_query.query_payload,
                                &iter_query.predicate_bytes,
                                &iter_query.selector_bytes,
                            ],
                        )?;
                        // Helper to run an iterable query using the encoded predicate/selector.
                        macro_rules! run_payload_or_default {
                            // Unit queries have an empty canonical payload. Reject any other bytes so
                            // parameterized or malformed payloads cannot become global queries.
                            ($itemty:ty, $find:ty) => {{
                                let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    decoder.decode(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    decoder.decode(&iter_query.selector_bytes)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Ephemeral,
                                    state,
                                )?;
                                let (output, processed_items) =
                                    apply_query_postprocessing_ephemeral_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }};
                            // For queries that always require a payload (e.g., FindPermissionsByAccountId)
                            (require_payload $itemty:ty, $find:ty) => {{
                                let concrete: $find = decoder.decode(&iter_query.query_payload)?;
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    decoder.decode(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    decoder.decode(&iter_query.selector_bytes)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Ephemeral,
                                    state,
                                )?;
                                let (output, processed_items) =
                                    apply_query_postprocessing_ephemeral_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }};
                        }
                        match iter_query.item {
                            QueryItemKind::Domain => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::domain::Domain,
                                        iroha_data_model::query::domain::prelude::FindDomainsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::domain::Domain,
                                    iroha_data_model::query::domain::prelude::FindDomains
                                )
                            }
                            QueryItemKind::Account => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::account::Account, iroha_data_model::query::account::prelude::FindAccountsWithAsset)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::account::Account,
                                    iroha_data_model::query::account::prelude::FindAccounts
                                )
                            }
                            QueryItemKind::AccountId => run_payload_or_default!(
                                iroha_data_model::account::AccountId,
                                iroha_data_model::query::account::prelude::FindAccountIds
                            ),
                            QueryItemKind::Asset => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::asset::value::Asset,
                                        iroha_data_model::query::asset::prelude::FindAssetsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::asset::value::Asset,
                                    iroha_data_model::query::asset::prelude::FindAssets
                                )
                            }
                            QueryItemKind::AssetDefinition => run_payload_or_default!(
                                iroha_data_model::asset::definition::AssetDefinition,
                                iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                            ),
                            QueryItemKind::RepoAgreement => run_payload_or_default!(
                                iroha_data_model::repo::RepoAgreement,
                                iroha_data_model::query::repo::prelude::FindRepoAgreements
                            ),
                            QueryItemKind::Nft => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::nft::Nft,
                                        iroha_data_model::query::nft::prelude::FindNftsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nft::Nft,
                                    iroha_data_model::query::nft::prelude::FindNfts
                                )
                            }
                            QueryItemKind::Rwa => run_payload_or_default!(
                                iroha_data_model::rwa::Rwa,
                                iroha_data_model::query::rwa::prelude::FindRwas
                            ),
                            QueryItemKind::Role => run_payload_or_default!(
                                iroha_data_model::role::Role,
                                iroha_data_model::query::role::prelude::FindRoles
                            ),
                            QueryItemKind::RoleId => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::role::RoleId, iroha_data_model::query::role::prelude::FindRolesByAccountId)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::role::RoleId,
                                    iroha_data_model::query::role::prelude::FindRoleIds
                                )
                            }
                            QueryItemKind::PeerId => run_payload_or_default!(
                                iroha_data_model::peer::PeerId,
                                iroha_data_model::query::peer::prelude::FindPeers
                            ),
                            QueryItemKind::TriggerId => run_payload_or_default!(
                                iroha_data_model::trigger::TriggerId,
                                iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                            ),
                            QueryItemKind::Trigger => run_payload_or_default!(
                                iroha_data_model::trigger::Trigger,
                                iroha_data_model::query::trigger::prelude::FindTriggers
                            ),
                            QueryItemKind::CommittedTransaction => {
                                let _concrete = decoder.decode::<
                                    iroha_data_model::query::transaction::prelude::FindTransactions,
                                >(&iter_query.query_payload)
                                ?;
                                let pred = decoder
                                    .decode::<CompoundPredicate<CommittedTransaction>>(
                                        &iter_query.predicate_bytes,
                                    )?;
                                let sel = decoder.decode::<SelectorTuple<CommittedTransaction>>(
                                    &iter_query.selector_bytes,
                                )?;
                                let (output, processed_items) = handle_find_transactions_ephemeral(
                                    state,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    budget_items,
                                )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }
                            QueryItemKind::SignedBlock => run_payload_or_default!(
                                iroha_data_model::block::SignedBlock,
                                iroha_data_model::query::block::prelude::FindBlocks
                            ),
                            QueryItemKind::BlockHeader => run_payload_or_default!(
                                iroha_data_model::block::BlockHeader,
                                iroha_data_model::query::block::prelude::FindBlockHeaders
                            ),
                            QueryItemKind::ProofRecord => {
                                let pred = decoder
                                    .decode::<iroha_data_model::query::dsl::CompoundPredicate<
                                    iroha_data_model::proof::ProofRecord,
                                >>(
                                    &iter_query.predicate_bytes
                                )?;
                                let sel = decoder
                                    .decode::<iroha_data_model::query::dsl::SelectorTuple<
                                        iroha_data_model::proof::ProofRecord,
                                    >>(
                                        &iter_query.selector_bytes
                                    )?;
                                macro_rules! try_proof_query {
                                    ($find:ty) => {{
                                        if let Some(concrete) = decoder
                                            .try_decode::<$find>(&iter_query.query_payload)?
                                        {
                                            let iter = execute_iterable_source(
                                                concrete,
                                                pred,
                                                params,
                                                limits,
                                                ordinary_memory::OrdinaryCursorMode::Ephemeral,
                                                state,
                                            )?;
                                            let (output, processed_items) =
                                                apply_query_postprocessing_ephemeral_with_budget(
                                                    iter,
                                                    sel,
                                                    params,
                                                    limits,
                                                    budget_items,
                                                )?;
                                            return Ok((
                                                QueryResponse::Iterable(output),
                                                processed_items,
                                            ));
                                        }
                                    }};
                                }
                                if !iter_query.query_payload.is_empty() {
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByBackend
                                    );
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByStatus
                                    );
                                    return Err(Error::Conversion(
                                        "failed to decode proof query payload".into(),
                                    ));
                                }
                                let concrete = decoder.decode::<
                                    iroha_data_model::query::proof::prelude::FindProofRecords,
                                >(&iter_query.query_payload)?;
                                let iter = execute_iterable_source(
                                    concrete,
                                    pred,
                                    params,
                                    limits,
                                    ordinary_memory::OrdinaryCursorMode::Ephemeral,
                                    state,
                                )?;
                                let (output, processed_items) =
                                    apply_query_postprocessing_ephemeral_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }
                            QueryItemKind::AssetEscrowRecord => run_payload_or_default!(
                                iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrows
                            ),
                            QueryItemKind::AssetEscrowsBySeller => run_payload_or_default!(
                                require_payload iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrowsBySeller
                            ),
                            QueryItemKind::AssetEscrowsByBuyer => run_payload_or_default!(
                                require_payload iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrowsByBuyer
                            ),
                            QueryItemKind::AssetEscrowsByStatus => run_payload_or_default!(
                                require_payload iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrowsByStatus
                            ),
                            QueryItemKind::OracleFeedConfig => run_payload_or_default!(
                                iroha_data_model::oracle::FeedConfig,
                                iroha_data_model::query::oracle::prelude::FindOracleFeeds
                            ),
                            QueryItemKind::OracleFeedEventRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::events::data::oracle::FeedEventRecord, iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId)
                            }
                            QueryItemKind::OracleProviderStatsRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::OracleProviderStatsRecord, iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId)
                            }
                            QueryItemKind::OracleDispute => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::oracle::OracleDispute,
                                        iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::oracle::OracleDispute,
                                    iroha_data_model::query::oracle::prelude::FindOracleDisputes
                                )
                            }
                            QueryItemKind::OracleChangeProposal => run_payload_or_default!(
                                iroha_data_model::oracle::OracleChangeProposal,
                                iroha_data_model::query::oracle::prelude::FindOracleChanges
                            ),
                            QueryItemKind::TwitterBindingRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::TwitterBindingRecord, iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid)
                            }
                            QueryItemKind::DefiOracleAttestation => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::DefiOracleAttestation, iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey)
                            }
                            QueryItemKind::Permission => {
                                run_payload_or_default!(require_payload iroha_data_model::permission::Permission, iroha_data_model::query::permission::prelude::FindPermissionsByAccountId)
                            }
                            QueryItemKind::FeeSponsorProgram => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::nexus::FeeSponsorProgram, iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramsBySponsor)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nexus::FeeSponsorProgram,
                                    iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                                )
                            }
                            QueryItemKind::FeeSponsorProgramId => run_payload_or_default!(
                                iroha_data_model::nexus::FeeSponsorProgramId,
                                iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                            ),
                        }
                    }
                    #[cfg(not(feature = "fast_dsl"))]
                    {
                        return Err(Error::Conversion("missing iterator payload".into()));
                    }
                }
                let Some(qbox) = legacy_query_box(&iter_query) else {
                    return Err(Error::Conversion("missing iterator payload".into()));
                };

                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomains,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::domain::prelude::FindDomains>(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccounts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::account::prelude::FindAccounts>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssets,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| try_decode_query::<iroha_data_model::query::asset::prelude::FindAssets>(e),
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::asset::definition::AssetDefinition,
                    iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNftsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::nft::prelude::FindNftsByAccountId>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNfts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| try_decode_query::<iroha_data_model::query::nft::prelude::FindNfts>(e),
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::role::Role,
                    iroha_data_model::query::role::prelude::FindRoles,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| try_decode_query::<iroha_data_model::query::role::prelude::FindRoles>(e),
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRoleIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        e.payload()
                            .is_empty()
                            .then_some(iroha_data_model::query::role::prelude::FindRoleIds)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRolesByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::role::prelude::FindRolesByAccountId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::peer::PeerId,
                    iroha_data_model::query::peer::prelude::FindPeers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| try_decode_query::<iroha_data_model::query::peer::prelude::FindPeers>(e),
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::trigger::TriggerId,
                    iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        e.payload().is_empty().then_some(
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::trigger::Trigger,
                    iroha_data_model::query::trigger::prelude::FindTriggers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::trigger::prelude::FindTriggers>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some(erased) = query::iter_query_inner::<CommittedTransaction>(qbox) {
                    if limits.canonical_output_limits.is_some() {
                        return Err(Error::Conversion(
                            "canonical fanout rejects `FindTransactions` before payload decoding or source execution"
                                .into(),
                        ));
                    }
                    let mut decoder =
                        FastIterComponentDecoder::new(limits, [erased.payload(), &[], &[]])?;
                    if decoder
                        .try_decode::<
                            iroha_data_model::query::transaction::prelude::FindTransactions,
                        >(erased.payload())?
                        .is_none()
                    {
                        return Err(Error::Conversion(
                            "malformed payload for transaction iterable query".into(),
                        ));
                    }
                    let (output, processed_items) = handle_find_transactions_ephemeral(
                        state,
                        erased.predicate_cloned(),
                        erased.selector_cloned(),
                        params,
                        limits,
                        budget_items,
                    )?;
                    return Ok((QueryResponse::Iterable(output), processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::block::SignedBlock,
                    iroha_data_model::query::block::prelude::FindBlocks,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| try_decode_query::<iroha_data_model::query::block::prelude::FindBlocks>(e),
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::block::BlockHeader,
                    iroha_data_model::query::block::prelude::FindBlockHeaders,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::block::prelude::FindBlockHeaders>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecords,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::proof::prelude::FindProofRecords>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::FeedConfig,
                    iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::oracle::prelude::FindOracleFeeds>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::events::data::oracle::FeedEventRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleProviderStatsRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleChangeProposal,
                    iroha_data_model::query::oracle::prelude::FindOracleChanges,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleChanges,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::TwitterBindingRecord,
                    iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::DefiOracleAttestation,
                    iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }

                macro_rules! try_remaining_ephemeral_dispatch {
                    ($item:ty, $query:ty) => {
                        if let Some((response, processed_items)) = run_dispatch::<$item, $query, _>(
                            qbox,
                            params,
                            limits,
                            budget_items,
                            state,
                            live_query_store,
                            authority,
                            None,
                            |erased| try_decode_query::<$query>(erased),
                        )? {
                            return Ok((response, processed_items));
                        }
                    };
                }
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::account::AccountId,
                    iroha_data_model::query::account::prelude::FindAccountIds
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::repo::RepoAgreement,
                    iroha_data_model::query::repo::prelude::FindRepoAgreements
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::rwa::Rwa,
                    iroha_data_model::query::rwa::prelude::FindRwas
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::permission::Permission,
                    iroha_data_model::query::permission::prelude::FindPermissionsByAccountId
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrowsBySeller
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrowsByBuyer
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrowsByStatus
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::escrow::AssetEscrowRecord,
                    iroha_data_model::query::escrow::prelude::FindAssetEscrows
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::nexus::FeeSponsorProgram,
                    iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramsBySponsor
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::nexus::FeeSponsorProgram,
                    iroha_data_model::query::nexus::prelude::FindFeeSponsorPrograms
                );
                try_remaining_ephemeral_dispatch!(
                    iroha_data_model::nexus::FeeSponsorProgramId,
                    iroha_data_model::query::nexus::prelude::FindFeeSponsorProgramIds
                );

                Err(Error::Conversion(
                    "unsupported iterable query in ephemeral execution".into(),
                ))
            }
            QueryRequest::Continue(_cursor) => Err(Error::Conversion(
                "ephemeral execution does not support continuation".into(),
            )),
        }
    }
}
