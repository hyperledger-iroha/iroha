//! Test-only retained SHA3 packets within the existing scalar-row payload charge.

use super::*;
use aggregate::AggregateStarkErrorV1 as Error;

thread_local! {
    static RETAINED_CLEARS: RefCell<Option<Vec<(&'static str, usize, bool, bool)>>> = const { RefCell::new(None) };
}

fn observing_clear() -> bool {
    RETAINED_CLEARS.with(|slot| slot.borrow().is_some())
}

fn observe_clear(kind: &'static str, address: usize, was_nonzero: bool, clear: bool) {
    RETAINED_CLEARS.with(|slot| {
        if let Some(observed) = slot.borrow_mut().as_mut() {
            observed.push((kind, address, was_nonzero, clear));
        }
    });
}

/// Explicit padding fills the complete 864-byte, 32-byte-aligned allocation slot.
/// This is an ephemeral test layout, never a serialized or protocol format.
#[repr(C, align(32))]
struct Packet {
    lanes: [[u64; 4]; 25],
    remaining: [usize; 4],
    cursor: [u8; 4],
    padding: [u8; 28],
}
impl Packet {
    fn zeroed() -> Self {
        Self {
            lanes: [[0; 4]; 25],
            remaining: [0; 4],
            cursor: [0; 4],
            padding: [0; 28],
        }
    }
    fn clear(&mut self) {
        self.lanes.zeroize();
        self.remaining.zeroize();
        self.cursor.zeroize();
        self.padding.zeroize();
    }
    fn is_clear(&self) -> bool {
        self.lanes.iter().flatten().all(|&value| value == 0)
            && self.remaining.iter().all(|&value| value == 0)
            && self.cursor.iter().all(|&value| value == 0)
            && self.padding.iter().all(|&value| value == 0)
    }
    fn import(&mut self, lane: usize, stream: &PrivacyOuterLastFieldStreamV1) {
        let serialized = stream.state.state_snapshot_v1();
        assert!(usize::from(serialized[200]) < RATE);
        for word in 0..25 {
            self.lanes[word][lane] =
                u64::from_le_bytes(serialized[word * 8..word * 8 + 8].try_into().unwrap());
        }
        self.cursor[lane] = serialized[200];
        self.remaining[lane] = stream.remaining;
    }
    fn snapshot(&self, lane: usize) -> zeroize::Zeroizing<SerializedState<Sha3_384>> {
        let mut serialized = zeroize::Zeroizing::new(SerializedState::<Sha3_384>::default());
        for word in 0..25 {
            serialized[word * 8..word * 8 + 8]
                .copy_from_slice(&self.lanes[word][lane].to_le_bytes());
        }
        serialized[200] = self.cursor[lane];
        serialized
    }
    fn admits(&self, bytes: usize) -> bool {
        self.remaining.iter().all(|&remaining| {
            remaining != usize::MAX && remaining >= bytes && remaining == self.remaining[0]
        }) && self
            .cursor
            .iter()
            .all(|&cursor| usize::from(cursor) < RATE && cursor == self.cursor[0])
    }
    /// Called only after whole-collection validation and runtime AVX2 admission.
    fn absorb(
        &mut self,
        packed: &[[u8; MAX_UPDATE]; 4],
        bytes: usize,
        temporary: &mut [[u64; 4]; 5],
    ) -> usize {
        let mut cursor = usize::from(self.cursor[0]);
        let mut consumed = 0;
        let mut permutations = 0;
        while consumed < bytes {
            let count = (RATE - cursor).min(bytes - consumed);
            let prefix = count.min((8 - cursor % 8) % 8);
            for (lane, input) in packed.iter().enumerate() {
                for offset in 0..prefix {
                    let position = cursor + offset;
                    self.lanes[position / 8][lane] ^=
                        u64::from(input[consumed + offset]) << ((position % 8) * 8);
                }
                // Sponge words are aligned; the canonical packed input need
                // not be. Decode its existing bytes without changing framing.
                let mut offset = prefix;
                while count - offset >= 8 {
                    let position = cursor + offset;
                    let first = consumed + offset;
                    self.lanes[position / 8][lane] ^=
                        u64::from_le_bytes(input[first..first + 8].try_into().unwrap());
                    offset += 8;
                }
                for offset in offset..count {
                    let position = cursor + offset;
                    self.lanes[position / 8][lane] ^=
                        u64::from(input[consumed + offset]) << ((position % 8) * 8);
                }
            }
            cursor += count;
            consumed += count;
            if cursor == RATE {
                // SAFETY: construction admits this owner only on AVX2 hosts;
                // all vector windows are complete four-word arrays.
                unsafe { permute_lanes(&mut self.lanes, temporary) };
                permutations += 1;
                cursor = 0;
            }
        }
        for lane in 0..4 {
            self.cursor[lane] = u8::try_from(cursor).unwrap();
            self.remaining[lane] -= bytes;
        }
        permutations
    }
    fn finalize_row(&mut self, lane: usize) -> Result<PrivacyOuterDigestV1, Error> {
        if self.remaining[lane] != 0 {
            return Err(Error::InvalidLayout);
        }
        let serialized = self.snapshot(lane);
        let staged =
            BoundaryState(Sha3_384::deserialize(&serialized).map_err(|_| Error::InvalidLayout)?);
        let mut stream = empty_stream();
        // The staged state keeps its original slot until its clearing Drop.
        stream.state.inner = staged.0.clone();
        let digest = stream
            .finalize_in_place_v1()
            .map_err(|_| Error::InvalidLayout)?;
        for word in &mut self.lanes {
            word[lane].zeroize();
        }
        self.cursor[lane] = 0;
        self.remaining[lane] = usize::MAX;
        Ok(digest)
    }
}
impl Drop for Packet {
    fn drop(&mut self) {
        let was_nonzero = observing_clear() && !self.is_clear();
        self.clear();
        observe_clear(
            "packet",
            core::ptr::from_ref(self).addr(),
            was_nonzero,
            self.is_clear(),
        );
    }
}

/// A bounded typed boundary replacement cannot be moved out before erasure.
struct BoundaryState(Sha3_384);
impl Drop for BoundaryState {
    fn drop(&mut self) {
        let was_nonzero = observing_clear()
            && zeroize::Zeroizing::new(self.0.serialize())
                .iter()
                .any(|&value| value != 0);
        self.0 = Sha3_384::default();
        if observing_clear() {
            let serialized = zeroize::Zeroizing::new(self.0.serialize());
            observe_clear(
                "boundary",
                core::ptr::from_ref(self).addr(),
                was_nonzero,
                serialized.iter().all(|&value| value == 0),
            );
        }
    }
}

fn empty_stream() -> PrivacyOuterLastFieldStreamV1 {
    PrivacyOuterLastFieldStreamV1 {
        state: WipingSha3V1::default(),
        remaining: 0,
    }
}

/// Packet and actual scalar-tail allocations replace, never mirror, row storage.
struct Rows {
    packets: Vec<Packet>,
    scalar: Vec<PrivacyOuterLastFieldStreamV1>,
    rows: usize,
    width: usize,
    received: usize,
    failed: bool,
    bootstrap_conversions: usize,
    finalize_conversions: usize,
}
impl Rows {
    fn new(
        rows: usize,
        width: usize,
        leaf: &[u8],
        vector: bool,
        limit: usize,
    ) -> Result<Self, Error> {
        Self::new_with_checkpoint(rows, width, leaf, vector, limit, |_| Ok(()))
    }
    fn new_with_checkpoint(
        rows: usize,
        width: usize,
        leaf: &[u8],
        vector: bool,
        limit: usize,
        mut checkpoint: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let invalid = Error::InvalidLayout;
        let row_charge = rows
            .checked_mul(core::mem::size_of::<PrivacyOuterLastFieldStreamV1>())
            .ok_or(invalid)?;
        let width_u16 = u16::try_from(width).map_err(|_| invalid)?;
        let value_bytes = width.checked_mul(8).ok_or(invalid)?;
        if rows == 0 || rows > 1 << 22 || width == 0 || leaf.is_empty() || row_charge > limit {
            return Err(invalid);
        }
        let native = vector && std::is_x86_feature_detected!("avx2");
        let packet_count = if native { rows / 4 } else { 0 };
        let scalar_count = rows - packet_count * 4;
        let packet_bytes = packet_count
            .checked_mul(core::mem::size_of::<Packet>())
            .ok_or(invalid)?;
        let scalar_bytes = scalar_count
            .checked_mul(core::mem::size_of::<PrivacyOuterLastFieldStreamV1>())
            .ok_or(invalid)?;
        if packet_bytes.checked_add(scalar_bytes).ok_or(invalid)? > row_charge {
            return Err(invalid);
        }
        let mut owner = Self {
            packets: Vec::new(),
            scalar: Vec::new(),
            rows,
            width,
            received: 0,
            failed: false,
            bootstrap_conversions: 0,
            finalize_conversions: 0,
        };
        owner
            .packets
            .try_reserve_exact(packet_count)
            .map_err(|_| Error::AllocationFailure)?;
        owner
            .scalar
            .try_reserve_exact(scalar_count)
            .map_err(|_| Error::AllocationFailure)?;
        if owner.packets.capacity() != packet_count || owner.scalar.capacity() != scalar_count {
            return Err(Error::AllocationFailure);
        }
        // Only public zero values move into final storage. Secret state is filled
        // through these retained slots; neither allocation grows afterward.
        owner.packets.resize_with(packet_count, Packet::zeroed);
        owner.scalar.resize_with(scalar_count, empty_stream);
        let catalog = TransparentStarkDigestContextV1::catalog_v1();
        let domain = row_context()
            .domain_v1(&catalog, leaf, b"vector-row-leaf", 0, 0, 0)
            .map_err(|_| invalid)?;
        let prefix = PrivacyOuterDomainPrefixV1::new(domain).ok_or(invalid)?;
        let group = u16::MAX.to_be_bytes();
        let width_bytes = width_u16.to_be_bytes();
        for row in 0..rows {
            let original = prefix
                .last_field_stream_at_with_counter(
                    row as u64,
                    u64::from(u16::MAX),
                    &[&group, &width_bytes],
                    value_bytes,
                )
                .map_err(|_| invalid)?;
            if row < packet_count * 4 {
                owner.packets[row / 4].import(row % 4, &original);
                owner.bootstrap_conversions += 1;
            } else {
                let destination = &mut owner.scalar[row - packet_count * 4];
                destination.state.inner = original.state.inner.clone();
                destination.remaining = original.remaining;
            }
            checkpoint(row)?;
        }
        if owner.packets.iter().any(|packet| !packet.admits(0)) {
            return Err(invalid);
        }
        Ok(owner)
    }
    fn payload_bytes(&self) -> usize {
        self.packets.capacity() * core::mem::size_of::<Packet>()
            + self.scalar.capacity() * core::mem::size_of::<PrivacyOuterLastFieldStreamV1>()
    }
    fn absorb(&mut self, columns: &[Vec<F>]) -> Result<usize, Error> {
        self.absorb_with_checkpoint(columns, || Ok(()))
    }
    fn absorb_with_checkpoint(
        &mut self,
        columns: &[Vec<F>],
        checkpoint: impl FnOnce() -> Result<(), Error>,
    ) -> Result<usize, Error> {
        if self.failed
            || columns.is_empty()
            || columns.len() > 8
            || self
                .received
                .checked_add(columns.len())
                .is_none_or(|end| end > self.width)
            || columns.iter().any(|column| column.len() != self.rows)
        {
            return Err(Error::InvalidLayout);
        }
        if columns
            .iter()
            .flatten()
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(Error::NonCanonicalField);
        }
        let bytes = columns.len() * 8;
        if self.packets.iter().any(|packet| !packet.admits(bytes))
            || self
                .scalar
                .iter()
                .any(|stream| stream.remaining == usize::MAX || stream.remaining < bytes)
        {
            return Err(Error::InvalidLayout);
        }
        // Expected preflight refusals are retryable. Any error or unwind after
        // this point leaves the actual retained collection poisoned until Drop.
        self.failed = true;
        let permutations: usize = self
            .packets
            .par_chunks_mut(256)
            .enumerate()
            .map(|(chunk, packets)| {
                let mut packed = zeroize::Zeroizing::new([[0u8; MAX_UPDATE]; 4]);
                let mut temporary = zeroize::Zeroizing::new([[0u64; 4]; 5]);
                let mut permutations = 0;
                for (offset, packet) in packets.iter_mut().enumerate() {
                    for lane in 0..4 {
                        let row = (chunk * 256 + offset) * 4 + lane;
                        for (column, values) in columns.iter().enumerate() {
                            packed[lane][column * 8..column * 8 + 8]
                                .copy_from_slice(&values[row].0.to_be_bytes());
                        }
                    }
                    permutations += packet.absorb(&packed, bytes, &mut temporary);
                }
                permutations
            })
            .sum();
        checkpoint()?;
        let scalar_start = self.packets.len() * 4;
        // The full scalar fallback retains the actual 1024-row scheduling and
        // the tails read their global row indices, never a rebased column slice.
        self.scalar
            .par_chunks_mut(1024)
            .enumerate()
            .try_for_each(|(chunk, streams)| {
                let mut packed = zeroize::Zeroizing::new([0u8; MAX_UPDATE]);
                for (offset, stream) in streams.iter_mut().enumerate() {
                    let row = scalar_start + chunk * 1024 + offset;
                    for (column, values) in columns.iter().enumerate() {
                        packed[column * 8..column * 8 + 8]
                            .copy_from_slice(&values[row].0.to_be_bytes());
                    }
                    stream
                        .update(&packed[..bytes])
                        .map_err(|_| Error::InvalidLayout)?;
                }
                Ok::<(), Error>(())
            })?;
        self.received += columns.len();
        self.failed = false;
        Ok(permutations)
    }
    fn clear_retained(&mut self) {
        for packet in &mut self.packets {
            let was_nonzero = observing_clear() && !packet.is_clear();
            packet.clear();
            observe_clear(
                "failed_packet",
                core::ptr::from_ref(packet).addr(),
                was_nonzero,
                packet.is_clear(),
            );
        }
        for stream in &mut self.scalar {
            let was_nonzero = observing_clear()
                && stream
                    .state
                    .state_snapshot_v1()
                    .iter()
                    .any(|&byte| byte != 0);
            stream.state.wipe_state_v1();
            stream.remaining = usize::MAX;
            if observing_clear() {
                observe_clear(
                    "failed_scalar",
                    core::ptr::from_ref(stream).addr(),
                    was_nonzero,
                    stream
                        .state
                        .state_snapshot_v1()
                        .iter()
                        .all(|&byte| byte == 0),
                );
            }
        }
    }
    fn finish(
        &mut self,
        node: &'static [u8],
        queries: &[usize],
    ) -> Result<aggregate::StreamingMerkleCommitmentV1, Error> {
        let invalid = self.failed || self.received != self.width;
        self.failed = true;
        if invalid {
            self.clear_retained();
            return Err(Error::InvalidLayout);
        }
        let native_rows = self.packets.len() * 4;
        let result = aggregate::streaming_merkle_commitment_v1(
            row_context(),
            node,
            self.rows,
            queries,
            (0..self.rows).map(|row| {
                if row < native_rows {
                    let digest = self.packets[row / 4].finalize_row(row % 4)?;
                    self.finalize_conversions += 1;
                    Ok(digest)
                } else {
                    self.scalar[row - native_rows]
                        .finalize_in_place_v1()
                        .map_err(|_| Error::InvalidLayout)
                }
            }),
        );
        if result.is_err() {
            self.clear_retained();
        }
        result
    }
}
impl Drop for Rows {
    fn drop(&mut self) {
        for stream in &mut self.scalar {
            let was_nonzero = observing_clear()
                && stream
                    .state
                    .state_snapshot_v1()
                    .iter()
                    .any(|&byte| byte != 0);
            stream.state.wipe_state_v1();
            stream.remaining = 0;
            if observing_clear() {
                observe_clear(
                    "scalar",
                    core::ptr::from_ref(stream).addr(),
                    was_nonzero,
                    stream
                        .state
                        .state_snapshot_v1()
                        .iter()
                        .all(|&byte| byte == 0),
                );
            }
        }
        // Packet Drop and the scalar owners' own Drop run in their real Vec slots.
    }
}

