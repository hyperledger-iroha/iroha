//! Viral incentive instruction handlers for SOC-2 (follow rewards and escrows).

use eyre::Result;
use iroha_config::parameters::actual::ViralIncentives;
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::AssetId,
    events::data::social::{
        SocialEvent, ViralEscrowCancelled, ViralEscrowCreated, ViralEscrowReleased,
        ViralRewardApplied,
    },
    nexus::UniversalAccountId,
    oracle::{KeyedHash, TwitterBindingStatus},
    prelude::*,
    social::{ViralCampaignBudget, ViralEscrowRecord, ViralRewardBudget},
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;

use super::{Error, Execute, asset::isi::assert_numeric_spec_with};
use crate::state::{StateTransaction, WorldTransaction};

const DAY_MS: u64 = 86_400_000;
const REJECT_HALTED: &str = "halted";
const REJECT_PROMO_WINDOW: &str = "promo_window";
const REJECT_BINDING_NOT_FOUND: &str = "binding_not_found";
const REJECT_BINDING_NOT_FOLLOW: &str = "binding_not_follow";
const REJECT_BINDING_EXPIRED: &str = "binding_expired";
const REJECT_DENY_UAID: &str = "deny_uaid";
const REJECT_DENY_BINDING: &str = "deny_binding";
const REJECT_DAILY_CAP: &str = "daily_cap";
const REJECT_BINDING_CAP: &str = "binding_cap";
const REJECT_CAMPAIGN_CAP: &str = "campaign_cap";
const REJECT_BUDGET_EXHAUSTED: &str = "budget_exhausted";
const REJECT_DUPLICATE_ESCROW: &str = "duplicate_escrow";
const REJECT_ZERO_AMOUNT: &str = "zero_amount";
const REJECT_ESCROW_MISSING: &str = "escrow_missing";
const REJECT_ESCROW_OWNER_MISMATCH: &str = "escrow_owner_mismatch";

#[cfg(feature = "telemetry")]
fn record_social_rejection(stx: &StateTransaction<'_, '_>, reason: &'static str) {
    crate::telemetry::record_social_rejection(stx.telemetry, reason);
}

#[cfg(not(feature = "telemetry"))]
fn record_social_rejection(_: &StateTransaction<'_, '_>, _: &'static str) {}

impl Execute for ClaimTwitterFollowReward {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let cfg = state_transaction.gov.viral_incentives.clone();

        let now_ms = state_transaction.block_unix_timestamp_ms();
        let promo = match promo_context(&cfg, now_ms) {
            Ok(promo) => promo,
            Err(err) => {
                let reason = if cfg.halt {
                    REJECT_HALTED
                } else {
                    REJECT_PROMO_WINDOW
                };
                record_social_rejection(state_transaction, reason);
                return Err(err);
            }
        };
        let day = current_day(now_ms);
        let binding_hash = self.binding_hash;
        let binding_digest = binding_hash.digest;

        let record = state_transaction
            .world
            .twitter_bindings
            .get(&binding_digest)
            .cloned()
            .ok_or_else(|| {
                record_social_rejection(state_transaction, REJECT_BINDING_NOT_FOUND);
                validation_err("twitter binding not found")
            })?;

        if record.attestation.status != TwitterBindingStatus::Following {
            record_social_rejection(state_transaction, REJECT_BINDING_NOT_FOLLOW);
            return Err(validation_err(
                "twitter binding is not a follow attestation",
            ));
        }
        if record.attestation.is_expired(now_ms) {
            record_social_rejection(state_transaction, REJECT_BINDING_EXPIRED);
            return Err(validation_err("twitter binding attestation expired"));
        }

        let uaid = record.attestation.uaid;
        ensure_not_denied(state_transaction, &cfg, uaid, &binding_hash)?;

        let reward_account = select_account_for_uaid(&state_transaction.world, uaid)?;
        enforce_daily_cap(state_transaction, &cfg, uaid, day)?;
        enforce_binding_cap(state_transaction, &cfg, binding_digest)?;

        let reward_asset_id = AssetId::new(
            cfg.reward_asset_definition_id.clone(),
            cfg.incentive_pool_account.clone(),
        );
        let recipient_asset = AssetId::new(
            cfg.reward_asset_definition_id.clone(),
            reward_account.clone(),
        );
        let spec = state_transaction
            .numeric_spec_for(reward_asset_id.definition())
            .map_err(Error::from)?;
        assert_numeric_spec_with(&cfg.follow_reward_amount, spec)?;
        let mut budget = refresh_budget(state_transaction, day);
        let mut campaign_budget = refresh_campaign_budget(state_transaction);
        let total = cfg.follow_reward_amount.clone();
        budget = consume_budget(state_transaction, &cfg, budget, total.clone())?;
        campaign_budget =
            consume_campaign_budget(state_transaction, &cfg, campaign_budget, total.clone())?;
        let campaign_cap = cfg.campaign_cap.clone();

        state_transaction
            .world
            .withdraw_numeric_asset(&reward_asset_id, &cfg.follow_reward_amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&recipient_asset, &cfg.follow_reward_amount)?;

        let mut payout_ctx = ViralPayoutContext {
            stx: state_transaction,
            cfg: &cfg,
            binding_hash: &binding_hash,
            budget: &mut budget,
            campaign: &mut campaign_budget,
            campaign_cap,
            promo,
            now_ms,
        };

        record_reward_claim(&mut payout_ctx, uaid, reward_account.clone(), &total);

        release_escrow_if_present(&mut payout_ctx, uaid, &reward_account)?;

        Ok(())
    }
}

