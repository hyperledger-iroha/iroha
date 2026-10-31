//! Differential controls against the exact pre-reuse SHA-word builders.
//!
//! The reference module is compiled only by this parent test module. It retains
//! the original duplicate construction to check complete rows, memory events,
//! fixed schedules and work counts without a second production implementation.

use super::super::sha256_word_air::observe_circuit_builds_v1;
use super::*;
use sha2::{Digest as _, Sha256};

mod reference {
    use super::super::*;

    pub(super) fn base_v1(
        statement: ZkX509ShaWordStarkStatementV1,
        message: &[u8],
    ) -> Result<ZkX509ShaWordStarkBaseV1, ZkX509ShaWordStarkErrorV1> {
        if message.len() != statement.message_len
            || statement.message_len > ZK_X509_DER_MAX_DOCUMENT_BYTES_V1
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let circuit = build_sha256_word_circuit_v1(message)?;
        if circuit.digest() != statement.digest {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let shape_message = vec![0_u8; statement.message_len];
        let shape = build_sha256_word_circuit_v1(&shape_message)?;
        if circuit.stark_words_v1().len() != shape.stark_words_v1().len()
            || circuit.stark_operations_v1().len() != shape.stark_operations_v1().len()
            || circuit
                .stark_operations_v1()
                .iter()
                .zip(shape.stark_operations_v1())
                .any(|(actual, expected)| !actual.same_topology(expected))
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let expected_blocks = capacity_blocks_v1(statement.message_len)?;
        let expected_rows = capacity_local_rows_v1(expected_blocks)?
            .checked_add(capacity_memory_rows_v1(expected_blocks)?)
            .ok_or(ZkX509ShaWordStarkErrorV1::Resource)?;
        let mut rows = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1);
        let mut fixed = PrivateTableV1::new(Vec::new(), zeroize_sha_word_private_fixed_v1);
        let mut events = PrivateTableV1::new(Vec::new(), zeroize_sha_word_event_rows_v1);
        rows.try_reserve_exact(expected_rows)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        fixed
            .try_reserve_exact(expected_rows)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        events
            .try_reserve_exact(expected_rows)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        let mut word_cursor = 0_usize;
        let mut operation_outputs = BTreeSet::new();
        for operation in circuit.stark_operations_v1() {
            let output = operation_output(operation);
            if !operation_outputs.insert(output.0) || output.0 < word_cursor {
                return Err(ZkX509ShaWordStarkErrorV1::Topology);
            }
            while word_cursor < output.0 {
                push_word_row(
                    &mut rows,
                    &mut fixed,
                    &mut events,
                    &circuit,
                    &shape,
                    word_cursor,
                )?;
                word_cursor += 1;
            }
            push_operation_rows(&mut rows, &mut fixed, &mut events, &circuit, operation)?;
            word_cursor = output
                .0
                .checked_add(1)
                .ok_or(ZkX509ShaWordStarkErrorV1::Resource)?;
        }
        while word_cursor < circuit.stark_words_v1().len() {
            push_word_row(
                &mut rows,
                &mut fixed,
                &mut events,
                &circuit,
                &shape,
                word_cursor,
            )?;
            word_cursor += 1;
        }
        let output_ids = circuit.stark_output_words_v1();
        let expected_digest = digest_words(statement.digest);
        for (index, output) in output_ids.into_iter().enumerate() {
            let word = circuit
                .stark_words_v1()
                .get(output.0)
                .ok_or(ZkX509ShaWordStarkErrorV1::Topology)?;
            let mut row = vec![F::ZERO; SHA_WORD_BASE_WIDTH_V1];
            row[0] = word.value;
            row[1..33].copy_from_slice(&word.bits);
            rows.push(row);
            fixed.push(ShaWordFixedRowV1::Digest {
                address: output.0,
                expected: expected_digest[index],
            });
            events.push(vec![read(output, word.value)?]);
        }
        if rows.len() != fixed.len() || rows.len() != events.len() {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let local_rows = rows.len();
        let event_count = events.iter().try_fold(0_usize, |count, row| {
            count
                .checked_add(row.len())
                .ok_or(ZkX509ShaWordStarkErrorV1::Resource)
        })?;
        let mut execution = PrivateTableV1::new(Vec::new(), zeroize_sha_word_events_v1);
        execution
            .try_reserve_exact(event_count)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        for row in events.iter() {
            execution.extend_from_slice(row);
        }
        let mut sorted = PrivateTableV1::new(Vec::new(), zeroize_sha_word_events_v1);
        sorted
            .try_reserve_exact(event_count)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        sorted.extend_from_slice(&execution);
        // Each word address has one immutable value. Equal address/write keys
        // therefore have identical event tuples; unstable sorting preserves the
        // canonical bytes without a heap scratch copy of private events.
        sorted
            .sort_unstable_by_key(|access| (access.address.0, u8::from(access.is_write != F::ONE)));
        if sorted.as_slice() != circuit.stark_memory_v1().sorted.as_slice() {
            return Err(ZkX509ShaWordStarkErrorV1::LocalCopy);
        }
        for index in 0..execution.len() {
            let mut row = vec![F::ZERO; SHA_WORD_BASE_WIDTH_V1];
            row[0] = execution[index].address;
            row[1] = execution[index].value;
            row[2] = execution[index].is_write;
            row[3] = sorted[index].address;
            row[4] = sorted[index].value;
            row[5] = sorted[index].is_write;
            rows.push(row);
            let execution_address = usize::try_from(execution[index].address.0)
                .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
            let sorted_address = usize::try_from(sorted[index].address.0)
                .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
            fixed.push(ShaWordFixedRowV1::Memory {
                execution_address,
                execution_write: execution[index].is_write == F::ONE,
                sorted_address,
                sorted_write: sorted[index].is_write == F::ONE,
                sorted_same_address_next: sorted
                    .get(index + 1)
                    .is_some_and(|next| next.address == sorted[index].address),
                memory_first: index == 0,
                memory_last: index + 1 == execution.len(),
            });
            events.push(Vec::new());
        }
        let segment_rows = SHA256_WORD_FIXED_BATCH_SEGMENT_ROWS_V1;
        let total_rows = rows.len();
        if total_rows != expected_rows {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let segment_count = total_rows.div_ceil(segment_rows);
        if segment_count == 0 || segment_count > SHA256_WORD_FIXED_BATCH_SEGMENT_COUNT_V1 {
            return Err(ZkX509ShaWordStarkErrorV1::Resource);
        }
        #[cfg(test)]
        let active_rows_per_segment = (0..segment_count)
            .map(|segment| {
                total_rows
                    .saturating_sub(segment * segment_rows)
                    .min(segment_rows)
            })
            .collect();
        // The suffix contains only empty memory-row event vectors. Move the local
        // events into their clearing owner rather than cloning private event cells.
        events.truncate(local_rows);
        Ok(ZkX509ShaWordStarkBaseV1 {
            #[cfg(test)]
            statement,
            base_rows: rows.into_vec(),
            fixed_rows: fixed.into_vec(),
            local_events: events.into_vec(),
            execution: execution.into_vec(),
            sorted: sorted.into_vec(),
            local_rows,
            #[cfg(test)]
            segment_rows,
            #[cfg(test)]
            active_rows_per_segment,
        })
    }

    pub(super) fn fixed_v1(
        maximum_message_len: usize,
        exact_length: bool,
    ) -> Result<ZkX509ShaWordCapacityFixedScheduleV1, ZkX509ShaWordStarkErrorV1> {
        if maximum_message_len > ZK_X509_DER_MAX_DOCUMENT_BYTES_V1 {
            return Err(ZkX509ShaWordStarkErrorV1::Resource);
        }
        let maximum_blocks = capacity_blocks_v1(maximum_message_len)?;
        let maximum_local_rows = capacity_local_rows_v1(maximum_blocks)?;
        let maximum_memory_rows = capacity_memory_rows_v1(maximum_blocks)?;
        let maximum_logical_rows = maximum_local_rows
            .checked_add(maximum_memory_rows)
            .ok_or(ZkX509ShaWordStarkErrorV1::Resource)?;
        if maximum_logical_rows > SHA_WORD_AGGREGATE_TRACE_SIZE_V1 {
            return Err(ZkX509ShaWordStarkErrorV1::Resource);
        }
        let maximum_compute_rows = maximum_local_rows
            .checked_sub(SHA_WORD_CAPACITY_LOCAL_ROWS_PER_CALL_V1)
            .ok_or(ZkX509ShaWordStarkErrorV1::Topology)?;
        if maximum_compute_rows
            != maximum_blocks * SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1
                + SHA_WORD_CAPACITY_LOCAL_INITIAL_ROWS_PER_CALL_V1
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        // Only topology is retained. The all-zero shape message is public,
        // deterministic compiler input and is dropped before this function
        // returns.
        let maximum_message = vec![0_u8; maximum_message_len];
        let maximum_circuit = build_sha256_word_circuit_v1(&maximum_message)?;
        let maximum_statement = ZkX509ShaWordStarkStatementV1 {
            message_len: maximum_message_len,
            digest: maximum_circuit.digest(),
        };
        let mut maximum = base_v1(maximum_statement, &maximum_message)?;
        drop(maximum_message);
        if maximum.local_rows != maximum_local_rows
            || maximum.execution.len() != maximum_memory_rows
            || maximum.fixed_rows.len() != maximum_logical_rows
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let slots = aggregate_slots_v1(maximum.local_rows, maximum_logical_rows)?;
        let fixed_rows = core::mem::take(&mut maximum.fixed_rows);
        drop(maximum);
        let word = ZkX509ShaWordStarkFixedScheduleV1 {
            #[cfg(test)]
            statement: maximum_statement,
            fixed_rows,
            local_rows: maximum_local_rows,
            logical_rows: maximum_logical_rows,
            slots,
        };
        let mut input_word_indices = maximum_circuit
            .stark_input_words_v1()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, word)| (word.0, index))
            .collect::<Vec<_>>();
        input_word_indices.sort_unstable_by_key(|entry| entry.0);
        drop(maximum_circuit);
        if input_word_indices.len() != maximum_blocks * 16
            || input_word_indices
                .windows(2)
                .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        Ok(ZkX509ShaWordCapacityFixedScheduleV1 {
            maximum_message_len,
            exact_length,
            maximum_blocks,
            maximum_local_rows,
            maximum_memory_rows,
            maximum_compute_rows,
            word,
            input_word_indices,
        })
    }

