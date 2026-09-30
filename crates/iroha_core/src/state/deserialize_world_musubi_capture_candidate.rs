//! Test-only paired-tree connection for the actual validated Musubi source.
//!
//! Uses existing semantic schemas and original-pool tree owners. Registration
//! in the complete State catalog remains closed until signature/backend,
//! serializer/schema and nested helper-error custody is complete. No production
//! table reader, finalized root, disclosure permit or IVM anchor is introduced.

use super::*;
use crate::state::authority_registry::leaf::{
    CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits,
};

const TABLES: [&str; 3] = [
    "world.musubi_archive_availability",
    "world.musubi_resolver_index",
    "world.musubi_public_directory",
];

impl<W: MusubiObservationCut> ValidatedMusubiSource<'_, W> {
    /// Capture only this original validated borrow under exact table/aggregate bounds.
    /// The fixed three-owner result drops completed tables if a later table fails.
    fn capture_tables_candidate(
        &self,
        limits: [LeafLimits; 3],
        max_total_rows: u64,
    ) -> Result<[CanonicalTablePairedSnapshot; 3], LeafError> {
        let lengths = [
            self.world.musubi_archive_availability().len(),
            self.world.musubi_resolver_index().len(),
            self.world.musubi_public_directory().len(),
        ];
        let mut total = 0_u64;
        for (length, limit) in lengths.into_iter().zip(limits) {
            let length = u64::try_from(length).map_err(|_| LeafError::RowLimit)?;
            if limit.max_tables == 0 {
                return Err(LeafError::TableLimit);
            }
            total = total.checked_add(length).ok_or(LeafError::RowLimit)?;
            if length > limit.max_rows || total > max_total_rows {
                return Err(LeafError::RowLimit);
            }
        }
        let availability = CanonicalTableLeafSet::paired_semantic_table_from_rows(
            TABLES[0],
            "iroha:state:musubi-availability-authority:v1",
            limits[0],
            self.execution_budget(),
            self.world.musubi_archive_availability().iter(),
            MusubiAvailabilityAuthorityV1::from_record,
        )?;
        let resolver = CanonicalTableLeafSet::paired_semantic_table_from_rows(
            TABLES[1],
            "iroha:state:musubi-resolver-authority:v1",
            limits[1],
            self.execution_budget(),
            self.world.musubi_resolver_index().iter(),
            MusubiResolverAuthorityV1::from_record,
        )?;
        let directory = CanonicalTableLeafSet::paired_semantic_table_from_rows(
            TABLES[2],
            "iroha:state:musubi-directory-authority:v1",
            limits[2],
            self.execution_budget(),
            self.world.musubi_public_directory().iter(),
            MusubiDirectoryAuthorityV1::from_record,
        )?;
        Ok([availability, resolver, directory])
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{SourceWorkLimits, validate};
    use super::*;
    use crate::state::deserialize::decode_tests::seeded_musubi_publication_snapshot;
    use iroha_data_model::musubi::source_work::SourceGeometryLimits;

    fn work() -> SourceWorkLimits {
        SourceWorkLimits {
            geometry: SourceGeometryLimits {
                elements: 1_000_000,
                variable_bytes: 1_000_000,
            },
            table_pass_rows: 1_000_000,
            lookup_index_entries: 1_000_000,
            model_operations: 1_000_000,
            signature_checks: 1_000_000,
        }
    }
    fn leaves() -> [LeafLimits; 3] {
        [LeafLimits {
            max_tables: 1,
            max_rows: 1,
            max_payload_bytes: 4096,
            max_ordered_table_bytes: 64 * 1024,
            max_streamed_value_bytes: 64 * 1024,
        }; 3]
    }

    #[test]
    fn actual_validated_rows_build_three_distinct_retained_pairs_and_paths() {
        let (world, release, archive, selector) = seeded_musubi_publication_snapshot();
        let view = world.view();
        let budget = AllocationBudget::new(8 * 1024 * 1024);
        let source = validate(&view, &budget, work()).unwrap();
        let nodes = source.capture_tables_candidate(leaves(), 3).unwrap();
        for (node, id) in nodes.iter().zip(TABLES) {
            assert_eq!(node.table_id(), id);
            assert_eq!(node.row_count(), 1);
        }
        assert_ne!(nodes[0].root(), nodes[1].root());
        assert_ne!(nodes[1].root(), nodes[2].root());
        macro_rules! check {
            ($index:expr, $key:expr) => {
                let node = &nodes[$index];
                let proof = node.prove_lookup(TABLES[$index], $key).unwrap();
                assert!(
                    CanonicalTableLeafSet::verify_paired_lookup(
                        TABLES[$index],
                        leaves()[$index],
                        &node.root(),
                        &node.ordered_root(),
                        $key,
                        &proof,
                    )
                    .unwrap()
                    .is_some()
                );
            };
        }
        check!(0, &archive);
        check!(1, &release);
        check!(2, &selector);
        assert!(budget.reserved_bytes() > 0);
        // Native storage publication does not alter the token's retained source cut.
        let mut changed = view.musubi_resolver_index().get(&release).unwrap().clone();
        changed.index_revision = 0;
        let mut update = world.musubi_resolver_index.block();
        update.insert(release, changed);
        update.commit();
        assert!(validate(&world.view(), &budget, work()).is_err());
        let same = source.capture_tables_candidate(leaves(), 3).unwrap();
        for (before, after) in nodes.iter().zip(&same) {
            assert_eq!(before.root(), after.root());
        }
        drop(same);
        drop(nodes);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn exact_authority_changes_alter_all_three_roots_after_full_revalidation() {
        let (world, release, archive, selector) = seeded_musubi_publication_snapshot();
        let budget = AllocationBudget::new(8 * 1024 * 1024);
        let old = {
            let view = world.view();
            let source = validate(&view, &budget, work()).unwrap();
            source.capture_tables_candidate(leaves(), 3).unwrap()
        };
        let mut block = world.block();
        let availability = block.musubi_archive_availability.get_mut(&archive).unwrap();
        availability.index_revision = 2;
        availability.finalized_block_hash[0] ^= 1;
        let availability = *availability;
        let resolver = block.musubi_resolver_index.get_mut(&release).unwrap();
        resolver.index_revision = 2;
        resolver.selection.storage = availability;
        block
            .musubi_public_directory
            .get_mut(&selector)
            .unwrap()
            .index_revision = 2;
        *block.musubi_resolver_index_revision.get_mut() =
            MusubiResolverIndexRevisionV1::new(2).unwrap();
        block.commit();
        let view = world.view();
        let source = validate(&view, &budget, work()).unwrap();
        let changed = source.capture_tables_candidate(leaves(), 3).unwrap();
        for (old, changed) in old.iter().zip(&changed) {
            assert_ne!(old.root(), changed.root());
            assert_ne!(old.lookup_root(), changed.lookup_root());
            assert_ne!(old.ordered_root(), changed.ordered_root());
        }
        drop(changed);
        drop(old);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn aggregate_and_each_table_count_are_admitted_before_tree_allocations() {
        let (world, _, _, _) = seeded_musubi_publication_snapshot();
        let view = world.view();
        let budget = AllocationBudget::new(8 * 1024 * 1024);
        let source = validate(&view, &budget, work()).unwrap();
        let peak = budget.peak_reserved_bytes();
        assert!(matches!(
            source.capture_tables_candidate(leaves(), 2),
            Err(LeafError::RowLimit)
        ));
        for index in 0..3 {
            let mut short = leaves();
            short[index].max_rows = 0;
            assert!(matches!(
                source.capture_tables_candidate(short, 3),
                Err(LeafError::RowLimit)
            ));
            short = leaves();
            short[index].max_tables = 0;
            assert!(matches!(
                source.capture_tables_candidate(short, 3),
                Err(LeafError::TableLimit)
            ));
        }
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(budget.peak_reserved_bytes(), peak);
    }

    #[test]
    fn later_table_failure_releases_every_earlier_owner_from_the_original_pool() {
        let (world, _, _, _) = seeded_musubi_publication_snapshot();
        let view = world.view();
        let budget = AllocationBudget::new(8 * 1024 * 1024);
        let source = validate(&view, &budget, work()).unwrap();
        let mut short = leaves();
        short[2].max_streamed_value_bytes = 1;
        assert!(source.capture_tables_candidate(short, 3).is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        budget.set_limit_bytes(0);
        assert!(source.capture_tables_candidate(leaves(), 3).is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        budget.set_limit_bytes(8 * 1024 * 1024);
        let nodes = source.capture_tables_candidate(leaves(), 3).unwrap();
        drop(nodes);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn empty_validated_cut_has_explicit_schema_bound_roots_without_fabricated_rows() {
        let world = World::new();
        let view = world.view();
        let budget = AllocationBudget::new(8 * 1024 * 1024);
        let source = validate(&view, &budget, work()).unwrap();
        let nodes = source.capture_tables_candidate(leaves(), 0).unwrap();
        for (node, table) in nodes.iter().zip(TABLES) {
            assert_eq!(node.table_id(), table);
            assert_eq!(node.row_count(), 0);
        }
        assert_ne!(nodes[0].root(), nodes[1].root());
        assert_ne!(nodes[1].root(), nodes[2].root());
        drop(nodes);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
