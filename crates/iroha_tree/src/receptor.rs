//! Module for [`Receptor`] and related components.

use super::*;

/// Represents readiness for status of each node.
pub type Receptor = FuzzyTree<ReadWriteStatusFilter>;

/// Each node value indicates readiness for status.
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub struct ReadWriteStatusFilter;

impl Mode for ReadWriteStatusFilter {
    type Authorizer = FilterU8;
    type Parameter = FilterU8;
    type Peer = FilterU8;
    type Domain = FilterU8;
    type Account = FilterU8;
    type Asset = FilterU8;
    type Nft = FilterU8;
    type AccountAsset = FilterU8;
    type Role = FilterU8;
    type Permission = FilterU8;
    type AccountRole = FilterU8;
    type AccountPermission = FilterU8;
    type RolePermission = FilterU8;
    type Trigger = FilterU8;
    type Condition = FilterU8;
    type Executable = FilterU8;
    type TriggerCondition = FilterU8;
    type TriggerExecutable = FilterU8;
    type DomainMetadata = FilterU8;
    type AccountMetadata = FilterU8;
    type AssetMetadata = FilterU8;
    type NftData = FilterU8;
    type TriggerMetadata = FilterU8;
    type DomainAdmin = FilterU8;
    type AssetAdmin = FilterU8;
    type NftAdmin = FilterU8;
    type NftOwner = FilterU8;
    type TriggerAdmin = FilterU8;
}

impl Filtered for event::Event {
    type Filter = Receptor;

    fn passes(&self, filter: &Self::Filter) -> Result<(), Self::Filter> {
        let mut obstacle = Self::Filter::default();
        for (key, signal) in self.iter() {
            let signal: FilterU8 = signal.into();
            let receptor_keys = key.receptor_keys();
            let receptor_union = filter
                .iter()
                .filter_map(|(k, v)| receptor_keys.contains(k).then_some(v).map(FilterU8::from))
                .fold(FilterU8::DENY, |acc, x| acc | x);
            if let Err(obs) = signal.passes(&receptor_union) {
                obstacle.insert(
                    FuzzyNodeEntry::try_from((key.fuzzy(), NodeValue::from((key, obs)))).unwrap(),
                );
            }
        }
        if obstacle.is_empty() {
            Ok(())
        } else {
            Err(obstacle)
        }
    }
}

macro_rules! receptor_keys {
    (0 $node_type:ident) => {
        [$node_type(())].into()
    };
    (1 $node_type:ident, $key:expr) => {
        [$node_type(Some($key.clone())), $node_type(None)].into()
    };
    (2 $node_type:ident, $key:expr) => {
        [
            $node_type((Some($key.0.clone()), Some($key.1.clone()))),
            $node_type((Some($key.0.clone()), None)),
            $node_type((None, Some($key.1.clone()))),
            $node_type((None, None)),
        ]
        .into()
    };
    (3 $node_type:ident, $key:expr) => {
        [
            $node_type((
                Some($key.0.clone()),
                Some($key.1.clone()),
                Some($key.2.clone()),
            )),
            $node_type((Some($key.0.clone()), Some($key.1.clone()), None)),
            $node_type((Some($key.0.clone()), None, Some($key.2.clone()))),
            $node_type((Some($key.0.clone()), None, None)),
            $node_type((None, Some($key.1.clone()), Some($key.2.clone()))),
            $node_type((None, Some($key.1.clone()), None)),
            $node_type((None, None, Some($key.2.clone()))),
            $node_type((None, None, None)),
        ]
        .into()
    };
    (4 $node_type:ident, $key:expr) => {
        [
            $node_type((
                Some($key.0.clone()),
                Some($key.1.clone()),
                Some($key.2.clone()),
                Some($key.3.clone()),
            )),
            $node_type((
                Some($key.0.clone()),
                Some($key.1.clone()),
                Some($key.2.clone()),
                None,
            )),
            $node_type((
                Some($key.0.clone()),
                Some($key.1.clone()),
                None,
                Some($key.3.clone()),
            )),
            $node_type((Some($key.0.clone()), Some($key.1.clone()), None, None)),
            $node_type((
                Some($key.0.clone()),
                None,
                Some($key.2.clone()),
                Some($key.3.clone()),
            )),
            $node_type((Some($key.0.clone()), None, Some($key.2.clone()), None)),
            $node_type((Some($key.0.clone()), None, None, Some($key.3.clone()))),
            $node_type((Some($key.0.clone()), None, None, None)),
            $node_type((
                None,
                Some($key.1.clone()),
                Some($key.2.clone()),
                Some($key.3.clone()),
            )),
            $node_type((None, Some($key.1.clone()), Some($key.2.clone()), None)),
            $node_type((None, Some($key.1.clone()), None, Some($key.3.clone()))),
            $node_type((None, Some($key.1.clone()), None, None)),
            $node_type((None, None, Some($key.2.clone()), Some($key.3.clone()))),
            $node_type((None, None, Some($key.2.clone()), None)),
            $node_type((None, None, None, Some($key.3.clone()))),
            $node_type((None, None, None, None)),
        ]
        .into()
    };
}

