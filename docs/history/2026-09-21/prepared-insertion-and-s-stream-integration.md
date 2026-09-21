# Prepared insertion, shared events and the complete S stream

The exact 41-file integration is retained in
`target/first-release-s-stream-event-storage-applied-20260921/identity.json`.
Every before/after image is checked, the original Git index is unchanged, and
preimages are preserved. This is development-source integration, not a sealed
candidate or closure of any of the fourteen release goals.

## Original insertion and shared reservation ownership

Concread now prepares insertion on the original exclusive writer and checkpoint
buffers, retaining the original key/value through planning refusal. MV can
partition one reservation's remaining bytes while retaining its original pool;
partitioning does not acquire capacity or allocate a replacement pool. The
provider policy itself still requires review before broader activation.

The root dependency build and **217 MV tests pass** with unchanged observed
source and ordinary worker stacks: 146 unit, six EBR, 22 generation, thirteen
linear custody, 24 map custody and six byte-buffer custody tests. All five new
MV controls execute. The packet is
`target/first-release-mv-prepared-insertion-native-20260921`.

Cargo refuses vendored-package unit tests outside workspace membership, both
with package features and with the default-feature retry. Those failures are
retained. An isolated copy of the actual unchanged vendor source, with a
separate test workspace and resolved dev-dependency lock, then builds and passes
**23 admission/checkpoint controls**, including the new original-input refusal.
`target/first-release-concread-isolated-native-20260921` records both source
identities, the test-only manifest/lock and binary. This supplements the root
MV run; its separately resolved dev dependencies are not production lockfile
qualification.

Storage is not activated on these interfaces yet. The complete original
operation must fund touch tracking, undo preimages and the current tree before
work. Removal/rebalance, mutable-value access, nested growth and retained State
funding remain required.

## Shared data events and authenticated S storage

`SharedDataEvent` now keeps its physical owner behind one wrapper. Raw Arc
constructors, escapes and compatibility conversions are removed; internal
publication and lazy trigger scanning retain that same wrapper. Nested payload
allocations and their own shared references still need actual funding.

The S producer now traverses all 1,600 original sampler blocks, installs 6,400
openings in the sole inventory, and seals the original authenticated encrypted
file. It preserves the original source, table, RNG, rhos and resource ledger;
only pre-operation capacity refusal returns the original owner. It stops at
inventory cursor 33,576, where complement production begins. Complement,
pre-z, qPCS redesign, full native40 source/composite admission and governed
production keys remain unfinished.

Authenticated read chunks retain a mutable borrow of the read owner through
normal destruction, with an explicit Drop boundary and the actual zeroizing
chunk. Twelve compiler controls test the complete source module with leaf
stand-ins; they prove the borrow boundary, not cryptographic or file execution.
Together with 31 corridor controls, **43 Python checks pass**. The source-matched
native run builds successfully and passes all **11 full-stream controls**,
**66 adjacent opening/retained-source controls**, **22 physical-spool controls**
and **three DataModel sharing/codec controls**. The stream tests include a real
complete sealed-file replay; bounded commitment fixtures still seed preceding
tickets explicitly. Core event/trigger tests pass eight controls and fail seven
fixtures: six enter callbacks without their source owner, and one omits the
predecessor while its helper hides the metadata error. Repairs are prepared
without weakening callback ownership. The continuing packet is
`target/first-release-s-stream-event-native-20260921`.

Full-size work, RSS and whole-proof admission remain unqualified. In particular,
the current qPCS design exceeds the unchanged tracked-work ceiling. No fixture
boundary seeded near the final ticket is treated as execution of all 6,400
production commitments.

## Tooling and failure preservation

The five stale STARK fixtures now use actual key lengths and the exact execution
circuit role; rejected development ballot relations remain rejected. Diagnostics
for the vendor execution barrier and unavailable lane-application frontier are
added without broadening accepted results. The native vendor run confirms the
Initial executor rejects unclassified SubmitBallot after consuming the latch.
The governance run passes three controls and reports the fourth cannot advance
an ordinary lane frontier without its authoritative application evidence.
Neither barrier is bypassed; fixture repair and actual Native integration remain
separate obligations. The STARK rerun passes 22 controls and fails the remaining bare execution-ID fixture; its exact-ID repair is being joined with removal of production identifier normalization.

The separate nine-file tooling integration is recorded in
`target/first-release-xtask-parser-applied-20260921`. The config-only archival
flag and alias-based storage moves are removed. Lane inventory observes current
opaque instance paths without deriving active/retired authority from config.
Authenticated lifecycle orchestration remains a Core release requirement.

The Rust source checker defers tokenization of delimiter headers until their
ancestors survive the scan. All **35 applied parser/scanner controls pass**.
Against the exact original profiled copied fixture, ordered output remains all
215 existing errors; the profiled invocation falls from 1,223.58 to 110.83
seconds. That performance comparison does not repair the source-binding errors,
change model seals or supply formal proof. Full canonical fidelity, mutation,
TLC, Apalache, TLAPS and Verus qualification remain open.

## Complement, original pools and the next integrated run

The six reviewed packets in
`target/first-release-next-reviewed-applied-20260921/identity.json` are applied
in dependency order, with all 35 distinct pre/postimages verified and the Git
index unchanged. They add the authenticated integer q−1−S complement producer,
original-ledger admission for both low-digit scalar vectors, original physical
map/pool wrappers and borrowed-key/preimage insertion. The same batch repairs
callback source ownership and propagates fixture predecessor errors; the vendor
control now tests the observed Initial barrier and preserves its direct semantic
rejection assertion. Their combined native validation is pending.

Complement admission reserves all four digit evaluations before the first
original encrypted-file read. Capacity refusal retains the same owner; later
failures consume the phase. Complete production commitment generation and
multiplicity remain open. The map wrappers bind actual original allocation pools
and map generations, but complete construction costs and the three-tree
Storage operation are still integration requirements.

The fresh xtask build with its explicit development tooling features passes,
as do all **16 focused lane-inventory, proof-artifact and refusal controls**.
Two pure-stdout Kagami runs generate identical **1,833-type** schemas including
the new privacy qualification and SDK-consumer roots. The official schema
update and consistency check pass, with canonical SHA-256
`e5cf64605783c0e8fd67a50805964a8216236d976bf053b6e8285ca4c3318e5a`.
The original runner's mixed stderr/JSON parsing failure is preserved separately;
the successful continuation uses the same observed source and rebuilt binary.
These are development validation results, not candidate or lifecycle closure.
