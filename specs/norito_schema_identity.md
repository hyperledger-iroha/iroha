# Canonical Norito schema identity

This is the reviewed first-release cutover design. The identity kernel and its
initial fixtures are implemented; **active codec cutover and complete type
coverage remain pending**. Implementing `NoritoSchema` alone does not change
existing headers.

The concrete model fixtures now preserve seven owned records and two
encoding-only projections: Action, DataEvent, the three execution contexts,
block subscriptions/messages, the block-send adapter and reputation event-ID
material. Their 52 root/container frames and three adapter projections are
documented in
[`model_concrete_identity_frames.md`](../crates/iroha_data_model/tests/fixtures/model_concrete_identity_frames.md).
The borrowed adapters retain their owning decoders and exact projected bytes.

DataEvent assigns explicit wire discriminants to all variants. Governance
reserves 19 even when disabled; Social, Bridge and GameSession retain 20, 21
and 22. A normal-dependency consumer exposed GameSession shifting to 21 when
governance was disabled. The canonical fixture remains the governance-enabled
capture, shared by every feature selection for common variants; disabled
capabilities reject their reserved variant. No alternate old tags are accepted.

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

`SerializePayload` now owns object-safe serialization and encoded-size methods.
Bounded writers, packed fields and borrowed model/Core adapters use that
contract. Bare `Encode` and sequence containers accept payload-only children;
framed writers retain `NoritoSerialize`. The typed derive emits both contracts,
while `#[derive(SerializePayload)]` emits only payload serialization and rejects
frame-schema attributes. Manual implementations and qualified calls use the
same ownership boundary; no old payload-method alias remains.

`DeserializePayload<'a>` owns `deserialize` and `try_deserialize`, reconstructing
values within the active bounded payload context. `NoritoDeserialize<'a>` is the
typed marker above that contract and retains only the active frame hash. The
`NoritoDeserialize`/`Decode` derive emits both implementations;
`#[derive(DeserializePayload)]` emits only reconstruction and rejects
`#[norito(schema_name = "...")]`. Payload-only records and their containers
cannot acquire a typed frame decoder from their reconstruction implementation.
Manual implementations and qualified reconstruction calls use the payload owner.

Public `IdBox` and `BlockSignature` retain one typed frame API each, with their
compiler-observed nominal identities and 24 immutable root/container frames.
`IdBoxCandidate` and the block header/signature tuple adapters expose only the
payload contracts. `IdBox::try_deserialize` delegates to its carrier's fallible
method, preserving unknown/truncated-tag errors without panicking. The
[public-owner fixture contract](../crates/iroha_data_model/tests/fixtures/public_frame_owner_identities.md)
records the exact captures and validation scope; these declarations do not
activate the pending global identity transition.

The remaining atomic identity transition is:

1. `SerializePayload` and `DeserializePayload<'a>` retain their respective bare
   serialization and reconstruction responsibilities.
2. `NoritoSerialize: SerializePayload + NoritoSchema` is the typed contract,
   implemented once for types satisfying both. Erased bare writers use
   `dyn SerializePayload`; typed generic callers inherit the identity bound.
3. `NoritoDeserialize<'a>: DeserializePayload<'a> + NoritoSchema` is the typed
   decoding contract. Reconstruction remains solely on `DeserializePayload`.
4. Both codec directions remove their independent `schema_hash` methods. Every
   typed frame writer/checker uses the fixed helper. Update framed callers,
   derives, manual implementations and qualified calls together. No legacy
   hash or reconstruction-method forwarding remains.

This keeps allocation-conscious bare streaming while preventing a type without
an identity from entering a typed frame. `schema-structural` remains explicit
inspection data, not a feature-selected active header digest.

Until that transition, the active typed codec directions retain their existing
`schema_hash` implementations. They are not yet bound to `NoritoSchema`.
Bare `Decode` requires `for<'de> DeserializePayload<'de> + SerializePayload`;
canonical field/container decoders use the payload contracts for reconstruction
and byte comparison. Exact slice helpers combine `DeserializePayload` with
`DecodeFromSlice` and do not require a frame identity. Typed frame consumers
explicitly retain their `NoritoSerialize`/`NoritoDeserialize` bounds; bare
`Encode`/`Decode` bounds do not imply those contracts. Option fields use the
shared canonical decoder without inventing
a nested frame schema, retaining advertised flags and resource limits.
Tuple and result slice decoders enforce exact child consumption with runtime
errors. Both result branches guard child nesting and restore the previous depth
before subsequent decodes in the same limit scope. Unit size diagnostics use
the shared layout calculation, including the existing packed zero-offset table;
canonical field/frame comparison continues to validate that metadata.

