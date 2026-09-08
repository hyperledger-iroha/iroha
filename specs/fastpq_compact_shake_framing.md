# SHAKE prefix/body candidate framing

2026-09-08. The framing and arithmetic below describe the prospective
401-label `compact_shake_candidate` successor.
It is not a qualified proof profile or a production admission path. The existing
136-query diagnostic protocol does not use this candidate. The mathematical
conditions remain those of [projected-XOF model](fastpq_compact_projected_xof.md) and
[typed profile calculation](fastpq_compact_typed_profile.md).

## Exact oracle input

Every H and G call has input `P || B`, where each component is independently
encoded by `norito::encode_canonical`. Both full Norito headers, schema hashes,
layout flags, lengths and checksums are part of the hash input. The schema names
are `fastpq_prover::compact_candidate::ShakePrefixV1` and
`fastpq_prover::compact_candidate::ShakeBodyV1`.

`P` fields, in order:

1. `version: u16 = 1`.
2. `identity: Vec<u8> = b"fastpq:compact-shake256:h16:g375:c401:342cols:923slots:65536rows:8blowup:17folds:prefix-body:v1"`.
3. `context: Vec<u8>`: complete immutable public context, 1..=262144 bytes.

`B` fields, in order:

1. `kind: u8`: 1 leaf H, 2 parent H, 3 chain H, 4 whole-message G.
2. `oracle: u8`: 1 row, 2 mixed, 3 quotient, 4 FRI; zero for chain/G.
3. `round: u8`: FRI index or one-based verifier message; zero for other trees.
4. `level: u32`: zero for leaves/chain/G, one-based for parents.
5. `position: u32`: exact leaf or parent position; zero for chain/G.
6. `output_bytes: u32`: fixed required tape length.
7. `fields: Vec<Vec<u8>>`: complete leaf payload; ordered left/right digests;
   complete raw pending tape and root; or predecessor digest, respectively.

The two canonical frame lengths uniquely delimit P and B. Neither public context
nor raw tape is replaced by a digest, pointer, or external reference. A canonical
outgoing encoder constructs these values; this module does not accept or parse
arbitrary incoming concatenations. A future formal raw-input recognizer must
require exact canonical frames, correct schemas, full consumption and permitted
field combinations. Treat all other raw inputs as the independent auxiliary
namespace described by the projected-XOF argument.

`Arc` shares the immutable prefix absorb state; tests also retain its encoded
bytes. Normal prefix construction still encodes the complete local P before
initializing that state. There is no `Arc` in either encoded schema. Norito's `Arc<T>` encoding adds an owned
payload length; it is not byte-transparent. Two initial baseline KATs caught a
reference encoder that had omitted that prefix. A direct Rust frame dump and
independent hashlib SHAKE calculation identified the discrepancy; it was not a
SHAKE implementation mismatch. The prefix/body KATs check complete input bytes as
well as raw outputs and projected roots.

## State reuse and logical oracle equality

`iroha_crypto::xof::Shake256Prefix` stores the unfinished `sha3::Shake256` absorb
state, including its partial 136-byte rate block. Each expansion clones that
state, appends B, and finalizes once. This is exactly SHAKE256(P || B), not
SHAKE256(SHAKE256(P) || B). Different output lengths share the normal SHAKE prefix;
the body explicitly binds the intended length to separate protocol calls.

The immutable cache is shared across workers. Its Debug output reveals no
absorbed bytes or internal state. Generic SHAKE remains a hash/XOF helper, not a
new authentication construction. No dependency, Cargo.lock or frozen native
Digest384 parameter change is needed.

H always squeezes 128 bytes, selects the first six canonical little-endian
Goldilocks values from 16 candidates and aborts if six are unavailable. G keeps
all raw tape bytes, including unused and rejected samples. The 22 verifier
messages and 21 chain updates, fixed zero anchor, 375 sorted query positions and
permanent decode/hash abort behavior are unchanged by prefix reuse. The
successor query tape contains 401 labels of 19 bits in 953 bytes. Its last
label consumes bits 0..2 of byte 952; only the five high padding bits are
ignored when decoding labels. The G input fixes the complete 953-byte raw-output length; the decoder
ignores only the final five padding bits. Changing the identity to `:g375:c401:` changes every
context-dependent hash and proof; it does not change the 375 accepted queries,
wire schemas or protocol version. A completed transcript alone is not proof
acceptance.

## Successor resource arithmetic

