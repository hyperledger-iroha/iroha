//! Module for [`ChangeSet`] and related components.

use super::*;

/// Represents write access for each node.
pub type ChangeSet = Tree<Write>;

/// Each node value indicates write access.
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub struct Write;

impl Mode for Write {
    type Authorizer = AuthorizerW;
    type Parameter = ParameterW;
    type Peer = UnitW;
    type Domain = DomainW;
    type Account = UnitW;
    type Asset = AssetW;
    type Nft = NftW;
    type AccountAsset = AccountAssetW;
    type Role = UnitW;
    type Permission = PermissionW;
    type AccountRole = UnitW;
    type AccountPermission = UnitW;
    type RolePermission = UnitW;
    type Trigger = TriggerW;
    type Condition = ConditionW;
    type Executable = ExecutableW;
    type TriggerCondition = UnitW;
    type TriggerExecutable = UnitW;
    type DomainMetadata = MetadataW;
    type AccountMetadata = MetadataW;
    type AssetMetadata = MetadataW;
    type NftData = MetadataW;
    type TriggerMetadata = MetadataW;
    type DomainAdmin = UnitW;
    type AssetAdmin = UnitW;
    type NftAdmin = UnitW;
    type NftOwner = UnitW;
    type TriggerAdmin = UnitW;
}

/// Write access at `Authorizer` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum AuthorizerW {
    Set(state::tr::AuthorizerV),
}

/// Write access at `Unit` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum UnitW {
    Create(()),
    Delete(()),
}

/// Write access at `Parameter` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum ParameterW {
    Set(state::tr::ParameterV),
    Unset(()),
}

/// Write access at `Domain` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum DomainW {
    Create(state::tr::DomainV),
    Delete(()),
}

/// Write access at `Asset` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum AssetW {
    MintabilityUpdate(dm::Mintable),
    Create(state::tr::AssetV),
    Delete(()),
}

/// Write access at `Nft` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum NftW {
    Create(state::tr::NftV),
    Delete(()),
}

/// Write access at `AccountAsset` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum AccountAssetW {
    Receive(dm::Numeric),
    Send(dm::Numeric),
    Mint(dm::Numeric),
    Burn(dm::Numeric),
}

/// Write access at `Permission` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum PermissionW {
    Set(state::tr::PermissionV),
    Unset(()),
}

/// Write access at `Trigger` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum TriggerW {
    Increase(u32),
    Decrease(u32),
    Create(state::tr::TriggerV),
    Delete(()),
}

/// Write access at `Condition` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum ConditionW {
    Set(state::tr::ConditionV),
    Unset(()),
}

/// Write access at `Executable` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum ExecutableW {
    Set(state::tr::ExecutableV),
    Unset(()),
}

/// Write access at `Metadata` type nodes.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub enum MetadataW {
    Set(state::tr::MetadataV),
    Unset(()),
}

impl NodeReadWrite for ChangeSet {
    type Status = event::Event;

    fn as_status(&self) -> Self::Status {
        self.iter()
            .map(|(k, write)| NodeEntry::try_from((k.clone(), write.into())).unwrap())
            .collect()
    }
}

impl Add for ChangeSet {
    type Output = Result<Self, Box<NodeConflict<Write>>>;

    fn add(self, mut rhs: Self) -> Self::Output {
        for (k, v0) in self {
            let v = match rhs.remove(&k) {
                None => v0,
                Some(v1) => match v0 + v1 {
                    Ok(v) => v,
                    Err((v0, v1)) => return Err(NodeConflict::new(k, v0, v1).into()),
                },
            };
            rhs.insert(NodeEntry::try_from((k, v)).unwrap());
        }
        Ok(rhs)
    }
}

macro_rules! impl_add_err {
    ($($ty:ty,)+) => {
        $(
        impl Add for $ty {
            type Output = Result<Self, (Self, Self)>;

            fn add(self, rhs: Self) -> Self::Output {
                Err((self, rhs))
            }
        }
        )+
    };
}

