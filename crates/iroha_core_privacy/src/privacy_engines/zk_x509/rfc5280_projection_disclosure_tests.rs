// Native controls for selected original Subject/OID/value provenance.

#[test]
fn projection_disclosure_fixed_schedule_preserves_oid_indices_and_fixed_column_capacity() {
    for mask in 0..16_u8 {
        let indices = (0..4)
            .filter(|index| mask & (1 << index) != 0)
            .collect::<Vec<_>>();
        let mut shape = ZkX509Rfc5280StarkShapeV1::default();
        shape.disclosed_attribute_count = indices.len() as u8;
        shape.disclosed_attribute_indices[..indices.len()].copy_from_slice(&indices);
        let schedule = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(shape).unwrap();
        let mut counts = [0; 4];
        for (ordinal, entry) in schedule.output_topology.iter().enumerate() {
            let selected = projection_disclosure::slot(shape, entry.channel);
            for family in [
                ZkX509Rfc5280StarkFamilyV1::OutputProducer,
                ZkX509Rfc5280StarkFamilyV1::OutputConsumer,
            ] {
                let fixed = schedule
                    .fixed_row(schedule.starts[family as usize] + ordinal)
                    .unwrap();
                assert_eq!(
                    fixed[projection_disclosure::FIX_DISCLOSURE],
                    F(u64::from(
                        selected.is_some() && family == ZkX509Rfc5280StarkFamilyV1::OutputProducer
                    ))
                );
                if family == ZkX509Rfc5280StarkFamilyV1::OutputProducer
                    && let Some((slot, length)) = selected
                {
                    counts[slot] += 1;
                    if length && (1..=3).contains(&entry.offset) {
                        let expected = [[85, 4, 6], [85, 4, 10], [85, 4, 11], [85, 4, 3]]
                            [indices[slot] as usize][entry.offset as usize - 1];
                        assert_eq!(fixed[projection_disclosure::FIX_OID_VALUE], F(expected));
                    }
                }
            }
        }
        for (slot, count) in counts.into_iter().enumerate() {
            assert_eq!(count, if slot < indices.len() { 264 } else { 0 });
        }
    }
    assert_eq!(projection_disclosure::FIX_END, 146);
    assert_eq!(projection_disclosure::FIX_END + 4 * 27, 254);
    assert!(u8::try_from(projection_disclosure::FIX_END + 4 * 27).is_ok());
}

#[test]
fn projection_disclosure_actual_material_authenticates_subject_oid_value_and_big_endian_length() {
    use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
    for trace in [canonical_trace_v1(), spki_maximum_release_trace_v1()] {
        let sources = projection_disclosure::sources(&trace).unwrap();
        let material = build_zk_x509_rfc5280_stark_base_material_v1(&trace).unwrap();
        let io = rfc5280_io_witnesses_v1(&trace, 0).unwrap();
        let mut queries = [[0; 2]; 4];
        for (ordinal, entry) in material.schedule.output_topology.iter().enumerate() {
            let Some((slot, _)) =
                projection_disclosure::slot(material.schedule.shape, entry.channel)
            else {
                continue;
            };
            let index = material.schedule.starts
                [ZkX509Rfc5280StarkFamilyV1::OutputProducer as usize]
                + ordinal;
            let row = material.base_row(index).unwrap();
            let next = material.base_row(index + 1).unwrap();
            let fixed = material.fixed_row(index).unwrap();
            assert_eq!(
                row[BASE_VALUE],
                F(u64::from(
                    io[entry.channel as usize].producer_value[entry.offset as usize]
                ))
            );
            assert_eq!(
                projection_disclosure::residues(&row, &next, &fixed),
                [F::ZERO; projection_disclosure::RESIDUES]
            );
            assert_eq!(
                projection_disclosure::residues(
                    &row.map(E::from_base),
                    &next.map(E::from_base),
                    &fixed.map(E::from_base)
                ),
                [E::ZERO; projection_disclosure::RESIDUES]
            );
            queries[slot][0] += row[BASE_SERIAL_BYTE_QUERY_ACTIVE].0;
            queries[slot][1] += output_source_node_query_gate_v1(&row, &fixed).0;
            if row[BASE_SERIAL_BYTE_QUERY_ACTIVE] == F::ONE {
                assert_eq!(
                    trace.documents[0].bytes[row[BASE_ADDRESS].0 as usize]
                        .value
                        .value,
                    row[BASE_SERIAL_BYTE_QUERY_VALUE]
                );
            }
        }
        for (slot, source) in sources
            .iter()
            .enumerate()
            .filter_map(|(slot, source)| source.as_ref().map(|source| (slot, source)))
        {
            assert_eq!(
                queries[slot],
                [source.bytes.len() as u64 + 3, source.bytes.len() as u64 + 5]
            );
            assert_eq!(
                u64::from_be_bytes(
                    io[5 + 2 * slot]
                        .producer_value
                        .as_slice()
                        .try_into()
                        .unwrap()
                ),
                source.bytes.len() as u64
            );
            assert_eq!(
                projection_disclosure::byte_multiplicity(
                    &sources,
                    0,
                    usize::from(source.value.content_start)
                ),
                1
            );
            assert_eq!(
                projection_disclosure::node_multiplicity(
                    &sources,
                    0,
                    usize::from(source.value.node)
                ),
                source.bytes.len() as u16 + 1
            );
        }
    }
}

