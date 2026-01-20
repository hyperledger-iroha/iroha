//! Subscription plan and billing helpers.

use std::{fs, path::PathBuf};

use eyre::{Result, WrapErr};
use iroha::{
    client::Client,
    data_model::account::AccountId,
    subscriptions::{
        SubscriptionActionRequest, SubscriptionCancelMode, SubscriptionCreateRequest,
        SubscriptionListParams, SubscriptionPlanCreateRequest, SubscriptionPlanListParams,
        SubscriptionUsageRequest,
    },
};
use iroha_crypto::PrivateKey;
use iroha_primitives::numeric::Numeric;

use crate::{Run, RunContext};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Manage subscription plans (asset definition metadata).
    #[command(subcommand)]
    Plan(PlanCommand),
    /// Manage subscriptions and billing actions.
    #[command(subcommand)]
    Subscription(SubscriptionCommand),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Plan(cmd) => cmd.run(context),
            Command::Subscription(cmd) => cmd.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum PlanCommand {
    /// Register a subscription plan on an asset definition.
    Create(PlanCreateArgs),
    /// List subscription plans, optionally filtered by provider.
    List(PlanListArgs),
}

impl Run for PlanCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            PlanCommand::Create(args) => args.run(context),
            PlanCommand::List(args) => args.run(context),
        }
    }
}

fn resolve_account_id_arg<C: RunContext>(context: &C, literal: &str, flag: &str) -> Result<AccountId> {
    crate::resolve_account_id(context, literal)
        .wrap_err_with(|| format!("failed to resolve {flag}"))
}

fn resolve_optional_account_id<C: RunContext>(
    context: &C,
    literal: Option<&str>,
    flag: &str,
) -> Result<Option<AccountId>> {
    literal
        .map(|value| resolve_account_id_arg(context, value, flag))
        .transpose()
}

#[derive(clap::Args, Debug)]
pub struct PlanCreateArgs {
    /// Authority account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub authority: String,
    /// Hex-encoded private key for signing.
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Asset definition id where the plan metadata is stored.
    #[arg(long, value_name = "ASSET_DEF_ID")]
    pub plan_id: iroha::data_model::asset::AssetDefinitionId,
    /// Path to JSON plan payload (reads stdin when omitted).
    #[arg(long, value_name = "PATH")]
    pub plan_json: Option<PathBuf>,
}

impl Run for PlanCreateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let plan = load_plan(context, self.plan_json.as_ref())?;
        let authority = resolve_account_id_arg(context, &self.authority, "--authority")?;
        let request = self.to_request(authority, plan)?;
        let response = client.create_subscription_plan(&request)?;
        context.print_data(&response)
    }
}

impl PlanCreateArgs {
    fn to_request(
        &self,
        authority: AccountId,
        plan: iroha::data_model::subscription::SubscriptionPlan,
    ) -> Result<SubscriptionPlanCreateRequest> {
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        Ok(SubscriptionPlanCreateRequest {
            authority,
            private_key: iroha::data_model::prelude::ExposedPrivateKey(private_key),
            plan_id: self.plan_id.clone(),
            plan,
        })
    }
}

#[derive(clap::Args, Debug)]
pub struct PlanListArgs {
    /// Filter by plan provider (account id).
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub provider: Option<String>,
    /// Limit number of results.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
}

impl Run for PlanListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let provider = resolve_optional_account_id(context, self.provider.as_deref(), "--provider")?
            .map(|account| account.to_string());
        let params = self.to_params(provider);
        let response = client.list_subscription_plans(&params)?;
        context.print_data(&response)
    }
}

