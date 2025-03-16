//! Module for [`StateView`] and related components.

use super::*;

/// Represents the state view of each node.
pub type StateView = Tree<View>;

/// Each node value indicates the state view.
#[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
pub struct View;

impl Mode for View {
    type Authorizer = tr::AuthorizerV;
    type Parameter = tr::ParameterV;
    type Peer = tr::UnitV;
    type Domain = tr::DomainV;
    type Account = tr::UnitV;
    type Asset = tr::AssetV;
    type Nft = tr::NftV;
    type AccountAsset = tr::AccountAssetV;
    type Role = tr::UnitV;
    type Permission = tr::PermissionV;
    type AccountRole = tr::UnitV;
    type AccountPermission = tr::UnitV;
    type RolePermission = tr::UnitV;
    type Trigger = tr::TriggerV;
    type Condition = tr::ConditionV;
    type Executable = tr::ExecutableV;
    type TriggerCondition = tr::UnitV;
    type TriggerExecutable = tr::UnitV;
    type DomainMetadata = tr::MetadataV;
    type AccountMetadata = tr::MetadataV;
    type AssetMetadata = tr::MetadataV;
    type NftData = tr::MetadataV;
    type TriggerMetadata = tr::MetadataV;
    type DomainAdmin = tr::UnitV;
    type AssetAdmin = tr::UnitV;
    type NftAdmin = tr::UnitV;
    type NftOwner = tr::UnitV;
    type TriggerAdmin = tr::UnitV;
}

impl NodeReadWrite for StateView {
    type Status = event::Event;

    fn as_status(&self) -> Self::Status {
        self.iter()
            .map(|(k, v)| NodeEntry::try_from((k.clone(), v.into())).unwrap())
            .collect()
    }
}

/// Interface for interacting with the main state of the application.
pub trait WorldState {
    /// Indicates that the write request was rejected due to data integrity violations.
    type InvariantViolation: From<Box<NodeConflict<changeset::Write>>>;

    /// Applies a write entry to the state.
    ///
    /// # Errors
    ///
    /// Fails if the update violates data integrity constraints.
    fn update_by(
        &mut self,
        entry: NodeEntry<changeset::Write>,
    ) -> Result<(), Self::InvariantViolation>;

    /// Scans for inconsistencies based on event predictions and attempts to resolve them, returning an additional changeset.
    ///
    /// # Errors
    ///
    /// Fails if the expected change is determined to break data integrity.
    fn sanitize(
        &self,
        event_prediction: &event::Event,
    ) -> Result<changeset::ChangeSet, Self::InvariantViolation>;

    /// Retrieve stored values based on the `readset` query.
    fn load(&self, readset: &readset::ReadSet) -> StateView;

    /// Applies an unordered changeset to the state, resulting in events.
    ///
    /// # Errors
    ///
    /// Fails if the update violates data integrity constraints.
    fn update(
        &mut self,
        changeset: changeset::ChangeSet,
    ) -> Result<event::Event, Self::InvariantViolation> {
        let event_prediction = changeset.as_status();
        let changeset = (changeset + self.sanitize(&event_prediction)?)?;
        let event = changeset.as_status();

        for (key, value) in changeset {
            self.update_by(NodeEntry::try_from((key, value)).unwrap())?;
        }

        Ok(event)
    }
}

#[allow(missing_docs)]
pub mod transitional {
    use hashbrown::{HashMap, HashSet};

    use super::*;

    /// State view at `Unit` type nodes.
    #[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
    pub struct UnitV;

    /// State view at `Authorizer` type nodes.
    #[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
    pub struct AuthorizerV;

