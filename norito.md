# Norito Format (v1)

This document is the source of truth for Norito's on-wire encoding in the
Iroha workspace. It defines the header, flags, and the canonical length and
string layouts used across components.

## Header

The Norito header is always present on wire and on disk. It frames the payload
and supplies the schema hash and checksum needed for deterministic decoding.

| Field | Size (bytes) | Notes |
| --- | --- | --- |
| Magic | 4 | ASCII `NRT0` |
| Major | 1 | `VERSION_MAJOR = 0` |
| Minor | 1 | `VERSION_MINOR = 0x00` |
| Schema hash | 16 | First 16 bytes of a domain-separated SHA-256 schema digest |
| Compression | 1 | `0 = None`, `1 = Zstd` |
| Payload length | 8 | Uncompressed payload length (u64, little-endian) |
| CRC64 | 8 | CRC64-XZ (ECMA polynomial, reflected, init/xor all ones) over the payload |
| Flags | 1 | Layout flags (see below) |

Total header size: 40 bytes.

Alignment padding:
- For uncompressed payloads, encoders must insert zero padding between the
  header and payload when the archived type's alignment would otherwise be
  violated.
- Padding length must be the exact alignment padding required for the type and
  padding bytes must be zero. Decoders without a concrete type alignment must
  accept any zero padding up to 64 bytes and treat the remaining bytes as the
  payload. Extra non-zero bytes are rejected.

Schema enforcement:
- Typed decoders must reject payloads whose header schema hash does not match
  the expected type. `ArchiveView::decode` performs this check; use
  `ArchiveView::decode_unchecked` only for raw inspection tools.

## Header Flags

These flags are ORed into the final header byte. Unknown bits are rejected.

| Flag | Hex | Meaning |
| --- | --- | --- |
| `PACKED_SEQ` | `0x01` | Packed sequence layout for variable-sized collections. |
| `COMPACT_LEN` | `0x02` | Per-value length prefixes are compact varints. |
| `PACKED_STRUCT` | `0x04` | Packed struct layout for derive-generated types. |
| `VARINT_OFFSETS` | `0x08` | Reserved in v1; packed sequences always use `(len + 1)` u64 offsets. |
| `COMPACT_SEQ_LEN` | `0x10` | Reserved in v1; sequence length headers are fixed u64. |
| `FIELD_BITSET` | `0x20` | Packed-struct hybrid uses a bitset indicating which fields carry explicit sizes (requires `PACKED_STRUCT` + `COMPACT_LEN`). |

Flag scoping rules:
- `COMPACT_LEN` affects per-value length prefixes only.
- Reserved layout bits (`VARINT_OFFSETS`, `COMPACT_SEQ_LEN`) are rejected when decoding headers.

Except for the declared `FIELD_BITSET` dependency, flags have no heuristic
cross-effects. Encoders and decoders reject `FIELD_BITSET` unless both
`PACKED_STRUCT` and `COMPACT_LEN` are present. When a hybrid packed struct emits
a field bitset, its final header retains those two required flags even if every
field is self-delimiting and therefore no explicit compact size prefix appears.

Default v1 payloads use `COMPACT_LEN` (`flags = 0x02`) while keeping the minor
version byte fixed at `0x00`. The header flag byte is therefore the source of
truth for compact per-value lengths; decoders must not infer compactness from
the version or from payload heuristics. Legacy fixed-width per-value prefixes
remain supported when a caller explicitly encodes with `flags = 0x00`.

## Length Prefixes

Norito uses length prefixes in multiple places, with explicit flags deciding the
encoding:

- Per-value prefixes (fields, elements, strings, blobs) use `COMPACT_LEN`.
  - If set: unsigned varint (7-bit continuation).
  - If not set: fixed 8-byte little-endian u64.
- Sequence length headers are fixed 8-byte little-endian u64 in v1.
- `Vec<u8>` is encoded as a fixed-size sequence: `[len_u64][raw-bytes]` (no per-element
  length prefixes), regardless of `PACKED_SEQ`. Decoders reject per-element
  length-prefixed byte vectors.
