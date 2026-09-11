# Foundational model extraction inventory

The 2026-09-10 implementation moves `Name`, `StatePath` and `ParseError`, with
their validation, normalization, schema, FFI and storage-key implementations,
into `iroha_model_base`. The 35 moved owner tests and four feature selections
pass; all targets in the 34-package consumer check compile. The final aggregate
library passes 3,705 tests and both captured wire-fixture checks. The
[checkpoint](../docs/history/2026-09-10/model-base-name-state-path.md) separates
these source seals from earlier checks and the outstanding aggregate feature
matrix. This is not completed model extraction or build-memory
evidence. The [redesign](first_release_architecture_redesign.md)
and [canonical identity contract](norito_schema_identity.md) govern acceptance.

The Metadata implementation, all 14 owner tests, FFI dispatch and consumers are
now moved to the base owner. The aggregate API export inventory has one source;
retired Metadata paths are removed. The subsequent [Metadata checkpoint](../docs/history/2026-09-10/model-base-metadata.md)
records passing owner, aggregate and combined-FFI suites on their own source seals,
plus the 37-package all-target consumer check with developer binaries enabled.
Broader runtime and release qualification remain in progress.

`ChainId` and its private structural codecs now live in `iroha_model_base::chain`.
The [chain checkpoint](../docs/history/2026-09-10/model-base-chain.md) records
the complete owner tests, private golden-frame assertions, canonical consumer
migration and current qualification scope. `NetworkId` and `IdBox` remain at
ledger composition; the primitive grammar and byte limit keep one owner.

`DomainId` now lives in `iroha_model_base::domain`, including its canonical
slice decoder, JSON object-key implementation and `mv` storage-key codec.
All 11 existing owner tests and three private-field compile-fail examples move
with it. The base adds an independent check of all seven original domain
fixture rows; the aggregate keeps its mixed fixture assertions. Its former
foreign inherent `encode_as_id_box` expansion is removed; the composition is
`IdBox::from(domain).encode()`. The [Domain checkpoint](../docs/history/2026-09-10/model-base-domain.md)
records passing owner configurations, the 46-package consumer check, composed
runtime suites, combined FFI execution and documentation tests. Broader release
qualification remains outstanding.

The topology owners now live in `iroha_model_base::topology`; all six original
identity tests move with them. A new owner test checks all 21 captured frames
and both storage keys. The 29 retained Nexus tests and aggregate fixtures remain
in place. Consumers import the canonical owner directly; no aggregate aliases
remain. The [topology checkpoint](../docs/history/2026-09-10/model-base-topology.md)
records passing owner configurations, the 47-package consumer check, composed
base/aggregate/SDK/Musubi suites, combined FFI, SDK/Musubi strict lint and
model/SDK documentation tests. Broader release qualification remains open.

`PeerId` now lives in `iroha_model_base::peer`, with its complete public-key
codec and JSON-key behavior. All three identity tests move with it; eight
entity/mixed tests remain in the aggregate and use the existing public-key
getter. The captured seven PeerId envelope rows remain unchanged. Consumers
use the base owner directly; aggregate/SDK/executor facade paths are removed.
The [Peer checkpoint](../docs/history/2026-09-10/model-base-peer.md) records passing
owner feature/FFI tests, strict owner lint, the 47-package consumer check and the
optional P2P QUIC check. Composed suites pass 81 base, 3,694 aggregate, 802 SDK
and 386 Musubi tests; combined FFI and model/SDK documentation also pass.
The [validation and test ownership changes](../docs/history/2026-09-10/model-validation-phases.md)
close the production and test lint findings. Strict library/test Clippy passes
in default and FFI configurations. The final selected aggregate run passes
3,718 unit and 183 fixture-group tests; the FFI configuration passes 3,722 unit
tests. All model files meet the 5,000/3,000-line limits without exceptions.
All 76 moved runtime tests execute under their new owner with matching
outcomes; wire fixture identities and assertions remain intact. Physical
extraction, measured memory reduction and full release qualification remain open.

