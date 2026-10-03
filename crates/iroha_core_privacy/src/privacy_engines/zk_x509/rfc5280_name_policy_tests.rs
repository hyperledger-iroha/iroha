// Original Name census, issuer/subject separation and coherent duplicate controls.

#[test]
fn name_policy_actual_complete_census_binds_original_instances_and_unique_keys() {
    use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
    for trace in [canonical_trace_v1(), spki_maximum_release_trace_v1()] {
        let material = build_zk_x509_rfc5280_stark_base_material_v1(&trace).unwrap();
        let semantic = build_zk_x509_rfc5280_semantic_witness_v1(&trace).unwrap();
        let expected = trace
            .semantic_provenance
            .iter()
            .flat_map(|d| &d.nodes)
            .filter(|n| n.role == ZkX509Rfc5280GrammarRoleV1::NameAttributeOid)
            .count();
        let mut seen = 0;
        let mut previous = 0;
        for family in [
            ZkX509Rfc5280StarkFamilyV1::SourceNode,
            ZkX509Rfc5280StarkFamilyV1::FixedByte,
        ] {
            let start = material.schedule.starts[family as usize];
            for ordinal in 0..material.family_rows[family as usize].len() {
                let row = material.base_row(start + ordinal).unwrap();
                let next = material.base_row(start + ordinal + 1).unwrap();
                let fixed = material.fixed_row(start + ordinal).unwrap();
                let result = name_policy::residues(&row, &next, &fixed);
                assert_eq!(
                    result,
                    [F::ZERO; name_policy::RESIDUES],
                    "{family:?}:{ordinal}"
                );
                assert_eq!(
                    name_policy::residues(
                        &row.map(E::from_base),
                        &next.map(E::from_base),
                        &fixed.map(E::from_base)
                    ),
                    result.map(E::from_base)
                );
                if family == ZkX509Rfc5280StarkFamilyV1::FixedByte {
                    let source = &semantic.fixed_bytes[ordinal];
                    assert_eq!(row[BASE_H], F(u64::from(source.source_node.role_instance)));
                    if row[BASE_IS_WRITE] == F::ONE {
                        let node_index = source.source_node.document as usize * 2_048
                            + source.source_node.node as usize;
                        let node = material
                            .base_row(
                                material.schedule.starts
                                    [ZkX509Rfc5280StarkFamilyV1::SourceNode as usize]
                                    + node_index,
                            )
                            .unwrap();
                        for lane in 0..4 {
                            assert_eq!(
                                profile_topology_source_factor_v1(&node, lane, challenges_v1()),
                                profile_topology_query_factor_v1(&row, lane, challenges_v1())
                            );
                            let mut changed = row;
                            changed[BASE_H] = changed[BASE_H].add(F(1024));
                            assert_ne!(
                                profile_topology_source_factor_v1(&node, lane, challenges_v1()),
                                profile_topology_query_factor_v1(&changed, lane, challenges_v1())
                            );
                        }
                        if row[BASE_ROLE] == F(9) {
                            let key = row[BASE_DOCUMENT].0 * 8
                                + row[BASE_H].0 / 1024 * 4
                                + row[BASE_ENDPOINT_ROLE].0
                                + 1;
                            assert!(key > previous);
                            previous = key;
                            seen += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(seen, expected);
        assert!(seen > 0);
    }
    assert_eq!(name_policy::NODE_PREFIX_END, 134);
    assert_eq!(ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1, 146);
    assert_eq!(ZK_X509_RFC5280_STARK_COMPRESSED_RELATIONS_V1, 39);
}

fn name_policy_oid_row(
    document: u64,
    name: u64,
    variant: u64,
    previous: &mut u64,
) -> ZkX509Rfc5280StarkBaseRowV1 {
    let mut row = active_zero_row_v1();
    row[BASE_DOCUMENT] = F(document);
    row[BASE_ROLE] = F(9);
    row[BASE_H] = F(name * 1024);
    row[BASE_ENDPOINT_ROLE] = F(variant);
    row[BASE_IS_WRITE] = F::ONE;
    row[BASE_STRICT] = F::ONE;
    name_policy::populate_fixed_byte(&mut row, previous).unwrap();
    row
}

#[test]
fn name_policy_duplicate_and_decreasing_oid_keys_cannot_reset_carried_state() {
    let mut fixed = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
    fixed[ZkX509Rfc5280StarkFamilyV1::FixedByte as usize] = F::ONE;
    fixed[FIX_ACTIVATION_CONTINUE] = F::ONE;
    let mut previous = 0;
    let mut rows = Vec::new();
    for document in 0..4 {
        for name in 0..2 {
            for variant in 0..4 {
                rows.push(name_policy_oid_row(document, name, variant, &mut previous));
            }
        }
    }
    assert_eq!(previous, 32);
    for i in 0..rows.len() {
        let next = rows
            .get(i + 1)
            .copied()
            .unwrap_or([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
        fixed[FIX_EXPECTED] = F(u64::from(i == 0));
        assert_eq!(
            name_policy::residues(&rows[i], &next, &fixed),
            [F::ZERO; name_policy::RESIDUES]
        );
        let mut duplicate = rows[i];
        duplicate[BASE_STATE_BEFORE] = duplicate[BASE_STATE_AFTER];
        assert!(name_policy::populate_fixed_byte(&mut duplicate, &mut previous).is_err());
        // Rebuilding all local helpers around a duplicated OID cannot make a
        // positive bounded increment from its original predecessor.
        let mut state = rows[i][BASE_STATE_AFTER].0;
        assert!(name_policy::populate_fixed_byte(&mut duplicate, &mut state).is_err());
        assert!(
            name_policy::residues(&duplicate, &next, &fixed)
                .iter()
                .any(|r| *r != F::ZERO)
        );
        if i + 1 < rows.len() {
            let mut reset = next;
            reset[BASE_STATE_BEFORE] = F::ZERO;
            assert!(
                name_policy::residues(&rows[i], &reset, &fixed)
                    .iter()
                    .any(|r| *r != F::ZERO)
            );
        }
    }
    let mut skipped = rows[0];
    skipped[BASE_STATE_BEFORE] = F(32);
    fixed[FIX_EXPECTED] = F::ONE;
    assert!(
        name_policy::residues(&skipped, &rows[1], &fixed)
            .iter()
            .any(|r| *r != F::ZERO)
    );
}

#[test]
fn name_policy_nonempty_names_rdns_and_bounded_original_ordinals_reject_aliases() {
    let mut fixed = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
    fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize] = F::ONE;
    let next = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
    for role in [
        ZkX509Rfc5280GrammarRoleV1::NameRdn,
        ZkX509Rfc5280GrammarRoleV1::NameAttribute,
        ZkX509Rfc5280GrammarRoleV1::CertificateIssuer,
        ZkX509Rfc5280GrammarRoleV1::CertificateSubject,
        ZkX509Rfc5280GrammarRoleV1::CrlIssuer,
    ] {
        let mut row = active_zero_row_v1();
        row[BASE_ROLE] = F(role as u64);
        row[BASE_D] = F(2);
        name_policy::populate_source_node(&mut row).unwrap();
        assert_eq!(
            name_policy::residues(&row, &next, &fixed),
            [F::ZERO; name_policy::RESIDUES]
        );
        if role != ZkX509Rfc5280GrammarRoleV1::NameAttribute {
            let mut empty = row;
            empty[BASE_D] = F::ZERO;
            assert!(name_policy::populate_source_node(&mut empty).is_err());
            assert!(
                name_policy::residues(&empty, &next, &fixed)
                    .iter()
                    .any(|r| *r != F::ZERO)
            );
        }
        if matches!(
            role,
            ZkX509Rfc5280GrammarRoleV1::NameRdn | ZkX509Rfc5280GrammarRoleV1::NameAttribute
        ) {
            for ordinal in [4, 16, 64, 255, 65535] {
                let mut alias = row;
                alias[BASE_CHILD] = F(ordinal);
                for bit in 0..16 {
                    alias[GRAMMAR_CHILD_ORDINAL_BITS + bit] = F((ordinal >> bit) & 1);
                }
                assert!(name_policy::populate_source_node(&mut alias).is_err());
                assert!(
                    name_policy::residues(&alias, &next, &fixed)
                        .iter()
                        .any(|r| *r != F::ZERO)
                );
            }
        }
        for column in CALENDAR_COLUMNS + 10..name_policy::NODE_PREFIX_END {
            let mut changed = row;
            changed[column] = changed[column].add(F::ONE);
            // A matching-role zero-test inverse is free when its delta is
            // zero. Every classifier and every nonempty inverse is bound.
            if (column - CALENDAR_COLUMNS - 10) % 2 == 1 && changed[column - 1] == F::ONE {
                continue;
            }
            assert!(
                name_policy::residues(&changed, &next, &fixed)
                    .iter()
                    .any(|r| *r != F::ZERO),
                "{role:?}:{column}"
            );
        }
    }
}

#[test]
fn name_policy_every_opened_input_has_affine_degree_at_most_four() {
    use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
    for seed in [1, 7, 19] {
        let samples = (0..9)
            .map(|point| {
                let row = core::array::from_fn(|i| affine_value_v1(seed, 1, i, point));
                let next = core::array::from_fn(|i| affine_value_v1(seed, 3, i, point));
                let fixed = core::array::from_fn(|i| affine_value_v1(seed, 5, i, point));
                let result = name_policy::residues(&row, &next, &fixed);
                assert_eq!(
                    name_policy::residues(
                        &row.map(E::from_base),
                        &next.map(E::from_base),
                        &fixed.map(E::from_base)
                    ),
                    result.map(E::from_base)
                );
                result
            })
            .collect::<Vec<_>>();
        for column in 0..name_policy::RESIDUES {
            assert!(
                finite_difference_degree_v1(samples.iter().map(|r| r[column]).collect()) <= 4,
                "column={column}"
            );
        }
    }
}
