//! SoraSwap-native DeFi instructions.

use iroha_primitives::numeric::Numeric;

use super::*;
use crate::rwa::RwaId;

isi! {
    /// Submit a solver-fillable SoraSwap intent.
    pub struct SubmitSoraSwapIntent {
        /// Caller-selected intent identifier.
        pub intent_id: Name,
        /// Asset provided by the owner.
        pub input_asset: AssetDefinitionId,
        /// Asset expected by the owner.
        pub output_asset: AssetDefinitionId,
        /// Input amount escrowed or made available to the intent route.
        pub amount_in: Numeric,
        /// Minimum output accepted by the owner.
        pub min_out: Numeric,
        /// Solver fee measured in basis points.
        pub solver_fee_bps: u16,
        /// Last slot at which the intent may be filled.
        pub deadline_slot: u64,
        /// Owner-scoped replay nonce.
        pub nonce: u64,
    }
}

isi! {
    /// Record a solver fill or terminal settlement for an intent.
    pub struct SettleSoraSwapIntent {
        /// Intent owner.
        pub owner: AccountId,
        /// Intent identifier.
        pub intent_id: Name,
        /// Solver that settled the intent.
        pub solver: AccountId,
        /// Output amount delivered.
        pub amount_out: Numeric,
        /// Fill slot recorded by the route.
        pub fill_slot: u64,
        /// Terminal status label, for example `filled` or `cancelled`.
        pub status: Name,
    }
}

isi! {
    /// Register a SoraSwap tokenized or async vault.
    pub struct RegisterSoraSwapVault {
        /// Vault identifier.
        pub vault_id: Name,
        /// Asset accepted by the vault.
        pub underlying_asset: AssetDefinitionId,
        /// Share asset issued by the vault.
        pub share_asset: AssetDefinitionId,
        /// Strategy label.
        pub strategy: Name,
        /// Whether redemptions use an async request/claim flow.
        pub async_redeem: bool,
    }
}

isi! {
    /// Record a vault deposit or redemption request.
    pub struct RecordSoraSwapVaultRequest {
        /// Vault identifier.
        pub vault_id: Name,
        /// Request identifier.
        pub request_id: Name,
        /// Account that owns the request.
        pub account: AccountId,
        /// Underlying amount or share amount associated with the request.
        pub amount: Numeric,
        /// Slot when the request may be claimed.
        pub claim_slot: u64,
        /// Request kind, for example `deposit`, `redeem`, or `claim`.
        pub request_kind: Name,
    }
}

isi! {
    /// Register a bonded SoraSwap service operator.
    pub struct RegisterSoraSwapOperator {
        /// Operator account.
        pub operator: AccountId,
        /// Service label, for example `solver`, `keeper`, `oracle`, or `relayer`.
        pub service: Name,
        /// Asset used for the service bond.
        pub bond_asset: AssetDefinitionId,
        /// Minimum bond required for eligibility.
        pub min_bond: Numeric,
    }
}

isi! {
    /// Record service health and accrued fees for an operator.
    pub struct RecordSoraSwapOperatorHeartbeat {
        /// Operator account.
        pub operator: AccountId,
        /// Service label.
        pub service: Name,
        /// Heartbeat slot.
        pub slot: u64,
        /// Health score in basis points.
        pub health_bps: u16,
        /// Fees accrued since the previous checkpoint.
        pub fees_accrued: Numeric,
    }
}

isi! {
    /// Configure a DLMM hook policy for a pool.
    pub struct ConfigureSoraSwapDlmmHook {
        /// Pool identifier.
        pub pool_id: Name,
        /// Hook identifier.
        pub hook_id: Name,
        /// Contract subject authorized to execute the hook.
        pub hook_contract: AccountId,
        /// Hook phase, for example `dynamic_fee`, `twamm`, `limit_order`, or `lp_fee`.
        pub phase: Name,
        /// Maximum fee the hook may apply.
        pub max_fee_pips: u32,
        /// Whether the hook is enabled.
        pub enabled: bool,
    }
}

