// Included by fixed_algebraic_sha::tests; exercises both production fixed owners.

#[test]
fn canonical_frame_atoms_match_exact_shape_counts_and_native_input_rows() {
    const FRAME_WIDTH: usize = 16;
    let preceding_columns: [usize; 4] = [55_703, 64_091, 34_614, 45_994];
    for disclosed_attributes in 0..=4 {
        let shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        };
        let provider = ZkX509ShaBatchFixedProviderV1::new_v1(shape).unwrap();
        let calls = ZkX509ShaCallScheduleV1::new(shape).unwrap();
        // The public constructor must validate the exact inventory, not merely
        // return raw children as the diagnostic accounting test does.
        let schedule = schedule(disclosed_attributes);
        for (segment, child) in schedule.children_v1().iter().enumerate() {
            let mut before_frame = 0_usize;
            let mut frame_rows = BTreeMap::<usize, [F; FRAME_WIDTH]>::new();
            for atom in child.atoms_v1().iter().copied() {
                let column = usize::from(match atom {
                    ZkX509FixedAlgebraicAtomV1::Affine { column, .. }
                    | ZkX509FixedAlgebraicAtomV1::Repeated { column, .. }
                    | ZkX509FixedAlgebraicAtomV1::Sparse { column, .. } => column,
                });
                if column < ZK_X509_SHA_FIXED_FRAME_MASK_V1 {
                    before_frame += 1;
                    continue;
                }
                if column >= projection_bus::FIX_WORD_V1 {
                    continue;
                }
                let local = column - ZK_X509_SHA_FIXED_FRAME_MASK_V1;
                assert!(local < FRAME_WIDTH);
                let mut add = |row: u64, value: F| {
                    let row = usize::try_from(row).unwrap();
                    assert!(row < ZK_X509_SHA_SEGMENT_ROWS_V1);
                    let cells = frame_rows.entry(row).or_insert([F::ZERO; FRAME_WIDTH]);
                    cells[local] = cells[local].add(value);
                };
                match atom {
                    ZkX509FixedAlgebraicAtomV1::Affine {
                        start,
                        end,
                        start_value,
                        step,
                        ..
                    } => {
                        for row in start..end {
                            add(row, start_value.add(step.mul(F(row - start))));
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
                        for occurrence in 0..count {
                            add(
                                first + occurrence * stride,
                                start_value.add(step.mul(F(occurrence))),
                            );
                        }
                    }
                    ZkX509FixedAlgebraicAtomV1::Sparse { row, value, .. } => add(row, value),
                }
            }
            assert_eq!(before_frame, preceding_columns[segment]);
            assert_eq!(
                child.atoms_v1().len(),
                SHA_FIXED_ALGEBRAIC_CHILD_ATOM_COUNTS_V1[disclosed_attributes][segment]
            );
            assert!(child.atoms_v1().len() <= ZK_X509_SHA_FIXED_ALGEBRAIC_MAX_ATOMS_V1);
            for call in calls
                .calls()
                .iter()
                .filter(|call| call.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1 == segment)
            {
                let start = call.first_logical_row % ZK_X509_SHA_SEGMENT_ROWS_V1;
                for word in 0..call.maximum_blocks * 16 {
                    let row = start
                        + INITIAL_LOCAL_ROWS_PER_CALL_V1
                        + (word / 16) * SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1
                        + word % 16;
                    let expected = provider.fixed_row_v1(segment, row).unwrap();
                    // Authenticate the row arithmetic against the independently
                    // generated production provider before comparing frame cells.
                    assert_eq!(expected[SHA_WORD_CAPACITY_INPUT_WORD_V1], F::ONE);
                    assert_eq!(
                        expected[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1],
                        F(u64::try_from(word).expect("bounded input word"))
                    );
                    let actual = frame_rows.remove(&row).unwrap_or([F::ZERO; FRAME_WIDTH]);
                    assert_eq!(
                        actual.as_slice(),
                        &expected[ZK_X509_SHA_FIXED_FRAME_MASK_V1..projection_bus::FIX_WORD_V1],
                        "shape {disclosed_attributes}, segment {segment}, call {}, word {word}",
                        call.call
                    );
                }
            }
            assert!(frame_rows.is_empty(), "frame atoms outside input rows");
        }
    }
}