    pub(super) fn capacity_v1(
        message: &[u8],
        maximum_message_len: usize,
        exact_length: bool,
    ) -> Result<ZkX509ShaWordCapacityBaseSourceV1, ZkX509ShaWordStarkErrorV1> {
        if message.len() > maximum_message_len
            || maximum_message_len > ZK_X509_DER_MAX_DOCUMENT_BYTES_V1
        {
            return Err(ZkX509ShaWordStarkErrorV1::Resource);
        }
        if exact_length && message.len() != maximum_message_len {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let active_blocks = capacity_blocks_v1(message.len())?;
        let fixed_schedule = fixed_v1(maximum_message_len, exact_length)?;
        let maximum_blocks = fixed_schedule.maximum_blocks();
        let maximum_local_rows = fixed_schedule.maximum_local_rows();
        let maximum_memory_rows = fixed_schedule.maximum_memory_rows();
        let maximum_logical_rows = fixed_schedule.logical_rows();
        let actual_circuit = build_sha256_word_circuit_v1(message)?;
        let actual_statement = ZkX509ShaWordStarkStatementV1 {
            message_len: message.len(),
            digest: actual_circuit.digest(),
        };
        let mut actual = base_v1(actual_statement, message)?;
        drop(actual_circuit);
        if actual.local_rows != capacity_local_rows_v1(active_blocks)?
            || actual.execution.len() != capacity_memory_rows_v1(active_blocks)?
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let maximum_compute_rows = fixed_schedule.maximum_compute_rows;
        let actual_compute_rows = actual
            .local_rows
            .checked_sub(SHA_WORD_CAPACITY_LOCAL_ROWS_PER_CALL_V1)
            .ok_or(ZkX509ShaWordStarkErrorV1::Topology)?;
        if maximum_compute_rows
            != maximum_blocks * SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1
                + SHA_WORD_CAPACITY_LOCAL_INITIAL_ROWS_PER_CALL_V1
            || actual_compute_rows
                != active_blocks * SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1
                    + SHA_WORD_CAPACITY_LOCAL_INITIAL_ROWS_PER_CALL_V1
        {
            return Err(ZkX509ShaWordStarkErrorV1::Topology);
        }
        let mut base_rows = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1);
        let mut fixed_rows = Vec::new();
        base_rows
            .try_reserve_exact(maximum_logical_rows)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        fixed_rows
            .try_reserve_exact(maximum_logical_rows)
            .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
        for index in 0..maximum_local_rows {
            let active_compute = index < actual_compute_rows;
            let digest_index = index
                .checked_sub(maximum_compute_rows)
                .filter(|digest| *digest < SHA_WORD_CAPACITY_LOCAL_ROWS_PER_CALL_V1);
            let mut base = [F::ZERO; SHA_WORD_CAPACITY_BASE_WIDTH_V1];
            if active_compute {
                base[..SHA_WORD_BASE_WIDTH_V1]
                    .copy_from_slice(&capacity_raw_base_row_v1(&actual.base_rows[index])?);
                base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1] = F::ONE;
            } else if let Some(digest_index) = digest_index {
                let actual_index = actual_compute_rows + digest_index;
                base[..SHA_WORD_BASE_WIDTH_V1]
                    .copy_from_slice(&capacity_raw_base_row_v1(&actual.base_rows[actual_index])?);
                base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1] = F::ONE;
                let ShaWordFixedRowV1::Digest { address, .. } = actual
                    .fixed_rows
                    .get(actual_index)
                    .ok_or(ZkX509ShaWordStarkErrorV1::Topology)?
                else {
                    return Err(ZkX509ShaWordStarkErrorV1::Topology);
                };
                base[SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1] =
                    F(u64::try_from(*address).map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?);
            }
            if index >= SHA_WORD_CAPACITY_LOCAL_INITIAL_ROWS_PER_CALL_V1
                && index < maximum_compute_rows
            {
                let block_row = index
                    .checked_sub(SHA_WORD_CAPACITY_LOCAL_INITIAL_ROWS_PER_CALL_V1)
                    .ok_or(ZkX509ShaWordStarkErrorV1::Topology)?;
                let block = block_row / SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1;
                base[SHA_WORD_CAPACITY_FINAL_BLOCK_V1] =
                    F(u64::from(active_compute && block + 1 == active_blocks));
            }
            let fixed = fixed_schedule.fixed_row_v1(index)?;
            if active_compute && fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE {
                let input_word = usize::try_from(fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0)
                    .map_err(|_| ZkX509ShaWordStarkErrorV1::Resource)?;
                for byte in 0..4 {
                    let byte_index = input_word
                        .checked_mul(4)
                        .and_then(|offset| offset.checked_add(byte))
                        .ok_or(ZkX509ShaWordStarkErrorV1::Resource)?;
                    if byte_index < message.len() {
                        base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte] = F::ONE;
                    } else if byte_index == message.len() {
                        base[SHA_WORD_CAPACITY_MARKER_MASK_V1 + byte] = F::ONE;
                    }
                }
            }
            base_rows.push(base);
            fixed_rows.push(fixed);
        }
        for value in actual.base_rows.iter_mut().flatten() {
            value.zeroize_v1();
        }
        drop(core::mem::take(&mut actual.base_rows));
        for fixed in &mut actual.fixed_rows {
            if let ShaWordFixedRowV1::Digest { expected, .. } = fixed {
                zeroize::Zeroize::zeroize(expected);
            }
        }
        drop(core::mem::take(&mut actual.fixed_rows));
        for memory_index in 0..maximum_memory_rows {
            let index = maximum_local_rows + memory_index;
            let memory_active = memory_index < actual.execution.len();
            let mut base = [F::ZERO; SHA_WORD_CAPACITY_BASE_WIDTH_V1];
            if memory_active {
                let execution = actual.execution[memory_index];
                let sorted = actual.sorted[memory_index];
                base[0] = execution.address;
                base[1] = execution.value;
                base[2] = execution.is_write;
                base[3] = sorted.address;
                base[4] = sorted.value;
                base[5] = sorted.is_write;
                base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1] = F::ONE;
                base[SHA_WORD_CAPACITY_SORTED_SAME_NEXT_V1] = F(u64::from(
                    actual
                        .sorted
                        .get(memory_index + 1)
                        .is_some_and(|next| next.address == sorted.address),
                ));
            }
            let fixed = fixed_schedule.fixed_row_v1(index)?;
            base_rows.push(base);
            fixed_rows.push(fixed);
        }
        drop(fixed_schedule);
        Ok(ZkX509ShaWordCapacityBaseSourceV1 {
            message_len: message.len(),
            #[cfg(test)]
            maximum_message_len,
            #[cfg(test)]
            exact_length,
            active_blocks,
            maximum_blocks,
            maximum_local_rows,
            maximum_memory_rows,
            actual_compute_rows,
            maximum_compute_rows,
            base_rows: base_rows.into_vec(),
            fixed_rows,
            local_events: core::mem::take(&mut actual.local_events),
            execution: core::mem::take(&mut actual.execution),
            sorted: core::mem::take(&mut actual.sorted),
        })
    }
}