isi! {
    /// Record a DLMM hook execution result.
    pub struct RecordSoraSwapHookExecution {
        /// Pool identifier.
        pub pool_id: Name,
        /// Hook identifier.
        pub hook_id: Name,
        /// Hook-owned order or schedule identifier.
        pub order_id: Name,
        /// Input amount consumed.
        pub amount_in: Numeric,
        /// Output amount produced.
        pub amount_out: Numeric,
        /// Fee applied by the hook.
        pub fee_pips: u32,
        /// Execution slot.
        pub slot: u64,
    }
}

isi! {
    /// Register a portfolio-margin market.
    pub struct RegisterSoraSwapMarginMarket {
        /// Market identifier.
        pub market_id: Name,
        /// Product family, for example `perps`, `options`, `cover`, or `rwa`.
        pub product: Name,
        /// Collateral asset accepted for the market.
        pub collateral_asset: AssetDefinitionId,
        /// Collateral risk weight in basis points.
        pub risk_weight_bps: u16,
        /// Liquidation threshold in basis points.
        pub liquidation_threshold_bps: u16,
    }
}

isi! {
    /// Record a portfolio-margin account update.
    pub struct UpdateSoraSwapMarginAccount {
        /// Account whose margin ledger is updated.
        pub account: AccountId,
        /// Market identifier.
        pub market_id: Name,
        /// Collateral delta.
        pub collateral_delta: Numeric,
        /// Exposure delta.
        pub exposure_delta: Numeric,
        /// Account health after the update.
        pub health_bps: u16,
        /// Status label, for example `healthy`, `warning`, or `liquidatable`.
        pub status: Name,
    }
}

isi! {
    /// Register an RWA-backed SoraSwap market.
    pub struct RegisterSoraSwapRwaMarket {
        /// Market identifier.
        pub market_id: Name,
        /// Native RWA lot associated with the market.
        pub lot_id: RwaId,
        /// Share asset used by SoraSwap routes.
        pub share_asset: AssetDefinitionId,
        /// Controller account for compliance and redemption actions.
        pub controller: AccountId,
        /// Asset used to denominate NAV reports.
        pub nav_asset: AssetDefinitionId,
    }
}

isi! {
    /// Record an RWA NAV or redemption checkpoint.
    pub struct ReportSoraSwapRwaNav {
        /// Market identifier.
        pub market_id: Name,
        /// NAV per share.
        pub nav_per_share: Numeric,
        /// Total outstanding shares.
        pub total_shares: Numeric,
        /// Report slot.
        pub report_slot: u64,
        /// Status label, for example `active`, `frozen`, or `redeeming`.
        pub status: Name,
    }
}

macro_rules! impl_soraswap_display {
    ($ty:ty, $label:literal, $id:ident) => {
        impl core::fmt::Display for $ty {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, concat!($label, " `{}`"), self.$id)
            }
        }
    };
}

impl_soraswap_display!(SubmitSoraSwapIntent, "SORASWAP_INTENT_SUBMIT", intent_id);
impl_soraswap_display!(SettleSoraSwapIntent, "SORASWAP_INTENT_SETTLE", intent_id);
impl_soraswap_display!(RegisterSoraSwapVault, "SORASWAP_VAULT_REGISTER", vault_id);
impl_soraswap_display!(
    RecordSoraSwapVaultRequest,
    "SORASWAP_VAULT_REQUEST",
    request_id
);
impl_soraswap_display!(
    RegisterSoraSwapOperator,
    "SORASWAP_OPERATOR_REGISTER",
    service
);
impl_soraswap_display!(
    RecordSoraSwapOperatorHeartbeat,
    "SORASWAP_OPERATOR_HEARTBEAT",
    service
);
impl_soraswap_display!(
    ConfigureSoraSwapDlmmHook,
    "SORASWAP_DLMM_HOOK_CONFIGURE",
    hook_id
);
impl_soraswap_display!(
    RecordSoraSwapHookExecution,
    "SORASWAP_DLMM_HOOK_EXECUTION",
    order_id
);
impl_soraswap_display!(
    RegisterSoraSwapMarginMarket,
    "SORASWAP_MARGIN_MARKET_REGISTER",
    market_id
);
impl_soraswap_display!(
    UpdateSoraSwapMarginAccount,
    "SORASWAP_MARGIN_ACCOUNT_UPDATE",
    market_id
);
impl_soraswap_display!(
    RegisterSoraSwapRwaMarket,
    "SORASWAP_RWA_MARKET_REGISTER",
    market_id
);
impl_soraswap_display!(ReportSoraSwapRwaNav, "SORASWAP_RWA_NAV_REPORT", market_id);