For the q375 upper-bound expansion (44,562 H and 22 G calls), the bodies total at
most 9,988,761 bytes. At the 262144-byte context cap, P is 262302 bytes, and its
partial rate buffer is 94 bytes. The logical inputs still total at most
11,704,461,129 bytes. With prefix reuse the absorb API receives at most 10,251,063
bytes: P once plus all B values. It performs at most 145,501 Keccak permutations,
versus 86,101,525 without reuse for the same framed calls. These are integer
bounds, not measured proof times, heap limits or an adversarial query discount.
The state clone and body encoding work remain real costs. Relative to the
predecessor, P is five bytes longer and the query tape grows by three bytes.
The final tape crosses seven 136-byte output blocks and requires an eighth,
adding one squeezing permutation per segment. These Python calculations are
build-independent checks; successor Rust compilation and fresh full-proof
execution have not run.

Canonical body lengths are 2817 bytes for a full row, 184 for a parent, 29724 for
the largest chain input, and 127 for G. The callback-based Merkle adapter
retains the existing O(q log L) schedule and caller limits; it does not construct a full tree during
verification.

## Retained predecessor measurements

All measurements, test counts, artifact hashes and applied-stage statements in
this section and the retained sections below belong to the predecessor identity
without `:c401:`. They are retained history, not evidence for this successor.
The earlier local aarch64 build measured a 352-byte prefix state. New sampler
vectors are independently derived by the Python checker, but the existing
full-proof pins must be regenerated and reconciled under the successor before
its retained-artifact tests can be counted as evidence.

A direct-rustc staged harness passed 26 tests (6 XOF, 13 candidate and 7 actual
field tests), including partial block boundaries and all tree/message roles. One
explicit ignored measurement compared 128 row hashes under identical prefix/body
framing and confirmed all raw bytes matched. At a 262144-byte context it measured
2.558 seconds cold versus 0.0306 seconds cached, with 0.0203 seconds prefix
preparation. This small local hash measurement excludes proof construction,
encoding the row bodies, decoder cost and network execution. It is not a full
prover/verifier or hardware acceleration qualification. The stage is applied.
The workspace Cargo gate with `dev-tools,fastpq-gpu` passes, followed by 43 focused
checks (6 XOF, 13 candidate, 14 Merkle, 4 joint, 5 model bridge and 1 coherent
current/next-row regression). The retained prover binary passes all 948 unit
tests with 11 explicit diagnostics ignored in 89.58 seconds. Its SHA-256 is
`eed8d1c0234171a807bea94bbe249cc5ad4b02953fde24bf751c6a8ba0783836`;
evidence is `target/fastpq-production-validation/compact-shake-prefix-binaries.json`.
The focused source snapshot stayed unchanged.

## Engine binding and retained predecessor proof

The common engine stage adds a fixed descriptor selected by the trusted entry
point. A proof cannot select that descriptor. It reuses the existing AIR,
joint-degree relation, fold arithmetic, opening plans and terminal-degree check.
The prototype transcript and complete FRI outputs retain byte/field parity with
the original implementation. Candidate reuse of a prepared row tree under a
different complete public context is rejected before prover work.

Before prefix construction, `ShakeEngineStatementV1` canonically frames the
relation identity, trace/extended row counts, width, constraint count, base
modulus, extension nonresidue, domain root/log size/coset, blowup, arity, fold
count, terminal length/degree, query count and full public statement bytes in
that order. Its fixture remains 225 bytes and the successor complete prefix is
382 bytes (the predecessor was 377 bytes). The five successor zero-leaf roots are rebound
from independent Python Norito/SHAKE calculations; their Rust assertions have
not yet run. Canonical String uses a compact length, while Vec uses the fixed sequence count; the
reference encoder's initial mismatch was corrected without changing the engine.
The self-contained `scripts/fastpq/check_compact_shake_encoding.py`
reproduces these and the prefix/body/resource controls without retained artifacts.

A distinct `ShakeSharedProofV1` has the same bounded payload fields as the
prototype frame. Moves between internal owned tables allocate no replacement
row/query tables; schema cross-decoding fails. The raw cap precedes geometry and
header work, and decode budgets come from the fixed 375-query descriptor plus
explicit caller allocation policy. With the current canonical 32-byte Fp4
carrier, a maximum loose-shape codec fixture encodes 6,713,525 bytes and checks
that cumulative allocation charges remain below a 64 MiB scope. The exact
framing projection with minimal frontiers is 4,279,877 bytes; the existing
4,326,227-byte runtime ceiling is unchanged. The loose shape is not a valid
proof and exceeds that ceiling. The prior 37-byte-carrier fixture measured
6,759,875 encoded bytes and 57,789,201 allocation-charge bytes; those are
historical measurements, not current carrier measurements.

The following predecessor engine evidence is unchanged. Its isolated compile
passes with pinned copies of 439 Cargo fingerprints
and 2,431 source/artifact inputs; this is not a complete dependency-source release
closure. Its final unit suite passes 960 tests with 12 diagnostics ignored
in 109.89 seconds, including the independent structured-context and largest-shape
checks. The complete public transfer diagnostic also passes after dropping all private
paths, witness rows, columns, expanded openings and retained prover trees:

