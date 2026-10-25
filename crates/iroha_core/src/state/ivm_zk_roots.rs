//! Immutable IVM confidential-root projections, shared only across unchanged storage.

use super::*;

/// Fields of one confidential tree observable through `ZK_ROOTS_GET`.
///
/// Commitment leaves and nullifiers stay in the World: a root read observes
/// the tree profile, the bounded retained root history and the commitment
/// count, never an individual leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IvmZkTreeRoots {
    /// Construction that defines the empty-tree root.
    pub tree_profile: ConfidentialTreeProfile,
    /// Retained roots, oldest first; the last one is the current root.
    pub root_history: Vec<[u8; 32]>,
    /// Number of commitments appended to the tree.
    pub commitment_count: usize,
}

/// Validated confidential-tree fields observable through IVM root reads.
pub type IvmZkRootsSnapshot = BTreeMap<AssetDefinitionId, IvmZkTreeRoots>;

/// A confidential tree's metadata contradicts its commitment count or profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid IVM confidential-root snapshot")]
pub struct InvalidIvmZkRootsSnapshot;

pub(super) type SnapshotCache =
    SyncOnceCell<Result<Arc<IvmZkRootsSnapshot>, InvalidIvmZkRootsSnapshot>>;

/// Storage evidence that the block-start projection is still exact.
pub(super) trait UnchangedZkAssets: WorldReadOnly {
    fn zk_assets_unchanged(&self) -> bool;
}

impl UnchangedZkAssets for WorldView<'_> {
    fn zk_assets_unchanged(&self) -> bool {
        true
    }
}

impl UnchangedZkAssets for WorldBlock<'_> {
    fn zk_assets_unchanged(&self) -> bool {
        !self.zk_assets.is_dirty()
    }
}

impl UnchangedZkAssets for WorldTransaction<'_, '_> {
    fn zk_assets_unchanged(&self) -> bool {
        self.zk_assets.changed_keys_in_block().next().is_none()
    }
}

impl UnchangedZkAssets for Box<WorldTransaction<'_, '_>> {
    fn zk_assets_unchanged(&self) -> bool {
        (**self).zk_assets_unchanged()
    }
}

pub(super) fn cached_snapshot(
    cache: &SnapshotCache,
    world: &impl UnchangedZkAssets,
) -> Result<Arc<IvmZkRootsSnapshot>, InvalidIvmZkRootsSnapshot> {
    if world.zk_assets_unchanged() {
        cache
            .get_or_init(|| project(world.zk_assets().iter()))
            .clone()
    } else {
        // A changed projection is invocation-local. It must not replace the
        // block-start cache: rollback can make that exact baseline usable again.
        project(world.zk_assets().iter())
    }
}

