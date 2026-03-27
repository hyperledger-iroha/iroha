//! JSON envelope helpers for the query DSL.
//!
//! These structures mirror the payloads accepted by Torii query endpoints. The
//! helpers convert between Norito JSON values and strongly-typed query objects,
//! ensuring field ordering stays deterministic and validation errors surface
//! with precise context.

use std::{
    borrow::ToOwned,
    format,
    num::NonZeroU64,
    str::FromStr,
    string::{String, ToString},
};

use norito::json::{self, JsonDeserialize, JsonSerialize, Map, Value};
use thiserror::Error;

use crate::{
    name::Name,
    query::{
        QueryBox, QueryOutputBatchBox, QueryRequest, QueryRequestWithAuthority, QueryWithFilter,
        QueryWithParams, SingularQueryBox,
        dsl::{CompoundPredicate, HasProjection, PredicateMarker, SelectorMarker, SelectorTuple},
        json::predicate::{PredicateJson, PredicateParseError},
        parameters::{FetchSize, Pagination, QueryParams, Sorting},
    },
};

fn build_query_with_filter<T, F>(
    predicate: CompoundPredicate<T>,
    selector: SelectorTuple<T>,
    builder: F,
) -> QueryBox<QueryOutputBatchBox>
where
    T: HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + Send
        + Sync
        + 'static,
    F: FnOnce() -> Box<dyn crate::query::Query<Item = T> + Send + Sync + 'static>,
{
    #[cfg(not(feature = "fast_dsl"))]
    {
        let qwf = QueryWithFilter::new_with_query(builder(), predicate, selector);
        QueryBox::from(qwf)
    }
    #[cfg(feature = "fast_dsl")]
    {
        let _ = builder;
        let qwf = QueryWithFilter::new_with_query((), predicate, selector);
        QueryBox::from(qwf)
    }
}

fn build_query_with_params<T, F>(
    predicate: CompoundPredicate<T>,
    selector: SelectorTuple<T>,
    params: QueryParams,
    builder: F,
) -> QueryWithParams
where
    T: HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + Send
        + Sync
        + 'static,
    F: FnOnce() -> Box<dyn crate::query::Query<Item = T> + Send + Sync + 'static>,
{
    let query_box = build_query_with_filter::<T, _>(predicate, selector, builder);
    #[cfg(not(feature = "fast_dsl"))]
    {
        QueryWithParams::new(query_box, params)
    }
    #[cfg(feature = "fast_dsl")]
    {
        let result = QueryWithParams::new(&query_box, params);
        drop(query_box);
        result
    }
}

/// JSON envelope selecting either a singular or iterable query definition.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryEnvelopeJson {
    /// Encodes a singular query (single object response).
    Singular(SingularQueryJson),
    /// Encodes an iterable query (batched response stream).
    Iterable(Box<IterableQueryJson>),
}

impl Eq for QueryEnvelopeJson {}

impl QueryEnvelopeJson {
    fn parse_map(map: &Map) -> Result<Self, QueryJsonError> {
        let mut singular: Option<&Value> = None;
        let mut iterable: Option<&Value> = None;

        for (key, value) in map {
            match key.as_str() {
                "singular" => singular = Some(value),
                "iterable" => iterable = Some(value),
                other => return Err(QueryJsonError::UnknownEnvelopeKey(other.to_owned())),
            }
        }

        match (singular, iterable) {
            (Some(value), None) => {
                let sing = SingularQueryJson::from_value(value)?;
                Ok(QueryEnvelopeJson::Singular(sing))
            }
            (None, Some(value)) => {
                let iter = IterableQueryJson::from_value(value)?;
                Ok(QueryEnvelopeJson::Iterable(Box::new(iter)))
            }
            (Some(_), Some(_)) => Err(QueryJsonError::AmbiguousEnvelope),
            (None, None) => Err(QueryJsonError::EmptyEnvelope),
        }
    }
}

impl JsonSerialize for QueryEnvelopeJson {
    fn json_serialize(&self, out: &mut String) {
        let mut map = Map::new();
        match self {
            QueryEnvelopeJson::Singular(s) => {
                map.insert("singular".to_owned(), s.to_value());
            }
            QueryEnvelopeJson::Iterable(i) => {
                map.insert("iterable".to_owned(), i.to_value());
            }
        }
        Value::Object(map).json_serialize(out);
    }
}

impl JsonDeserialize for QueryEnvelopeJson {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        match value {
            Value::Object(map) => QueryEnvelopeJson::parse_map(&map)
                .map_err(|err| json::Error::Message(err.to_string())),
            other => Err(json::Error::Message(format!(
                "expected envelope object, found {}",
                value_type_name(&other)
            ))),
        }
    }
}

