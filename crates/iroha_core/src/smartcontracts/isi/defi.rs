//! Metadata-backed DeFi instruction handlers.

use iroha_data_model::{
    events::data::prelude::{AccountEvent, MetadataChanged},
    isi::{
        defi::{
            ConfigureDefiAmmHook, DeFiInstructionBox, RecordDefiHookExecution,
            RecordDefiOperatorHeartbeat, RecordDefiVaultRequest, RegisterDefiMarginMarket,
            RegisterDefiOperator, RegisterDefiRwaMarket, RegisterDefiVault, ReportDefiRwaNav,
            SettleDefiIntent, SubmitDefiIntent, UpdateDefiMarginAccount,
        },
        error::InstructionExecutionError,
    },
    prelude::*,
};

use super::prelude::*;

fn invalid(message: impl Into<String>) -> Error {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

fn metadata_key(module: &str, id: &Name) -> Result<Name, Error> {
    format!("defi/{module}/{id}")
        .parse()
        .map_err(|_| invalid(format!("invalid DeFi metadata key for {module}/{id}")))
}

fn metadata_key_u64(module: &str, id: u64) -> Result<Name, Error> {
    format!("defi/{module}/{id}")
        .parse()
        .map_err(|_| invalid(format!("invalid DeFi metadata key for {module}/{id}")))
}

fn ensure_account_record_missing(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
    key: &Name,
    label: &str,
) -> Result<(), Error> {
    let account = state_transaction.world.account(account)?;
    if account.metadata().contains(key) {
        return Err(invalid(format!("{label} already exists")));
    }
    Ok(())
}

fn account_record(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
    key: &Name,
    label: &str,
) -> Result<Json, Error> {
    let account = state_transaction.world.account(account)?;
    account
        .metadata()
        .get(key)
        .cloned()
        .ok_or_else(|| invalid(format!("{label} is missing")))
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

fn record_status(value: &Json) -> Result<String, Error> {
    let json = value
        .try_into_any_norito::<norito::json::Value>()
        .map_err(|err| invalid(format!("invalid DeFi metadata JSON: {err}")))?;
    let object = json
        .as_object()
        .ok_or_else(|| invalid("invalid DeFi metadata JSON object"))?;
    object
        .get("status")
        .and_then(norito::json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid("DeFi metadata record is missing status"))
}

fn ensure_open_record(value: &Json, label: &str) -> Result<(), Error> {
    match record_status(value)?.as_str() {
        "open" | "submitted" => Ok(()),
        status => Err(invalid(format!("{label} is already terminal: {status}"))),
    }
}

fn ensure_terminal_status(status: &Name) -> Result<(), Error> {
    match status.as_ref() {
        "filled" | "cancelled" | "settled" => Ok(()),
        _ => Err(invalid(
            "intent settlement status must be filled, cancelled, or settled",
        )),
    }
}

macro_rules! record_json {
    ($module:literal, $action:literal, $status:expr, { $($key:literal : $value:expr),* $(,)? }) => {
        Json::from(norito::json!({
            "version": 1_u64,
            "module": $module,
            "action": $action,
            "status": $status,
            $($key: $value,)*
        }))
    };
}

impl Execute for SubmitDefiIntent {
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

        let intent_key = metadata_key("intent", &self.intent_id)?;
        let nonce_key = metadata_key_u64("intent_nonce", self.nonce)?;
        ensure_account_record_missing(state_transaction, authority, &intent_key, "DeFi intent")?;
        ensure_account_record_missing(
            state_transaction,
            authority,
            &nonce_key,
            "DeFi intent nonce",
        )?;

        write_account_record(
            state_transaction,
            authority.clone(),
            intent_key,
            record_json!("intent", "submit", "open", {
                "intent_id": self.intent_id.to_string(),
                "input_asset": self.input_asset.to_string(),
                "output_asset": self.output_asset.to_string(),
                "amount_in": self.amount_in.to_string(),
                "min_out": self.min_out.to_string(),
                "solver_fee_bps": u64::from(self.solver_fee_bps),
                "deadline_slot": self.deadline_slot,
                "nonce": self.nonce,
                "owner": authority.to_string(),
            }),
        )?;
        write_account_record(
            state_transaction,
            authority.clone(),
            nonce_key,
            record_json!("intent_nonce", "reserve", "reserved", {
                "intent_id": self.intent_id.to_string(),
                "nonce": self.nonce,
                "owner": authority.to_string(),
            }),
        )
    }
}

impl Execute for SettleDefiIntent {
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
        ensure_terminal_status(&self.status)?;

        let key = metadata_key("intent", &self.intent_id)?;
        let existing = account_record(state_transaction, &self.owner, &key, "DeFi intent")?;
        ensure_open_record(&existing, "DeFi intent")?;

        write_account_record(
            state_transaction,
            self.owner.clone(),
            key,
            record_json!("intent", "settle", self.status.to_string(), {
                "intent_id": self.intent_id.to_string(),
                "solver": self.solver.to_string(),
                "amount_out": self.amount_out.to_string(),
                "fill_slot": self.fill_slot,
            }),
        )
    }
}

impl Execute for RegisterDefiVault {
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
        let key = metadata_key("vault", &self.vault_id)?;
        ensure_account_record_missing(state_transaction, authority, &key, "DeFi vault")?;
        write_account_record(
            state_transaction,
            authority.clone(),
            key,
            record_json!("vault", "register", "active", {
                "vault_id": self.vault_id.to_string(),
                "underlying_asset": self.underlying_asset.to_string(),
                "share_asset": self.share_asset.to_string(),
                "strategy": self.strategy.to_string(),
                "async_redeem": self.async_redeem,
            }),
        )
    }
}