The next AccountId prerequisite has a concrete, unapplied
[JSON context and ordinary Value owner candidate](../docs/history/2026-09-11/json-value-destruction.md).
The coherent codec/derive/MV/primitives selection passes 1,862 tests across all 18
executables, including both full compiler UI suites, with one existing ignored test.
Ordinary iterative Value destruction supersedes cleanup callbacks and fixes both
actual 32,768-level overflow reproducers on ordinary stacks. Allocation regressions
cover admitted-depth and wide-container destruction. Strict all-target Clippy passes
with benchmark internals enabled. Separate primitive default and Rust FFI selections
each pass 327 library tests; FFI strict lint, 17 doctests (two existing ignores),
strict Rustdoc, formatting and codec guards pass. Earlier telemetry and benchmark
results retain their separate source scopes. Original lockfile, profiles, wire
fixtures and dirty source are preserved. External context/owned-value consumers and
scalar backend/final-tree admission still require completion and qualification
before the API replacement or account move.

## Captured evidence

The original compiler capture contains 189 complete frames across 105 nominal
identities: all 19 planned named types plus Option/Vec/HashOf/SignatureOf and
mixed envelopes. Twenty-four root samples include canonical JSON, exported
schema and sorted recursive schema identifiers. Ten key samples exercise all
eight moving storage-key implementations, including both account controller
and asset-scope cases. Binary and JSON/key roundtrips and signing verification
pass during capture. The first capture remains preserved; every original field
matches the enriched capture exactly.

The immutable fixture is
`crates/iroha_data_model/tests/fixtures/base_model_wire_identity_frames.json`,
155,592 bytes, SHA-256
`9eaba83f63302101a16a324d07bd85bd316dfc1cea8eca7bbafb876e0b57342a`.
The capture writer was removed before two permanent fixture tests passed. All
19 named declarations now have explicit captured identities; the same two tests
pass after those declarations, preserving every frame, signature, JSON value,
storage key and schema record. Address-context isolation is also covered. This
default-feature host capture does not qualify every supported feature/target or
the eventual extraction.

Derived schema identifiers for the 16 public types use stable short names.
AccountAddress's manual schema identifier is the sole path-dependent public
case and now uses its captured literal for relocation. The two private
ChainId helpers have binary codecs but no separate schema-export surface.
No additional schema-derive option is needed for this boundary.

## Ownership

Move selected definitions and their complete validation/codec implementations.
Moving whole account, asset, Nexus or domain modules would pull ledger
composition back into the foundation.

| Base module | Existing types and implementation closure |
| --- | --- |
| `error` | `ParseError`; retain aggregate `EnumTryAsError` with its enums. |
| `name`, `state_path` | `Name`, `StatePath`, their shared normalization helpers and fingerprinted Unicode policy. |
| `metadata` | `Metadata`, `Path`, entry codecs and bounded JSON behavior; depend on base `Name` and primitives `Json`. |
| `chain` | Implemented: `ChainId`, bounded text parsing, and private `ChainIdText`/`ChainIdWire` helpers. |
| `account` | `AccountId`, `AccountController`, `MultisigPolicy`, `MultisigMember`, `AccountAddress`, associated errors, curve identifiers, scoped address context, bounded I105 JSON and compliance vectors. |
| `domain` | Implemented: `DomainId`, normalization, bounded binary/JSON codecs and both storage-key protocols. Domain entities and IdBox composition remain in the aggregate. |
| `asset` | `AssetDefinitionId`, `AssetBalanceScope`, `AssetId`; close over account, domain, name and dataspace identities. |
| `topology` | Implemented: `DataSpaceId`, `LaneId`, `ShardId` and `LaneIdError`, including numeric validation, binary/JSON codecs and applicable storage keys. Catalogs and lifecycle policy remain in the aggregate. |
| `peer` | Implemented: `PeerId`, canonical public-key codecs, JSON object keys and the slice decoder. Ledger `Peer` remains in the aggregate. |