/// Supported singular queries for the JSON DSL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingularQueryJson {
    /// Returns the active ABI version exposed by the runtime.
    FindAbiVersion,
    /// Returns metadata describing the executor data model.
    FindExecutorDataModel,
    /// Returns the current blockchain parameter set.
    FindParameters,
    /// Lists domains linked to an account subject.
    FindDomainsByAccountId {
        /// Account identifier used to resolve the subject.
        account_id: String,
    },
    /// Lists aliases bound to an account subject.
    FindAliasesByAccountId {
        /// Account identifier used to resolve the subject.
        account_id: String,
        /// Optional dataspace alias filter such as `sbp`.
        dataspace: Option<String>,
        /// Optional exact domain filter such as `hbl`.
        domain: Option<String>,
    },
    /// Lists account identifiers linked to a domain.
    FindAccountIdsByDomainId {
        /// Domain identifier used to resolve linked subjects.
        domain_id: String,
    },
    /// Looks up an asset by identifier.
    FindAssetById {
        /// Asset definition address identifying the asset type.
        asset: String,
        /// Canonical I105 account identifier owning the asset.
        account_id: String,
        /// Optional balance scope selector.
        scope: Option<Value>,
    },
    /// Looks up an asset definition by identifier.
    FindAssetDefinitionById {
        /// Asset definition address identifying the asset type.
        asset: String,
    },
    /// Looks up a contract manifest by code hash.
    FindContractManifestByCodeHash {
        /// Hex-encoded 32-byte code hash identifying the contract.
        code_hash: String,
    },
    /// Looks up a twitter follow binding by keyed hash.
    FindTwitterBindingByHash {
        /// Keyed hash identifying the binding.
        binding_hash: crate::oracle::KeyedHash,
    },
}

