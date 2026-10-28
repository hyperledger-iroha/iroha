// Included by the structural SHA compiler tests; independent native selector owner.

#[test]
fn projection_selector_atoms_match_all_five_actual_schedules_and_event_counts() {
    let preceding = [
        [56_572, 64_656, 34_614, 46_543],
        [56_572, 64_790, 34_614, 46_543],
        [56_572, 64_924, 34_614, 46_543],
        [56_572, 64_924, 34_748, 46_543],
        [56_572, 64_924, 34_748, 46_677],
    ];
    for disclosed_attributes in 0..=4 {
        let shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        };
        let native = ZkX509ShaBatchFixedProviderV1::new_v1(shape).unwrap();
        let calls = ZkX509ShaCallScheduleV1::new(shape).unwrap();
        let compiled = schedule(disclosed_attributes);
        for (segment, child) in compiled.children_v1().iter().enumerate() {
            let mut rows = BTreeMap::<usize, [F; 3]>::new();
            let mut old_atoms = 0;
            let mut new_atoms = 0;
            for atom in child.atoms_v1().iter().copied() {
                let column = match atom {
                    ZkX509FixedAlgebraicAtomV1::Affine { column, .. }
                    | ZkX509FixedAlgebraicAtomV1::Repeated { column, .. }
                    | ZkX509FixedAlgebraicAtomV1::Sparse { column, .. } => usize::from(column),
                };
                if column < projection_bus::FIX_WORD_V1 {
                    old_atoms += 1;
                    continue;
                }
                assert!(column < ZK_X509_SHA_BATCH_FIXED_WIDTH_V1);
                new_atoms += 1;
                let local = column - projection_bus::FIX_WORD_V1;
                let mut add = |row, value| {
                    let row = usize::try_from(row).unwrap();
                    assert!(row < ZK_X509_SHA_SEGMENT_ROWS_V1);
                    let entry = rows.entry(row).or_insert([F::ZERO; 3]);
                    assert_eq!(entry[local], F::ZERO, "duplicate event atom");
                    assert_eq!(value, F::ONE);
                    entry[local] = value;
                };
                match atom {
                    ZkX509FixedAlgebraicAtomV1::Affine {
                        start,
                        end,
                        start_value,
                        step,
                        ..
                    } => {
                        assert_eq!(step, F::ZERO);
                        for row in start..end {
                            add(row, start_value);
                        }
                    }
                    ZkX509FixedAlgebraicAtomV1::Repeated {
                        first,
                        count,
                        stride,
                        start_value,
                        step,
                        ..
                    } => {
                        assert_eq!(column, projection_bus::FIX_WORD_V1);
                        assert_eq!(count, 32);
                        assert_eq!(stride, SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1 as u64);
                        assert_eq!(step, F::ZERO);
                        for i in 0..count {
                            add(first + i * stride, start_value);
                        }
                    }
                    ZkX509FixedAlgebraicAtomV1::Sparse { row, value, .. } => add(row, value),
                }
            }
            let mut active = 0;
            for manifest in calls
                .calls()
                .iter()
                .filter(|call| call.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1 == segment)
            {
                let start = manifest.first_logical_row % ZK_X509_SHA_SEGMENT_ROWS_V1;
                active += usize::from(projection_bus::is_active_v1(*manifest));
                let mut check = |row, expected_kind: usize| {
                    let native_row = native.fixed_row_v1(segment, row).unwrap();
                    let expected: [F; 3] = native_row[projection_bus::FIX_WORD_V1..]
                        .try_into()
                        .unwrap();
                    let actual = rows.remove(&row).unwrap_or([F::ZERO; 3]);
                    assert_eq!(
                        actual, expected,
                        "shape{disclosed_attributes} segment{segment} call{} row{row}",
                        manifest.call
                    );
                    assert!(actual.iter().filter(|v| **v == F::ONE).count() <= 1);
                    if projection_bus::is_active_v1(*manifest) && expected_kind < 3 {
                        assert_eq!(actual[expected_kind], F::ONE);
                    } else {
                        assert_eq!(actual, [F::ZERO; 3]);
                    }
                };
                for word in 0..manifest.maximum_blocks * 16 {
                    check(
                        start
                            + 8
                            + (word / 16) * SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1
                            + word % 16,
                        if word < 512 { 0 } else { 3 },
                    );
                }
                for word in 0..8 {
                    check(start + manifest.maximum_local_rows - 8 + word, 2);
                }
                check(start + manifest.maximum_logical_rows() - 1, 1);
                check(start, 3);
            }
            assert_eq!(old_atoms, preceding[disclosed_attributes][segment]);
            assert_eq!(new_atoms, 18 * active);
            assert_eq!(
                child.atoms_v1().len(),
                SHA_FIXED_ALGEBRAIC_CHILD_ATOM_COUNTS_V1[disclosed_attributes][segment]
            );
            assert!(rows.is_empty(), "unexpected selector rows");
            assert_eq!(
                &native
                    .fixed_row_v1(segment, ZK_X509_SHA_SEGMENT_ROWS_V1 - 1)
                    .unwrap()[projection_bus::FIX_WORD_V1..],
                &[F::ZERO; 3]
            );
        }
    }
}

