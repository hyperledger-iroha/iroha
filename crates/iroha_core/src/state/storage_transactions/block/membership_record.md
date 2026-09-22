# Membership record layout

`membership_record.rs` owns the sole fixed physical record layout for located
membership nodes and canonical height preimages. It is a prerequisite; the
funded append batch, original file-generation leases and durable State publication
are not yet connected. These records do not replace the logical map commitment.

Each record is exactly 184 bytes: the standard 40-byte uncompressed Norito header
followed by a 144-byte payload. The declared schema name is
`iroha_core::state::membership::RecordV1`, with header flags zero and byte
alignment one. The payload implements raw fixed bytes explicitly; ordinary Norito
array encoding is not its layout. The cached typed Norito framing owner writes
and checks the header, CRC, schema, flags, exact length and padding. There is no
fallback layout, inferred flag set, native-memory cast or alternate header codec.

All payloads have version `1` at byte 0, a kind at byte 1, and zero bytes 2–7.
Ranges below exclude their ending offset. All integers use little endian.

| Kind | Fields | Remaining bytes |
| --- | --- | --- |
| 1: leaf | key hash 8–40; value hash 40–72; value location 72–88 | 88–144 must be zero |
| 2: branch | split bit u16 8–10; zero 10–16; raw prefix 16–48; left hash 48–80; left location 80–96; right hash 96–128; right location 128–144 | none |
| 3: height | nonzero height u64 8–16 | 16–144 must be zero |

A location is a nonzero generation u64 followed by a record-area-relative offset
u64. The offset must be a multiple of 184 and its complete extent must fit u64.
The original physical owner defines the record-area origin and retained generation;
a scalar generation alone grants no authority or lease. Resolve only within that
owner's complete readable record extent before I/O. A partial provisional tail is
not readable merely because bytes exist in the file.

Every hash must already carry Iroha's low-bit marker; malformed bytes are rejected
before construction, never normalized. A branch split is below 256, all prefix
bits at/after it are zero, and the two child hashes differ. The shared map reader
still verifies the expected logical hash and parent/child path. The membership
reader still authenticates a height's exact domain-separated preimage and current
or rollback frontier. CRC validation is not logical authentication. Missing,
truncated, malformed or substituted records never establish nonmembership.

The codec retains fixed metadata and uses a bounded stack payload or borrowed
slices for record operations. Schema construction, caller output/workspaces,
external writer errors, file handles and segment/root leases require their own
original admission. A short write leaves a provisional prefix; only the append
owner may retry it, acknowledge durability or publish its location. Codec success
does not authorize any of those transitions.

`membership_record_height.hex` is an independently computed height-17 golden frame.
Roundtrip, malformed-field, every-byte write/truncation and real membership-root
tests exercise the codec; their in-memory framed store is not disk durability or
resource-admission evidence.
