# FASTPQ public artifact boundary

Source contract updated 2026-09-08. The model codecs in
`crates/iroha_data_model/src/fastpq/public_artifact.rs` describe unverified public
transport data. They register no qualified compact profile. Production proof
admission still uses the raw replay verifier.

## Statement and identity

`FastpqPublicTransferStatementV1` carries the six model public inputs, the
separate ordering hash, the complete transition table and ordered public
transcript occurrences. A public delta retains exact quantities and account and
asset identities without SMT paths. Projection preserves duplicates, order,
optional digests and all supplied quantity precision; semantic preparation and
authentication are separate operations. The prover aliases these public types;
its prepared AIR context encoding remains unchanged.

Ordinary and AXT artifacts use distinct nominal Norito schemas. Each carries a
profile identifier, a public statement and an opaque inner bundle frame. AXT
also carries its complete binding, exact execution metadata, outer mirrors and
ordered remote claim preimages. Optional remote claims distinguish absent from
present-empty. Decoding does not establish the authority or finality of any
advertised source state.

An identity description explicitly distinguishes the statement digest, complete
wrapper digest, inner bundle digest and wrapper byte length. Its commitment
variant distinguishes a legacy preprocessing root from ordered compact AIR row
roots. Every root retains six canonical Goldilocks words, including the sixth;
the decoder rejects words at least the field modulus. The advertised segment
count and roots are untrusted claims until checked against bounded proof parsing.

`iroha_core::fastpq::batch_and_public_statement_from_transcripts` now returns the
private batch and a separate owned public statement. It projects finalized
in-memory transcript occurrences before private metadata serialization and
includes complete sorted rows and their recomputed ordering hash. The legacy
factory uses the same construction sequence with a no-op projection callback.
Neither factory mutates the caller's captured transcripts. Existing SMT
attachment can replace stale repeated-key quantities with chained quantities
and supplies touched-tree roots; the public output describes that produced
relation, not an attestation that captured quantities or finalized state match.
The producer does not select a compact profile or apply its admission limits.

## Resource and routing rules

Offline artifact decoding requires an explicit expected profile and explicit
wire, opaque bundle and Norito limits. Profile equality is a caller filter,
not a qualification registry. The raw wire cap precedes header inspection;
Norito enforces canonical framing, exact consumption and field, sequence,
allocation and depth budgets. An enclosing decode scope retains cumulative
charges across sequential child decodes. The separate opaque bundle cap does
not parse a proof or authenticate its contents.

At legacy AXT ingress, `fastpq_prover::artifact_dispatch` checks the raw cap
first and rejects either recognized compact schema after reading only the
fixed 40-byte header. It performs no body decode, checksum scan or proof hash
for that rejection. Invalid and unknown headers retain the original strict
legacy decoder and error policy. There is no compact acceptance branch.

Lane proof output now encodes canonical Norito bytes before deriving its byte
digest. Kura snapshot batches and Torii recovery batches likewise select the
canonical layout; Torii retains its bounded two-pass encoding and existing
per-batch and cumulative byte limits. These are serialization corrections,
not new compact persistence or admission support.

## Offline verifier mapping

The normal-library `fastpq_prover::offline_compact` facade connects canonical
quantity model wrappers to the sole [six-lane V1 verifier](fastpq_compact_protocol_contract.md).
It derives its metadata profile ID from the complete fixed
`QuantityArtifactProfileV1` description, never accepts a proof-selected
implementation, and compares all seven PublicIO fields with independently
supplied expectations. The AXT entry point additionally compares the complete
binding, original metadata, pre-proof mirrors and ordered remote preimages.
The caller must authenticate those expectations from authoritative state.

One enclosing Norito budget accumulates transport, inner carrier and every child
proof decode; stricter outer scopes remain effective. The verifier retains each
complete AIR row commitment from the same bounded decode and publishes the
ordered list only after every child verifies. Its private-field result recomputes
canonical statement, wrapper and exact inner-bundle digests using Iroha `Hash`.
The short profile ID is SHA-256 of a canonical metadata description that binds
the catalog/protocol, compact geometry, lane parameters, tape schedule and complete
quantity relation identities. These metadata/content hashes are distinct from
the six-word commitments; none substitutes for the complete logical hash context.