The wire capture scope contains 17 public binary types and the two private
ChainId helpers. Supporting errors and lookup/context types move with their
owner without acquiring a new binary wire surface.

Move the account controllers' validating binary derives with their public
owners. `MultisigPolicy` and `MultisigMember` invoke their existing strict
constructors through `#[norito(validate = "path")]`; the hook returns the
validated owner without sorting or normalizing external components. Their
private `PolicyFields`/`MemberFields` carriers are JSON-only and need no binary
identity or archived cast across the new boundary. Preserve these validators,
strict JSON handling and explicit canonical slice adapters during the move.
The atomic declared-identity framing cutover is implemented. The remaining
physical owners, feature qualification and measured build-memory comparison
are still outstanding.

Keep accounts and their registration/recovery/rekey records, domain and asset
definitions, aliases, lane catalogs, governance, instructions, queries,
transactions and blocks at the aggregate layer. `NetworkId` currently contains
`HashOf<BlockHeader>` and therefore stays there for this first move. A later
genesis-digest API decision must resolve that dependency explicitly; an opaque
replacement marker would merely conceal it.

## Cross-crate implementation ownership

- Move the eight applicable `mv::json::JsonKeyCodec` implementations from
  `json_key_codec.rs`: account/name/state-path, domain, two asset IDs, dataspace
  and lane IDs. A foreign trait implementation must live with its type.
- Move external slice decoders for `PeerId`, `DomainId`, `AssetId` and
  `AssetDefinitionId` from `norito_slice_decode.rs` for the same reason.
- Keep aggregate `Identifiable` and `IdBox` conversions with the local trait
  and enum. The base crate must not inherit the `Into<IdBox>` dependency.
- All `ParseError` consumers now construct and read errors through the existing
  `new()` and `reason()` methods. The owner retains its private representation
  and every rejection message. Private settlement still wipes `AssetDefinitionId.aid_bytes`;
  migrate those sites to an owned discard operation rather than exposing the
  raw field across the new boundary.
- Move Metadata FFI ownership out of the aggregate handle/export inventory.
  There must be one canonical export owner.

Base dependencies may include existing primitives, crypto, Norito/schema,
derive utilities, normalization/hash/Base58 libraries and JSON `mv`. Forbid
paths to the aggregate, service/privacy models, SoraFS manifests, configuration,
Halo2, FASTPQ and execution engines. Feature-resolved checks must include build
dependencies and every shipping feature selection.

## Ordered implementation and validation

1. Complete the five-type crypto identity capture required by account and peer
   records: Algorithm, public key/compact key, exposed private key and signature.
   Ordinary private keys and keypairs remain outside the binary codec surface.
2. Capture all 19 base wire identities under the pinned compiler, including
   private `model` paths, both codec hashes, complete frames, bare bytes, JSON
   and schema output. Include `Option`, `Vec`, `BTreeMap<Name, Json>`, typed
   hashes/signatures and mixed envelopes. Delegated payloads must retain their
   distinct nominal frame identity.
3. Qualify explicit declarations and the implemented atomic framing cutover.
   Active framed codecs use declared nominal identities, including generic
   parents. Preserve the captured headers when moving their source owners.
4. Move definitions and migrate consumers atomically. Remove retired root,
   prelude, transparent/non-transparent and module import paths; add no shims.
   Migrate registries, FFI, query projections, wildcard users and examples.
5. Check base features and fixtures, aggregate/schema consumers, SDK/CLI/config,
   Core/Torii/daemon, executor/genesis/P2P/SCCP, IVM/compiler, storage/Musubi,
   native bridges, Mochi, Python, integration tests and xtask. Compile wildcard
   consumers rather than relying only on textual import replacement.
6. Enforce the dependency boundary and compare source-sealed memory profiles.
   Logger directly needs only lane/dataspace IDs, but its configuration path
   also reaches the aggregate; removing one direct import is not graph isolation.
