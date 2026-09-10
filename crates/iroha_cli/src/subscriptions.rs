//! Subscription queries and explicit unsigned draft preparation.
use crate::{Run, RunContext};
use eyre::{Result, WrapErr};
use iroha_model_base::name::Name;
use iroha::{
    blocking,
    data_model::{
        account::AccountId, asset::AssetDefinitionId,  nft::NftId, trigger::TriggerId,
    },
    subscriptions::{SubscriptionCreate, SubscriptionUsage},
};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::subscriptions::{
    SubscriptionCancelMode, SubscriptionListParams, SubscriptionPlanListParams,
};
use std::{fs, path::PathBuf};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Prepare or inspect subscription plans.
    #[command(subcommand)]
    Plan(PlanCommand),
    /// Query subscriptions and prepare unsigned billing drafts.
    #[command(subcommand)]
    Subscription(SubscriptionCommand),
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Plan(command) => command.run(context),
            Self::Subscription(command) => command.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum PlanCommand {
    /// Prepare an unsigned plan registration under the configured account.
    Prepare(PlanPrepareArgs),
    /// List subscription plans.
    List(PlanListArgs),
}
impl Run for PlanCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Prepare(args) => args.run(context),
            Self::List(args) => args.run(context),
        }
    }
}

fn account_client<C: RunContext>(context: &C) -> Result<blocking::AccountClient> {
    Ok(blocking::AccountClient::from_client(
        context.client_from_config()?.account_client()?,
    )?)
}
fn public_client<C: RunContext>(context: &C) -> Result<blocking::Client> {
    blocking::Client::from_client(context.client_from_config()?)
}
fn resolve_optional_account_id<C: RunContext>(
    context: &C,
    literal: Option<&str>,
    flag: &str,
) -> Result<Option<AccountId>> {
    literal
        .map(|literal| {
            crate::resolve_account_id(context, literal)
                .wrap_err_with(|| format!("failed to resolve {flag}"))
        })
        .transpose()
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum CountMode {
    /// Return page availability without an exact total.
    Bounded,
    /// Return exact matching totals.
    Exact,
}
impl CountMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bounded => "bounded",
            Self::Exact => "exact",
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct PlanPrepareArgs {
    /// Plan asset definition to register.
    #[arg(long, value_name = "ASSET_DEF_ID")]
    pub plan_id: AssetDefinitionId,
    /// JSON plan file; reads stdin when omitted.
    #[arg(long, value_name = "PATH")]
    pub plan_json: Option<PathBuf>,
}
impl Run for PlanPrepareArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let plan = load_plan(context, self.plan_json.as_ref())?;
        let draft = account_client(context)?
            .subscriptions()
            .prepare_plan(&self.plan_id, &plan)?;
        context.print_data(&draft)
    }
}

