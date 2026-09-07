//! Authority-scoped asynchronous subscription capability implementations.

mod validation;

use iroha_data_model::{asset::AssetDefinitionId, nft::NftId, subscription::SubscriptionPlan};
use iroha_torii_shared::subscriptions::{
    SubscriptionActionRequest, SubscriptionCancelMode, SubscriptionCreateRequest,
    SubscriptionGetResponse, SubscriptionListParams, SubscriptionListResponse,
    SubscriptionPlanCreateRequest, SubscriptionPlanListParams, SubscriptionPlanListResponse,
    SubscriptionUsageRequest,
};
use norito::json::{JsonDeserialize, JsonSerialize};

use super::{AccountClient, Client, DefaultRequestBuilder, HttpMethod, join_torii_url};
use crate::{
    Error, Result,
    http::{RequestBuilder, Response, StatusCode},
    subscriptions::{
        SubscriptionCreate, SubscriptionDraft, SubscriptionDraftArtifact,
        SubscriptionDraftOperation, SubscriptionUsage,
    },
};

// Preserve the transport's existing response ceiling while making it explicit
// for injected transports and all subscription operations.
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

/// Public subscription queries on one network and endpoint context.
#[derive(Clone, Copy, Debug)]
pub struct Subscriptions<'a> {
    client: &'a Client,
}

/// Unsigned subscription drafts authorized by one immutable account context.
#[derive(Clone, Copy, Debug)]
pub struct AccountSubscriptions<'a> {
    account: &'a AccountClient,
}

impl Client {
    /// Access public subscription plan and state queries.
    #[must_use]
    pub const fn subscriptions(&self) -> Subscriptions<'_> {
        Subscriptions { client: self }
    }
}

impl AccountClient {
    /// Access unsigned subscription preparation under this account's authority.
    #[must_use]
    pub const fn subscriptions(&self) -> AccountSubscriptions<'_> {
        AccountSubscriptions { account: self }
    }
}

fn invalid_request(operation: &'static str, error: impl std::fmt::Display) -> Error {
    Error::InvalidRequest {
        operation,
        details: error.to_string(),
    }
}

fn transport_error(operation: &'static str, error: eyre::Report) -> Error {
    if let Some(typed) = error.downcast_ref::<Error>() {
        return typed.clone();
    }
    if error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(reqwest::Error::is_timeout)
    {
        return Error::Timeout { operation };
    }
    Error::Transport {
        operation,
        details: error.to_string(),
    }
}

async fn dispatch<T: JsonDeserialize>(
    client: &Client,
    operation: &'static str,
    builder: DefaultRequestBuilder,
) -> Result<T> {
    let request = builder
        .header("Accept", "application/json")
        .max_response_bytes(MAX_RESPONSE_BYTES)
        .build()
        .map_err(|error| invalid_request(operation, error))?;
    let response = if client.torii_request_timeout.is_zero() {
        client.dispatch_request(request).await
    } else {
        tokio::time::timeout(
            client.torii_request_timeout,
            client.dispatch_request(request),
        )
        .await
        .map_err(|_| Error::Timeout { operation })?
    }
    .map_err(|error| transport_error(operation, error))?;
    decode_response(operation, response)
}

fn decode_response<T: JsonDeserialize>(
    operation: &'static str,
    response: Response<Vec<u8>>,
) -> Result<T> {
    if response.status() != StatusCode::OK {
        return Err(Error::Http {
            operation,
            status: response.status().as_u16(),
            body: response.into_body(),
        });
    }
    let mut content_types = response
        .headers()
        .get_all(http::header::CONTENT_TYPE)
        .iter();
    if !content_types
        .next()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
        || content_types.next().is_some()
    {
        return Err(Error::Decode {
            operation,
            details: "expected exactly one application/json response content type".to_owned(),
        });
    }
    norito::json::from_slice(response.body()).map_err(|error| Error::Decode {
        operation,
        details: error.to_string(),
    })
}

fn subscription_url(
    client: &Client,
    operation: &'static str,
    id: &NftId,
    action: Option<&str>,
) -> Result<url::Url> {
    let mut url = join_torii_url(&client.torii_url, "v1/subscriptions/");
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| invalid_request(operation, "endpoint cannot contain path segments"))?;
        segments.pop_if_empty().push(&id.to_string());
        if let Some(action) = action {
            segments.push(action);
        }
    }
    Ok(url)
}