The actual `MultisigPolicy` and `MultisigMember` owners derive binary decoding
with the fallible
`#[norito(validate = "path")]` hook. It consumes the reconstructed owner and
returns `Result<Self, norito::Error>` once per reconstruction, preserving typed
errors and the existing strict constructors. Their private `PolicyFields` and
`MemberFields` carriers retain only JSON decoding and acquire no binary frame
identity. Moving the generated reconstruction implementation to
`DeserializePayload` retains this validation behavior and one reconstruction
owner. The atomic identity transition and remaining owner/feature
qualification are still pending.

## Streaming wire declaration checkpoint

The integrated batch closes declarations for all 84 current production binary
owners in `norito::streaming`: 70 root records and 14 `codec` records. Each
explicit nominal identity is captured from the pinned compiler and checked
against both existing codec directions. The immutable
[`streaming_wire_identities.json`](../crates/norito/tests/fixtures/streaming_wire_identities.json)
and [`streaming_wire_identity_frames.json`](../crates/norito/tests/fixtures/streaming_wire_identity_frames.json)
retain those observations and 396 complete/bare frame records across 132
populated values and their root, Option and two-element Vec forms. Root enum
variants are covered. Header checks include the actual required alignment
padding, zero padding bytes, advertised flags and exact payload length;
decode/re-encode comparisons preserve the captured bytes.

The implementation and its large test module move into `streaming/codec.rs`
and `streaming/codec/tests.rs`, retaining the original module namespaces,
visibility, declared identities and every existing assertion. Final default Norito tests pass 1,308 cases, and
`--no-default-features --features base-codec --lib` passes 456 tests; each
selection has one existing ignored case. Strict
`cargo clippy -p norito --tests --no-deps --locked --offline -- -D warnings`
also passes. All three runs retain the same 19,297 final candidate inputs
unchanged, with no stack override. The integrated code/budget paths match those
qualified inputs. A fresh rerun of eight settlement cases and the removed-layout
case also passes on that seal, without a stack override; the owning live model
and hash sources match the candidate. This includes the populated 4,096-source
schema/codec/hash/drop regression.

A subsequent isolated structural-feature check exposes an existing enum derive
mismatch: serialization selects the structural hash, while deserialization
retains the nominal default. The unfixed runtime regression fails at directional
hash equality. The integrated correction emits the same existing hash-selection
body in both directions, retaining explicit `schema_name` projections and
unchanged default-mode identities. Its regression checks high-level, reader,
core archived/slice and ArchiveView paths, including wrong-header rejection.

The streaming guards compare all 84 declared nominal identities to the
unchanged captures independently of the active hash mode. Default mode still
compares all 396 complete frames; the structural branch compares populated bare
bytes, advertised flags and alignment while checking active directional/header
equality, exact roundtrips and malformed-frame rejection. Runtime qualification
of these streaming guards is limited to default/base builds: the structural
library selection cannot yet compile. The captured rANS checksum stays fixed in
the fixture value, and default mode verifies the original checksum calculation.
These fixtures validate wire representation, not signed-table authorization.

The final combined default run passes 1,380 tests: 1,309 Norito, 57 derive-library
and 14 strict JSON cases, with one existing ignored case. Base-codec library
tests pass 456 cases with one ignored. Strict Norito `--tests --no-deps` and
isolated derive-library `--lib --no-deps` Clippy both pass with warnings denied.
The derive cleanup removes unused `Debug` from private attribute records,
retaining all parser diagnostics through explicit error extraction at 11 test
sites; no dependency or dependency feature is added. All final checkpoints bind
`qualified-correction-inputs.json`, with 19,297 unchanged inputs and no stack
override. These results are distinct from the earlier 1,308-test checkpoint;
the integrated correction paths match the qualified candidate exactly. The final Native AMX
selection also passes all 32 tests, with no failures or ignored cases and no
stack override on that same seal. It includes the populated 4,096-source
schema/codec/hash/drop and removed-recursive-layout regressions. The
`qualified-correction-native-amx-checkpoint.json` checkpoint binds this result
and exact live model/hash owner correspondence; the earlier nine-case run
remains separately bound to its prior seal.