#[derive(clap::Args, Debug)]
pub struct PlanListArgs {
    /// Filter by provider account identifier or alias.
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub provider: Option<String>,
    /// Maximum page size.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Page offset.
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
    /// Select bounded or exact counting.
    #[arg(long, value_enum)]
    pub count_mode: Option<CountMode>,
}
impl Run for PlanListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let provider =
            resolve_optional_account_id(context, self.provider.as_deref(), "--provider")?
                .map(|id| id.to_string());
        let params = SubscriptionPlanListParams {
            provider,
            limit: self.limit,
            offset: self.offset,
            count_mode: self.count_mode.map(|mode| mode.as_str().to_owned()),
        };
        context.print_data(
            &public_client(context)?
                .subscriptions()
                .list_plans(&params)?,
        )
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum SubscriptionCommand {
    /// Prepare subscription creation under the configured account.
    Prepare(SubscriptionPrepareArgs),
    /// List matching subscriptions.
    List(SubscriptionListArgs),
    /// Read one subscription.
    Get(SubscriptionIdArgs),
    /// Prepare pausing billing.
    Pause(SubscriptionIdArgs),
    /// Prepare resuming billing.
    Resume(SubscriptionChargeArgs),
    /// Prepare cancellation using an explicit mode.
    Cancel(SubscriptionCancelArgs),
    /// Prepare keeping a subscription scheduled for cancellation.
    Keep(SubscriptionIdArgs),
    /// Prepare an immediate charge.
    ChargeNow(SubscriptionChargeArgs),
    /// Prepare a usage increment.
    Usage(SubscriptionUsageArgs),
}
impl Run for SubscriptionCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let draft = match self {
            Self::Prepare(args) => return args.run(context),
            Self::List(args) => return args.run(context),
            Self::Get(args) => {
                return context.print_data(
                    &public_client(context)?
                        .subscriptions()
                        .get(&args.subscription_id)?,
                );
            }
            Self::Pause(args) => account_client(context)?
                .subscriptions()
                .prepare_pause(&args.subscription_id)?,
            Self::Resume(args) => account_client(context)?
                .subscriptions()
                .prepare_resume(&args.subscription_id, args.charge_at_ms)?,
            Self::Cancel(args) => account_client(context)?
                .subscriptions()
                .prepare_cancel(&args.subscription_id, args.mode.into())?,
            Self::Keep(args) => account_client(context)?
                .subscriptions()
                .prepare_keep(&args.subscription_id)?,
            Self::ChargeNow(args) => account_client(context)?
                .subscriptions()
                .prepare_charge(&args.subscription_id, args.charge_at_ms)?,
            Self::Usage(args) => return args.run(context),
        };
        context.print_data(&draft)
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionPrepareArgs {
    /// Subscription NFT to register.
    #[arg(long)]
    pub subscription_id: NftId,
    /// Selected plan asset definition.
    #[arg(long)]
    pub plan_id: AssetDefinitionId,
    /// Explicit billing trigger; deterministic default when omitted.
    #[arg(long)]
    pub billing_trigger_id: Option<TriggerId>,
    /// Explicit usage trigger for usage pricing.
    #[arg(long)]
    pub usage_trigger_id: Option<TriggerId>,
    /// First charge timestamp in UTC milliseconds.
    #[arg(long)]
    pub first_charge_ms: Option<u64>,
    /// Grant the provider usage permission; true by default for usage pricing.
    #[arg(long, value_parser = clap::value_parser!(bool), action = clap::ArgAction::Set)]
    pub grant_usage_to_provider: Option<bool>,
}
impl SubscriptionPrepareArgs {
    fn into_intent(self) -> SubscriptionCreate {
        SubscriptionCreate {
            subscription_id: self.subscription_id,
            plan_id: self.plan_id,
            billing_trigger_id: self.billing_trigger_id,
            usage_trigger_id: self.usage_trigger_id,
            first_charge_ms: self.first_charge_ms,
            grant_usage_to_provider: self.grant_usage_to_provider,
        }
    }
}
impl Run for SubscriptionPrepareArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let draft = account_client(context)?
            .subscriptions()
            .prepare(&self.into_intent())?;
        context.print_data(&draft)
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionListArgs {
    /// Filter by subscriber account identifier or alias.
    #[arg(long)]
    pub owned_by: Option<String>,
    /// Filter by provider account identifier or alias.
    #[arg(long)]
    pub provider: Option<String>,
    /// Filter by subscription status.
    #[arg(long, value_parser = ["active", "paused", "past_due", "canceled", "suspended"])]
    pub status: Option<String>,
    /// Maximum page size.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Page offset.
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
    /// Select bounded or exact counting.
    #[arg(long, value_enum)]
    pub count_mode: Option<CountMode>,
}
impl Run for SubscriptionListArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let owned_by =
            resolve_optional_account_id(context, self.owned_by.as_deref(), "--owned-by")?
                .map(|id| id.to_string());
        let provider =
            resolve_optional_account_id(context, self.provider.as_deref(), "--provider")?
                .map(|id| id.to_string());
        let params = SubscriptionListParams {
            owned_by,
            provider,
            status: self.status,
            limit: self.limit,
            offset: self.offset,
            count_mode: self.count_mode.map(|mode| mode.as_str().to_owned()),
        };
        context.print_data(&public_client(context)?.subscriptions().list(&params)?)
    }
}