impl Subscriptions<'_> {
    /// List subscription plans, preserving every supplied filter and count mode.
    ///
    /// # Errors
    /// Returns structured request, transport, timeout, HTTP or decoding failures.
    pub async fn list_plans(
        &self,
        params: &SubscriptionPlanListParams,
    ) -> Result<SubscriptionPlanListResponse> {
        const OP: &str = "subscriptions.list_plans";
        let url = join_torii_url(&self.client.torii_url, "v1/subscriptions/plans");
        let mut request = self
            .client
            .request_without_canonical_account_auth(HttpMethod::GET, url);
        if let Some(provider) = &params.provider {
            request = request.param("provider", provider);
        }
        if let Some(limit) = params.limit {
            request = request.param("limit", &limit);
        }
        if params.offset != 0 {
            request = request.param("offset", &params.offset);
        }
        if let Some(mode) = &params.count_mode {
            request = request.param("count_mode", mode);
        }
        dispatch(self.client, OP, request).await
    }

    /// List subscriptions, preserving every supplied filter and count mode.
    ///
    /// # Errors
    /// Returns structured request, transport, timeout, HTTP or decoding failures.
    pub async fn list(&self, params: &SubscriptionListParams) -> Result<SubscriptionListResponse> {
        const OP: &str = "subscriptions.list";
        let url = join_torii_url(&self.client.torii_url, "v1/subscriptions");
        let mut request = self
            .client
            .request_without_canonical_account_auth(HttpMethod::GET, url);
        if let Some(owner) = &params.owned_by {
            request = request.param("owned_by", owner);
        }
        if let Some(provider) = &params.provider {
            request = request.param("provider", provider);
        }
        if let Some(status) = &params.status {
            request = request.param("status", status);
        }
        if let Some(limit) = params.limit {
            request = request.param("limit", &limit);
        }
        if params.offset != 0 {
            request = request.param("offset", &params.offset);
        }
        if let Some(mode) = &params.count_mode {
            request = request.param("count_mode", mode);
        }
        dispatch(self.client, OP, request).await
    }

    /// Read exactly one subscription NFT.
    ///
    /// # Errors
    /// Returns structured request, transport, timeout, HTTP, decoding or identity failures.
    pub async fn get(&self, subscription_id: &NftId) -> Result<SubscriptionGetResponse> {
        const OP: &str = "subscriptions.get";
        let url = subscription_url(self.client, OP, subscription_id, None)?;
        let response: SubscriptionGetResponse = dispatch(
            self.client,
            OP,
            self.client
                .request_without_canonical_account_auth(HttpMethod::GET, url),
        )
        .await?;
        validation::require(
            response.subscription_id == *subscription_id,
            OP,
            "subscription_id",
        )?;
        Ok(response)
    }
}