#[test]
fn structural_constant_singletons_use_sparse_atoms_and_preserve_ranges() {
    let domain = ZkX509FixedAlgebraicDomainV1::new_v1(
        19,
        ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
        F(GOLDILOCKS_GENERATOR_V1),
    )
    .unwrap();
    let column = projection_bus::FIX_LENGTH_V1;
    let last = ZK_X509_SHA_SEGMENT_ROWS_V1 - 1;
    let mut output = StructuralBuilderV1::new_v1(domain, 0).unwrap();
    output.push_constant_v1(0, column, 0, 1, F::ONE).unwrap();
    output
        .push_constant_v1(0, column, last, last + 1, F(7))
        .unwrap();
    output.push_constant_v1(0, column, 8, 16, F(3)).unwrap();
    assert_eq!(output.atom_count, 3);
    let schedule = output.finish_v1().unwrap();
    assert_eq!(schedule.atoms_v1().len(), 3);
    assert_eq!(
        schedule.atoms_v1(),
        &[
            ZkX509FixedAlgebraicAtomV1::sparse_v1(column as u16, 0, F::ONE).unwrap(),
            ZkX509FixedAlgebraicAtomV1::affine_v1(column as u16, 8, 16, F(3), F::ZERO).unwrap(),
            ZkX509FixedAlgebraicAtomV1::sparse_v1(column as u16, last as u64, F(7)).unwrap(),
        ],
    );
    for (row, expected) in [
        (0, F::ONE),
        (1, F::ZERO),
        (7, F::ZERO),
        (8, F(3)),
        (15, F(3)),
        (16, F::ZERO),
        (last - 1, F::ZERO),
        (last, F(7)),
    ] {
        let mut actual = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        schedule.native_row_v1(row as u64, &mut actual).unwrap();
        assert_eq!(actual[column], expected);
        assert!(
            actual
                .iter()
                .enumerate()
                .all(|(index, value)| { index == column || *value == F::ZERO })
        );
    }
    // The wrapper chooses the canonical atom; the kernel still refuses a
    // degenerate affine representation of the same singleton.
    assert_eq!(
        ZkX509FixedAlgebraicAtomV1::affine_v1(column as u16, 0, 1, F::ONE, F::ZERO),
        Err(ZkX509FixedAlgebraicErrorV1::InvalidAtom)
    );
}

#[test]
fn structural_constant_singletons_preserve_geometry_and_atom_limits() {
    let domain = ZkX509FixedAlgebraicDomainV1::new_v1(
        19,
        ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
        F(GOLDILOCKS_GENERATOR_V1),
    )
    .unwrap();
    let column = projection_bus::FIX_LENGTH_V1;
    let mut output = StructuralBuilderV1::new_v1(domain, 0).unwrap();
    for (segment, column, start, end) in [
        (0, column, 0, 0),
        (0, column, 1, 0),
        (
            0,
            column,
            ZK_X509_SHA_SEGMENT_ROWS_V1,
            ZK_X509_SHA_SEGMENT_ROWS_V1 + 1,
        ),
        (1, column, 0, 1),
        (0, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, 0, 1),
    ] {
        assert_eq!(
            output.push_constant_v1(segment, column, start, end, F::ONE),
            Err(ZkX509ShaFixedAlgebraicErrorV1::Topology)
        );
    }
    assert_eq!(output.atom_count, 0);
    output.atom_count = ZK_X509_SHA_FIXED_ALGEBRAIC_MAX_ATOMS_V1;
    assert_eq!(
        output.push_constant_v1(0, column, 0, 1, F::ONE),
        Err(ZkX509ShaFixedAlgebraicErrorV1::Resource)
    );
}
