# Foundational model extraction inventory

This 2026-09-06 source review identifies the first dependency-closed move into
`iroha_model_base`. It is implementation guidance, not completed extraction or
build-memory evidence. The [redesign](first_release_architecture_redesign.md)
and [canonical identity contract](norito_schema_identity.md) govern acceptance.

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
| `chain` | `ChainId`, bounded text parsing, and private `ChainIdText`/`ChainIdWire` helpers from the current `id` module. |
| `account` | `AccountId`, `AccountController`, `MultisigPolicy`, `MultisigMember`, `AccountAddress`, associated errors, curve identifiers, scoped address context, bounded I105 JSON and compliance vectors. |
| `domain` | `DomainId` and its complete parsing/codec implementation. |
| `asset` | `AssetDefinitionId`, `AssetBalanceScope`, `AssetId`; close over account, domain, name and dataspace identities. |
| `topology` | `DataSpaceId`, `LaneId`, `ShardId` and `LaneIdError`, including their numeric JSON implementations. |
| `peer` | `PeerId` and its codecs; keep ledger `Peer` in the aggregate. |

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
This prerequisite does not complete the atomic identity cutover, physical
extraction, feature qualification or measured build-memory comparison.

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
3. Qualify explicit declarations and complete the atomic framing cutover.
   Declarations alone cannot preserve current generic parent headers after a
   physical move because active codecs still derive names from source paths.
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