impl NodeKey {
    pub(crate) fn receptor_keys(&self) -> BTreeSet<FuzzyNodeKey> {
        use FuzzyNodeKey::*;
        match self {
            Self::Authorizer(()) => receptor_keys!(0 Authorizer),
            Self::Parameter(key) => receptor_keys!(1 Parameter, key),
            Self::Peer(key) => receptor_keys!(1 Peer, key),
            Self::Domain(key) => receptor_keys!(1 Domain, key),
            Self::Account(key) => receptor_keys!(2 Account, key),
            Self::Asset(key) => receptor_keys!(2 Asset, key),
            Self::Nft(key) => receptor_keys!(2 Nft, key),
            Self::AccountAsset(key) => receptor_keys!(4 AccountAsset, key),
            Self::Role(key) => receptor_keys!(1 Role, key),
            Self::Permission(key) => receptor_keys!(1 Permission, key),
            Self::AccountRole(key) => receptor_keys!(3 AccountRole, key),
            Self::AccountPermission(key) => receptor_keys!(3 AccountPermission, key),
            Self::RolePermission(key) => receptor_keys!(2 RolePermission, key),
            Self::Trigger(key) => receptor_keys!(1 Trigger, key),
            Self::Condition(key) => receptor_keys!(1 Condition, key),
            Self::Executable(key) => receptor_keys!(1 Executable, key),
            Self::TriggerCondition(key) => receptor_keys!(2 TriggerCondition, key),
            Self::TriggerExecutable(key) => receptor_keys!(2 TriggerExecutable, key),
            Self::DomainMetadata(key) => receptor_keys!(2 DomainMetadata, key),
            Self::AccountMetadata(key) => receptor_keys!(3 AccountMetadata, key),
            Self::AssetMetadata(key) => receptor_keys!(3 AssetMetadata, key),
            Self::NftData(key) => receptor_keys!(3 NftData, key),
            Self::TriggerMetadata(key) => receptor_keys!(2 TriggerMetadata, key),
            Self::DomainAdmin(key) => receptor_keys!(3 DomainAdmin, key),
            Self::AssetAdmin(key) => receptor_keys!(4 AssetAdmin, key),
            Self::NftAdmin(key) => receptor_keys!(4 NftAdmin, key),
            Self::NftOwner(key) => receptor_keys!(4 NftOwner, key),
            Self::TriggerAdmin(key) => receptor_keys!(3 TriggerAdmin, key),
        }
    }
}

mod transitional {
    use super::*;
    use crate::event::*;

    impl TryFrom<dm::EventFilterBox> for state::tr::ConditionV {
        type Error = &'static str;

        fn try_from(value: dm::EventFilterBox) -> Result<Self, Self::Error> {
            use dm::{EventFilterBox, ExecutionTime};
            match value {
                EventFilterBox::Data(filter) => Ok(Receptor::from(filter).into()),
                EventFilterBox::Time(filter) => match filter.0 {
                    ExecutionTime::PreCommit => Ok(state::tr::BlockCommit.into()),
                    ExecutionTime::Schedule(schedule) => Ok(schedule.into()),
                },
                EventFilterBox::Pipeline(_) => Err("pipeline triggers are scheduled for removal"),
                _ => Err("these event types are scheduled for removal"),
            }
        }
    }

