# Generic carrier codec V1

Source owners: `iroha_data_model::runtime::carrier` and Core's
`runtime::{catalog::generic_carriers, generic_carriers}`. This is a new, currently
unselected codec contract. It changes no installed definition, instruction wire
identity, native value codec, ABI hash or State commitment. A decoded carrier is
data; it is not an admission, storage or execution capability.

## Canonical record and schema identity

`CarrierRecordV1`, nominal identity `iroha.runtime.CarrierRecordV1`, contains
`schema_hash: [u8; 32]` followed by `payload: Vec<u8>`. The complete record uses
the current uncompressed canonical Norito frame: major 0, minor 0, flags `0x02`
(`COMPACT_LEN`). The usual header, schema identity, checksum, zero alignment
padding and exact-consumption rules apply. No fixed or dense alternative is
attempted. Its native `Vec<u8>` payload field uses Norito's raw-byte specialization.

The schema is the exact existing `CarrierSchemaV1` descriptor, validated under
its children-first DAG rules. Its identity is

```text
Hash("iroha:runtime:carrier-schema:v1\0" || encode_canonical(schema))
```

where `Hash` is the chain's existing `iroha_crypto::Hash` operation. The complete
schema frame includes names, bounds, field order, discriminants and every listed
node, including a node unreachable from the last/root node. The record must name
that exact identity. Names are not normalized and schemas are not inferred from
payloads. Bare storage/map keys have no record envelope; their owner must retain
the exact schema separately.

## Inner compact payload

Every child field prefix is the shortest unsigned compact Norito varint of the
child's byte length. Every container payload must be consumed exactly.

| Schema node | Payload |
| --- | --- |
| `Unit` | Empty. |
| `Bool`, unsigned integers, `I64` | The corresponding native compact Norito scalar; little-endian fixed-width integers and only boolean `0`/`1`. |
| `String` | Native compact UTF-8 string, checked against `max_bytes` in UTF-8 bytes. |
| `Bytes` | Native `Vec<u8>`: fixed little-endian `u64` count followed by raw bytes, checked against `max_bytes`. |
| `AccountId`, `AssetDefinitionId`, `Hash` | The corresponding native compact codec, with all canonical identity and hash-marker validation. |
| `Option` | One byte `0`, or byte `1` followed by exactly one length-prefixed child. |
| `Vec` | Fixed little-endian `u64` count, then one length-prefixed child per element, in order. |
| `Map` | Fixed little-endian `u64` entry count, then length-prefixed key and value children for each entry. |
| `Struct` | One length-prefixed child for each field in declaration order; no extra count or names. |
| `Enum` | Fixed little-endian `u32` discriminant, followed by exactly one length-prefixed payload child, including a zero-length child for `Unit`. |

Generic vectors always frame their children, including `Vec` of `U8`. This
differs from the native `Vec<u8>` specialization represented by the `Bytes`
node. Enum unit payloads likewise retain the explicit child prefix. Neither
case introduces a change to an existing native Rust codec.

Map entries are strictly lexicographically ordered by **bare encoded key bytes**.
Duplicate bytes and non-increasing order are rejected on both encode and decode;
the encoder does not sort supplied entries. This is a distinct carrier rule,
not native `BTreeMap<K, V>` typed ordering. For example, `U16(256)` encodes as
`00 01`, so it precedes `U16(1)` (`01 00`). The same canonical key encoder owns
future carrier-storage ordering. Struct/enum names affect the schema hash, not
the value payload.

## Bounds and refusal ownership

| Bound | Maximum |
| --- | ---: |
| Complete carrier record frame | 1,048,576 bytes |
| Complete canonical schema frame | 65,536 bytes |
| Bare map or storage key | 4,096 bytes |
| Visited semantic value nodes, including root | 65,536 |
| Schema nodes / depth / members | Existing descriptor limits: 256 / 16 / 64 |
| String, bytes or container length | Its schema's declared bound, within the existing descriptor ceiling |

