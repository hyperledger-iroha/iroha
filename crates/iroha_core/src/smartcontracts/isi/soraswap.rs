//! Metadata-backed SoraSwap DeFi instruction handlers.

use iroha_data_model::{
    events::data::prelude::{AccountEvent, MetadataChanged},
    isi::{
        error::InstructionExecutionError,
        soraswap::{
            ConfigureSoraSwapDlmmHook, RecordSoraSwapHookExecution,
            RecordSoraSwapOperatorHeartbeat, RecordSoraSwapVaultRequest,
            RegisterSoraSwapMarginMarket, RegisterSoraSwapOperator, RegisterSoraSwapRwaMarket,
            RegisterSoraSwapVault, ReportSoraSwapRwaNav, SettleSoraSwapIntent,
            SoraSwapInstructionBox, SubmitSoraSwapIntent, UpdateSoraSwapMarginAccount,
        },
    },
    prelude::*,
};

use super::prelude::*;

fn invalid(message: impl Into<String>) -> Error {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

fn metadata_key(module: &str, id: &Name) -> Result<Name, Error> {
    format!("soraswap/{module}/{id}")
        .parse()
        .map_err(|_| invalid(format!("invalid SoraSwap metadata key for {module}/{id}")))
}

fn write_account_record(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: AccountId,
    key: Name,
    value: Json,
) -> Result<(), Error> {
    crate::smartcontracts::limits::enforce_json_size(
        state_transaction,
        &value,
        "max_metadata_value_bytes",
        crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
    )?;
    state_transaction
        .world
        .account_mut(&account)
        .map_err(Error::from)
        .map(|entry| entry.insert(key.clone(), value.clone()))?;
    state_transaction
        .world
        .emit_events(Some(AccountEvent::MetadataInserted(MetadataChanged {
            target: account,
            key,
            value,
        })));
    Ok(())
}

fn ensure_non_zero(value: &Numeric, label: &str) -> Result<(), Error> {
    if value.is_zero() || value.mantissa().is_negative() {
        return Err(invalid(format!("{label} must be greater than zero")));
    }
    Ok(())
}

fn ensure_bps(value: u16, label: &str) -> Result<(), Error> {
    if value > 10_000 {
        return Err(invalid(format!("{label} exceeds 10000 bps")));
    }
    Ok(())
}

fn record_string(module: &str, action: &str, pairs: &[(&str, String)]) -> Json {
    let mut value = format!("module={module};action={action}");
    for (key, item) in pairs {
        value.push(';');
        value.push_str(key);
        value.push('=');
        value.push_str(item);
    }
    Json::new(value)
}

impl Execute for SubmitSoraSwapIntent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_non_zero(&self.amount_in, "intent amount_in")?;
        ensure_non_zero(&self.min_out, "intent min_out")?;
        ensure_bps(self.solver_fee_bps, "solver fee")?;
        state_transaction
            .world
            .asset_definition(&self.input_asset)?;
        state_transaction
            .world
            .asset_definition(&self.output_asset)?;

        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("intent", &self.intent_id)?,
            record_string(
                "intent",
                "submit",
                &[
                    ("intent_id", self.intent_id.to_string()),
                    ("input_asset", self.input_asset.to_string()),
                    ("output_asset", self.output_asset.to_string()),
                    ("amount_in", self.amount_in.to_string()),
                    ("min_out", self.min_out.to_string()),
                    ("solver_fee_bps", self.solver_fee_bps.to_string()),
                    ("deadline_slot", self.deadline_slot.to_string()),
                    ("nonce", self.nonce.to_string()),
                    ("owner", authority.to_string()),
                ],
            ),
        )
    }
}

