//! Declarative alias setup, lease lifecycle, and binding lifecycle instructions.
use super::*;
isi! {
    /// Ensure one alias/SNS resource matches an exact declarative intent.
    ///
    /// Consensus classifies the resource before quoting: exact state is a no-op,
    /// missing derived state is repaired without charge, absence is acquired once,
    /// and authoritative drift fails closed.
    #[norito(decode_from_slice)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct EnsureAlias {
        /// Exact desired resource state.
        pub intent: crate::alias_setup::AliasIntentV1,
        /// Lease terms used only when the resource is absent.
        pub acquisition: crate::alias_setup::AliasLeaseAcquisitionV1,
        /// Policy, payment asset, cap, and deadline guard.
        pub quote_guard: crate::alias_setup::AliasQuoteGuardV1,
    }
}
impl EnsureAlias {
    /// Stable wire identifier for declarative alias setup.
    pub const WIRE_ID: &'static str = "iroha.alias.ensure";
    /// Construct a declarative alias setup instruction.
    #[must_use]
    pub const fn new(
        intent: crate::alias_setup::AliasIntentV1,
        acquisition: crate::alias_setup::AliasLeaseAcquisitionV1,
        quote_guard: crate::alias_setup::AliasQuoteGuardV1,
    ) -> Self {
        Self {
            intent,
            acquisition,
            quote_guard,
        }
    }
}
impl crate::seal::Instruction for EnsureAlias {}
isi! {
    /// Renew one lease using expiry compare-and-set and an absolute target expiry.
    #[norito(decode_from_slice)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RenewAliasLease {
        /// Exact resolved lease target.
        pub target: crate::alias_setup::AliasTargetV1,
        /// Expiry that must be current when the instruction executes.
        pub expected_current_expiry_ms: u64,
        /// Absolute expiry to install after charging the exact recomputed quote.
        pub target_expiry_ms: u64,
        /// Policy, payment asset, cap, and deadline guard.
        pub quote_guard: crate::alias_setup::AliasQuoteGuardV1,
    }
}
impl RenewAliasLease {
    /// Stable wire identifier for guarded alias lease renewal.
    pub const WIRE_ID: &'static str = "iroha.alias.lease.renew";
    /// Construct an expiry-CAS lease renewal.
    #[must_use]
    pub const fn new(
        target: crate::alias_setup::AliasTargetV1,
        expected_current_expiry_ms: u64,
        target_expiry_ms: u64,
        quote_guard: crate::alias_setup::AliasQuoteGuardV1,
    ) -> Self {
        Self {
            target,
            expected_current_expiry_ms,
            target_expiry_ms,
            quote_guard,
        }
    }
}
impl crate::seal::Instruction for RenewAliasLease {}
isi! {
    /// Configure or disable native deterministic alias auto-renew.
    #[norito(decode_from_slice)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ConfigureAliasAutoRenew {
        /// Exact resolved lease target.
        pub target: crate::alias_setup::AliasTargetV1,
        /// Configuration revision that must be current at execution.
        pub expected_revision: u64,
        /// New configuration, or `None` to disable auto-renew.
        #[norito(default)]
        pub config: Option<crate::alias_setup::AliasAutoRenewConfigV1>,
    }
}
impl ConfigureAliasAutoRenew {
    /// Stable wire identifier for alias auto-renew configuration.
    pub const WIRE_ID: &'static str = "iroha.alias.auto_renew.configure";
    /// Construct an auto-renew configuration compare-and-set.
    #[must_use]
    pub const fn new(
        target: crate::alias_setup::AliasTargetV1,
        expected_revision: u64,
        config: Option<crate::alias_setup::AliasAutoRenewConfigV1>,
    ) -> Self {
        Self {
            target,
            expected_revision,
            config,
        }
    }
}
impl crate::seal::Instruction for ConfigureAliasAutoRenew {}
isi! {
    /// Explicitly rebind an account alias using target-account compare-and-set.
    ///
    /// This lifecycle operation never changes or accepts lease expiry state.
    #[norito(decode_from_slice)]
    pub struct RebindAccountAlias {
        /// Exact resolved alias being rebound.
        pub alias: crate::alias_setup::ResolvedAccountAliasV1,
        /// Account that must currently be bound to the alias.
        pub expected_target_account: AccountId,
        /// Account to bind after the compare-and-set succeeds.
        pub new_target_account: AccountId,
    }
}
impl RebindAccountAlias {
    /// Stable wire identifier for account alias rebinding.
    pub const WIRE_ID: &'static str = "iroha.account.alias.rebind";
    /// Construct an exact target-account compare-and-set rebind.
    #[must_use]
    pub const fn new(
        alias: crate::alias_setup::ResolvedAccountAliasV1,
        expected_target_account: AccountId,
        new_target_account: AccountId,
    ) -> Self {
        Self {
            alias,
            expected_target_account,
            new_target_account,
        }
    }
}
impl crate::seal::Instruction for RebindAccountAlias {}
isi! {
    /// Explicitly update an account's primary alias using compare-and-set.
    ///
    /// This lifecycle operation never changes or accepts lease expiry state.
    #[norito(decode_from_slice)]
    pub struct CompareAndSetPrimaryAccountAlias {
        /// Account whose primary alias is changing.
        pub account: AccountId,
        /// Alias that must currently be primary, or `None` if no primary is expected.
        #[norito(default)]
        pub expected_alias: Option<crate::alias_setup::ResolvedAccountAliasV1>,
        /// New primary alias, or `None` to clear it.
        #[norito(default)]
        pub new_alias: Option<crate::alias_setup::ResolvedAccountAliasV1>,
    }
}
impl CompareAndSetPrimaryAccountAlias {
    /// Stable wire identifier for primary account-alias compare-and-set.
    pub const WIRE_ID: &'static str = "iroha.account.alias.primary.compare_and_set";
    /// Construct a primary alias compare-and-set.
    #[must_use]
    pub const fn new(
        account: AccountId,
        expected_alias: Option<crate::alias_setup::ResolvedAccountAliasV1>,
        new_alias: Option<crate::alias_setup::ResolvedAccountAliasV1>,
    ) -> Self {
        Self {
            account,
            expected_alias,
            new_alias,
        }
    }
}
impl crate::seal::Instruction for CompareAndSetPrimaryAccountAlias {}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        alias_setup::{
            AccountAliasName, AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1,
            AliasAutoRenewConfigV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1,
            AliasTargetV1, ResolvedAccountAliasV1,
        },
        asset::AssetDefinitionId,
        domain::DomainId,
        nexus::DataSpaceId,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::{Numeric, Quantity};
    use crate::isi::test_support::assert_slice_roundtrip;
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked alias ISI fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn alias() -> ResolvedAccountAliasV1 {
        ResolvedAccountAliasV1::new(
            "merchant@banka.paynet"
                .parse::<AccountAliasName>()
                .expect("alias name"),
            DataSpaceId::new(7),
        )
    }
    fn payment_asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("assets", "paynet").expect("asset domain"),
            "xor".parse().expect("asset name"),
        )
    }
    fn amount(value: u32) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, 0)).expect("quantity")
    }
    fn guard() -> AliasQuoteGuardV1 {
        AliasQuoteGuardV1 {
            expected_policy_version: 2,
            expected_payment_asset: payment_asset(),
            max_amount: amount(10),
            valid_until_ms: 50_000,
        }
    }
    #[test]
    fn alias_setup_instructions_decode_from_slice_roundtrip() {
        let first = account(0xC1);
        let second = account(0xC2);
        let alias = alias();
        let target = AliasTargetV1::AccountAlias(alias.clone());
        assert_slice_roundtrip(EnsureAlias::new(
            AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                alias: alias.clone(),
                target_account: first.clone(),
                provision: AccountProvisionV1::Create,
                role: AccountAliasRoleV1::Primary,
            }),
            AliasLeaseAcquisitionV1::new(1, None),
            guard(),
        ));
        assert_slice_roundtrip(RenewAliasLease::new(target.clone(), 1_000, 2_000, guard()));
        assert_slice_roundtrip(ConfigureAliasAutoRenew::new(
            target.clone(),
            4,
            Some(AliasAutoRenewConfigV1 {
                term_years: 1,
                policy_version: 2,
                payment_asset: payment_asset(),
                max_amount: amount(9),
                renew_before_expiry_ms: 100,
                retry_backoff_ms: 50,
                max_failures: 5,
            }),
        ));
        assert_slice_roundtrip(ConfigureAliasAutoRenew::new(target, 5, None));
        assert_slice_roundtrip(RebindAccountAlias::new(
            alias.clone(),
            first.clone(),
            second,
        ));
        assert_slice_roundtrip(CompareAndSetPrimaryAccountAlias::new(
            first,
            None,
            Some(alias),
        ));
    }
    #[test]
    fn default_registry_uses_stable_alias_setup_wire_ids() {
        let registry = crate::instruction_registry::default();
        let cases = [
            (std::any::type_name::<EnsureAlias>(), EnsureAlias::WIRE_ID),
            (
                std::any::type_name::<RenewAliasLease>(),
                RenewAliasLease::WIRE_ID,
            ),
            (
                std::any::type_name::<ConfigureAliasAutoRenew>(),
                ConfigureAliasAutoRenew::WIRE_ID,
            ),
            (
                std::any::type_name::<RebindAccountAlias>(),
                RebindAccountAlias::WIRE_ID,
            ),
            (
                std::any::type_name::<CompareAndSetPrimaryAccountAlias>(),
                CompareAndSetPrimaryAccountAlias::WIRE_ID,
            ),
        ];
        for (type_name, wire_id) in cases {
            assert_eq!(registry.wire_id(type_name), Some(wire_id));
        }
        let owner = account(0xD1);
        let ensure = EnsureAlias::new(
            AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                alias: alias(),
                target_account: owner,
                provision: AccountProvisionV1::Existing,
                role: AccountAliasRoleV1::Additional,
            }),
            AliasLeaseAcquisitionV1::new(1, None),
            guard(),
        );
        let boxed: InstructionBox = ensure.clone().into();
        let (wire_id, framed) = super::super::encoded_instruction_pair_payload(&boxed)
            .expect("encode registered instruction pair");
        assert_eq!(wire_id, EnsureAlias::WIRE_ID);
        let decoded = registry
            .decode(wire_id, &framed)
            .expect("registered stable wire id")
            .expect("decode framed ensure instruction");
        assert_eq!(
            decoded
                .as_any()
                .downcast_ref::<EnsureAlias>()
                .expect("concrete EnsureAlias"),
            &ensure
        );
    }
}
