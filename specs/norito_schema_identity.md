# Canonical Norito schema identity

This is the reviewed first-release cutover design. The identity kernel and its
initial fixtures are implemented; **active codec cutover and complete type
coverage remain pending**. Implementing `NoritoSchema` alone does not change
existing headers.

## One identity contract

`norito::NoritoSchema` declares a nominal name and its single root-frame
projection. Generic parents compose nominal children, never their projected
root names. Type and const arguments retain declaration order. Lifetime slots
use the fixed erased marker `'_`; the pinned compiler names `Cow<str>` as
`alloc::borrow::Cow<'_, str>`, so dropping that slot changes enclosing frames.
The fixed `schema::identity::frame_hash` helper applies the existing
domain-separated SHA-256 digest truncated to 16 bytes. Implementers cannot
override its algorithm.

Root projections preserve the existing distinction between `String` and its
borrowed/boxed forms, explicitly named DTOs and their containing vectors,
transparent wrappers and their inner types. A generic derive cannot use a
fixed projection that erases its type or const arguments. Generic forwarding
projections, such as an owned wrapper advertising its inner type's frame, need
an explicit manual identity implementation. Markers used by `HashOf<T>` and
`SignatureOf<T>` need an identity, not a payload codec.

There will be no source-path registry, alternate accepted header names,
compatibility dispatch or opt-in framing mode. Actual pre-cutover names must be
captured under the pinned compiler: public imports can conceal private `model`
modules retained by `iroha_data_model_derive/src/model.rs`.

## Final trait and framing boundary

The existing serializer is used as `dyn NoritoSerialize` in Norito's bounded
writers (`core.rs:2219`, `2240`, `2496`, `4259–4293`) and in model/Core borrowed
field serializers. Adding the current static identity trait as a supertrait
would break object safety. The reviewed atomic transition is:

1. `SerializePayload` owns object-safe serialization and encoded-size methods.
2. `NoritoSerialize: SerializePayload + NoritoSchema` is the typed contract,
   implemented once for types satisfying both. Erased bare writers use
   `dyn SerializePayload`; typed generic callers inherit the identity bound.
3. `NoritoDeserialize<'a>: Sized + NoritoSchema` retains reconstruction methods.
4. Both codec directions remove their independent `schema_hash` methods. Every
   typed frame writer/checker uses the fixed helper. Update Encode/Decode,
   derives, manual implementations and qualified calls together. No legacy
   alias is retained for the old payload trait.

This keeps allocation-conscious bare streaming while preventing a type without
an identity from entering a typed frame. `schema-structural` remains explicit
inspection data, not a feature-selected active header digest.

## Source review queue

The 2026-09-06 lexical inventory scanned 4,808 Rust files under `crates`,
`integration_tests`, `mochi`, `xtask`, `data_model`, `scripts`, `tools` and `python`.
Its source-content digest is
`cc2c760fe2ec0080c200c9c096b8ae8f75d3acf120628958b7ce629dd5f0021e`.
Generated rows and the read-only scanner are retained, untracked, under
`target/architecture-redesign/norito-schema-source-inventory.*`.

The queue contains 5,557 codec-derive attribute sites (verified import aliases
included), 262 explicit Serialize implementation sites, 189 explicit
Deserialize sites and 155 `schema_name` attributes. Sites occur in 661 files
across 47 owners; 122 manual implementation sites have generic self types.
These are **lexical source sites**, including tests, cfg branches and macro
templates, not expanded types or production-only counts. They are historical
review evidence bound to the digest, not a numerical completion target.

| Owner | Codec/identity derive sites | Explicit Serialize | Explicit Deserialize |
| --- | ---: | ---: | ---: |
| iroha_data_model | 2692 | 52 | 54 |
| iroha_core | 757 | 19 | 7 |
| sorafs_node | 414 | 1 | 1 |
| sorafs_manifest | 276 | 34 | 0 |
| iroha_torii | 271 | 20 | 10 |
| norito | 235 | 71 | 55 |
| iroha_torii_shared | 141 | 12 | 12 |
| iroha_crypto | 119 | 13 | 11 |

The source queue is not a coverage proof. The syntax inventory follows
source declarations, local items, aliases and cfg attributes; compiler checks
must cover generated version enums (`iroha_version_derive/src/lib.rs:313`),
macro families and every supported target/feature selection.

The subsequent syntax scan contains 4,827 physical files, 5,551 candidate
codec/identity declarations and 452 manual implementation sites. Of the
declarations, 40 have generics and 148 are local items. Its source-set digest is
`7a8a9f89e332e1c56e74b3383acc98aaf01f4a6edd207c2861ce85d1293c8279`.
The expression-only `irohad/src/main/online_peers_provider.rs` include fragment
remains a parse gap in physical-file mode; the scan exits nonzero. These counts
include alias hints and inactive configurations, not compiler-confirmed types.
The compact review queue is retained under
`target/architecture-redesign/norito-syntax-codec-review-queue.json`.