Only the six focused schema integration tests ran successfully under
`--no-default-features --features base-codec,schema-structural`, with zero
failures or ignored cases on that same final seal. The complete structural
group diagnostic remains failed: 109 pass and ten fail, comprising eight
compression assumptions when compression is disabled and two existing nominal
streaming-ticket goldens. The separate structural library selection fails
compilation with 24 errors across 15 test owners. In particular, payload-only
generic decoder regressions must retain their schema-free leaves; adding
frame/schema bounds would weaken their contract. Closing that coupling requires
the planned identity separation. No tests are disabled to obtain a pass. The
atomic trait transition, remaining owner/feature closure and physical model
extraction are still pending.

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
| `model` / `model_single` | Preserve explicit attributes through ordinary and FFI expansions; capture actual private `model` names. The 12 Nexus `model_single!` records have their own captured declarations and complete populated frame/carrier fixtures. Remaining callers and features still require closure. |
| `declare_versioned` | Explicit identity for the generated enum; version aliases share their underlying identity and acquire no duplicate implementation. |
| `EventSet` | All 25 production `*Set` types and the test caller supply a required literal child identity. The generator derives that identity once, independently of the parent, and shares strict metadata parsing with `RegistrableBuilder`. |
| `RegistrableBuilder` | Each of the two production callers supplies a required literal identity for its generated `New*` child. The generator has no fallback and rejects missing, duplicate, unknown, non-literal and invalid metadata. Eight generator tests, three compile-fail cases and 23 asset/NFT owner tests pass. |
| `data_event!` | All 12 generated parent enums and their set children have explicit captured identities; the template emits one parent identity derive. |
| `queries!` | All 127 concrete query declarations supply captured literal names; the template emits one identity derive and requires each declaration's identity. |
| `enum_type!` | All 12 current instruction enum callers supply captured literal identities; the template emits one identity derive. The compiler exposed two callers absent from the historical review, and both received their own pre-declaration capture. Private discriminators keep their visibility. |
| `isi_box!` | All 12 current callers supply captured literal identities; the template emits one identity derive. Exhaustive matches and immutable fixtures cover all 68 variants, including populated generic instruction payloads. |
| `isi!` | All 292 current declarations supply captured literal identities; the template emits one identity derive. The 283 nongeneric records and 39 instantiated generic forms preserve 357 values and 1,428 complete frames. All 325 record tests and the 12-argument marker test pass. |
| Musubi digest/text/page macros | All 12 digests, three bounded-text wrappers and two page wrappers supply captured literal identities. Each template emits one identity derive. |
| Governance hash macros | All 16 handwritten-codec wrappers supply captured literal identities; the generator emits one independent identity derive and retains the codec bodies. |
| Privacy/spentness carrier macros | All 62 generated types supply required captured literals; each of the four templates emits one identity derive without changing constructors, codecs or validation. |
| `EnumRef` | Explicit child identities only for existing codec or typed-marker uses; no new wire surface for ordinary helpers. |

The historical generated-family review is a capture subset. A current source
inventory finds 292 `isi!`, 12 `isi_box!` and 12 `enum_type!` invocations,
compared with its 144, ten and ten named observations respectively. The two
additional enum callers now have separate compiler captures. The current
instruction callers now have their own capture and populated value coverage;
the historical counts must not be treated as closure.
The untracked `target/architecture-redesign/current-instruction-macro-inventory.json`
records source hashes and that intersection without inferring missing names.

A fresh local test capture now records 433 compiler identities, including every
current instruction registry entry, all nongeneric `isi!` and `isi_box!` callers,
39 concrete generic instruction forms and their model markers. Existing tests
supplied 386 distinct values across 303 concrete types; the initial intersection
exposed 51 nongeneric instruction records missing value fixtures. The
subsequent box capture adds deterministic generic values and closes all box
variants. A further capture fills all 51 record gaps from typed fixture values
before the declarations change. The [record fixture](../crates/iroha_data_model/tests/fixtures/instruction_record_generated_identity_frames.md)
now covers all 322 current instantiated record types and preserves 1,428 frames.
Neither the registry nor the historical capture is used as a proxy for this
coverage. Twelve additional generic argument markers retain their captured
nominal names; both existing codec directions match the independent identity.
The source hashes and original captures remain under untracked
`target/architecture-redesign/instruction-macro-current-before/`.

The event family is qualified locally by 27 derive unit tests and 21 integration
tests, including the compile-fail cases. The 25 set types preserve all 225
pre-declaration root/`Vec`/`Option` frames and 75 JSON values; the 12 generated
parent enums preserve captured nominal names and both directional hashes. The
[fixture record](../crates/iroha_data_model_derive/tests/fixtures/README.md)
records the capture digest and scope. Full-suite validation also removed the
utility parser's duplicate-attribute tolerance and replaced the stale numerical
direct-FFI check with the exact reviewed declaration inventory. These checks do
not qualify FFI expansion, native execution or other feature selections.

