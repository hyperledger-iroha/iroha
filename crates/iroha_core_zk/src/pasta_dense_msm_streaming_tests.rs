/// Independent old-vector oracle, exact key/proof equivalence, and bounded trace checks.
mod streaming_tests {
    use super::*;
    use halo2_base::{ContextCell, EXTERNAL_CELL_TYPE_ID};
    use std::time::Instant;

    // These two reference functions are frozen from the maintained preimage. Keep their
    // witness and copy ordering independent from the new emitter so comparison is meaningful.
    fn reference_build_job_lane_rows<C>(
        job: &DenseMsmJob<C>,
        job_index: usize,
        lane: usize,
        source_start: usize,
        source_end: usize,
        offset: C::Curve,
    ) -> Result<(Vec<RawRow<Base<C>>>, C::Curve), Error>
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField,
    {
        debug_assert!(source_start < source_end && source_end <= job.sources.len());
        let (offset_x, offset_y) =
            affine_coordinates::<C>(&offset).map_err(|_| Error::Synthesis)?;
        let mut rows = Vec::new();
        let mut state = MachineWitness::<Base<C>>::default();
        let mut start = raw_row(
            state,
            Base::<C>::ONE,
            None,
            Some(BusBinding::Start { job: job_index }),
        );
        start.values[START] = Base::<C>::ONE;
        start.values[ADD_INVERSE] =
            Option::<Base<C>>::from(offset_y.invert()).ok_or(Error::Synthesis)?;
        rows.push(start);
        state.acc_x = offset_x;
        state.acc_y = offset_y;
        state.offset_x = offset_x;
        state.offset_y = offset_y;
        let source_count =
            u64::try_from(source_end - source_start).map_err(|_| Error::Synthesis)?;
        rows.push(raw_row(
            state,
            Base::<C>::from(source_count),
            Some(MODE_COUNT),
            Some(BusBinding::SourceCount {
                job: job_index,
                lane,
            }),
        ));
        state.remaining_sources = Base::<C>::from(source_count);
        state.remaining_segments = Base::<C>::from(SEGMENTS_PER_SCALAR as u64);
        let mut accumulator = offset;
        for (source_offset, source) in job.sources[source_start..source_end].iter().enumerate() {
            let source_index = source_start + source_offset;
            let (r_x, r_y) = source.r.into_coordinates();
            rows.push(raw_row(
                state,
                r_x,
                Some(MODE_LOAD_X),
                Some(BusBinding::SourceX {
                    job: job_index,
                    source: source_index,
                }),
            ));
            state.source_x = r_x;
            state.source_y = Base::<C>::ZERO;
            state.remaining_segments = Base::<C>::from(SEGMENTS_PER_SCALAR as u64);
            let mut load_y = raw_row(
                state,
                r_y,
                Some(MODE_LOAD_Y),
                Some(BusBinding::SourceY {
                    job: job_index,
                    source: source_index,
                }),
            );
            load_y.values[ADD_INVERSE] =
                Option::<Base<C>>::from(r_y.invert()).ok_or(Error::Synthesis)?;
            rows.push(load_y);
            state.source_y = r_y;
            let mut running_source = source.r.to_curve();
            for segment in 0..SEGMENTS_PER_SCALAR {
                let (bit_start, width) = segment_spec(segment);
                let part_1 = segment_integer(source, 0, segment);
                let part_2 = segment_integer(source, 1, segment);
                state.part_1 = Base::<C>::from(part_1);
                state.part_2 = Base::<C>::from(part_2);
                state.remaining_bits = Base::<C>::from(width as u64);
                for local_bit in 0..width {
                    let bit = bit_start + local_bit;
                    let bit_1 = source.bits[0][bit];
                    let bit_2 = source.bits[1][bit];
                    let segment_start = local_bit == 0;
                    let segment_binding = segment_start.then_some(BusBinding::Segment {
                        job: job_index,
                        source: source_index,
                        segment,
                    });
                    let mut operation = raw_row(
                        state,
                        if segment_start {
                            *source.paired_segments[segment].value()
                        } else {
                            Base::<C>::ZERO
                        },
                        Some(MODE_OP),
                        segment_binding,
                    );
                    operation.values[SEGMENT_START] = Base::<C>::from(segment_start as u64);
                    operation.values[BIT_1] = Base::<C>::from(bit_1 as u64);
                    operation.values[BIT_2] = Base::<C>::from(bit_2 as u64);
                    let (last_bit, last_bit_inverse) =
                        indicator_witness(state.remaining_bits, Base::<C>::ONE)?;
                    let (last_segment, last_segment_inverse) =
                        indicator_witness(state.remaining_segments, Base::<C>::ONE)?;
                    let (last_source, last_source_inverse) =
                        indicator_witness(state.remaining_sources, Base::<C>::ONE)?;
                    operation.values[LAST_BIT] = last_bit;
                    operation.values[LAST_BIT_INVERSE] = last_bit_inverse;
                    operation.values[LAST_SEGMENT] = last_segment;
                    operation.values[LAST_SEGMENT_INVERSE] = last_segment_inverse;
                    operation.values[LAST_SOURCE] = last_source;
                    operation.values[LAST_SOURCE_INVERSE] = last_source_inverse;
                    operation.values[SOURCE_ENDPOINT] = last_bit * last_segment;
                    let job_endpoint = last_bit * last_segment * last_source;
                    operation.values[JOB_ENDPOINT] = job_endpoint;
                    let (short_segment, short_segment_inverse) =
                        indicator_witness(state.remaining_segments, Base::<C>::from(7))?;
                    operation.values[SHORT_SEGMENT] = short_segment;
                    operation.values[SHORT_SEGMENT_INVERSE] = short_segment_inverse;
                    let (source_current_x, source_current_y) =
                        affine_coordinates::<C>(&running_source).map_err(|_| Error::Synthesis)?;
                    debug_assert_eq!(state.source_x, source_current_x);
                    debug_assert_eq!(state.source_y, source_current_y);
                    let double_denominator = source_current_y + source_current_y;
                    let double_inverse = Option::<Base<C>>::from(double_denominator.invert())
                        .ok_or(Error::Synthesis)?;
                    operation.values[DOUBLE_INVERSE] = double_inverse;
                    operation.values[DOUBLE_LAMBDA] =
                        Base::<C>::from(3) * source_current_x.square() * double_inverse;
                    if let Some(addend) = joint_digit_point::<C>(&running_source, bit_1, bit_2) {
                        let (accumulator_x, accumulator_y) =
                            affine_coordinates::<C>(&accumulator).map_err(|_| Error::Synthesis)?;
                        let (addend_x, addend_y) =
                            affine_coordinates::<C>(&addend).map_err(|_| Error::Synthesis)?;
                        let delta_x = addend_x - accumulator_x;
                        let add_inverse =
                            Option::<Base<C>>::from(delta_x.invert()).ok_or(Error::Synthesis)?;
                        operation.values[ADD_INVERSE] = add_inverse;
                        operation.values[ADD_LAMBDA] = (addend_y - accumulator_y) * add_inverse;
                        operation.values[DIGIT_ACTIVE] = Base::<C>::ONE;
                        operation.values[DIGIT_X] = addend_x;
                        operation.values[DIGIT_Y] = addend_y;
                        accumulator += addend;
                    }
                    running_source = running_source.double();
                    let (next_source_x, next_source_y) =
                        affine_coordinates::<C>(&running_source).map_err(|_| Error::Synthesis)?;
                    state.source_x = next_source_x;
                    state.source_y = next_source_y;
                    let (next_acc_x, next_acc_y) =
                        affine_coordinates::<C>(&accumulator).map_err(|_| Error::Synthesis)?;
                    operation.values[BUS] = if job_endpoint == Base::<C>::ONE {
                        next_acc_x
                    } else if segment_start {
                        *source.paired_segments[segment].value()
                    } else if last_bit == Base::<C>::ONE {
                        offset_y
                    } else {
                        offset_x
                    };
                    rows.push(operation);
                    state.acc_x = next_acc_x;
                    state.acc_y = next_acc_y;
                    state.part_1 = Base::<C>::from(part_1 >> (local_bit + 1));
                    state.part_2 = Base::<C>::from(part_2 >> (local_bit + 1));
                    state.remaining_bits = Base::<C>::from((width - local_bit - 1) as u64);
                    if local_bit + 1 == width {
                        state.remaining_segments -= Base::<C>::ONE;
                        if segment + 1 == SEGMENTS_PER_SCALAR {
                            state.remaining_sources -= Base::<C>::ONE;
                        }
                    }
                }
            }
        }
        // The final operation constrains this otherwise inactive bus cell to the
        // terminal accumulator's y coordinate.
        rows.push(raw_row(state, state.acc_y, None, None));
        debug_assert_eq!(
            rows.len(),
            (source_end - source_start) * ROWS_PER_SOURCE + ROWS_PER_JOB
        );
        Ok((rows, accumulator))
    }

