//! Ledger-backed Sora Name Service (SNS) HTTP handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use iroha_core::{
    sns::{
        SnsError as CoreSnsError, SnsNamespace, apply_with_state_block, get_name_record,
        policy_by_id, register_name, renew_name, transfer_name, update_name_controllers,
    },
    state::{StateReadOnly, StateReadOnlyWithTransactions},
};
use iroha_data_model::sns::{
    FreezeNameRequestV1, GovernanceHookV1, RegisterNameRequestV1, RegisterNameResponseV1,
    RenewNameRequestV1, SuffixId, TransferNameRequestV1, UpdateControllersRequestV1,
};

use crate::{JsonBody, SharedAppState};

/// HTTP-friendly error wrapper for SNS routes.
#[derive(Debug)]
pub enum SnsError {
    /// Entity was not found.
    NotFound(String),
    /// Request failed validation.
    BadRequest(String),
    /// Request conflicts with existing state.
    Conflict(String),
    /// Internal state mutation failed.
    Internal(String),
}

impl From<CoreSnsError> for SnsError {
    fn from(error: CoreSnsError) -> Self {
        match error {
            CoreSnsError::NotFound(msg) => Self::NotFound(msg),
            CoreSnsError::BadRequest(msg) => Self::BadRequest(msg),
            CoreSnsError::Conflict(msg) => Self::Conflict(msg),
            CoreSnsError::Internal(msg) => Self::Internal(msg),
        }
    }
}

impl IntoResponse for SnsError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, message).into_response()
    }
}

fn metric_namespace_from_suffix_id(suffix_id: SuffixId) -> String {
    SnsNamespace::from_suffix_id(suffix_id)
        .map(|namespace| namespace.as_path().to_owned())
        .unwrap_or_else(|_| suffix_id.to_string())
}

fn record_registrar_status<T>(
    app: &SharedAppState,
    namespace: &str,
    outcome: &Result<T, SnsError>,
) {
    let result = if outcome.is_ok() { "ok" } else { "error" };
    app.telemetry.with_metrics(|telemetry| {
        telemetry.inc_sns_registrar_status(result, namespace);
    });
}

fn current_ledger_time_ms(app: &SharedAppState) -> u64 {
    let latest_block_ms = app.state.view().latest_block().map_or(0, |block| {
        u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
    });
    let wall_clock_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(latest_block_ms);
    wall_clock_ms.max(latest_block_ms)
}

/// Handle `POST /v1/sns/names`.
#[axum::debug_handler(state = SharedAppState)]
pub async fn handle_register_name(
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<RegisterNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = metric_namespace_from_suffix_id(request.selector.suffix_id);
    let outcome =
        apply_with_state_block(app.state.as_ref(), |tx| register_name(tx, request.clone()))
            .map_err(SnsError::from);
    record_registrar_status(&app, &namespace, &outcome);
    let record = outcome?;
    Ok((
        StatusCode::CREATED,
        JsonBody(RegisterNameResponseV1 {
            name_record: record,
        }),
    ))
}

/// Handle `GET /v1/sns/names/{namespace}/{literal}`.
pub async fn handle_get_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let view = app.state.view();
    let record = get_name_record(
        view.world(),
        &view.nexus.dataspace_catalog,
        namespace,
        &literal,
        current_ledger_time_ms(&app),
    )
    .map_err(SnsError::from)?;
    Ok(JsonBody(record))
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/renew`.
pub async fn handle_renew_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<RenewNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = apply_with_state_block(app.state.as_ref(), |tx| {
        renew_name(tx, namespace, &literal, request.clone())
    })
    .map_err(SnsError::from);
    record_registrar_status(&app, &namespace_label, &outcome);
    Ok(JsonBody(outcome?))
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/transfer`.
pub async fn handle_transfer_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<TransferNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = apply_with_state_block(app.state.as_ref(), |tx| {
        transfer_name(tx, namespace, &literal, request.clone())
    })
    .map_err(SnsError::from);
    record_registrar_status(&app, &namespace_label, &outcome);
    Ok(JsonBody(outcome?))
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/controllers`.
pub async fn handle_update_name_controllers(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<UpdateControllersRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = apply_with_state_block(app.state.as_ref(), |tx| {
        update_name_controllers(tx, namespace, &literal, request.clone())
    })
    .map_err(SnsError::from);
    record_registrar_status(&app, &namespace_label, &outcome);
    Ok(JsonBody(outcome?))
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/freeze`.
pub async fn handle_freeze_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<FreezeNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = apply_with_state_block(app.state.as_ref(), |tx| {
        iroha_core::sns::freeze_name(tx, namespace, &literal, request.clone())
    })
    .map_err(SnsError::from);
    record_registrar_status(&app, &namespace_label, &outcome);
    Ok(JsonBody(outcome?))
}

/// Handle `DELETE /v1/sns/names/{namespace}/{literal}/freeze`.
pub async fn handle_unfreeze_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(governance): crate::JsonOnly<GovernanceHookV1>,
) -> Result<impl IntoResponse, SnsError> {
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = apply_with_state_block(app.state.as_ref(), |tx| {
        iroha_core::sns::unfreeze_name(tx, namespace, &literal, governance.clone())
    })
    .map_err(SnsError::from);
    record_registrar_status(&app, &namespace_label, &outcome);
    Ok(JsonBody(outcome?))
}

/// Handle `GET /v1/sns/policies/{suffix_id}`.
pub async fn handle_get_policy(
    Path(suffix_id): Path<SuffixId>,
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, SnsError> {
    let view = app.state.view();
    let policy = policy_by_id(view.world(), suffix_id).ok_or_else(|| {
        SnsError::NotFound(format!("suffix policy {suffix_id} is not registered"))
    })?;
    Ok(JsonBody(policy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_namespace_uses_fixed_namespace_paths() {
        assert_eq!(
            metric_namespace_from_suffix_id(iroha_core::sns::ACCOUNT_ALIAS_SUFFIX_ID),
            "account-alias"
        );
        assert_eq!(
            metric_namespace_from_suffix_id(iroha_core::sns::DOMAIN_NAME_SUFFIX_ID),
            "domain"
        );
        assert_eq!(
            metric_namespace_from_suffix_id(iroha_core::sns::DATASPACE_ALIAS_SUFFIX_ID),
            "dataspace"
        );
    }

    #[test]
    fn core_error_maps_to_http_status_family() {
        let not_found: SnsError = CoreSnsError::NotFound("missing".to_owned()).into();
        let response = not_found.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let internal: SnsError = CoreSnsError::Internal("boom".to_owned()).into();
        let response = internal.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
