//! Native DeFi instructions.

use iroha_primitives::numeric::Numeric;

use super::*;
use crate::rwa::RwaId;

isi! {
    /// Submit a solver-fillable `DeFi` intent.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitDefiIntent {
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(tag = "kind", content = "value")
    )]
    pub struct SettleDefiIntent {
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
    /// Register a `DeFi` tokenized or async vault.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterDefiVault {
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordDefiVaultRequest {
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
    /// Register a bonded `DeFi` service operator.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterDefiOperator {
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordDefiOperatorHeartbeat {
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
    /// Configure an AMM hook policy for a pool.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ConfigureDefiAmmHook {
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
    /// Record an AMM hook execution result.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordDefiHookExecution {
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterDefiMarginMarket {
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct UpdateDefiMarginAccount {
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
    /// Register an RWA-backed `DeFi` market.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterDefiRwaMarket {
        /// Market identifier.
        pub market_id: Name,
        /// Native RWA lot associated with the market.
        pub lot_id: RwaId,
        /// Share asset used by `DeFi` routes.
        pub share_asset: AssetDefinitionId,
        /// Controller account for compliance and redemption actions.
        pub controller: AccountId,
        /// Asset used to denominate NAV reports.
        pub nav_asset: AssetDefinitionId,
    }
}

isi! {
    /// Record an RWA NAV or redemption checkpoint.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ReportDefiRwaNav {
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

macro_rules! impl_defi_display {
    ($ty:ty, $label:literal, $id:ident) => {
        impl core::fmt::Display for $ty {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, concat!($label, " `{}`"), self.$id)
            }
        }
    };
}

impl_defi_display!(SubmitDefiIntent, "DEFI_INTENT_SUBMIT", intent_id);
impl_defi_display!(SettleDefiIntent, "DEFI_INTENT_SETTLE", intent_id);
impl_defi_display!(RegisterDefiVault, "DEFI_VAULT_REGISTER", vault_id);
impl_defi_display!(RecordDefiVaultRequest, "DEFI_VAULT_REQUEST", request_id);
impl_defi_display!(RegisterDefiOperator, "DEFI_OPERATOR_REGISTER", service);
impl_defi_display!(
    RecordDefiOperatorHeartbeat,
    "DEFI_OPERATOR_HEARTBEAT",
    service
);
impl_defi_display!(ConfigureDefiAmmHook, "DEFI_AMM_HOOK_CONFIGURE", hook_id);
impl_defi_display!(RecordDefiHookExecution, "DEFI_AMM_HOOK_EXECUTION", order_id);
impl_defi_display!(
    RegisterDefiMarginMarket,
    "DEFI_MARGIN_MARKET_REGISTER",
    market_id
);
impl_defi_display!(
    UpdateDefiMarginAccount,
    "DEFI_MARGIN_ACCOUNT_UPDATE",
    market_id
);
impl_defi_display!(RegisterDefiRwaMarket, "DEFI_RWA_MARKET_REGISTER", market_id);
impl_defi_display!(ReportDefiRwaNav, "DEFI_RWA_NAV_REPORT", market_id);

impl SubmitDefiIntent {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.intent.submit";
}
impl SettleDefiIntent {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.intent.settle";
}
impl RegisterDefiVault {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.vault.register";
}
impl RecordDefiVaultRequest {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.vault.request";
}
impl RegisterDefiOperator {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.operator.register";
}
impl RecordDefiOperatorHeartbeat {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.operator.heartbeat";
}
impl ConfigureDefiAmmHook {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.amm_hook.configure";
}
impl RecordDefiHookExecution {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.amm_hook.execution";
}
impl RegisterDefiMarginMarket {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.margin.market.register";
}
impl UpdateDefiMarginAccount {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.margin.account.update";
}
impl RegisterDefiRwaMarket {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.rwa.market.register";
}
impl ReportDefiRwaNav {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.defi.rwa.nav.report";
}

impl crate::seal::Instruction for SubmitDefiIntent {}
impl crate::seal::Instruction for SettleDefiIntent {}
impl crate::seal::Instruction for RegisterDefiVault {}
impl crate::seal::Instruction for RecordDefiVaultRequest {}
impl crate::seal::Instruction for RegisterDefiOperator {}
impl crate::seal::Instruction for RecordDefiOperatorHeartbeat {}
impl crate::seal::Instruction for ConfigureDefiAmmHook {}
impl crate::seal::Instruction for RecordDefiHookExecution {}
impl crate::seal::Instruction for RegisterDefiMarginMarket {}
impl crate::seal::Instruction for UpdateDefiMarginAccount {}
impl crate::seal::Instruction for RegisterDefiRwaMarket {}
impl crate::seal::Instruction for ReportDefiRwaNav {}

isi_box! {
    /// Grouping enum for DeFi-native instructions.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(tag = "kind", content = "value")
    )]
    pub enum DeFiInstructionBox {
        /// Submit a solver intent.
        SubmitIntent(SubmitDefiIntent),
        /// Settle a solver intent.
        SettleIntent(SettleDefiIntent),
        /// Register a tokenized vault.
        RegisterVault(RegisterDefiVault),
        /// Record a vault request.
        VaultRequest(RecordDefiVaultRequest),
        /// Register a bonded service operator.
        RegisterOperator(RegisterDefiOperator),
        /// Record an operator heartbeat.
        OperatorHeartbeat(RecordDefiOperatorHeartbeat),
        /// Configure an AMM hook.
        ConfigureAmmHook(ConfigureDefiAmmHook),
        /// Record an AMM hook execution.
        HookExecution(RecordDefiHookExecution),
        /// Register a portfolio-margin market.
        RegisterMarginMarket(RegisterDefiMarginMarket),
        /// Update a margin account.
        UpdateMarginAccount(UpdateDefiMarginAccount),
        /// Register an RWA-backed market.
        RegisterRwaMarket(RegisterDefiRwaMarket),
        /// Record an RWA NAV checkpoint.
        ReportRwaNav(ReportDefiRwaNav),
    }
}