The 17 Musubi generated types preserve 196 pre-declaration frames across 49
values, including text bounds, full-byte digests and populated pages. Root,
`Vec`, `Option` and `BTreeMap` frames, both directional hashes and JSON are
checked against the immutable [capture record](../crates/iroha_data_model/tests/fixtures/musubi_generated_identity_frames.md).
This remains preparation: no active codec or schema identity dispatch changes.

The 16 generated governance hash wrappers also preserve all 256 captured frames
and 64 JSON values. Their handwritten codecs were outside the derive-probe
inventory, so the unchanged generator received a separate direct compiler/frame
capture. Its actual `parliament_types` nominal names and both active hashes now
match the explicit declarations. The [capture record](../crates/iroha_data_model/tests/fixtures/governance_generated_identity_frames.md)
documents the source and fixture digests; the permanent test has no writer or
source-path inference.

The 127 generated query types preserve all 684 captured frames across 171
payloads, with JSON and frame roundtrips. Coverage includes both alias scopes,
optional anchors and all query-filter enum variants; the focused query suite
passes 28 tests. The [query capture record](../crates/iroha_data_model/tests/fixtures/query_generated_identity_frames.md)
records immutable fixture and pre-declaration source digests.

The [generic query captures](../crates/iroha_data_model/tests/fixtures/query_generic_identity_frames.md)
add 96 default-feature and 116 `ids_projection` frames. Five generic owners,
four concrete query records and two typed-hash markers now declare their
captured nominal identities. Permanent tests preserve complete frames, marker
composition and membership decode budgets. The default query selection passes
202 tests and `ids_projection` passes 203, with zero failures or ignored tests
and 5,109 unchanged selected inputs in each run. Active codec dispatch and
schema export behavior remain unchanged.

The [generic model fixtures](../crates/iroha_data_model/tests/fixtures/model_generic_identity_frames.md)
preserve another 68 complete frames and five actual FHE signing preimages.
MetadataChanged, Validate and Mismatch compose captured nominal argument names;
six concrete argument owners and the borrowed FHE preimage now declare their
identities. InstructionBox preserves its existing root projection to the wire-ID
and framed-payload pair while containers retain its nominal instruction identity.
The FHE helper exposes encoding only; its unusable borrowed Decode derive is
removed. All five model and four adjacent query identity tests pass on a rebuilt
default-feature artifact with 5,613 selected inputs unchanged. These checks do
not qualify other features or change active framing dispatch.

The 62 generated privacy/spentness carriers have a separate
[capture record](../crates/iroha_data_model/tests/fixtures/privacy_generated_identity_frames.md)
covering 248 payloads and 992 complete frames, including canonical six-lane
Goldilocks boundary values. Raw Ristretto carrier fixtures exercise byte
preservation only; native point validation remains a separate protocol check.
All four generated-model fixture suites pass together locally after these
declaration changes.

The instruction enum generator has one required identity derive for all 12
current callers. Its [capture record](../crates/iroha_data_model/tests/fixtures/instruction_enum_generated_identity_frames.md)
preserves 57 numeric tags and JSON values with 228 complete root and container
frames. Every possible byte tag is checked, and private discriminators remain
private. The two additional mint/burn callers received their own pre-declaration
capture without rewriting the earlier ten-type fixture. Both fixture suites
pass after the declarations; active codec bodies are unchanged.

The 12 Nexus `model_single!` instruction records preserve 120 captured frames
across 24 populated values, including root, container and `InstructionBox`
carriers. All nine Nexus instruction tests pass after their declarations.
Eleven records have no direct JSON codec; their supported instruction-carrier
JSON is captured without adding an extra wire API. Withdrawal also retains its
direct JSON fixture. The [Nexus capture record](../crates/iroha_data_model/tests/fixtures/nexus_instruction_generated_identity_frames.md)
binds the exact pre-declaration source and fixture digests. Synthetic proof
payloads establish codec preservation, not proof admission or finality.

The 12 generated instruction-box types now have required identities and an
immutable [box capture record](../crates/iroha_data_model/tests/fixtures/instruction_box_generated_identity_frames.md).
It preserves all 68 variants with 340 complete frames and 68 instruction-carrier
JSON values. The tests use exhaustive Rust variant matches and compare every
captured field; no writer or inferred identity remains in the model source.
That instruction library selection passed 336 tests after the box declarations.
The subsequent record suite passes 325 tests and the generic-argument test passes
for all 12 added markers. The current file-budget check reports 231 findings;
all newly added instruction fixture modules are within their existing limits.