impl Execute for RecordDefiVaultRequest {
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
        let key = metadata_key("vault_request", &self.request_id)?;
        ensure_account_record_missing(
            state_transaction,
            &self.account,
            &key,
            "DeFi vault request",
        )?;
        write_account_record(
            state_transaction,
            self.account.clone(),
            key,
            record_json!("vault", "request", "pending", {
                "vault_id": self.vault_id.to_string(),
                "request_id": self.request_id.to_string(),
                "amount": self.amount.to_string(),
                "claim_slot": self.claim_slot,
                "request_kind": self.request_kind.to_string(),
            }),
        )
    }
}

impl Execute for RegisterDefiOperator {
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
        let key = metadata_key("operator_registration", &self.service)?;
        ensure_account_record_missing(state_transaction, &self.operator, &key, "DeFi operator")?;
        write_account_record(
            state_transaction,
            self.operator.clone(),
            key,
            record_json!("operator", "register", "active", {
                "operator": self.operator.to_string(),
                "service": self.service.to_string(),
                "bond_asset": self.bond_asset.to_string(),
                "min_bond": self.min_bond.to_string(),
            }),
        )
    }
}

impl Execute for RecordDefiOperatorHeartbeat {
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
            metadata_key("operator_heartbeat", &self.service)?,
            record_json!("operator", "heartbeat", "alive", {
                "operator": self.operator.to_string(),
                "service": self.service.to_string(),
                "slot": self.slot,
                "health_bps": u64::from(self.health_bps),
                "fees_accrued": self.fees_accrued.to_string(),
            }),
        )
    }
}

impl Execute for ConfigureDefiAmmHook {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.account(&self.hook_contract)?;
        write_account_record(
            state_transaction,
            authority.clone(),
            metadata_key("amm_hook", &self.hook_id)?,
            record_json!("amm_hook", "configure", if self.enabled { "enabled" } else { "disabled" }, {
                "pool_id": self.pool_id.to_string(),
                "hook_id": self.hook_id.to_string(),
                "hook_contract": self.hook_contract.to_string(),
                "phase": self.phase.to_string(),
                "max_fee_pips": u64::from(self.max_fee_pips),
                "enabled": self.enabled,
            }),
        )
    }
}

impl Execute for RecordDefiHookExecution {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_non_zero(&self.amount_in, "hook amount_in")?;
        let key = metadata_key("amm_hook_execution", &self.order_id)?;
        ensure_account_record_missing(state_transaction, authority, &key, "DeFi hook execution")?;
        write_account_record(
            state_transaction,
            authority.clone(),
            key,
            record_json!("amm_hook", "execution", "executed", {
                "pool_id": self.pool_id.to_string(),
                "hook_id": self.hook_id.to_string(),
                "order_id": self.order_id.to_string(),
                "amount_in": self.amount_in.to_string(),
                "amount_out": self.amount_out.to_string(),
                "fee_pips": u64::from(self.fee_pips),
                "slot": self.slot,
            }),
        )
    }
}

impl Execute for RegisterDefiMarginMarket {
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
        let key = metadata_key("margin_market", &self.market_id)?;
        ensure_account_record_missing(state_transaction, authority, &key, "DeFi margin market")?;
        write_account_record(
            state_transaction,
            authority.clone(),
            key,
            record_json!("margin", "register_market", "active", {
                "market_id": self.market_id.to_string(),
                "product": self.product.to_string(),
                "collateral_asset": self.collateral_asset.to_string(),
                "risk_weight_bps": u64::from(self.risk_weight_bps),
                "liquidation_threshold_bps": u64::from(self.liquidation_threshold_bps),
            }),
        )
    }
}

impl Execute for UpdateDefiMarginAccount {
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
            record_json!("margin", "update_account", self.status.to_string(), {
                "account": self.account.to_string(),
                "market_id": self.market_id.to_string(),
                "collateral_delta": self.collateral_delta.to_string(),
                "exposure_delta": self.exposure_delta.to_string(),
                "health_bps": u64::from(self.health_bps),
            }),
        )
    }
}

impl Execute for RegisterDefiRwaMarket {
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
        let key = metadata_key("rwa_market", &self.market_id)?;
        ensure_account_record_missing(state_transaction, authority, &key, "DeFi RWA market")?;
        write_account_record(
            state_transaction,
            authority.clone(),
            key,
            record_json!("rwa", "register_market", "active", {
                "market_id": self.market_id.to_string(),
                "lot_id": self.lot_id.to_string(),
                "share_asset": self.share_asset.to_string(),
                "controller": self.controller.to_string(),
                "nav_asset": self.nav_asset.to_string(),
            }),
        )
    }
}

impl Execute for ReportDefiRwaNav {
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
            record_json!("rwa", "nav", self.status.to_string(), {
                "market_id": self.market_id.to_string(),
                "nav_per_share": self.nav_per_share.to_string(),
                "total_shares": self.total_shares.to_string(),
                "report_slot": self.report_slot,
            }),
        )
    }
}

impl Execute for DeFiInstructionBox {
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
            Self::ConfigureAmmHook(isi) => isi.execute(authority, state_transaction),
            Self::HookExecution(isi) => isi.execute(authority, state_transaction),
            Self::RegisterMarginMarket(isi) => isi.execute(authority, state_transaction),
            Self::UpdateMarginAccount(isi) => isi.execute(authority, state_transaction),
            Self::RegisterRwaMarket(isi) => isi.execute(authority, state_transaction),
            Self::ReportRwaNav(isi) => isi.execute(authority, state_transaction),
        }
    }
}