fn message(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index * 17 + 3) % 251) as u8)
        .collect()
}

fn assert_base_equal(actual: &ZkX509ShaWordStarkBaseV1, expected: &ZkX509ShaWordStarkBaseV1) {
    assert_eq!(actual.statement, expected.statement);
    assert_eq!(actual.local_rows, expected.local_rows);
    assert_eq!(actual.segment_rows, expected.segment_rows);
    assert_eq!(
        actual.active_rows_per_segment,
        expected.active_rows_per_segment
    );
    assert!(
        actual.base_rows == expected.base_rows,
        "all base rows and columns"
    );
    assert!(
        actual.fixed_rows == expected.fixed_rows,
        "all fixed rows and columns"
    );
    assert!(
        actual.local_events == expected.local_events,
        "local memory events"
    );
    assert!(
        actual.execution == expected.execution,
        "execution memory rows"
    );
    assert!(
        actual.sorted == expected.sorted,
        "canonical sorted memory rows"
    );
}

fn assert_capacity_equal(
    actual: &ZkX509ShaWordCapacityBaseSourceV1,
    expected: &ZkX509ShaWordCapacityBaseSourceV1,
) {
    assert_eq!(actual.message_len, expected.message_len);
    assert_eq!(actual.maximum_message_len, expected.maximum_message_len);
    assert_eq!(actual.exact_length, expected.exact_length);
    assert_eq!(actual.active_blocks, expected.active_blocks);
    assert_eq!(actual.maximum_blocks, expected.maximum_blocks);
    assert_eq!(actual.maximum_local_rows, expected.maximum_local_rows);
    assert_eq!(actual.maximum_memory_rows, expected.maximum_memory_rows);
    assert_eq!(actual.actual_compute_rows, expected.actual_compute_rows);
    assert_eq!(actual.maximum_compute_rows, expected.maximum_compute_rows);
    assert!(
        actual.base_rows == expected.base_rows,
        "all capacity base rows and columns"
    );
    assert!(
        actual.fixed_rows == expected.fixed_rows,
        "all capacity fixed rows and columns"
    );
    assert!(
        actual.local_events == expected.local_events,
        "capacity local memory events"
    );
    assert!(
        actual.execution == expected.execution,
        "capacity execution memory rows"
    );
    assert!(
        actual.sorted == expected.sorted,
        "capacity sorted memory rows"
    );
}