impl Execute for SendToTwitter {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let cfg = state_transaction.gov.viral_incentives.clone();

        let now_ms = state_transaction.block_unix_timestamp_ms();
        let promo = match promo_context(&cfg, now_ms) {
            Ok(promo) => promo,
            Err(err) => {
                let reason = if cfg.halt {
                    REJECT_HALTED
                } else {
                    REJECT_PROMO_WINDOW
                };
                record_social_rejection(state_transaction, reason);
                return Err(err);
            }
        };
        let day = current_day(now_ms);
        let binding_hash = self.binding_hash;
        let binding_digest = binding_hash.digest;
        let amount = self.amount;

        if amount.is_zero() {
            record_social_rejection(state_transaction, REJECT_ZERO_AMOUNT);
            return Err(validation_err("send amount must be non-zero"));
        }

        let sender_asset = AssetId::new(cfg.reward_asset_definition_id.clone(), authority.clone());
        let escrow_asset = AssetId::new(
            cfg.reward_asset_definition_id.clone(),
            cfg.escrow_account.clone(),
        );
        let spec = state_transaction
            .numeric_spec_for(sender_asset.definition())
            .map_err(Error::from)?;
        assert_numeric_spec_with(&amount, spec)?;

        let binding_record = state_transaction
            .world
            .twitter_bindings
            .get(&binding_digest)
            .cloned();
        if let Some(record) = binding_record {
            if record.attestation.status == TwitterBindingStatus::Following
                && !record.attestation.is_expired(now_ms)
            {
                let uaid = record.attestation.uaid;
                ensure_not_denied(state_transaction, &cfg, uaid, &binding_hash)?;
                let reward_account = select_account_for_uaid(&state_transaction.world, uaid)?;
                let recipient_asset = AssetId::new(
                    cfg.reward_asset_definition_id.clone(),
                    reward_account.clone(),
                );

                state_transaction
                    .world
                    .withdraw_numeric_asset(&sender_asset, &amount)?;
                state_transaction
                    .world
                    .deposit_numeric_asset(&recipient_asset, &amount)?;

                let mut budget = refresh_budget(state_transaction, day);
                let mut campaign_budget = refresh_campaign_budget(state_transaction);
                let mut payout_ctx = ViralPayoutContext {
                    stx: state_transaction,
                    cfg: &cfg,
                    binding_hash: &binding_hash,
                    budget: &mut budget,
                    campaign: &mut campaign_budget,
                    campaign_cap: cfg.campaign_cap.clone(),
                    promo,
                    now_ms,
                };
                let bonus_paid = maybe_pay_bonus(&mut payout_ctx, authority, uaid)?;

                payout_ctx
                    .stx
                    .world
                    .emit_events(Some(SocialEvent::EscrowReleased(ViralEscrowReleased {
                        escrow: ViralEscrowRecord {
                            binding_hash: binding_hash.clone(),
                            sender: authority.clone(),
                            amount,
                            created_at_ms: now_ms,
                        },
                        uaid,
                        account: reward_account,
                        bonus_paid,
                        released_at_ms: now_ms,
                    })));
                return Ok(());
            }
        }