    fn reference_synthesize<C>(
        jobs: &PastaDenseMsmJobsV1<C>,
        config: &PastaDenseMsmConfigV1,
        layouter: &mut impl Layouter<Base<C>>,
        copy_manager: &SharedCopyConstraintManager<Base<C>>,
        witness_gen_only: bool,
        usable_rows: usize,
    ) -> Result<(), Error>
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField,
    {
        let configured_lanes = config.lane_count();
        jobs.validate_capacity_with_lanes(usable_rows, configured_lanes)
            .map_err(|_| Error::Synthesis)?;
        let physical_cells = if witness_gen_only {
            None
        } else {
            // Base synthesis is complete, so the virtual-to-physical map is
            // immutable for this pass. Keep a guard instead of cloning the
            // multi-million-entry map beside the dense trace.
            Some(copy_manager.lock().map_err(|_| Error::Synthesis)?)
        };
        let mut lane_rows = (0..configured_lanes)
            .map(|_| Vec::<RawRow<Base<C>>>::new())
            .collect::<Vec<_>>();
        let mut rings = Vec::<Vec<LaneEndpoint>>::with_capacity(jobs.jobs.len());
        for (job_index, job) in jobs.jobs.iter().enumerate() {
            let lane_count = job.source_count_tags.len();
            let mut offset = choose_offset::<C>(&job.sources).map_err(|_| Error::Synthesis)?;
            let mut endpoints = Vec::with_capacity(lane_count);
            for (logical_lane, physical_lane) in job.physical_lanes.iter().copied().enumerate() {
                let (source_start, source_end) =
                    dense_shard_bounds(job.sources.len(), lane_count, logical_lane);
                let rows_for_lane = &mut lane_rows[physical_lane];
                let row_start = rows_for_lane.len();
                let (mut rows, terminal) = reference_build_job_lane_rows::<C>(
                    job,
                    job_index,
                    logical_lane,
                    source_start,
                    source_end,
                    offset,
                )?;
                endpoints.push(LaneEndpoint {
                    lane: physical_lane,
                    offset_x_row: row_start + OFFSET_X_BRIDGE_ROW,
                    offset_y_row: row_start + OFFSET_Y_BRIDGE_ROW,
                    terminal_x_row: row_start + rows.len() - 2,
                    terminal_y_row: row_start + rows.len() - 1,
                });
                rows_for_lane.append(&mut rows);
                offset = terminal;
            }
            rings.push(endpoints);
        }
        layouter.assign_region(
            || "Paired Pasta dense normalized-GLV MSM",
            |mut region| {
                let mut buses = (0..configured_lanes)
                    .map(|_| Vec::<Cell>::new())
                    .collect::<Vec<_>>();
                let schedule_rows = lane_rows.iter().map(Vec::len).max().unwrap_or(0);
                for row_index in 0..schedule_rows {
                    region.assign_fixed(
                        config.packed_schedule,
                        row_index,
                        packed_enable_tag_at(&lane_rows, row_index)?,
                    );
                }
                for (lane, rows) in lane_rows.iter().enumerate() {
                    let lane_config = &config.lanes[lane];
                    buses[lane].reserve(rows.len());
                    for (row_index, row) in rows.iter().enumerate() {
                        for column in 0..DENSE_COLUMNS {
                            let value = if jobs.use_unknown {
                                Value::unknown()
                            } else {
                                Value::known(row.values[column])
                            };
                            let cell = region.assign_advice_discarding_value(
                                lane_config.columns[column],
                                row_index,
                                value,
                            );
                            if column == BUS {
                                buses[lane].push(cell);
                            }
                        }
                    }
                }
                if let Some(physical_cells) = &physical_cells {
                    for (lane, rows) in lane_rows.iter().enumerate() {
                        for (row_index, row) in rows.iter().enumerate() {
                            let Some(binding) = row.binding else {
                                continue;
                            };
                            let virtual_value = match binding {
                                BusBinding::Start { job } => jobs.jobs[job].start_tag,
                                BusBinding::SourceCount { job, lane } => {
                                    jobs.jobs[job].source_count_tags[lane]
                                }
                                BusBinding::SourceX { job, source } => {
                                    jobs.jobs[job].sources[source].r_x
                                }
                                BusBinding::SourceY { job, source } => {
                                    jobs.jobs[job].sources[source].r_y
                                }
                                BusBinding::Segment {
                                    job,
                                    source,
                                    segment,
                                } => jobs.jobs[job].sources[source].paired_segments[segment],
                            };
                            bind_virtual(
                                &mut region,
                                buses[lane][row_index],
                                virtual_value,
                                &physical_cells.assigned_advices,
                            )?;
                        }
                    }
                }
                for endpoints in &rings {
                    for (index, endpoint) in endpoints.iter().enumerate() {
                        let next = endpoints[(index + 1) % endpoints.len()];
                        region.constrain_equal(
                            buses[endpoint.lane][endpoint.terminal_x_row],
                            buses[next.lane][next.offset_x_row],
                        );
                        region.constrain_equal(
                            buses[endpoint.lane][endpoint.terminal_y_row],
                            buses[next.lane][next.offset_y_row],
                        );
                    }
                }
                Ok(())
            },
        )
    }