fn funded_bytes(rows: usize) -> usize {
    rows.checked_mul(core::mem::size_of::<PrivacyOuterLastFieldStreamV1>())
        .unwrap()
}

fn assert_row_state(owner: &Rows, reference: &[PrivacyOuterLastFieldStreamV1]) {
    assert_eq!(owner.rows, reference.len());
    let native_rows = owner.packets.len() * 4;
    for (row, expected) in reference.iter().enumerate() {
        if row < native_rows {
            let packet = &owner.packets[row / 4];
            assert_eq!(packet.remaining[row % 4], expected.remaining);
            assert_eq!(
                &*packet.snapshot(row % 4),
                &*expected.state.state_snapshot_v1()
            );
        } else {
            let actual = &owner.scalar[row - native_rows];
            assert_eq!(actual.remaining, expected.remaining);
            assert_eq!(
                &*actual.state.state_snapshot_v1(),
                &*expected.state.state_snapshot_v1()
            );
        }
    }
}

#[test]
#[ignore = "mandatory persistent owner layout, exact funding and original-slot clearing"]
fn required_avx2_persistent_layout_funding_and_clearing() {
    assert!(std::is_x86_feature_detected!("avx2"));
    assert_eq!(core::mem::size_of::<PrivacyOuterLastFieldStreamV1>(), 216);
    assert_eq!(core::mem::size_of::<Packet>(), 864);
    assert_eq!(core::mem::align_of::<Packet>(), 32);
    assert_eq!(core::mem::offset_of!(Packet, lanes), 0);
    assert_eq!(core::mem::offset_of!(Packet, remaining), 800);
    assert_eq!(core::mem::offset_of!(Packet, cursor), 832);
    assert_eq!(core::mem::offset_of!(Packet, padding), 836);
    let (groups, full_rows, stream_limit, _, runtime_limit) =
        crate::privacy_engines::zk_x509::stark::ZkX509MainRegistrationShapeV1::row_hash_diagnostic_geometry_v1().unwrap();
    assert_eq!(funded_bytes(full_rows), stream_limit);
    assert_eq!(runtime_limit, 1 << 30);
    // Formula coverage of actual registered powers does not allocate full rows.
    for rows in groups
        .iter()
        .map(|&(log, _, _)| 1usize << log)
        .chain([full_rows])
    {
        let packet_bytes = (rows / 4)
            .checked_mul(core::mem::size_of::<Packet>())
            .unwrap();
        let tail_bytes = (rows % 4)
            .checked_mul(core::mem::size_of::<PrivacyOuterLastFieldStreamV1>())
            .unwrap();
        assert_eq!(
            packet_bytes.checked_add(tail_bytes).unwrap(),
            funded_bytes(rows)
        );
        assert!(funded_bytes(rows) <= stream_limit);
    }
    for rows in (1..=9).chain([64, 1024]) {
        for vector in [false, true] {
            let mut owner = Rows::new(rows, 8, BASE_LEAF, vector, funded_bytes(rows)).unwrap();
            assert_eq!(owner.payload_bytes(), funded_bytes(rows));
            assert_eq!(owner.packets.capacity(), if vector { rows / 4 } else { 0 });
            assert_eq!(
                owner.scalar.capacity(),
                if vector { rows % 4 } else { rows }
            );
            assert_eq!(owner.bootstrap_conversions, owner.packets.len() * 4);
            assert!(owner.packets.iter().all(|packet| packet.padding == [0; 28]));
            assert!(
                owner
                    .packets
                    .iter()
                    .all(|packet| (core::ptr::from_ref(packet).addr()).is_multiple_of(32))
            );
            let mut slots: Vec<_> = owner
                .packets
                .iter()
                .map(|packet| ("packet", core::ptr::from_ref(packet).addr()))
                .chain(
                    owner
                        .scalar
                        .iter()
                        .map(|stream| ("scalar", core::ptr::from_ref(stream).addr())),
                )
                .collect();
            slots.sort_unstable();
            if let Some(packet) = owner.packets.first_mut() {
                packet.padding.fill(0x9b);
            }
            let columns = field_columns(rows, 0, 8);
            let permutations = owner.absorb(&columns).unwrap();
            assert_eq!(permutations, if vector { rows / 4 } else { 0 });
            RETAINED_CLEARS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
            drop(owner);
            let observed = RETAINED_CLEARS.with(|slot| slot.borrow_mut().take().unwrap());
            assert!(observed.iter().all(|entry| entry.2 && entry.3));
            let mut actual: Vec<_> = observed.iter().map(|entry| (entry.0, entry.1)).collect();
            actual.sort_unstable();
            assert_eq!(
                actual, slots,
                "clearing must observe the original retained slots"
            );
            assert!(Rows::new(rows, 8, BASE_LEAF, vector, funded_bytes(rows) - 1).is_err());
        }
    }
    assert!(Rows::new(usize::MAX, 8, BASE_LEAF, true, usize::MAX).is_err());
    assert!(Rows::new(0, 8, BASE_LEAF, true, 0).is_err());
    assert!(Rows::new(4, 0, BASE_LEAF, true, funded_bytes(4)).is_err());
    assert!(
        Rows::new(
            4,
            usize::from(u16::MAX) + 1,
            BASE_LEAF,
            true,
            funded_bytes(4)
        )
        .is_err()
    );
    for panic in [false, true] {
        RETAINED_CLEARS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
        let result = std::panic::catch_unwind(|| {
            Rows::new_with_checkpoint(9, 8, BASE_LEAF, true, funded_bytes(9), |row| {
                if row == 4 {
                    if panic {
                        panic!("injected persistent bootstrap unwind");
                    }
                    return Err(Error::InvalidLayout);
                }
                Ok(())
            })
        });
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        let observed = RETAINED_CLEARS.with(|slot| slot.borrow_mut().take().unwrap());
        assert_eq!(
            observed.iter().filter(|entry| entry.0 == "packet").count(),
            2
        );
        assert_eq!(
            observed.iter().filter(|entry| entry.0 == "scalar").count(),
            1
        );
        assert!(observed.iter().all(|entry| entry.3));
        assert!(observed.iter().any(|entry| entry.2));
    }
    println!(
        "sha3_persistent_layout packet_bytes=864 alignment=32 padding_bytes=28 row_charge=216 exact_tail_charge=true allocated_rows=1..9,64,1024 capacity_formula=registered_powers,common_rows clearing=original_slots,partial_init,error,unwind"
    );
}

