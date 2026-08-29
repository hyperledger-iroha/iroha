// Bounded SoraFS alias projection regressions.
#[test]
fn alias_page_projects_only_the_bounded_selection() {
    let state = make_state();
    let issuer = test_account();
    let valid_digest = ManifestDigest::new([0x31; 32]);
    let missing_digest = ManifestDigest::new([0x32; 32]);
    let successor_digest = ManifestDigest::new([0x36; 32]);
    let mut block = state.block(default_block_header());
    let mut tx = block.transaction();
    tx.world_mut_for_testing()
        .pin_manifests_mut_for_testing()
        .insert(
            valid_digest,
            PinManifestRecord::new(
                valid_digest,
                canonical_fixture_manifest_root_cid(),
                default_chunker_handle(),
                [0x33; 32],
                [0x34; 32],
                1,
                RegistryPinPolicy::default(),
                issuer.clone(),
                1,
                None,
                None,
                Metadata::default(),
            ),
        );
    let mut successor = PinManifestRecord::new(
        successor_digest,
        canonical_fixture_manifest_root_cid(),
        default_chunker_handle(),
        [0x37; 32],
        [0x38; 32],
        1,
        RegistryPinPolicy::default(),
        issuer.clone(),
        2,
        None,
        Some(valid_digest),
        Metadata::default(),
    );
    successor.approve(3, None);
    successor.retire(4, None);
    tx.world_mut_for_testing()
        .pin_manifests_mut_for_testing()
        .insert(successor_digest, successor);
    for (name, digest) in [("a-valid", valid_digest), ("z-missing", missing_digest)] {
        let binding = ManifestAliasBinding {
            namespace: "sora".to_owned(),
            name: name.to_owned(),
            proof: vec![0x35],
        };
        let record = ManifestAliasRecord::new(binding, digest, issuer.clone(), 1, 100);
        tx.world_mut_for_testing()
            .manifest_aliases_mut_for_testing()
            .insert(record.alias_id(), record);
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit bounded alias page fixture");
    let view = state.view();
    let page = collect_alias_page(view.world(), 0, 1, None, None)
        .expect("unselected broken alias must not be projected");
    assert_eq!(page.total_count, 2);
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].alias.alias_label(), "sora/a-valid");
    let differently_cased = collect_alias_page(view.world(), 0, 1, Some("SORA"), None)
        .expect("case-sensitive namespace filter");
    assert_eq!(differently_cased.total_count, 0);
    assert!(differently_cased.entries.is_empty());
    assert_eq!(
        page.entries[0].lineage.head_hex,
        hex::encode(successor_digest.as_bytes())
    );
    assert_eq!(page.entries[0].lineage.depth_to_head, 1);
    let approved_successor = page.entries[0]
        .lineage
        .approved_successor
        .as_ref()
        .expect("retired successor retains approval history");
    assert_eq!(approved_successor.approved_epoch, Some(3));
    assert!(matches!(
        approved_successor.status,
        crate::sorafs::registry::ManifestStatusProjection::Retired { epoch: 4 }
    ));
    assert!(matches!(
        collect_alias_page(view.world(), 1, 1, None, None),
        Err(PinRegistryError::MissingAliasManifest { .. })
    ));
    let invalid_filter = collect_alias_page(view.world(), 0, 1, None, Some("not-a-digest"))
        .expect("invalid digest filter has no matches");
    assert_eq!(invalid_filter.total_count, 0);
    assert!(invalid_filter.entries.is_empty());
}
#[test]
fn alias_offset_clamp_does_not_truncate_large_totals() {
    if usize::BITS > u32::BITS {
        let total = (u32::MAX as usize).saturating_add(2);
        assert_eq!(normalize_offset(Some(u32::MAX), total), u32::MAX as usize);
    }
}
