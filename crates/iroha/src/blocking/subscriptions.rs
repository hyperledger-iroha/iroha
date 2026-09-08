//! Subscription facades using the same asynchronous capability implementation.

use super::{AccountClient, RuntimeOwner};
use crate::{
    Result,
    subscriptions::{SubscriptionCreate, SubscriptionDraft, SubscriptionUsage},
};
use iroha_data_model::{asset::AssetDefinitionId, nft::NftId, subscription::SubscriptionPlan};
use iroha_torii_shared::subscriptions::{
    SubscriptionCancelMode, SubscriptionGetResponse, SubscriptionListParams,
    SubscriptionListResponse, SubscriptionPlanListParams, SubscriptionPlanListResponse,
};

impl AccountClient {
    /// Access subscription drafts bound to this account.
    #[must_use]
    pub fn subscriptions(&self) -> AccountSubscriptions<'_> {
        AccountSubscriptions {
            inner: self.inner.subscriptions(),
            runtime: &self.runtime,
        }
    }
}

/// Blocking public subscription reads on a client's owned runtime.
#[derive(Clone, Copy, Debug)]
pub struct Subscriptions<'a> {
    inner: crate::subscriptions::Subscriptions<'a>,
    runtime: &'a RuntimeOwner,
}

/// Blocking unsigned subscription preparation on an account's owned runtime.
#[derive(Clone, Copy, Debug)]
pub struct AccountSubscriptions<'a> {
    inner: crate::subscriptions::AccountSubscriptions<'a>,
    runtime: &'a RuntimeOwner,
}

impl super::Client {
    /// Access public subscription reads through the facade's reusable runtime.
    #[must_use]
    pub fn subscriptions(&self) -> Subscriptions<'_> {
        Subscriptions {
            inner: self.inner.subscriptions(),
            runtime: &self.runtime,
        }
    }
}

impl Subscriptions<'_> {
    /// List public subscription plans.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn list_plans(
        &self,
        params: &SubscriptionPlanListParams,
    ) -> Result<SubscriptionPlanListResponse> {
        self.runtime.block_on(self.inner.list_plans(params))?
    }
    /// List public subscription state.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn list(&self, params: &SubscriptionListParams) -> Result<SubscriptionListResponse> {
        self.runtime.block_on(self.inner.list(params))?
    }
    /// Read one subscription NFT.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn get(&self, id: &NftId) -> Result<SubscriptionGetResponse> {
        self.runtime.block_on(self.inner.get(id))?
    }
}

impl AccountSubscriptions<'_> {
    /// Prepare an unsigned plan registration under the bound account.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_plan(
        &self,
        id: &AssetDefinitionId,
        plan: &SubscriptionPlan,
    ) -> Result<SubscriptionDraft> {
        self.runtime.block_on(self.inner.prepare_plan(id, plan))?
    }
    /// Prepare an unsigned subscription creation draft.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare(&self, intent: &SubscriptionCreate) -> Result<SubscriptionDraft> {
        self.runtime.block_on(self.inner.prepare(intent))?
    }
    /// Prepare pausing a subscription.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_pause(&self, id: &NftId) -> Result<SubscriptionDraft> {
        self.runtime.block_on(self.inner.prepare_pause(id))?
    }
    /// Prepare resuming a subscription.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_resume(
        &self,
        id: &NftId,
        charge_at_ms: Option<u64>,
    ) -> Result<SubscriptionDraft> {
        self.runtime
            .block_on(self.inner.prepare_resume(id, charge_at_ms))?
    }
    /// Prepare cancellation with an explicit mode.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_cancel(
        &self,
        id: &NftId,
        mode: SubscriptionCancelMode,
    ) -> Result<SubscriptionDraft> {
        self.runtime.block_on(self.inner.prepare_cancel(id, mode))?
    }
    /// Prepare keeping a subscription scheduled for cancellation.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_keep(&self, id: &NftId) -> Result<SubscriptionDraft> {
        self.runtime.block_on(self.inner.prepare_keep(id))?
    }
    /// Prepare an immediate charge.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_charge(
        &self,
        id: &NftId,
        charge_at_ms: Option<u64>,
    ) -> Result<SubscriptionDraft> {
        self.runtime
            .block_on(self.inner.prepare_charge(id, charge_at_ms))?
    }
    /// Prepare one usage increment.
    ///
    /// # Errors
    /// Returns the asynchronous operation error or a typed blocking-runtime rejection.
    pub fn prepare_usage(
        &self,
        id: &NftId,
        intent: &SubscriptionUsage,
    ) -> Result<SubscriptionDraft> {
        self.runtime
            .block_on(self.inner.prepare_usage(id, intent))?
    }
}