#[test]
#[ignore = "mandatory persistent four-state rate, padding, seal and scalar-tail parity"]
fn required_avx2_persistent_all_rates_and_global_tail_rows() {
    assert!(std::is_x86_feature_detected!("avx2"));
    let mut first_permutations = 0;
    let mut total_permutations = 0;
    let mut expected_permutations = 0;
    for cursor in 0..RATE {
        for length in 0..=MAX_UPDATE {
            let mut reference = streams(4, 512);
            let position = usize::from(reference[0].state.state_snapshot_v1()[200]);
            let advance = (cursor + RATE - position) % RATE;
            for (row, stream) in reference.iter_mut().enumerate() {
                stream.update(&vec![row as u8 + 1; advance]).unwrap();
            }
            let mut packet = Packet::zeroed();
            for (row, stream) in reference.iter().enumerate() {
                packet.import(row, stream);
            }
            expected_permutations += (cursor + packet.remaining[0]) / RATE;
            let mut packed = zeroize::Zeroizing::new([[0u8; MAX_UPDATE]; 4]);
            for row in 0..4 {
                for byte in 0..MAX_UPDATE {
                    packed[row][byte] = (row * 67 + byte * 19) as u8;
                }
            }
            let mut temporary = zeroize::Zeroizing::new([[0u64; 4]; 5]);
            assert!(packet.admits(length));
            let count = packet.absorb(&packed, length, &mut temporary);
            assert_eq!(count, usize::from(cursor + length >= RATE));
            first_permutations += count;
            total_permutations += count;
            for row in 0..4 {
                reference[row].update(&packed[row][..length]).unwrap();
                assert_eq!(
                    &*packet.snapshot(row),
                    &*reference[row].state.state_snapshot_v1()
                );
                assert_eq!(packet.remaining[row], reference[row].remaining);
            }
            while packet.remaining[0] != 0 {
                let count = packet.remaining[0].min(MAX_UPDATE);
                total_permutations += packet.absorb(&packed, count, &mut temporary);
                for row in 0..4 {
                    reference[row].update(&packed[row][..count]).unwrap();
                }
            }
            for row in 0..4 {
                assert_eq!(
                    packet.finalize_row(row).unwrap(),
                    reference[row].finalize_in_place_v1().unwrap()
                );
                assert_eq!(packet.remaining[row], usize::MAX);
                assert!(packet.lanes.iter().all(|word| word[row] == 0));
                assert_eq!(packet.cursor[row], 0);
                assert!(
                    !packet.admits(0),
                    "one sealed lane seals the packet to further updates"
                );
                assert!(packet.finalize_row(row).is_err());
            }
        }
    }
    assert_eq!(first_permutations, 2080);
    assert_eq!(total_permutations, expected_permutations);
    for rows in 1..=9 {
        for width in [1, 2, 7, 8, 9, 13, 31] {
            let expected = {
                let mut reference = row_streams(rows, width, BASE_LEAF);
                for first in (0..width).step_by(8) {
                    let columns = field_columns(rows, first, (width - first).min(8));
                    absorb_fields(&mut reference, &columns, false).unwrap();
                }
                reference
                    .iter_mut()
                    .map(|stream| stream.finalize_in_place_v1().unwrap())
                    .collect::<Vec<_>>()
            }; // No scalar row-state matrix coexists with the retained candidate.
            for vector in [false, true] {
                let mut owner =
                    Rows::new(rows, width, BASE_LEAF, vector, funded_bytes(rows)).unwrap();
                for first in (0..width).step_by(8) {
                    let columns = field_columns(rows, first, (width - first).min(8));
                    let count = owner.absorb(&columns).unwrap();
                    if !vector || rows < 4 {
                        assert_eq!(count, 0);
                    }
                }
                let native_rows = owner.packets.len() * 4;
                for (row, expected) in expected.iter().enumerate() {
                    let actual = if row < native_rows {
                        owner.packets[row / 4].finalize_row(row % 4).unwrap()
                    } else {
                        owner.scalar[row - native_rows]
                            .finalize_in_place_v1()
                            .unwrap()
                    };
                    assert_eq!(
                        &actual, expected,
                        "global row counter {row} with {rows} rows"
                    );
                }
                assert!(owner.absorb(&field_columns(rows, 0, 1)).is_err());
            }
        }
    }
    println!(
        "sha3_persistent_rate first_update_four_state_permutations={first_permutations} total_four_state_permutations={total_permutations} all_cursors=true tails=1,2,3 global_row_counters=exact seal=per_row"
    );
}