impl AccountSubscriptions<'_> {
    async fn post<T: JsonSerialize, R: JsonDeserialize>(
        &self,
        operation: &'static str,
        url: url::Url,
        request: &T,
    ) -> Result<R> {
        self.account.ensure_direct_signing_capability()?;
        let body =
            norito::json::to_vec(request).map_err(|error| invalid_request(operation, error))?;
        let builder = self
            .account
            .client()
            .account_signed_request(HttpMethod::POST, url, body)
            .map_err(|error| Error::RequestSigning {
                operation,
                details: error.to_string(),
            })?
            .header("Content-Type", "application/json");
        dispatch(self.account.client(), operation, builder).await
    }

    /// Prepare registration of a plan owned by this account; never submit it.
    ///
    /// # Errors
    /// Returns structured authority, signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_plan(
        &self,
        plan_id: &AssetDefinitionId,
        plan: &SubscriptionPlan,
    ) -> Result<SubscriptionDraft> {
        const OP: &str = "subscriptions.prepare_plan";
        if plan.provider != *self.account.authority() {
            return Err(invalid_request(
                OP,
                "plan provider must equal the bound account authority",
            ));
        }
        let request = SubscriptionPlanCreateRequest {
            authority: self.account.authority().clone(),
            plan_id: plan_id.clone(),
            plan: plan.clone(),
        };
        let url = join_torii_url(self.account.endpoint(), "v1/subscriptions/plans");
        let response = self.post(OP, url, &request).await?;
        let payload = validation::plan(self.account, &request, &response)?;
        Ok(SubscriptionDraft::new(
            self.account,
            SubscriptionDraftOperation::Plan {
                plan_id: plan_id.clone(),
            },
            SubscriptionDraftArtifact::Payload(payload),
            None,
        ))
    }

    /// Prepare a subscription and its billing resources under this account.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare(&self, intent: &SubscriptionCreate) -> Result<SubscriptionDraft> {
        const OP: &str = "subscriptions.prepare";
        let request = SubscriptionCreateRequest {
            authority: self.account.authority().clone(),
            subscription_id: intent.subscription_id.clone(),
            plan_id: intent.plan_id.clone(),
            billing_trigger_id: intent.billing_trigger_id.clone(),
            usage_trigger_id: intent.usage_trigger_id.clone(),
            first_charge_ms: intent.first_charge_ms,
            grant_usage_to_provider: intent.grant_usage_to_provider,
        };
        let url = join_torii_url(self.account.endpoint(), "v1/subscriptions");
        let response = self.post(OP, url, &request).await?;
        let instructions = validation::create(&request, &response)?;
        Ok(SubscriptionDraft::new(
            self.account,
            SubscriptionDraftOperation::Create {
                subscription_id: intent.subscription_id.clone(),
                plan_id: intent.plan_id.clone(),
            },
            SubscriptionDraftArtifact::Instructions(instructions),
            Some(response.resulting_subscription),
        ))
    }

    /// Prepare pausing a subscription without submitting the draft.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_pause(&self, id: &NftId) -> Result<SubscriptionDraft> {
        self.action(id, "pause", None, None).await
    }

    /// Prepare resuming a subscription at an explicit or plan-derived charge time.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_resume(
        &self,
        id: &NftId,
        charge_at_ms: Option<u64>,
    ) -> Result<SubscriptionDraft> {
        self.action(id, "resume", charge_at_ms, None).await
    }

    /// Prepare cancellation using one explicit cancellation mode.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_cancel(
        &self,
        id: &NftId,
        mode: SubscriptionCancelMode,
    ) -> Result<SubscriptionDraft> {
        self.action(id, "cancel", None, Some(mode)).await
    }

    /// Prepare keeping a subscription scheduled for cancellation.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_keep(&self, id: &NftId) -> Result<SubscriptionDraft> {
        self.action(id, "keep", None, None).await
    }

    /// Prepare an immediate charge, optionally at an explicit timestamp.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_charge(
        &self,
        id: &NftId,
        charge_at_ms: Option<u64>,
    ) -> Result<SubscriptionDraft> {
        self.action(id, "charge-now", charge_at_ms, None).await
    }

    async fn action(
        &self,
        id: &NftId,
        action: &'static str,
        charge_at_ms: Option<u64>,
        cancel_mode: Option<SubscriptionCancelMode>,
    ) -> Result<SubscriptionDraft> {
        let operation = match action {
            "pause" => "subscriptions.prepare_pause",
            "resume" => "subscriptions.prepare_resume",
            "cancel" => "subscriptions.prepare_cancel",
            "keep" => "subscriptions.prepare_keep",
            "charge-now" => "subscriptions.prepare_charge",
            _ => return Err(invalid_request("subscriptions", "unknown action")),
        };
        let request = SubscriptionActionRequest {
            authority: self.account.authority().clone(),
            charge_at_ms,
            cancel_mode,
        };
        let url = subscription_url(self.account.client(), operation, id, Some(action))?;
        let response = self.post(operation, url, &request).await?;
        let instructions = validation::action(operation, action, id, &request, &response)?;
        let subscription_id = id.clone();
        let kind = match action {
            "pause" => SubscriptionDraftOperation::Pause { subscription_id },
            "resume" => SubscriptionDraftOperation::Resume { subscription_id },
            "cancel" => SubscriptionDraftOperation::Cancel {
                subscription_id,
                mode: cancel_mode
                    .ok_or_else(|| invalid_request(operation, "cancel mode missing"))?,
            },
            "keep" => SubscriptionDraftOperation::Keep { subscription_id },
            "charge-now" => SubscriptionDraftOperation::Charge { subscription_id },
            _ => return Err(invalid_request(operation, "unknown action")),
        };
        Ok(SubscriptionDraft::new(
            self.account,
            kind,
            SubscriptionDraftArtifact::Instructions(instructions),
            Some(response.details.resulting_subscription),
        ))
    }

    /// Prepare one usage increment under this account's authority.
    ///
    /// # Errors
    /// Returns structured signing, transport, HTTP, decoding or draft-binding failures.
    pub async fn prepare_usage(
        &self,
        id: &NftId,
        intent: &SubscriptionUsage,
    ) -> Result<SubscriptionDraft> {
        const OP: &str = "subscriptions.prepare_usage";
        let request = SubscriptionUsageRequest {
            authority: self.account.authority().clone(),
            unit_key: intent.unit_key.clone(),
            delta: intent.delta.clone(),
            usage_trigger_id: intent.usage_trigger_id.clone(),
        };
        let url = subscription_url(self.account.client(), OP, id, Some("usage"))?;
        let response = self.post(OP, url, &request).await?;
        let payload = validation::usage(self.account, id, &request, &response)?;
        Ok(SubscriptionDraft::new(
            self.account,
            SubscriptionDraftOperation::Usage {
                subscription_id: id.clone(),
                unit_key: intent.unit_key.clone(),
                delta: intent.delta.clone(),
            },
            SubscriptionDraftArtifact::Payload(payload),
            None,
        ))
    }
}
