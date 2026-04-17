//! Torii HTTP helpers for the Musubi Kotodama package registry.
//!
//! The read endpoints execute the same on-chain queries used by the Musubi CLI.
//! The instruction endpoints are intentionally pre-signing helpers only: Torii
//! returns deterministic instruction payloads and never accepts private keys.

use std::{fmt, str::FromStr};

use axum::extract::{Path, State};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_core::smartcontracts::ValidSingularQuery;
use iroha_data_model::{
    ValidationFail,
    isi::{
        InstructionBox,
        musubi::{
            AssertMusubiReleaseExists, PublishMusubiRelease, SetMusubiShortAlias, YankMusubiRelease,
        },
    },
    musubi::{
        MusubiPackageId, MusubiPackageRef, MusubiPackageSummary, MusubiRelease,
        MusubiReleaseSummary, MusubiShortAlias, MusubiVersion,
    },
    name::Name,
    query::{SingularQuery, error::QueryExecutionFail, musubi::prelude::*},
};
use norito::json::{self, JsonSerialize, Map, Value};

use crate::{Error, JsonBody, NoritoJson, NoritoQuery, Result, SharedAppState};

const DEFAULT_SEARCH_LIMIT: u32 = 20;
const MUSUBI_SEARCH_LIMIT_CAP: u32 = 1_000;

/// Query parameters for `GET /v1/musubi/packages`.
#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
pub struct MusubiPackageSearchParams {
    /// Case-sensitive substring query over `namespace/package`.
    #[norito(default)]
    pub query: Option<String>,
    /// Optional namespace filter.
    #[norito(default)]
    pub namespace: Option<String>,
    /// Include packages with only yanked releases.
    #[norito(default)]
    pub include_yanked: Option<bool>,
    /// Deterministic offset into the sorted result set.
    #[norito(default)]
    pub offset: Option<u32>,
    /// Maximum number of package summaries to return, capped by Torii.
    #[norito(default)]
    pub limit: Option<u32>,
}

/// Query parameters carrying a package id.
#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
pub struct MusubiPackageQueryParams {
    /// Package id in `namespace/name` form.
    #[norito(default)]
    pub package: Option<String>,
    /// Include yanked releases where supported.
    #[norito(default)]
    pub include_yanked: Option<bool>,
}

/// Query parameters carrying an exact package release reference.
#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
pub struct MusubiReleaseQueryParams {
    /// Release reference in `namespace/name@version` form.
    #[norito(default)]
    pub package: Option<String>,
}

/// Request body for publish-release instruction construction.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct PublishMusubiReleaseInstructionRequest {
    /// Complete release record to publish.
    pub release: MusubiRelease,
}

/// Request body for yank-release instruction construction.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct YankMusubiReleaseInstructionRequest {
    /// Exact release reference in `namespace/name@version` form.
    pub package: String,
    /// Human-readable yank reason.
    pub reason: String,
}

/// Request body for short-alias instruction construction.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct SetMusubiAliasInstructionRequest {
    /// Curated short alias without a namespace prefix.
    pub alias: String,
    /// Target package id in `namespace/name` form.
    pub target: String,
}

/// Request body for release-existence assertion instruction construction.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct AssertMusubiReleaseExistsInstructionRequest {
    /// Package id in `namespace/name` form.
    pub package: String,
    /// Exact semantic version.
    pub version: String,
}

/// Deterministic unsigned instruction payload returned for local signing.
#[derive(Debug, Clone, crate::json_macros::JsonSerialize)]
pub struct MusubiInstructionEnvelopeDto {
    /// Stable instruction wire identifier.
    pub wire_id: String,
    /// Base64-encoded Norito-framed [`InstructionBox`].
    pub instruction_base64: String,
    /// Hex-encoded Norito-framed [`InstructionBox`].
    pub instruction_hex: String,
    /// Human-readable JSON preview of the instruction payload.
    pub instruction_json: Value,
}

/// HTTP handler for `GET /v1/musubi/packages`.
pub async fn handler_search_packages(
    State(app): State<SharedAppState>,
    NoritoQuery(params): NoritoQuery<MusubiPackageSearchParams>,
) -> Result<JsonBody<Vec<MusubiPackageSummary>>> {
    let query = SearchMusubiPackages {
        namespace: params
            .namespace
            .as_deref()
            .map(parse_literal::<iroha_data_model::musubi::MusubiNamespace>)
            .transpose()?,
        query: params.query.unwrap_or_default(),
        include_yanked: params.include_yanked.unwrap_or(false),
        offset: params.offset.unwrap_or(0),
        limit: params
            .limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .min(MUSUBI_SEARCH_LIMIT_CAP),
    };
    Ok(JsonBody(execute_query(&app, query)?))
}