The inventory's `--crate-root` mode retains each module/include occurrence,
raw cfg predicates and unresolved expansion sites while sharing physical file
records. Its 32 tests include five independent compiler checks for ordinary
modules, explicit paths and include-relative resolution. The model/crypto graph
visits 953 contexts across 367 physical files and resolves 951 edges. Three
dynamic include arguments in the model crate root remain explicit access gaps;
macros and attribute expansion are review records, not simulated expansion.
The graph source-set digest is
`b23217a8f1b81ee58fc2ecabd0fe8d5abf715792af18910a064d17ee9eae9abd`.
Physical-file mode and crate-root mode are mutually exclusive. Function-local
include fragments retain their occurrence and bytes but remain parse review.

### Compiler capture

`scripts/norito_schema_capture.py` prepares an isolated source snapshot and
runs one explicitly selected library or integration-test harness. Preparation
reuses the authoritative build profiler's source-copy and sealing primitives.
Initialized Git links are expanded into explicit dirty file inventories, with
indexed and checked-out revisions recorded. Absent links remain recorded.
Only the copied Norito derive entry points append capture probes; their
original codec generation bodies and the shipping source are unchanged.

Each registered nongeneric probe reports the actual compiler `type_name`,
the existing directional codec hash and its root-name hint. A mismatching hint
remains unresolved evidence. Generic declarations emit review records without
invented instantiations. Function-local tests are not registered by Rust and
remain inventory gaps; source lint policy is retained. Eight generator tests
pass, including an independent compiler fixture covering private, generated,
raw-identifier, generic and unregistered local cases.
For prepared nongeneric declarations, probes additionally check the explicit
identity against the still-active codec; this check is for the preparation
phase before physical moves and atomic codec cutover.

Seventeen driver tests cover source isolation, dirty Git links, drift rejection,
strict collection and preservation of prior evidence. Each actual run retains
the command, environment digest, source seals, concrete Cargo/rustc identities,
Cargo's test artifact, harness digest and transcript. Both compiler wrappers
are explicitly disabled, including configuration overrides. Each run builds
into a fresh target with `--no-run`, then directly invokes the sealed harness;
Cargo runner configuration cannot substitute the test process. Postchecks
also reject driver or capture-manifest drift. The collector requires
one successful harness and a record for each executed probe. This evidence
does not prove declaration coverage, wire-frame equivalence, or release
qualification. Remaining manual/generic/macro and feature/target gaps require
review and the immutable fixtures before the codec cutover.

```sh
python3 scripts/norito_schema_capture.py prepare --out /absolute/external/capture
python3 scripts/norito_schema_capture.py run \
  --snapshot /absolute/external/capture --name crypto-lib --package iroha_crypto
```

The controlled crypto library capture passes 206 probes: 110 concrete nominal
identities and two generic review records. All captured root hints match the
existing directional hashes. It binds source digest
`022600a39250a065c28d88855a141ce2587758928f6a66d15d9a7f79cd11fcf7`
and report SHA-256
`fb59df9e9407f74b2ec39d972ebcd1ebeb75ef78c52eda49b9dfec50793333ca`.
The earlier pilot lacked explicit wrapper controls; its retained records match
the controlled run exactly, but the controlled report owns this evidence.

The controlled model library-test capture passes 6,112 probes, yielding 3,121
concrete nominal identities and 34 generic review records. All root hints match;
54 directional records have explicit root projections. It uses the same frozen
source and report SHA-256
`be82d3661d9e2a79fd1a60d6922f1387d3821a0ad5a65d80d294aca1251caedd`.
Its resolved Cargo features, including `transparent_api` from the test graph,
are retained in the report. It does not qualify other feature combinations.
The source intersection matches 2,556 ordinary declaration sites; 842 generated
or unmatched observations and 288 observations with stale/missing physical
inventory remain separate review records. A reviewed 1,771-declaration ordinary
batch is applied across 107 files. Its 3,467 captured directions comprise 1,696
bidirectional types, 68 serializer-only types and seven decoder-only types.
Module-owned immutable fixture suites preserve those codec capabilities and
original cfg scopes. All 110 owner-scoped tests pass on the sealed
post-declaration harness, and a second controlled 6,112-probe capture is exactly
equal to the pre-declaration capture after excluding source coordinates. A
wider sealed model run completed 3,045 passes and exposed 37 failures that
still require disposition. That historical run predates the finite Native AMX
participant-settlement wire repair; therefore it does not qualify the complete
model suite.

### Batch declaration migration