#[test]
fn reused_circuit_matches_original_base_at_padding_and_profile_boundaries() {
    for length in [0, 1, 55, 56, 63, 64, 65, 127, 313, 4_096] {
        let bytes = message(length);
        let statement = ZkX509ShaWordStarkStatementV1 {
            message_len: length,
            digest: Sha256::digest(&bytes).into(),
        };
        let expected = reference::base_v1(statement, &bytes).expect("original base");
        let circuit = build_sha256_word_circuit_v1(&bytes).expect("checked circuit");
        let (actual, builds) = observe_circuit_builds_v1(|| {
            build_sha_word_stark_base_from_circuit_v1(statement, &circuit)
        });
        assert_eq!(
            builds, 1,
            "only the independent zero-message shape is rebuilt"
        );
        assert_base_equal(&actual.expect("reused circuit base"), &expected);
    }
}

#[test]
fn capacity_reuses_circuits_with_exact_original_rows_and_less_construction() {
    for (length, maximum, exact) in [
        (0, 0, true),
        (55, 63, false),
        (56, 127, false),
        (65, 127, false),
        (313, 313, true),
        (4_096, 4_096, true),
    ] {
        let bytes = message(length);
        let (expected, old_builds) =
            observe_circuit_builds_v1(|| reference::capacity_v1(&bytes, maximum, exact));
        let (actual, new_builds) = observe_circuit_builds_v1(|| {
            build_sha_word_capacity_base_source_v1(&bytes, maximum, exact)
        });
        assert_eq!(old_builds, 6, "frozen reference constructs six circuits");
        assert_eq!(
            new_builds, 4,
            "remove two duplicate circuits, keep both independent shapes"
        );
        assert_capacity_equal(
            &actual.expect("reused capacity"),
            &expected.expect("original capacity"),
        );
    }
}