#[derive(clap::Args, Debug)]
pub struct SubscriptionIdArgs {
    /// Subscription NFT identifier.
    #[arg(long)]
    pub subscription_id: NftId,
}
#[derive(clap::Args, Debug)]
pub struct SubscriptionChargeArgs {
    /// Subscription NFT identifier.
    #[arg(long)]
    pub subscription_id: NftId,
    /// Explicit charge timestamp in UTC milliseconds.
    #[arg(long)]
    pub charge_at_ms: Option<u64>,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum CancelMode {
    /// Cancel immediately.
    Immediate,
    /// Cancel at the end of the current billing period.
    PeriodEnd,
}
impl From<CancelMode> for SubscriptionCancelMode {
    fn from(mode: CancelMode) -> Self {
        match mode {
            CancelMode::Immediate => Self::Immediate,
            CancelMode::PeriodEnd => Self::PeriodEnd,
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct SubscriptionCancelArgs {
    /// Subscription NFT identifier.
    #[arg(long)]
    pub subscription_id: NftId,
    /// Explicit cancellation mode.
    #[arg(long, value_enum)]
    pub mode: CancelMode,
}
#[derive(clap::Args, Debug)]
pub struct SubscriptionUsageArgs {
    /// Subscription NFT identifier.
    #[arg(long)]
    pub subscription_id: NftId,
    /// Usage counter to increment.
    #[arg(long)]
    pub unit_key: Name,
    /// Non-negative usage increment.
    #[arg(long)]
    pub delta: Quantity,
    /// Explicit usage trigger; deterministic default when omitted.
    #[arg(long)]
    pub usage_trigger_id: Option<TriggerId>,
}
impl Run for SubscriptionUsageArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let intent = SubscriptionUsage {
            unit_key: self.unit_key,
            delta: self.delta,
            usage_trigger_id: self.usage_trigger_id,
        };
        let draft = account_client(context)?
            .subscriptions()
            .prepare_usage(&self.subscription_id, &intent)?;
        context.print_data(&draft)
    }
}
fn load_plan<C: RunContext>(
    context: &C,
    path: Option<&PathBuf>,
) -> Result<iroha::data_model::subscription::SubscriptionPlan> {
    if let Some(path) = path {
        let payload = fs::read_to_string(path).wrap_err("failed to read subscription plan JSON")?;
        crate::parse_json(&payload)
    } else {
        crate::parse_json_stdin(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: Command,
    }
    const ID: &str = "sub-1$subscriptions.universal";

    #[test]
    fn action_arguments_match_their_exact_operation() {
        for action in ["pause", "keep", "get"] {
            assert!(
                Cli::try_parse_from(["iroha", "subscription", action, "--subscription-id", ID])
                    .is_ok()
            );
            for flag in ["--authority", "--charge-at-ms", "--mode"] {
                assert!(
                    Cli::try_parse_from([
                        "iroha",
                        "subscription",
                        action,
                        "--subscription-id",
                        ID,
                        flag,
                        "1"
                    ])
                    .is_err()
                );
            }
        }
        for action in ["resume", "charge-now"] {
            assert!(
                Cli::try_parse_from([
                    "iroha",
                    "subscription",
                    action,
                    "--subscription-id",
                    ID,
                    "--charge-at-ms",
                    "42"
                ])
                .is_ok()
            );
        }
        assert!(
            Cli::try_parse_from(["iroha", "subscription", "cancel", "--subscription-id", ID])
                .is_err()
        );
        for mode in ["immediate", "period-end"] {
            let cli = Cli::try_parse_from([
                "iroha",
                "subscription",
                "cancel",
                "--subscription-id",
                ID,
                "--mode",
                mode,
            ])
            .unwrap();
            let Command::Subscription(SubscriptionCommand::Cancel(args)) = cli.command else {
                panic!("cancel command");
            };
            assert_eq!(
                SubscriptionCancelMode::from(args.mode),
                if mode == "immediate" {
                    SubscriptionCancelMode::Immediate
                } else {
                    SubscriptionCancelMode::PeriodEnd
                }
            );
        }
    }

    #[test]
    fn pagination_count_modes_and_status_are_closed() {
        for group in ["plan", "subscription"] {
            for mode in ["exact", "bounded"] {
                assert!(
                    Cli::try_parse_from([
                        "iroha",
                        group,
                        "list",
                        "--count-mode",
                        mode,
                        "--limit",
                        "10",
                        "--offset",
                        "2"
                    ])
                    .is_ok()
                );
            }
            assert!(
                Cli::try_parse_from(["iroha", group, "list", "--count-mode", "unknown"]).is_err()
            );
        }
        assert!(
            Cli::try_parse_from(["iroha", "subscription", "list", "--status", "unknown"]).is_err()
        );
        assert_eq!(CountMode::Bounded.as_str(), "bounded");
        assert_eq!(CountMode::Exact.as_str(), "exact");
    }

    #[test]
    fn retired_creation_commands_and_authority_overrides_are_rejected() {
        assert!(Cli::try_parse_from(["iroha", "plan", "create"]).is_err());
        assert!(Cli::try_parse_from(["iroha", "subscription", "create"]).is_err());
        assert!(
            Cli::try_parse_from([
                "iroha",
                "subscription",
                "usage",
                "--subscription-id",
                ID,
                "--unit-key",
                "compute_ms",
                "--delta",
                "3",
                "--authority",
                "foreign"
            ])
            .is_err()
        );
    }
}