impl SingularQueryJson {
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "type".to_owned(),
            Value::String(self.kind_label().to_owned()),
        );
        match self {
            Self::FindContractManifestByCodeHash { code_hash } => {
                let mut payload = Map::new();
                payload.insert("code_hash".to_owned(), Value::String(code_hash.clone()));
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            Self::FindAssetById {
                asset,
                account_id,
                scope,
            } => {
                let mut payload = Map::new();
                payload.insert("asset".to_owned(), Value::String(asset.clone()));
                payload.insert("account_id".to_owned(), Value::String(account_id.clone()));
                if let Some(scope) = scope {
                    payload.insert("scope".to_owned(), scope.clone());
                }
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            Self::FindAssetDefinitionById { asset } => {
                let mut payload = Map::new();
                payload.insert("asset".to_owned(), Value::String(asset.clone()));
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            Self::FindDomainsByAccountId { account_id } => {
                let mut payload = Map::new();
                payload.insert("account_id".to_owned(), Value::String(account_id.clone()));
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            Self::FindAliasesByAccountId {
                account_id,
                dataspace,
                domain,
            } => {
                let mut payload = Map::new();
                payload.insert("account_id".to_owned(), Value::String(account_id.clone()));
                if let Some(dataspace) = dataspace {
                    payload.insert("dataspace".to_owned(), Value::String(dataspace.clone()));
                }
                if let Some(domain) = domain {
                    payload.insert("domain".to_owned(), Value::String(domain.clone()));
                }
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            Self::FindAccountIdsByDomainId { domain_id } => {
                let mut payload = Map::new();
                payload.insert("domain_id".to_owned(), Value::String(domain_id.clone()));
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            Self::FindTwitterBindingByHash { binding_hash } => {
                let mut payload = Map::new();
                payload.insert(
                    "binding_hash".to_owned(),
                    json::to_value(binding_hash).expect("binding hash serializes"),
                );
                map.insert("payload".to_owned(), Value::Object(payload));
            }
            _ => {}
        }
        Value::Object(map)
    }

    fn from_value(value: &Value) -> Result<Self, QueryJsonError> {
        let map = value
            .as_object()
            .ok_or(QueryJsonError::ExpectedObject("singular"))?;
        let ty = map
            .get("type")
            .and_then(Value::as_str)
            .ok_or(QueryJsonError::MissingField("singular", "type"))?;
        match ty {
            "FindAbiVersion" => Ok(SingularQueryJson::FindAbiVersion),
            "FindExecutorDataModel" => Ok(SingularQueryJson::FindExecutorDataModel),
            "FindParameters" => Ok(SingularQueryJson::FindParameters),
            "FindDomainsByAccountId" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let account_id = payload
                    .get("account_id")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "account_id"))?;
                Ok(SingularQueryJson::FindDomainsByAccountId {
                    account_id: account_id.to_owned(),
                })
            }
            "FindAliasesByAccountId" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let account_id = payload
                    .get("account_id")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "account_id"))?;
                let dataspace = payload
                    .get("dataspace")
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or(QueryJsonError::InvalidField("payload", "dataspace"))
                            .map(str::to_owned)
                    })
                    .transpose()?;
                let domain = payload
                    .get("domain")
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or(QueryJsonError::InvalidField("payload", "domain"))
                            .map(str::to_owned)
                    })
                    .transpose()?;
                Ok(SingularQueryJson::FindAliasesByAccountId {
                    account_id: account_id.to_owned(),
                    dataspace,
                    domain,
                })
            }
            "FindAccountIdsByDomainId" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let domain_id = payload
                    .get("domain_id")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "domain_id"))?;
                Ok(SingularQueryJson::FindAccountIdsByDomainId {
                    domain_id: domain_id.to_owned(),
                })
            }
            "FindContractManifestByCodeHash" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let code_hash = payload
                    .get("code_hash")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "code_hash"))?;
                Ok(SingularQueryJson::FindContractManifestByCodeHash {
                    code_hash: code_hash.to_owned(),
                })
            }
            "FindAssetById" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let asset = payload
                    .get("asset")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "asset"))?;
                let account_id = payload
                    .get("account_id")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "account_id"))?;
                Ok(SingularQueryJson::FindAssetById {
                    asset: asset.to_owned(),
                    account_id: account_id.to_owned(),
                    scope: payload.get("scope").cloned(),
                })
            }
            "FindAssetDefinitionById" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let asset = payload
                    .get("asset")
                    .and_then(Value::as_str)
                    .ok_or(QueryJsonError::MissingField("payload", "asset"))?;
                Ok(SingularQueryJson::FindAssetDefinitionById {
                    asset: asset.to_owned(),
                })
            }
            "FindTwitterBindingByHash" => {
                let payload = map
                    .get("payload")
                    .and_then(Value::as_object)
                    .ok_or(QueryJsonError::MissingField("singular", "payload"))?;
                let binding_hash_value = payload
                    .get("binding_hash")
                    .ok_or(QueryJsonError::MissingField("payload", "binding_hash"))?;
                let binding_hash =
                    json::from_value::<crate::oracle::KeyedHash>(binding_hash_value.clone())
                        .map_err(|_| QueryJsonError::InvalidField("payload", "binding_hash"))?;
                Ok(SingularQueryJson::FindTwitterBindingByHash { binding_hash })
            }
            other => Err(QueryJsonError::UnknownSingularType(other.to_owned())),
        }
    }

    fn kind_label(&self) -> &'static str {
        match self {
            SingularQueryJson::FindAbiVersion => "FindAbiVersion",
            SingularQueryJson::FindExecutorDataModel => "FindExecutorDataModel",
            SingularQueryJson::FindParameters => "FindParameters",
            SingularQueryJson::FindDomainsByAccountId { .. } => "FindDomainsByAccountId",
            SingularQueryJson::FindAliasesByAccountId { .. } => "FindAliasesByAccountId",
            SingularQueryJson::FindAccountIdsByDomainId { .. } => "FindAccountIdsByDomainId",
            SingularQueryJson::FindAssetById { .. } => "FindAssetById",
            SingularQueryJson::FindAssetDefinitionById { .. } => "FindAssetDefinitionById",
            SingularQueryJson::FindContractManifestByCodeHash { .. } => {
                "FindContractManifestByCodeHash"
            }
            SingularQueryJson::FindTwitterBindingByHash { .. } => "FindTwitterBindingByHash",
        }
    }

    fn into_box(self) -> Result<SingularQueryBox, QueryJsonError> {
        match self {
            SingularQueryJson::FindAbiVersion => Ok(SingularQueryBox::FindAbiVersion(
                crate::query::runtime::prelude::FindAbiVersion,
            )),
            SingularQueryJson::FindExecutorDataModel => {
                Ok(SingularQueryBox::FindExecutorDataModel(
                    crate::query::executor::prelude::FindExecutorDataModel,
                ))
            }
            SingularQueryJson::FindParameters => Ok(SingularQueryBox::FindParameters(
                crate::query::executor::prelude::FindParameters,
            )),
            SingularQueryJson::FindDomainsByAccountId { account_id } => {
                let id = crate::account::AccountId::parse_encoded(&account_id)
                    .map(crate::account::ParsedAccountId::into_account_id)
                    .map_err(|_| QueryJsonError::InvalidField("payload", "account_id"))?;
                Ok(SingularQueryBox::FindDomainsByAccountId(
                    crate::query::account::prelude::FindDomainsByAccountId::new(id),
                ))
            }
            SingularQueryJson::FindAliasesByAccountId {
                account_id,
                dataspace,
                domain,
            } => {
                let id = crate::account::AccountId::parse_encoded(&account_id)
                    .map(crate::account::ParsedAccountId::into_account_id)
                    .map_err(|_| QueryJsonError::InvalidField("payload", "account_id"))?;
                Ok(SingularQueryBox::FindAliasesByAccountId(
                    crate::query::account::prelude::FindAliasesByAccountId::new(
                        id, dataspace, domain,
                    ),
                ))
            }
            SingularQueryJson::FindAccountIdsByDomainId { domain_id } => {
                let id = domain_id
                    .parse()
                    .map_err(|_| QueryJsonError::InvalidField("payload", "domain_id"))?;
                Ok(SingularQueryBox::FindAccountIdsByDomainId(
                    crate::query::domain::prelude::FindAccountIdsByDomainId::new(id),
                ))
            }
            SingularQueryJson::FindAssetById {
                asset,
                account_id,
                scope,
            } => {
                let definition = crate::asset::AssetDefinitionId::parse_address_literal(&asset)
                    .map_err(|_| QueryJsonError::InvalidField("payload", "asset"))?;
                let account = crate::account::AccountId::parse_encoded(&account_id)
                    .map(crate::account::ParsedAccountId::into_account_id)
                    .map_err(|_| QueryJsonError::InvalidField("payload", "account_id"))?;
                let scope = scope
                    .map(|value| {
                        json::from_value::<crate::asset::AssetBalanceScope>(value)
                            .map_err(|_| QueryJsonError::InvalidField("payload", "scope"))
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok(SingularQueryBox::FindAssetById(
                    crate::query::asset::prelude::FindAssetById::new(
                        crate::asset::AssetId::with_scope(definition, account, scope),
                    ),
                ))
            }
            SingularQueryJson::FindAssetDefinitionById { asset } => {
                let id = crate::asset::AssetDefinitionId::parse_address_literal(&asset)
                    .map_err(|_| QueryJsonError::InvalidField("payload", "asset"))?;
                Ok(SingularQueryBox::FindAssetDefinitionById(
                    crate::query::asset::prelude::FindAssetDefinitionById::new(id),
                ))
            }
            SingularQueryJson::FindContractManifestByCodeHash { code_hash } => {
                let bytes = hex::decode(code_hash.trim_start_matches("0x"))
                    .map_err(|_| QueryJsonError::InvalidHex("code_hash".to_owned()))?;
                if bytes.len() != 32 {
                    return Err(QueryJsonError::InvalidLength {
                        field: "code_hash".to_owned(),
                        expected: 32,
                        actual: bytes.len(),
                    });
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                let hash = iroha_crypto::Hash::prehashed(arr);
                Ok(SingularQueryBox::FindContractManifestByCodeHash(
                    crate::query::smart_contract::prelude::FindContractManifestByCodeHash {
                        code_hash: hash,
                    },
                ))
            }
            SingularQueryJson::FindTwitterBindingByHash { binding_hash } => {
                Ok(SingularQueryBox::FindTwitterBindingByHash(
                    crate::query::oracle::prelude::FindTwitterBindingByHash { binding_hash },
                ))
            }
        }
    }
}

/// Supported iterable queries for the JSON DSL.
#[derive(Debug, Clone, PartialEq)]
pub struct IterableQueryJson {
    /// Query selector describing which dataset to enumerate.
    pub kind: IterableQueryKind,
    /// Optional pagination, sorting, and projection controls.
    pub params: IterableQueryParamsJson,
    /// Optional predicate evaluated against each item.
    pub predicate: Option<PredicateJson>,
}

impl Eq for IterableQueryJson {}

impl IterableQueryJson {
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "type".to_owned(),
            Value::String(self.kind.as_str().to_owned()),
        );
        if !self.params.is_empty() {
            map.insert("params".to_owned(), self.params.to_value());
        }
        if let Some(pred) = &self.predicate {
            map.insert(
                "predicate".to_owned(),
                json::to_value(pred).expect("predicate serialize"),
            );
        }
        Value::Object(map)
    }

    fn from_value(value: &Value) -> Result<Self, QueryJsonError> {
        let map = value
            .as_object()
            .ok_or(QueryJsonError::ExpectedObject("iterable"))?;
        let mut kind: Option<IterableQueryKind> = None;
        let mut params: Option<IterableQueryParamsJson> = None;
        let mut predicate: Option<PredicateJson> = None;

        for (key, value) in map {
            match key.as_str() {
                "type" => {
                    let raw = value
                        .as_str()
                        .ok_or(QueryJsonError::InvalidField("iterable", "type"))?;
                    let parsed = IterableQueryKind::from_str(raw)
                        .map_err(|()| QueryJsonError::UnknownIterableType(raw.to_owned()))?;
                    kind = Some(parsed);
                }
                "params" => {
                    params = Some(IterableQueryParamsJson::from_value(value)?);
                }
                "predicate" => {
                    predicate =
                        Some(PredicateJson::try_from_value(value).map_err(QueryJsonError::from)?);
                }
                other => {
                    return Err(QueryJsonError::UnknownField {
                        section: "iterable",
                        field: other.to_owned(),
                    });
                }
            }
        }

        let kind = kind.ok_or(QueryJsonError::MissingField("iterable", "type"))?;
        let params = params.unwrap_or_default();
        Ok(IterableQueryJson {
            kind,
            params,
            predicate,
        })
    }

    fn predicate_or_pass<T>(&self) -> Result<CompoundPredicate<T>, QueryJsonError> {
        self.predicate.as_ref().map_or_else(
            || Ok(CompoundPredicate::PASS),
            |pred| {
                pred.into_compound::<T>()
                    .map_err(|err| QueryJsonError::PredicateEncode(err.to_string()))
            },
        )
    }

    fn selector<T>(&self) -> Result<SelectorTuple<T>, QueryJsonError> {
        #[cfg(not(feature = "ids_projection"))]
        {
            if self.params.ids_projection.unwrap_or(false) {
                Err(QueryJsonError::IdsProjectionUnavailable)
            } else {
                Ok(SelectorTuple::default())
            }
        }
        #[cfg(feature = "ids_projection")]
        {
            if self.params.ids_projection.unwrap_or(false) {
                Ok(SelectorTuple::ids_only())
            } else {
                Ok(SelectorTuple::default())
            }
        }
    }

    fn build_for_kind<Item, F>(
        &self,
        params: QueryParams,
        constructor: F,
    ) -> Result<QueryWithParams, QueryJsonError>
    where
        Item: HasProjection<PredicateMarker>
            + HasProjection<SelectorMarker, AtomType = ()>
            + Send
            + Sync
            + 'static,
        F: FnOnce() -> Box<dyn crate::query::Query<Item = Item> + Send + Sync + 'static>,
    {
        let predicate = self.predicate_or_pass::<Item>()?;
        let selector = self.selector::<Item>()?;
        Ok(build_query_with_params::<Item, _>(
            predicate,
            selector,
            params,
            constructor,
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn into_query_with_params(self) -> Result<QueryWithParams, QueryJsonError> {
        let params = self.params.clone().into_query_params()?;
        match self.kind {
            IterableQueryKind::FindPeers => {
                type Item = crate::peer::PeerId;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::peer::prelude::FindPeers)
                })
            }
            IterableQueryKind::FindDomains => {
                type Item = crate::domain::Domain;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::domain::prelude::FindDomains)
                })
            }
            IterableQueryKind::FindAccounts => {
                type Item = crate::account::Account;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::account::prelude::FindAccounts)
                })
            }
            IterableQueryKind::FindAccountIds => {
                type Item = crate::account::AccountId;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::account::prelude::FindAccountIds)
                })
            }
            IterableQueryKind::FindAssetsDefinitions => {
                type Item = crate::asset::definition::AssetDefinition;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::asset::prelude::FindAssetsDefinitions)
                })
            }
            IterableQueryKind::FindRepoAgreements => {
                type Item = crate::repo::RepoAgreement;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::repo::prelude::FindRepoAgreements)
                })
            }
            IterableQueryKind::FindNfts => {
                type Item = crate::nft::Nft;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::nft::prelude::FindNfts)
                })
            }
            IterableQueryKind::FindRwas => {
                type Item = crate::rwa::Rwa;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::rwa::prelude::FindRwas)
                })
            }
            IterableQueryKind::FindRoles => {
                type Item = crate::role::Role;
                self.build_for_kind::<Item, _>(params.clone(), || {
                    Box::new(crate::query::role::prelude::FindRoles)
                })
            }
            IterableQueryKind::FindRoleIds => {
                type Item = crate::role::RoleId;
                self.build_for_kind::<Item, _>(params, || {
                    Box::new(crate::query::role::prelude::FindRoleIds)
                })
            }
        }
    }
}