impl SubmitSoraSwapIntent {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.intent.submit";
}
impl SettleSoraSwapIntent {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.intent.settle";
}
impl RegisterSoraSwapVault {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.vault.register";
}
impl RecordSoraSwapVaultRequest {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.vault.request";
}
impl RegisterSoraSwapOperator {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.operator.register";
}
impl RecordSoraSwapOperatorHeartbeat {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.operator.heartbeat";
}
impl ConfigureSoraSwapDlmmHook {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.dlmm_hook.configure";
}
impl RecordSoraSwapHookExecution {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.dlmm_hook.execution";
}
impl RegisterSoraSwapMarginMarket {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.margin.market.register";
}
impl UpdateSoraSwapMarginAccount {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.margin.account.update";
}
impl RegisterSoraSwapRwaMarket {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.rwa.market.register";
}
impl ReportSoraSwapRwaNav {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.soraswap.rwa.nav.report";
}

impl crate::seal::Instruction for SubmitSoraSwapIntent {}
impl crate::seal::Instruction for SettleSoraSwapIntent {}
impl crate::seal::Instruction for RegisterSoraSwapVault {}
impl crate::seal::Instruction for RecordSoraSwapVaultRequest {}
impl crate::seal::Instruction for RegisterSoraSwapOperator {}
impl crate::seal::Instruction for RecordSoraSwapOperatorHeartbeat {}
impl crate::seal::Instruction for ConfigureSoraSwapDlmmHook {}
impl crate::seal::Instruction for RecordSoraSwapHookExecution {}
impl crate::seal::Instruction for RegisterSoraSwapMarginMarket {}
impl crate::seal::Instruction for UpdateSoraSwapMarginAccount {}
impl crate::seal::Instruction for RegisterSoraSwapRwaMarket {}
impl crate::seal::Instruction for ReportSoraSwapRwaNav {}

isi_box! {
    /// Grouping enum for SoraSwap DeFi-native instructions.
    pub enum SoraSwapInstructionBox {
        /// Submit a solver intent.
        SubmitIntent(SubmitSoraSwapIntent),
        /// Settle a solver intent.
        SettleIntent(SettleSoraSwapIntent),
        /// Register a tokenized vault.
        RegisterVault(RegisterSoraSwapVault),
        /// Record a vault request.
        VaultRequest(RecordSoraSwapVaultRequest),
        /// Register a bonded service operator.
        RegisterOperator(RegisterSoraSwapOperator),
        /// Record an operator heartbeat.
        OperatorHeartbeat(RecordSoraSwapOperatorHeartbeat),
        /// Configure a DLMM hook.
        ConfigureDlmmHook(ConfigureSoraSwapDlmmHook),
        /// Record a DLMM hook execution.
        HookExecution(RecordSoraSwapHookExecution),
        /// Register a portfolio-margin market.
        RegisterMarginMarket(RegisterSoraSwapMarginMarket),
        /// Update a margin account.
        UpdateMarginAccount(UpdateSoraSwapMarginAccount),
        /// Register an RWA-backed market.
        RegisterRwaMarket(RegisterSoraSwapRwaMarket),
        /// Record an RWA NAV checkpoint.
        ReportRwaNav(ReportSoraSwapRwaNav),
    }
}

impl_into_box! {
    SubmitSoraSwapIntent
    | SettleSoraSwapIntent
    | RegisterSoraSwapVault
    | RecordSoraSwapVaultRequest
    | RegisterSoraSwapOperator
    | RecordSoraSwapOperatorHeartbeat
    | ConfigureSoraSwapDlmmHook
    | RecordSoraSwapHookExecution
    | RegisterSoraSwapMarginMarket
    | UpdateSoraSwapMarginAccount
    | RegisterSoraSwapRwaMarket
    | ReportSoraSwapRwaNav
    => SoraSwapInstructionBox
}

impl crate::seal::Instruction for SoraSwapInstructionBox {}

impl SoraSwapInstructionBox {
    /// Stable wire identifier for boxed SoraSwap instructions.
    pub const WIRE_ID: &'static str = "iroha.soraswap";
}