- Packed-sequence offsets are always `(len + 1)` u64 offsets, monotonic with the
  first offset 0.

Varint encodings must fit in `u64` and use the shortest (canonical) encoding;
overflow or overlong encodings are rejected.

## Binary Sequence Span Planning

Norito implementations may plan binary sequence payload spans before semantic
decode. The planner is an internal optimization and does not change the wire
layout:

- Length-prefixed sequences are planned from `[count_u64][len][payload]...`,
  honoring the header's `COMPACT_LEN` flag for each element length.
- Packed sequences are planned from `[count_u64][(count + 1) u64 offsets]`
  followed by concatenated element payloads. Offsets must start at `0`, be
  monotonic, and the final offset must fit inside the available payload bytes.
- The plan returns element byte ranges in original sequence order and the total
  bytes consumed from the sequence payload. Semantic decode and validation still
  happen on CPU and must report failures in original index order.
- Optional Metal/CUDA helpers may compute the spans for large payloads, but
  helper results are self-tested and validated against the scalar planner before
  use. An unavailable backend falls back to the scalar planner for that call.
  Helper errors, malformed span output, or scalar mismatches fall back and
  disable that helper for the process. GPU-named helper exports report
  unavailable or backend failure instead of silently substituting CPU work; the
  Norito caller owns deterministic scalar fallback.
- Helper use is performance-only: decoded values, rejection class, ordering,
  hashes, and emitted bytes must remain identical. Native helper waits are
  bounded before CPU fallback.

### Decode-scoped resource limits

Archive byte limits do not bound collection reservations: an eight-byte
sequence header can advertise an element count far larger than the containing
payload, while nested length-delimited fields can amplify otherwise modest
archives. Hosts decoding untrusted data with known semantic bounds
must use `decode_from_bytes_with_limits` (or
`decode_from_reader_with_limits`) and an explicit `DecodeLimits` value. The
budget specifies a per-sequence element count, a per-field/blob byte length,
cumulative element and allocation-byte totals, and a maximum nesting depth.
Norito validates declared bodies against the bytes remaining before allocating
temporary storage and returns typed resource-limit errors on violation.
Compatibility layout fallbacks treat every resource-limit and allocation error
as terminal: they never retry the same field through an alternate decoder after
a budget has rejected it.

Nested decode scopes may tighten but never relax an outer budget. Counters are
shared with Norito-managed Rayon workers. They are not implicitly copied to
arbitrary threads created by application-defined deserializers; such code must
pass a bounded decode operation explicitly. Lazy callers must use
`stream_seq_iter_with_limits`, `StreamSeqIter::new_with_limits`, or the bounded
`StreamMapIter` constructors so the iterator owns a cloneable budget context
and reapplies it for every `next`/`finish` call. No thread-local guard is moved
with an iterator.

The ordinary unbounded decode and iterator APIs remain available for trusted
callers. A host must choose cumulative budgets with enough headroom for
temporary alignment copies and container metadata; accounting is intentionally
conservative and may charge both a declared field body and a temporary copy.

## Bounded data-model text leaves

The first-release `ChainId`, `Name`, and `Json` wrappers enforce their semantic
and resource invariants on every safe construction and decode path. A
`ChainId` is exact, case-sensitive ASCII, is 1–128 bytes, begins and ends with
an ASCII alphanumeric, and otherwise permits only alphanumerics plus `.`, `_`,
`:`, and `-`. Its decoder rejects an oversized declared string length before
reading or allocating the body. A `Name` is
NFC-normalized, is at most 255 UTF-8 bytes, and rejects whitespace, Unicode
control characters (including NUL), Unicode bidirectional controls, and the
reserved `@`, `#`, and `$` delimiters.

A `Json` value is exactly one well-formed Norito JSON document with no
duplicate object keys or trailing tokens. Its UTF-8 representation is at most
1,048,576 bytes and its structural depth is at most
`norito::json::MAX_JSON_VALUE_NESTING_DEPTH`. The fixed type-level limits apply
before any lower ledger or application-specific metadata limit. Norito
decoders inspect the nested string length before allocating its backing
storage; malformed, oversized, or over-depth wire values never become a
`Json`. Raw JSON producers use the fallible `Json::from_raw_json`, while plain
text that should become a JSON string uses `Json::new` or `Json::from`.