/// Iterable query kinds understood by the JSON DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterableQueryKind {
    /// Enumerate registered peers.
    FindPeers,
    /// Enumerate domain records.
    FindDomains,
    /// Enumerate account records.
    FindAccounts,
    /// Enumerate account identifiers only.
    FindAccountIds,
    /// Enumerate asset definitions.
    FindAssetsDefinitions,
    /// Enumerate registered NFTs.
    FindNfts,
    /// Enumerate registered RWA lots.
    FindRwas,
    /// Enumerate defined roles.
    FindRoles,
    /// Enumerate role identifiers only.
    FindRoleIds,
    /// Enumerate repo agreements.
    FindRepoAgreements,
}

impl IterableQueryKind {
    fn as_str(self) -> &'static str {
        match self {
            IterableQueryKind::FindPeers => "FindPeers",
            IterableQueryKind::FindDomains => "FindDomains",
            IterableQueryKind::FindAccounts => "FindAccounts",
            IterableQueryKind::FindAccountIds => "FindAccountIds",
            IterableQueryKind::FindAssetsDefinitions => "FindAssetsDefinitions",
            IterableQueryKind::FindNfts => "FindNfts",
            IterableQueryKind::FindRwas => "FindRwas",
            IterableQueryKind::FindRoles => "FindRoles",
            IterableQueryKind::FindRoleIds => "FindRoleIds",
            IterableQueryKind::FindRepoAgreements => "FindRepoAgreements",
        }
    }
}