// Multiple modifications to the same node within a single transaction are generally not allowed.
impl_add_err!(
    AuthorizerW,
    UnitW,
    ParameterW,
    DomainW,
    AssetW,
    NftW,
    PermissionW,
    ConditionW,
    ExecutableW,
    MetadataW,
);

impl Add for AccountAssetW {
    type Output = Result<Self, (Self, Self)>;

    fn add(self, rhs: Self) -> Self::Output {
        let add = match (self, rhs) {
            (Self::Receive(l), Self::Receive(r)) => match l.checked_add(r) {
                Some(add) => Self::Receive(add),
                _ => return Err((Self::Receive(l), Self::Receive(r))),
            },
            (Self::Send(l), Self::Send(r)) => match l.checked_add(r) {
                Some(add) => Self::Send(add),
                _ => return Err((Self::Send(l), Self::Send(r))),
            },
            (Self::Mint(l), Self::Mint(r)) => match l.checked_add(r) {
                Some(add) => Self::Mint(add),
                _ => return Err((Self::Mint(l), Self::Mint(r))),
            },
            (Self::Burn(l), Self::Burn(r)) => match l.checked_add(r) {
                Some(add) => Self::Burn(add),
                _ => return Err((Self::Burn(l), Self::Burn(r))),
            },
            (l, r) => return Err((l, r)),
        };
        Ok(add)
    }
}

impl Add for TriggerW {
    type Output = Result<Self, (Self, Self)>;

    fn add(self, rhs: Self) -> Self::Output {
        let add = match (self, rhs) {
            (Self::Increase(l), Self::Increase(r)) => match l.checked_add(r) {
                Some(add) => Self::Increase(add),
                _ => return Err((Self::Increase(l), Self::Increase(r))),
            },
            (Self::Decrease(l), Self::Decrease(r)) => match l.checked_add(r) {
                Some(add) => Self::Decrease(add),
                _ => return Err((Self::Decrease(l), Self::Decrease(r))),
            },
            (l, r) => return Err((l, r)),
        };
        Ok(add)
    }
}

mod transitional {
    use super::*;

    impl TryFrom<(dm::AccountId, Vec<dm::InstructionBox>)> for ChangeSet {
        type Error = Box<NodeConflict<Write>>;

        fn try_from(
            (authority, instructions): (dm::AccountId, Vec<dm::InstructionBox>),
        ) -> Result<Self, Self::Error> {
            instructions
                .into_iter()
                .try_fold(Self::default(), |acc, x| {
                    acc + (authority.clone(), x).try_into()?
                })
        }
    }

    impl TryFrom<(dm::AccountId, dm::InstructionBox)> for ChangeSet {
        type Error = Box<NodeConflict<Write>>;