        if state_transaction
            .world
            .viral_escrows
            .get(&binding_digest)
            .is_some()
        {
            record_social_rejection(state_transaction, REJECT_DUPLICATE_ESCROW);
            return Err(validation_err("escrow already exists for binding"));
        }

        state_transaction
            .world
            .withdraw_numeric_asset(&sender_asset, &amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&escrow_asset, &amount)?;

        let record = ViralEscrowRecord {
            binding_hash: binding_hash.clone(),
            sender: authority.clone(),
            amount,
            created_at_ms: now_ms,
        };
        state_transaction
            .world
            .viral_escrows
            .insert(binding_digest, record.clone());
        state_transaction
            .world
            .emit_events(Some(SocialEvent::EscrowCreated(ViralEscrowCreated {
                escrow: record,
            })));
        Ok(())
    }
}

impl Execute for CancelTwitterEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let cfg = state_transaction.gov.viral_incentives.clone();
        if cfg.halt {
            record_social_rejection(state_transaction, REJECT_HALTED);
            return Err(validation_err("viral incentives are halted by governance"));
        }

        let binding_digest = self.binding_hash.digest;
        let Some(record) = state_transaction.world.viral_escrows.remove(binding_digest) else {
            record_social_rejection(state_transaction, REJECT_ESCROW_MISSING);
            return Err(validation_err("no escrow found for binding hash"));
        };

        if &record.sender != authority {
            record_social_rejection(state_transaction, REJECT_ESCROW_OWNER_MISMATCH);
            return Err(validation_err("only escrow sender may cancel it"));
        }

        let escrow_asset = AssetId::new(
            cfg.reward_asset_definition_id.clone(),
            cfg.escrow_account.clone(),
        );
        let sender_asset = AssetId::new(cfg.reward_asset_definition_id.clone(), authority.clone());

        state_transaction
            .world
            .withdraw_numeric_asset(&escrow_asset, &record.amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&sender_asset, &record.amount)?;

        state_transaction
            .world
            .emit_events(Some(SocialEvent::EscrowCancelled(ViralEscrowCancelled {
                escrow: record,
                cancelled_at_ms: state_transaction.block_unix_timestamp_ms(),
            })));
        Ok(())
    }
}

fn ensure_not_halted(cfg: &ViralIncentives) -> Result<(), Error> {
    if cfg.halt {
        return Err(validation_err("viral incentives are halted by governance"));
    }
    Ok(())
}