Old SHAKE/prototype carrier schemas are rejected. Production ingress still rejects
compact artifacts, and no compact persistence/admission path uses the offline
success result. The current complete-row DTO exceeds both production byte caps
before framing; explicit diagnostic budgets do not widen production policy.
Concrete security, witness privacy, authenticated caller integration and release
hardware evidence remain separate obligations.

## Validation and remaining work

The counts, artifacts and measurements below retain their original earlier source
scope. They are not validation of the current six-lane compact transcript or its
changed profile and wire identities. New evidence must bind the actual source,
executable and complete proof bytes; compiler success alone is insufficient.


The retained prover harness with `dev-tools,fastpq-gpu` passes 927 unit tests
with ten explicitly ignored diagnostics; the final default-feature rebuild
passes 795 tests with seven ignored. Twenty focused checks cover the rejection
dispatcher, legacy AXT behavior, public claim preparation and exact ordinary
and AXT prepared-context byte parity. All 20 FASTPQ model tests now pass,
including canonical typed round trips and rejection of a noncanonical sixth
root word in either commitment variant. The initial test fixture's adaptive
layout mismatch was corrected without changing decoder behavior. The final
model/prover rebuild has zero changes in its focused source snapshot; this is
not a complete immutable dependency closure. A later retained build passes all
five complete-model-to-prover adapter tests and the coherently reproved
nonconstant current/next-row regression. The current Core harness passes all six
public-producer groups plus the lane-proof and Kura-snapshot canonical encoding
tests. Core build provenance reports unrelated source drift and is development
evidence; it is not a release closure. Torii's canonical recovery and
cumulative-budget regression pass in both retained rebuilt harnesses. The latest
build and affected runtime checks keep 2,183 focused source inputs unchanged;
account routing, SCCP finality binding and storage restart also pass. Its
15-filter run records 21 passing and 11 failing tests. A reviewed follow-up fixes
unsigned token mint construction, checkpoint file ownership, canonical UAID JSON
and current-contract fixtures. The combined crypto/storage/Torii harness now
compiles and records 85 passing and two failing tests with all 3,168 focused
source inputs unchanged. Proof-token and checkpoint module/API regressions pass;
an alias-permission fixture correction and a bounded BLS peer-copy fix are
applied. The exact private copy helper passes row/aggregate budget tests; the
rebuilt Torii5 harness passes all six focused regressions, including both prior
failures, with all 3,168 captured inputs unchanged. The full Core peer-collector
module passes all 13 tests in Core38 with 4,072 inputs unchanged. An unchanged
release-profile default-feature control also passes 842 prover unit tests and
both complete artifact cases, retaining exact artifact bytes and bounded work
with 3,299 source inputs unchanged. Its local raw verification measurements are
0.875/0.887 seconds; concurrent host load prevents performance qualification.
These checks do not implement compact artifact persistence or authenticate remote
public statements.
The earlier broader Core run passed 62 FASTPQ, nine Kura and two
unanchored-proof rejection tests while excluding two deadlocked lifecycle
fixtures. Those fixtures blocked a single-thread Tokio runtime on a standard
barrier before its worker started. The asynchronous startup/drop-safe release
fix now passes both exact tests and the complete 64-test FASTPQ filter without
exclusions in a rebuilt Core harness (SHA-256
`08b485ef523cfb635c24dd4990e912410fb81a9dfd0e4c2422ef38a5a4841ff7`).
That build reports one unrelated development-source change; it is not an
immutable release closure. Lifecycle evidence is
`target/fastpq-production-validation/core-public-producer-lifecycle-evidence.json`.
Evidence for the preceding boundary checks is retained under
`target/fastpq-production-validation/public-artifact-prover-evidence.json` and
`target/fastpq-production-validation/core-public-producer-current-evidence.json`.

Complete validation of the producer bridge, qualified profile and typed
transcript, bounded artifact verification, authenticated execution-state expectations and durable
proof persistence/recovery before routing compact artifacts into production.
The [production goals](fastpq_production_readiness.md) retain these release
obligations and the separate cryptographic and hardware qualification gates.