Typed JSON floating-point values must be finite. Norito rejects literals whose
decimal exponent overflows `f64`; finite values are rendered with Ryu's
locale-independent shortest-roundtrip representation, preserving their exact
IEEE-754 bits when decoded again.

## Transaction Payload Layout

`TransactionPayload` is an eight-field canonical struct. Its fields are encoded
in this exact order, with the active per-field length-prefix rules:

```text
chain
authority
creation_time_ms
instructions
time_to_live_ms
nonce
fee_payment
metadata
```

`fee_payment` is required; it is not an optional extension and it precedes
`metadata` on wire. It contains either an authority payer or one exact sponsor
program and immutable revision, followed by canonically ordered charge limits
and the optional positive executable gas bound. The retired transaction
metadata keys `fee_sponsor`, `gas_asset_id`, and `gas_limit` are not alternate
encodings of this field and are rejected by transaction construction and
admission. SDK encoders and fixture exporters must use this eight-field layout;
the former seven-field payload is not a supported compatibility format.
`time_to_live_ms` retains its canonical option discriminant so a malformed
signed payload can be decoded into a typed admission rejection, but
first-release transactions must encode `Some(positive milliseconds)`. Safe
builders assign `100_000` ms when no explicit lifetime is selected, and
stateless admission also enforces the governed
`transaction.max_time_to_live_ms` ceiling.

The `instructions` field contains the `Executable` enum. Its canonical variant
tags are stable and append-only:

```text
0  Instructions(ConstVec<InstructionBox>)
1  ContractCall(ContractInvocation)
2  Ivm(IvmBytecode)
3  IvmProved(IvmProved)
4  Batch(ConstVec<ExecutableBatchItem>)
```

`Batch` is the flat ordered form for atomically interleaving native ISIs and
deployed-contract calls. Each `ExecutableBatchItem` uses tag `0` for
`Instruction(InstructionBox)` and tag `1` for
`ContractCall(ContractInvocation)`. Raw IVM bytecode and nested batches are not
batch-item variants. Nodes reject an empty `Batch`; SDKs should reject one
before signing. Existing instruction-only transactions continue to use
`Executable` tag `0`, so adding the mixed form does not rewrite their canonical
bytes. The append-only variant is advertised by `DATA_MODEL_VERSION = 3`.

Dynamic `InstructionBox` and erased `QueryBox` payloads carry a registry wire
identifier plus the concrete Norito payload. First-release built-ins use
explicit, frozen identifiers: instruction IDs are inventoried in
`crates/iroha_data_model/src/isi/registry/wire_ids.rs`, and query IDs are pinned
by `crates/iroha_data_model/tests/fixtures/query_wire_ids_v1.txt`. Encoders emit
those identifiers rather than deriving new values from the current Rust module
layout. Registries retain the concrete Rust type name as a decode lookup alias,
so internal refactors can move an implementation without changing canonical
bytes or breaking already encoded values. New built-ins must add a unique
identifier and update the corresponding golden inventory; an existing V1
identifier must not be renamed or reused for a different layout.

The current SDK/node compatibility handshake is `DATA_MODEL_VERSION = 4`.
Version 4 changes the canonical validation-fee governance layout:
`ProposeValidationFeePolicy` and `ProposeValidationFeePayoutLifecycle` require
the exact `plain_electorate_rules` used by their ballot lifecycle—including
the voting asset, bond-escrow account, and slash-receiver account—and enacted
registry entries retain the same rules for historical verification. New locks
retain the same custody identities so lock, release, slash, and restitution
cannot be redirected by later configuration changes. At the referendum start
height, the node freezes a canonical, account-sorted PLAIN electorate of at
most 256 members from the pre-transaction committed state.
The retained proposal state binds that full electorate, its member count, the
capture and approval-gate heights, and its domain-separated roster root.
Verified Parliament projections expose those immutable anchors; proposal read
APIs may expose the full frozen member list. Missing or inconsistent snapshot
evidence fails closed for voting, finalization, enactment, and fee admission.
Version 3 peers and SDKs must therefore reject the version 4 wire contract
instead of attempting a compatibility decode.