#[test]
#[ignore = "mandatory persistent canonical roots, retryable preflight and poisoned-owner controls"]
fn required_avx2_persistent_roots_refusals_and_poisoned_cleanup() {
    assert!(std::is_x86_feature_detected!("avx2"));
    let mut permutations = 0;
    for (leaf, node) in [(BASE_LEAF, BASE_NODE), (AUX_LEAF, AUX_NODE)] {
        for rows in [1, 2, 4, 8, 64, 2048] {
            for width in [1, 2, 7, 8, 9, 13, 31] {
                let mut queries = vec![0, rows / 2, rows - 1];
                queries.sort_unstable();
                queries.dedup();
                let expected = {
                    let mut reference = aggregate::StreamingRowCommitmentV1::new(
                        row_context(),
                        leaf,
                        node,
                        usize::from(u16::MAX),
                        rows,
                        width,
                        &queries,
                    )
                    .unwrap();
                    for first in (0..width).step_by(8) {
                        reference
                            .absorb_columns_v1(&field_columns(rows, first, (width - first).min(8)))
                            .unwrap();
                    }
                    reference.finish().unwrap().commitment
                };
                let mut owner = Rows::new(rows, width, leaf, true, funded_bytes(rows)).unwrap();
                for first in (0..width).step_by(8) {
                    permutations += owner
                        .absorb(&field_columns(rows, first, (width - first).min(8)))
                        .unwrap();
                }
                assert_eq!(owner.finish(node, &queries).unwrap(), expected);
                assert_eq!(owner.bootstrap_conversions, rows / 4 * 4);
                assert_eq!(owner.finalize_conversions, rows / 4 * 4);
                assert!(owner.finish(node, &queries).is_err());
                assert!(
                    owner.packets.iter().all(|packet| packet
                        .lanes
                        .iter()
                        .flatten()
                        .all(|&value| value == 0))
                );
            }
        }
    }
    for bad in 0..4 {
        let mut owner = Rows::new(4, 8, BASE_LEAF, true, funded_bytes(4)).unwrap();
        let reference = row_streams(4, 8, BASE_LEAF); // One bounded group is the refusal oracle.
        let mut columns = field_columns(4, 0, 8);
        match bad {
            0 => {
                columns[7][3] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1)
            }
            1 => {
                columns[7].pop();
            }
            2 => {
                for column in &mut *columns {
                    column.zeroize();
                }
                columns.clear();
            }
            _ => columns.push(vec![F::ONE; 4]),
        }
        assert!(owner.absorb(&columns).is_err());
        assert!(!owner.failed);
        assert_row_state(&owner, &reference);
        drop(reference);
        owner.absorb(&field_columns(4, 0, 8)).unwrap();
        owner.finish(BASE_NODE, &[]).unwrap();
    }
    for malformed in 0..4 {
        let mut owner = Rows::new(4, 8, BASE_LEAF, true, funded_bytes(4)).unwrap();
        let cursor = owner.packets[0].cursor;
        let remaining = owner.packets[0].remaining;
        match malformed {
            0 => owner.packets[0].cursor[1] = ((usize::from(cursor[1]) + 1) % RATE) as u8,
            1 => owner.packets[0].cursor[1] = RATE as u8,
            2 => owner.packets[0].remaining[1] = usize::MAX,
            _ => owner.packets[0].remaining[1] -= 1,
        }
        let mutated_cursor = owner.packets[0].cursor;
        let mutated_remaining = owner.packets[0].remaining;
        assert!(owner.absorb(&field_columns(4, 0, 8)).is_err());
        assert!(!owner.failed);
        assert_eq!(owner.packets[0].cursor, mutated_cursor);
        assert_eq!(owner.packets[0].remaining, mutated_remaining);
        owner.packets[0].cursor = cursor;
        owner.packets[0].remaining = remaining;
        owner.absorb(&field_columns(4, 0, 8)).unwrap();
        owner.finish(BASE_NODE, &[]).unwrap();
    }
    for (rows, queries) in [(5, vec![]), (4, vec![4]), (4, vec![0, 0]), (4, vec![2, 1])] {
        let mut owner = Rows::new(rows, 1, BASE_LEAF, true, funded_bytes(rows)).unwrap();
        owner.absorb(&field_columns(rows, 0, 1)).unwrap();
        assert!(owner.finish(BASE_NODE, &queries).is_err());
        assert!(owner.failed);
        assert!(owner.packets.iter().all(Packet::is_clear));
        assert!(owner.scalar.iter().all(|stream| {
            stream
                .state
                .state_snapshot_v1()
                .iter()
                .all(|&byte| byte == 0)
        }));
        assert!(owner.absorb(&field_columns(rows, 0, 1)).is_err());
        assert!(owner.finish(BASE_NODE, &[]).is_err());
    }
    for panic in [false, true] {
        let mut owner = Rows::new(9, 8, BASE_LEAF, true, funded_bytes(9)).unwrap();
        let columns = field_columns(9, 0, 8);
        RETAINED_CLEARS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.absorb_with_checkpoint(&columns, || {
                if panic {
                    panic!("injected persistent post-mutation unwind");
                }
                Err(Error::InvalidLayout)
            })
        }));
        if panic {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().is_err());
        }
        assert!(owner.failed);
        assert!(owner.absorb(&columns).is_err());
        assert!(owner.finish(BASE_NODE, &[]).is_err());
        drop(owner);
        let observed = RETAINED_CLEARS.with(|slot| slot.borrow_mut().take().unwrap());
        assert_eq!(
            observed.iter().filter(|entry| entry.0 == "packet").count(),
            2
        );
        assert_eq!(
            observed.iter().filter(|entry| entry.0 == "scalar").count(),
            1
        );
        assert!(observed.iter().all(|entry| entry.3));
        assert!(
            observed
                .iter()
                .any(|entry| entry.0 == "failed_packet" && entry.2)
        );
    }
    {
        let mut owner = Rows::new(5, 1, BASE_LEAF, true, funded_bytes(5)).unwrap();
        assert!(owner.finish(BASE_NODE, &[]).is_err());
        assert!(owner.failed);
        assert!(owner.packets.iter().all(Packet::is_clear));
        assert!(owner.scalar.iter().all(|stream| {
            stream
                .state
                .state_snapshot_v1()
                .iter()
                .all(|&byte| byte == 0)
        }));
        assert!(owner.absorb(&field_columns(5, 0, 1)).is_err());
        assert!(owner.finish(BASE_NODE, &[]).is_err());
    }
    RETAINED_CLEARS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
    {
        let mut owner = Rows::new(4, 1, BASE_LEAF, true, funded_bytes(4)).unwrap();
        owner.absorb(&field_columns(4, 0, 1)).unwrap();
        owner.finish(BASE_NODE, &[]).unwrap();
    }
    let observed = RETAINED_CLEARS.with(|slot| slot.borrow_mut().take().unwrap());
    assert_eq!(
        observed
            .iter()
            .filter(|entry| entry.0 == "boundary")
            .count(),
        4
    );
    assert!(observed.iter().all(|entry| entry.3));
    assert!(permutations > 0);
    println!(
        "sha3_persistent_owner_control four_state_permutations={permutations} root_frontier=exact preflight=retryable post_mutation=poisoned clearing=success,error,unwind"
    );
}

