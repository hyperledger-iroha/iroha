# FASTPQ production readiness

Updated: 2026-09-07. **Production qualification is unavailable.** The selected
completion target is succinct verification from bounded authenticated openings.
A successful local test, feature build, arithmetic calculation, or benchmark
manifest is not a release qualification decision.

## Current optimizations integration

Work is restricted to `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The scoped integration candidate remains under ignored
`target/fastpq-optimizations-integration/port-candidate` during the coordinated
source freeze. It has not been applied to the branch or compiled there. The
retained implementation and validation records below identify their own source
snapshots; they do not qualify this integration candidate.

The port preserves the branch's ordered canonical transaction-wire commitment in
`PublicIO.tx_set_hash`. It independently binds every ordered execution source,
including entries without transfer statements, through the manifest's
`source_entries_digest`. Missing or zero wire commitments reject before source
digest finalization; no execution-identity-list fallback is allowed. Source archive
validation receives the complete expected entry projection separately and retains
its bounded leaf decoder. Artifact persistence preserves the current Kura lock
order and checks pending canonical and reserved lifecycle/recovery capacity before
publishing new bytes. These reconciliations have unrun regression tests.

The imported reference completed 544 selected tests and two complete ordinary/AXT
artifact cases on a different source snapshot. Those receipts establish reference
behavior only. Current-branch compilation, exact test inventory, fresh complete
proof runs and immutable source capture are required after integration. Production
continues to use replay; the compact admission registry remains unqualified.

## Completion goals

| Goal | Required evidence | Current state |
| --- | --- | --- |
| G1: Close admission and evidence gaps | Regression rejection of unanchored remote spend, exact bound arithmetic, full-width contextual commitments, authenticated benchmark evidence | Corrections in progress; validation below |
| G2: Constrain the complete transfer statement | Reviewed AIR ledger or equivalent bounded public-input checks, with negative tests for every relation below | Complete 923-slot one-delta hash/SMT ledgers, bounded public checks and typed PublicIO/claim adapter pass; external authority/root authentication remains separate |
| G3: Implement succinct verification | Quotient/zerofier relation, correct terminal degree bound, bounded openings, no witness/trace reconstruction in the public verifier | One-delta and ordered two-delta ordinary/AXT bundles verify after dropping all private traces; public production API still replays |
| G4: Qualify cryptography | Protocol-specific qROM argument, final multi-target digest analysis, independently reproduced constants and vectors, independent review bound to final artifacts | Unavailable |
| G5: Qualify performance and resources | End-to-end proof/verification latency and peak memory, proof size, CPU/Metal/CUDA parity and failure quarantine on release hardware | Native Digest384 proof commitments/transcript and proof LDE execute on CPU; 423 M4 Max dispatch comparisons cover auxiliary shader ABI only; end-to-end, fleet and CUDA qualification remain incomplete |
| G6: Qualify integration and release | Same-source four-validator tests, restart/recovery and adversarial admission, signed immutable source and artifacts, rollout/rollback evidence | Incomplete |

Goals G2 and G3 must complete together before removing replay. The selected
product remains a transfer proof system; replacing it with a replay-only format
would not satisfy this goal.

The [compact profile analysis](fastpq_compact_profile_analysis.md) derives
2,865,251 framed bytes for one complete delta at 136 queries and 4,213,091 at
200, using the current separate-path wire. The exact-shape Rust test confirms
the 136-query hash and complete-transfer sizes; the 200-query projection is
not an implemented or qualified profile. The
complete typed adapter binds all seven caller-expected PublicIO fields, original
identities/quantities/transcript commitments and selected semantics, deriving
SMT ports only from validated path-free public claims. It does not authenticate
the caller's authority or turn a touched-balance root into a consensus root.
The [compact protocol contract](fastpq_compact_protocol_contract.md) records
the exact commitments, transcript order, shared opening sets and equations,
separating implemented checks from the outstanding soundness reductions. The
[primary-source soundness map](fastpq_compact_soundness_sources.md) records
applicable theorem hypotheses and concrete gaps; it is not a reduction or
parameter qualification. Conditional [row-linkage](fastpq_compact_row_linkage.md)
and [query-sampler](fastpq_compact_query_sampler.md) lemmas establish narrow
algebraic/distributional components under explicit premises; exact rational
sampler certificates pass. The reviewed conditional
[formal AIR interactive bound](fastpq_compact_air_bound.md) now supplies common
row/quotient recovery and current/next linkage for the existing openings. Its
exact arithmetic needs 237 queries to put its total below `2^-128`; the current
136-query profile does not meet that bound. This is neither a production
parameter choice nor an attack on the current profile. The
[degree ledger](fastpq_compact_air_degree_ledger.md) and
[semantic audit](fastpq_compact_air_semantics.md) cover all 923 source slots and
the bounded native public checks. External review, authenticated ledger context,
oracle/hash assumptions and Fiat–Shamir/qROM compilation remain qualification
obligations.

The [public artifact boundary](fastpq_public_artifacts.md) now defines path-free
model statements, distinct ordinary/AXT schemas, full-width commitment descriptions
and explicit offline decode budgets. Production AXT ingress rejects recognized
compact schemas before body work; these codecs do not enable compact admission.

The conditional [typed whole-tape compiler](fastpq_compact_typed_compiler.md),
[adaptive-context extension](fastpq_compact_adaptive_context.md) and
[projected raw-XOF model](fastpq_compact_projected_xof.md) now have separate
internal mathematical reviews. Under their explicit ideal-oracle assumptions,
[exact profile arithmetic](fastpq_compact_typed_profile.md) certifies 375 initial
positions for the conservative two-query simulation, a `2^32` adversary query
budget and 54 targets. The projected valid shared frame upper bound is
4,326,227 bytes per segment. The staged candidate now has a measured complete
shared proof frame of 4,046,360 bytes; its surrounding public artifact adds its
own bytes. This remains an unqualified candidate; the admitted implementation
has not changed. Its isolated [SHAKE prefix/body framing](fastpq_compact_shake_framing.md)
binds full public context and raw tapes while reusing unfinished absorb state.
The prefix and common Merkle/joint coefficient seams pass the workspace Cargo
gate, 43 focused tests and the full prover unit suite (948 passed, 11 diagnostic
tests ignored). The focused source snapshot stayed unchanged across this build
and validation; it is not a complete immutable dependency closure. Actual
compact-engine integration is applied. Its isolated compile, final 960-test unit
suite (12 diagnostics ignored), complete transfer proof and independent context KAT pass. The full proof
uses 375 AIR evaluations and one terminal degree check after private paths and
columns are dropped. Local proving/raw verification take 138.38/6.68 seconds;
decode charges are 38,302,664 bytes and full-process peak RSS is 2,541,617,152
bytes. No production default or admission path changes. Its later workspace
gate passes as recorded below. Concrete cryptographic qualification, authenticated
source expectations and release-hardware/corridor measurements remain required.

The applied public facade and bundle integration passes 966 unit tests and
four complete single/two-delta ordinary/AXT diagnostics. Its model artifact
adapter adds five focused groups (971 unit tests total), and both retained
complete artifact verification cases pass. Two-delta ordinary/AXT artifacts
measure 8,076,204/8,097,956 bytes, 8.65/9.47 seconds raw verification and exactly
750 AIR evaluations plus two terminal checks. The artifact-only processes peak
at 70,139,904/70,729,728 RSS bytes; cumulative Norito allocation charges are
156,565,622/156,922,753 bytes. Charges are not simultaneous heap use. All caller
public inputs and original AXT context are exact-compared before child work.
Both reviewed patches are applied after coordinated source capture. The workspace
build, all 971 unit tests and both retained artifact cases pass, with 100 focused
input hashes unchanged. These checks do not authenticate caller authority or
qualify production admission. The subsequent applied identity patch passes all 974
unit tests and both artifact cases in isolation and in the subsequent rebuilt
workspace harness. All 100 focused input hashes stay unchanged. It retains full
verified AIR roots and recomputes distinct canonical content digests for future
persistence integration.
The [bundle arithmetic](fastpq_compact_typed_profile.md#bundles-and-honest-abort)
charges every segment and keeps honest-abort and false-acceptance claims separate.
The Torii harness now compiles after the reviewed test migration. Its latest
retained binary passes canonical FASTPQ recovery, all three account-route cases,
exact SCCP finality binding and storage restart. All 2,183 focused source inputs
stayed unchanged through compilation and the subsequent 15-filter runtime run.
That run records 21 passing and 11 failing tests; failures exposed explicit JSON
negotiation, canonical UAID output, current permission fixtures, a test-marked
key identifier, and checkpoint-writer contention. The reviewed ten-file follow-up
is now applied: minting signs the unsigned token body before constructing a token;
PDP/PoTR ownership binds the opened file identity while preserving OS locking and
path validation; bounded UAID JSON renders the established canonical literal.
The combined crypto/storage/Torii harness now compiles and records 85 passing
and two failing tests, with all 3,168 focused source inputs unchanged. All 23
proof-token, 31 checkpoint ownership/provider and seven parallel Torii startup
tests pass, as do token API enforcement, bounded UAID JSON and all ten pipeline
status cases. The remaining failures are traced to an alias-permission fixture
expectation and bounded peer copying that charges BLS decode scratch against
the retained graph allowance. The applied private validated-key clone preserves
exact key bytes and rejects undersized row or enclosing budgets in two isolated
tests. The rebuilt Torii5 harness passes all six focused regressions, including
both prior failures, canonical FASTPQ recovery and all three account-route cases;
all 3,168 captured inputs remain unchanged. The full Core peer-collector module
subsequently passes all 13 tests in Core38 with 4,072 inputs unchanged. A separate
staged strict public-statement producer rejects changes to finalized quantities,
identities, optional digests and ordered occurrences. Its extracted implementation
passes six regression groups, including exact self-transfer legs; it is not yet
applied or an authenticated source statement. A private ordinary source-manifest
prototype passes 11 groups covering ordered entry geometry, all source/route
fields, canonical nominal leaf hashing, bounded raw decoding and fixed-write
inclusion against the existing Core sparse-tree oracle. Enclosing decode limits
remain cumulative. The proposed witness key remains unregistered; validator
insertion, production ingress and finality authentication are pending.

An unchanged-release-profile CPU control passes 842 unit tests (14 diagnostics
ignored) and both complete artifact cases, with 3,299 source inputs unchanged.
Ordinary/AXT raw verification measures 0.875/0.887 seconds and process peak RSS
60,129,280/60,194,816 bytes. Artifact hashes, sizes, cumulative charges and proof
work match the earlier fixtures. This default-feature harness differs from the
earlier development harness with GPU features; the recorded inventory explains
all differences. Concurrent load and one sample per artifact prevent a qualified
speedup claim. End-to-end release proving, hardware parity and production
qualification remain open.

## Findings and corrections

1. **Arithmetic overflow:** `u128::checked_shl` checks shift counts rather than
   discarded high bits. Security-bound numerators now check for lost high bits
   after shifting and reject overflow and zero-target accounting.
2. **Quotient and terminal geometry:** composition now combines row and
   transition quotients, with conservative exclusive degree `2 * N_trace`.
   Row-local constraints divide by `x^N - 1`; active-prefix and stability
   constraints exclude the final trace row from their zerofier. Binary FRI stops
   at four evaluations, with at most 17 reductions for the maximum domain, so
   terminal rounding preserves the intended inverse code rate of four. Full
   replay remains mandatory. Joint FRI now checks the fixed trace and quotient
   interpolants together as `J = Q + rho*T + sigma*x^N*T`, with independent Fp4
   challenges sampled after both oracle roots; its exclusive degree bound is
   `2N`, while the mixed trace bound is `N`. The unshifted term prevents high
   trace degrees from wrapping into low coefficients on the evaluation coset.
3. **Unsupported arithmetic claim:** the corrected diagnostic model accounts for
   quotient degree and the four-point terminal. With the current 136 queries its
   sampling term is `54 * 2^64 / 2^136 = 54 * 2^-72`, which does not meet the
   128-bit target. The same model selects 200 queries (192 fail). Neither count
   establishes protocol security. Composition alpha and column-mix coefficients
   now use all four Fp4 coordinates. Both trace roots and exact geometry are
   committed before these challenges, and transcript Norito encoding pins the
   canonical layout. The wider field removes the base-field aggregation ceiling
   but does not establish the complete proximity/Fiat–Shamir argument. The wire
   query count is unchanged;
   coherent protocol, parameter, fixture and resource-limit qualification remains
   necessary.
4. **Unanchored remote spending:** a reusable issuer-signed capability does not
   authenticate the exact intent, proof, effective amount, source state roots or
   transaction set. CoreHost rejects an otherwise valid capability before proof
   processing and rejects preexisting handle state at commit. Block admission
   also rejects otherwise valid envelopes carrying handles. Existing malformed
   statement diagnostics remain tested. Finalized lane-relay and fee-vault paths
   retain their separately authenticated boundaries.
5. **Narrow permission commitment:** the host previously zero-padded one
   Goldilocks field element to 32 bytes and used duplicate-last Merkle padding.
   Nonempty permission tables now hash a fixed domain, little-endian u64 entry
   count, and sorted fixed-width role/permission/epoch entries with the existing
   `iroha_crypto::Hash` implementation. Empty tables retain the zero sentinel.
   Existing proofs/witnesses that bind a nonempty permission table must be
   regenerated. This remains contextual input, not an AIR permission proof, and
   its 32-byte width does not establish aggregate 128-bit post-quantum security.
6. **Mismatched default prover/verifier capacity:** the 256-transition ceiling
   and 512 KiB approximate proof ceiling are independent. Even a minimally wide
   16-row transfer proof exceeds the byte ceiling under the current opening
   layout. Public proving now enforces the same default envelope as public
   verification, while raw development diagnostics can use explicit larger
   limits. Raising query counts without redesigning proof size and measured
   resource budgets would aggravate this mismatch. The 20,000-row accelerator
   microbenchmark is not evidence of admitted end-to-end proof capacity.
   The final public-API diagnostic's 360-column fixtures accepted two and four
   transition rows (147,685 and 321,406 actual Norito wire bytes), but rejected
   eight and sixteen at approximate payload sizes 652,958 and 1,366,766 bytes.
7. **Rollout evidence:** validation must inspect captured workload shape, actual
   CPU/GPU timings, backend availability, both captured-file hashes, numeric
   finiteness, telemetry and externally trusted manifest signatures. Hardware CI
   must fail on unavailable/broken CUDA rather than silently skip qualification.
8. **Exact row arithmetic:** 205 quadratic constraints now bind each transfer
   balance and inferred amount to 64 activated Boolean bits and 56+8-bit packed
   limbs, require exact eight-byte balance lengths, and check debit/credit
   arithmetic with two 32-bit equations and one Boolean carry. The trace adds
   196 auxiliary columns on transfer-containing batches and two byte-length
   columns globally; inactive auxiliary columns are zero. Schema limits still
   reject widths above 512, without silently expanding the resource envelope.
   This does not authenticate the inferred amount or pair membership.
9. **Native Metal defect:** the multi-state shader's `ulong3` representation was
   replaced with a scalar struct after a native regression exposed wrong results.
   The Rust host retains its single-state clamp. The corrected M4 Max smoke
   matrix covers 141 cases repeated three times across FFT, IFFT, LDE,
   permutation and row/column hashing. Its 423 dispatch comparisons establish
   local shader-ABI parity, not end-to-end prover or fleet qualification.
10. **Actual proof execution:** the native-STARK Digest384 commitment and
    transcript helpers execute on CPU even when a GPU mode was requested, and
    `derive_polynomial_data` constructs the proof LDE on CPU. Auxiliary Metal
    kernel parity therefore does not demonstrate acceleration of the six-lane
    proof pipeline. The observer now reports the requested policy together with
    the actual resolved CPU mode and no GPU backend at the real hashing site.

11. **Repeated proof work:** verifier authentication paths now share a local cache
    of at most 4,096 exact typed node computations. Role, FRI round, level,
    position and both complete child digests are part of each key; every path
    still checks its root and high index bits. Separate exact borrowed leaf
    caches retain at most 136 LDE chunks and 272 AIR rows per verification;
    complete field contents and indices are checked, and hash roles are isolated.
    Native profiling also identified FRI opening construction rebuilding each
    layer for every query; construction now builds each tree once per layer; byte parity and
    duplicate/bounds regressions pass. Test builds now optimize the
    small `fastpq_isi` arithmetic crate while retaining test assertions and
    overflow checks; release settings are unchanged.
    AIR row hashing now uses indexed parallel CPU work with reusable per-worker
    row buffers above 31 rows. Collection preserves exact row order and reports
    the lowest malformed row deterministically. Worker-parity regressions and
    complete-proof diagnostics pass in the integrated validation below; GPU
    execution is not claimed by this change.
12. **Remaining semantic building blocks:** standalone pair/permutation and ARX64
    polynomial modules are implemented for the next AIR phase, with passing adversarial
    Rust tests. Pair tuples include both counterpart accounts,
    asset, call, authority, full key/length and exact amounts; four committed Fp4
    auxiliaries require a separately authenticated table and phased transcript.
    ARX constraints cover exact wrapping addition and BLAKE2b XOR/rotations.
    The composed G and twelve-round compression modules now bind every register,
    message, counter, final-flag and feed-forward word. This direct compression
    witness has 253,185 cells and 404,306 quadratic numerators, requiring 976 ARX
    operation rows before a committed register layout. A separate output relation
    binds all 32 digest bytes with the existing marker at little-endian bit 248.
    These modules are not wired into proof admission. A test-only Fp4 interpolation
    helper prepares the required second trace phase. The narrow committed layout,
    message framing, digest initialization, block chaining, full roots and source
    authorization still require complete constraints before replay can be removed.
13. **SMT hash encoding:** witness roots and all siblings now require the existing
    Iroha hash marker. `Hash::prehashed` silently sets byte 31's low bit, so root
    recomputation alone admitted an alternate sibling encoding. A regression
    constructs this root-equivalent alias and requires admission to reject it;
    every root/sibling position is covered. Both admission regressions pass in the full GPU-enabled unit suite.
14. **Semantic trace budget:** the direct hash building blocks do not fit the
    current profile merely by moving operations into rows. Two full balance
    updates require 128 old/new internal-node compressions. Including four leaf
    wrappers, four value hashes and two one-block keys gives at least 138
    compressions, or 134,688 ARX operations. Even optimistic two-operation rows
    need 67,344 rows before register/message relations, exceeding the canonical
    65,536-row maximum. Longer keys require more compressions. This is a cost of
    the current building blocks, not a lower bound for every redesigned AIR;
    compact layouts, proof format and parameter/resource qualification remain open.
15. **Compact implementation and public-input scope:** the new single-block hash
    primitive uses 408 logical rows of 310 base-field columns through exact u32
    register copies and fused addition/XOR steps. Its Rust regressions pass.
    Current accounts, amounts and before/after balances are public claims:
    canonical normalization, pair arithmetic, key allocation and exact leaf
    hashing may remain bounded public-input processing. The private statement
    then needs 128 internal-node hashes per delta, plus complete direction,
    shared-sibling, update and root-boundary relations. Padding each hash to 512
    rows permits periodic fixed selectors and places one delta at the current
    65,536-row ceiling. The logical and physical SMT modules now carry all four
    eight-u32 ports in 342 columns, constrain all hash padding cells to zero,
    and link export, update and final boundaries without a cyclic final edge.
    Their Rust regressions pass; they are not an admitted proof. The 342
    columns require 744,192 bytes for 136 pairs of full row
    openings alone, so coherent proof/query/resource qualification is still
    necessary. LDE openings are full field values and cannot be narrowed to u32.
    New sparse public-polynomial and consuming phased-transcript helpers prepare
    this integration. Scalar root/order/coherence checks avoid allocating a
    trace or blowup-sized domain table, and opening mappings preserve all full
    field values. Fixed-coset ledgers now provide 680 hash slots plus 243 SMT
    slots with passing reference/interpolation/adversarial tests. The path-free
    public-claim constructor also passes its native differential tests. The
    complete one-delta prototype verifies through bounded raw openings in the
    integrated diagnostics below; private witness reconstruction remains
    mandatory in production verification.

16. **Query sampling runtime:** the sampler previously retried rejected and
    duplicate candidates until a checked counter eventually panicked. The
    applied shared implementation now returns explicit errors and permits at
    most `max(64,8*desired)` digest draws, with at most 512 selected indices.
    Current 136-query proofs permit 1,088 draws. Unsupported domains above the
    field modulus, excessive counts and transcript-counter exhaustion reject;
    no partial list or fallback is returned. Successful transcripts retain their
    exact tag/lane/state sequence. Injected-source and old/new parity tests are
    added and pass the 878-test integrated suite. No completion probability or security
    bound is inferred from this engineering work ceiling.

## Succinct proof acceptance contract

The current composition has 219 fixed residues: 14 selector/delta/stability
relations plus 205 integer relations. Every old/new packed limb beyond index one
adds its own independently challenged `s_transfer * limb` zero constraint. Every
fixed and additional residue receives a distinct transcript-derived alpha.
Residue index 4 (active prefix) and indices 6 through 13 (stable statement data)
use the transition zerofier `(x^N - 1) / (x - g^(N-1))`; all other residues use
`x^N - 1`. These relations are not a complete transfer AIR. `verify_with_limits`
must retain its deterministic batch, transcript, SMT, trace, LDE and root
reconstruction until all these gaps close:

The constraint-to-quotient construction follows the polynomial divisibility
reduction described in [StarkWare's polynomial constraints notes](https://starkware.co/wp-content/uploads/2021/12/STARK101-Part2.pdf).
That general construction does not establish soundness of this incomplete
FASTPQ statement or its challenge and parameter choices.

- Bind the now-constrained per-row u64 amount and direction to the exact
  authenticated transfer pair. Prove pair cardinality and conservation, and
  constrain canonical byte encodings for all identity and hash inputs. Inferring
  a valid unsigned row difference does not establish authorization to transfer it.
- Bind canonical accounts, asset identity, key paths, transaction ordering and
  cardinality to authenticated public inputs. Prove the required multiset and
  sequential execution relations, including repeated touched keys.
- Constrain complete hash inputs/outputs for every balance leaf and SMT node;
  scalar projections of 256-bit hashes cannot establish full root binding.
  Prove direction-bit/key agreement, sibling selection, old/new leaf relation,
  root chaining and public boundary roots.
- Authenticate the source state and exact authorization facts independently of
  a prover-carried witness. The private touched-balance root is not automatically
  the consensus-wide state root. The remote-spend gate can reopen only with
  authoritative finalized/QC source anchoring and exact spend authentication.
- Extend the implemented row/transition quotients to every missing boundary and
  transfer relation, with independently reviewed degree bounds. Qualify the
  implemented joint trace/quotient FRI argument, including proximity and sampled
  linkage; its exact-interpolant degree lemma alone is not full soundness.
- Qualify the Fp4 alpha, column-mix and joint-FRI aggregation under the requested
  soundness target, including adaptive transcript attempts and every batching
  failure event. All four coefficients are encoded, preflighted and counted
  toward the unchanged proof-size limit.
- Sample and authenticate the required openings and public-input relations with
  a protocol-specific soundness argument. Removing a full-trace check is allowed
  only after a test demonstrates the equivalent authenticated constraint.
- Freeze the resulting first-release parameter/profile identity, query count,
  proof schema, fixtures and decoding/resource limits together. Rebuild every
  consuming SDK, host, proof envelope and release artifact from that source.

Adversarial tests must change each accepted-state relation independently,
including equal-looking field reductions, omitted/duplicated transfers, root
substitution, final-row violations and unauthorized source claims. The final
public verifier must not build a complete trace, FFT/LDE, row hash collection or
Merkle tree. Instrumented scaling tests must demonstrate verification work
bounded by the declared proof openings and public input size.

## Evidence and limits

The worktree contains concurrent unrelated edits. Local checks do not qualify
an immutable release candidate. No live deployment is authorized by a passing
source check, and no production deployment has been performed by this work.

- `cargo iroha-fast -- test --locked -p fastpq_isi --lib`: 42 passed,
  including the 31 independently generated digest vectors checked against Rust
  one-shot and streaming implementations.
- `python3 -m pytest -q scripts/fastpq/tests/test_reference_digest384.py
  scripts/fastpq/tests/test_validate_rollout_manifest.py
  scripts/fastpq/tests/test_launch_geometry_sweep.py`: 72 passed, 14 subtests.
- The final rollout/wrapper/geometry suite passed 98 tests. The expanded tooling
  suite passed 165 and failed 15 at the existing authenticated release bootstrap
  hash pin; that unrelated reviewed-hash mismatch was not bypassed.
- Public documentation: the English page and all 20 translations were corrected
  in the sibling `iroha-docs` checkout. Node 24 production build, built links,
  FASTPQ locale/source-hash checks, RTL output, content/provenance checks,
  TypeScript and 127 focused documentation tests passed. Whole-site locale
  validation still reports 60 existing errors on untouched
  `sora-nexus-services.md` translations. Nothing was published.
- `scripts/check_no_legacy_codec.sh`: passed.
- A fresh RustSec scan against advisory database commit
  `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` found the previously recorded
  `rkyv 0.7.46` archive-validation advisory in the workspace lockfile; it is
  absent from FASTPQ's all-features/all-targets normal/build dependency graph.
  The graph does include `lru 0.16.4` through `concread` and `mv`, with the
  conditional panic-safety advisory `RUSTSEC-2026-0253`, eight unmaintained
  dependencies, and yanked `chacha20 0.10.0`/`spin 0.9.8` resolutions. The
  affected `concread::threadcache` module has no use in FASTPQ or its local
  `mv`/crypto/data-model dependency paths; this is a scoped call-site review,
  not removal of the upstream defect. These upstream maintenance and resolution
  findings remain tracked dependency debt. Reports and the captured
  dependency graph are under `target/fastpq-production-validation/`.
- Actual native Metal compilation and execution: all three shaders compiled with the exact
  build-script Metal 2.4/O3 flags under Apple Metal/AIR-LLD 32023.883 and
  MetalToolchain 17.6.109.0. Linked `fastpq.metallib` is 120,098 bytes with SHA256
  `9f4bf0d66796a8fca6a08c566b0ace3d0f77c23e375a1ed55a5520aece11e1a2`.
  Commands, source hashes and timings are in the local untracked
  `target/fastpq-native-metal-validation/native_compile.json`. The M4 Max native
  runner passed 141 cases in each of three runs (423 dispatch comparisons);
  `runtime_results.json` and `runtime_original_regression.log` in the same
  directory record the corrected shader-ABI smoke matrix and original failure.
  These are auxiliary shader dispatches: the current Digest384 proof commitment/
  transcript path and proof LDE construction still execute on CPU. The runtime
  evidence does not qualify GPU acceleration of the six-lane proof pipeline.
  Rust end-to-end parity, 20k-row throughput, fleet coverage and CUDA remain
  unqualified.
- The internal six-lane Digest384 Metal continuation primitive separately passed
  24 native M4 Max cases in three repetitions (72 dispatches). Final four-unit
  library: 130,084 bytes, SHA256
  `efe0852d7fd8cae003e790c79a046f22d9c0f6359e9658f3c45c063ff4c3176a`.
  Exact source hashes, compile/runtime commands and results are retained under
  `target/fastpq-native-digest384-validation/`. Three subsequent Rust host tests
  passed with actual Metal execution, including independent reference frames,
  heterogeneous payloads and a local AIR-row timing diagnostic. Retained binary,
  log hashes, exact samples and measurements are in
  `target/fastpq-production-validation/digest384-rust-metal-evidence.json`.
  Median prefix-plus-continuation times for 256 columns were CPU/Metal 28.92/29.04 ms
  at 16 rows, 115.39/42.80 ms at 64 rows and 461.47/110.58 ms at 256 rows.
  These are test-profile diagnostics in a concurrent worktree. The primitive is
  not adopted by the proof pipeline; these are not end-to-end or release timings.
- `cargo iroha-fast -- check --locked -p fastpq_prover --lib` passed after the
  Fp4/joint-FRI migration. It reported unused diagnostic helpers; those are now
  gated by their test/developer usage. Final feature builds and tests are pending.
- `cargo iroha-fast -- test --locked -p fastpq_prover --lib backend::`:
  54 passed, including all-row and transition quotients, integer column binding,
  four-point FRI terminal checks, and truthful native CPU telemetry.
- The final developer/GPU unit executable passed 699 tests with zero failures
  in 97.44 seconds; three explicit hardware diagnostics were ignored there and
  had already passed separately with actual Metal dispatch. This includes all
  Fp4, cache, integer/pair, ARX/G, compression, full-width output, single-block,
  SMT relation and marker-admission tests, plus corrected CPU telemetry. The log
  is `target/fastpq-production-validation/prover-final-unit-tests.log`.
- The next CPU unit executable passed all 630 tests, with no ignored tests or
  failures, in 46.72 seconds. This covers compact BLAKE2b, the full logical and
  physical SMT program, bounded fixed-domain/public-table/selector helpers,
  every opening column, phased transcript, occurrence adapter and deterministic
  parallel AIR hashing. Test counts differ because CPU builds omit Metal-only
  groups. The log is
  `target/fastpq-production-validation/prover-compact-cpu-unit-tests.log`.
- The compiled hash/SMT ledgers and public-claim constructor then passed the
  expanded CPU suite: 655 passed, zero failures/ignored, in 72.01 seconds.
  Hash compilation shares 3,682 nodes (3,033 arithmetic), 209 fixed masks and
  2,405 weighted output terms instead of evaluating 199,026 reference residues
  per full period at every opening. The optional 4,096-point prover mask cache
  uses 6,848,512 bytes. The retained executable, log hashes and diagnostic
  timings are recorded in
  `target/fastpq-production-validation/compiled-ledger-baseline-evidence.json`.
  These test-build timings under concurrent load are not end-to-end SLO or
  signed release evidence. Reusable evaluator scratch, scalar arithmetic
  optimization and the compact proof protocol are subsequent work validated
  by the next executable below.
- The expanded CPU executable passed 688 tests, zero failures and three
  explicitly separate resource diagnostics ignored, in 65.76 seconds. It
  includes the bounded-opening protocol, typed PublicIO/claim adapter, complete
  hash/SMT adapter, scalar arithmetic, scratch reuse and canonical multipaths.
  A fresh false-public-digest proof fails the actual AIR/FRI relation; a valid
  digest proof verifies after all private traces are dropped, using 136 AIR
  evaluations, 272 row leaves and 136 FRI query checks. Its 1,762,083-byte
  canonical proof matches exact shape accounting. The complete-transfer shape
  is independently confirmed at 2,865,251 bytes. Both exceed the unchanged
  production limit. The compiler's Norito slice mismatch was fixed with a
  bounded canonical Vec copy after preflight.
  The separate fixed-column cache diagnostic passed in 4.02 seconds, comparing
  cached and bounded verifier evaluations at cycle/domain boundaries; its
  49 fixed LDE columns use 205,520,896 bytes and phase cycle 16,777,216 bytes.
  The complete typed-transfer proof diagnostic passed after dropping all
  private witnesses/traces; changed permission context and the default 512 KiB
  envelope both reject. Construction took 1.195 seconds, proving 2,065.168
  seconds and bounded verification 10.085 seconds, with 2,865,251 canonical
  proof bytes. Maximum resident memory was 2,456,535,040 bytes. Verification
  used 136 AIR evaluations, 272 row leaves and 136 FRI checks. The explicitly
  selected diagnostic envelope was 4 MiB. This baseline predates the new
  prefix/parallel hashing and shared-wire work. Executable and completed-log hashes are retained in
  `target/fastpq-production-validation/compact-protocol-validation-evidence.json`.
  These are unsigned local test-profile results under concurrent work, not
  release qualification. Subsequent AXT context factoring is outside this run.
- Subsequent source work adds canonical shared row/FRI tables and one minimal
  Merkle frontier per oracle, with direct bounded verification and one complete
  terminal degree check. A distinct AXT AIR identity binds the full validated
  public context before challenges. Prefix reuse caches the six immutable
  typed-domain states, while indexed CPU jobs retain canonical leaf/node order.
  Independent code review found no acceptance or hash-layout regression. The
  integrated `dev-tools,fastpq-gpu` Cargo build passed, and its retained unit
  executable passed 850 tests, zero failures and six explicitly separate
  diagnostics ignored, in 47.05 seconds. The shared full-hash proof is 972,311
  canonical bytes versus 1,762,083 previously; it verified using 269 row leaves,
  272 oracle leaves, 705 FRI leaves, 4,222 parent hashes and one terminal degree
  check. A freshly generated false proof still fails that degree check. AXT
  canonical context/mirror/remote-fact checks and complete-proof parity across
  one/four workers pass. The full-transfer prefix/shared diagnostic also passed:
  proving took 311.421 seconds, shared conversion 9.109 seconds, and bounded
  shared verification 10.870 seconds. Shared wire size is 1,608,631 bytes versus
  2,865,251 separate-path bytes. It checks 272 row leaves, 272 oracle leaves,
  1,629 FRI leaves, 14,782 shared parents, 136 AIR evaluations and one terminal
  degree bound, after all private traces are dropped. Both default admission
  and changed permission context reject. Maximum resident memory was
  2,494,824,448 bytes. Concurrent test-profile runs do not establish release
  latency or peak-memory SLOs; producing shared openings still converts the
  separate-path proof. Bounded canonical raw-byte admission is implemented
  around the typed verifier; its later integrated validation is recorded below. Its caller byte
  limit precedes header/CRC work, trusted geometry bounds all dynamic sequence
  counts, and Norito enforces 32 MiB of cumulative allocation charges and depth
  16. These charges are not an RSS claim. Stricter outer decoder limits remain
  mandatory. The raw entry point does not change production admission.
  Evidence is retained in `target/fastpq-production-validation/prover-prefix-shared-axt-evidence.json`.
  The direct
  dependency-free digest test executable passed 49 tests (one timing diagnostic
  excluded), and the separate 1,000-pair diagnostic passed with identical outputs
  at 412.207 ms canonical versus 188.257 ms cached. Those direct Rust tests retain
  assertions and overflow checks but do not exercise workspace Cargo lints;
  Cargo's first pass found five trivial test casts, now corrected; the Cargo
  retry also passed all 49 tests with one separate timing diagnostic excluded
  (0.25 seconds). No pipeline,
  GPU or release speedup follows from this primitive measurement.
- The domain cache was subsequently isolated into its own module to preserve
  the canonical digest source commitment used by retained native execution
  profiles. `poseidon_digest384.rs` is byte-exact to that retained source
  (SHA-256 `d97552e693a324b96cc4149945aca538656dd14a6ba8500481ee3efff5fc6899`).
  The borrowed immutable cache obtains canonical parameters through the public
  accessors; the one-shot and streaming implementations remain independent
  test oracles. All 51 direct unit tests pass, including all 31 independent
  Python vectors and field/permutation parity. The 1,000-pair diagnostic passes
  at 410.410 ms canonical versus 173.338 ms cached. Cargo also passes all 51
  tests with one timing diagnostic excluded (0.26 seconds; build including
  the shared-lane wait took 15 minutes). Integrated prover unit
  revalidation of this relocation plus the raw decoder and typed ordinary/AXT
  entry points passes 863 tests, zero failures and six separate diagnostics
  excluded in 46.58 seconds. The initial build exposed a type mismatch in one
  added nesting-limit test; its measured-scope helper was corrected before the
  successful retry. The 95-file focused source snapshot is unchanged across
  that retry. The full-transfer diagnostic also passes in 305.50 seconds:
  258.019 seconds proving, 7.684 seconds shared conversion, 7.832 seconds typed
  shared verification, 8.229 seconds raw decoding/verification and 8.254 seconds
  through the ordinary-transfer facade. Shared proof bytes remain 1,608,631;
  maximum RSS was 2,480,603,136 bytes. All private witnesses/traces were dropped,
  changed permission context rejects, and default admission still rejects.
  Its retained public fixture has SHA-256
  `72838ce11648e25f4c8b3e7496651d2e4eb63efdab08154d8cc5fd8ef0a574e7`.
  The focused source snapshot remains unchanged through that full run. Decoder
  maximum-shape tests charged 20,652,707 bytes at width 342 and 24,945,615 at
  width 512, below 32 MiB; these are cumulative allocation charges, not RSS.
  The typed entry points reject mismatched semantic profiles before decoding;
  their checked result grants no authority or finality. Evidence is retained
  in `target/fastpq-production-validation/prover-isolated-prefix-codec-api-evidence.json`.
  The subsequent retained-tree, sampler-limit and shared-parent-prefix follow-ups
  pass 878 unit tests, zero failures and six separately selected diagnostics
  excluded, in 47.56 seconds. Cargo compiled the retained harness successfully
  in 4 minutes 4 seconds; its outer shell later exited 2 because the shared
  wrapper was edited in place during execution, confirmed by its owner. The
  linked harness was run directly for all reported results. The full-transfer
  diagnostic passes in 258.06 seconds: 220.079 seconds proving, 7.559 seconds
  shared conversion, 4.813 seconds shared verification, 5.201 seconds raw
  decoding/verification and 5.203 seconds through the ordinary facade. Maximum
  RSS was 2,411,184,128 bytes. The 1,608,631-byte proof is byte-identical to the
  retained fixture above, and all 95 focused source hashes remained unchanged
  through the run. Default admission and changed permission context reject.
  These concurrent test-profile measurements are not release benchmarks or
  proof of optimality. Evidence is retained in
  `target/fastpq-production-validation/prover-retained-sampler-prefix-evidence.json`.
  The full AXT raw-facade diagnostic also passes with genuine SMT paths and
  complete remote context, after all private material is dropped. It proves in
  303.704 seconds and verifies through the raw facade in 9.011 seconds, with
  1,607,488 shared bytes and 2,334,621,696-byte maximum RSS. These timings
  include concurrent Core/build load. Changed expected inputs, coherent roots,
  canonical receipts, inconsistent mirrors, missing remote facts, semantic-route
  substitution and trailing bytes reject. Default admission still rejects; the
  four-MiB envelope is diagnostic only. Its retained public proof SHA-256 is
  `c6e7488aa6fd2fcc4dec12513a7f8c3f7b13083b4cb98cdd78b140127a7d6bc1`.
  All 95 focused source hashes remained unchanged. The adjacent API/context
  suites pass five and seven tests. Evidence is retained in
  `target/fastpq-production-validation/prover-full-axt-evidence.json`.
- Ordered ordinary bundles pass 907 unit tests with zero failures and eight
  separately selected diagnostics excluded, in 56.38 seconds. A complete
  two-delta proof also passes after the whole batch's private SMT witnesses,
  columns and proof DTOs leave scope. The bundle binds the complete original
  public statement, both segment ordinals and the exact intermediate root.
  Its 3,210,403 wire bytes require 49,432,874 cumulative Norito allocation
  charges; verification performs 272 AIR evaluations and two terminal checks.
  The exact measured allocation ceiling accepts, while one fewer byte rejects
  with `TotalAllocationExceeded`. Reordered, repeated, omitted, extra or corrupt
  children, altered midpoint/context and trailing bytes reject. Default limits
  still reject; the eight-MiB outer/four-MiB child envelope is diagnostic only.
  Measured construction/proving took 622.290 seconds and raw verification
  23.021 seconds, with 2,474,295,296-byte maximum RSS. Rust reported 697.20
  seconds for the test, while the external timer reported 2,973.23 seconds;
  the discrepancy's cause is unknown, so these are not comparable release
  timings. All 215 focused source hashes remained unchanged through the run.
  Proof SHA-256 is
  `53046db232aafad0bf29343940c45532bd84a51b5420ae629d387fa8ac2673a0`;
  evidence is `target/fastpq-production-validation/prover-ordinary-bundle-evidence.json`.
- The AXT bundle and AIR-row domain-prefix changes pass 920 unit tests, zero
  failures and ten separately selected diagnostics excluded, in 45.87 seconds.
  The full two-delta AXT diagnostic passes in 454.35 seconds after all private
  material is discarded. It binds both remote occurrences, whole-batch context
  and exact intermediate root; coherent alternate manifest/remote-handle
  contexts, route substitution and corrupt/reordered children reject. Exact
  cumulative allocation admission accepts and one fewer byte rejects. Proving
  took 189.020 and 202.784 seconds, raw verification 10.480 seconds; the wire is
  3,216,261 bytes, statement 6,826 bytes and cumulative decode charges
  49,347,997 bytes. Maximum RSS was 2,555,822,080 bytes. Verification performs
  272 AIR evaluations and two terminal checks. The 512-by-342 row microdiagnostic
  preserves canonical leaves at 1.166 seconds scalar, 1.071 seconds cached with
  one worker and 0.284 seconds cached with four workers. These local test-profile
  measurements do not establish release performance or an end-to-end speedup.
  All 217 focused source hashes remained unchanged, the codec guard passes,
  and the frozen canonical digest is unchanged. Default admission still rejects
  the explicit diagnostic envelope. Proof SHA-256 is
  `f8a097a00a7cd3d3bc0f965dec799b646048dc45117d711da9d3101e5e7a0aa8`;
  evidence is `target/fastpq-production-validation/prover-axt-bundle-prefix-evidence.json`.
- The xtask manifest verifier's three signature/trust/tamper tests and its
  external-trust parser test passed (0.11 and 0.01 seconds). The build finished
  after the concurrent Torii game/NFT macro errors were fixed by their owners.
  Results are retained in `target/fastpq-production-validation/xtask-verify-bench-manifest-tests.log`
  and `xtask-parse-manifest-tests.log`; they establish local tooling regression
  behavior, not a signed production benchmark or source closure.
- Focused Core FASTPQ and host/block admission now pass all 47 exact selected
  tests in 57.63 seconds. This includes full-width/cardinality permission roots,
  transcript/batch invariants, standalone proof rejection, valid-issuer handle
  rejection, commit rejection and no state/cache mutation. The retained native
  game-owner binary has SHA-256
  `aeef730de65eab2f5a09cb046147d85935517ede2827dd699998d278942a49ce`.
  Its hash and test inventory were checked before execution; it includes the
  current mandatory game/NFT modules but predates that owner's subsequent V3
  adapter and fixture repairs. This is focused regression evidence, not current
  whole-source or release qualification. The four focused files present in
  the adjacent Core19 source manifest remain unchanged; that limited comparison
  is not an exact source closure for this separate retained binary. The older
  missing-module binary, failed links and a later mismatched Core19 copy were
  rejected before testing. Exact filters, logs and provenance limitations are in
  `target/fastpq-production-validation/core-before-v3-retained-evidence.json`.
- The developer/GPU integration executable passed all 12 enabled tests, with
  four resource diagnostics run separately, in 432.94 seconds. This includes
  byte-identical CPU-requested/GPU-requested proofs for a 1,000-row mixed raw
  statement. The statement uses expanded developer limits and CPU native proof
  commitments; it does not establish admitted production capacity or actual GPU
  proof acceleration. The log is
  `target/fastpq-production-validation/prover-integration-tests.log`.
- The public CPU resource runner completed all four 2/4/8/16-row workloads with
  unchanged source/binary hashes and valid artifact checks. Its record is
  `target/fastpq-public-resource-validation/run_20260905T115134.972369Z/measurements.json`.
  Two/four rows produced default-verifier-accepted proofs of 147,685/321,406
  complete Norito bytes; eight/sixteen rows hit the unchanged 524,288-byte
  approximate payload limit. This records the first two larger fixtures as
  rejected, with no invented returned wire size or verification time. Process
  RSS excludes Cargo; timings are local test-profile diagnostics under concurrent
  machine work. The runner's seven Python tests and codec guard pass.
- `cargo fmt --all --check`: reports unrelated worktree formatting differences.
  Changed FASTPQ files are formatted individually; `git diff --check` passes.

The 64-row raw transcript fixture has been regenerated for this first-release
change. The recompiled embedded-fixture test passed in 44.35 seconds, including
verification and byte-identical regeneration, with no fixture-update flag.
Changes requiring fixture review include
integer columns, Fp4 oracle values and challenges, joint FRI, quotient evaluations,
transcript schema binding and the four-value terminal opening change their contents. Full replay remains mandatory.
The independent Python reference reproduces the pinned parameter SHA3 digest
`84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6`.
Neither result closes the missing succinct semantic argument.

Release prerequisites include independent cryptographic review and access to
release-class Apple/NVIDIA hardware with actual shader/kernel execution. The
source-coupled [digest qualification boundary](fastpq_compact_digest_security.md)
records the one-element capacity per lane, restricted message alphabet and
missing combiner/oracle argument. Limited matrix checks and output width do not
qualify the construction. The
current six-lane construction has `p^6` possible canonical outputs, not exactly
`2^384`; its idealized collision term and all 32-byte external commitments need
explicit treatment in the final argument. `iroha_crypto::Hash` also forces one
marker bit, leaving 255 variable output bits; a 32-byte container must not be
counted as 256 unconstrained digest bits. An implementation cross-check is
regression evidence, not an independent security audit.

## Pending Rust and release validation

Use the existing warm Cargo target and `cargo iroha-fast`; no clean build or
new target directory is needed. Re-run the applicable focused and corridor gates
against the settled candidate. Some earlier development invocations below passed
as recorded above; this list is not a failure inventory or a same-source release
qualification claim:

```sh
cargo iroha-fast -- test --locked -p fastpq_prover --lib
cargo iroha-fast -- test --locked -p fastpq_prover --features dev-tools --test fastpq_integration --test transcript_replay
cargo iroha-fast -- test --locked -p iroha_core --lib fastpq::
cargo iroha-fast -- test --locked -p iroha_core --lib axt_unanchored_admission_tests
cargo iroha-fast -- test --locked -p iroha_core --lib ivm_corehost_axt_tests
cargo iroha-fast -- test --locked -p iroha_core --lib axt_validation
cargo iroha-fast -- test --locked -p xtask --features dev-tools --bin xtask verify_bench_manifest
cargo iroha-fast -- test --locked -p xtask --features dev-tools --bin xtask parse_fastpq_manifest_verification
cargo iroha-fast -- test --locked -p fastpq_prover --features fastpq-gpu --lib cuda_test_requirement
cargo iroha-fast -- test --locked -p fastpq_prover --features dev-tools,fastpq-gpu --bin fastpq_cuda_bench --bin fastpq_metal_bench collect_operations_rejects_gpu_timings_without_a_dispatch
```

Follow with real hardware parity, the four-validator admission/recovery corridor,
full workspace tests and strict Clippy against the settled candidate. A pending
command or missing-device skip is never passing release evidence.
