// Exact physical accounting at metadata suffix deletion and recovery boundaries.


fn metadata_physical_register(kura: &Kura) -> IndexResourceCounts {
    let actual = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    kura.resource_inventory
        .initialize(
            kura.resource_inventory.reconciliation_generation().unwrap(),
            &PHYSICAL_RESOURCE_FAMILIES
                .iter()
                .map(|family| (*family, actual[*family as usize]))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    metadata_physical_assert_actual(kura)
}

fn metadata_physical_assert_actual(kura: &Kura) -> IndexResourceCounts {
    let actual = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap(),
            actual[family as usize],
            "{family:?}"
        );
    }
    actual
}