    fn fixture_value<F: BigPrimeField>(values: &mut Vec<F>, value: F) -> AssignedValue<F> {
        let cell = ContextCell::new(EXTERNAL_CELL_TYPE_ID, 0, values.len());
        values.push(value);
        AssignedValue {
            value: Assigned::Trivial(value),
            cell: Some(cell),
        }
    }

    fn fixture_jobs<C>(
        placements: &[Vec<usize>],
        sources_per_job: usize,
    ) -> (PastaDenseMsmJobsV1<C>, Vec<Base<C>>)
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField,
    {
        assert!(sources_per_job > 0 && sources_per_job % 2 == 0);
        let mut values = Vec::new();
        let point = C::Curve::generator().to_affine();
        let mut jobs = Vec::new();
        for placement in placements {
            let start_tag = fixture_value(&mut values, Base::<C>::ONE);
            let source_count_tags = (0..placement.len())
                .map(|lane| {
                    let (start, end) = dense_shard_bounds(sources_per_job, placement.len(), lane);
                    fixture_value(&mut values, Base::<C>::from((end - start) as u64))
                })
                .collect();
            let sources = (0..sources_per_job)
                .map(|index| {
                    // Matching positive/negative halves close the logical MSM across lane boundaries.
                    let r = if index < sources_per_job / 2 {
                        point
                    } else {
                        -point
                    };
                    let (x, y) = r.into_coordinates();
                    let r_x = fixture_value(&mut values, x);
                    let r_y = fixture_value(&mut values, y);
                    let scalars = [
                        0x8000_0000_0000_0000_0000_0000_0020_4081_u128,
                        0x1234_5678_9abc_def0_u128,
                    ];
                    let bits =
                        scalars.map(|scalar| std::array::from_fn(|bit| ((scalar >> bit) & 1) != 0));
                    let paired_segments = std::array::from_fn(|segment| {
                        let (start, width) = segment_spec(segment);
                        let mask = (1_u128 << width) - 1;
                        let first = (scalars[0] >> start) & mask;
                        let second = (scalars[1] >> start) & mask;
                        fixture_value(
                            &mut values,
                            Base::<C>::from(first as u64)
                                + pow2::<Base<C>>(LIMB_BITS) * Base::<C>::from(second as u64),
                        )
                    });
                    ConstrainedDenseSource {
                        r,
                        r_x,
                        r_y,
                        paired_segments,
                        bits,
                    }
                })
                .collect();
            jobs.push(DenseMsmJob {
                start_tag,
                source_count_tags,
                physical_lanes: placement.clone(),
                sources,
            });
        }
        (
            PastaDenseMsmJobsV1 {
                jobs,
                use_unknown: false,
            },
            values,
        )
    }