#[test]
#[ignore = "full 16 KiB capacity parity; run explicitly in an isolated focused test"]
fn full_supported_capacity_reuse_matches_original_every_row_and_column() {
    let bytes = message(ZK_X509_DER_MAX_DOCUMENT_BYTES_V1);
    let expected =
        reference::capacity_v1(&bytes, bytes.len(), true).expect("original full capacity");
    let actual = build_sha_word_capacity_base_source_v1(&bytes, bytes.len(), true)
        .expect("reused full capacity");
    assert_capacity_equal(&actual, &expected);
}

#[test]
fn fixed_schedule_reuses_circuit_and_preserves_complete_public_topology() {
    for (maximum, exact) in [(0, true), (127, false), (4_096, false)] {
        let (expected, old_builds) =
            observe_circuit_builds_v1(|| reference::fixed_v1(maximum, exact));
        let (actual, new_builds) = observe_circuit_builds_v1(|| {
            compile_sha_word_capacity_fixed_schedule_v1(maximum, exact)
        });
        assert_eq!(old_builds, 3);
        assert_eq!(
            new_builds, 2,
            "one maximum circuit and one independent zero shape"
        );
        let expected = expected.expect("original fixed schedule");
        let actual = actual.expect("reused fixed schedule");
        assert_eq!(actual.maximum_message_len, expected.maximum_message_len);
        assert_eq!(actual.exact_length, expected.exact_length);
        assert_eq!(actual.maximum_blocks, expected.maximum_blocks);
        assert_eq!(actual.maximum_local_rows, expected.maximum_local_rows);
        assert_eq!(actual.maximum_memory_rows, expected.maximum_memory_rows);
        assert_eq!(actual.maximum_compute_rows, expected.maximum_compute_rows);
        assert_eq!(actual.input_word_indices, expected.input_word_indices);
        for row in 0..expected.logical_rows() {
            assert_eq!(
                actual.fixed_row_v1(row),
                expected.fixed_row_v1(row),
                "row {row}"
            );
        }
    }
}