/// HTTP handler for `GET /v1/musubi/release`.
pub async fn handler_get_release(
    State(app): State<SharedAppState>,
    NoritoQuery(params): NoritoQuery<MusubiReleaseQueryParams>,
) -> Result<JsonBody<MusubiRelease>> {
    let package = required_param(params.package, "package")?;
    let query = FindMusubiReleaseByRef {
        package: parse_literal::<MusubiPackageRef>(&package)?,
    };
    Ok(JsonBody(execute_query(&app, query)?))
}

/// HTTP handler for `GET /v1/musubi/releases`.
pub async fn handler_list_releases(
    State(app): State<SharedAppState>,
    NoritoQuery(params): NoritoQuery<MusubiPackageQueryParams>,
) -> Result<JsonBody<Vec<MusubiReleaseSummary>>> {
    let package = required_param(params.package, "package")?;
    let query = FindMusubiPackageReleases {
        package: parse_literal::<MusubiPackageId>(&package)?,
        include_yanked: params.include_yanked.unwrap_or(false),
    };
    Ok(JsonBody(execute_query(&app, query)?))
}

/// HTTP handler for `GET /v1/musubi/versions`.
pub async fn handler_list_versions(
    State(app): State<SharedAppState>,
    NoritoQuery(params): NoritoQuery<MusubiPackageQueryParams>,
) -> Result<JsonBody<Vec<MusubiVersion>>> {
    let package = required_param(params.package, "package")?;
    let query = FindMusubiPackageVersions {
        package: parse_literal::<MusubiPackageId>(&package)?,
    };
    Ok(JsonBody(execute_query(&app, query)?))
}

/// HTTP handler for `GET /v1/musubi/aliases/{alias}`.
pub async fn handler_resolve_alias(
    State(app): State<SharedAppState>,
    Path(alias): Path<String>,
) -> Result<JsonBody<MusubiPackageId>> {
    let query = FindMusubiShortAliasByName {
        alias: parse_literal::<Name>(&alias)?,
    };
    Ok(JsonBody(execute_query(&app, query)?))
}

/// HTTP handler for `POST /v1/musubi/instructions/publish-release`.
pub async fn handler_build_publish_release_instruction(
    NoritoJson(request): NoritoJson<PublishMusubiReleaseInstructionRequest>,
) -> Result<JsonBody<MusubiInstructionEnvelopeDto>> {
    request.release.validate_publishable().map_err(|err| {
        crate::routing::conversion_error(format!("invalid Musubi release: {err}"))
    })?;
    let release_json = to_json_value(&request.release, "musubi publish release preview")?;
    let instruction = PublishMusubiRelease::new(request.release);
    let mut payload = Map::new();
    payload.insert("release".to_owned(), release_json);
    Ok(JsonBody(build_instruction_envelope(
        PublishMusubiRelease::WIRE_ID,
        instruction,
        payload,
    )?))
}

/// HTTP handler for `POST /v1/musubi/instructions/yank-release`.
pub async fn handler_build_yank_release_instruction(
    NoritoJson(request): NoritoJson<YankMusubiReleaseInstructionRequest>,
) -> Result<JsonBody<MusubiInstructionEnvelopeDto>> {
    let package = parse_literal::<MusubiPackageRef>(&request.package)?;
    let mut payload = Map::new();
    payload.insert("package".to_owned(), Value::String(package.to_string()));
    payload.insert("reason".to_owned(), Value::String(request.reason.clone()));
    let instruction = YankMusubiRelease::new(package, request.reason);
    Ok(JsonBody(build_instruction_envelope(
        YankMusubiRelease::WIRE_ID,
        instruction,
        payload,
    )?))
}

/// HTTP handler for `POST /v1/musubi/instructions/set-alias`.
pub async fn handler_build_set_alias_instruction(
    NoritoJson(request): NoritoJson<SetMusubiAliasInstructionRequest>,
) -> Result<JsonBody<MusubiInstructionEnvelopeDto>> {
    let alias = parse_literal::<Name>(&request.alias)?;
    let target = parse_literal::<MusubiPackageId>(&request.target)?;
    let binding = MusubiShortAlias::new(alias.clone(), target.clone());
    let mut payload = Map::new();
    payload.insert("alias".to_owned(), Value::String(alias.to_string()));
    payload.insert("target".to_owned(), Value::String(target.to_string()));
    let instruction = SetMusubiShortAlias::new(binding);
    Ok(JsonBody(build_instruction_envelope(
        SetMusubiShortAlias::WIRE_ID,
        instruction,
        payload,
    )?))
}

