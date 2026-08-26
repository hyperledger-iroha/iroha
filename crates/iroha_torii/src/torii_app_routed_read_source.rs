// Source-bounded materialization for application-API routed reads.
use std::marker::PhantomData;
use norito::core::{DecodeFlagsGuard, DeriveSmallBuf, Encoder, NoritoDeserialize};
/// Borrowed wire-equivalent of a derived struct in declaration order.
///
/// App routes use this adapter when world state stores an identifier separately
/// from its value. It writes the target DTO directly into the admitted
/// canonical frame, avoiding an unmetered deep clone before the first bounded
/// decode.
struct ToriiBorrowedRoutedReadStruct<'a, T, const N: usize> {
    fields: [&'a dyn norito::core::NoritoSerialize; N],
    marker: PhantomData<T>,
}
impl<'a, T, const N: usize> ToriiBorrowedRoutedReadStruct<'a, T, N> {
    const fn new(fields: [&'a dyn norito::core::NoritoSerialize; N]) -> Self {
        Self {
            fields,
            marker: PhantomData,
        }
    }
}
impl<T, const N: usize> norito::core::NoritoSerialize for ToriiBorrowedRoutedReadStruct<'_, T, N>
where
    T: norito::core::NoritoSerialize,
{
    fn schema_hash() -> [u8; 16] {
        T::schema_hash()
    }
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::core::Error> {
        if norito::core::use_packed_struct() {
            return Err(norito::core::Error::UnsupportedFeature(
                "borrowed routed-read packed struct",
            ));
        }
        let mut scratch = DeriveSmallBuf::new();
        for value in self.fields.iter().copied() {
            norito::core::write_len_prefixed(writer, value, &mut scratch)?;
        }
        Ok(())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        if norito::core::use_packed_struct() {
            return None;
        }
        self.fields.iter().try_fold(0usize, |total, value| {
            let value_len = value.encoded_len_exact()?;
            total
                .checked_add(norito::core::len_prefix_len(value_len))?
                .checked_add(value_len)
        })
    }
}
/// Materialize the first owned route result through the admitted E/D corridor.
///
/// `source` may borrow arbitrarily nested world-state fields. The only
/// source-sized allocation made here is the hard-capped canonical frame. Its
/// owned replacement is then decoded with explicit Norito limits and charged
/// to the routed-read accumulator before it can escape this function.
fn torii_bounded_routed_read_source_payload<T, S>(
    source: &S,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<ToriiBoundedNoritoPayload<T>, Response>
where
    S: norito::core::NoritoSerialize,
    T: norito::core::NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let plan = budget.decode_plan(0)?;
    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let canonical_bytes = norito::core::to_bytes_bounded(source, plan.canonical_limit_bytes)
        .map_err(|_| torii_routed_read_norito_encode_response())?;
    let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::decode_from_bytes_with_limits::<T>(&canonical_bytes, plan.limits)
    });
    let value = value.map_err(torii_routed_read_norito_decode_response)?;
    budget.retain_decode_usage(usage)?;
    budget.retain_canonical_capacity(canonical_bytes.capacity())?;
    Ok(ToriiBoundedNoritoPayload {
        value,
        canonical_bytes,
    })
}
fn torii_bounded_routed_read_payload_response<T>(
    payload: ToriiBoundedNoritoPayload<T>,
    format: ResponseFormat,
    budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response>
where
    T: JsonSerialize,
{
    match format {
        ResponseFormat::Norito => {
            torii_routed_read_ensure(
                "local route response body",
                payload.canonical_bytes.len(),
                budget.route_body_limit(),
            )?;
            drop(payload.value);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
                )
                .body(Body::from(payload.canonical_bytes))
                .expect("build preflighted source-bounded routed-read response"))
        }
        ResponseFormat::Json => {
            let ToriiBoundedNoritoPayload {
                value,
                canonical_bytes,
            } = payload;
            drop(canonical_bytes);
            let body = budget.json_body(&value)?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                )
                .body(Body::from(Bytes::from(body)))
                .expect("build preflighted source-bounded routed-read JSON response"))
        }
    }
}
fn torii_bounded_routed_read_source_response<T, S>(
    source: &S,
    format: ResponseFormat,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response>
where
    S: norito::core::NoritoSerialize,
    T: JsonSerialize + norito::core::NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let payload = torii_bounded_routed_read_source_payload::<T, S>(source, &mut budget)?;
    torii_bounded_routed_read_payload_response(payload, format, budget)
}
fn torii_local_routed_read_budget(
    app: &SharedAppState,
) -> Result<ToriiRoutedReadMemoryBudget, Response> {
    ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    )
}
fn execute_torii_account_local_source_read(
    app: &SharedAppState,
    account_literal: &str,
    format: ResponseFormat,
) -> Response {
    let telemetry = app.telemetry_handle();
    let (account_id, _) = match routing::parse_account_path_segment_with_state(
        app.state.as_ref(),
        account_literal,
        &telemetry,
        routing::ENDPOINT_ACCOUNTS_GET,
    ) {
        Ok(parsed) => parsed,
        Err(error) => return error_response_with_format(error, format),
    };
    let state_view = app.state.view();
    let world = state_view.world();
    let account = match world.account(&account_id) {
        Ok(account) => account,
        Err(_) => {
            return error_response_with_format(
                Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::NotFound,
                )),
                format,
            );
        }
    };
    let details = account.value().as_ref();
    let no_label = None::<iroha_data_model::account::AccountAlias>;
    let source = ToriiBorrowedRoutedReadStruct::<AccountReadResponse, 4>::new([
        account.id(),
        &no_label,
        &details.uaid,
        &details.opaque_ids,
    ]);
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    torii_bounded_routed_read_source_response::<AccountReadResponse, _>(&source, format, budget)
        .unwrap_or_else(|response| response)
}
fn execute_torii_internal_account_local_source_read(
    app: &SharedAppState,
    account_literal: &str,
    format: ResponseFormat,
) -> Response {
    let (account_id, _) = match parse_exact_account_id_literal(account_literal) {
        Ok(parsed) => parsed,
        Err(error) => return error_response_with_format(error, format),
    };
    let state_view = app.state.view();
    let world = state_view.world();
    let account = match world.account(&account_id) {
        Ok(account) => account,
        Err(_) => {
            return trusted_internal_read_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "the exact canonical account was not found on this route",
                format,
            );
        }
    };
    let details = account.value().as_ref();
    let source = ToriiBorrowedRoutedReadStruct::<InternalAccountReadResponse, 4>::new([
        account.id(),
        &details.metadata,
        &details.uaid,
        &details.opaque_ids,
    ]);
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    torii_bounded_routed_read_source_response::<InternalAccountReadResponse, _>(
        &source, format, budget,
    )
    .unwrap_or_else(|response| response)
}
fn execute_torii_internal_account_asset_local_source_read(
    app: &SharedAppState,
    account_literal: &str,
    asset_definition_literal: &str,
    scope_literal: &str,
    format: ResponseFormat,
) -> Response {
    let (account_id, _) = match parse_exact_account_id_literal(account_literal) {
        Ok(parsed) => parsed,
        Err(error) => return error_response_with_format(error, format),
    };
    let asset_definition_id =
        match parse_exact_asset_definition_id_literal(asset_definition_literal) {
            Ok(definition) => definition,
            Err(error) => return error_response_with_format(error, format),
        };
    let scope = match parse_exact_asset_balance_scope_literal(scope_literal) {
        Ok(scope) => scope,
        Err(error) => return error_response_with_format(error, format),
    };
    let asset_id = AssetId::with_scope(asset_definition_id, account_id, scope);
    let state_view = app.state.view();
    let world = state_view.world();
    let asset = match world.asset(&asset_id) {
        Ok(asset) => asset,
        Err(_) => {
            return trusted_internal_read_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "the exact account asset bucket was not found on this route",
                format,
            );
        }
    };
    let source =
        ToriiBorrowedRoutedReadStruct::<Asset, 2>::new([asset.id(), asset.value().as_ref()]);
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    torii_bounded_routed_read_source_response::<Asset, _>(&source, format, budget)
        .unwrap_or_else(|response| response)
}
/// Borrowed JSON projection for the single asset-definition route.
///
/// The public shape historically passed the definition through a native JSON
/// `Value` so an active alias binding could replace `alias` and add
/// `alias_binding`. Writing the same sorted object directly avoids cloning the
/// complete definition and materializing that intermediate value graph before
/// the routed-read body cap is enforced.
struct ToriiAssetDefinitionJsonSource<'a> {
    definition: &'a iroha_data_model::asset::definition::AssetDefinition,
    alias_binding: Option<&'a iroha_core::state::AssetDefinitionAliasBindingRecord>,
    observation_time_ms: u64,
}
impl norito::json::FastJsonWrite for ToriiAssetDefinitionJsonSource<'_> {
    fn write_json(&self, output: &mut String) {
        norito::json::write_json_unbounded(self, output);
    }
    fn write_json_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        use norito::json::JsonSerialize as _;
        output.begin_container()?;
        output.push_str("{\"alias\":")?;
        if let Some(binding) = self.alias_binding {
            norito::json::write_json_string_to(binding.alias.as_ref(), output)?;
            output.push_str(",\"alias_binding\":")?;
            write_torii_asset_alias_binding_json(binding, self.observation_time_ms, output)?;
        } else {
            self.definition.alias.json_serialize_to(output)?;
        }
        output.push_str(",\"balance_scope_policy\":")?;
        self.definition
            .balance_scope_policy
            .json_serialize_to(output)?;
        output.push_str(",\"confidential_policy\":")?;
        self.definition
            .confidential_policy
            .json_serialize_to(output)?;
        output.push_str(",\"description\":")?;
        self.definition.description.json_serialize_to(output)?;
        output.push_str(",\"id\":")?;
        self.definition.id.json_serialize_to(output)?;
        output.push_str(",\"logo\":")?;
        self.definition.logo.json_serialize_to(output)?;
        output.push_str(",\"metadata\":")?;
        iroha_data_model::HasMetadata::metadata(self.definition).json_serialize_to(output)?;
        output.push_str(",\"mintable\":")?;
        self.definition.mintable.json_serialize_to(output)?;
        output.push_str(",\"name\":")?;
        self.definition.name.json_serialize_to(output)?;
        output.push_str(",\"owned_by\":")?;
        self.definition.owned_by.json_serialize_to(output)?;
        output.push_str(",\"owning_domain\":")?;
        self.definition.owning_domain.json_serialize_to(output)?;
        output.push_str(",\"spec\":")?;
        self.definition.spec.json_serialize_to(output)?;
        output.push_str(",\"total_quantity\":")?;
        self.definition.total_quantity.json_serialize_to(output)?;
        output.push('}')?;
        output.end_container();
        Ok(())
    }
}
fn write_torii_asset_alias_binding_json(
    binding: &iroha_core::state::AssetDefinitionAliasBindingRecord,
    observation_time_ms: u64,
    output: &mut dyn norito::json::JsonWriteSink,
) -> Result<(), norito::json::BoundedJsonError> {
    use iroha_core::state::AssetDefinitionAliasLeaseStatus;
    use norito::json::JsonSerialize as _;
    let status = match binding.status_at(observation_time_ms) {
        AssetDefinitionAliasLeaseStatus::Permanent => "permanent",
        AssetDefinitionAliasLeaseStatus::LeasedActive => "leased_active",
        AssetDefinitionAliasLeaseStatus::LeasedGrace => "leased_grace",
        AssetDefinitionAliasLeaseStatus::ExpiredPendingCleanup => "expired_pending_cleanup",
    };
    output.begin_container()?;
    output.push_str("{\"alias\":")?;
    norito::json::write_json_string_to(binding.alias.as_ref(), output)?;
    output.push_str(",\"bound_at_ms\":")?;
    binding.bound_at_ms.json_serialize_to(output)?;
    if let Some(grace_until_ms) = binding.grace_until_ms {
        output.push_str(",\"grace_until_ms\":")?;
        grace_until_ms.json_serialize_to(output)?;
    }
    if let Some(lease_expiry_ms) = binding.lease_expiry_ms {
        output.push_str(",\"lease_expiry_ms\":")?;
        lease_expiry_ms.json_serialize_to(output)?;
    }
    output.push_str(",\"status\":")?;
    norito::json::write_json_string_to(status, output)?;
    output.push('}')?;
    output.end_container();
    Ok(())
}
fn resolve_torii_asset_definition_source_selector(
    world: &impl iroha_core::state::WorldReadOnly,
    asset_literal: &str,
    observation_time_ms: u64,
) -> Result<iroha_data_model::asset::AssetDefinitionId, Error> {
    const INVALID_SELECTOR_MSG: &str = "invalid asset selector; expected a canonical Base58 asset id or an on-chain asset alias `<name>#<domain>.<dataspace>` / `<name>#<dataspace>`";
    let selector = asset_literal.trim();
    if selector.is_empty() {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(INVALID_SELECTOR_MSG.to_owned()),
        ));
    }
    if let Ok(id) = selector.parse::<iroha_data_model::asset::AssetDefinitionId>() {
        return world
            .asset_definitions()
            .get(&id)
            .map(|_| id)
            .ok_or_else(|| {
                Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::NotFound,
                ))
            });
    }
    let alias: iroha_data_model::asset::AssetDefinitionAlias = selector.parse().map_err(|_| {
        Error::Query(iroha_data_model::ValidationFail::NotPermitted(
            INVALID_SELECTOR_MSG.to_owned(),
        ))
    })?;
    world
        .asset_definition_id_by_alias_at(&alias, observation_time_ms)
        .ok_or_else(|| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            ))
        })
}
fn execute_torii_asset_definition_local_source_read(
    app: &SharedAppState,
    asset_literal: &str,
) -> Response {
    let state_view = app.state.view();
    let world = state_view.world();
    let observation_time_ms = routing::asset_alias_observation_time_ms(app.state.as_ref());
    let definition_id = match resolve_torii_asset_definition_source_selector(
        world,
        asset_literal,
        observation_time_ms,
    ) {
        Ok(id) => id,
        Err(error) => return error_response_with_format(error, ResponseFormat::Json),
    };
    let Some(definition) = world.asset_definitions().get(&definition_id) else {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            )),
            ResponseFormat::Json,
        );
    };
    let source = ToriiAssetDefinitionJsonSource {
        definition,
        alias_binding: world.asset_definition_alias_bindings().get(&definition_id),
        observation_time_ms,
    };
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    budget
        .json_response(&source)
        .unwrap_or_else(|response| response)
}
struct ToriiSpaceDirectoryBindingsJsonSource<'a> {
    uaid: &'a iroha_data_model::nexus::UniversalAccountId,
    bindings: Option<&'a iroha_core::nexus::space_directory::UaidDataspaceBindings>,
    catalog: &'a iroha_data_model::nexus::DataSpaceCatalog,
}
impl norito::json::FastJsonWrite for ToriiSpaceDirectoryBindingsJsonSource<'_> {
    fn write_json(&self, output: &mut String) {
        norito::json::write_json_unbounded(self, output);
    }
    fn write_json_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        use norito::json::JsonSerialize as _;
        output.begin_container()?;
        output.push_str("{\"dataspaces\":[")?;
        if let Some(bindings) = self.bindings {
            for (index, (dataspace_id, accounts)) in bindings.iter().enumerate() {
                if index != 0 {
                    output.push(',')?;
                }
                output.begin_container()?;
                output.push_str("{\"accounts\":[")?;
                for (account_index, account_id) in accounts.iter().enumerate() {
                    if account_index != 0 {
                        output.push(',')?;
                    }
                    account_id.json_serialize_to(output)?;
                }
                output.push_str("],\"dataspace_alias\":")?;
                self.catalog
                    .entries()
                    .iter()
                    .find(|entry| entry.id == *dataspace_id)
                    .map(|entry| entry.alias.as_str())
                    .json_serialize_to(output)?;
                output.push_str(",\"dataspace_id\":")?;
                dataspace_id.as_u64().json_serialize_to(output)?;
                output.push('}')?;
                output.end_container();
            }
        }
        output.push_str("],\"uaid\":")?;
        self.uaid.json_serialize_to(output)?;
        output.push('}')?;
        output.end_container();
        Ok(())
    }
}
fn parse_torii_space_directory_uaid_literal(
    raw: &str,
) -> Result<iroha_data_model::nexus::UniversalAccountId, Error> {
    use core::str::FromStr as _;
    iroha_data_model::nexus::UniversalAccountId::from_str(raw).map_err(|_| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::InvalidSingularParameters,
        ))
    })
}
fn execute_torii_space_directory_bindings_local_source_read(
    app: &SharedAppState,
    uaid_literal: &str,
) -> Response {
    let uaid = match parse_torii_space_directory_uaid_literal(uaid_literal) {
        Ok(uaid) => uaid,
        Err(error) => return error_response_with_format(error, ResponseFormat::Json),
    };
    let state_view = app.state.view();
    let world = state_view.world();
    let source = ToriiSpaceDirectoryBindingsJsonSource {
        uaid: &uaid,
        bindings: world.uaid_dataspaces().get(&uaid),
        catalog: world.dataspace_catalog(),
    };
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    budget
        .json_response(&source)
        .unwrap_or_else(|response| response)
}
struct ToriiContractAliasJsonSource<'a> {
    contract_alias: &'a iroha_data_model::smart_contract::ContractAlias,
    contract_address: &'a iroha_data_model::smart_contract::ContractAddress,
    contract_subject: &'a iroha_data_model::account::AccountId,
    dataspace_alias: &'a str,
    binding: &'a iroha_core::state::ContractAliasBindingRecord,
    observation_time_ms: u64,
}
impl norito::json::FastJsonWrite for ToriiContractAliasJsonSource<'_> {
    fn write_json(&self, output: &mut String) {
        norito::json::write_json_unbounded(self, output);
    }
    fn write_json_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        use norito::json::JsonSerialize as _;
        output.begin_container()?;
        output.push_str("{\"contract_alias\":")?;
        self.contract_alias.json_serialize_to(output)?;
        output.push_str(",\"contract_address\":")?;
        self.contract_address.json_serialize_to(output)?;
        output.push_str(",\"contract_subject_account\":")?;
        self.contract_subject.json_serialize_to(output)?;
        output.push_str(",\"dataspace\":")?;
        norito::json::write_json_string_to(self.dataspace_alias, output)?;
        output.push_str(",\"contract_alias_binding\":")?;
        write_torii_contract_alias_binding_json(self.binding, self.observation_time_ms, output)?;
        output.push_str(",\"source\":\"world_state\"}")?;
        output.end_container();
        Ok(())
    }
}
fn write_torii_contract_alias_binding_json(
    binding: &iroha_core::state::ContractAliasBindingRecord,
    observation_time_ms: u64,
    output: &mut dyn norito::json::JsonWriteSink,
) -> Result<(), norito::json::BoundedJsonError> {
    use iroha_core::state::ContractAliasLeaseStatus;
    use norito::json::JsonSerialize as _;
    let status = match binding.status_at(observation_time_ms) {
        ContractAliasLeaseStatus::Permanent => "permanent",
        ContractAliasLeaseStatus::LeasedActive => "leased_active",
        ContractAliasLeaseStatus::LeasedGrace => "leased_grace",
        ContractAliasLeaseStatus::ExpiredPendingCleanup => "expired_pending_cleanup",
    };
    output.begin_container()?;
    output.push_str("{\"alias\":")?;
    binding.alias.json_serialize_to(output)?;
    output.push_str(",\"status\":")?;
    norito::json::write_json_string_to(status, output)?;
    if let Some(lease_expiry_ms) = binding.lease_expiry_ms {
        output.push_str(",\"lease_expiry_ms\":")?;
        lease_expiry_ms.json_serialize_to(output)?;
    }
    if let Some(grace_until_ms) = binding.grace_until_ms {
        output.push_str(",\"grace_until_ms\":")?;
        grace_until_ms.json_serialize_to(output)?;
    }
    output.push_str(",\"bound_at_ms\":")?;
    binding.bound_at_ms.json_serialize_to(output)?;
    output.push('}')?;
    output.end_container();
    Ok(())
}
fn execute_torii_contract_alias_local_source_read(
    app: &SharedAppState,
    alias_input: &str,
) -> Response {
    use core::str::FromStr as _;
    if alias_input.is_empty() {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "contract alias must not be empty".to_owned(),
                ),
            )),
            ResponseFormat::Json,
        );
    }
    if alias_input.trim() != alias_input {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "contract alias must be a canonical literal without surrounding whitespace"
                        .to_owned(),
                ),
            )),
            ResponseFormat::Json,
        );
    }
    let contract_alias =
        match iroha_data_model::smart_contract::ContractAlias::from_str(alias_input) {
            Ok(alias) => alias,
            Err(error) => {
                return error_response_with_format(
                    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::Conversion(
                            error.to_string(),
                        ),
                    )),
                    ResponseFormat::Json,
                );
            }
        };
    let Some(dataspace_id) =
        dataspace_id_for_alias_segment(app, contract_alias.dataspace_segment())
    else {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "unknown or inactive dataspace alias `{}` in contract alias",
                    contract_alias.dataspace_segment()
                )),
            )),
            ResponseFormat::Json,
        );
    };
    let observation_time_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let state_view = app.state.view();
    let world = state_view.world();
    let Some(contract_address) = world.contract_aliases().get(&contract_alias) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(binding) = world
        .contract_alias_bindings()
        .get(contract_address)
        .filter(|binding| {
            binding.alias == contract_alias && !binding.is_grace_expired_at(observation_time_ms)
        })
    else {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                "contract alias index has no matching active consensus binding".to_owned(),
            )),
            ResponseFormat::Json,
        );
    };
    if contract_address.dataspace_id().ok() != Some(dataspace_id) {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "contract alias dataspace does not match bound contract address".to_owned(),
                ),
            )),
            ResponseFormat::Json,
        );
    }
    let Some(contract_subject) =
        iroha_core::smartcontracts::code::borrow_bound_contract_subject_from_world(
            world,
            contract_address,
        )
    else {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                "active contract alias has no valid consensus subject binding".to_owned(),
            )),
            ResponseFormat::Json,
        );
    };
    let dataspace_alias = world
        .dataspace_catalog()
        .by_id(dataspace_id)
        .map(|entry| entry.alias.as_str())
        .unwrap_or_else(|| contract_alias.dataspace_segment());
    let source = ToriiContractAliasJsonSource {
        contract_alias: &contract_alias,
        contract_address,
        contract_subject,
        dataspace_alias,
        binding,
        observation_time_ms,
    };
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    budget
        .json_response(&source)
        .unwrap_or_else(|response| response)
}
struct ToriiExplorerAssetDefinitionJsonSource<'a> {
    definition: &'a iroha_data_model::asset::definition::AssetDefinition,
    assets: u32,
    locked_quantity: Option<&'a iroha_primitives::numeric::Quantity>,
    circulating_quantity: Option<&'a iroha_primitives::numeric::Quantity>,
}
impl norito::json::FastJsonWrite for ToriiExplorerAssetDefinitionJsonSource<'_> {
    fn write_json(&self, output: &mut String) {
        norito::json::write_json_unbounded(self, output);
    }
    fn write_json_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        use norito::json::JsonSerialize as _;
        output.begin_container()?;
        output.push_str("{\"id\":")?;
        self.definition.id.json_serialize_to(output)?;
        output.push_str(",\"owning_domain\":")?;
        self.definition.owning_domain.json_serialize_to(output)?;
        output.push_str(",\"mintable\":")?;
        self.definition.mintable.json_serialize_to(output)?;
        output.push_str(",\"logo\":")?;
        self.definition.logo.json_serialize_to(output)?;
        output.push_str(",\"metadata\":")?;
        iroha_data_model::HasMetadata::metadata(self.definition).json_serialize_to(output)?;
        output.push_str(",\"owned_by\":")?;
        self.definition.owned_by.json_serialize_to(output)?;
        output.push_str(",\"assets\":")?;
        self.assets.json_serialize_to(output)?;
        output.push_str(",\"total_quantity\":")?;
        self.definition.total_quantity.json_serialize_to(output)?;
        output.push_str(",\"locked_quantity\":")?;
        self.locked_quantity.json_serialize_to(output)?;
        output.push_str(",\"circulating_quantity\":")?;
        self.circulating_quantity.json_serialize_to(output)?;
        output.push('}')?;
        output.end_container();
        Ok(())
    }
}
fn execute_torii_explorer_asset_definition_local_source_read(
    app: &SharedAppState,
    definition_id: &iroha_data_model::asset::AssetDefinitionId,
) -> Response {
    let world = app.state.world_view();
    let Some(definition) = world.asset_definitions().get(definition_id) else {
        return error_response_with_format(
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            )),
            ResponseFormat::Json,
        );
    };
    let assets = world
        .asset_definition_assets()
        .get(definition_id)
        .map_or(0, |assets| u32::try_from(assets.len()).unwrap_or(u32::MAX));
    let zero_locked_quantity = iroha_primitives::numeric::Quantity::zero();
    let mut locked_quantity = None;
    let mut circulating_quantity = None;
    if definition_id == &app.state.gov.voting_asset_id {
        let locked = world
            .assets_iter()
            .find(|entry| {
                entry.id().definition() == definition_id
                    && entry.id().account() == &app.state.gov.bond_escrow_account
                    && entry.id().scope() == &iroha_data_model::asset::AssetBalanceScope::Global
            })
            .map_or(&zero_locked_quantity, |entry| entry.value.as_ref());
        let circulating = match definition.total_quantity.checked_sub(locked) {
            Ok(circulating) => circulating,
            Err(error) => {
                return error_response_with_format(
                    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                            "governance locked quantity exceeds total issuance: {error}"
                        )),
                    )),
                    ResponseFormat::Json,
                );
            }
        };
        locked_quantity = Some(locked);
        circulating_quantity = Some(circulating);
    }
    let source = ToriiExplorerAssetDefinitionJsonSource {
        definition,
        assets,
        locked_quantity,
        circulating_quantity: circulating_quantity.as_ref(),
    };
    let budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    budget
        .json_response(&source)
        .unwrap_or_else(|response| response)
}
fn torii_bounded_local_proof_record_payload(
    app: &SharedAppState,
    proof_id: &iroha_data_model::proof::ProofId,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<Option<ToriiBoundedNoritoPayload<ProofRecord>>, Response> {
    let state_view = app.state.query_view();
    let Some(record) = state_view.world().proofs().get(proof_id) else {
        return Ok(None);
    };
    torii_bounded_routed_read_source_payload::<ProofRecord, _>(record, budget).map(Some)
}
fn execute_torii_proof_record_local_source_read(
    app: &SharedAppState,
    proof_id: &iroha_data_model::proof::ProofId,
    format: ResponseFormat,
) -> Response {
    let mut budget = match torii_local_routed_read_budget(app) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    let payload = match torii_bounded_local_proof_record_payload(app, proof_id, &mut budget) {
        Ok(Some(payload)) => payload,
        Ok(None) => {
            return torii_proxy_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "the requested proof record was not found on this route",
            );
        }
        Err(response) => return response,
    };
    torii_bounded_routed_read_payload_response(payload, format, budget)
        .unwrap_or_else(|response| response)
}
#[cfg(test)]
include!("tests/lib_routed_reads/routed_read_source_bounds.rs");