impl_into_box! {
    SubmitDefiIntent
    | SettleDefiIntent
    | RegisterDefiVault
    | RecordDefiVaultRequest
    | RegisterDefiOperator
    | RecordDefiOperatorHeartbeat
    | ConfigureDefiAmmHook
    | RecordDefiHookExecution
    | RegisterDefiMarginMarket
    | UpdateDefiMarginAccount
    | RegisterDefiRwaMarket
    | ReportDefiRwaNav
    => DeFiInstructionBox
}

impl crate::seal::Instruction for DeFiInstructionBox {}

impl DeFiInstructionBox {
    /// Stable wire identifier for boxed `DeFi` instructions.
    pub const WIRE_ID: &'static str = "iroha.defi";
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    use super::*;

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn domain() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("domain")
    }

    fn asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(domain(), name.parse().expect("asset name"))
    }

    fn name(value: &str) -> Name {
        value.parse().expect("name")
    }

    fn rwa_id(seed: &'static str) -> RwaId {
        RwaId::generated(domain(), Hash::new(seed))
    }

    fn submit_intent() -> SubmitDefiIntent {
        SubmitDefiIntent {
            intent_id: name("intent_a"),
            input_asset: asset("xor"),
            output_asset: asset("usdt"),
            amount_in: Numeric::from(100_u64),
            min_out: Numeric::from(99_u64),
            solver_fee_bps: 30,
            deadline_slot: 1000,
            nonce: 7,
        }
    }

    fn settle_intent() -> SettleDefiIntent {
        SettleDefiIntent {
            owner: account(1),
            intent_id: name("intent_a"),
            solver: account(2),
            amount_out: Numeric::from(101_u64),
            fill_slot: 990,
            status: name("filled"),
        }
    }

    fn vault() -> RegisterDefiVault {
        RegisterDefiVault {
            vault_id: name("vault_a"),
            underlying_asset: asset("xor"),
            share_asset: asset("vxor"),
            strategy: name("savings"),
            async_redeem: true,
        }
    }

    fn vault_request() -> RecordDefiVaultRequest {
        RecordDefiVaultRequest {
            vault_id: name("vault_a"),
            request_id: name("request_a"),
            account: account(1),
            amount: Numeric::from(50_u64),
            claim_slot: 2000,
            request_kind: name("redeem"),
        }
    }

    fn operator() -> RegisterDefiOperator {
        RegisterDefiOperator {
            operator: account(3),
            service: name("solver"),
            bond_asset: asset("xor"),
            min_bond: Numeric::from(10_000_u64),
        }
    }

    fn heartbeat() -> RecordDefiOperatorHeartbeat {
        RecordDefiOperatorHeartbeat {
            operator: account(3),
            service: name("solver"),
            slot: 3000,
            health_bps: 9_900,
            fees_accrued: Numeric::from(12_u64),
        }
    }

    fn hook_policy() -> ConfigureDefiAmmHook {
        ConfigureDefiAmmHook {
            pool_id: name("pool_a"),
            hook_id: name("hook_a"),
            hook_contract: account(4),
            phase: name("dynamic_fee"),
            max_fee_pips: 2_500,
            enabled: true,
        }
    }

    fn hook_execution() -> RecordDefiHookExecution {
        RecordDefiHookExecution {
            pool_id: name("pool_a"),
            hook_id: name("hook_a"),
            order_id: name("order_a"),
            amount_in: Numeric::from(25_u64),
            amount_out: Numeric::from(26_u64),
            fee_pips: 100,
            slot: 4000,
        }
    }

    fn margin_market() -> RegisterDefiMarginMarket {
        RegisterDefiMarginMarket {
            market_id: name("perps_xor"),
            product: name("perps"),
            collateral_asset: asset("usdt"),
            risk_weight_bps: 8_000,
            liquidation_threshold_bps: 6_500,
        }
    }

    fn margin_account() -> UpdateDefiMarginAccount {
        UpdateDefiMarginAccount {
            account: account(5),
            market_id: name("perps_xor"),
            collateral_delta: Numeric::from(1_000_u64),
            exposure_delta: Numeric::from(500_u64),
            health_bps: 9_000,
            status: name("healthy"),
        }
    }

    fn rwa_market() -> RegisterDefiRwaMarket {
        RegisterDefiRwaMarket {
            market_id: name("tbill_a"),
            lot_id: rwa_id("defi-rwa"),
            share_asset: asset("tbill"),
            controller: account(6),
            nav_asset: asset("usdt"),
        }
    }

    fn rwa_nav() -> ReportDefiRwaNav {
        ReportDefiRwaNav {
            market_id: name("tbill_a"),
            nav_per_share: Numeric::from(1_u64),
            total_shares: Numeric::from(10_000_u64),
            report_slot: 5000,
            status: name("active"),
        }
    }

    fn assert_norito_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let bytes = norito::to_bytes(&value).expect("encode");
        let decoded = norito::decode_from_bytes::<T>(&bytes).expect("decode");
        assert_eq!(decoded, value);
    }

    #[cfg(feature = "json")]
    fn assert_json_roundtrip<T>(value: T)
    where
        T: Clone
            + PartialEq
            + core::fmt::Debug
            + norito::json::JsonSerialize
            + norito::json::JsonDeserializeOwned,
    {
        let json = norito::json::to_json(&value).expect("json encode");
        let decoded = norito::json::from_str::<T>(&json).expect("json decode");
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes(value: DeFiInstructionBox) {
        let registry = crate::isi::registry::default();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<DeFiInstructionBox>(&payload, flags)
                .expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            DeFiInstructionBox::WIRE_ID,
            &framed,
        )
        .expect("registered")
        .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn defi_wire_ids_are_stable() {
        assert_eq!(DeFiInstructionBox::WIRE_ID, "iroha.defi");
        assert_eq!(SubmitDefiIntent::WIRE_ID, "iroha.defi.intent.submit");
        assert_eq!(SettleDefiIntent::WIRE_ID, "iroha.defi.intent.settle");
        assert_eq!(RegisterDefiVault::WIRE_ID, "iroha.defi.vault.register");
        assert_eq!(RecordDefiVaultRequest::WIRE_ID, "iroha.defi.vault.request");
        assert_eq!(
            RegisterDefiOperator::WIRE_ID,
            "iroha.defi.operator.register"
        );
        assert_eq!(
            RecordDefiOperatorHeartbeat::WIRE_ID,
            "iroha.defi.operator.heartbeat"
        );
        assert_eq!(
            ConfigureDefiAmmHook::WIRE_ID,
            "iroha.defi.amm_hook.configure"
        );
        assert_eq!(
            RecordDefiHookExecution::WIRE_ID,
            "iroha.defi.amm_hook.execution"
        );
        assert_eq!(
            RegisterDefiMarginMarket::WIRE_ID,
            "iroha.defi.margin.market.register"
        );
        assert_eq!(
            UpdateDefiMarginAccount::WIRE_ID,
            "iroha.defi.margin.account.update"
        );
        assert_eq!(
            RegisterDefiRwaMarket::WIRE_ID,
            "iroha.defi.rwa.market.register"
        );
        assert_eq!(ReportDefiRwaNav::WIRE_ID, "iroha.defi.rwa.nav.report");
    }

    #[test]
    fn defi_display_strings_are_stable() {
        assert_eq!(submit_intent().to_string(), "DEFI_INTENT_SUBMIT `intent_a`");
        assert_eq!(settle_intent().to_string(), "DEFI_INTENT_SETTLE `intent_a`");
        assert_eq!(vault().to_string(), "DEFI_VAULT_REGISTER `vault_a`");
        assert_eq!(
            vault_request().to_string(),
            "DEFI_VAULT_REQUEST `request_a`"
        );
        assert_eq!(operator().to_string(), "DEFI_OPERATOR_REGISTER `solver`");
        assert_eq!(heartbeat().to_string(), "DEFI_OPERATOR_HEARTBEAT `solver`");
        assert_eq!(
            hook_policy().to_string(),
            "DEFI_AMM_HOOK_CONFIGURE `hook_a`"
        );
        assert_eq!(
            hook_execution().to_string(),
            "DEFI_AMM_HOOK_EXECUTION `order_a`"
        );
        assert_eq!(
            margin_market().to_string(),
            "DEFI_MARGIN_MARKET_REGISTER `perps_xor`"
        );
        assert_eq!(
            margin_account().to_string(),
            "DEFI_MARGIN_ACCOUNT_UPDATE `perps_xor`"
        );
        assert_eq!(
            rwa_market().to_string(),
            "DEFI_RWA_MARKET_REGISTER `tbill_a`"
        );
        assert_eq!(rwa_nav().to_string(), "DEFI_RWA_NAV_REPORT `tbill_a`");
    }

    #[test]
    fn defi_norito_roundtrips() {
        assert_norito_roundtrip(submit_intent());
        assert_norito_roundtrip(settle_intent());
        assert_norito_roundtrip(vault());
        assert_norito_roundtrip(vault_request());
        assert_norito_roundtrip(operator());
        assert_norito_roundtrip(heartbeat());
        assert_norito_roundtrip(hook_policy());
        assert_norito_roundtrip(hook_execution());
        assert_norito_roundtrip(margin_market());
        assert_norito_roundtrip(margin_account());
        assert_norito_roundtrip(rwa_market());
        assert_norito_roundtrip(rwa_nav());
        assert_norito_roundtrip(DeFiInstructionBox::SubmitIntent(submit_intent()));
        assert_norito_roundtrip(DeFiInstructionBox::ReportRwaNav(rwa_nav()));
    }

    #[test]
    #[cfg(feature = "json")]
    fn defi_json_roundtrips() {
        assert_json_roundtrip(submit_intent());
        assert_json_roundtrip(settle_intent());
        assert_json_roundtrip(vault());
        assert_json_roundtrip(vault_request());
        assert_json_roundtrip(operator());
        assert_json_roundtrip(heartbeat());
        assert_json_roundtrip(hook_policy());
        assert_json_roundtrip(hook_execution());
        assert_json_roundtrip(margin_market());
        assert_json_roundtrip(margin_account());
        assert_json_roundtrip(rwa_market());
        assert_json_roundtrip(rwa_nav());
        assert_json_roundtrip(DeFiInstructionBox::SubmitIntent(submit_intent()));
        assert_json_roundtrip(DeFiInstructionBox::ReportRwaNav(rwa_nav()));
    }

    #[test]
    fn defi_default_registry_decodes_boxed_surface() {
        assert_registry_decodes(DeFiInstructionBox::SubmitIntent(submit_intent()));
        assert_registry_decodes(DeFiInstructionBox::SettleIntent(settle_intent()));
        assert_registry_decodes(DeFiInstructionBox::RegisterVault(vault()));
        assert_registry_decodes(DeFiInstructionBox::VaultRequest(vault_request()));
        assert_registry_decodes(DeFiInstructionBox::RegisterOperator(operator()));
        assert_registry_decodes(DeFiInstructionBox::OperatorHeartbeat(heartbeat()));
        assert_registry_decodes(DeFiInstructionBox::ConfigureAmmHook(hook_policy()));
        assert_registry_decodes(DeFiInstructionBox::HookExecution(hook_execution()));
        assert_registry_decodes(DeFiInstructionBox::RegisterMarginMarket(margin_market()));
        assert_registry_decodes(DeFiInstructionBox::UpdateMarginAccount(margin_account()));
        assert_registry_decodes(DeFiInstructionBox::RegisterRwaMarket(rwa_market()));
        assert_registry_decodes(DeFiInstructionBox::ReportRwaNav(rwa_nav()));
    }
}