        #[expect(clippy::too_many_lines)]
        fn try_from(
            (authority, instruction): (dm::AccountId, dm::InstructionBox),
        ) -> Result<Self, Self::Error> {
            use dm::{
                numeric, BurnBox, GrantBox, InstructionBox, MintBox, Numeric, RegisterBox,
                RemoveKeyValueBox, RevokeBox, SetKeyValueBox, TransferBox, UnregisterBox,
            };

            let changeset: Self = match instruction {
                InstructionBox::Register(inst) => match inst {
                    RegisterBox::Peer(inst) => [node!(Peer, inst.object, UnitW::Create(()))]
                        .into_iter()
                        .collect(),
                    RegisterBox::Domain(inst) => [
                        node!(
                            DomainAdmin,
                            inst.object.id.clone(),
                            authority.signatory,
                            authority.domain,
                            UnitW::Create(())
                        ),
                        node!(
                            Domain,
                            inst.object.id,
                            DomainW::Create(inst.object.logo.into())
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    RegisterBox::Account(inst) => [node!(
                        Account,
                        inst.object.id.signatory,
                        inst.object.id.domain,
                        UnitW::Create(())
                    )]
                    .into_iter()
                    .collect(),
                    RegisterBox::AssetDefinition(inst) => [
                        node!(
                            AssetAdmin,
                            inst.object.id.name.clone(),
                            inst.object.id.domain.clone(),
                            authority.signatory,
                            authority.domain,
                            UnitW::Create(())
                        ),
                        node!(
                            Asset,
                            inst.object.id.name,
                            inst.object.id.domain,
                            AssetW::Create(state::tr::AssetV::new(
                                numeric!(0),
                                inst.object.mintable,
                                inst.object.logo
                            ))
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    RegisterBox::Nft(inst) => [
                        node!(
                            NftAdmin,
                            inst.object.id.name.clone(),
                            inst.object.id.domain.clone(),
                            authority.signatory.clone(),
                            authority.domain.clone(),
                            UnitW::Create(())
                        ),
                        node!(
                            NftOwner,
                            inst.object.id.name.clone(),
                            inst.object.id.domain.clone(),
                            authority.signatory,
                            authority.domain,
                            UnitW::Create(())
                        ),
                        node!(
                            Nft,
                            inst.object.id.name.clone(),
                            inst.object.id.domain.clone(),
                            NftW::Create(state::tr::NftV)
                        ),
                    ]
                    .into_iter()
                    .chain(inst.object.content.iter().map(|(k, v)| {
                        node!(
                            NftData,
                            inst.object.id.name.clone(),
                            inst.object.id.domain.clone(),
                            k.clone(),
                            MetadataW::Set(v.clone().into())
                        )
                    }))
                    .collect(),
                    RegisterBox::Role(inst) => {
                        [node!(Role, inst.object.inner.id, UnitW::Create(()))]
                            .into_iter()
                            .collect()
                    }
                    RegisterBox::Trigger(inst) => {
                        let trigger = state::tr::TriggerV::from(inst.object.action.repeats);
                        let condition = state::tr::ConditionV::try_from(inst.object.action.filter)
                            .expect("event filter type should be either data or time");
                        let executable = state::tr::ExecutableV::try_from((
                            authority.clone(),
                            inst.object.action.executable,
                        ))?;
                        let trigger_id = inst.object.id;
                        let condition_id = tr::ConditionId::from(&condition);
                        let executable_id = tr::ExecutableId::from(&executable);
                        [
                            node!(Trigger, trigger_id.clone(), TriggerW::Create(trigger)),
                            node!(Condition, condition_id.clone(), ConditionW::Set(condition)),
                            node!(
                                Executable,
                                executable_id.clone(),
                                ExecutableW::Set(executable)
                            ),
                            node!(
                                TriggerCondition,
                                trigger_id.clone(),
                                condition_id,
                                UnitW::Create(())
                            ),
                            node!(
                                TriggerExecutable,
                                trigger_id.clone(),
                                executable_id,
                                UnitW::Create(())
                            ),
                            node!(
                                TriggerAdmin,
                                trigger_id,
                                authority.signatory,
                                authority.domain,
                                UnitW::Create(())
                            ),
                        ]
                        .into_iter()
                        .collect()
                    }
                },
                InstructionBox::Unregister(inst) => match inst {
                    UnregisterBox::Peer(inst) => [node!(Peer, inst.object, UnitW::Delete(()))]
                        .into_iter()
                        .collect(),
                    UnregisterBox::Domain(inst) => {
                        [node!(Domain, inst.object, DomainW::Delete(()))]
                            .into_iter()
                            .collect()
                    }
                    UnregisterBox::Account(inst) => [node!(
                        Account,
                        inst.object.signatory,
                        inst.object.domain,
                        UnitW::Delete(())
                    )]
                    .into_iter()
                    .collect(),
                    UnregisterBox::AssetDefinition(inst) => [node!(
                        Asset,
                        inst.object.name,
                        inst.object.domain,
                        AssetW::Delete(())
                    )]
                    .into_iter()
                    .collect(),
                    UnregisterBox::Nft(inst) => [node!(
                        Nft,
                        inst.object.name,
                        inst.object.domain,
                        NftW::Delete(())
                    )]
                    .into_iter()
                    .collect(),
                    UnregisterBox::Role(inst) => [node!(Role, inst.object, UnitW::Delete(()))]
                        .into_iter()
                        .collect(),
                    UnregisterBox::Trigger(inst) => {
                        [node!(Trigger, inst.object, TriggerW::Delete(()))]
                            .into_iter()
                            .collect()
                    }
                },
                InstructionBox::Mint(inst) => match inst {
                    MintBox::Asset(inst) => [node!(
                        AccountAsset,
                        inst.destination.account.signatory,
                        inst.destination.account.domain,
                        inst.destination.definition.name,
                        inst.destination.definition.domain,
                        AccountAssetW::Mint(inst.object)
                    )]
                    .into_iter()
                    .collect(),
                    MintBox::TriggerRepetitions(inst) => [node!(
                        Trigger,
                        inst.destination,
                        TriggerW::Increase(inst.object)
                    )]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::Burn(inst) => match inst {
                    BurnBox::Asset(inst) => [node!(
                        AccountAsset,
                        inst.destination.account.signatory,
                        inst.destination.account.domain,
                        inst.destination.definition.name,
                        inst.destination.definition.domain,
                        AccountAssetW::Burn(inst.object)
                    )]
                    .into_iter()
                    .collect(),
                    BurnBox::TriggerRepetitions(inst) => [node!(
                        Trigger,
                        inst.destination,
                        TriggerW::Increase(inst.object)
                    )]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::Transfer(inst) => match inst {
                    TransferBox::Domain(inst) => [
                        node!(
                            DomainAdmin,
                            inst.object.clone(),
                            inst.source.signatory,
                            inst.source.domain,
                            UnitW::Delete(())
                        ),
                        node!(
                            DomainAdmin,
                            inst.object,
                            inst.destination.signatory,
                            inst.destination.domain,
                            UnitW::Create(())
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    TransferBox::AssetDefinition(inst) => [
                        node!(
                            AssetAdmin,
                            inst.object.name.clone(),
                            inst.object.domain.clone(),
                            inst.source.signatory,
                            inst.source.domain,
                            UnitW::Delete(())
                        ),
                        node!(
                            AssetAdmin,
                            inst.object.name,
                            inst.object.domain,
                            inst.destination.signatory,
                            inst.destination.domain,
                            UnitW::Create(())
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    TransferBox::Nft(inst) => [
                        node!(
                            NftOwner,
                            inst.object.name.clone(),
                            inst.object.domain.clone(),
                            inst.source.signatory,
                            inst.source.domain,
                            UnitW::Delete(())
                        ),
                        node!(
                            NftOwner,
                            inst.object.name,
                            inst.object.domain,
                            inst.destination.signatory,
                            inst.destination.domain,
                            UnitW::Create(())
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    TransferBox::Asset(inst) => [
                        node!(
                            AccountAsset,
                            inst.source.account.signatory,
                            inst.source.account.domain,
                            inst.source.definition.name.clone(),
                            inst.source.definition.domain.clone(),
                            AccountAssetW::Send(inst.object)
                        ),
                        node!(
                            AccountAsset,
                            inst.destination.signatory,
                            inst.destination.domain,
                            inst.source.definition.name,
                            inst.source.definition.domain,
                            AccountAssetW::Receive(inst.object)
                        ),
                    ]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::SetKeyValue(inst) => match inst {
                    SetKeyValueBox::Domain(inst) => [node!(
                        DomainMetadata,
                        inst.object,
                        inst.key,
                        MetadataW::Set(inst.value.into())
                    )]
                    .into_iter()
                    .collect(),
                    SetKeyValueBox::Account(inst) => [node!(
                        AccountMetadata,
                        inst.object.signatory,
                        inst.object.domain,
                        inst.key,
                        MetadataW::Set(inst.value.into())
                    )]
                    .into_iter()
                    .collect(),
                    SetKeyValueBox::AssetDefinition(inst) => [node!(
                        AssetMetadata,
                        inst.object.name,
                        inst.object.domain,
                        inst.key,
                        MetadataW::Set(inst.value.into())
                    )]
                    .into_iter()
                    .collect(),
                    SetKeyValueBox::Nft(inst) => [node!(
                        NftData,
                        inst.object.name,
                        inst.object.domain,
                        inst.key,
                        MetadataW::Set(inst.value.into())
                    )]
                    .into_iter()
                    .collect(),
                    SetKeyValueBox::Trigger(inst) => [node!(
                        TriggerMetadata,
                        inst.object,
                        inst.key,
                        MetadataW::Set(inst.value.into())
                    )]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::RemoveKeyValue(inst) => match inst {
                    RemoveKeyValueBox::Domain(inst) => [node!(
                        DomainMetadata,
                        inst.object,
                        inst.key,
                        MetadataW::Unset(())
                    )]
                    .into_iter()
                    .collect(),
                    RemoveKeyValueBox::Account(inst) => [node!(
                        AccountMetadata,
                        inst.object.signatory,
                        inst.object.domain,
                        inst.key,
                        MetadataW::Unset(())
                    )]
                    .into_iter()
                    .collect(),
                    RemoveKeyValueBox::AssetDefinition(inst) => [node!(
                        AssetMetadata,
                        inst.object.name,
                        inst.object.domain,
                        inst.key,
                        MetadataW::Unset(())
                    )]
                    .into_iter()
                    .collect(),
                    RemoveKeyValueBox::Nft(inst) => [node!(
                        NftData,
                        inst.object.name,
                        inst.object.domain,
                        inst.key,
                        MetadataW::Unset(())
                    )]
                    .into_iter()
                    .collect(),
                    RemoveKeyValueBox::Trigger(inst) => [node!(
                        TriggerMetadata,
                        inst.object,
                        inst.key,
                        MetadataW::Unset(())
                    )]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::Grant(inst) => match inst {
                    GrantBox::Permission(inst) => [node!(
                        AccountPermission,
                        inst.destination.signatory,
                        inst.destination.domain,
                        tr::PermissionId::from(&inst.object),
                        UnitW::Create(())
                    )]
                    .into_iter()
                    .collect(),
                    GrantBox::Role(inst) => [node!(
                        AccountRole,
                        inst.destination.signatory,
                        inst.destination.domain,
                        inst.object,
                        UnitW::Create(())
                    )]
                    .into_iter()
                    .collect(),
                    GrantBox::RolePermission(inst) => [node!(
                        RolePermission,
                        inst.destination,
                        tr::PermissionId::from(&inst.object),
                        UnitW::Create(())
                    )]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::Revoke(inst) => match inst {
                    RevokeBox::Permission(inst) => [node!(
                        AccountPermission,
                        inst.destination.signatory,
                        inst.destination.domain,
                        tr::PermissionId::from(&inst.object),
                        UnitW::Delete(())
                    )]
                    .into_iter()
                    .collect(),
                    RevokeBox::Role(inst) => [node!(
                        AccountRole,
                        inst.destination.signatory,
                        inst.destination.domain,
                        inst.object,
                        UnitW::Delete(())
                    )]
                    .into_iter()
                    .collect(),
                    RevokeBox::RolePermission(inst) => [node!(
                        RolePermission,
                        inst.destination,
                        tr::PermissionId::from(&inst.object),
                        UnitW::Delete(())
                    )]
                    .into_iter()
                    .collect(),
                },
                InstructionBox::ExecuteTrigger(_inst) => unimplemented!(
                    "planned to be replaced with calls to pre-registered executables (#5147)"
                ),
                InstructionBox::SetParameter(inst) => [node!(
                    Parameter,
                    tr::ParameterId,
                    ParameterW::Set(inst.0.into())
                )]
                .into_iter()
                .collect(),
                InstructionBox::Upgrade(_inst) => {
                    [node!(Authorizer, AuthorizerW::Set(state::tr::AuthorizerV))]
                        .into_iter()
                        .collect()
                }
                InstructionBox::Log(inst) => {
                    const TARGET: &str = "log_isi";
                    match inst.level {
                        dm::Level::TRACE => iroha_logger::trace!(target: TARGET, "{}", inst.msg),
                        dm::Level::DEBUG => iroha_logger::debug!(target: TARGET, "{}", inst.msg),
                        dm::Level::INFO => iroha_logger::info!(target: TARGET, "{}", inst.msg),
                        dm::Level::WARN => iroha_logger::warn!(target: TARGET, "{}", inst.msg),
                        dm::Level::ERROR => iroha_logger::error!(target: TARGET, "{}", inst.msg),
                    }
                    [].into_iter().collect()
                }
                InstructionBox::Custom(_inst) => unimplemented!(
                    "planned to be replaced with calls to pre-registered executables (#5147)"
                ),
            };

            Ok(changeset)
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use super::*;
    use crate::event::UnitS;

    #[test]
    fn aggregates() {
        let role_w = |i: usize, w: UnitW| node!(Role, format!("role_{i}").parse().unwrap(), w);
        let role_w_set = |i: usize, w: UnitW| ChangeSet::from_iter([role_w(i, w)]);
        assert!((role_w_set(0, UnitW::Create(())) + role_w_set(0, UnitW::Create(()))).is_err());
        assert_eq!(
            role_w_set(0, UnitW::Create(())) + role_w_set(1, UnitW::Create(())),
            Ok(ChangeSet::from_iter([
                role_w(0, UnitW::Create(())),
                role_w(1, UnitW::Create(()))
            ]))
        );
        assert_eq!(
            role_w_set(0, UnitW::Create(())) + role_w_set(0, UnitW::Delete(())),
            Err(Box::new(NodeConflict::new(
                NodeKey::Role(Rc::new("role_0".parse().unwrap())),
                NodeValue::Role(UnitW::Create(())),
                NodeValue::Role(UnitW::Delete(())),
            )))
        );

        let trigger_inc =
            |n: u32| node!(Trigger, "trigger".parse().unwrap(), TriggerW::Increase(n));
        let trigger_inc_set = |n: u32| ChangeSet::from_iter([trigger_inc(n)]);
        assert_eq!(
            (0..5)
                .map(|_| trigger_inc_set(1))
                .try_fold(ChangeSet::default(), |acc, x| acc + x),
            Ok(ChangeSet::from_iter([trigger_inc(5)]))
        );
    }

    #[test]
    fn passes_permission() {
        use permission::Permission;

        let key = |i: usize| dm::RoleId::from_str(&format!("role_{i}")).unwrap();
        let changesets = [
            ChangeSet::default(),
            ChangeSet::from_iter([node!(Role, key(0), UnitW::Create(()))]),
            ChangeSet::from_iter([
                node!(Role, key(0), UnitW::Create(())),
                node!(Role, key(1), UnitW::Create(())),
            ]),
            ChangeSet::from_iter([
                node!(Role, key(0), UnitW::Delete(())),
                node!(Role, key(1), UnitW::Create(())),
            ]),
            ChangeSet::from_iter([node!(Role, key(1), UnitW::Delete(()))]),
        ];
        let permissions = [
            Permission::default(),
            Permission::from_iter([fuzzy_node!(Role, some!(key(0)), UnitS::Create)]),
            Permission::from_iter([fuzzy_node!(Role, None, FilterU8::from_str("c").unwrap())]),
            Permission::from_iter([
                fuzzy_node!(Role, some!(key(0)), UnitS::Delete),
                fuzzy_node!(Role, None, UnitS::Create),
            ]),
            Permission::from_iter([fuzzy_node!(Role, None, FilterU8::from_str("cd").unwrap())]),
        ];

        let missing_permission = changesets[3].passes(&permissions[1]).unwrap_err();
        let complemented_permission = permissions[1].clone() | missing_permission;
        assert!(changesets[3].passes(&complemented_permission).is_ok());

        for (i, changeset) in changesets.iter().enumerate() {
            for (j, permission) in permissions.iter().enumerate() {
                assert_eq!(i <= j, changeset.passes(permission).is_ok());
            }
        }
    }
}
