//! Torii HTTP helpers for the first-release Musubi package registry.
//!
//! Query endpoints accept the bounded data-model request objects directly and return the
//! corresponding typed finalized result. Instruction endpoints are pre-signing helpers: they return
//! deterministic Norito-framed instructions and never accept or load signing material.
use crate::{Error, JsonBody, NoritoJson, Result, SharedAppState};
use axum::extract::State;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_core::{
    musubi_search::MusubiSearchError,
    smartcontracts::{ValidSingularQuery, isi::musubi::ValidMusubiSingularQuery},
    telemetry::MusubiCursorFailureReasonV1,
};
use iroha_data_model::{
    ValidationFail,
    isi::{
        InstructionBox,
        musubi::{
            AcceptMusubiPackageMaintainerV1, AddMusubiArchiveLocationV1,
            AssertMusubiReleaseDigestV1, InviteMusubiPackageMaintainerV1, PublishMusubiReleaseV1,
            RecoverMusubiPackageV1, RegisterMusubiAliasV1, RegisterMusubiArchiveV1,
            RegisterMusubiNamespaceBindingV1, RegisterMusubiProviderBundleAttestationV1,
            RemoveMusubiPackageMaintainerV1, RetargetMusubiAliasV1, RetireMusubiArchiveLocationV1,
            RevokeMusubiPackageMaintainerInvitationV1, SetMusubiArtifactTakedownV1,
            SetMusubiPackageMaintainerRoleV1, SetMusubiPackageMetadataV1,
            SetMusubiRegistryPolicyV1, SetMusubiReleaseYankV1,
        },
    },
    musubi::{
        MusubiAliasHistoryPageV1, MusubiAliasQueryV1, MusubiAliasRecordV1,
        MusubiArchiveLocationPageV1, MusubiArchiveLocationQueryV1, MusubiArchiveRetentionPageV1,
        MusubiArchiveRetentionQueryV1, MusubiCursorFailureV1, MusubiExactPackageQueryV1,
        MusubiExactReleaseQueryV1, MusubiExactReleaseSnapshotV1, MusubiMaintainerPageV1,
        MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1, MusubiPackagePageQueryV1,
        MusubiPackageRecordV1, MusubiPageRequestV1, MusubiProviderBundleAttestationKeyV1,
        MusubiProviderBundleAttestationRecordV1, MusubiResolverIndexPageV1,
        MusubiResolverIndexQueryV1, MusubiSearchPageV1, MusubiSearchQueryV1, MusubiVersionPageV1,
    },
    query::{SingularQuery, error::QueryExecutionFail, musubi::prelude::*},
};
use norito::json::{self, JsonSerialize, Map, Value};
/// Schema name for deterministic unsigned Musubi instruction envelopes.
pub const MUSUBI_INSTRUCTION_ENVELOPE_SCHEMA_V1: &str = "musubi-instruction-envelope";
/// First and only supported instruction-envelope version.
pub const MUSUBI_INSTRUCTION_ENVELOPE_VERSION_V1: u8 = 1;
/// Deterministic unsigned instruction payload returned for local signing.
#[derive(Debug, Clone, crate::json_macros::JsonSerialize)]
pub struct MusubiInstructionEnvelopeV1 {
    /// Stable envelope schema identifier.
    pub schema: String,
    /// Envelope schema version; always one.
    pub version: u8,
    /// Stable first-release instruction wire identifier.
    pub wire_id: String,
    /// Base64-encoded Norito-framed [`InstructionBox`].
    pub instruction_base64: String,
    /// Hex-encoded Norito-framed [`InstructionBox`].
    pub instruction_hex: String,
    /// Human-readable JSON preview containing the wire id and exact payload.
    pub instruction_json: Value,
}
/// Execute an exact structural package query.
pub async fn handler_find_exact_package(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiExactPackageQueryV1>,
) -> Result<JsonBody<MusubiPackageRecordV1>> {
    request.package.validate().map_err(invalid_query_request)?;
    Ok(JsonBody(execute_query(
        &app,
        FindMusubiExactPackageV1::new(request),
    )?))
}
/// Execute an exact structural release query.
pub async fn handler_find_exact_release(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiExactReleaseQueryV1>,
) -> Result<JsonBody<MusubiExactReleaseSnapshotV1>> {
    request.release.validate().map_err(invalid_query_request)?;
    Ok(JsonBody(execute_query(
        &app,
        FindMusubiExactReleaseV1::new(request),
    )?))
}
/// Execute an exact immutable provider bundle-attestation audit query.
pub async fn handler_find_provider_bundle_attestation(
    State(app): State<SharedAppState>,
    NoritoJson(key): NoritoJson<MusubiProviderBundleAttestationKeyV1>,
) -> Result<JsonBody<MusubiProviderBundleAttestationRecordV1>> {
    key.validate().map_err(invalid_query_request)?;
    Ok(JsonBody(execute_query(
        &app,
        FindMusubiProviderBundleAttestationV1::new(key),
    )?))
}
/// Execute a finalized universal resolver-index query.
pub async fn handler_find_resolver_index(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiResolverIndexQueryV1>,
) -> Result<JsonBody<MusubiResolverIndexPageV1>> {
    request.package.validate().map_err(invalid_query_request)?;
    if let Some(requirement) = &request.requirement {
        requirement.validate().map_err(invalid_query_request)?;
    }
    validate_cursor_page(&app, &request.page)?;
    Ok(JsonBody(execute_musubi_query(
        &app,
        FindMusubiResolverIndexV1::new(request),
    )?))
}
/// Execute a finalized structured-version page query.
pub async fn handler_find_versions(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiPackagePageQueryV1>,
) -> Result<JsonBody<MusubiVersionPageV1>> {
    validate_package_page_request(&app, &request)?;
    Ok(JsonBody(execute_musubi_query(
        &app,
        FindMusubiVersionsV1::new(request),
    )?))
}
/// Execute a finalized accepted-member and pending-invitation page query.
pub async fn handler_find_maintainers(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiPackagePageQueryV1>,
) -> Result<JsonBody<MusubiMaintainerPageV1>> {
    validate_package_page_request(&app, &request)?;
    Ok(JsonBody(execute_musubi_query(
        &app,
        FindMusubiMaintainersV1::new(request),
    )?))
}
/// Execute a finalized archive-location page query.
pub async fn handler_find_archive_locations(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiArchiveLocationQueryV1>,
) -> Result<JsonBody<MusubiArchiveLocationPageV1>> {
    if request.archive_id.is_zero() {
        return Err(crate::routing::conversion_error(
            "Musubi archive id must not be the all-zero sentinel".to_owned(),
        ));
    }
    validate_cursor_page(&app, &request.page)?;
    Ok(JsonBody(execute_musubi_query(
        &app,
        FindMusubiArchiveLocationsV1::new(request),
    )?))
}
/// Execute a bounded exact finalized archive cache-retention query.
pub async fn handler_find_archive_retention(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiArchiveRetentionQueryV1>,
) -> Result<JsonBody<MusubiArchiveRetentionPageV1>> {
    request.validate().map_err(invalid_query_request)?;
    Ok(JsonBody(execute_query(
        &app,
        FindMusubiArchiveRetentionV1::new(request),
    )?))
}
/// Execute an exact permanent-alias query.
pub async fn handler_find_alias(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiAliasQueryV1>,
) -> Result<JsonBody<MusubiAliasRecordV1>> {
    validate_alias_query_request(&app, &request)?;
    Ok(JsonBody(execute_query(
        &app,
        FindMusubiAliasV1::new(request),
    )?))
}
/// Execute a finalized permanent-alias history query.
pub async fn handler_find_alias_history(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiAliasQueryV1>,
) -> Result<JsonBody<MusubiAliasHistoryPageV1>> {
    validate_alias_query_request(&app, &request)?;
    Ok(JsonBody(execute_musubi_query(
        &app,
        FindMusubiAliasHistoryV1::new(request),
    )?))
}
/// Execute a finalized byte-ordered package-prefix query.
pub async fn handler_find_ordered_prefix(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiOrderedPrefixQueryV1>,
) -> Result<JsonBody<MusubiOrderedPackagePageV1>> {
    request.prefix.validate().map_err(invalid_query_request)?;
    validate_cursor_page(&app, &request.page)?;
    Ok(JsonBody(execute_musubi_query(
        &app,
        FindMusubiOrderedPrefixV1::new(request),
    )?))
}
/// Execute an exact-token query against the rebuildable finalized-event search projection.
pub async fn handler_search_packages(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<MusubiSearchQueryV1>,
) -> Result<JsonBody<MusubiSearchPageV1>> {
    request.validate().map_err(invalid_query_request)?;
    let result = app.musubi_search.read().await.search(&request);
    match result {
        Ok(page) => Ok(JsonBody(page)),
        Err(MusubiSearchError::StaleCursor) => {
            record_cursor_failure(&app, MusubiCursorFailureReasonV1::Other);
            Err(Error::Query(ValidationFail::QueryFailed(
                QueryExecutionFail::Expired,
            )))
        }
        Err(
            error @ (MusubiSearchError::InvalidQuery
            | MusubiSearchError::InvalidPageSize
            | MusubiSearchError::QueryTooBroad),
        ) => Err(crate::routing::conversion_error(format!(
            "invalid Musubi V1 search request: {error}"
        ))),
        Err(MusubiSearchError::ProjectionUnavailable) => Err(Error::AppServiceUnavailable {
            code: "musubi_search_projection_unavailable",
            message: "The finalized Musubi package-search projection is unavailable.".to_owned(),
        }),
        Err(
            error @ (MusubiSearchError::InconsistentFinalizedEvent
            | MusubiSearchError::RevisionOverflow),
        ) => {
            iroha_logger::error!(%error, "Musubi package-search projection failed closed");
            Err(Error::AppServiceUnavailable {
                code: "musubi_search_projection_inconsistent",
                message: "The finalized Musubi package-search projection requires recovery."
                    .to_owned(),
            })
        }
    }
}
macro_rules! instruction_handler {
    ($handler:ident, $instruction:ty, $doc:literal) => {
        #[doc = $doc]
        pub async fn $handler(
            NoritoJson(instruction): NoritoJson<$instruction>,
        ) -> Result<JsonBody<MusubiInstructionEnvelopeV1>> {
            Ok(JsonBody(build_instruction_envelope(
                <$instruction>::WIRE_ID,
                instruction,
            )?))
        }
    };
}
instruction_handler!(
    handler_build_namespace_binding_register,
    RegisterMusubiNamespaceBindingV1,
    "Build an unsigned first-release namespace-binding registration."
);
instruction_handler!(
    handler_build_archive_register,
    RegisterMusubiArchiveV1,
    "Build an unsigned first-release archive registration."
);
instruction_handler!(
    handler_build_provider_bundle_attestation_register,
    RegisterMusubiProviderBundleAttestationV1,
    "Build an unsigned immutable first-release provider bundle-attestation registration."
);
instruction_handler!(
    handler_build_archive_location_add,
    AddMusubiArchiveLocationV1,
    "Build an unsigned first-release archive-location add or renewal."
);
instruction_handler!(
    handler_build_archive_location_retire,
    RetireMusubiArchiveLocationV1,
    "Build an unsigned first-release archive-location retirement."
);
instruction_handler!(
    handler_build_release_publish,
    PublishMusubiReleaseV1,
    "Build an unsigned first-release release publication."
);
instruction_handler!(
    handler_build_release_yank_set,
    SetMusubiReleaseYankV1,
    "Build an unsigned reversible first-release yank or unyank transition."
);
instruction_handler!(
    handler_build_package_metadata_set,
    SetMusubiPackageMetadataV1,
    "Build an unsigned first-release package metadata replacement."
);
instruction_handler!(
    handler_build_package_member_invite,
    InviteMusubiPackageMaintainerV1,
    "Build an unsigned first-release package-member invitation."
);
instruction_handler!(
    handler_build_package_member_accept,
    AcceptMusubiPackageMaintainerV1,
    "Build an unsigned first-release package-member invitation acceptance."
);
instruction_handler!(
    handler_build_package_member_invitation_revoke,
    RevokeMusubiPackageMaintainerInvitationV1,
    "Build an unsigned first-release pending package-member invitation revocation."
);
instruction_handler!(
    handler_build_package_member_set_role,
    SetMusubiPackageMaintainerRoleV1,
    "Build an unsigned first-release package-member role replacement."
);
instruction_handler!(
    handler_build_package_member_remove,
    RemoveMusubiPackageMaintainerV1,
    "Build an unsigned first-release package-member removal."
);
instruction_handler!(
    handler_build_alias_register,
    RegisterMusubiAliasV1,
    "Build an unsigned paid permanent-alias registration."
);
instruction_handler!(
    handler_build_package_recover,
    RecoverMusubiPackageV1,
    "Build an unsigned Parliament-enacted package recovery."
);
instruction_handler!(
    handler_build_alias_retarget,
    RetargetMusubiAliasV1,
    "Build an unsigned Parliament-enacted permanent-alias retarget."
);
instruction_handler!(
    handler_build_artifact_takedown,
    SetMusubiArtifactTakedownV1,
    "Build an unsigned Parliament-enacted artifact takedown."
);
instruction_handler!(
    handler_build_registry_policy_set,
    SetMusubiRegistryPolicyV1,
    "Build an unsigned Parliament-enacted registry-policy replacement."
);
instruction_handler!(
    handler_build_release_digest_assert,
    AssertMusubiReleaseDigestV1,
    "Build an unsigned exact release-digest assertion."
);
fn validate_package_page_request(
    app: &SharedAppState,
    request: &MusubiPackagePageQueryV1,
) -> Result<()> {
    request.package.validate().map_err(invalid_query_request)?;
    validate_cursor_page(app, &request.page)
}
fn validate_alias_query_request(app: &SharedAppState, request: &MusubiAliasQueryV1) -> Result<()> {
    request.alias.validate().map_err(invalid_query_request)?;
    validate_cursor_page(app, &request.page)
}
fn validate_cursor_page(app: &SharedAppState, page: &MusubiPageRequestV1) -> Result<()> {
    page.validate().map_err(|error| {
        if page_validation_failure_is_invalid_cursor(page) {
            record_cursor_failure(app, INVALID_CURSOR_FAILURE_REASON);
        }
        invalid_query_request(error)
    })
}
fn page_validation_failure_is_invalid_cursor(page: &MusubiPageRequestV1) -> bool {
    let Some(cursor) = &page.cursor else {
        return false;
    };
    // Preserve `MusubiPageRequestV1::validate` precedence: an invalid limit is
    // a request-bound failure even when the same request also carries a bad cursor.
    let limit_only = MusubiPageRequestV1 {
        limit: page.limit,
        cursor: None,
    };
    limit_only.validate().is_ok() && cursor.validate().is_err()
}
const INVALID_CURSOR_FAILURE_REASON: MusubiCursorFailureReasonV1 =
    MusubiCursorFailureReasonV1::Invalid;