/// Validate confidential trees in iteration order and copy only what root reads observe.
///
/// # Errors
/// Rejects the complete projection when any tree's metadata fails
/// [`ZkAssetState::validate_tree_metadata`].
pub fn project<'a>(
    trees: impl IntoIterator<Item = (&'a AssetDefinitionId, &'a ZkAssetState)>,
) -> Result<Arc<IvmZkRootsSnapshot>, InvalidIvmZkRootsSnapshot> {
    let mut roots = IvmZkRootsSnapshot::new();
    for (asset_id, tree) in trees {
        tree.validate_tree_metadata()
            .map_err(|_| InvalidIvmZkRootsSnapshot)?;
        roots.insert(
            asset_id.clone(),
            IvmZkTreeRoots {
                tree_profile: tree.tree_profile,
                root_history: tree.root_history.clone(),
                commitment_count: tree.commitments.len(),
            },
        );
    }
    Ok(Arc::new(roots))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("zkroots", "universal").expect("fixture domain"),
            name.parse().expect("fixture asset name"),
        )
    }

    fn tree(leaves: &[[u8; 32]]) -> ZkAssetState {
        let mut tree = ZkAssetState::default();
        tree.push_commitments(leaves, NonZeroUsize::new(2).expect("non-zero cap"))
            .expect("canonical confidential tree");
        tree
    }

    /// A tree that retains more roots than it has commitments.
    fn invalid_tree() -> ZkAssetState {
        let mut tree = tree(&[[0x21; 32]]);
        tree.root_history.insert(0, [0x66; 32]);
        tree
    }

    fn history(snapshot: &IvmZkRootsSnapshot, name: &str) -> Vec<[u8; 32]> {
        snapshot[&asset(name)].root_history.clone()
    }

    fn state() -> State {
        let mut world = World::new();
        world.zk_assets.insert(asset("a"), tree(&[[0x21; 32]]));
        State::new_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        )
    }

    #[test]
    fn projection_copies_read_fields_and_rejects_any_invalid_tree() {
        let filled = tree(&[[0x21; 32], [0x22; 32], [0x23; 32]]);
        let empty = ZkAssetState::default();
        let trees = BTreeMap::from([(asset("a"), filled.clone()), (asset("b"), empty.clone())]);
        let snapshot = project(&trees).unwrap();
        assert_eq!(
            snapshot[&asset("a")],
            IvmZkTreeRoots {
                tree_profile: filled.tree_profile,
                root_history: filled.root_history.clone(),
                commitment_count: 3,
            }
        );
        assert_eq!(filled.root_history.len(), 2);
        assert_eq!(snapshot[&asset("b")].commitment_count, 0);
        assert!(snapshot[&asset("b")].root_history.is_empty());
        assert!(
            project(&BTreeMap::<AssetDefinitionId, ZkAssetState>::new())
                .unwrap()
                .is_empty()
        );
        for position in ["a", "m", "z"] {
            let mut trees = trees.clone();
            trees.insert(asset(position), invalid_tree());
            assert_eq!(project(&trees), Err(InvalidIvmZkRootsSnapshot));
        }
    }

    #[test]
    fn immutable_views_share_their_exact_validated_projection() {
        let state = state();
        let view = state.view();
        let first = view.ivm_zk_roots_snapshot().unwrap();
        assert!(Arc::ptr_eq(&first, &view.ivm_zk_roots_snapshot().unwrap()));
        assert_eq!(history(&first, "a"), tree(&[[0x21; 32]]).root_history);
        let query = state.query_view();
        let first_query = query.ivm_zk_roots_snapshot().unwrap();
        assert!(Arc::ptr_eq(
            &first_query,
            &query.ivm_zk_roots_snapshot().unwrap()
        ));
        assert_eq!(first, first_query);
    }

    #[test]
    fn untouched_siblings_share_cache_but_edits_apply_and_rollback_exactly() {
        let one = tree(&[[0x21; 32]]).root_history;
        let two = tree(&[[0x21; 32], [0x22; 32]]).root_history;
        let three = tree(&[[0x21; 32], [0x22; 32], [0x23; 32]]).root_history;
        let state = state();
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let baseline = block.ivm_zk_roots_snapshot().unwrap();
        {
            let mut tx = block.transaction();
            assert!(Arc::ptr_eq(&baseline, &tx.ivm_zk_roots_snapshot().unwrap()));
            tx.world
                .zk_assets
                .insert(asset("a"), tree(&[[0x21; 32], [0x22; 32]]));
            let changed = tx.ivm_zk_roots_snapshot().unwrap();
            assert_eq!(history(&changed, "a"), two);
            assert_eq!(history(&baseline, "a"), one);
            assert!(!Arc::ptr_eq(&baseline, &changed));
            // Drop rolls the tree update back; it must not poison baseline.
        }
        assert!(Arc::ptr_eq(
            &baseline,
            &block.ivm_zk_roots_snapshot().unwrap()
        ));
        {
            let tx = block.transaction();
            assert!(Arc::ptr_eq(&baseline, &tx.ivm_zk_roots_snapshot().unwrap()));
            tx.apply();
        }
        assert!(Arc::ptr_eq(
            &baseline,
            &block.ivm_zk_roots_snapshot().unwrap()
        ));
        {
            let mut tx = block.transaction();
            tx.world
                .zk_assets
                .insert(asset("a"), tree(&[[0x21; 32], [0x22; 32], [0x23; 32]]));
            tx.apply();
        }
        assert_eq!(history(&block.ivm_zk_roots_snapshot().unwrap(), "a"), three);
        {
            let mut tx = block.transaction();
            assert_eq!(history(&tx.ivm_zk_roots_snapshot().unwrap(), "a"), three);
            tx.world.zk_assets.remove(asset("a"));
            assert!(tx.ivm_zk_roots_snapshot().unwrap().is_empty());
        }
        assert_eq!(history(&block.ivm_zk_roots_snapshot().unwrap(), "a"), three);
        assert_eq!(history(&baseline, "a"), one);
    }

    #[test]
    fn cached_success_never_hides_invalid_edits_or_repaired_state() {
        let mut world = World::new();
        world.zk_assets.insert(asset("a"), tree(&[[0x21; 32]]));
        let mut block = world.block();
        let cache = SnapshotCache::new();
        let baseline = cached_snapshot(&cache, &block).unwrap();
        block.zk_assets.insert(asset("a"), invalid_tree());
        assert!(cached_snapshot(&cache, &block).is_err());
        block
            .zk_assets
            .insert(asset("a"), tree(&[[0x21; 32], [0x22; 32]]));
        let repaired = cached_snapshot(&cache, &block).unwrap();
        assert_eq!(
            history(&repaired, "a"),
            tree(&[[0x21; 32], [0x22; 32]]).root_history
        );
        block.zk_assets.insert(asset("b"), invalid_tree());
        assert!(cached_snapshot(&cache, &block).is_err());
        block.zk_assets.remove(asset("b"));
        assert_eq!(cached_snapshot(&cache, &block).unwrap(), repaired);
        assert_eq!(history(&baseline, "a"), tree(&[[0x21; 32]]).root_history);
        assert!(Arc::ptr_eq(
            &baseline,
            cache.get().unwrap().as_ref().unwrap()
        ));
    }

    #[test]
    fn invalid_baseline_is_cached_only_while_storage_remains_unchanged() {
        let mut world = World::new();
        world.zk_assets.insert(asset("a"), invalid_tree());
        let mut block = world.block();
        let cache = SnapshotCache::new();
        assert!(cached_snapshot(&cache, &block).is_err());
        assert!(cache.get().unwrap().is_err());
        {
            let mut tx = block
                .try_transaction(
                    #[cfg(feature = "telemetry")]
                    None,
                    LaneConfig::default(),
                    0,
                )
                .unwrap();
            assert!(cached_snapshot(&cache, &tx).is_err());
            tx.zk_assets.insert(asset("a"), tree(&[[0x21; 32]]));
            assert_eq!(
                history(&cached_snapshot(&cache, &tx).unwrap(), "a"),
                tree(&[[0x21; 32]]).root_history
            );
        }
        assert!(cached_snapshot(&cache, &block).is_err());
        assert!(cache.get().unwrap().is_err());
    }
}
