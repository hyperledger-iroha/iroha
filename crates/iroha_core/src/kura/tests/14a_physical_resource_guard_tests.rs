// Composed production guards on actual configured Kura stores; no owner is inferred absent.

fn physical_guard_fixture() -> (TempDir, Arc<Kura>, PathBuf, PathBuf) {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let owner = kura.store_root.join("blocks/guard-observation");
    fs::create_dir(&owner).unwrap();
    let left = owner.join(DATA_FILE_NAME);
    let right = owner.join(INDEX_FILE_NAME);
    fs::write(&left, [1_u8; 9]).unwrap();
    fs::write(&right, [0_u8; 16]).unwrap();
    let counts = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    kura.resource_inventory
        .initialize(
            kura.resource_inventory.reconciliation_generation().unwrap(),
            &PHYSICAL_RESOURCE_FAMILIES
                .iter()
                .map(|family| (*family, counts[*family as usize]))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    physical_guard_assert_actual(&kura);
    (directory, kura, left, right)
}

fn physical_guard_assert_actual(kura: &Kura) {
    let counts = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap(),
            counts[family as usize],
            "{family:?}"
        );
    }
}

fn physical_guard_assert_unavailable(kura: &Kura) {
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?}"
        );
    }
}

fn physical_guard_assert_busy(kura: &Kura) {
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap_err(),
            resource_inventory::Unavailable::Busy,
            "{family:?}"
        );
    }
}

fn select_physical_guard_scope<'a>(
    guard: TotalDiskUsageMutation<'a>,
    selection: usize,
    left: &Path,
) -> TotalDiskUsageMutation<'a> {
    let owner = left.parent().unwrap();
    match selection {
        0 => guard.with_resource_paths(vec![left.to_path_buf()]),
        1 => guard.with_startup_resource_tree(owner),
        2 => guard.removing_resource_tree(owner),
        3 => guard.with_resource_tree_move(owner, &owner.with_file_name("moved-observation")),
        4 => guard.with_resource_children(2),
        _ => unreachable!("closed fixture scope selection"),
    }
}

#[test]
fn physical_guard_mixed_direct_and_child_scope_cannot_publish_double_delta() {
    let (_directory, kura, left, _) = physical_guard_fixture();
    let mut parent = kura
        .begin_total_disk_usage_mutation()
        .with_resource_paths(vec![left.clone()])
        .with_resource_children(1);
    let child = parent.resource_child(vec![left.clone()]);
    fs::write(&left, [2_u8; 23]).unwrap();
    child.finish();
    parent.finish();
    assert_eq!(fs::metadata(left).unwrap().len(), 23);
    physical_guard_assert_unavailable(&kura);
}

#[test]
fn physical_guard_every_second_scope_selection_is_rejected() {
    for first in 0..5 {
        for second in 0..5 {
            let (_directory, kura, left, _) = physical_guard_fixture();
            let guard =
                select_physical_guard_scope(kura.begin_total_disk_usage_mutation(), first, &left);
            let guard = select_physical_guard_scope(guard, second, &left);
            guard.finish();
            physical_guard_assert_unavailable(&kura);
        }
    }
}

#[test]
fn physical_guard_missing_child_budget_cannot_be_reset_to_zero() {
    let (_directory, kura, _, _) = physical_guard_fixture();
    kura.begin_total_disk_usage_mutation()
        .with_resource_children(1)
        .with_resource_children(0)
        .finish();
    physical_guard_assert_unavailable(&kura);
}

#[test]
fn physical_guard_exact_direct_children_publish_once_under_parent_busy() {
    let (_directory, kura, left, right) = physical_guard_fixture();
    let mut parent = kura
        .begin_total_disk_usage_mutation()
        .with_resource_children(2);
    let first = parent.resource_child(vec![left.clone()]);
    fs::write(left, [2_u8; 23]).unwrap();
    first.finish();
    physical_guard_assert_busy(&kura);
    let second = parent.resource_child(vec![right.clone()]);
    fs::write(right, [0_u8; 48]).unwrap();
    second.finish();
    physical_guard_assert_busy(&kura);
    parent.finish();
    physical_guard_assert_actual(&kura);
}

