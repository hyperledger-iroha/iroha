//! Module for [`Permission`] and related components.

use super::*;

/// Represents readiness for access of each node.
pub type Permission = FuzzyTree<ReadWriteStatusFilter>;

/// Each node value indicates readiness for access.
pub type ReadWriteStatusFilter = receptor::ReadWriteStatusFilter;

impl Filtered for state::StateView {
    type Filter = Permission;

    /// Post-execution validation of read access.
    fn passes(&self, filter: &Self::Filter) -> Result<(), Self::Filter> {
        self.as_status().passes(filter)
    }
}

impl Filtered for changeset::ChangeSet {
    type Filter = Permission;

    /// Pre-execution validation of write access.
    fn passes(&self, filter: &Self::Filter) -> Result<(), Self::Filter> {
        self.as_status().passes(filter)
    }
}

impl BitOr for Permission {
    type Output = Self;

    fn bitor(self, mut rhs: Self) -> Self::Output {
        for (k, v0) in self {
            let v = match rhs.remove(&k) {
                None => v0,
                Some(v1) => (v0 | v1).expect("value types should be consistent"),
            };
            rhs.insert(FuzzyNodeEntry::try_from((k, v)).unwrap());
        }
        rhs
    }
}

mod transitional {
    use event::*;
    use iroha_executor_data_model::permission as xp;

    use super::*;

    impl From<&dm::Permission> for tr::PermissionId {
        fn from(value: &dm::Permission) -> Self {
            Self::from(&state::tr::PermissionV::from(Permission::from(value)))
        }
    }

    macro_rules! impl_from_data_model_permission {
        ($(($can:path, $node_type:ident, |$source:ident| $key:expr, $statuses:expr),)+) => {
            impl From<&dm::Permission> for Permission {
                fn from(value: &dm::Permission) -> Self {
                    $(
                    if let Ok(can) = <$can as TryFrom<&dm::Permission>>::try_from(value) { return can.into() }
                    )+
                    unreachable!("data model permission should convert into one of the permission tokens")
                }
            }

            $(
            impl From<$can> for Permission {
                fn from($source: $can) -> Self {
                    [FuzzyNodeEntry::try_from((
                        FuzzyNodeKey::$node_type($key),
                        NodeValue::$node_type(
                            $statuses
                                .into_iter()
                                .map(FilterU8::from)
                                .reduce(|acc, x| acc | x)
                                .unwrap(),
                        ),
                    )).unwrap()]
                    .into_iter()
                    .collect()
                }
            }
            )+
        };
    }