impl FromStr for IterableQueryKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "FindPeers" => Ok(IterableQueryKind::FindPeers),
            "FindDomains" => Ok(IterableQueryKind::FindDomains),
            "FindAccounts" => Ok(IterableQueryKind::FindAccounts),
            "FindAccountIds" => Ok(IterableQueryKind::FindAccountIds),
            "FindAssetsDefinitions" => Ok(IterableQueryKind::FindAssetsDefinitions),
            "FindNfts" => Ok(IterableQueryKind::FindNfts),
            "FindRwas" => Ok(IterableQueryKind::FindRwas),
            "FindRoles" => Ok(IterableQueryKind::FindRoles),
            "FindRoleIds" => Ok(IterableQueryKind::FindRoleIds),
            "FindRepoAgreements" => Ok(IterableQueryKind::FindRepoAgreements),
            _ => Err(()),
        }
    }
}

/// Execution modifiers for iterable queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IterableQueryParamsJson {
    /// Maximum number of records to return.
    pub limit: Option<u64>,
    /// Offset inside the full result set.
    pub offset: Option<u64>,
    /// Fetch-size hint used by the query executor.
    pub fetch_size: Option<u64>,
    /// Metadata key used for sorting (optional).
    pub sort_by_metadata_key: Option<String>,
    /// Sort direction applied when a metadata key is provided.
    pub order: Option<crate::query::parameters::SortOrder>,
    /// Whether to project identifiers only (requires the `ids_projection` feature).
    pub ids_projection: Option<bool>,
    /// Optional lane identifier filter.
    pub lane_id: Option<u64>,
    /// Optional data-space identifier filter.
    pub dsid: Option<String>,
}

impl IterableQueryParamsJson {
    fn is_empty(&self) -> bool {
        self.limit.is_none()
            && self.offset.is_none()
            && self.fetch_size.is_none()
            && self.sort_by_metadata_key.is_none()
            && self.order.is_none()
            && self.ids_projection.is_none()
            && self.lane_id.is_none()
            && self.dsid.is_none()
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        if let Some(limit) = self.limit {
            map.insert("limit".to_owned(), Value::from(limit));
        }
        if let Some(offset) = self.offset {
            map.insert("offset".to_owned(), Value::from(offset));
        }
        if let Some(fetch_size) = self.fetch_size {
            map.insert("fetch_size".to_owned(), Value::from(fetch_size));
        }
        if let Some(key) = &self.sort_by_metadata_key {
            map.insert(
                "sort_by_metadata_key".to_owned(),
                Value::String(key.clone()),
            );
        }
        if let Some(order) = &self.order {
            map.insert(
                "order".to_owned(),
                json::to_value(order).expect("SortOrder serializes"),
            );
        }
        if let Some(ids) = self.ids_projection {
            map.insert("ids_projection".to_owned(), Value::from(ids));
        }
        if let Some(lane) = self.lane_id {
            map.insert("lane_id".to_owned(), Value::from(lane));
        }
        if let Some(dsid) = &self.dsid {
            map.insert("dsid".to_owned(), Value::String(dsid.clone()));
        }
        Value::Object(map)
    }