Admission schedules a mixed batch as one global live-state barrier. Items run
in canonical input order against the same transaction view, and failure of any
item rolls back every staged state change. A signed transaction containing a
contract-call item binds one gas limit in `fee_payment`; explicit native-ISI gas
and contract-call gas consume that shared limit, and fee settlement happens
once for the transaction rather than once per item. Trigger actions may store
the same `Batch` form. One trigger invocation executes the items atomically and
shares its deterministic trigger gas budget across the complete sequence.

## Sumeragi v2 Consensus Evidence Layout

Sumeragi v2 votes and quorum certificates carry both `round` and
`proposal_round`, but first-release validity requires them to be equal for
every Prepare and Commit item. The duplicated authenticated fields make the
proposal carrier explicit; they do not permit a split-round Commit. The
canonical struct field order is:

```text
Vote:
round
proposal_round
phase
subject
execution_commitment
signer
signature

QuorumCertificateRef:
round
proposal_round
phase
subject
execution_commitment

QuorumCertificate:
round
proposal_round
phase
subject
execution_commitment
signers
aggregate_signature

SumeragiV2VoteQuorumStatus:
round
proposal_round
subject
execution_commitment
signer_count
signed_power
min_signers
total_power

SumeragiV2OutboundIntentStatus:
kind
round
proposal_round
subject
execution_commitment
stage
```

`proposal_round` is mandatory and is included in the vote signature preimage.
For both Prepare and Commit evidence it must equal `round`, including context,
height, and view. Body requests, durable manifests, validation receipts, and
finality artifacts bind the exact round-specific carrier. Application and
successor-context derivation additionally use semantic decision identity:
context, height, Commit phase, subject, and execution commitment. That
projection ignores the reproposal/QC round, signer subset, and aggregate bytes,
so an unchanged locked body decided after reproposal cannot fork successor
context. The unreleased Vote/QC layout without `proposal_round` has no decoder
or compatibility fallback. Liveness vote-quorum rows carry the same mandatory
round. Outbound proposal,
Prepare-vote, Commit-vote, Prepare-QC, and Commit-QC intents carry
`Some(proposal_round)`; timeout-vote and timeout-certificate intents carry
`None`. This status field is diagnostic, but every proposal-authenticating row
obeys the same exact-round rule as the evidence it describes. An old durable
same-round Commit may remain visible for retransmission after timeout; a later
intent cannot reuse its `proposal_round`.

The successor [`HeightContext`](crates/iroha_data_model/src/block/consensus_v2.rs)
identity projection excludes the parent's `round` and `proposal_round` together
with its signer subset and aggregate signature. It retains the parent context,
height, Commit phase, subject, and execution commitment. Consequently nodes
that decide an unchanged body in different reproposal rounds derive one
successor context, while body- or execution-distinct parent decisions cannot
alias.

## Hardware Acceleration Validation

Norito hardware acceleration is performance-only. Accelerated paths must either
produce the same semantic result as the scalar path or fall back:

- GPU CRC64 helpers must pass startup self-tests that include large payloads and
  chunk-boundary sizes. Sampled production calls compare the GPU checksum to the
  portable CRC64-XZ fallback; any mismatch marks the helper unavailable and the
  call is recomputed on CPU.
- CPU SIMD CRC64 candidates are selected only after startup parity checks against
  the portable fallback. Targets with a broken local SIMD routine use
  `crc64fast`'s runtime-selected implementation instead.
- GPU zstd compression is validated by requiring sampled GPU output to be a
  single zstd frame, decoding it on CPU, and comparing the uncompressed bytes to
  the original payload. GPU helpers may emit different valid single-frame zstd
  byte streams from CPU zstd; the canonical Norito payload remains the decoded
  bytes plus the header `Payload length` and CRC64. A sampled frame-shape or
  decode mismatch disables the GPU backend and falls back to CPU compression.
  Consensus-critical code must not hash or sign public Norito compressed bytes
  unless that callsite fixes its own compression implementation.
