//! Exact physical layout, local failure and original-root codec controls.

use super::super::{
    CommittedMembershipRoot, Key, MembershipReadError, MembershipStore, TransactionsStorage, Value,
    canonical_height_digest,
};
use super::*;
use iroha_crypto::{HashOf, MerkleMapNodeStore, MerkleMapReadError, MerkleMapUpdateWorkspace};
use std::{collections::BTreeMap, io, num::NonZeroUsize};

fn generation(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn location(n: u64) -> MembershipLocation {
    MembershipLocation::new(generation(7), n * RECORD_BYTES).unwrap()
}

fn records() -> [MembershipRecord; 3] {
    [
        MembershipRecord::Height(generation(17)),
        MembershipRecord::Node(MerkleMapNode::Leaf {
            key: Hash::new(b"key"),
            value: MerkleMapValueRef {
                hash: canonical_height_digest(17),
                location: location(0),
            },
        }),
        MembershipRecord::Node(MerkleMapNode::Branch {
            bit: 9,
            prefix: [0; 32],
            left: MerkleMapNodeRef {
                hash: Hash::new(b"left"),
                location: location(1),
            },
            right: MerkleMapNodeRef {
                hash: Hash::new(b"right"),
                location: location(2),
            },
        }),
    ]
}

fn frame(codec: &MembershipRecordCodec, record: MembershipRecord) -> [u8; FRAME_BYTES] {
    let mut bytes = [0; FRAME_BYTES];
    let mut output = bytes.as_mut_slice();
    codec.write(&mut output, record).unwrap();
    assert!(output.is_empty());
    bytes
}

fn reframe(codec: &MembershipRecordCodec, payload: &[u8; PAYLOAD_BYTES]) -> [u8; FRAME_BYTES] {
    let mut bytes = [0; FRAME_BYTES];
    codec
        .layout
        .write(&mut bytes.as_mut_slice(), payload)
        .unwrap();
    bytes
}

#[test]
fn fixed_record_roundtrips_match_the_declared_norito_payload_at_unaligned_starts() {
    let codec = MembershipRecordCodec::new().unwrap();
    assert_eq!(FRAME_BYTES, 184);
    for record in records() {
        let encoded = frame(&codec, record);
        let payload = encode_payload(record).unwrap();
        let mut reference = Vec::new();
        norito::core::write_bare_frame_with_header_flags::<MembershipRecordPayload, _>(
            &mut reference,
            &payload.0,
            0,
        )
        .unwrap();
        assert_eq!(reference, encoded);
        let bare = norito::codec::Encode::encode(&payload);
        assert_eq!(bare, payload.0);
        assert_eq!(payload.encoded_len_hint(), Some(PAYLOAD_BYTES));
        assert_eq!(payload.encoded_len_exact(), Some(PAYLOAD_BYTES));
        for start in 0..16 {
            let mut misaligned = [0; FRAME_BYTES + 16];
            misaligned[start..start + FRAME_BYTES].copy_from_slice(&encoded);
            assert_eq!(
                codec.read(&misaligned[start..start + FRAME_BYTES]).unwrap(),
                record
            );
        }
    }
}

#[test]
fn fixed_height_frame_has_one_exact_golden_layout() {
    let codec = MembershipRecordCodec::new().unwrap();
    let bytes = frame(&codec, MembershipRecord::Height(generation(17)));
    let mut expected = [0; PAYLOAD_BYTES];
    expected[0] = 1;
    expected[1] = 3;
    expected[8] = 17;
    assert_eq!(codec.layout.payload(&bytes).unwrap(), expected);
    let header = Header::read(bytes.as_slice()).unwrap();
    assert_eq!(
        header.schema,
        norito::schema::identity::frame_hash::<MembershipRecordPayload>()
    );
    assert_eq!(header.length, PAYLOAD_BYTES as u64);
    assert_eq!(header.flags, 0);
    assert_eq!(header.compression, norito::core::Compression::None);
    // This vector also binds the declared schema identity and the CRC/header bytes.
    assert_eq!(
        hex::encode(bytes),
        include_str!("membership_record_height.hex").trim()
    );
}

#[test]
fn fixed_record_rejects_every_truncation_suffix_and_substituted_frame_layout() {
    let codec = MembershipRecordCodec::new().unwrap();
    for record in records() {
        let bytes = frame(&codec, record);
        for cut in 0..FRAME_BYTES {
            assert!(
                matches!(codec.read(&bytes[..cut]), Err(RecordError::Frame(_))),
                "cut {cut}"
            );
        }
        let mut extended = bytes.to_vec();
        extended.push(0);
        assert!(matches!(codec.read(&extended), Err(RecordError::Frame(_))));
        for offset in [0, 4, 5, 6, 22, 23, 31, 39] {
            let mut wrong = bytes;
            wrong[offset] ^= 0xff;
            assert!(
                matches!(codec.read(&wrong), Err(RecordError::Frame(_))),
                "header byte {offset}"
            );
        }
    }
}

#[test]
fn fixed_record_rejects_unknown_kinds_reserved_bytes_zero_heights_and_unmarked_hashes() {
    let codec = MembershipRecordCodec::new().unwrap();
    for record in records() {
        let original = encode_payload(record).unwrap().0;
        for (offset, value) in [(0, 0), (0, 2), (1, 0), (1, 4)] {
            let mut bytes = original;
            bytes[offset] = value;
            assert!(matches!(
                codec.read(&reframe(&codec, &bytes)),
                Err(RecordError::UnknownKind)
            ));
        }
        let reserved: Vec<usize> = match record {
            MembershipRecord::Height(_) => (2..8).chain(16..PAYLOAD_BYTES).collect(),
            MembershipRecord::Node(MerkleMapNode::Leaf { .. }) => {
                (2..8).chain(88..PAYLOAD_BYTES).collect()
            }
            MembershipRecord::Node(MerkleMapNode::Branch { .. }) => (2..8).chain(10..16).collect(),
        };
        for offset in reserved {
            let mut bytes = original;
            bytes[offset] = 1;
            assert!(
                matches!(
                    codec.read(&reframe(&codec, &bytes)),
                    Err(RecordError::ReservedBytes)
                ),
                "reserved {offset}"
            );
        }
        let markers: &[usize] = match record {
            MembershipRecord::Height(_) => &[],
            MembershipRecord::Node(MerkleMapNode::Leaf { .. }) => &[39, 71],
            MembershipRecord::Node(MerkleMapNode::Branch { .. }) => &[79, 127],
        };
        for &offset in markers {
            let mut bytes = original;
            bytes[offset] &= !1;
            assert!(matches!(
                codec.read(&reframe(&codec, &bytes)),
                Err(RecordError::InvalidHash)
            ));
        }
    }
    let mut zero = encode_payload(records()[0]).unwrap().0;
    zero[8..16].fill(0);
    assert!(matches!(
        codec.read(&reframe(&codec, &zero)),
        Err(RecordError::InvalidHeight)
    ));
}

#[test]
fn fixed_record_branch_shape_and_all_location_fields_are_strict() {
    let codec = MembershipRecordCodec::new().unwrap();
    let original = encode_payload(records()[2]).unwrap().0;
    for bit in [256_u16, u16::MAX] {
        let mut bytes = original;
        bytes[8..10].copy_from_slice(&bit.to_le_bytes());
        assert!(matches!(
            codec.read(&reframe(&codec, &bytes)),
            Err(RecordError::InvalidBranch)
        ));
    }
    for bit in 0..256_u16 {
        let mut bytes = original;
        bytes[8..10].copy_from_slice(&bit.to_le_bytes());
        bytes[16 + usize::from(bit / 8)] = 0x80 >> (bit % 8);
        assert!(
            matches!(
                codec.read(&reframe(&codec, &bytes)),
                Err(RecordError::InvalidBranch)
            ),
            "split {bit}"
        );
    }
    let mut duplicate = original;
    duplicate[96..128].copy_from_slice(&original[48..80]);
    assert!(matches!(
        codec.read(&reframe(&codec, &duplicate)),
        Err(RecordError::InvalidBranch)
    ));
    for (record, starts) in [
        (records()[1], &[72_usize][..]),
        (records()[2], &[80_usize, 128][..]),
    ] {
        let original = encode_payload(record).unwrap().0;
        for &start in starts {
            let mut bytes = original;
            bytes[start..start + 8].fill(0);
            assert!(matches!(
                codec.read(&reframe(&codec, &bytes)),
                Err(RecordError::InvalidLocation)
            ));
            for bad in [1_u64, u64::MAX - (u64::MAX % RECORD_BYTES)] {
                let mut bytes = original;
                bytes[start + 8..start + 16].copy_from_slice(&bad.to_le_bytes());
                assert!(matches!(
                    codec.read(&reframe(&codec, &bytes)),
                    Err(RecordError::InvalidLocation)
                ));
            }
        }
    }
}

#[test]
fn locations_check_original_generation_and_complete_readable_extent_before_io() {
    let first = location(0);
    let second = location(1);
    assert_eq!(
        first.checked_range(generation(7), RECORD_BYTES).unwrap(),
        0..RECORD_BYTES
    );
    assert_eq!(
        second
            .checked_range(generation(7), 2 * RECORD_BYTES)
            .unwrap(),
        RECORD_BYTES..2 * RECORD_BYTES
    );
    assert!(matches!(
        first.checked_range(generation(8), RECORD_BYTES),
        Err(RecordError::ForeignGeneration)
    ));
    for extent in [0, RECORD_BYTES - 1, RECORD_BYTES + 1] {
        assert!(matches!(
            first.checked_range(generation(7), extent),
            Err(RecordError::OutsideReadableExtent)
        ));
    }
    assert!(matches!(
        second.checked_range(generation(7), RECORD_BYTES),
        Err(RecordError::OutsideReadableExtent)
    ));
    assert!(matches!(
        MembershipLocation::new(generation(7), 1),
        Err(RecordError::InvalidLocation)
    ));
    assert!(matches!(
        MembershipLocation::new(generation(7), u64::MAX - u64::MAX % RECORD_BYTES),
        Err(RecordError::InvalidLocation)
    ));
}

struct CutWriter {
    bytes: [u8; FRAME_BYTES],
    written: usize,
    cut: usize,
}

impl Write for CutWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.written == self.cut {
            return Err(io::ErrorKind::StorageFull.into());
        }
        let len = bytes.len().min(self.cut - self.written).min(3);
        self.bytes[self.written..self.written + len].copy_from_slice(&bytes[..len]);
        self.written += len;
        Ok(len)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn every_record_write_cut_retains_the_same_codec_input_and_exact_retry_bytes() {
    let codec = MembershipRecordCodec::new().unwrap();
    for record in records() {
        let expected = frame(&codec, record);
        for cut in 0..FRAME_BYTES {
            let mut writer = CutWriter {
                bytes: [0; FRAME_BYTES],
                written: 0,
                cut,
            };
            assert!(matches!(
                codec.write(&mut writer, record),
                Err(RecordError::Frame(norito::Error::Io(_)))
            ));
            assert_eq!(writer.written, cut);
            assert_eq!(&writer.bytes[..cut], &expected[..cut]);
            writer.written = 0;
            writer.cut = FRAME_BYTES;
            codec.write(&mut writer, record).unwrap();
            assert_eq!(writer.bytes, expected);
            assert_eq!(codec.read(&writer.bytes).unwrap(), record);
        }
    }
}

#[test]
fn invalid_typed_nodes_fail_before_the_first_external_write() {
    use zeroize::Zeroize;
    let codec = MembershipRecordCodec::new().unwrap();
    let mut invalid_hash = Hash::new(b"zeroized");
    invalid_hash.zeroize();
    let invalid = [
        MerkleMapNode::Leaf {
            key: invalid_hash,
            value: MerkleMapValueRef {
                hash: Hash::new(b"v"),
                location: location(0),
            },
        },
        MerkleMapNode::Leaf {
            key: Hash::new(b"k"),
            value: MerkleMapValueRef {
                hash: invalid_hash,
                location: location(0),
            },
        },
        MerkleMapNode::Branch {
            bit: 256,
            prefix: [0; 32],
            left: MerkleMapNodeRef {
                hash: Hash::new(b"a"),
                location: location(1),
            },
            right: MerkleMapNodeRef {
                hash: Hash::new(b"b"),
                location: location(2),
            },
        },
    ];
    for node in invalid {
        let mut writer = CutWriter {
            bytes: [0; FRAME_BYTES],
            written: 0,
            cut: FRAME_BYTES,
        };
        assert!(
            codec
                .write(&mut writer, MembershipRecord::Node(node))
                .is_err()
        );
        assert_eq!(writer.written, 0);
    }
}

// The fixture stores only actual encoded records; it does not establish file
// durability, funded append custody or production segment leases.
struct FramedStore {
    codec: MembershipRecordCodec,
    bytes: BTreeMap<u64, [u8; FRAME_BYTES]>,
    extent: u64,
}

impl FramedStore {
    fn new() -> Self {
        Self {
            codec: MembershipRecordCodec::new().unwrap(),
            bytes: BTreeMap::new(),
            extent: 0,
        }
    }
    fn append(&mut self, record: MembershipRecord) -> Result<MembershipLocation, RecordError> {
        let location = MembershipLocation::new(generation(7), self.extent)?;
        let mut bytes = [0; FRAME_BYTES];
        self.codec.write(&mut bytes.as_mut_slice(), record)?;
        self.bytes.insert(self.extent, bytes);
        self.extent += RECORD_BYTES;
        Ok(location)
    }
    fn load(&self, location: MembershipLocation) -> Result<Option<MembershipRecord>, RecordError> {
        let range = location.checked_range(generation(7), self.extent)?;
        self.bytes
            .get(&range.start)
            .map(|bytes| self.codec.read(bytes))
            .transpose()
    }
}

impl MerkleMapNodeStore for FramedStore {
    type NodeLocation = MembershipLocation;
    type ValueLocation = MembershipLocation;
    type Error = RecordError;
    fn read(
        &mut self,
        reference: &MerkleMapNodeRef<MembershipLocation>,
    ) -> Result<Option<MerkleMapNode<MembershipLocation, MembershipLocation>>, RecordError> {
        match self.load(reference.location)? {
            None => Ok(None),
            Some(MembershipRecord::Node(node)) => Ok(Some(node)),
            Some(_) => Err(RecordError::UnknownKind),
        }
    }
    fn write(
        &mut self,
        node: MerkleMapNode<MembershipLocation, MembershipLocation>,
    ) -> Result<MembershipLocation, RecordError> {
        self.append(MembershipRecord::Node(node))
    }
}

impl MembershipStore for FramedStore {
    fn read_height(
        &mut self,
        reference: &MerkleMapValueRef<MembershipLocation>,
    ) -> Result<Option<u64>, RecordError> {
        match self.load(reference.location)? {
            None => Ok(None),
            Some(MembershipRecord::Height(height)) => Ok(Some(height.get())),
            Some(_) => Err(RecordError::UnknownKind),
        }
    }
    fn write_height(
        &mut self,
        _hash: Hash,
        height: u64,
    ) -> Result<MembershipLocation, RecordError> {
        self.append(MembershipRecord::Height(
            NonZeroU64::new(height).ok_or(RecordError::InvalidHeight)?,
        ))
    }
}

#[test]
fn original_current_and_rollback_roots_authenticate_framed_records_through_corruption() {
    fn key(n: u8) -> Key {
        HashOf::from_untyped_unchecked(Hash::new([n]))
    }
    fn height(n: usize) -> Value {
        NonZeroUsize::new(n).unwrap()
    }
    let storage = TransactionsStorage::new();
    for (at, keys) in [(1, vec![key(1), key(2)]), (2, vec![key(1), key(3)])] {
        let mut block = storage.block();
        block.insert_block(keys.into_iter().collect(), height(at));
        block.commit().unwrap();
    }
    let mut store = FramedStore::new();
    let mut workspace = MerkleMapUpdateWorkspace::new();
    let root: CommittedMembershipRoot<MembershipLocation> = storage
        .block()
        .capture_committed_root(&mut store, &mut workspace)
        .unwrap();
    assert_eq!(root.read(&key(1), &mut store).unwrap(), Some(height(2)));
    assert_eq!(
        root.read_predecessor(&key(1), &mut store).unwrap(),
        Some(height(1))
    );
    assert_eq!(root.read(&key(9), &mut store).unwrap(), None);
    let root_reference = root.root.parts().1.unwrap();
    let old_record = store.bytes[&root_reference.location.offset];
    store
        .bytes
        .get_mut(&root_reference.location.offset)
        .unwrap()[FRAME_BYTES - 1] ^= 1;
    assert!(matches!(
        root.read(&key(1), &mut store),
        Err(MembershipReadError::Tree(MerkleMapReadError::Source(
            RecordError::Frame(_)
        )))
    ));
    store.bytes.remove(&root_reference.location.offset);
    assert!(matches!(
        root.read(&key(1), &mut store),
        Err(MembershipReadError::Tree(MerkleMapReadError::MissingNode(
            _
        )))
    ));
    store.bytes.insert(
        root_reference.location.offset,
        frame(&store.codec, records()[1]),
    );
    assert!(matches!(
        root.read(&key(1), &mut store),
        Err(MembershipReadError::Tree(
            MerkleMapReadError::NodeHashMismatch(_)
        ))
    ));
    store
        .bytes
        .insert(root_reference.location.offset, old_record);
    assert_eq!(root.read(&key(1), &mut store).unwrap(), Some(height(2)));
    assert_eq!(
        root.read_predecessor(&key(1), &mut store).unwrap(),
        Some(height(1))
    );
    let value = root
        .root
        .lookup(
            &root.root.hash(),
            &super::super::key_digest(&key(1)),
            |node| store.read(node),
        )
        .unwrap()
        .unwrap();
    let old_value = store.bytes[&value.location.offset];
    store.bytes.insert(
        value.location.offset,
        frame(&store.codec, MembershipRecord::Height(generation(1))),
    );
    assert!(matches!(
        root.read(&key(1), &mut store),
        Err(MembershipReadError::InvalidValue(_))
    ));
    store.bytes.remove(&value.location.offset);
    assert!(matches!(
        root.read(&key(1), &mut store),
        Err(MembershipReadError::MissingValue(_))
    ));
    store.bytes.insert(value.location.offset, old_value);
    assert_eq!(root.read(&key(1), &mut store).unwrap(), Some(height(2)));
}