    fn from_value(value: &Value) -> Result<Self, QueryJsonError> {
        let map = value
            .as_object()
            .ok_or(QueryJsonError::ExpectedObject("params"))?;
        let mut limit = None;
        let mut offset = None;
        let mut fetch_size = None;
        let mut sort_by_metadata_key = None;
        let mut order = None;
        let mut ids_projection = None;
        let mut lane_id = None;
        let mut dsid = None;

        for (key, value) in map {
            match key.as_str() {
                "limit" => limit = value.as_u64(),
                "offset" => offset = value.as_u64(),
                "fetch_size" => fetch_size = value.as_u64(),
                "sort_by_metadata_key" => {
                    sort_by_metadata_key = value.as_str().map(ToOwned::to_owned);
                }
                "order" => {
                    order = Some(match value.as_str() {
                        Some("Asc") => crate::query::parameters::SortOrder::Asc,
                        Some("Desc") => crate::query::parameters::SortOrder::Desc,
                        Some(other) => {
                            return Err(QueryJsonError::InvalidSortOrder(other.to_owned()));
                        }
                        None => {
                            return Err(QueryJsonError::InvalidSortOrder(
                                value_type_name(value).to_owned(),
                            ));
                        }
                    });
                }
                "ids_projection" => ids_projection = value.as_bool(),
                "lane_id" => lane_id = value.as_u64(),
                "dsid" => dsid = value.as_str().map(ToOwned::to_owned),
                other => {
                    return Err(QueryJsonError::UnknownField {
                        section: "params",
                        field: other.to_owned(),
                    });
                }
            }
        }

        Ok(Self {
            limit,
            offset,
            fetch_size,
            sort_by_metadata_key,
            order,
            ids_projection,
            lane_id,
            dsid,
        })
    }

    fn into_query_params(self) -> Result<QueryParams, QueryJsonError> {
        let Self {
            limit,
            offset,
            fetch_size,
            sort_by_metadata_key,
            order,
            ids_projection: _,
            lane_id: _,
            dsid: _,
        } = self;

        let limit = limit
            .map(|value| {
                NonZeroU64::new(value).ok_or(QueryJsonError::InvalidNonZero("limit", value))
            })
            .transpose()?;
        let offset = offset.unwrap_or(0);
        let pagination = Pagination::new(limit, offset);

        let fetch_size = fetch_size
            .map(|value| {
                NonZeroU64::new(value).ok_or(QueryJsonError::InvalidNonZero("fetch_size", value))
            })
            .transpose()?;

        if order.is_some() && sort_by_metadata_key.is_none() {
            Err(QueryJsonError::OrderWithoutSortKey)
        } else {
            let mut sorting = if let Some(key) = sort_by_metadata_key {
                let name = Name::from_str(&key)
                    .map_err(|_| QueryJsonError::InvalidMetadataKey(key.clone()))?;
                Sorting::by_metadata_key(name)
            } else {
                Sorting::default()
            };
            sorting.order = order;

            let fetch_size = FetchSize::new(fetch_size);
            Ok(QueryParams::new(pagination, sorting, fetch_size))
        }
    }
}

impl QueryEnvelopeJson {
    /// Convert the envelope into a [`QueryRequest`] ready for signing with the
    /// provided authority.
    ///
    /// # Errors
    /// Returns an error when the envelope payload cannot be converted into a query.
    pub fn into_signed_request(
        self,
        authority: crate::account::AccountId,
    ) -> Result<QueryRequestWithAuthority, QueryJsonError> {
        let request = match self {
            QueryEnvelopeJson::Singular(s) => QueryRequest::Singular(s.into_box()?),
            QueryEnvelopeJson::Iterable(i) => {
                let iterable = *i;
                QueryRequest::Start(iterable.into_query_with_params()?)
            }
        };
        Ok(QueryRequestWithAuthority { authority, request })
    }
}