#[test]
fn reused_circuit_rejects_statement_substitution_before_shape_construction() {
    let bytes = message(63);
    let circuit = build_sha256_word_circuit_v1(&bytes).expect("checked circuit");
    let correct = ZkX509ShaWordStarkStatementV1 {
        message_len: bytes.len(),
        digest: circuit.digest(),
    };
    let mut wrong_digest = correct;
    wrong_digest.digest[0] ^= 1;
    let wrong_length = ZkX509ShaWordStarkStatementV1 {
        message_len: 64,
        ..correct
    };
    let oversized = ZkX509ShaWordStarkStatementV1 {
        message_len: ZK_X509_DER_MAX_DOCUMENT_BYTES_V1 + 1,
        ..correct
    };
    for changed in [wrong_digest, wrong_length, oversized] {
        let (result, builds) = observe_circuit_builds_v1(|| {
            build_sha_word_stark_base_from_circuit_v1(changed, &circuit)
        });
        assert_eq!(result.err(), Some(ZkX509ShaWordStarkErrorV1::Topology));
        assert_eq!(
            builds, 0,
            "reject mismatching context before another allocation"
        );
        assert_eq!(
            reference::base_v1(changed, &bytes).err(),
            Some(ZkX509ShaWordStarkErrorV1::Topology)
        );
        assert_eq!(
            build_sha_word_stark_base_v1(changed, &bytes).err(),
            Some(ZkX509ShaWordStarkErrorV1::Topology)
        );
    }
    assert_eq!(
        circuit.digest(),
        correct.digest,
        "borrowed owner remains intact"
    );
}