#[test]
fn projection_disclosure_lengths_one_255_256_padding_and_phase_metadata_are_constrained() {
    let trace = canonical_trace_v1();
    let source = projection_disclosure::sources(&trace).unwrap()[0].unwrap();
    let shape = ZkX509Rfc5280StarkShapeV1::from_statement(&trace.statement).unwrap();
    let schedule = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(shape).unwrap();
    for size in [1_usize, 255, 256] {
        let bytes = vec![b'A'; size];
        let mut subject = *source.subject;
        let mut oid = *source.oid;
        let mut value = *source.value;
        subject.content_start = 10;
        subject.content_end = 1000;
        oid.start = 20;
        oid.content_start = 22;
        oid.content_end = 25;
        value.start = 25;
        value.content_start = 29;
        value.content_end = 29 + size as u16;
        let source = projection_disclosure::Source {
            subject: &subject,
            oid: &oid,
            value: &value,
            bytes: &bytes,
        };
        let entries = schedule
            .output_topology
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                matches!(
                    projection_disclosure::slot(shape, entry.channel),
                    Some((0, _))
                )
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        let mut fixed_rows = Vec::new();
        for (ordinal, entry) in entries {
            let (_, length) = projection_disclosure::slot(shape, entry.channel).unwrap();
            let offset = entry.offset as usize;
            let value = if length {
                (size as u64).to_be_bytes()[offset]
            } else {
                bytes.get(offset).copied().unwrap_or(0)
            };
            let mut row = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
            row[BASE_VALUE] = F(u64::from(value));
            write_u8_bits_v1(&mut row, BASE_BYTE_BITS, value);
            projection_disclosure::populate_row(&mut row, &source, length, offset).unwrap();
            rows.push(row);
            fixed_rows.push(
                schedule
                    .fixed_row(
                        schedule.starts[ZkX509Rfc5280StarkFamilyV1::OutputProducer as usize]
                            + ordinal,
                    )
                    .unwrap(),
            );
        }
        assert_eq!(rows.len(), 264);
        for i in 0..264 {
            let next = rows
                .get(i + 1)
                .copied()
                .unwrap_or([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
            assert_eq!(
                projection_disclosure::residues(&rows[i], &next, &fixed_rows[i]),
                [F::ZERO; projection_disclosure::RESIDUES],
                "size={size}, row={i}"
            );
        }
        for (i, column) in [
            (0, BASE_G),
            (0, BASE_H),
            (1, BASE_START),
            (1, BASE_ADDRESS),
            (2, BASE_NODE),
            (3, BASE_CONTENT_END),
            (4, BASE_CONTENT_END),
            (4, BASE_TAG_NUMBER),
            (6, BASE_VALUE),
            (7, BASE_VALUE),
            (8, BASE_PARENT),
            (8, BASE_DEPTH),
            (8, BASE_CHILD),
            (263, BASE_C),
        ] {
            let mut row = rows[i];
            row[column] = row[column].add(F::ONE);
            let next = rows
                .get(i + 1)
                .copied()
                .unwrap_or([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
            assert!(
                projection_disclosure::residues(&row, &next, &fixed_rows[i])
                    .iter()
                    .any(|v| *v != F::ZERO),
                "size={size}, row={i}, col={column}"
            );
        }
    }
}

#[test]
fn projection_disclosure_constructor_rejects_wrong_subject_oid_value_and_alias_source() {
    let trace = canonical_trace_v1();
    let source = projection_disclosure::sources(&trace).unwrap()[0].unwrap();
    let subject_index = source.subject.node as usize;
    let oid_index = source.oid.node as usize;
    let value_index = source.value.node as usize;
    for mode in 0..7 {
        let mut changed = trace.clone();
        match mode {
            0 => changed.semantic_provenance[0].nodes[subject_index].role_instance = 0,
            1 => changed.semantic_provenance[0].nodes[oid_index].document = 1,
            2 => changed.semantic_provenance[0].nodes[oid_index].start = 0,
            3 => changed.semantic_provenance[0].nodes[value_index].parent_node ^= 1,
            4 => changed.semantic_provenance[0].nodes[value_index].role_instance ^= 1,
            5 => changed.semantic_provenance[0].nodes[value_index].tag_number = 2,
            _ => {
                changed.certificates[0].subject.attributes
                    [usize::from(trace.statement.disclosed_attribute_indices[0])]
                .as_mut()
                .unwrap()[0] ^= 1
            }
        }
        assert!(
            projection_disclosure::sources(&changed).is_err(),
            "mode={mode}"
        );
    }
}

#[test]
fn projection_disclosure_affine_degree_is_at_most_four_for_every_opened_input() {
    use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
    for seed in [1, 7, 19] {
        let samples = (0..9)
            .map(|point| {
                let row = core::array::from_fn(|i| affine_value_v1(seed, 1, i, point));
                let next = core::array::from_fn(|i| affine_value_v1(seed, 3, i, point));
                let fixed = core::array::from_fn(|i| affine_value_v1(seed, 5, i, point));
                let result = projection_disclosure::residues(&row, &next, &fixed);
                assert_eq!(
                    projection_disclosure::residues(
                        &row.map(E::from_base),
                        &next.map(E::from_base),
                        &fixed.map(E::from_base)
                    ),
                    result.map(E::from_base)
                );
                result
            })
            .collect::<Vec<_>>();
        for column in 0..projection_disclosure::RESIDUES {
            assert!(
                finite_difference_degree_v1(samples.iter().map(|r| r[column]).collect()) <= 4,
                "seed={seed}, column={column}"
            );
        }
    }
    assert_eq!(ZK_X509_RFC5280_STARK_COMPRESSED_RELATIONS_V1, 39);
}