fn invalid_query_request(error: impl core::fmt::Display) -> Error {
    crate::routing::conversion_error(format!("invalid Musubi V1 query request: {error}"))
}
fn execute_query<Q>(app: &SharedAppState, query: Q) -> Result<Q::Output>
where
    Q: ValidSingularQuery + SingularQuery,
{
    ValidSingularQuery::execute(&query, &app.state.view()).map_err(|error| {
        if let Some(reason) = core_cursor_failure_reason(&error) {
            record_cursor_failure(app, reason);
        }
        Error::Query(ValidationFail::QueryFailed(error))
    })
}
fn execute_musubi_query<Q>(app: &SharedAppState, query: Q) -> Result<Q::Output>
where
    Q: ValidMusubiSingularQuery + SingularQuery,
{
    ValidMusubiSingularQuery::execute_musubi(&query, &app.state.view()).map_err(|error| {
        if let Some(reason) = error.cursor_failure() {
            record_cursor_failure(app, cursor_metric_reason(reason));
        }
        Error::Query(ValidationFail::QueryFailed(error.into_query_error()))
    })
}
const fn cursor_metric_reason(reason: MusubiCursorFailureV1) -> MusubiCursorFailureReasonV1 {
    match reason {
        MusubiCursorFailureV1::FinalizedAnchorMismatch => MusubiCursorFailureReasonV1::StaleAnchor,
        MusubiCursorFailureV1::IndexRevisionMismatch => MusubiCursorFailureReasonV1::StaleRevision,
        MusubiCursorFailureV1::QueryMismatch => MusubiCursorFailureReasonV1::WrongQuery,
        MusubiCursorFailureV1::CallerMismatch => MusubiCursorFailureReasonV1::WrongCaller,
        MusubiCursorFailureV1::LastKeyStale => MusubiCursorFailureReasonV1::Boundary,
    }
}
const fn core_cursor_failure_reason(
    error: &QueryExecutionFail,
) -> Option<MusubiCursorFailureReasonV1> {
    if matches!(error, QueryExecutionFail::Expired) {
        // Generic query paths expose only Expired. Do not invent a more
        // specific telemetry label here; the six paged registry paths use
        // `execute_musubi_query` and retain their typed reason separately.
        Some(MusubiCursorFailureReasonV1::Other)
    } else {
        None
    }
}
fn record_cursor_failure(app: &SharedAppState, reason: MusubiCursorFailureReasonV1) {
    app.telemetry_handle().with_metrics(|telemetry| {
        telemetry.record_musubi_cursor_failure(reason);
    });
}
fn to_json_value<T: JsonSerialize>(value: &T, context: &'static str) -> Result<Value> {
    json::to_value(value).map_err(|source| Error::SerializationFailure {
        context,
        source: Box::new(source),
    })
}
fn build_instruction_envelope<T>(
    wire_id: &'static str,
    instruction: T,
) -> Result<MusubiInstructionEnvelopeV1>
where
    T: Into<InstructionBox> + JsonSerialize,
{
    let payload = to_json_value(&instruction, "Musubi V1 instruction preview")?;
    let instruction: InstructionBox = instruction.into();
    let framed = norito::encode_canonical(&instruction).map_err(|error| {
        crate::routing::conversion_error(format!("failed to encode Musubi V1 instruction: {error}"))
    })?;
    let mut instruction_json = Map::new();
    instruction_json.insert("wire_id".to_owned(), Value::String(wire_id.to_owned()));
    instruction_json.insert("payload".to_owned(), payload);
    Ok(MusubiInstructionEnvelopeV1 {
        schema: MUSUBI_INSTRUCTION_ENVELOPE_SCHEMA_V1.to_owned(),
        version: MUSUBI_INSTRUCTION_ENVELOPE_VERSION_V1,
        wire_id: wire_id.to_owned(),
        instruction_base64: BASE64_STANDARD.encode(&framed),
        instruction_hex: hex::encode(&framed),
        instruction_json: Value::Object(instruction_json),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        isi::musubi::SetMusubiReleaseYankV1,
        musubi::{
            MUSUBI_MAX_PAGE_SIZE_V1, MusubiFinalizedCursorV1, MusubiPackageIdV1,
            MusubiPackageScopeV1, MusubiQueryHashV1, MusubiRegistrySnapshotV1, MusubiReleaseIdV1,
        },
        nexus::DataSpaceId,
    };
    fn release() -> MusubiReleaseIdV1 {
        MusubiReleaseIdV1::new(
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                "math".parse().expect("package name"),
            ),
            "1.2.3".parse().expect("version"),
        )
    }
    #[test]
    fn v1_yank_envelope_carries_framed_instruction_bytes() {
        let instruction =
            SetMusubiReleaseYankV1::new(release(), true, "withdrawn".parse().expect("reason"), 3);
        let envelope = build_instruction_envelope(SetMusubiReleaseYankV1::WIRE_ID, instruction)
            .expect("instruction envelope");
        assert_eq!(envelope.schema, MUSUBI_INSTRUCTION_ENVELOPE_SCHEMA_V1);
        assert_eq!(envelope.version, MUSUBI_INSTRUCTION_ENVELOPE_VERSION_V1);
        assert_eq!(envelope.wire_id, SetMusubiReleaseYankV1::WIRE_ID);
        let bytes = BASE64_STANDARD
            .decode(envelope.instruction_base64.as_bytes())
            .expect("base64 instruction");
        assert_eq!(hex::encode(&bytes), envelope.instruction_hex);
        let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("decode box");
        assert!(decoded.as_any().is::<SetMusubiReleaseYankV1>());
        assert_eq!(
            envelope
                .instruction_json
                .pointer("/payload/yanked")
                .and_then(Value::as_bool),
            Some(true)
        );
    }
    #[test]
    fn v1_instruction_envelope_ignores_ambient_layout_flags() {
        let expected = build_instruction_envelope(
            SetMusubiReleaseYankV1::WIRE_ID,
            SetMusubiReleaseYankV1::new(release(), true, "withdrawn".parse().expect("reason"), 3),
        )
        .expect("canonical instruction envelope");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let actual = build_instruction_envelope(
            SetMusubiReleaseYankV1::WIRE_ID,
            SetMusubiReleaseYankV1::new(release(), true, "withdrawn".parse().expect("reason"), 3),
        )
        .expect("instruction envelope under alternate ambient flags");
        assert_eq!(actual.instruction_base64, expected.instruction_base64);
        assert_eq!(actual.instruction_hex, expected.instruction_hex);
    }
    #[test]
    fn envelope_json_is_exactly_one_v1_document() {
        let instruction =
            SetMusubiReleaseYankV1::new(release(), false, "restored".parse().expect("reason"), 4);
        let envelope = build_instruction_envelope(SetMusubiReleaseYankV1::WIRE_ID, instruction)
            .expect("instruction envelope");
        let document = json::to_string(&envelope).expect("serialize envelope");
        let value: Value = json::from_str(&document).expect("one JSON document");
        assert_eq!(
            value.get("schema").and_then(Value::as_str),
            Some(MUSUBI_INSTRUCTION_ENVELOPE_SCHEMA_V1)
        );
        assert_eq!(value.get("version").and_then(Value::as_u64), Some(1));
        assert!(document.trim_start().starts_with('{'));
        assert!(document.trim_end().ends_with('}'));
    }
    #[test]
    fn only_core_expired_is_classified_as_a_cursor_failure() {
        assert_eq!(
            core_cursor_failure_reason(&QueryExecutionFail::Expired),
            Some(MusubiCursorFailureReasonV1::Other)
        );
        assert_eq!(
            core_cursor_failure_reason(&QueryExecutionFail::NotFound),
            None
        );
    }
    #[test]
    fn page_validation_classifies_only_structurally_invalid_supplied_cursors() {
        let valid_cursor = MusubiFinalizedCursorV1 {
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 7,
                finalized_block_hash: [0x42; 32],
                index_revision: 3,
            },
            query_hash: MusubiQueryHashV1::new([0x24; 32]),
            last_key: "cursor-key".to_owned(),
            caller: None,
        };
        let mut invalid_cursor = valid_cursor.clone();
        invalid_cursor.query_hash = MusubiQueryHashV1::new([0; 32]);
        let oversized_limit =
            u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1 + 1).expect("page maximum fits u32");
        assert!(!page_validation_failure_is_invalid_cursor(
            &MusubiPageRequestV1 {
                limit: oversized_limit,
                cursor: None,
            }
        ));
        assert!(!page_validation_failure_is_invalid_cursor(
            &MusubiPageRequestV1 {
                limit: oversized_limit,
                cursor: Some(valid_cursor.clone()),
            }
        ));
        assert!(page_validation_failure_is_invalid_cursor(
            &MusubiPageRequestV1 {
                limit: 0,
                cursor: Some(invalid_cursor.clone()),
            }
        ));
        assert!(!page_validation_failure_is_invalid_cursor(
            &MusubiPageRequestV1 {
                limit: oversized_limit,
                cursor: Some(invalid_cursor),
            }
        ));
        assert!(!page_validation_failure_is_invalid_cursor(
            &MusubiPageRequestV1 {
                limit: 0,
                cursor: Some(valid_cursor),
            }
        ));
    }
    #[test]
    fn exact_musubi_cursor_failures_map_to_closed_metric_reasons() {
        for (failure, expected) in [
            (
                MusubiCursorFailureV1::FinalizedAnchorMismatch,
                MusubiCursorFailureReasonV1::StaleAnchor,
            ),
            (
                MusubiCursorFailureV1::IndexRevisionMismatch,
                MusubiCursorFailureReasonV1::StaleRevision,
            ),
            (
                MusubiCursorFailureV1::QueryMismatch,
                MusubiCursorFailureReasonV1::WrongQuery,
            ),
            (
                MusubiCursorFailureV1::CallerMismatch,
                MusubiCursorFailureReasonV1::WrongCaller,
            ),
            (
                MusubiCursorFailureV1::LastKeyStale,
                MusubiCursorFailureReasonV1::Boundary,
            ),
        ] {
            assert_eq!(cursor_metric_reason(failure), expected);
        }
        assert_eq!(
            INVALID_CURSOR_FAILURE_REASON,
            MusubiCursorFailureReasonV1::Invalid
        );
    }
}