    /// State view at `Parameter` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub struct ParameterV {
        pub(crate) parameter: dm::Parameter,
    }

    /// State view at `Domain` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub struct DomainV {
        pub(crate) logo: Option<dm::IpfsPath>,
    }

    /// State view at `Asset` type nodes.
    #[derive(Debug, PartialEq, Eq, Constructor, Clone, Decode, Encode)]
    pub struct AssetV {
        pub(crate) total_quantity: dm::Numeric,
        pub(crate) mintable: dm::Mintable,
        pub(crate) logo: Option<dm::IpfsPath>,
    }

    /// State view at `Nft` type nodes.
    #[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
    pub struct NftV;

    /// State view at `AccountAsset` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub struct AccountAssetV {
        pub(crate) balance: dm::Numeric,
    }

    /// State view at `Permission` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub struct PermissionV {
        pub(crate) permission: permission::Permission,
    }

    /// State view at `Trigger` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub struct TriggerV {
        pub(crate) repeats: dm::Repeats,
    }

    /// State view at `Condition` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub enum ConditionV {
        World(receptor::Receptor),
        Time(dm::TimeSchedule),
        Block(BlockCommit),
    }

    #[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
    pub struct BlockCommit;

    /// State view at `Executable` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub enum ExecutableV {
        Static(changeset::ChangeSet),
        Dynamic(WasmExecutable),
    }

    #[derive(Debug, PartialEq, Eq, Clone, Decode, Encode)]
    pub struct WasmExecutable;

    /// State view at `Metadata` type nodes.
    #[derive(Debug, PartialEq, Eq, From, Clone, Decode, Encode)]
    pub struct MetadataV {
        pub(crate) json: dm::Json,
    }

    #[derive(Debug, PartialEq, Eq, Constructor, Clone)]
    pub struct TriggerEntry<'a> {
        pub id: &'a dm::TriggerId,
        pub condition: &'a ConditionV,
        pub executable: &'a ExecutableV,
    }

    #[derive(Debug, PartialEq, Eq, Constructor, Clone)]
    pub struct WorldTriggerEntry<'a> {
        pub id: &'a dm::TriggerId,
        pub receptor: &'a receptor::Receptor,
        pub executable: &'a ExecutableV,
    }

    impl<'a> TryFrom<TriggerEntry<'a>> for WorldTriggerEntry<'a> {
        type Error = &'static str;

        fn try_from(entry: TriggerEntry<'a>) -> Result<Self, Self::Error> {
            match entry.condition {
                ConditionV::World(receptor) => {
                    Ok(WorldTriggerEntry::new(entry.id, receptor, entry.executable))
                }
                _ => Err("conversion succeeds only when this trigger subscribes to world events"),
            }
        }
    }

    impl StateView {
        pub fn triggers(&self) -> impl Iterator<Item = TriggerEntry> {
            let ids = self.keys().filter_map(|k| match k {
                NodeKey::Trigger(id) => Some(&**id),
                _ => None,
            });
            let conditions: HashMap<_, _> = self
                .keys()
                .filter_map(|k| match k {
                    NodeKey::TriggerCondition((trg, con)) => {
                        let Some(NodeValue::Condition(condition)) =
                            self.get(&NodeKey::Condition(con.clone()))
                        else {
                            panic!("should be loaded into the state view")
                        };
                        Some((&**trg, condition))
                    }
                    _ => None,
                })
                .collect();
            let executables: HashMap<_, _> = self
                .keys()
                .filter_map(|k| match k {
                    NodeKey::TriggerExecutable((trg, exe)) => {
                        let Some(NodeValue::Executable(executable)) =
                            self.get(&NodeKey::Executable(exe.clone()))
                        else {
                            panic!("should be loaded into the state view")
                        };
                        Some((&**trg, executable))
                    }
                    _ => None,
                })
                .collect();

            ids.map(move |id| {
                let condition = *conditions
                    .get(id)
                    .expect("should be loaded into the state view");
                let executable = *executables
                    .get(id)
                    .expect("should be loaded into the state view");
                TriggerEntry::new(id, condition, executable)
            })
        }

        pub fn world_triggers(&self) -> impl Iterator<Item = WorldTriggerEntry> {
            self.triggers().filter_map(|entry| entry.try_into().ok())
        }
    }

    impl TriggerEntry<'_> {
        pub fn leads_to_event_loop(&self, state: &StateView) -> bool {
            let empty_receptor = receptor::Receptor::default();
            let mut world_triggers: HashMap<_, _> = state
                .world_triggers()
                .map(|entry| (entry.id, (entry.receptor, entry.executable)))
                .collect();
            let receptor = match self.condition {
                ConditionV::World(receptor) => receptor,
                _ => &empty_receptor,
            };
            world_triggers.insert(self.id, (receptor, self.executable));
            let mut stack = vec![self.id];
            let mut seen = HashSet::new();
            while let Some(trigger_id) = stack.pop() {
                if seen.contains(&trigger_id) {
                    return true;
                }
                seen.insert(trigger_id);
                let event_expected = match &world_triggers[trigger_id].1 {
                    state::tr::ExecutableV::Static(changeset) => changeset.as_status(),
                    state::tr::ExecutableV::Dynamic(_wasm) => {
                        todo!("Wasm executable should declare the union of possible events (#5362)")
                    }
                };
                if event_expected.iter().any(|(_k, v)| {
                    matches!(
                        v,
                        NodeValue::TriggerCondition(event::UnitS::Create)
                            | NodeValue::TriggerExecutable(event::UnitS::Create)
                    )
                }) {
                    // Trigger registration by another trigger is not allowed unless Wasm executables declare the candidate trigger executables.
                    return true;
                }
                let next_trigger_ids = world_triggers.iter().filter_map(|(id, (receptor, _))| {
                    event_expected.passes(receptor).is_ok().then_some(id)
                });
                stack.extend(next_trigger_ids);
            }
            false
        }
    }

    impl TryFrom<(dm::AccountId, dm::Executable)> for ExecutableV {
        type Error = Box<NodeConflict<changeset::Write>>;

        fn try_from(
            (authority, executable): (dm::AccountId, dm::Executable),
        ) -> Result<Self, Self::Error> {
            match executable {
                dm::Executable::Instructions(instructions) => {
                    let changeset =
                        changeset::ChangeSet::try_from((authority, instructions.into_vec()))?;
                    Ok(changeset.into())
                }
                dm::Executable::Wasm(_wasm) => Ok(WasmExecutable.into()),
            }
        }
    }
}