fn borrowed_schedule_memory_challenges() -> ZkX509WordMemoryChallengesV1 {
    ZkX509WordMemoryChallengesV1 {
        lanes: core::array::from_fn(|lane| {
            let offset = u64::try_from(lane).unwrap() * 16;
            ZkX509WordMemoryLaneChallengesV1 {
                beta: F(offset + 3),
                address: F(offset + 5),
                value: F(offset + 7),
                is_write: F(offset + 11),
            }
        }),
    }
}

#[test]
fn borrowed_fixed_capacity_matches_fresh_reference_rows_products_and_construction_count() {
    for (length, maximum, exact) in [
        (0, 0, true),
        (0, 127, false),
        (55, 63, false),
        (56, 127, false),
        (63, 127, false),
        (64, 127, false),
        (65, 127, false),
        (127, 127, true),
        (313, 313, true),
        (4_096, 4_096, true),
    ] {
        let bytes = message(length);
        let reference = reference::capacity_v1(&bytes, maximum, exact).unwrap();
        let (fresh, fresh_builds) = observe_circuit_builds_v1(|| {
            build_sha_word_capacity_base_source_v1(&bytes, maximum, exact)
        });
        let fresh = fresh.unwrap();
        assert_capacity_equal(&fresh, &reference);
        drop(reference);
        let fixed = compile_sha_word_capacity_fixed_schedule_v1(maximum, exact).unwrap();
        let payload = fixed.allocated_heap_bytes_v1();
        let (borrowed, borrowed_builds) = observe_circuit_builds_v1(|| {
            build_sha_word_capacity_base_with_fixed_v1(&bytes, maximum, exact, &fixed)
        });
        let borrowed = borrowed.unwrap();
        assert_eq!(fresh_builds, 4);
        assert_eq!(
            borrowed_builds, 2,
            "actual circuit and independent zero-shape remain"
        );
        assert_capacity_equal(&borrowed, &fresh);
        assert_eq!(fixed.allocated_heap_bytes_v1(), payload);
        let fresh_bound = fresh
            .bind_memory_challenges_v1(borrowed_schedule_memory_challenges())
            .unwrap();
        let borrowed_bound = borrowed
            .bind_memory_challenges_v1(borrowed_schedule_memory_challenges())
            .unwrap();
        assert_eq!(borrowed_bound.base_rows, fresh_bound.base_rows);
        assert_eq!(borrowed_bound.fixed_rows, fresh_bound.fixed_rows);
        assert!(
            borrowed_bound.aux_rows == fresh_bound.aux_rows,
            "every bound product and endpoint"
        );
    }
}