impl Execute for SettleSoraSwapIntent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &self.solver != authority {
            return Err(invalid("intent settlement solver must match authority"));
        }
        ensure_non_zero(&self.amount_out, "intent amount_out")?;
        state_transaction.world.account(&self.owner)?;
        state_transaction.world.account(&self.solver)?;
        write_account_record(
            state_transaction,
            self.owner.clone(),
            metadata_key("intent", &self.intent_id)?,
            record_string(
                "intent",
                "settle",
                &[
                    ("intent_id", self.intent_id.to_string()),
                    ("solver", self.solver.to_string()),
                    ("amount_out", self.amount_out.to_string()),
                    ("fill_slot", self.fill_slot.to_string()),
                    ("status", self.status.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RegisterSoraSwapVault {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction
            .world
            .asset_definition(&self.underlying_asset)?;
        state_transaction
            .world
            .asset_definition(&self.share_asset)?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("vault", &self.vault_id)?,
            record_string(
                "vault",
                "register",
                &[
                    ("vault_id", self.vault_id.to_string()),
                    ("underlying_asset", self.underlying_asset.to_string()),
                    ("share_asset", self.share_asset.to_string()),
                    ("strategy", self.strategy.to_string()),
                    ("async_redeem", self.async_redeem.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RecordSoraSwapVaultRequest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &self.account != authority {
            return Err(invalid("vault request account must match authority"));
        }
        ensure_non_zero(&self.amount, "vault request amount")?;
        state_transaction.world.account(&self.account)?;
        write_account_record(
            state_transaction,
            self.account.clone(),
            metadata_key("vault_request", &self.request_id)?,
            record_string(
                "vault",
                "request",
                &[
                    ("vault_id", self.vault_id.to_string()),
                    ("request_id", self.request_id.to_string()),
                    ("amount", self.amount.to_string()),
                    ("claim_slot", self.claim_slot.to_string()),
                    ("request_kind", self.request_kind.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RegisterSoraSwapOperator {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &self.operator != authority {
            return Err(invalid("operator registration must match authority"));
        }
        ensure_non_zero(&self.min_bond, "operator min_bond")?;
        state_transaction.world.asset_definition(&self.bond_asset)?;
        write_account_record(
            state_transaction,
            self.operator.clone(),
            metadata_key("operator", &self.service)?,
            record_string(
                "operator",
                "register",
                &[
                    ("operator", self.operator.to_string()),
                    ("service", self.service.to_string()),
                    ("bond_asset", self.bond_asset.to_string()),
                    ("min_bond", self.min_bond.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RecordSoraSwapOperatorHeartbeat {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &self.operator != authority {
            return Err(invalid("operator heartbeat must match authority"));
        }
        ensure_bps(self.health_bps, "operator health")?;
        write_account_record(
            state_transaction,
            self.operator.clone(),
            metadata_key("operator", &self.service)?,
            record_string(
                "operator",
                "heartbeat",
                &[
                    ("operator", self.operator.to_string()),
                    ("service", self.service.to_string()),
                    ("slot", self.slot.to_string()),
                    ("health_bps", self.health_bps.to_string()),
                    ("fees_accrued", self.fees_accrued.to_string()),
                ],
            ),
        )
    }
}

impl Execute for ConfigureSoraSwapDlmmHook {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.account(&self.hook_contract)?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("dlmm_hook", &self.hook_id)?,
            record_string(
                "dlmm_hook",
                "configure",
                &[
                    ("pool_id", self.pool_id.to_string()),
                    ("hook_id", self.hook_id.to_string()),
                    ("hook_contract", self.hook_contract.to_string()),
                    ("phase", self.phase.to_string()),
                    ("max_fee_pips", self.max_fee_pips.to_string()),
                    ("enabled", self.enabled.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RecordSoraSwapHookExecution {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_non_zero(&self.amount_in, "hook amount_in")?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("dlmm_hook_execution", &self.order_id)?,
            record_string(
                "dlmm_hook",
                "execution",
                &[
                    ("pool_id", self.pool_id.to_string()),
                    ("hook_id", self.hook_id.to_string()),
                    ("order_id", self.order_id.to_string()),
                    ("amount_in", self.amount_in.to_string()),
                    ("amount_out", self.amount_out.to_string()),
                    ("fee_pips", self.fee_pips.to_string()),
                    ("slot", self.slot.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RegisterSoraSwapMarginMarket {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_bps(self.risk_weight_bps, "risk weight")?;
        ensure_bps(self.liquidation_threshold_bps, "liquidation threshold")?;
        state_transaction
            .world
            .asset_definition(&self.collateral_asset)?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("margin_market", &self.market_id)?,
            record_string(
                "margin",
                "register_market",
                &[
                    ("market_id", self.market_id.to_string()),
                    ("product", self.product.to_string()),
                    ("collateral_asset", self.collateral_asset.to_string()),
                    ("risk_weight_bps", self.risk_weight_bps.to_string()),
                    (
                        "liquidation_threshold_bps",
                        self.liquidation_threshold_bps.to_string(),
                    ),
                ],
            ),
        )
    }
}

impl Execute for UpdateSoraSwapMarginAccount {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &self.account != authority {
            return Err(invalid("margin account update must match authority"));
        }
        ensure_bps(self.health_bps, "account health")?;
        write_account_record(
            state_transaction,
            self.account.clone(),
            metadata_key("margin_account", &self.market_id)?,
            record_string(
                "margin",
                "update_account",
                &[
                    ("account", self.account.to_string()),
                    ("market_id", self.market_id.to_string()),
                    ("collateral_delta", self.collateral_delta.to_string()),
                    ("exposure_delta", self.exposure_delta.to_string()),
                    ("health_bps", self.health_bps.to_string()),
                    ("status", self.status.to_string()),
                ],
            ),
        )
    }
}

impl Execute for RegisterSoraSwapRwaMarket {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.rwa(&self.lot_id)?;
        state_transaction
            .world
            .asset_definition(&self.share_asset)?;
        state_transaction.world.account(&self.controller)?;
        state_transaction.world.asset_definition(&self.nav_asset)?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("rwa_market", &self.market_id)?,
            record_string(
                "rwa",
                "register_market",
                &[
                    ("market_id", self.market_id.to_string()),
                    ("lot_id", self.lot_id.to_string()),
                    ("share_asset", self.share_asset.to_string()),
                    ("controller", self.controller.to_string()),
                    ("nav_asset", self.nav_asset.to_string()),
                ],
            ),
        )
    }
}

impl Execute for ReportSoraSwapRwaNav {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_non_zero(&self.nav_per_share, "rwa nav_per_share")?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("rwa_nav", &self.market_id)?,
            record_string(
                "rwa",
                "nav",
                &[
                    ("market_id", self.market_id.to_string()),
                    ("nav_per_share", self.nav_per_share.to_string()),
                    ("total_shares", self.total_shares.to_string()),
                    ("report_slot", self.report_slot.to_string()),
                    ("status", self.status.to_string()),
                ],
            ),
        )
    }
}

impl Execute for SoraSwapInstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::SubmitIntent(isi) => isi.execute(authority, state_transaction),
            Self::SettleIntent(isi) => isi.execute(authority, state_transaction),
            Self::RegisterVault(isi) => isi.execute(authority, state_transaction),
            Self::VaultRequest(isi) => isi.execute(authority, state_transaction),
            Self::RegisterOperator(isi) => isi.execute(authority, state_transaction),
            Self::OperatorHeartbeat(isi) => isi.execute(authority, state_transaction),
            Self::ConfigureDlmmHook(isi) => isi.execute(authority, state_transaction),
            Self::HookExecution(isi) => isi.execute(authority, state_transaction),
            Self::RegisterMarginMarket(isi) => isi.execute(authority, state_transaction),
            Self::UpdateMarginAccount(isi) => isi.execute(authority, state_transaction),
            Self::RegisterRwaMarket(isi) => isi.execute(authority, state_transaction),
            Self::ReportRwaNav(isi) => isi.execute(authority, state_transaction),
        }
    }
}
