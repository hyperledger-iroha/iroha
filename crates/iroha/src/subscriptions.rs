//! Public subscription reads and account-bound unsigned draft preparation.
//!
//! Obtain reads with [`crate::client::Client::subscriptions`] and prepare drafts
//! with [`crate::client::AccountClient::subscriptions`]. Preparing a draft never
//! submits a transaction. Signing and submission are explicit account operations.

pub use crate::client::subscriptions::{AccountSubscriptions, Subscriptions};
use iroha_data_model::{asset::AssetDefinitionId, nft::NftId, trigger::TriggerId};
use iroha_model_base::name::Name;
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::subscriptions::SubscriptionCancelMode;

/// Subscription creation intent; the account context supplies its signing authority.
#[derive(Clone, Debug)]
pub struct SubscriptionCreate {
    /// Subscription NFT to register.
    pub subscription_id: NftId,
    /// Asset definition containing the selected plan.
    pub plan_id: AssetDefinitionId,
    /// Explicit billing trigger, or the deterministic subscription trigger when absent.
    pub billing_trigger_id: Option<TriggerId>,
    /// Explicit usage trigger for a usage-priced plan.
    pub usage_trigger_id: Option<TriggerId>,
    /// First charge time in UTC milliseconds, or the plan's resolved default.
    pub first_charge_ms: Option<u64>,
    /// Whether to grant the plan provider permission to record usage.
    pub grant_usage_to_provider: Option<bool>,
}

/// Usage-recording intent authorized by the bound account.
#[derive(Clone, Debug)]
pub struct SubscriptionUsage {
    /// Usage counter to update.
    pub unit_key: Name,
    /// Non-negative usage increment.
    pub delta: Quantity,
    /// Explicit usage trigger, or the deterministic subscription trigger when absent.
    pub usage_trigger_id: Option<TriggerId>,
}

/// Canonical unsigned transaction material returned by subscription preparation.
#[derive(Clone, Debug, norito::derive::JsonSerialize)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SubscriptionDraftArtifact {
    /// Exact payload and fee intent returned by Torii, ready for explicit signing.
    Payload(Box<iroha_data_model::transaction::TransactionPayload>),
    /// Exact instructions requiring explicit fee selection and transaction preparation.
    Instructions(Vec<iroha_data_model::isi::InstructionBox>),
}

/// Operation and resources to which a prepared subscription draft is bound.
#[derive(Clone, Debug, norito::derive::JsonSerialize)]
#[norito(tag = "action", content = "value", rename_all = "snake_case")]
pub enum SubscriptionDraftOperation {
    /// Register one subscription plan.
    Plan {
        /// Plan being registered.
        plan_id: AssetDefinitionId,
    },
    /// Register a subscription under one plan.
    Create {
        /// Subscription being registered.
        subscription_id: NftId,
        /// Selected plan.
        plan_id: AssetDefinitionId,
    },
    /// Pause a subscription.
    Pause {
        /// Subscription being paused.
        subscription_id: NftId,
    },
    /// Resume a subscription.
    Resume {
        /// Subscription being resumed.
        subscription_id: NftId,
    },
    /// Cancel a subscription using an explicit mode.
    Cancel {
        /// Subscription being canceled.
        subscription_id: NftId,
        /// Requested cancellation mode.
        mode: SubscriptionCancelMode,
    },
    /// Keep a subscription scheduled for cancellation.
    Keep {
        /// Subscription being kept.
        subscription_id: NftId,
    },
    /// Schedule a charge for a subscription.
    Charge {
        /// Subscription being charged.
        subscription_id: NftId,
    },
    /// Record one usage increment.
    Usage {
        /// Subscription whose usage changes.
        subscription_id: NftId,
        /// Usage counter being incremented.
        unit_key: Name,
        /// Exact increment.
        delta: Quantity,
    },
}

/// Decoded unsigned draft bound to its account, network, operation and resources.
///
/// This is a preparation result, never a submission receipt. Instruction drafts
/// retain the server's executable programs for explicit review before signing.
#[derive(Clone, Debug, norito::derive::JsonSerialize)]
pub struct SubscriptionDraft {
    network_id: iroha_data_model::NetworkId,
    authority: iroha_data_model::account::AccountId,
    operation: SubscriptionDraftOperation,
    artifact: SubscriptionDraftArtifact,
    resulting_subscription: Option<iroha_data_model::subscription::SubscriptionState>,
}

impl SubscriptionDraft {
    pub(crate) fn new(
        account: &crate::client::AccountClient,
        operation: SubscriptionDraftOperation,
        artifact: SubscriptionDraftArtifact,
        resulting_subscription: Option<iroha_data_model::subscription::SubscriptionState>,
    ) -> Self {
        Self {
            network_id: *account.network_id(),
            authority: account.authority().clone(),
            operation,
            artifact,
            resulting_subscription,
        }
    }
    /// Network to which this draft is bound.
    #[must_use]
    pub fn network_id(&self) -> &iroha_data_model::NetworkId {
        &self.network_id
    }
    /// Account that authorized draft preparation.
    #[must_use]
    pub fn authority(&self) -> &iroha_data_model::account::AccountId {
        &self.authority
    }
    /// Exact requested operation and resources.
    #[must_use]
    pub fn operation(&self) -> &SubscriptionDraftOperation {
        &self.operation
    }
    /// Decoded transaction material for explicit local signing or preparation.
    #[must_use]
    pub fn artifact(&self) -> &SubscriptionDraftArtifact {
        &self.artifact
    }
    /// Resulting subscription state when supplied by an instruction draft.
    #[must_use]
    pub fn resulting_subscription(
        &self,
    ) -> Option<&iroha_data_model::subscription::SubscriptionState> {
        self.resulting_subscription.as_ref()
    }
    /// Consume the preparation result and take its decoded transaction material.
    #[must_use]
    pub fn into_artifact(self) -> SubscriptionDraftArtifact {
        self.artifact
    }
}