The inventory counts derive sites, not distinct types. Insert one independent
`NoritoSchema` implementation per logical declaration; do not independently
generate identities from both Encode and Decode. A one-time syntax-aware source
tool can perform verified insertions without weakening the explicit contract.

Inventory package/source spans, inclusion contexts and cfg predicates. Resolve
derive aliases and follow module, path and include declarations, grouped test
roots and supported macro grammars. Unknown expansions, repeated inclusions and
ambiguous owners require review. Proposed names reconstructed from paths are
not evidence: isolated capture probes must obtain actual names and both codec
hashes from the pinned compiler before the tool inserts any literal. Source
hash checks, stale-source rejection, idempotence and duplicate-impl rejection
are mandatory. The runtime has no inferred-name fallback.

`scripts/apply_norito_schema_identities.py` implements the read-only patch stage
for reviewed ordinary struct/enum mappings. It validates the complete batch,
exact UTF-8 spans, original source digests and explicit names before emitting
one diff. Its 25 tests cover stale/ambiguous input, idempotence and actual Git
patch application with CRLF and missing final newlines. It deliberately rejects
unresolved contexts and visible alias/wildcard cases. External semantic
resolution and compiler checks still own trait-implementation uniqueness; this
helper is not a compiler capture or coverage proof.

| Generator | Required declaration input |
| --- | --- |
| `model` / `model_single` | Preserve explicit attributes through ordinary and FFI expansions; capture actual private `model` names. |
| `declare_versioned` | Explicit identity for the generated enum; version aliases share their underlying identity and acquire no duplicate implementation. |
| `EventSet` | Separate explicit identity for the generated `*Set`, alongside the parent event identity. |
| `RegistrableBuilder` | Each of the two production callers supplies a required literal identity for its generated `New*` child. The generator has no fallback and rejects missing, duplicate, unknown, non-literal and invalid metadata. Eight generator tests, three compile-fail cases and 23 asset/NFT owner tests pass. |
| `queries!`, `isi!`, `isi_box!`, `enum_type!`, `data_event!` | One identity derive in each template, with captured literal attributes at declarations/invocations. |
| Musubi digest/text and governance hash macros | Required literal arguments at each invocation, without source-path concatenation. |
| `EnumRef` | Explicit child identities only for existing codec or typed-marker uses; no new wire surface for ordinary helpers. |

Generic constructors require representative compiler captures preserving type,
const and erased lifetime order. Projection overrides, private/local borrowed
writers, foreign containers and marker-only types retain an explicit review
queue. Complete the batches in dependency order: Norito containers, primitives,
crypto, independent wire crates, aggregate models, then runtime/SDK consumers and
fixtures. Preparation must leave active frame bytes unchanged. Only after
coverage and fixture verification may the payload/typed-trait and codec-generator
cutover occur atomically; compile upward through supported features and targets.

## Required identity families and acceptance

- Prepared 19 primitive identities cover ConstVec, UniqueVec, SmallStr/SmallVec,
  ConstString, Json, BigInt, Numeric, Quantity, XorQuantity, NumericSpec, the three
  numeric ABI values and five private JSON/numeric wire helpers. SmallVec
  composes its array parameter, including capacity, independently of its
  Vec-shaped payload. The borrowed BigInt view retains its distinct nominal
  identity while projecting to BigInt at a frame root. Numeric ABI names/hashes
  and zero-flag fixed-width frames remain specified in
  `iroha_primitives/src/numeric_abi.rs:18–34`.
- Prepared MerkleTree, MerkleTreeCommitment, MerkleProof and CompactMerkleProof
  identities compose markers without payload or JSON codec bounds. The compact
  proof has no binary Norito codec; its identity and existing JSON/full-proof
  projection are tested without adding a new binary wire surface. Remaining
  cryptographic codecs and private borrowed writers still need declarations.
- Algorithm, PublicKeyCompact, PublicKey, ExposedPrivateKey and Signature now
  have explicit captured identities. The immutable fixture contains 73 records
  across 25 nominal identities, including all 11 available algorithm tags,
  generic frames and deterministic signed envelopes. Seven focused tests pass;
  they also reject truncation, substituted schema hashes and cross-decoding of
  distinct named roots that share a bare payload. The capture writer is removed.
  SHA-256: `27fd72e39a96118a8a32768ab08948c87db0b2158d854d1086adbf69bf014c88`.
- Another 108 ordinary crypto declarations now use compiler-captured nominal
  identities. Nine module-owned tests pass against 201 directional hashes in
  `captured_codec_schema_identities.json` (28,481 bytes; SHA-256
  `948233b20dd7f481885f8e0372398394667d02d6b661efab76f10c5398179d3b`).
  The source spans and names match the isolated capture exactly. This is
  identity/header evidence, not complete value/frame coverage. Seven existing
  crypto identity/wire-golden tests also pass after the declarations. One historical
  test type in a wildcard-import scope remains outside the ordinary batch;
  generic, manual and function-local writers still require explicit review.