pub use transitional as tr;

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use dm::{DomainId, Repeats, TriggerId};

    use super::{transitional::TriggerEntry, *};
    use crate::{
        changeset::{ChangeSet, ConditionW, DomainW, ExecutableW, TriggerW, UnitW},
        receptor::Receptor,
    };

    /// See the corresponding integration test `triggers::not_registered_when_potential_event_loop_detected`.
    #[expect(clippy::too_many_lines)]
    #[test]
    fn detects_event_loop() {
        // Subscribes to changes in the domain "dom_{i}" with statuses "{s}".
        let receptor = |i: usize, s: &str| {
            Receptor::from_iter([fuzzy_node!(
                Domain,
                some!(DomainId::from_str(&format!("dom_{i}")).unwrap()),
                FilterU8::from_str(s).unwrap()
            )])
        };
        // Publishes the deletion of the domain "dom_{j}".
        let changeset = |j: usize| {
            ChangeSet::from_iter([node!(
                Domain,
                DomainId::from_str(&format!("dom_{j}")).unwrap(),
                DomainW::Delete(())
            )])
        };
        // Bridges the above subscriber and publisher.
        let trigger = |i: usize, s: &str, j: usize| {
            let condition = tr::ConditionV::from(receptor(i, s));
            let executable = tr::ExecutableV::from(changeset(j));
            (
                (
                    TriggerId::from_str(&format!("trg_{i}{s}_{j}d")).unwrap(),
                    tr::TriggerV::from(Repeats::Indefinitely),
                ),
                (crate::tr::ConditionId::from(&condition), condition),
                (crate::tr::ExecutableId::from(&executable), executable),
            )
        };
        // A potential connection exists through the deletion of "dom_1".
        let (trg_0d_1d, trg_1d_2d) = (trigger(0, "d", 1), trigger(1, "d", 2));
        // The state after registering the above triggers.
        let state = StateView::from_iter([
            node!(Condition, trg_0d_1d.1 .0.clone(), trg_0d_1d.1 .1),
            node!(Condition, trg_1d_2d.1 .0.clone(), trg_1d_2d.1 .1),
            node!(Executable, trg_0d_1d.2 .0.clone(), trg_0d_1d.2 .1),
            node!(Executable, trg_1d_2d.2 .0.clone(), trg_1d_2d.2 .1),
            node!(
                TriggerCondition,
                trg_0d_1d.0 .0.clone(),
                trg_0d_1d.1 .0,
                tr::UnitV
            ),
            node!(
                TriggerCondition,
                trg_1d_2d.0 .0.clone(),
                trg_1d_2d.1 .0,
                tr::UnitV
            ),
            node!(
                TriggerExecutable,
                trg_0d_1d.0 .0.clone(),
                trg_0d_1d.2 .0,
                tr::UnitV
            ),
            node!(
                TriggerExecutable,
                trg_1d_2d.0 .0.clone(),
                trg_1d_2d.2 .0,
                tr::UnitV
            ),
            node!(Trigger, trg_0d_1d.0 .0, trg_0d_1d.0 .1),
            node!(Trigger, trg_1d_2d.0 .0, trg_1d_2d.0 .1),
        ]);

        for (entry, leads_to_event_loop) in [
            // Short-circuiting.
            (trigger(2, "d", 0), true),
            // No short-circuiting due to status mismatch.
            (trigger(2, "cu", 0), false),
            // Extending the graph.
            (trigger(2, "d", 3), false),
            // Creating another cyclic cluster.
            (trigger(3, "d", 3), true),
            // Creating another acyclic cluster.
            (trigger(3, "d", 4), false),
            {
                let mut trg_3d_x = trigger(3, "d", 4);
                let another = trigger(10, "", 20);
                trg_3d_x.2 .1 = ChangeSet::from_iter([
                    node!(
                        Condition,
                        another.1 .0.clone(),
                        ConditionW::Set(another.1 .1)
                    ),
                    node!(
                        Executable,
                        another.2 .0.clone(),
                        ExecutableW::Set(another.2 .1)
                    ),
                    node!(
                        TriggerCondition,
                        another.0 .0.clone(),
                        another.1 .0,
                        UnitW::Create(())
                    ),
                    node!(
                        TriggerExecutable,
                        another.0 .0.clone(),
                        another.2 .0,
                        UnitW::Create(())
                    ),
                    node!(Trigger, another.0 .0, TriggerW::Create(another.0 .1)),
                ])
                .into();
                // Creating an additional trigger.
                (trg_3d_x, true)
            },
        ]
        .iter()
        .map(|(trg, b)| (TriggerEntry::new(&trg.0 .0, &trg.1 .1, &trg.2 .1), *b))
        {
            assert_eq!(leads_to_event_loop, entry.leads_to_event_loop(&state));
        }
    }

    #[test]
    fn passes_permission() {
        use permission::Permission;

        let key = |i: usize| dm::RoleId::from_str(&format!("role_{i}")).unwrap();
        let views = [
            StateView::default(),
            StateView::from_iter([node!(Role, key(0), tr::UnitV)]),
            StateView::from_iter([
                node!(Role, key(0), tr::UnitV),
                node!(Role, key(1), tr::UnitV),
            ]),
        ];
        let permissions = [
            Permission::default(),
            Permission::from_iter([fuzzy_node!(Role, some!(key(0)), event::UnitS::Read)]),
            Permission::from_iter([fuzzy_node!(Role, None, FilterU8::from_str("r").unwrap())]),
        ];

        let missing_permission = views[2].passes(&permissions[1]).unwrap_err();
        let complemented_permission = permissions[1].clone() | missing_permission;
        assert!(views[2].passes(&complemented_permission).is_ok());

        for (i, view) in views.iter().enumerate() {
            for (j, permission) in permissions.iter().enumerate() {
                assert_eq!(i <= j, view.passes(permission).is_ok());
            }
        }
    }
}