    fn trace_equivalence<C>()
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField,
    {
        let (jobs, _) = fixture_jobs::<C>(&[vec![1, 0], vec![0, 2], vec![3, 1]], 4);
        let plan = plan_dense_rows(&jobs.jobs, 4, 1024).unwrap();
        let mut reference = vec![Vec::<RawRow<Base<C>>>::new(); 4];
        let mut streamed = vec![Vec::<RawRow<Base<C>>>::new(); 4];
        let mut emitted = 0;
        for (job_index, job) in jobs.jobs.iter().enumerate() {
            let mut old_offset = choose_offset::<C>(&job.sources).unwrap();
            let mut new_offset = old_offset;
            for shard in &plan.jobs[job_index] {
                assert_eq!(shard.row_start, reference[shard.physical_lane].len());
                let (rows, terminal) = reference_build_job_lane_rows::<C>(
                    job,
                    job_index,
                    shard.logical_lane,
                    shard.source_start,
                    shard.source_end,
                    old_offset,
                )
                .unwrap();
                let (count, new_terminal) = emit_job_lane_rows::<C>(
                    job,
                    job_index,
                    shard.logical_lane,
                    shard.source_start,
                    shard.source_end,
                    new_offset,
                    |row_index, row| {
                        assert_eq!(
                            row_index + shard.row_start,
                            streamed[shard.physical_lane].len()
                        );
                        assert_eq!(
                            row.enable_tag,
                            dense_row_enable_tag(row_index, shard.row_count)?
                        );
                        streamed[shard.physical_lane].push(row);
                        emitted += 1;
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(count, rows.len());
                assert_eq!(count, shard.row_count);
                assert_eq!(new_terminal, terminal);
                reference[shard.physical_lane].extend(rows);
                old_offset = terminal;
                new_offset = new_terminal;
            }
        }
        assert_eq!(emitted, reference.iter().map(Vec::len).sum::<usize>());
        for (lane, (old, new)) in reference.iter().zip(&streamed).enumerate() {
            assert_eq!(old.len(), new.len());
            for (old, new) in old.iter().zip(new) {
                assert_eq!(old.values, new.values);
                assert_eq!(old.binding, new.binding);
                assert_eq!(old.enable_tag, new.enable_tag);
            }
            let old_copies = old
                .iter()
                .enumerate()
                .filter_map(|(row, value)| value.binding.map(|binding| (lane, row, binding)))
                .collect::<Vec<_>>();
            let new_copies = new
                .iter()
                .enumerate()
                .filter_map(|(row, value)| value.binding.map(|binding| (lane, row, binding)))
                .collect::<Vec<_>>();
            assert_eq!(old_copies, new_copies);
            assert_eq!(new_copies.len(), plan.bound_bus_counts[lane]);
        }
        assert_eq!(
            plan.packed_schedule.len(),
            reference.iter().map(Vec::len).max().unwrap()
        );
        for (row, packed) in plan.packed_schedule.iter().enumerate() {
            assert_eq!(
                Base::<C>::from(*packed),
                packed_enable_tag_at(&reference, row).unwrap()
            );
        }
    }

    #[test]
    fn streamed_rows_match_independent_vector_values_bindings_offsets_and_tags() {
        trace_equivalence::<EqAffine>();
        trace_equivalence::<EpAffine>();
    }

    #[test]
    fn streaming_plan_bounds_full_capacity_and_rejects_invalid_shapes() {
        let (jobs, _) = fixture_jobs::<EqAffine>(&[vec![0, 1]], 1008);
        let plan = plan_dense_rows(&jobs.jobs, 2, K16_USABLE_ROWS).unwrap();
        assert_eq!(plan.packed_schedule.len(), 65_523);
        assert_eq!(plan.bound_bus_counts, vec![10_586, 10_586]);
        let retained_bound = plan.packed_schedule.capacity() * std::mem::size_of::<u64>()
            + plan.bound_bus_counts.iter().sum::<usize>()
                * std::mem::size_of::<(Cell, BusBinding)>()
            + 2 * (std::mem::size_of::<DenseShardRows>() + 4 * std::mem::size_of::<Cell>());
        assert!(
            retained_bound < 2 * 1024 * 1024,
            "selected metadata bound {retained_bound}"
        );
        assert!(plan_dense_rows(&jobs.jobs, 2, 65_522).is_err());
        assert!(plan_dense_rows(&jobs.jobs, 0, K16_USABLE_ROWS).is_err());
        assert!(plan_dense_rows(&jobs.jobs, 5, K16_USABLE_ROWS).is_err());
        let mut duplicated = jobs.clone();
        duplicated.jobs[0].physical_lanes = vec![0, 0];
        assert!(plan_dense_rows(&duplicated.jobs, 2, K16_USABLE_ROWS).is_err());
        duplicated.jobs[0].physical_lanes = vec![0, 2];
        assert!(plan_dense_rows(&duplicated.jobs, 2, K16_USABLE_ROWS).is_err());
        duplicated.jobs[0].physical_lanes = vec![0];
        assert!(plan_dense_rows(&duplicated.jobs, 2, K16_USABLE_ROWS).is_err());
        let empty = plan_dense_rows::<EqAffine>(&[], 2, K16_USABLE_ROWS).unwrap();
        assert!(empty.jobs.is_empty() && empty.packed_schedule.is_empty());
        assert_eq!(empty.bound_bus_counts, vec![0, 0]);
        assert!(dense_row_enable_tag(0, ROWS_PER_SOURCE).is_err());
        assert!(dense_row_enable_tag(133, 133).is_err());
    }

    #[test]
    fn streaming_emitter_stops_at_sink_error_without_later_rows() {
        let (jobs, _) = fixture_jobs::<EqAffine>(&[vec![0]], 2);
        let job = &jobs.jobs[0];
        let offset = choose_offset::<EqAffine>(&job.sources).unwrap();
        for fail_at in [0, 1, 4, 10, 2 * ROWS_PER_SOURCE + ROWS_PER_JOB - 1] {
            let mut calls = 0;
            let result = emit_job_lane_rows::<EqAffine>(job, 0, 0, 0, 2, offset, |row_index, _| {
                assert_eq!(row_index, calls);
                calls += 1;
                if row_index == fail_at {
                    Err(Error::Synthesis)
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err());
            assert_eq!(calls, fail_at + 1);
        }
    }

    #[derive(Clone)]
    struct StreamingFixture<C: CurveAffineExt, const REFERENCE: bool>
    where
        Base<C>: BigPrimeField,
    {
        jobs: PastaDenseMsmJobsV1<C>,
        values: Vec<Base<C>>,
    }
    #[derive(Clone)]
    struct StreamingConfig {
        source: Column<Advice>,
        dense: PastaDenseMsmConfigV1,
    }
    impl<C: CurveAffineExt, const REFERENCE: bool> Circuit<Base<C>> for StreamingFixture<C, REFERENCE>
    where
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField,
    {
        type Config = StreamingConfig;
        type FloorPlanner = V1;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self {
                jobs: self.jobs.unknown(),
                values: self.values.clone(),
            }
        }
        fn configure(meta: &mut ConstraintSystem<Base<C>>) -> Self::Config {
            let source = meta.advice_column();
            meta.enable_equality(source);
            StreamingConfig {
                source,
                dense: PastaDenseMsmConfigV1::configure::<C>(meta),
            }
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Base<C>>,
        ) -> Result<(), Error> {
            let copy_manager: SharedCopyConstraintManager<Base<C>> = Default::default();
            layouter.assign_region(
                || "exact source BUS bindings",
                |mut region| {
                    let mut copies = copy_manager.lock().map_err(|_| Error::Synthesis)?;
                    for (row, value) in self.values.iter().copied().enumerate() {
                        let value = if self.jobs.use_unknown {
                            Value::unknown()
                        } else {
                            Value::known(value)
                        };
                        let cell = region.assign_advice_discarding_value(config.source, row, value);
                        copies
                            .assigned_advices
                            .insert(ContextCell::new(EXTERNAL_CELL_TYPE_ID, 0, row), cell);
                    }
                    Ok(())
                },
            )?;
            if REFERENCE {
                reference_synthesize(
                    &self.jobs,
                    &config.dense,
                    &mut layouter,
                    &copy_manager,
                    false,
                    1024 - 9,
                )
            } else {
                self.jobs
                    .synthesize(&config.dense, &mut layouter, &copy_manager, false, 1024 - 9)
            }
        }
    }

    #[test]
    fn streamed_multi_job_keys_and_proofs_match_independent_vector_reference_both_parities() {
        use halo2_base::halo2_proofs::{
            SerdeFormat,
            plonk::{create_proof, keygen_pk2, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::ParamsProver,
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                    strategy::SingleStrategy,
                },
            },
            transcript::{
                Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer,
                TranscriptWriterBuffer,
            },
        };
        macro_rules! check {
            ($dense:ty, $proof:ty, $field:ty) => {{
                let (jobs, values) =
                    fixture_jobs::<$dense>(&[vec![1, 0], vec![0, 2], vec![3, 1]], 4);
                let old = StreamingFixture::<$dense, true> {
                    jobs: jobs.clone(),
                    values: values.clone(),
                };
                let new = StreamingFixture::<$dense, false> { jobs, values };
                MockProver::run(10, &old, vec![])
                    .unwrap()
                    .assert_satisfied();
                MockProver::run(10, &new, vec![])
                    .unwrap()
                    .assert_satisfied();
                let params = ParamsIPA::<$proof>::new(10);
                let old_pk = keygen_pk2(&params, &old, true).unwrap();
                let new_pk = keygen_pk2(&params, &new, true).unwrap();
                assert_eq!(
                    old_pk.get_vk().to_bytes(SerdeFormat::Processed),
                    new_pk.get_vk().to_bytes(SerdeFormat::Processed)
                );
                assert_eq!(
                    old_pk.to_bytes(SerdeFormat::Processed),
                    new_pk.to_bytes(SerdeFormat::Processed)
                );
                let instances: &[&[&[$field]]] = &[&[]];
                let seed =
                    iroha_crypto::kagemusha::KagemushaRecoverySeedV1::from_unsealed([71; 32])
                        .expect("public fixture seed");
                let mut old_transcript =
                    Blake2bWrite::<_, $proof, Challenge255<$proof>>::init(Vec::new());
                create_proof::<IPACommitmentScheme<$proof>, ProverIPA<$proof>, _, _, _, _>(
                    &params,
                    &old_pk,
                    &[old],
                    instances,
                    seed.rng(b"dense-streaming-test", &[0; 32])
                        .expect("fixture proof RNG"),
                    &mut old_transcript,
                )
                .unwrap();
                let mut new_transcript =
                    Blake2bWrite::<_, $proof, Challenge255<$proof>>::init(Vec::new());
                create_proof::<IPACommitmentScheme<$proof>, ProverIPA<$proof>, _, _, _, _>(
                    &params,
                    &new_pk,
                    &[new],
                    instances,
                    seed.rng(b"dense-streaming-test", &[0; 32])
                        .expect("fixture proof RNG"),
                    &mut new_transcript,
                )
                .unwrap();
                let old_bytes = old_transcript.finalize();
                let new_bytes = new_transcript.finalize();
                assert_eq!(old_bytes, new_bytes);
                let mut verifier =
                    Blake2bRead::<_, $proof, Challenge255<$proof>>::init(new_bytes.as_slice());
                verify_proof::<IPACommitmentScheme<$proof>, VerifierIPA<$proof>, _, _, _>(
                    &params,
                    new_pk.get_vk(),
                    SingleStrategy::new(&params),
                    instances,
                    &mut verifier,
                )
                .unwrap();
                let mut corrupted = new_bytes;
                let last = corrupted.len() - 1;
                corrupted[last] ^= 1;
                let mut verifier =
                    Blake2bRead::<_, $proof, Challenge255<$proof>>::init(corrupted.as_slice());
                assert!(
                    verify_proof::<IPACommitmentScheme<$proof>, VerifierIPA<$proof>, _, _, _>(
                        &params,
                        new_pk.get_vk(),
                        SingleStrategy::new(&params),
                        instances,
                        &mut verifier
                    )
                    .is_err()
                );
            }};
        }
        check!(EqAffine, EpAffine, Fq);
        check!(EpAffine, EqAffine, Fp);
    }

    fn benchmark_full_rows<const REFERENCE: bool>() {
        let (jobs, _) = fixture_jobs::<EqAffine>(&[vec![0, 1]], 1008);
        let start = Instant::now();
        let job = &jobs.jobs[0];
        let mut offset = choose_offset::<EqAffine>(&job.sources).unwrap();
        let mut checksum = Fq::ZERO;
        let mut row_count = 0;
        let mut retained = Vec::<Vec<RawRow<Fq>>>::new();
        let plan = if REFERENCE {
            None
        } else {
            Some(plan_dense_rows(&jobs.jobs, 2, K16_USABLE_ROWS).unwrap())
        };
        for lane in 0..2 {
            let (source_start, source_end) = dense_shard_bounds(1008, 2, lane);
            if REFERENCE {
                let (rows, terminal) = reference_build_job_lane_rows::<EqAffine>(
                    job,
                    0,
                    lane,
                    source_start,
                    source_end,
                    offset,
                )
                .unwrap();
                offset = terminal;
                row_count += rows.len();
                retained.push(rows);
            } else {
                let (count, terminal) = emit_job_lane_rows::<EqAffine>(
                    job,
                    0,
                    lane,
                    source_start,
                    source_end,
                    offset,
                    |_, row| {
                        checksum += row.values[BUS] + row.values[ACC_X];
                        std::hint::black_box(row);
                        Ok(())
                    },
                )
                .unwrap();
                offset = terminal;
                row_count += count;
            }
        }
        if REFERENCE {
            for rows in &retained {
                for row in rows {
                    checksum += row.values[BUS] + row.values[ACC_X];
                    std::hint::black_box(row);
                }
            }
        }
        assert_eq!(row_count, 2 * 65_523);
        std::hint::black_box(&plan);
        eprintln!(
            "DENSE_ROW_BENCH reference={REFERENCE} sources=1008 rows={row_count} elapsed_ms={} retained_raw_rows={} checksum={checksum:?}",
            start.elapsed().as_millis(),
            retained.iter().map(Vec::len).sum::<usize>()
        );
    }

    #[test]
    #[ignore = "Run separately with /usr/bin/time -l for comparable 1008-source row-generation measurements"]
    fn dense_rows_benchmark_reference_1008_sources() {
        benchmark_full_rows::<true>();
    }

    #[test]
    #[ignore = "Run separately with /usr/bin/time -l for comparable 1008-source row-generation measurements"]
    fn dense_rows_benchmark_streaming_1008_sources() {
        benchmark_full_rows::<false>();
    }
}