fn ensure_not_denied(
    stx: &StateTransaction<'_, '_>,
    cfg: &ViralIncentives,
    uaid: UniversalAccountId,
    binding: &KeyedHash,
) -> Result<(), Error> {
    if cfg.deny_uaids.iter().any(|denied| denied == &uaid) {
        record_social_rejection(stx, REJECT_DENY_UAID);
        return Err(validation_err("uaid is deny-listed for viral rewards"));
    }
    if cfg
        .deny_binding_digests
        .iter()
        .any(|digest| digest == &binding.digest)
    {
        record_social_rejection(stx, REJECT_DENY_BINDING);
        return Err(validation_err("binding is deny-listed for viral rewards"));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PromoContext {
    promo_active: bool,
    halted: bool,
}

fn promo_context(cfg: &ViralIncentives, now_ms: u64) -> Result<PromoContext, Error> {
    ensure_not_halted(cfg)?;
    ensure_promo_window(cfg, now_ms)?;
    Ok(PromoContext {
        promo_active: is_promo_active(cfg, now_ms),
        halted: cfg.halt,
    })
}

fn is_promo_active(cfg: &ViralIncentives, now_ms: u64) -> bool {
    let after_start = cfg.promo_starts_at_ms.is_none_or(|start| now_ms >= start);
    let before_end = cfg.promo_ends_at_ms.is_none_or(|end| now_ms < end);
    after_start && before_end
}

fn ensure_promo_window(cfg: &ViralIncentives, now_ms: u64) -> Result<(), Error> {
    if is_promo_active(cfg, now_ms) {
        return Ok(());
    }
    Err(validation_err("viral promotion window is not active"))
}

fn enforce_daily_cap(
    stx: &mut StateTransaction<'_, '_>,
    cfg: &ViralIncentives,
    uaid: UniversalAccountId,
    day: u64,
) -> Result<(), Error> {
    let mut counter = stx
        .world
        .viral_daily_counters
        .get(&uaid)
        .copied()
        .unwrap_or_default();
    if counter.day != day {
        counter.day = day;
        counter.claims = 0;
    }
    if counter.claims >= cfg.max_daily_claims_per_uaid {
        record_social_rejection(stx, REJECT_DAILY_CAP);
        return Err(validation_err("daily viral reward cap reached for UAID"));
    }
    counter.claims = counter.claims.saturating_add(1);
    stx.world.viral_daily_counters.insert(uaid, counter);
    Ok(())
}

fn enforce_binding_cap(
    stx: &mut StateTransaction<'_, '_>,
    cfg: &ViralIncentives,
    binding_digest: Hash,
) -> Result<(), Error> {
    let claims = stx
        .world
        .viral_binding_claims
        .get(&binding_digest)
        .copied()
        .unwrap_or(0);
    if claims >= cfg.max_claims_per_binding {
        record_social_rejection(stx, REJECT_BINDING_CAP);
        return Err(validation_err("viral reward already claimed for binding"));
    }
    stx.world
        .viral_binding_claims
        .insert(binding_digest, claims.saturating_add(1));
    Ok(())
}

fn refresh_budget(stx: &mut StateTransaction<'_, '_>, day: u64) -> ViralRewardBudget {
    let mut budget = stx.world.viral_reward_budget.get().clone();
    if budget.day != day {
        budget.day = day;
        budget.spent = Numeric::zero();
    }
    budget
}

fn refresh_campaign_budget(stx: &mut StateTransaction<'_, '_>) -> ViralCampaignBudget {
    stx.world.viral_campaign_budget.get().clone()
}

fn consume_budget(
    stx: &mut StateTransaction<'_, '_>,
    cfg: &ViralIncentives,
    mut budget: ViralRewardBudget,
    delta: Numeric,
) -> Result<ViralRewardBudget, Error> {
    if delta.is_zero() {
        return Ok(budget);
    }
    let spent = budget.spent.checked_add(delta).ok_or_else(|| {
        record_social_rejection(stx, REJECT_BUDGET_EXHAUSTED);
        validation_err("viral reward budget overflow")
    })?;
    if spent > cfg.daily_budget {
        record_social_rejection(stx, REJECT_BUDGET_EXHAUSTED);
        return Err(validation_err("viral reward budget exceeded"));
    }
    budget.spent = spent;
    *stx.world.viral_reward_budget.get_mut() = budget.clone();
    Ok(budget)
}

fn consume_campaign_budget(
    stx: &mut StateTransaction<'_, '_>,
    cfg: &ViralIncentives,
    mut campaign: ViralCampaignBudget,
    delta: Numeric,
) -> Result<ViralCampaignBudget, Error> {
    if delta.is_zero() {
        return Ok(campaign);
    }
    let spent = campaign.spent.checked_add(delta).ok_or_else(|| {
        record_social_rejection(stx, REJECT_CAMPAIGN_CAP);
        validation_err("viral campaign budget overflow")
    })?;
    if !cfg.campaign_cap.is_zero() && spent > cfg.campaign_cap {
        record_social_rejection(stx, REJECT_CAMPAIGN_CAP);
        return Err(validation_err("viral campaign budget exceeded"));
    }
    campaign.spent = spent;
    *stx.world.viral_campaign_budget.get_mut() = campaign.clone();
    Ok(campaign)
}

struct ViralPayoutContext<'tx, 'view, 'state> {
    stx: &'tx mut StateTransaction<'view, 'state>,
    cfg: &'tx ViralIncentives,
    binding_hash: &'tx KeyedHash,
    budget: &'tx mut ViralRewardBudget,
    campaign: &'tx mut ViralCampaignBudget,
    campaign_cap: Numeric,
    promo: PromoContext,
    now_ms: u64,
}

fn maybe_pay_bonus(
    ctx: &mut ViralPayoutContext<'_, '_, '_>,
    sender: &AccountId,
    uaid: UniversalAccountId,
) -> Result<bool, Error> {
    if ctx
        .stx
        .world
        .viral_bonus_paid
        .get(&ctx.binding_hash.digest)
        .copied()
        .unwrap_or(false)
    {
        return Ok(false);
    }
    if ctx.cfg.sender_bonus_amount.is_zero() {
        return Ok(false);
    }

    let pool_asset = AssetId::new(
        ctx.cfg.reward_asset_definition_id.clone(),
        ctx.cfg.incentive_pool_account.clone(),
    );
    let sender_asset = AssetId::new(ctx.cfg.reward_asset_definition_id.clone(), sender.clone());
    let spec = ctx
        .stx
        .numeric_spec_for(pool_asset.definition())
        .map_err(Error::from)?;
    assert_numeric_spec_with(&ctx.cfg.sender_bonus_amount, spec)?;
    let bonus = ctx.cfg.sender_bonus_amount.clone();
    let campaign_cap = ctx.campaign_cap.clone();
    *ctx.budget = consume_budget(ctx.stx, ctx.cfg, ctx.budget.clone(), bonus.clone())?;
    *ctx.campaign = consume_campaign_budget(ctx.stx, ctx.cfg, ctx.campaign.clone(), bonus.clone())?;

    ctx.stx.world.withdraw_numeric_asset(&pool_asset, &bonus)?;
    ctx.stx.world.deposit_numeric_asset(&sender_asset, &bonus)?;

    ctx.stx
        .world
        .viral_bonus_paid
        .insert(ctx.binding_hash.digest, true);
    ctx.stx
        .world
        .emit_events(Some(SocialEvent::RewardPaid(ViralRewardApplied {
            uaid,
            account: sender.clone(),
            binding_hash: ctx.binding_hash.clone(),
            amount: bonus,
            budget: ctx.budget.clone(),
            campaign: Some(ctx.campaign.clone()),
            campaign_cap,
            promo_active: ctx.promo.promo_active,
            halted: ctx.promo.halted,
            recorded_at_ms: ctx.now_ms,
        })));
    Ok(true)
}

fn record_reward_claim(
    ctx: &mut ViralPayoutContext<'_, '_, '_>,
    uaid: UniversalAccountId,
    account: AccountId,
    amount: &Numeric,
) {
    ctx.stx
        .world
        .emit_events(Some(SocialEvent::RewardPaid(ViralRewardApplied {
            uaid,
            account,
            binding_hash: ctx.binding_hash.clone(),
            amount: amount.clone(),
            budget: ctx.budget.clone(),
            campaign: Some(ctx.campaign.clone()),
            campaign_cap: ctx.campaign_cap.clone(),
            promo_active: ctx.promo.promo_active,
            halted: ctx.promo.halted,
            recorded_at_ms: ctx.now_ms,
        })));
}

fn release_escrow_if_present(
    ctx: &mut ViralPayoutContext<'_, '_, '_>,
    uaid: UniversalAccountId,
    account: &AccountId,
) -> Result<(), Error> {
    let Some(escrow) = ctx.stx.world.viral_escrows.remove(ctx.binding_hash.digest) else {
        return Ok(());
    };

    let escrow_asset = AssetId::new(
        ctx.cfg.reward_asset_definition_id.clone(),
        ctx.cfg.escrow_account.clone(),
    );
    let recipient_asset = AssetId::new(ctx.cfg.reward_asset_definition_id.clone(), account.clone());
    ctx.stx
        .world
        .withdraw_numeric_asset(&escrow_asset, &escrow.amount)?;
    ctx.stx
        .world
        .deposit_numeric_asset(&recipient_asset, &escrow.amount)?;

    let bonus_paid = maybe_pay_bonus(ctx, &escrow.sender, uaid)?;

    ctx.stx
        .world
        .emit_events(Some(SocialEvent::EscrowReleased(ViralEscrowReleased {
            escrow,
            uaid,
            account: account.clone(),
            bonus_paid,
            released_at_ms: ctx.now_ms,
        })));

    Ok(())
}

fn select_account_for_uaid(
    world: &WorldTransaction<'_, '_>,
    uaid: UniversalAccountId,
) -> Result<AccountId, Error> {
    world
        .uaid_accounts
        .get(&uaid)
        .cloned()
        .ok_or_else(|| validation_err("no account registered for UAID"))
}

const fn current_day(timestamp_ms: u64) -> u64 {
    timestamp_ms / DAY_MS
}

fn validation_err(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{block::BlockHeader, prelude::*};
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[test]
    fn select_account_for_uaid_uses_index() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query);

        let domain_id: DomainId = "uaid.reward".parse().expect("domain id");
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::reward"));
        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::domain(Domain::new(domain_id))
            .execute(&ALICE_ID, &mut tx)
            .expect("register domain");
        Register::account(new_account)
            .execute(&ALICE_ID, &mut tx)
            .expect("register account");

        let selected = select_account_for_uaid(&tx.world, uaid).expect("select account");
        assert_eq!(selected, account_id);
    }
}