    impl From<dm::DataEventFilter> for Receptor {
        #[expect(clippy::too_many_lines)]
        fn from(value: dm::DataEventFilter) -> Self {
            use dm::{
                AccountEventSet, AssetDefinitionEventSet, AssetEventSet, ConfigurationEventSet,
                DataEventFilter::*, DomainEventSet, ExecutorEventSet, NftEventSet, PeerEventSet,
                RoleEventSet, TriggerEventSet,
            };

            match value {
                Any => [
                    fuzzy_node!(Authorizer, FilterU8::ANY),
                    fuzzy_node!(Parameter, None, FilterU8::ANY),
                    fuzzy_node!(Peer, None, FilterU8::ANY),
                    fuzzy_node!(Domain, None, FilterU8::ANY),
                    fuzzy_node!(Account, None, None, FilterU8::ANY),
                    fuzzy_node!(Asset, None, None, FilterU8::ANY),
                    fuzzy_node!(Nft, None, None, FilterU8::ANY),
                    fuzzy_node!(AccountAsset, None, None, None, None, FilterU8::ANY),
                    fuzzy_node!(Role, None, FilterU8::ANY),
                    fuzzy_node!(Permission, None, FilterU8::ANY),
                    fuzzy_node!(AccountRole, None, None, None, FilterU8::ANY),
                    fuzzy_node!(AccountPermission, None, None, None, FilterU8::ANY),
                    fuzzy_node!(RolePermission, None, None, FilterU8::ANY),
                    fuzzy_node!(Trigger, None, FilterU8::ANY),
                    fuzzy_node!(Executable, None, FilterU8::ANY),
                    fuzzy_node!(DomainMetadata, None, None, FilterU8::ANY),
                    fuzzy_node!(AccountMetadata, None, None, None, FilterU8::ANY),
                    fuzzy_node!(AssetMetadata, None, None, None, FilterU8::ANY),
                    fuzzy_node!(NftData, None, None, None, FilterU8::ANY),
                    fuzzy_node!(TriggerMetadata, None, None, FilterU8::ANY),
                    fuzzy_node!(DomainAdmin, None, None, None, FilterU8::ANY),
                    fuzzy_node!(AssetAdmin, None, None, None, None, FilterU8::ANY),
                    fuzzy_node!(NftAdmin, None, None, None, None, FilterU8::ANY),
                    fuzzy_node!(NftOwner, None, None, None, None, FilterU8::ANY),
                    fuzzy_node!(TriggerAdmin, None, None, None, FilterU8::ANY),
                ]
                .into_iter()
                .collect(),
                Peer(ef) => {
                    let id = ef.id_matcher.map(Rc::new);
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            PeerEventSet::Added => {
                                fuzzy_node!(Peer, id.clone(), UnitS::Create)
                            }
                            PeerEventSet::Removed => {
                                fuzzy_node!(Peer, id.clone(), UnitS::Delete)
                            }
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Domain(ef) => {
                    let domain = ef.id_matcher.map(Rc::new);
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            DomainEventSet::Created => {
                                fuzzy_node!(Domain, domain.clone(), DomainS::Create)
                            }
                            DomainEventSet::Deleted => {
                                fuzzy_node!(Domain, domain.clone(), DomainS::Delete)
                            }
                            DomainEventSet::AnyAssetDefinition => {
                                fuzzy_node!(Asset, None, domain.clone(), FilterU8::ANY)
                            }
                            DomainEventSet::AnyNft => {
                                fuzzy_node!(Nft, None, domain.clone(), FilterU8::ANY)
                            }
                            DomainEventSet::AnyAccount => {
                                fuzzy_node!(Account, None, domain.clone(), FilterU8::ANY)
                            }
                            DomainEventSet::MetadataInserted => {
                                fuzzy_node!(DomainMetadata, domain.clone(), None, MetadataS::Set)
                            }
                            DomainEventSet::MetadataRemoved => {
                                fuzzy_node!(DomainMetadata, domain.clone(), None, MetadataS::Unset)
                            }
                            DomainEventSet::OwnerChanged => fuzzy_node!(
                                DomainAdmin,
                                domain.clone(),
                                None,
                                None,
                                UnitS::Create as u8 | UnitS::Delete as u8
                            ),
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Account(ef) => {
                    let (signatory, domain) = match ef.id_matcher {
                        None => (None, None),
                        Some(id) => (some!(id.signatory), some!(id.domain)),
                    };
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            AccountEventSet::Created => fuzzy_node!(
                                Account,
                                signatory.clone(),
                                domain.clone(),
                                UnitS::Create
                            ),
                            AccountEventSet::Deleted => fuzzy_node!(
                                Account,
                                signatory.clone(),
                                domain.clone(),
                                UnitS::Delete
                            ),
                            AccountEventSet::AnyAsset => fuzzy_node!(
                                AccountAsset,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                None,
                                FilterU8::ANY
                            ),
                            AccountEventSet::PermissionAdded => fuzzy_node!(
                                AccountPermission,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                UnitS::Create
                            ),
                            AccountEventSet::PermissionRemoved => fuzzy_node!(
                                AccountPermission,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                UnitS::Delete
                            ),
                            AccountEventSet::RoleGranted => fuzzy_node!(
                                AccountRole,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                UnitS::Create
                            ),
                            AccountEventSet::RoleRevoked => fuzzy_node!(
                                AccountRole,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                UnitS::Delete
                            ),
                            AccountEventSet::MetadataInserted => fuzzy_node!(
                                AccountMetadata,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                MetadataS::Set
                            ),
                            AccountEventSet::MetadataRemoved => fuzzy_node!(
                                AccountMetadata,
                                signatory.clone(),
                                domain.clone(),
                                None,
                                MetadataS::Unset
                            ),
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Asset(ef) => {
                    let (account_key, account_domain, asset_name, asset_domain) =
                        match ef.id_matcher {
                            None => (None, None, None, None),
                            Some(id) => (
                                some!(id.account.signatory),
                                some!(id.account.domain),
                                some!(id.definition.name),
                                some!(id.definition.domain),
                            ),
                        };
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            AssetEventSet::Created => {
                                fuzzy_node!(
                                    AccountAsset,
                                    account_key.clone(),
                                    account_domain.clone(),
                                    asset_name.clone(),
                                    asset_domain.clone(),
                                    AccountAssetS::Mint
                                )
                            }
                            AssetEventSet::Deleted => {
                                fuzzy_node!(
                                    AccountAsset,
                                    account_key.clone(),
                                    account_domain.clone(),
                                    asset_name.clone(),
                                    asset_domain.clone(),
                                    AccountAssetS::Burn
                                )
                            }
                            AssetEventSet::Added => {
                                fuzzy_node!(
                                    AccountAsset,
                                    account_key.clone(),
                                    account_domain.clone(),
                                    asset_name.clone(),
                                    asset_domain.clone(),
                                    AccountAssetS::Receive
                                )
                            }
                            AssetEventSet::Removed => {
                                fuzzy_node!(
                                    AccountAsset,
                                    account_key.clone(),
                                    account_domain.clone(),
                                    asset_name.clone(),
                                    asset_domain.clone(),
                                    AccountAssetS::Send
                                )
                            }
                            _ => unreachable!(),
                        })
                        .collect()
                }
                AssetDefinition(ef) => {
                    let (name, domain) = match ef.id_matcher {
                        None => (None, None),
                        Some(id) => (some!(id.name), some!(id.domain)),
                    };
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            AssetDefinitionEventSet::Created => {
                                fuzzy_node!(Asset, name.clone(), domain.clone(), AssetS::Create)
                            }
                            AssetDefinitionEventSet::Deleted => {
                                fuzzy_node!(Asset, name.clone(), domain.clone(), AssetS::Delete)
                            }
                            AssetDefinitionEventSet::MetadataInserted => fuzzy_node!(
                                AssetMetadata,
                                name.clone(),
                                domain.clone(),
                                None,
                                MetadataS::Set
                            ),
                            AssetDefinitionEventSet::MetadataRemoved => fuzzy_node!(
                                AssetMetadata,
                                name.clone(),
                                domain.clone(),
                                None,
                                MetadataS::Unset
                            ),
                            AssetDefinitionEventSet::MintabilityChanged => fuzzy_node!(
                                Asset,
                                name.clone(),
                                domain.clone(),
                                AssetS::MintabilityUpdate
                            ),
                            AssetDefinitionEventSet::TotalQuantityChanged => fuzzy_node!(
                                AccountAsset,
                                None,
                                None,
                                name.clone(),
                                domain.clone(),
                                AccountAssetS::Mint as u8 | AccountAssetS::Burn as u8
                            ),
                            AssetDefinitionEventSet::OwnerChanged => fuzzy_node!(
                                AssetAdmin,
                                name.clone(),
                                domain.clone(),
                                None,
                                None,
                                UnitS::Create as u8 | UnitS::Delete as u8
                            ),
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Nft(ef) => {
                    let (name, domain) = match ef.id_matcher {
                        None => (None, None),
                        Some(id) => (some!(id.name), some!(id.domain)),
                    };
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            NftEventSet::Created => {
                                fuzzy_node!(Nft, name.clone(), domain.clone(), NftS::Create)
                            }
                            NftEventSet::Deleted => {
                                fuzzy_node!(Nft, name.clone(), domain.clone(), NftS::Delete)
                            }
                            NftEventSet::MetadataInserted => fuzzy_node!(
                                NftData,
                                name.clone(),
                                domain.clone(),
                                None,
                                MetadataS::Set
                            ),
                            NftEventSet::MetadataRemoved => fuzzy_node!(
                                NftData,
                                name.clone(),
                                domain.clone(),
                                None,
                                MetadataS::Unset
                            ),
                            NftEventSet::OwnerChanged => fuzzy_node!(
                                NftOwner,
                                name.clone(),
                                domain.clone(),
                                None,
                                None,
                                UnitS::Create as u8 | UnitS::Delete as u8
                            ),
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Trigger(ef) => {
                    let id = ef.id_matcher.map(Rc::new);
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            TriggerEventSet::Created => {
                                fuzzy_node!(Trigger, id.clone(), TriggerS::Create)
                            }
                            TriggerEventSet::Deleted => {
                                fuzzy_node!(Trigger, id.clone(), TriggerS::Delete)
                            }
                            TriggerEventSet::Extended => {
                                fuzzy_node!(Trigger, id.clone(), TriggerS::Increase)
                            }
                            TriggerEventSet::Shortened => {
                                fuzzy_node!(Trigger, id.clone(), TriggerS::Decrease)
                            }
                            TriggerEventSet::MetadataInserted => {
                                fuzzy_node!(TriggerMetadata, id.clone(), None, MetadataS::Set)
                            }
                            TriggerEventSet::MetadataRemoved => {
                                fuzzy_node!(TriggerMetadata, id.clone(), None, MetadataS::Unset)
                            }
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Role(ef) => {
                    let id = ef.id_matcher.map(Rc::new);
                    ef.event_set
                        .decompose()
                        .into_iter()
                        .map(|es| match es {
                            RoleEventSet::Created => {
                                fuzzy_node!(Role, id.clone(), UnitS::Create)
                            }
                            RoleEventSet::Deleted => {
                                fuzzy_node!(Role, id.clone(), UnitS::Delete)
                            }
                            RoleEventSet::PermissionAdded => {
                                fuzzy_node!(RolePermission, id.clone(), None, UnitS::Create)
                            }
                            RoleEventSet::PermissionRemoved => {
                                fuzzy_node!(RolePermission, id.clone(), None, UnitS::Delete)
                            }
                            _ => unreachable!(),
                        })
                        .collect()
                }
                Configuration(ef) => ef
                    .event_set
                    .decompose()
                    .into_iter()
                    .map(|es| match es {
                        ConfigurationEventSet::Changed => {
                            fuzzy_node!(Parameter, None, ParameterS::Set)
                        }
                        _ => unreachable!(),
                    })
                    .collect(),
                Executor(ef) => ef
                    .event_set
                    .decompose()
                    .into_iter()
                    .map(|es| match es {
                        ExecutorEventSet::Upgraded => {
                            fuzzy_node!(Authorizer, AuthorizerS::Set)
                        }
                        _ => unreachable!(),
                    })
                    .collect(),
            }
        }
    }
}