All applicable bounds hold together. Empty values still count as semantic nodes.
Counts and minimum field extents are checked before container reservation;
fallible reservations charge the active Norito allocation scopes. Map keys are
encoded under their own byte ceiling. Native leaf decoders retain their canonical
field, sequence, identity and resource checks. Dynamic containers enter the same
Norito depth guard through `core::with_decode_depth_scope`; no local scope is reset
or replaced. The independent schema-depth limit still applies.

Public codec entry points return `DecodeAttemptError`. Invalid input or a
protocol-bound violation remains `Invalid`; a refusal from the original enclosing
resource owner remains `EnclosingLimit`; an allocator refusal remains `Allocator`.
These observations grant no release or retry entitlement. Production invocation
owners must still fund original input, reconstructed values, codec scratch and
effects from their actual retained allocation owner. The codec's cumulative
accounting alone is not physical custody.

## Selected execution contract

The catalog now records a distinct `iroha.kernel.generic_carriers.v1` kernel and
a distinct restricted C10-backed ABI V1 host contract. The host binds the C10
gas schedule, a 10,000,000 gas ceiling, 64 effects and 16 storage dependencies
per access class. Interfaces use metering class `ivm.generic_carriers.v1`.
This first contract admits one interface per IVM implementation, no events,
permission or parameter access, scans or nested calls, and only installed
carrier-keyed/carrier-valued tables with bare projection. Core's selected
descriptor decoder enforces those bounds and ordinary restrictions before
validating the exact opaque frame. The admission path borrows the authenticated
State definitions and original frame, using schema-directed canonical validation
without reconstructing an owned value graph. The result grants no invocation
authority. Methods in this initial profile are public: no authenticated caller,
account permission or authority is inferred from carrier payload bytes.

The release explicitly reports both dormant kernel and host as unsupported and
omits the host from handshake advertisements. They are catalog review data;
neither their presence nor successful local decoding allows governance support
attestation or block execution. They are not an empty native instruction addition.
Baseline opaque-envelope admission remains unchanged. The separate native
addition `iroha.runtime.generic_carrier_artifacts.v1` stages exact restricted
artifacts of at most 1,048,576 total bytes through
`iroha.runtime.generic_carrier.stage_artifact.v1` into its own
`world.runtime_generic_artifacts` owner. It does not reinterpret the installed
contract upload instructions or their storage. Staging is also unqualified and
reported unsupported until its original signed publication and retained-byte
custody checks pass. Publishing bytes never grants execution or activation
authority.

The new external admission seam recognizes only one opaque instruction in the
original signed instruction vector under the exact carrier kernel. Mixed batches,
sealed reveals and deferred instruction groups keep their existing rejection
paths. The execution owner checks the original network entrypoint, transaction
hash, authority and ordinary intent signature, and refuses genesis, callbacks and
captured execution contexts. Recognizing an envelope does not grant this source
authority.

Carrier fee admission reserves the original signed gas limit. Execution debits
the ordinary envelope cost first and gives the VM the remaining signed and block
budget. The existing transaction meter retains actual VM gas on a semantic trap
or effect rejection; successful execution settles through the existing fee
owner. A local resource refusal keeps its original deferred attempt and cannot
become a committed rejection or fee result. These source paths still require
current-candidate Core execution and paid rollback/replay qualification.

## Physical storage and keyed projection

`world.runtime_generic_values` is a separate physical backing owner, indexed by
`CarrierStorageKeyV1 { storage: DefinitionId, key: Vec<u8> }`. The key bytes are the
canonical bare key under that exact installed virtual storage definition; the
value bytes are the complete canonical schema-bound carrier record. The new
kernel requires this physical owner and its separately identified
`iroha.keyed-state.carrier-radix-blake3.v1` commitment profile. It never redirects
an existing native table or changes the installed native keyed profile.