impl PlanListArgs {
    fn to_params(&self, provider: Option<String>) -> SubscriptionPlanListParams {
        SubscriptionPlanListParams {
            provider,
            limit: self.limit,
            offset: self.offset,
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum SubscriptionCommand {
    /// Create a subscription and billing trigger.
    Create(SubscriptionCreateArgs),
    /// List subscriptions with optional filters.
    List(SubscriptionListArgs),
    /// Fetch a subscription by id.
    Get(SubscriptionGetArgs),
    /// Pause billing for a subscription.
    Pause(SubscriptionActionArgs),
    /// Resume billing for a subscription.
    Resume(SubscriptionActionArgs),
    /// Cancel a subscription and remove its billing trigger.
    Cancel(SubscriptionActionArgs),
    /// Undo a scheduled period-end cancellation.
    Keep(SubscriptionActionArgs),
    /// Execute billing immediately.
    ChargeNow(SubscriptionActionArgs),
    /// Record usage for a subscription usage plan.
    Usage(SubscriptionUsageArgs),
}

impl Run for SubscriptionCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            SubscriptionCommand::Create(args) => args.run(context),
            SubscriptionCommand::List(args) => args.run(context),
            SubscriptionCommand::Get(args) => args.run(context),
            SubscriptionCommand::Pause(args) => {
                let client: Client = context.client_from_config();
                let authority = resolve_account_id_arg(context, &args.authority, "--authority")?;
                let request = args.to_request(authority)?;
                let response = client.pause_subscription(&args.subscription_id, &request)?;
                context.print_data(&response)
            }
            SubscriptionCommand::Resume(args) => {
                let client: Client = context.client_from_config();
                let authority = resolve_account_id_arg(context, &args.authority, "--authority")?;
                let request = args.to_request(authority)?;
                let response = client.resume_subscription(&args.subscription_id, &request)?;
                context.print_data(&response)
            }
            SubscriptionCommand::Cancel(args) => {
                let client: Client = context.client_from_config();
                let authority = resolve_account_id_arg(context, &args.authority, "--authority")?;
                let request = args.to_request(authority)?;
                let response = client.cancel_subscription(&args.subscription_id, &request)?;
                context.print_data(&response)
            }
            SubscriptionCommand::Keep(args) => {
                let client: Client = context.client_from_config();
                let authority = resolve_account_id_arg(context, &args.authority, "--authority")?;
                let request = args.to_request(authority)?;
                let response = client.keep_subscription(&args.subscription_id, &request)?;
                context.print_data(&response)
            }
            SubscriptionCommand::ChargeNow(args) => {
                let client: Client = context.client_from_config();
                let authority = resolve_account_id_arg(context, &args.authority, "--authority")?;
                let request = args.to_request(authority)?;
                let response = client.charge_subscription_now(&args.subscription_id, &request)?;
                context.print_data(&response)
            }
            SubscriptionCommand::Usage(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionCreateArgs {
    /// Authority account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub authority: String,
    /// Hex-encoded private key for signing.
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Subscription NFT id to register.
    #[arg(long, value_name = "NFT_ID")]
    pub subscription_id: iroha::data_model::nft::NftId,
    /// Subscription plan asset definition id.
    #[arg(long, value_name = "ASSET_DEF_ID")]
    pub plan_id: iroha::data_model::asset::AssetDefinitionId,
    /// Optional billing trigger id to use.
    #[arg(long)]
    pub billing_trigger_id: Option<iroha::data_model::trigger::TriggerId>,
    /// Optional usage trigger id to use (usage plans only).
    #[arg(long)]
    pub usage_trigger_id: Option<iroha::data_model::trigger::TriggerId>,
    /// Optional first charge timestamp in UTC milliseconds.
    #[arg(long)]
    pub first_charge_ms: Option<u64>,
    /// Grant usage reporting permission to the plan provider.
    #[arg(long)]
    pub grant_usage_to_provider: Option<bool>,
}

impl Run for SubscriptionCreateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let authority = resolve_account_id_arg(context, &self.authority, "--authority")?;
        let request = self.to_request(authority)?;
        let response = client.create_subscription(&request)?;
        context.print_data(&response)
    }
}

impl SubscriptionCreateArgs {
    fn to_request(&self, authority: AccountId) -> Result<SubscriptionCreateRequest> {
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        Ok(SubscriptionCreateRequest {
            authority,
            private_key: iroha::data_model::prelude::ExposedPrivateKey(private_key),
            subscription_id: self.subscription_id.clone(),
            plan_id: self.plan_id.clone(),
            billing_trigger_id: self.billing_trigger_id.clone(),
            usage_trigger_id: self.usage_trigger_id.clone(),
            first_charge_ms: self.first_charge_ms,
            grant_usage_to_provider: self.grant_usage_to_provider,
        })
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionListArgs {
    /// Filter by subscriber account.
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub owned_by: Option<String>,
    /// Filter by plan provider account.
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub provider: Option<String>,
    /// Filter by status (active, paused, `past_due`, canceled, suspended).
    #[arg(long)]
    pub status: Option<String>,
    /// Limit number of results.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Offset for pagination (default 0).
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
}

impl Run for SubscriptionListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let owned_by =
            resolve_optional_account_id(context, self.owned_by.as_deref(), "--owned-by")?
                .map(|account| account.to_string());
        let provider =
            resolve_optional_account_id(context, self.provider.as_deref(), "--provider")?
                .map(|account| account.to_string());
        let params = self.to_params(owned_by, provider);
        let response = client.list_subscriptions(&params)?;
        context.print_data(&response)
    }
}

impl SubscriptionListArgs {
    fn to_params(&self, owned_by: Option<String>, provider: Option<String>) -> SubscriptionListParams {
        SubscriptionListParams {
            owned_by,
            provider,
            status: self.status.clone(),
            limit: self.limit,
            offset: self.offset,
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionGetArgs {
    /// Subscription NFT id.
    #[arg(long, value_name = "NFT_ID")]
    pub subscription_id: iroha::data_model::nft::NftId,
}

impl Run for SubscriptionGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let response = client.get_subscription(&self.subscription_id)?;
        context.print_data(&response)
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionActionArgs {
    /// Subscription NFT id.
    #[arg(long, value_name = "NFT_ID")]
    pub subscription_id: iroha::data_model::nft::NftId,
    /// Authority account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub authority: String,
    /// Hex-encoded private key for signing.
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Optional charge time override in UTC milliseconds.
    #[arg(long)]
    pub charge_at_ms: Option<u64>,
    /// Cancel at the end of the current billing period (cancel only).
    #[arg(long)]
    pub cancel_at_period_end: bool,
}

impl SubscriptionActionArgs {
    fn to_request(&self, authority: AccountId) -> Result<SubscriptionActionRequest> {
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        Ok(SubscriptionActionRequest {
            authority,
            private_key: iroha::data_model::prelude::ExposedPrivateKey(private_key),
            charge_at_ms: self.charge_at_ms,
            cancel_mode: self
                .cancel_at_period_end
                .then_some(SubscriptionCancelMode::PeriodEnd),
        })
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionUsageArgs {
    /// Subscription NFT id.
    #[arg(long, value_name = "NFT_ID")]
    pub subscription_id: iroha::data_model::nft::NftId,
    /// Authority account identifier (IH58/compressed/0x, uaid:, opaque:, or <alias|public_key>@domain).
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub authority: String,
    /// Hex-encoded private key for signing.
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Usage counter key to update.
    #[arg(long)]
    pub unit_key: iroha::data_model::name::Name,
    /// Usage increment (must be non-negative).
    #[arg(long)]
    pub delta: Numeric,
    /// Optional usage trigger id override.
    #[arg(long)]
    pub usage_trigger_id: Option<iroha::data_model::trigger::TriggerId>,
}

impl Run for SubscriptionUsageArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let authority = resolve_account_id_arg(context, &self.authority, "--authority")?;
        let request = self.to_request(authority)?;
        let response = client.record_subscription_usage(&self.subscription_id, &request)?;
        context.print_data(&response)
    }
}

impl SubscriptionUsageArgs {
    fn to_request(&self, authority: AccountId) -> Result<SubscriptionUsageRequest> {
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        Ok(SubscriptionUsageRequest {
            authority,
            private_key: iroha::data_model::prelude::ExposedPrivateKey(private_key),
            unit_key: self.unit_key.clone(),
            delta: self.delta.clone(),
            usage_trigger_id: self.usage_trigger_id.clone(),
        })
    }
}

fn load_plan<C: RunContext>(
    context: &C,
    path: Option<&PathBuf>,
) -> Result<iroha::data_model::subscription::SubscriptionPlan> {
    if let Some(path) = path {
        let payload =
            fs::read_to_string(path).wrap_err("failed to read subscription plan JSON")?;
        crate::parse_json(&payload)
    } else {
        crate::parse_json_stdin(context)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use iroha::{
        data_model::{
            account::AccountId,
            asset::AssetDefinitionId,
            name::Name,
            nft::NftId,
            prelude::ExposedPrivateKey,
            subscription::{
                SubscriptionBillFor, SubscriptionBilling, SubscriptionCadence,
                SubscriptionFixedPeriodCadence, SubscriptionFixedPricing, SubscriptionPlan,
                SubscriptionPricing,
            },
            trigger::TriggerId,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
    use iroha_primitives::numeric::Numeric;

    use super::*;

    struct DummyContext;

    impl RunContext for DummyContext {
        fn config(&self) -> &crate::Config {
            unreachable!("dummy context does not provide config")
        }

        fn transaction_metadata(&self) -> Option<&crate::Metadata> {
            unreachable!("dummy context does not provide metadata")
        }

        fn input_instructions(&self) -> bool {
            unreachable!("dummy context does not read stdin")
        }

        fn output_instructions(&self) -> bool {
            unreachable!("dummy context does not write stdout")
        }

        fn i18n(&self) -> &crate::Localizer {
            unreachable!("dummy context does not provide i18n")
        }

        fn print_data<T>(&mut self, _data: &T) -> Result<()>
        where
            T: crate::JsonSerialize + ?Sized,
        {
            unreachable!("dummy context does not print")
        }

        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            unreachable!("dummy context does not print")
        }
    }

    fn sample_private_key() -> (PrivateKey, String) {
        let key_pair = KeyPair::from_seed(vec![1_u8; 32], Algorithm::Ed25519);
        let private_key = key_pair.private_key().clone();
        let private_key_str = ExposedPrivateKey(private_key.clone()).to_string();
        (private_key, private_key_str)
    }

    fn sample_account_id(seed: u8) -> AccountId {
        let domain = "wonderland".parse().expect("domain");
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(domain, key_pair.public_key().clone())
    }

    fn sample_plan(provider: AccountId, asset_definition: AssetDefinitionId) -> SubscriptionPlan {
        SubscriptionPlan {
            provider,
            billing: SubscriptionBilling {
                cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                    period_ms: 1_000,
                }),
                bill_for: SubscriptionBillFor::PreviousPeriod,
                retry_backoff_ms: 100,
                max_failures: 3,
                grace_ms: 0,
            },
            pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
                amount: Numeric::new(5_u32, 0),
                asset_definition,
            }),
        }
    }

    #[test]
    fn plan_create_args_build_request() {
        let provider = sample_account_id(1);
        let plan_id: AssetDefinitionId = "plan#commerce".parse().expect("plan id");
        let asset_definition: AssetDefinitionId = "usd#pay".parse().expect("asset def");
        let plan = sample_plan(provider.clone(), asset_definition);
        let (private_key, private_key_str) = sample_private_key();
        let args = PlanCreateArgs {
            authority: provider.to_string(),
            private_key: private_key_str,
            plan_id: plan_id.clone(),
            plan_json: None,
        };

        let request = args.to_request(provider.clone(), plan.clone()).expect("request");
        assert_eq!(request.authority, provider);
        assert_eq!(request.plan_id, plan_id);
        assert_eq!(request.plan, plan);
        assert_eq!(request.private_key, ExposedPrivateKey(private_key));
    }

    #[test]
    fn plan_list_args_build_params() {
        let provider = sample_account_id(1).to_string();
        let args = PlanListArgs {
            provider: Some(provider.clone()),
            limit: Some(5),
            offset: 2,
        };
        let params = args.to_params(args.provider.clone());
        assert_eq!(params.provider.as_deref(), Some(provider.as_str()));
        assert_eq!(params.limit, Some(5));
        assert_eq!(params.offset, 2);
    }

    #[test]
    fn subscription_create_args_build_request() {
        let subscriber = sample_account_id(2);
        let plan_id: AssetDefinitionId = "plan#commerce".parse().expect("plan id");
        let subscription_id: NftId = "sub-1$subscriptions".parse().expect("subscription id");
        let billing_trigger_id: TriggerId = "sub-1-bill".parse().expect("billing trigger");
        let usage_trigger_id: TriggerId = "sub-1-usage".parse().expect("usage trigger");
        let (private_key, private_key_str) = sample_private_key();
        let args = SubscriptionCreateArgs {
            authority: subscriber.to_string(),
            private_key: private_key_str,
            subscription_id: subscription_id.clone(),
            plan_id: plan_id.clone(),
            billing_trigger_id: Some(billing_trigger_id.clone()),
            usage_trigger_id: Some(usage_trigger_id.clone()),
            first_charge_ms: Some(1_700),
            grant_usage_to_provider: Some(true),
        };

        let request = args.to_request(subscriber.clone()).expect("request");
        assert_eq!(request.authority, subscriber);
        assert_eq!(request.subscription_id, subscription_id);
        assert_eq!(request.plan_id, plan_id);
        assert_eq!(request.billing_trigger_id, Some(billing_trigger_id));
        assert_eq!(request.usage_trigger_id, Some(usage_trigger_id));
        assert_eq!(request.first_charge_ms, Some(1_700));
        assert_eq!(request.grant_usage_to_provider, Some(true));
        assert_eq!(request.private_key, ExposedPrivateKey(private_key));
    }

    #[test]
    fn subscription_list_args_build_params() {
        let owned_by = sample_account_id(2).to_string();
        let provider = sample_account_id(1).to_string();
        let args = SubscriptionListArgs {
            owned_by: Some(owned_by.clone()),
            provider: Some(provider.clone()),
            status: Some("active".to_string()),
            limit: Some(10),
            offset: 4,
        };
        let params = args.to_params(args.owned_by.clone(), args.provider.clone());
        assert_eq!(params.owned_by.as_deref(), Some(owned_by.as_str()));
        assert_eq!(params.provider.as_deref(), Some(provider.as_str()));
        assert_eq!(params.status.as_deref(), Some("active"));
        assert_eq!(params.limit, Some(10));
        assert_eq!(params.offset, 4);
    }

    #[test]
    fn subscription_action_args_build_request() {
        let subscriber = sample_account_id(2);
        let subscription_id: NftId = "sub-1$subscriptions".parse().expect("subscription id");
        let (private_key, private_key_str) = sample_private_key();
        let args = SubscriptionActionArgs {
            subscription_id,
            authority: subscriber.to_string(),
            private_key: private_key_str,
            charge_at_ms: Some(500),
            cancel_at_period_end: false,
        };

        let request = args.to_request(subscriber.clone()).expect("request");
        assert_eq!(request.authority, subscriber);
        assert_eq!(request.charge_at_ms, Some(500));
        assert_eq!(request.private_key, ExposedPrivateKey(private_key));
    }

    #[test]
    fn subscription_usage_args_build_request() {
        let subscriber = sample_account_id(2);
        let subscription_id: NftId = "sub-1$subscriptions".parse().expect("subscription id");
        let unit_key: Name = "compute_ms".parse().expect("unit key");
        let usage_trigger_id: TriggerId = "usage-1".parse().expect("usage trigger");
        let (private_key, private_key_str) = sample_private_key();
        let args = SubscriptionUsageArgs {
            subscription_id,
            authority: subscriber.to_string(),
            private_key: private_key_str,
            unit_key: unit_key.clone(),
            delta: Numeric::new(4_u32, 0),
            usage_trigger_id: Some(usage_trigger_id.clone()),
        };

        let request = args.to_request(subscriber.clone()).expect("request");
        assert_eq!(request.authority, subscriber);
        assert_eq!(request.unit_key, unit_key);
        assert_eq!(request.delta, Numeric::new(4_u32, 0));
        assert_eq!(request.usage_trigger_id, Some(usage_trigger_id));
        assert_eq!(request.private_key, ExposedPrivateKey(private_key));
    }

    #[test]
    fn load_plan_reads_json_file() {
        let provider = sample_account_id(1);
        let asset_definition: AssetDefinitionId = "usd#pay".parse().expect("asset def");
        let plan = sample_plan(provider, asset_definition);
        let payload = norito::json::to_json(&plan).expect("encode plan");

        let mut path = std::env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("iroha_cli_subscription_plan_{ts}.json"));
        fs::write(&path, payload).expect("write plan json");

        let context = DummyContext;
        let parsed = load_plan(&context, Some(&path)).expect("load plan");
        assert_eq!(parsed, plan);

        let _ = fs::remove_file(path);
    }
}