    impl_from_data_model_permission!(
        (
            xp::peer::CanManagePeers,
            Peer,
            |_v| None,
            [UnitS::Create, UnitS::Delete]
        ),
        (
            xp::domain::CanRegisterDomain,
            Domain,
            |_v| None,
            [DomainS::Create]
        ),
        (
            xp::domain::CanUnregisterDomain,
            Domain,
            |v| some!(v.domain),
            [DomainS::Delete]
        ),
        (
            xp::domain::CanModifyDomainMetadata,
            DomainMetadata,
            |v| (some!(v.domain), None),
            [MetadataS::Set, MetadataS::Unset]
        ),
        (
            xp::account::CanRegisterAccount,
            Account,
            |v| (None, some!(v.domain)),
            [UnitS::Create]
        ),
        (
            xp::account::CanUnregisterAccount,
            Account,
            |v| (some!(v.account.signatory), some!(v.account.domain)),
            [UnitS::Delete]
        ),
        (
            xp::account::CanModifyAccountMetadata,
            AccountMetadata,
            |v| (some!(v.account.signatory), some!(v.account.domain), None),
            [MetadataS::Set, MetadataS::Unset]
        ),
        (
            xp::asset_definition::CanRegisterAssetDefinition,
            Asset,
            |v| (None, some!(v.domain)),
            [AssetS::Create]
        ),
        (
            xp::asset_definition::CanUnregisterAssetDefinition,
            Asset,
            |v| (
                some!(v.asset_definition.name),
                some!(v.asset_definition.domain)
            ),
            [AssetS::Delete]
        ),
        (
            xp::asset_definition::CanModifyAssetDefinitionMetadata,
            AssetMetadata,
            |v| (
                some!(v.asset_definition.name),
                some!(v.asset_definition.domain),
                None
            ),
            [MetadataS::Set, MetadataS::Unset]
        ),
        (
            xp::asset::CanMintAssetWithDefinition,
            AccountAsset,
            |v| (
                None,
                None,
                some!(v.asset_definition.name),
                some!(v.asset_definition.domain)
            ),
            [AccountAssetS::Mint]
        ),
        (
            xp::asset::CanBurnAssetWithDefinition,
            AccountAsset,
            |v| (
                None,
                None,
                some!(v.asset_definition.name),
                some!(v.asset_definition.domain)
            ),
            [AccountAssetS::Burn]
        ),
        (
            xp::asset::CanTransferAssetWithDefinition,
            AccountAsset,
            |v| (
                None,
                None,
                some!(v.asset_definition.name),
                some!(v.asset_definition.domain)
            ),
            [AccountAssetS::Send]
        ),
        (
            xp::asset::CanMintAsset,
            AccountAsset,
            |v| (
                some!(v.asset.account.signatory),
                some!(v.asset.account.domain),
                some!(v.asset.definition.name),
                some!(v.asset.definition.domain)
            ),
            [AccountAssetS::Mint]
        ),
        (
            xp::asset::CanBurnAsset,
            AccountAsset,
            |v| (
                some!(v.asset.account.signatory),
                some!(v.asset.account.domain),
                some!(v.asset.definition.name),
                some!(v.asset.definition.domain)
            ),
            [AccountAssetS::Burn]
        ),
        (
            xp::asset::CanTransferAsset,
            AccountAsset,
            |v| (
                some!(v.asset.account.signatory),
                some!(v.asset.account.domain),
                some!(v.asset.definition.name),
                some!(v.asset.definition.domain)
            ),
            [AccountAssetS::Send]
        ),
        (
            xp::nft::CanRegisterNft,
            Nft,
            |v| (None, some!(v.domain)),
            [NftS::Create]
        ),
        (
            xp::nft::CanUnregisterNft,
            Nft,
            |v| (some!(v.nft.name), some!(v.nft.domain)),
            [NftS::Delete]
        ),
        (
            xp::nft::CanTransferNft,
            NftOwner,
            |v| (some!(v.nft.name), some!(v.nft.domain), None, None),
            [UnitS::Create, UnitS::Delete]
        ),
        (
            xp::nft::CanModifyNftMetadata,
            NftData,
            |v| (some!(v.nft.name), some!(v.nft.domain), None),
            [MetadataS::Set, MetadataS::Unset]
        ),
        (
            xp::parameter::CanSetParameters,
            Parameter,
            |_v| None,
            [ParameterS::Set]
        ),
        (
            xp::role::CanManageRoles,
            Role,
            |_v| None,
            [UnitS::Create, UnitS::Delete]
        ),
        // TODO: Separate into registration and ownership transfer.
        // xp::trigger::CanRegisterTrigger

        // TODO: No validation should be performed when calling Wasm executables, as they are resolved into event predictions and then validated.
        // xp::trigger::CanExecuteTrigger
        (
            xp::trigger::CanUnregisterTrigger,
            Trigger,
            |v| some!(v.trigger),
            [TriggerS::Delete]
        ),
        (
            xp::trigger::CanModifyTrigger,
            Trigger,
            |v| some!(v.trigger),
            [TriggerS::Increase, TriggerS::Decrease]
        ),
        (
            xp::trigger::CanModifyTriggerMetadata,
            TriggerMetadata,
            |v| (some!(v.trigger), None),
            [MetadataS::Set, MetadataS::Unset]
        ),
        (
            xp::executor::CanUpgradeExecutor,
            Authorizer,
            |_v| (),
            [AuthorizerS::Set]
        ),
    );
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use super::*;

    #[test]
    fn aggregates() {
        let role_f = |i: usize, f: &str| {
            fuzzy_node!(
                Role,
                some!(format!("role_{i}").parse().unwrap()),
                FilterU8::from_str(f).unwrap()
            )
        };
        let role_f_set = |i: usize, f: &str| Permission::from_iter([role_f(i, f)]);
        assert_eq!(role_f_set(0, "c") | role_f_set(0, "c"), role_f_set(0, "c"));
        assert_eq!(
            role_f_set(0, "c") | role_f_set(1, "c"),
            Permission::from_iter([role_f(0, "c"), role_f(1, "c")])
        );
        assert_eq!(
            role_f_set(0, "c") | role_f_set(0, "d"),
            Permission::from_iter([role_f(0, "cd"),])
        );
    }
}