The current implementation retains physical key and value allocations in
immutable shared owners charged to the original State pool. Both current and
undo maps use admitted nodes; copies retain the same payload custody. The staging
table uses the same immutable byte custody under the distinct native
`ContractArtifactId` key and its own map policy. Bytes have no shared semantic
interpretation: the carrier table validates record schemas, while the artifact
table validates the exact restricted executable profile and full code hash.

Both new stores have one binary JSON snapshot envelope:
`{"revert":[{"key":{"encoded_hex":"..."},"value":null}],"blocks":[]}`.
Present values use the same lowercase `encoded_hex` blob wrapper. Entries are in
strict native physical key order. The bytes inside each blob are the exact bare
Norito encoding of its model key or native `Vec<u8>` value. Restore borrows the
original JSON, rejects alternate spelling, order, duplicate keys and trailing
bytes, and admits bounded hex scratch plus retained payloads from the original
State pool. Current and predecessor cuts validate their selected owners and
record or artifact domains independently. Uninstalled stores remain absent
from the canonical World snapshot and empty in memory.

Under the new keyed profile, the physical carrier wrapper is excluded from tree
leaves. Every selected virtual table contributes its declared namespace, bare
canonical key and bare record payload. The schema includes empty tables and all
installed immutable storage identities; native tables retain their existing
codecs and owners. Complete capture sorts borrowed positions by namespace and
key bytes using original-pool scratch. Incremental capture validates the exact
physical predecessor and projects insert, update and delete through the existing
keyed tree. The selected directory and its control are retained by the original
State allocation owner.

The governed carrier cutover requires an existing complete native keyed
predecessor, the exact new kernel, active host, backing and artifact owners, and
every original State-level owner. Its first cut retains every predecessor
storage definition. Proposal and activation inspect the original published plus
staged definitions without constructing a directory or hashing a replacement
source. Execution uses the existing retained keyed predecessor as its parent;
a changed schema selects complete capture, while subsequent identical schemas
use the same incremental version owner. A direct World-only-to-carrier jump or
carrier downgrade to either prior known commitment is refused. This source path
still requires execution and replay qualification.

The new IVM path uses the existing `CoreHost` ordered artifact queue and rollback
owner. It prepares retained payload custody for every queued carrier write before
applying the group; map admission remains fallible and a local refusal arms the
enclosing State transaction against publication. Read-your-writes scans that
same ordered queue. Neither prepared bytes nor a serialized storage definition
identifier grants access: the invocation owns bounded read/write capability
positions resolved from its authenticated selected implementation.

Fresh carrier execution and artifact validation construct the VM with the original
allocation pool and no built-in `DefaultHost` or boxed syscall dispatcher. The
shared constructor retains ordinary guest-memory, register and program admission.
Execution supplies its admitted `CoreHost` through `run_with_host`; calling `run`
without an explicitly installed host preserves the existing `HostUnavailable`
refusal. It never creates a fallback host. Ordinary VM constructors retain their
current default-host behavior.

The restricted `CoreHost` likewise omits native-only host facilities, including
native codec hosts, crypto configuration and shared account/ZK caches. Its ordered
queue is reserved from the original execution pool before allocation and remains
attached through artifact disposal. Syscall dispatch rejects unavailable
capabilities before accessing those facilities. ALLOC and INPUT publication use
the existing shared kernels; a literal
TLV copy retains its original VM-pool reservation until the host-arena copy
finishes. These constructor changes do not replace the invocation's input,
capability, queued-value or persistent-storage owners, and their focused tests
remain part of the qualification work below.

TODO(N14): qualify the current execution, snapshot, retained storage, keyed
capture and signed fee paths together; qualify governed selection transition and
replay coverage for the new commitment profile, including exact original-pool
refusal and rollback. Current-candidate Core tests must cover selection, artifact
staging, public-method execution, storage ownership, malformed carriers, complete
and incremental commitments, snapshot predecessors and replay before these
profiles or the staging addition may advertise support. The source integration
and component tests alone do not authorize activation.