/// HTTP handler for `POST /v1/musubi/instructions/assert-release-exists`.
pub async fn handler_build_assert_release_exists_instruction(
    NoritoJson(request): NoritoJson<AssertMusubiReleaseExistsInstructionRequest>,
) -> Result<JsonBody<MusubiInstructionEnvelopeDto>> {
    let package = parse_literal::<MusubiPackageId>(&request.package)?;
    let version = parse_literal::<MusubiVersion>(&request.version)?;
    let mut payload = Map::new();
    payload.insert("package".to_owned(), Value::String(package.to_string()));
    payload.insert("version".to_owned(), Value::String(version.to_string()));
    let instruction = AssertMusubiReleaseExists::new(package, version);
    Ok(JsonBody(build_instruction_envelope(
        AssertMusubiReleaseExists::WIRE_ID,
        instruction,
        payload,
    )?))
}

fn execute_query<Q>(app: &SharedAppState, query: Q) -> Result<Q::Output>
where
    Q: ValidSingularQuery + SingularQuery,
{
    query
        .execute(&app.state.view())
        .map_err(|err| Error::Query(ValidationFail::QueryFailed(err)))
}

fn required_param(value: Option<String>, name: &'static str) -> Result<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::routing::conversion_error(format!("missing required `{name}` query parameter"))
        })
}

fn parse_literal<T>(raw: &str) -> Result<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    raw.parse().map_err(|err| {
        crate::routing::conversion_error(format!("invalid Musubi literal `{raw}`: {err}"))
    })
}

fn to_json_value<T: JsonSerialize>(value: &T, context: &'static str) -> Result<Value> {
    json::to_value(value).map_err(|source| Error::SerializationFailure { context, source })
}

fn build_instruction_envelope(
    wire_id: &'static str,
    instruction: impl Into<InstructionBox>,
    payload: Map,
) -> Result<MusubiInstructionEnvelopeDto> {
    let instruction: InstructionBox = instruction.into();
    let framed = norito::to_bytes(&instruction).map_err(|err| {
        crate::routing::conversion_error(format!("failed to encode Musubi instruction: {err}"))
    })?;
    let mut instruction_json = Map::new();
    instruction_json.insert("wire_id".to_owned(), Value::String(wire_id.to_owned()));
    instruction_json.insert("payload".to_owned(), Value::Object(payload));

    Ok(MusubiInstructionEnvelopeDto {
        wire_id: wire_id.to_owned(),
        instruction_base64: BASE64_STANDARD.encode(&framed),
        instruction_hex: hex::encode(&framed),
        instruction_json: Value::Object(instruction_json),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yank_instruction_envelope_carries_framed_instruction_bytes() {
        let package: MusubiPackageRef = "dex.universal/swap-core@1.2.3".parse().expect("package");
        let instruction = YankMusubiRelease::new(package.clone(), "bad archive");
        let mut payload = Map::new();
        payload.insert("package".to_owned(), Value::String(package.to_string()));
        payload.insert("reason".to_owned(), Value::String("bad archive".to_owned()));

        let envelope = build_instruction_envelope(YankMusubiRelease::WIRE_ID, instruction, payload)
            .expect("instruction envelope");

        let bytes = BASE64_STANDARD
            .decode(envelope.instruction_base64.as_bytes())
            .expect("base64 instruction");
        assert_eq!(hex::encode(&bytes), envelope.instruction_hex);
        assert_eq!(envelope.wire_id, YankMusubiRelease::WIRE_ID);
        let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("decode box");
        assert!(decoded.as_any().is::<YankMusubiRelease>());
    }

    #[test]
    fn search_limit_cap_matches_public_http_contract() {
        let params = MusubiPackageSearchParams {
            limit: Some(MUSUBI_SEARCH_LIMIT_CAP + 1),
            ..MusubiPackageSearchParams::default()
        };

        let limit = params
            .limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .min(MUSUBI_SEARCH_LIMIT_CAP);

        assert_eq!(limit, MUSUBI_SEARCH_LIMIT_CAP);
    }
}