- JSON Stage-1 and binary sequence helper output is validated against scalar
  results before use so quote/string state, element ranges, and error ordering
  remain hardware-independent.

## String Encoding

`String` and `&str` values are encoded as:

```
[len][utf8-bytes]
```

`len` uses the per-value prefix rules above (`COMPACT_LEN`). Decoders must not
apply nested-length heuristics or reinterpret string payloads based on their
contents.

## Numeric and BigInt

`BigInt` encodes as:

```
[len_u32][twos_complement_le_bytes]
```

`len_u32` is a 4-byte little-endian length of the following payload. The bytes
are the unique minimal little-endian two's-complement representation; zero has
an empty payload. The value is bounded to the signed 4,096-bit domain
`-2^4095..=2^4095-1`, so the canonical payload is at most 512 bytes. Values
outside that signed domain and redundant sign-extension bytes are rejected.

`Numeric` encodes as a struct `(mantissa, scale)`:
- `mantissa` is a `BigInt` containing the raw integer value (no decimal scale
  is embedded in the integer).
- `scale` is a `u32` count of fractional digits (e.g., `1.88` is mantissa `188`,
  scale `2`).

The V1 Kotodama `decimal` profile uses `Numeric` with scale `0..=28`; its
canonical pointer representation removes fractional trailing zeroes and stores
zero at scale zero. `quantity` applies the same canonical representation and
additionally requires a non-negative mantissa. Arithmetic never rounds unless
an explicitly rounded operation supplies a scale and rounding mode.

Kotodama V1 numeric pointers carry one complete, uncompressed, schema-bound
Norito frame. Numeric frames always use header flags `0`, compression `None`,
and no alignment padding. Their payloads are:

```text
IntValueV1      := byte_len_u32_le || mantissa_twos_complement_le
DecimalValueV1  := byte_len_u32_le || mantissa_twos_complement_le || scale_u8
QuantityValueV1 := byte_len_u32_le || mantissa_twos_complement_le || scale_u8
```

`byte_len_u32_le` is fixed-width (never a compact varint), is at most 64, and
must consume the payload exactly (apart from the required scale byte in the two
scaled forms). The signed-byte and decimal canonicality rules above are checked
after the frame checksum, schema, declared length, compression, and flags have
been validated. In particular, an empty mantissa is the only zero encoding;
`[00]`, redundant `00`/`ff` sign extension, zero at nonzero scale, a nonzero
scaled mantissa divisible by ten, scale 29 or greater, and a negative quantity
are invalid.

The normative nominal schema names and type-name hashes are:

| Type | Schema name | 16-byte schema hash (hex) | Maximum frame bytes |
|---|---|---:|---:|
| `int` | `iroha.numeric.IntValueV1` | `07c039457363b9e1d36bbd31d93dec4a` | 108 |
| `decimal` | `iroha.numeric.DecimalValueV1` | `ba2ffed52e4d8ee16f17efefe1828524` | 109 |
| `quantity` | `iroha.numeric.QuantityValueV1` | `e4769984c81ce0e8b678f2eb06274ee3` | 109 |

Including the 39-byte pointer-TLV envelope, the corresponding hard maxima are
147, 148, and 148 bytes. Lengths beyond those caps must be rejected before any
allocation based on the declared length.

Exact decimal arithmetic is defined over conceptual unbounded integer
intermediates. The exact mathematical result is normalized first; only its
canonical mantissa and scale are checked against the value domain. Exact
division first reduces the mathematical fraction and classifies its denominator.
A remaining prime factor other than 2 or 5 is a repeating decimal. Otherwise,
the larger multiplicity of 2 or 5 is the proven minimum output scale; a value
above 28 is an exact-division scale overflow. A representable quotient performs
exactly one quotient/remainder attempt at that proven scale. Implementations
must expose deterministic charge points before each denominator-classification
division, scale construction, quotient/remainder, and normalization division;
an out-of-gas decision occurs at that charge point before the arithmetic work
begins.

## Kotodama V1 Schema-Bound Aggregates