#[test]
fn physical_guard_missing_extra_and_invalid_children_remain_unavailable() {
    for scenario in 0..5 {
        let (_directory, kura, left, right) = physical_guard_fixture();
        let mut parent = kura
            .begin_total_disk_usage_mutation()
            .with_resource_children(if scenario == 0 { 2 } else { 1 });
        match scenario {
            0 => parent.resource_child(vec![left.clone()]).finish(),
            1 => {
                parent.resource_child(vec![left.clone()]).finish();
                parent.resource_child(vec![right.clone()]).finish();
            }
            2 => parent
                .resource_child(vec![left.clone(), left.clone()])
                .finish(),
            3 => {
                let child = parent.resource_child(vec![left.clone()]);
                fs::remove_file(&left).unwrap();
                fs::create_dir(&left).unwrap();
                child.finish();
            }
            4 => parent
                .resource_child(vec![kura.store_root.parent().unwrap().join("outside.data")])
                .finish(),
            _ => unreachable!(),
        }
        parent.finish();
        physical_guard_assert_unavailable(&kura);
    }
}

#[test]
fn physical_guard_dropped_or_forgotten_child_cannot_qualify_parent() {
    for forget in [false, true] {
        let (_directory, kura, left, _) = physical_guard_fixture();
        let mut parent = kura
            .begin_total_disk_usage_mutation()
            .with_resource_children(1);
        let child = parent.resource_child(vec![left.clone()]);
        fs::write(left, [2_u8; 23]).unwrap();
        if forget {
            std::mem::forget(child);
        } else {
            drop(child);
        }
        parent.finish();
        physical_guard_assert_unavailable(&kura);
    }
}

#[test]
fn physical_guard_nested_batch_discharges_exactly_one_parent_slot() {
    let (_directory, kura, left, right) = physical_guard_fixture();
    let mut parent = kura
        .begin_total_disk_usage_mutation()
        .with_resource_children(2);
    let mut nested = parent.resource_batch(2);
    let first = nested.guard().resource_child(vec![left.clone()]);
    fs::write(&left, [2_u8; 23]).unwrap();
    first.finish();
    let second = nested.guard().resource_child(vec![right.clone()]);
    fs::write(right, [0_u8; 48]).unwrap();
    second.finish();
    physical_guard_assert_busy(&kura);
    nested.finish();
    physical_guard_assert_busy(&kura);
    let last = parent.resource_child(vec![left.clone()]);
    fs::write(left, [3_u8; 29]).unwrap();
    last.finish();
    parent.finish();
    physical_guard_assert_actual(&kura);
}

#[test]
fn physical_guard_failed_dropped_and_forgotten_nested_batches_remain_unavailable() {
    for scenario in 0..5 {
        let (_directory, kura, left, _) = physical_guard_fixture();
        let mut parent = kura
            .begin_total_disk_usage_mutation()
            .with_resource_children(1);
        let mut nested = parent.resource_batch(1);
        match scenario {
            0 => nested.finish(),
            1 => {
                nested.guard().resource_child(vec![left.clone()]).finish();
                nested.guard().resource_child(vec![left]).finish();
                nested.finish();
            }
            2 => {
                nested
                    .guard()
                    .resource_child(vec![left.clone(), left])
                    .finish();
                nested.finish();
            }
            3 => drop(nested),
            4 => std::mem::forget(nested),
            _ => unreachable!(),
        }
        parent.finish();
        physical_guard_assert_unavailable(&kura);
    }
}

#[test]
fn physical_guard_empty_nested_batch_and_explicit_disk_rescan_keep_distinct_semantics() {
    let (_directory, kura, left, _) = physical_guard_fixture();
    let mut parent = kura
        .begin_total_disk_usage_mutation()
        .with_resource_children(1);
    parent.resource_batch(0).finish();
    physical_guard_assert_busy(&kura);
    parent.finish();
    physical_guard_assert_actual(&kura);
    kura.disk_usage_initialized.store(true, Ordering::Relaxed);
    kura.disk_usage_total_initialized
        .store(true, Ordering::Relaxed);
    let guard = kura
        .begin_total_disk_usage_mutation()
        .with_resource_paths(vec![left.clone()]);
    fs::write(left, [2_u8; 23]).unwrap();
    guard.finish_resources_before_disk_rescan();
    physical_guard_assert_actual(&kura);
    assert!(!kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Relaxed));
}

#[test]
fn physical_guard_tree_deletion_and_move_reject_out_of_store_and_overlap() {
    for scenario in 0..4 {
        let (_directory, kura, left, _) = physical_guard_fixture();
        let owner = left.parent().unwrap();
        let outside = kura.store_root.parent().unwrap().join("outside");
        let guard = kura.begin_total_disk_usage_mutation();
        let guard = match scenario {
            0 => guard.removing_resource_tree(&outside),
            1 => guard.with_resource_tree_move(owner, &outside),
            2 => guard.with_resource_tree_move(owner, owner),
            3 => guard.with_resource_tree_move(owner, &owner.join("nested")),
            _ => unreachable!(),
        };
        guard.finish();
        physical_guard_assert_unavailable(&kura);
        assert_eq!(fs::metadata(&left).unwrap().len(), 9);
    }
}