The complete model `group_02` harness passes 178 tests, with two fixture writers
intentionally ignored. Its Soracloud quota tests now keep placement cardinality
consistent, check the pre-placement source quota, and prove acceptance at the
exact aggregate storage ceiling before rejecting an excess. Production
admission and codec implementations were not changed by that fixture repair.
At that checkpoint the file-budget check reported 232 findings; these
query/privacy production and test files were each within their existing limits.
All four focused Goldilocks library tests also pass, including noncanonical
field rejection through the fixed-width binary and JSON decoders. Strict
model-library Clippy remains unresolved across the wider model source; this
checkpoint does not claim a passing strict-lint or workspace qualification.
Its current run reports 130 diagnostics, including documentation, oversized
functions and comparison-grouping review. Exact primary spans and lint codes
are retained in the untracked
`target/architecture-redesign/model-clippy-query-privacy-review.json` for the
next scoped fixes; no lint suppression or file-budget exception was added.

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
- `Owned<T>` now declares its compiler-captured nominal constructor and forwards
  `T::frame_name()`, including nested projections without codec bounds. Its
  [storage fixture](../crates/iroha_data_model/tests/fixtures/owned_storage_identity_frames.md)
  preserves 432 complete frames across nine values and 40 nominal identities;
  account/NFT/RWA storage records have their captured declarations too.
  Active codec selection remains unchanged by these declarations.
  Metadata's private `MetadataEntryRef` already implements `SerializePayload`
  for unframed borrowed fields. It needs no identity or further migration at
  the atomic cutover; it never enters typed framing.
- Preserve remaining forwarding projections such as InstructionBox's tuple
  frame (`isi/mod.rs:812`), and SoraFS
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

## Manual public frame ownership

Sixteen additional manual model owners now declare their actual captured
nominal identities. X.509 key usage retains its explicit boolean root
projection and distinct wrapper identity inside containers; every other owner
retains its own root identity. The complete fixtures and projections are
documented in the [public frame contract](../crates/iroha_data_model/tests/fixtures/public_frame_owner_identities.md).

`iroha_data_model` has one explicit `manual_frame_identity` integration target
for these public contracts. It also owns the existing IdBox and BlockSignature
assertions, preserving all 144 immutable frame records with one shared binary
checker and JSON checks only where supported. The seven tests pass after the
declarations and relocation, including direct fallible malformed-input checks,
on 19,313 unchanged default/HTTP inputs without a stack override. Prior full
library qualification remains tied to its separate preceding source seal.

The subsequent query/time stage adds fifteen declarations and two permanent
tests containing 181 unchanged pre-declaration frames. QueryBox remains
specialized to its existing aggregate output; QuerySignature forwards only its
root projection to SignatureOf<QueryRequestWithAuthority>. Query reconstruction
candidates have payload contracts only, and the redundant private TimeInterval
carrier is removed. The public target now passes nine tests and preserves all
325 frames, with 19,317 unchanged inputs and no stack override. Its callback
checker preserves complete semantics without expanding the public query API.

A further nine time-event and query-parameter declarations preserve 97 actual
pre-declaration frames. Schedule and fetch hints retain their structural codecs;
Core and service admission retain their execution policy. All 11 public tests
preserve 422 frames, and the same 19,321-input source passes 256 model, 33 query/SM
integration and one allocation test without a stack override. The single
combined build reports no warnings after removing an unused framing-trait
import. These counts have their own source seal in the public frame contract.

The next event stage adds 196 owner declarations and 103 fresh captured frames.
Four slice decoders now reconstruct the complete owner without dropping the
message envelope or resetting caller layout context. Nine direct regressions
retain typed budget errors, exact consumption and context restoration. All 21
public tests preserve 525 frames; the same 19,341-input source passes 329 model,
33 query/SM and one allocation test on the default stack. The complete Native
AMX include closure retains 104 functions and 75 tests after moving settlement
tests to their topic module. All 32 Native AMX cases pass. Source-size findings
fall to 235 with 173 unchanged exceptions. Strict Clippy stops at the model
library with 225 diagnostics, leaving the test target's strict lint unqualified.
See the event section of the public frame contract for precise scoped evidence.

Remaining generated/manual declarations and the atomic transition from
directional hashes still require completion before physical model moves. These
stages change no active frame-selection algorithm, codec layout, ABI version,
dependencies or optimization setting. Full workspace/release and strict-lint
qualification remain separate requirements.