Kotodama values crossing an entrypoint or durable-state boundary use canonical
Norito records bound to an exact recursive schema. The schema and record are
wire data; compiler-owned VM handles and their heap layouts are not.

For entrypoints, `EntrypointValueTypeV1` contains one flat preorder node tape.
A `List<T, N>` is represented by `EntrypointValueTypeNodeV1::List`; that node
stores the compile-time capacity `N`, and the complete element subtree follows
it immediately in the same tape. Struct field counts, tuple arities, and the
fixed Option/Result/List child counts make every subtree boundary
deterministic. This representation preserves recursive type structure without
building a recursively owned Rust value, so binary/JSON decoding, validation,
cloning, comparison, and destruction do not consume native stack per aggregate
level. State values use the analogous recursive `StateValueNodeV1::List` node.
Both profiles require `1 <= N <= 64`, permit nested lists and structured
elements, and reject records whose logical length exceeds the schema capacity.
An entrypoint value schema is limited to 256 nodes and aggregate depth 256.
`EntrypointValueTypeV1` validates the complete tape during binary and JSON
deserialization, so truncated trees, trailing trees, over-limit depths, and
otherwise invalid schemas are never returned as decoded values.
The dynamic JSON `Value` parser permits 257 structural levels: the extra level
covers the required outer entrypoint parameter object around a value at
the full V1 type depth. Recursively owned typed JSON decoders retain their
independent 256-level guard.
The built-in `QueryPage<View>` product uses the canonical nominal schema name
`QueryPage`; its `items` list child is followed by the exact `View`
specialization, so
generic source punctuation never becomes part of an ABI identifier and the
five projection schemas remain structurally distinct. `QueryPage` and those
five projection names are reserved ABI nominals: a decoder rejects schemas
whose ordered fields, leaf kinds, list capacity, or continuation type differ
from their declared V1 shapes. Canonical public type strings retain those
nominals (`AccountView`, `Option<AccountView>`, and
`QueryPage<AccountView>`), while ordinary user structs retain the explicit
`struct Name` rendering.
The exact encoded schema is domain-separated and hashed into its argument,
return, or state record; a decoder must reject a record whose schema hash or
flat schema-delimited atom tape does not match.

On wire, a list starts with one flat `List(u8)` atom containing its active
element count. The count is followed immediately in the record's single atom
tape by one schema-delimited atom stream for each active element, in order.
The count must not exceed the schema capacity. Unused capacity, recursive
per-list containers, end markers, and placeholder elements are not serialized.
`Option` and `Result` likewise encode one boolean tag followed by atoms for
only the selected branch (`some`/`ok` when true, `none`/`err` when false); the
inactive branch is supplied by the schema and contributes no record atoms.
This rule applies recursively inside products, lists, options, and results.

After schema validation and Norito decoding, the VM materializes different,
VM-local layouts:

- A list handle names one contiguous owned-heap allocation
  `[len: u64][capacity: u64][capacity * element_words]`. The allocation reserves
  the schema capacity up front, while only the `len` active slots have semantic
  values. The element width comes from the schema and is never inferred from
  heap bytes.
- An option/result handle names one owned-heap allocation
  `[tag: u64][max(branch_words) payload capacity]`. Only the selected branch
  payload is materialized; inactive payload bytes have no semantic value.

These raw handles are neither pointer-ABI TLVs nor Norito wire values. They
must never be persisted or transmitted directly. Crossing another boundary
requires re-encoding the active logical value into its schema-bound canonical
Norito record.

## Map Encoding

Maps encode deterministically with the same active layout flags:

- Entry count uses a fixed 8-byte little-endian u64 header.
- Compat layout (`PACKED_SEQ` unset): for each entry,
  `[key_len][key_payload][value_len][value_payload]` with key/value lengths
  encoded via `COMPACT_LEN`.
- Packed layout (`PACKED_SEQ` set): key sizes and value sizes precede the data,
  followed by concatenated key payloads and concatenated value payloads. Uses
  `(len + 1)` u64 offsets for keys, then `(len + 1)` u64 offsets for values;
  offsets are monotonic with the first offset 0.