#[test]
#[ignore = "paired persistent SHA3 owner cost; requires matched optimized crypto dependencies"]
fn persistent_rows_report_paired_owner_cost() {
    use std::time::Instant;
    assert!(std::is_x86_feature_detected!("avx2"));
    // Keep this runtime refusal compilable in ordinary debug test harnesses.
    assert!(!std::hint::black_box(cfg!(debug_assertions)));
    let (groups, full_rows, stream_limit, replay_limit, runtime_limit) =
        crate::privacy_engines::zk_x509::stark::ZkX509MainRegistrationShapeV1::row_hash_diagnostic_geometry_v1().unwrap();
    let shapes = [
        (3213usize, BASE_LEAF, BASE_NODE),
        (2618usize, AUX_LEAF, AUX_NODE),
    ];
    let batches: [Vec<core::ops::Range<usize>>; 2] = core::array::from_fn(|kind| {
        let mut offset = 0;
        let mut batches = Vec::new();
        for &(_, base, aux) in &groups {
            let width = if kind == 0 { base } else { aux };
            for first in (0..width).step_by(8) {
                batches.push(offset + first..offset + (first + 8).min(width));
            }
            offset += width;
        }
        assert_eq!(offset, shapes[kind].0);
        batches
    });
    assert_eq!([batches[0].len(), batches[1].len()], [404, 329]);
    let named_worker =
        core::mem::size_of::<[[u8; MAX_UPDATE]; 4]>() + core::mem::size_of::<[[u64; 4]; 5]>();
    assert_eq!(named_worker, 416);
    println!(
        "sha3_persistent_cost_header expected_records=48 workers=8,16 rows=4096,262144 rounds=3 canonical_framing=true native_group_boundaries=true scope=root_only_absorption primary_metric=absorb_ns input_generation_and_column_clear_timed=false candidate_leaf_finalize=serial current_leaf_finalize=parallel conversion=bootstrap,finalize named_update_worker_bytes={named_worker} named_only_not_stack_bound=true packet_bytes={} packet_alignment={} groups={groups:?} full_rows={full_rows} stream_limit={stream_limit} replay_limit={replay_limit} runtime_limit={runtime_limit} wholeproof_claim=false",
        core::mem::size_of::<Packet>(),
        core::mem::align_of::<Packet>()
    );
    let mut records = 0;
    let mut total_permutations = 0;
    let mut total_bootstrap = 0;
    let mut total_finalize = 0;
    for workers in [8, 16] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            for rows in [4096usize, 1 << 18] {
                let row_payload = funded_bytes(rows);
                let column_payload = rows * 8 * core::mem::size_of::<F>();
                assert!(rows <= full_rows && row_payload <= stream_limit);
                assert!(column_payload + 8 * core::mem::size_of::<Vec<F>>() <= replay_limit);
                assert!(workers * named_worker <= runtime_limit);
                for (kind, (width, leaf, node)) in shapes.into_iter().enumerate() {
                    let mut expected_root = None;
                    for round in 0..3 {
                        for vector in if round % 2 == 0 { [false, true] } else { [true, false] } {
                            let initial = Instant::now();
                            let mut current = (!vector).then(|| aggregate::StreamingRowCommitmentV1::new(row_context(), leaf, node, usize::from(u16::MAX), rows, width, &[]).unwrap());
                            let mut candidate = vector.then(|| Rows::new(rows, width, leaf, true, row_payload).unwrap());
                            let init_ns = initial.elapsed().as_nanos();
                            if let Some(owner) = &candidate {
                                assert_eq!(owner.payload_bytes(), row_payload);
                                assert!(owner.scalar.is_empty());
                                assert_eq!(owner.bootstrap_conversions, rows);
                            }
                            let mut absorb_ns = 0;
                            let mut permutations = 0;
                            for batch in &batches[kind] {
                                let columns = field_columns(rows, batch.start, batch.len());
                                assert!(columns.capacity() <= 8);
                                assert!(columns.iter().all(|column| column.capacity() == rows));
                                let start = Instant::now();
                                if let Some(builder) = &mut current { builder.absorb_columns_v1(&columns).unwrap(); }
                                else { permutations += candidate.as_mut().unwrap().absorb(&columns).unwrap(); }
                                absorb_ns += start.elapsed().as_nanos();
                            }
                            let finalizing = Instant::now();
                            let root = if let Some(builder) = current { builder.finish().unwrap().commitment.root }
                                else { candidate.as_mut().unwrap().finish(node, &[]).unwrap().root };
                            let finalize_ns = finalizing.elapsed().as_nanos();
                            let (bootstrap_conversions, finalize_conversions) = candidate.as_ref().map_or((0, 0), |owner| (owner.bootstrap_conversions, owner.finalize_conversions));
                            assert_eq!(bootstrap_conversions, if vector { rows } else { 0 });
                            assert_eq!(finalize_conversions, if vector { rows } else { 0 });
                            let clearing = Instant::now(); drop(candidate);
                            let final_drop_ns = clearing.elapsed().as_nanos();
                            let total_ns = init_ns + absorb_ns + finalize_ns + final_drop_ns;
                            if let Some(expected) = expected_root { assert_eq!(root, expected); } else { expected_root = Some(root); }
                            assert_eq!(permutations > 0, vector);
                            records += 1; total_permutations += permutations;
                            total_bootstrap += bootstrap_conversions; total_finalize += finalize_conversions;
                            println!("sha3_persistent_cost workers={workers} rows={rows} width={width} round={round} vector={vector} four_state_permutations={permutations} bootstrap_conversions={bootstrap_conversions} finalize_conversions={finalize_conversions} init_ns={init_ns} absorb_ns={absorb_ns} finalize_ns={finalize_ns} final_drop_ns={final_drop_ns} total_ns={total_ns} row_payload_bytes={row_payload} column_payload_bytes={column_payload} parity=exact");
                        }
                    }
                }
            }
        });
    }
    assert_eq!(records, 48);
    assert_eq!(total_bootstrap, 3_194_880);
    assert_eq!(total_finalize, 3_194_880);
    assert!(total_permutations > 0);
    println!(
        "sha3_persistent_cost_complete records={records} four_state_permutations={total_permutations} bootstrap_conversions={total_bootstrap} finalize_conversions={total_finalize} parity=exact"
    );
}