#[test]
#[ignore = "full 16 KiB fresh/reference/borrowed capacity parity; run explicitly with one test thread"]
fn borrowed_fixed_full_supported_capacity_matches_every_row_and_column() {
    let bytes = message(ZK_X509_DER_MAX_DOCUMENT_BYTES_V1);
    let reference = reference::capacity_v1(&bytes, bytes.len(), true).unwrap();
    let fresh = build_sha_word_capacity_base_source_v1(&bytes, bytes.len(), true).unwrap();
    assert_capacity_equal(&fresh, &reference);
    drop(reference);
    let fixed = compile_sha_word_capacity_fixed_schedule_v1(bytes.len(), true).unwrap();
    let (borrowed, builds) = observe_circuit_builds_v1(|| {
        build_sha_word_capacity_base_with_fixed_v1(&bytes, bytes.len(), true, &fixed)
    });
    assert_eq!(builds, 2);
    assert_capacity_equal(&borrowed.unwrap(), &fresh);
}

#[test]
fn borrowed_fixed_capacity_rejects_substitutions_before_private_construction() {
    let fixed = compile_sha_word_capacity_fixed_schedule_v1(127, false).unwrap();
    for (length, maximum, exact) in [
        (0, 126, false),
        (127, 127, true),
        (128, 127, false),
        (0, ZK_X509_DER_MAX_DOCUMENT_BYTES_V1 + 1, false),
    ] {
        let bytes = message(length);
        let (result, builds) = observe_circuit_builds_v1(|| {
            build_sha_word_capacity_base_with_fixed_v1(&bytes, maximum, exact, &fixed)
        });
        assert!(result.is_err());
        assert_eq!(builds, 0);
    }
    let exact = compile_sha_word_capacity_fixed_schedule_v1(127, true).unwrap();
    assert!(build_sha_word_capacity_base_with_fixed_v1(b"short", 127, true, &exact).is_err());
}

#[test]
fn borrowed_fixed_capacity_retains_explicit_erasure_and_late_failure_cleanup() {
    use super::super::private_table::inspection::observe_v1;
    let fixed = compile_sha_word_capacity_fixed_schedule_v1(63, false).unwrap();
    let first_fixed = fixed.fixed_row_v1(0).unwrap();
    let mut source = build_sha_word_capacity_base_with_fixed_v1(b"abc", 63, false, &fixed).unwrap();
    source.zeroize_private_v1();
    assert!(source.private_is_zeroized_v1());
    assert_eq!(fixed.fixed_row_v1(0).unwrap(), first_fixed);
    let mut source = build_sha_word_capacity_base_with_fixed_v1(b"abc", 63, false, &fixed).unwrap();
    let expected_cells = source.logical_rows() * SHA_WORD_CAPACITY_AUX_WIDTH_V1;
    source.execution[0].value = source.execution[0].value.add(F::ONE);
    let (result, erasures) =
        observe_v1(|| source.bind_memory_challenges_v1(borrowed_schedule_memory_challenges()));
    assert!(matches!(result, Err(ZkX509ShaWordStarkErrorV1::LocalCopy)));
    assert!(erasures.iter().any(|item| item.cells == expected_cells
        && item.nonzero_before > 0
        && item.nonzero_after == 0));
    assert_eq!(fixed.fixed_row_v1(0).unwrap(), first_fixed);
}