- All 19 planned foundational model types have explicit captured identities.
  Two immutable tests pass both before and after declarations: 189 frames across
  105 nominal identities, 24 JSON/schema samples and 10 storage-key samples
  remain exact. The tests preserve signatures and scoped network formatting.
  The [extraction inventory](model_base_extraction.md) records their ownership
  closure and fixture digest. Physical moves still await the atomic cutover.
- Preserve forwarding projections such as `Owned<T>` (`data_model/src/common/
  mod.rs:92`), InstructionBox's tuple frame (`isi/mod.rs:812`), and SoraFS
  borrowed signing views in orderbook, governance, por, potr, provider_advert and
  pop_credentials. Do not confuse a validated public type with its decode-only
  wire twin merely because their fields match.
- Capture actual nominal names, both codec-direction hashes and complete frames
  before model moves. The existing schema exporter is a seed, not exhaustive
  coverage. The initial kernel has 57 Norito and two crypto frame goldens;
  `Vec<&str>` and `Vec<Cow<str>>` currently lack the required higher-ranked
  decoder, so their recorded decode hashes are explicitly null. Status has a
  separate 32-type pre-move fixture. These fixtures do not qualify all models.
- Require explicit declarations in codec derives; add compile failures for
  missing identities, missing generic/marker bounds, duplicate declarations and
  collapsed generic projections. Keep positive object-safe payload and
  marker-only wrapper tests. Close manual and generated families before the
  active transition; do not suppress unresolved items with feature reductions.
- At cutover, replace every schema selection, including nested Option payload
  contexts (`norito/src/core.rs:3679`), core frame writers/checkers from `:6814`,
  framed decode entry points, streaming Vec/map readers (`lib.rs:9196`, `9222`,
  `9808`, `10363`, `10388`), and SignedBlock's canonical envelope checks
  (`iroha_data_model/src/block/mod.rs:1452`, `1493`).
- Require unchanged complete frames and bare payloads, including compressed and
  uncompressed input, bounded/streaming decode, root projections, signed blocks
  and numeric pointer ABI. Relocated generic headers must then match; the
  current preparation tests deliberately show that active headers still differ.
  Wrong headers, argument mismatches and truncation must reject. Run cross-SDK
  fixtures and ABI goldens without silently authorizing changed wire bytes.
- Require all supported workspace targets and feature selections to compile.
  Existing unrelated Core grouped-governance fixture failures remain unresolved;
  the integration core_api target now compiles, while its replacement
  four-validator configuration scenario still needs runtime validation. Finally,
  compare source-sealed per-unit build profiles; these preparation tests do not
  establish build-memory improvement or release qualification.

## Prepared wrapper fixtures

The primitive fixture at
`crates/iroha_primitives/tests/fixtures/schema_identity_frames.json` captures
23 complete pre-declaration frames, both available codec-direction hashes, and
actual compiler nominal/root names. Three private borrowed helpers serialize
only, so their decode-hash fields are explicitly null. The separate numeric ABI
fixture captures three complete fixed-layout frames; tests preserve zero flags,
roundtrip each numeric domain and reject the wrong domain header. The two
SmallVec capacities share payload bytes but must reject each other's headers.

`crates/iroha_crypto/tests/fixtures/merkle_schema_identity_frames.json` captures
four complete pre-declaration frames: tree, proof, commitment and a typed root
hash. Marker-only tests prove nominal composition and unchanged bare payloads
across equivalent declarations while explicitly retaining the pre-cutover
active-header difference. These files were recorded before adding the wrapper
identity implementations; capture writers were removed after verification.

Focused validation uses the existing `architecture-sdk` target slot with locked,
offline Cargo and no incremental compilation:

```sh
scripts/cargo_fast.sh --target-slot architecture-sdk --stable-local-metadata --no-incremental -- test --locked --offline -j 2 -p iroha_primitives --lib schema_identity
scripts/cargo_fast.sh --target-slot architecture-sdk --stable-local-metadata --no-incremental -- test --locked --offline -j 2 -p iroha_crypto --test iroha_crypto_group_01 schema_identity
scripts/cargo_fast.sh --target-slot architecture-sdk --stable-local-metadata --no-incremental -- test --locked --offline -j 2 -p norito_derive --features trybuild-tests --lib --test strict_json schema_identity
```

The derive emits Rust's standard `automatically_derived` marker; strict-lint UI
coverage includes lifetime-only uses so wrapper crates can retain
`deny(warnings)`. Complete generated/manual identity closure, the atomic active
codec transition, all-feature qualification and source-bound memory comparison
remain pending.