/// Errors produced when parsing or converting JSON query envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueryJsonError {
    /// query envelope must contain either `singular` or `iterable`
    #[error("query envelope must contain either `singular` or `iterable`")]
    EmptyEnvelope,
    /// `singular` and `iterable` sections cannot be combined
    #[error("`singular` and `iterable` sections cannot be combined")]
    AmbiguousEnvelope,
    /// unknown envelope key `{0}`
    #[error("unknown envelope key `{0}`")]
    UnknownEnvelopeKey(String),
    /// expected object for `{0}` section
    #[error("expected object for `{0}` section")]
    ExpectedObject(&'static str),
    /// missing `{1}` field inside `{0}`
    #[error("missing `{1}` field inside `{0}`")]
    MissingField(&'static str, &'static str),
    /// unknown singular query `{0}`
    #[error("unknown singular query `{0}`")]
    UnknownSingularType(String),
    /// unknown iterable query `{0}`
    #[error("unknown iterable query `{0}`")]
    UnknownIterableType(String),
    /// unknown field `{field}` inside `{section}`
    #[error("unknown field `{field}` inside `{section}`")]
    UnknownField {
        /// Section containing the unknown field.
        section: &'static str,
        /// Name of the offending field.
        field: String,
    },
    /// invalid field `{1}` inside `{0}`
    #[error("invalid field `{1}` inside `{0}`")]
    InvalidField(&'static str, &'static str),
    /// invalid hexadecimal value for `{0}`
    #[error("invalid hexadecimal value for `{0}`")]
    InvalidHex(String),
    /// `{field}` must contain {expected} bytes (got {actual})
    #[error("`{field}` must contain {expected} bytes (got {actual})")]
    InvalidLength {
        /// Name of the offending field.
        field: String,
        /// Expected byte length for the field.
        expected: usize,
        /// Actual byte length that was provided.
        actual: usize,
    },
    /// predicate JSON parse error: {0}
    #[error("predicate JSON parse error: {0}")]
    PredicateParse(String),
    /// predicate encoding error: {0}
    #[error("predicate encoding error: {0}")]
    PredicateEncode(String),
    /// `{0}` must be non-zero (got {1})
    #[error("`{0}` must be non-zero (got {1})")]
    InvalidNonZero(&'static str, u64),
    /// sort order requires `sort_by_metadata_key`
    #[error("sort order requires `sort_by_metadata_key`")]
    OrderWithoutSortKey,
    /// invalid metadata key `{0}`
    #[error("invalid metadata key `{0}`")]
    InvalidMetadataKey(String),
    /// invalid sort order: {0}
    #[error("invalid sort order: {0}")]
    InvalidSortOrder(String),
    /// ids projection requested but feature is disabled
    #[error("ids projection requested but feature is disabled")]
    IdsProjectionUnavailable,
}

impl From<PredicateParseError> for QueryJsonError {
    fn from(err: PredicateParseError) -> Self {
        QueryJsonError::PredicateParse(err.to_string())
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

impl IterableQueryJson {
    /// Build a [`QueryRequestWithAuthority`] from this JSON definition.
    ///
    /// # Errors
    /// Returns an error when pagination or predicate parameters are invalid.
    pub fn into_request(
        self,
        authority: crate::account::AccountId,
    ) -> Result<QueryRequestWithAuthority, QueryJsonError> {
        let request = QueryRequest::Start(self.into_query_with_params()?);
        Ok(QueryRequestWithAuthority { authority, request })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[test]
    fn singular_roundtrip() {
        let singular = SingularQueryJson::FindParameters;
        let envelope = QueryEnvelopeJson::Singular(singular.clone());
        let json = norito::json::to_json(&envelope).expect("serialize");
        let parsed: QueryEnvelopeJson = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, envelope);
        let request = match parsed {
            QueryEnvelopeJson::Singular(s) => s.into_box().expect("into box"),
            _ => unreachable!(),
        };
        assert!(matches!(
            request,
            SingularQueryBox::FindParameters(crate::query::executor::prelude::FindParameters)
        ));
    }

    #[test]
    fn twitter_binding_query_roundtrip() {
        let binding_hash = crate::oracle::KeyedHash::new("pepper-social-v1", b"pepper", b"payload");
        let singular = SingularQueryJson::FindTwitterBindingByHash {
            binding_hash: binding_hash.clone(),
        };
        let envelope = QueryEnvelopeJson::Singular(singular.clone());
        let json = norito::json::to_json(&envelope).expect("serialize");
        let parsed: QueryEnvelopeJson = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, envelope);
        let request = match parsed {
            QueryEnvelopeJson::Singular(s) => s.into_box().expect("into box"),
            _ => unreachable!(),
        };
        assert!(matches!(
            request,
            SingularQueryBox::FindTwitterBindingByHash(query)
                if query.binding_hash == binding_hash
        ));
    }

    #[test]
    fn find_domains_by_account_id_accepts_canonical_i105_literal() {
        let _domain: crate::domain::DomainId = "wonderland".parse().expect("valid domain id");
        let keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let account_id = crate::account::AccountId::new(keypair.public_key().clone());

        let singular = SingularQueryJson::FindDomainsByAccountId {
            account_id: account_id.to_string(),
        };
        let query = singular.into_box().expect("query conversion succeeds");
        let parsed = match query {
            SingularQueryBox::FindDomainsByAccountId(query) => query.account_id().clone(),
            other => panic!("unexpected query variant: {other:?}"),
        };
        assert_eq!(parsed.controller(), account_id.controller());
        assert_eq!(
            parsed
                .canonical_i105()
                .expect("parsed account id should emit canonical literal"),
            account_id
                .canonical_i105()
                .expect("source account id should emit canonical literal")
        );
    }

    #[test]
    fn find_aliases_by_account_id_roundtrip_with_filters() {
        let keypair = KeyPair::from_seed(vec![0xAC; 32], Algorithm::Ed25519);
        let account_id = crate::account::AccountId::new(keypair.public_key().clone());
        let singular = SingularQueryJson::FindAliasesByAccountId {
            account_id: account_id.to_string(),
            dataspace: Some("sbp".to_owned()),
            domain: Some("hbl".to_owned()),
        };
        let envelope = QueryEnvelopeJson::Singular(singular.clone());
        let json = norito::json::to_json(&envelope).expect("serialize");
        let parsed: QueryEnvelopeJson = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, envelope);
        let query = match parsed {
            QueryEnvelopeJson::Singular(s) => s.into_box().expect("into box"),
            _ => unreachable!(),
        };
        match query {
            SingularQueryBox::FindAliasesByAccountId(q) => {
                assert_eq!(q.account_id(), &account_id);
                assert_eq!(q.dataspace(), Some("sbp"));
                assert_eq!(q.domain(), Some("hbl"));
            }
            other => panic!("unexpected query variant: {other:?}"),
        }
    }

    #[test]
    fn find_account_ids_by_domain_id_roundtrip() {
        let domain_id: crate::domain::DomainId = "wonderland".parse().expect("valid domain id");
        let singular = SingularQueryJson::FindAccountIdsByDomainId {
            domain_id: domain_id.to_string(),
        };
        let envelope = QueryEnvelopeJson::Singular(singular.clone());
        let json = norito::json::to_json(&envelope).expect("serialize");
        let parsed: QueryEnvelopeJson = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, envelope);

        let query = match parsed {
            QueryEnvelopeJson::Singular(s) => s.into_box().expect("into box"),
            _ => unreachable!(),
        };
        match query {
            SingularQueryBox::FindAccountIdsByDomainId(q) => {
                assert_eq!(q.domain_id(), &domain_id);
            }
            other => panic!("unexpected query variant: {other:?}"),
        }
    }

    #[test]
    fn find_asset_queries_roundtrip_with_public_selectors() {
        let definition_id = crate::asset::AssetDefinitionId::new(
            "wonderland"
                .parse::<crate::domain::DomainId>()
                .expect("valid domain id"),
            "rose".parse::<crate::Name>().expect("valid asset name"),
        );
        let keypair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account_id = crate::account::AccountId::new(keypair.public_key().clone());

        let singular = SingularQueryJson::FindAssetById {
            asset: definition_id.to_string(),
            account_id: account_id.to_string(),
            scope: Some(norito::json!({ "kind": "Global" })),
        };
        let envelope = QueryEnvelopeJson::Singular(singular.clone());
        let json = norito::json::to_json(&envelope).expect("serialize");
        let parsed: QueryEnvelopeJson = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, envelope);

        let query = match parsed {
            QueryEnvelopeJson::Singular(s) => s.into_box().expect("into box"),
            _ => unreachable!(),
        };
        match query {
            SingularQueryBox::FindAssetById(q) => {
                assert_eq!(q.asset_id().definition(), &definition_id);
                assert_eq!(q.asset_id().account(), &account_id);
                assert_eq!(
                    q.asset_id().scope(),
                    &crate::asset::AssetBalanceScope::Global
                );
            }
            other => panic!("unexpected query variant: {other:?}"),
        }

        let definition_query = SingularQueryJson::FindAssetDefinitionById {
            asset: definition_id.to_string(),
        };
        let query = definition_query
            .into_box()
            .expect("query conversion succeeds");
        match query {
            SingularQueryBox::FindAssetDefinitionById(q) => {
                assert_eq!(q.asset_definition_id(), &definition_id);
            }
            other => panic!("unexpected query variant: {other:?}"),
        }

        let invalid = SingularQueryJson::FindAssetDefinitionById {
            asset: "prefix:2f17c72466f84a4bb8a8e24884fdcd2f".to_owned(),
        };
        let err = invalid
            .into_box()
            .expect_err("prefixed literal must be rejected");
        assert_eq!(err, QueryJsonError::InvalidField("payload", "asset"));
    }

    #[test]
    fn iterable_params_validation() {
        let value = norito::json!({
            "type": "FindDomains",
            "params": {
                "limit": 10,
                "offset": 5,
                "fetch_size": 50,
                "sort_by_metadata_key": "ui.order",
                "order": "Desc"
            }
        });
        let parsed = IterableQueryJson::from_value(&value).expect("parse");
        let params = parsed
            .clone()
            .params
            .into_query_params()
            .expect("build params");
        assert_eq!(params.pagination.offset_value(), 5);
        assert_eq!(
            params
                .pagination
                .limit_value()
                .map(NonZeroU64::get)
                .unwrap(),
            10
        );
        assert_eq!(
            params
                .sorting
                .sort_by_metadata_key
                .as_ref()
                .map(ToString::to_string)
                .unwrap(),
            "ui.order"
        );
        assert_eq!(
            params.fetch_size.fetch_size.map(NonZeroU64::get).unwrap(),
            50
        );
    }

    #[test]
    fn iterable_query_unknown_field_rejected() {
        let value = norito::json!({
            "type": "FindDomains",
            "extra": true
        });
        let err = IterableQueryJson::from_value(&value).expect_err("should reject unknown key");
        assert_eq!(
            err,
            QueryJsonError::UnknownField {
                section: "iterable",
                field: "extra".to_owned(),
            }
        );
    }

    #[test]
    fn iterable_params_unknown_field_rejected() {
        let value = norito::json!({
            "limit": 1,
            "foo": "bar"
        });
        let err =
            IterableQueryParamsJson::from_value(&value).expect_err("should reject unknown key");
        assert_eq!(
            err,
            QueryJsonError::UnknownField {
                section: "params",
                field: "foo".to_owned(),
            }
        );
    }

    #[test]
    fn ids_projection_requires_feature() {
        let json = norito::json!({
            "type": "FindPeers",
            "params": {
                "ids_projection": true
            }
        });
        let parsed = IterableQueryJson::from_value(&json).expect("parse");
        #[cfg(not(feature = "ids_projection"))]
        assert!(matches!(
            parsed.clone().into_query_with_params(),
            Err(QueryJsonError::IdsProjectionUnavailable)
        ));
        #[cfg(feature = "ids_projection")]
        assert!(parsed.into_query_with_params().is_ok());
    }
}
