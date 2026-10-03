//! Exact cycle errors and allocation-free authority dependency traversal.

use super::super::{
    Canonical, CompleteInventoryError, DerivationCheck, Field, Role, STATE_FIELDS,
    check_derivation_source, visit,
};
use super::DerivationPath;
use crate::{state::authority_registry::schema, test_allocations::allocations_during};

#[test]
fn borrowed_branches_keep_only_their_own_ancestors() {
    assert_eq!(
        allocations_during(|| {
            let root = DerivationPath::root("root");
            let left = root.child("left");
            let inner = left.child("inner");
            let right = root.child("right");
            assert!(inner.contains("root"));
            assert!(inner.contains("left"));
            assert!(inner.contains("inner"));
            assert!(!inner.contains("right"));
            assert!(right.contains("root"));
            assert!(!right.contains("left"));
            assert!(!root.contains("inner"));
        }),
        0
    );
}

#[test]
fn every_actual_rebuild_source_checks_without_heap_scratch() {
    let mut checked = 0;
    assert_eq!(
        allocations_during(|| {
            visit(STATE_FIELDS, &mut |field| {
                if let Role::Derived {
                    sources,
                    check: DerivationCheck::Rebuild(_),
                } = field.role
                {
                    let path = DerivationPath::root(field.id);
                    for source in sources {
                        assert_eq!(check_derivation_source(STATE_FIELDS, source, &path), Ok(()));
                        checked += 1;
                    }
                }
            });
        }),
        0
    );
    assert!(checked > 20, "exercise the actual nested State inventory");
}

#[test]
fn shared_dependencies_and_back_edges_preserve_exact_errors_without_allocation() {
    const FIELDS: &[Field] = &[
        Field::new(
            "test.rows",
            Role::Canonical(Canonical::Cell(schema::<u64>())),
        ),
        Field::new("test.cache", Role::Local("physical cache")),
        Field::new(
            "test.shared",
            Role::Derived {
                sources: &["test.rows"],
                check: DerivationCheck::Rebuild("shared"),
            },
        ),
        Field::new(
            "test.diamond",
            Role::Derived {
                sources: &["test.shared", "test.rows", "test.shared"],
                check: DerivationCheck::Rebuild("diamond"),
            },
        ),
        Field::new(
            "test.first",
            Role::Derived {
                sources: &["test.second"],
                check: DerivationCheck::Rebuild("first"),
            },
        ),
        Field::new(
            "test.second",
            Role::Derived {
                sources: &["test.first"],
                check: DerivationCheck::Rebuild("second"),
            },
        ),
    ];
    assert_eq!(
        allocations_during(|| {
            let path = DerivationPath::root("test.root");
            assert_eq!(
                check_derivation_source(FIELDS, "test.diamond", &path),
                Ok(())
            );
            assert_eq!(
                check_derivation_source(FIELDS, "test.first", &path),
                Err(CompleteInventoryError::DerivationCycle("test.first"))
            );
            assert_eq!(
                check_derivation_source(FIELDS, "test.cache", &path),
                Err(CompleteInventoryError::NonAuthoritySource("test.cache"))
            );
            assert_eq!(
                check_derivation_source(FIELDS, "test.absent", &path),
                Err(CompleteInventoryError::UnknownSource("test.absent"))
            );
            assert_eq!(check_derivation_source(FIELDS, "test.rows", &path), Ok(()));
        }),
        0
    );
}