| Measurement | Local diagnostic result |
| --- | ---: |
| Canonical shared proof frame | 4,046,360 bytes |
| Proving including shared conversion | 138.3818 s |
| Raw decoding and verification | 6.6833 s |
| AIR evaluations / terminal checks | 375 / 1 |
| Row / mixed+quotient / FRI leaves | 750 / 750 / 3,918 |
| Shared parent hashes | 33,307 |
| Decode elements / allocation charges | 289,490 / 38,302,664 bytes |
| Whole diagnostic process peak RSS | 2,541,617,152 bytes |

Exact allocation-boundary, default-size, query-cap, schema, public-context and
seven independent opening/root/field substitutions reject. The first full test
attempt omitted the existing single-delta Poseidon preimage digest in its public
fixture; it failed before proving and only that fixture was repaired. Inputs
stayed unchanged through the passing proof and their exact versions are retained.
Proof SHA-256 is
`ccb9eb92d8f517d85213399c276a2f5ac2da4f6eb740b29d4df79b54bd487e0e`;
evidence is `target/fastpq-production-validation/compact-shake-engine-full-transfer-evidence.json`.
The unoptimized test binary ran under concurrent local machine work. This is not
GPU proof, fleet, external cryptographic or signed release qualification. The
reviewed engine stage is applied; its later workspace gate and full971 suite pass. Production
verification still requires replay.


## Retained predecessor facade, bundle and model-artifact evidence

The typed candidate facades use the same public AIR constructors and require the
fixed 375-query candidate codec. Separate ordinary/AXT carrier schemas also bind
whole-batch context and ordered intermediate roots. All 966 unit tests and the
four complete proof diagnostics pass; the ordinary single frame retains its
previous exact SHA-256. No private paths, trace columns or expanded proof tables
remain when these raw verification measurements begin.

| Candidate diagnostic | Raw wire bytes | Raw verification | AIR / terminal checks |
| --- | ---: | ---: | ---: |
| Ordinary single | 4,046,360 | 4.1900 s | 375 / 1 |
| AXT single | 4,058,623 | 4.5554 s | 375 / 1 |
| Ordinary two-delta carrier | 8,074,469 | 8.7540 s | 750 / 2 |
| AXT two-delta carrier | 8,094,617 | 10.9207 s | 750 / 2 |

The candidate model-artifact adapter separately checks fixed profile identity,
all seven expected PublicIO fields and every caller AXT binding, original metadata
field, mirror and ordered remote preimage before child verification. One enclosing
Norito scope spans the model wrapper, carrier and every child decode. Five new
focused groups and all 971 unit tests pass; both complete model-artifact cases
also pass. Exact cumulative allocation and element limits pass, while either
one less rejects, including when imposed by an enclosing caller scope.

| Two-delta model artifact | Ordinary | AXT |
| --- | ---: | ---: |
| Canonical wrapper bytes | 8,076,204 | 8,097,956 |
| Raw verification | 8.6459 s | 9.4672 s |
| Cumulative decode allocation charges | 156,565,622 | 156,922,753 |
| Cumulative decoded elements | 16,728,094 | 16,768,793 |
| Artifact-only test process peak RSS | 70,139,904 | 70,729,728 |

Allocation charges include repeated decoding work and are not simultaneous heap
use. The last tests load previously generated proof bytes and construct only
public expected facts; they never build a prover trace. Timings come from
unoptimized isolated binaries under concurrent local work and are not speedup,
GPU, fleet or production latency claims. Evidence is retained in
`target/fastpq-production-validation/compact-shake-public-complete-evidence.json`
and `compact-shake-artifact-complete-evidence.json` in the same directory. Public
and artifact patches are applied after coordinated source capture; the workspace
gate, full 971-test unit suite and both complete artifact cases pass. Production qualification and caller-state authentication remain
open; the production qualification registry is empty.


A separate control compiles the same captured candidate source at optimization
level 3, preserving debug assertions, overflow checks and every dependency
artifact. All 971 unit tests and both complete artifact cases pass with identical
wire hashes, decode charges and verification work. Ordinary/AXT raw verification
measures 7.0370/10.3454 seconds and process peak RSS 64,307,200/64,684,032 bytes.
The dependencies retain their original build profiles and concurrent load varies;
this is not a complete release build or a speedup claim. Exact inputs and logs
are recorded in `compact-shake-artifact-optimized-evidence.json` under the same
evidence directory. The later workspace gate passes separately with all 971 unit tests and both
complete model-artifact cases; its 100 focused input hashes stay unchanged.