- `HashMap` encodes entries in sorted key order for deterministic output;
  `BTreeMap` uses its natural ordering.

## NCB Columnar (internal)

NCB payloads are exact and canonical:
- Alignment padding between NCB columns must be zero-filled.
- Bitset padding bits (flags and presence) must be zero.
- Trailing bytes after the NCB payload are rejected.

## AoS Ad-hoc (Adaptive Columnar)

The `norito::aos` helpers used by adaptive columnar encoders follow the same
length prefix rules and honor the active `COMPACT_LEN` flag, so embedded AoS
payloads stay consistent with their parent Norito headers.

## Packed-Struct Layout

When the `PACKED_STRUCT` flag is set, derive-generated structs/tuples are
encoded as a single packed payload with one of two layouts:

- Compat packed-struct (no `FIELD_BITSET`): `(field_count + 1)` little-endian
  `u64` offsets followed by concatenated field payloads. Offsets start at 0,
  are cumulative byte lengths of each field payload in declaration order, and
  the final offset equals the total data length. Offsets are fixed-width even
  when `COMPACT_LEN` is enabled.
- Hybrid packed-struct (`FIELD_BITSET` + `COMPACT_LEN`): a bitset of length
  `ceil(field_count / 8)` bytes, followed by size prefixes for fields whose
  bit is set (varint-encoded per `COMPACT_LEN`), followed by concatenated field
  payloads in declaration order. Bit 0 of byte 0 refers to field 0, bit 1 to
  field 1, and so on. Fields that are fixed-size or self-delimiting omit the
  explicit size header and are decoded sequentially. The bitset is part of the
  type's canonical layout: a decoder recomputes it from the same compile-time
  field classification as the encoder and rejects any mismatch, including
  non-zero padding bits. Each declared size is decoded only as the compact
  varint advertised by `COMPACT_LEN`; zero is a canonical one-byte varint and
  is never reinterpreted as a fixed-width `u64`.

Field payloads themselves use the active layout flags (e.g., `PACKED_SEQ`,
`COMPACT_LEN`) when encoding nested collections or string/blob values.

## Compression Selection and Validation

The header `Compression` byte identifies the payload encoding:

- `0 = None`: payload bytes follow the header (with optional alignment padding).
- `1 = Zstd`: payload bytes are compressed with Zstandard.

`Payload length` and `CRC64` always describe the uncompressed payload. For
compressed payloads, the encoded byte stream begins immediately after the
header with no alignment padding. Decoders must reject unknown compression
values or unsupported algorithms; builds without the `compression` feature
accept only `None`.

Encoders choose compression explicitly (`to_compressed_bytes`) or via the
adaptive helper (`to_bytes_auto`) that applies deterministic heuristics. The
chosen algorithm is recorded in the header; there is no on-wire negotiation.

## Schema Hash Details

The 16-byte schema hash is computed as the first 16 bytes of SHA-256 over a
domain prefix followed by canonical schema bytes:

- Default: `SHA-256("norito:v1:type-name\0" || fully-qualified type name)`.
  Rust uses `core::any::type_name::<T>()` for the type-name bytes.
- With `schema-structural`: `SHA-256("norito:v1:structural-schema\0" ||
  canonical JSON schema)`, where the schema is produced by
  `iroha_schema::IntoSchema` and serialized with Norito’s JSON writer.
- A struct or enum derived with
  `#[norito(schema_name = "stable.public.schema.id")]` uses
  `SHA-256("norito:v1:type-name\0" || "stable.public.schema.id")` instead.
  The explicit name takes precedence over both defaults for Encode and Decode,
  including builds with `schema-structural`, so Rust module paths and private
  implementation type names do not leak into a public wire header.

An explicit schema name is a wire-compatibility promise. The same name must not
be reused for different layouts or for generic instantiations whose layouts can
differ. Renaming the Rust type or moving it between modules does not change a
named schema hash; changing the explicit name does.

Typed decoders must reject payloads whose header schema hash does not match the
expected type. `ArchiveView::decode` enforces this check; `decode_unchecked`
is reserved for tooling that explicitly opts out of schema validation.
