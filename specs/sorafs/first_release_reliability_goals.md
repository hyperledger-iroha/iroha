# SoraFS first-release reliability goals

Accepted on 2026-09-26 following the implementation critique. These goals refine
G01–G06 and G13–G15 in [V1 implementation goals](v1_implementation_goals.md).
The [closure ledger](v1_closure_ledger.md) remains the release evidence index.
This plan does not attest production readiness or authorize public-network
mutation. All goals remain open until their implementation and acceptance
checks pass against the same source candidate.

## Design constraints

- Ship one first-release implementation. Replace obsolete APIs and persistence
  layouts; do not add compatibility aliases, fallback decoders or parallel paths.
- Preserve canonical Norito, deterministic ledger execution, governed software
  signing, purpose binding, current revocation and verifiable finality.
- Retain authentication and refusal checks while completing their real producers.
  A mock, a local snapshot or an unconditional successful preflight cannot replace
  production authority.
- Separate immutable content metadata, durable ownership, volatile access recency
  and payload health. Damage to payload bytes must not make valid metadata
  authoritative for serving unverified content.
- Bound actual retained bytes, retry time and work per operation; bounding only
  task count or metadata allocation is insufficient.
- Preserve unrelated work in the shared checkout. Do not interrupt other build
  or agent processes. Record failed or blocked checks accurately.

## Goals and acceptance

| ID | Outcome | Owner | Required acceptance | State |
| --- | --- | --- | --- | --- |
| SR1 | One ordered durable storage index commit path; ordinary reads never rewrite the global index. Immutable metadata is shared without inventory-sized read clones. | Storage | Barrier-controlled read-A/ingest-B and read-A/evict-B interleavings survive restart; acknowledged manifests remain present; evicted manifests never reappear; failed/uncertain writes preserve explicit durability semantics. Read cost and persistence do not scale with the global inventory. | complete |
| SR2 | Expired manifests retire independently of repeated/shared content. | Storage/GC | Two expired manifests sharing bytes are reclaimed; repeated chunks within one manifest do not block GC; an unexpired sharing manifest remains readable; capacity/refcounts/audits remain exact after restart. | complete |
| SR3 | Discovery consumes current governed finalized admission, renewal and revocation state. | Core/DataModel/Torii | Production constructor and same-State reader accept valid governed admission, observe live renewal/revocation, evict stale adverts, reject substituted/stale heads and preserve revocation across restart. Local envelope directories are not a second authority. | active |
| SR4 | Fresh production tokens and authenticated publication complete through real authority. | Core/Torii/daemon/publisher | Reserve, Complete and challenged Check evidence authenticates exact execution and finality; issuer produces a usable token through production constructors. Authenticated initial-source staging breaks the empty-provider bootstrap cycle. Publisher waits for exact finalized registration, assignments and provider completion, then verifies every asset before declaring success. Recovery never double-spends or repeats a completed signature. | active |
| SR5 | Payload corruption permits authenticated degraded startup and bounded repair. | Storage/repair | Corrupt/missing chunks quarantine only affected manifests; healthy objects serve; corrupted objects cannot serve or produce successful proofs. Finalized lease-bound repair restores bytes and verifies original commitments before clearing quarantine. Metadata/authority corruption still rejects startup. | active |
| SR6 | Retrieval retains bounded working memory and incrementally verifies canonical content. | CAR/orchestrator | Consuming sink and bounded reorder window release committed chunks; memory is bounded independently of total object size; chunk/root/plan/CAR integrity and deterministic output remain identical; cancellation, truncation, corruption and sink failure release resources. Public SDK/CLI consumers use the canonical new path. | complete |
| SR7 | Gateway throttling and policy errors preserve typed meaning across scheduling. | CAR/gateway/orchestrator | Actual HTTP 429 honors bounded Retry-After and byte/request budgets; throttling does not mark a provider unhealthy; retry deadlines and cancellation remain bounded; full-path policy-denial tests retain structured evidence. | complete |
| SR8 | Integrated first-release validation closes the seven findings. | Integration/release | Focused affected-crate tests, canonical wire roundtrips, codec guard, formatting and applicable lint pass. Four validators with mandatory signed RS16 DA/RBC exercise publish/replicate/retrieve/restart/repair/revoke through production constructors; no fixture adapter stands in for the authority under test. Broader workspace results and limitations are recorded separately. | active |

SR1/SR2/SR5 share the storage ownership design. SR3 and SR7 can progress
independently. SR4 depends on SR3 for live source admission; SR6 and SR7 must
converge on one retrieval abstraction. SR8 closes only after all seven preceding
goals have implementation and regression evidence.

## Execution record

- 2026-09-26: active goal created for all seven findings. Storage, admission and
  retrieval implementation are delegated; the coordinating owner implements
  stream-token completion/publication and integrates validation. The previous
  critique was source inspection only and supplied no fresh test results.
- The current shared checkout has unrelated staged and unstaged changes. The
  previously observed merge conflicts are resolved; existing build processes
  remain owned by their original tasks.
- Initial metadata check of `iroha_data_model` and `sorafs_manifest` passed.
  The first Core/provider check found two configuration `Copy` lint errors;
  those were corrected. Retrieval compile errors in the scheduler were fixed;
  an overlapping publisher edit also required canonical schema derives.
  Follow-up native and retrieval compiler checks are pending.
- `scripts/check_no_legacy_codec.sh` passed. Focused portable-finality tests
  compile in the stable `sorafs-reliability` target slot while the ordinary
  workspace build directory is occupied. This is partial validation only;
  production-constructor, four-validator and broader tests remain required.

- The five SCCP finality descendant tests passed in the dedicated target slot,
  including the portable successor verifier, exact epoch transition, and fork/QC
  corruption refusal. The first combined Core/Torii/daemon check then stopped at
  a concurrent privacy accumulator type mismatch; it did not reach Torii or
  daemon validation. Three private CLI spool lifecycle tests passed in a scoped
  standalone harness. These results do not close the integration goal.

- The three publication-proof tests passed: assigned/completed evidence succeeds;
  failed execution, replay, phase substitution, a valid fork and an unbound
  checkpoint are rejected. The full `sorafs_node` library suite then ran:
  1,552 passed, six failed and two ignored. The ordered-index interleaving,
  degraded startup/proof recovery, streaming repair, duplicate delivery and
  current-lease-loss regressions passed. The six failures are being corrected;
  the suite is not qualified yet. The codec guard passed again and the three
  OpenAPI mirrors agree on authenticated publication and repair contracts.
- Production composition now includes explicit software transaction signers,
  stream-token Reserve/Complete/Check producers, provider completion custody,
  local crash-durable checkpoint CAS, and bounded assignment/repair source
  transports. Local checkpoint durability is not hardware rollback protection.
  Native daemon checks, production-constructor tests and four-validator evidence
  remain open. Genesis admission is being bound to signed-genesis provenance
  without circular inputs containing the hash of that same genesis.

- The consuming retrieval suites passed: `sorafs_car` 341 tests and
  `sorafs_orchestrator` 227 tests (two ignored). The old generic-429 assertion was
  updated to typed throttling. These library results do not close publication,
  daemon or network qualification.
- Real publication qualification exposed a capacity registration race: callers
  were required to predict the future consensus timestamp. The first-release
  instruction now carries only canonical declaration bytes; execution derives
  its timestamp and registry projection. Rust callers, SDKs and producer tooling
  are being migrated together, with no retired wire shape accepted.

- The daemon test compilation reached its test module after Core and Torii
  compiled, then rejected two test calls to a private Kura accessor. Fixtures now
  retain or expose their own test-owned Kura instead of widening production
  visibility. The subsequent attempt hit a concurrent privacy-module import
  error before daemon qualification; native and network tests remain pending.
- The canonical Kotlin capacity API passes its Java-consumer tests. The revised
  capacity simulation tests pass two cases. The codec guard passes after the
  instruction wire change. Actual wire recapture now covers the canonical
  capacity declaration, populated genesis admission and populated publication
  assertion. The generated identity suite passes 325 cases (three explicit
  maintenance captures ignored); all 324 model tests matching SoraFS pass.
- The actual SDK exporter and generated documentation include the two new
  instructions and canonical capacity payload. Documentation/replay tooling unit
  tests pass 40 cases. Broader SDK parity currently stops on an unrelated
  JavaScript retail-family inventory mismatch; the release-proxy fixture replay
  requires its separately configured provenance environment and has not run.
- The second complete storage suite passes 1,555 cases, with three GC failures
  and two ignored tests. An obsolete shared-reference gate in durable eviction
  preparation caused those three failures and has been removed; the new build
  must verify that correction. Transport changes also separate authenticated
  metadata from bounded chunk frames and bind staged consumption to the current
  assignment revision. These changes remain under validation.
- All 25 schema tests pass. The SoraFS configuration suite passes 157 cases and
  finds one unconsumed diagnostic collector in a new test; its cleanup is fixed
  and awaiting rerun. Current-admission and repair expiry now also respect
  authenticated finalized time without relaxing local issuance/acquisition
  bounds. Core compilation stopped in a concurrent FastPQ merge conflict before
  validating those regressions.
- The existing public SoraFS page and all 20 locale equivalents in the optional
  sibling documentation checkout describe the implemented publication and
  software-custody paths and their remaining runtime dependencies. The sibling
  content, translation and artifact-provenance validation passes. This does not
  substitute for the outstanding native and four-validator checks.

- Storage ordering and GC (SR1/SR2), bounded retrieval (SR6), and typed throttling
  (SR7) have their focused acceptance evidence. The final CAR suite passes 346
  tests; the orchestrator suite passes 227, with two unrelated ignored tests.
  The third full node suite passes 1,564 tests, has two ignored tests, and finds
  one missing pin-policy field in a new fixture. After correcting only that
  fixture, the rebuilt prepublication-refusal regression passes; no production
  storage change separates those two results. All three prior GC failures pass.
  The rebuilt SoraFS configuration library suite passes all 158 tests.
- Core/Torii/daemon compilation next stopped on 11 missing APIs in concurrent
  FastPQ integration changes. Those changes are owned by the active merge task.
  SR3/SR4/SR5/SR8 remain open pending native tests, binary builds and actual
  four-validator publication, restart, repair and Parliament revocation.
- 2026-09-27: review of PRs #5629/#5630 confirmed that public revocation
  structural validation and the complete consuming retrieval runtime are already
  tracked on `optimizations`; the draft rollback is unnecessary. Scheduler
  policy eligibility now precedes temporary quota and capacity checks, so a
  permitted provider can recover while a genuinely all-denied set fails
  immediately. Regression coverage combines policy filters with request quotas,
  typed throttling, stream capacity and burst limits.
  Fresh focused library tests pass: `sorafs_manifest` with
  `provider_admission::tests::` selects 37 tests; `sorafs_car` with
  `multi_fetch::tests::` selects 32 tests. Both run through `cargo_fast.sh` in
  the existing `kagemusha-v1` target slot. Scoped formatting and diff whitespace
  checks pass; these shared-checkout results do not qualify the full workspace.

- 2026-09-28: fresh current-source compilation reaches Core and Torii production
  libraries; Core test compilation finds a retired SCCP permission name in an
  existing refusal test, now corrected to the canonical route-proposal permission.
  The codec guard and 79 focused capacity/provider-ingest/TLS contract tests pass.
  Previous broad test counts remain historical evidence while shared sources change.
- The current consensus migration exposed two active integration gaps: publication
  still read retired finality sidecars, and the daemon explicitly refused configured
  provider/reputation archives because the current executor did not capture them.
  Publication now uses current embedded certificates and a compact independently
  trusted checkpoint; model and native validation are pending. Current archive
  capture is being connected to commit completion with exact-decision retry, while
  obsolete archive-specific V2 receipt/publication plumbing is being removed.
  TODO: finish equivalent current archive failure/restart tests before retiring their
  old path's assertions, then qualify native constructors and all four-validator
  workflows. SR3/SR4/SR5/SR8 remain active.

- Current compact checkpoint/publication model tests pass all 11 selected cases.
  The CAR library passes all 362 tests on the current source. The replacement
  archive path now binds to the current executor before the driver starts,
  retains one committed decision after a capture failure, and retries capture
  without reexecuting its transactions or emitting completion twice. Equivalent
  current-certificate authority, I/O failure, contention and restart tests are
  present; native compilation and their execution are still required. Retired
  archive-only V2 reservation/publication types and their daemon replay setup
  are removed, with release-gate selectors migrated to the current cases.
- Current-source storage/retrieval suites finish successfully: `sorafs_node`
  1,565 passed/two ignored, `sorafs_car` 362 passed, and `sorafs_orchestrator`
  227 passed/two ignored. The updated provider-ingest source-contract suite
  passes 29 cases. These scoped results preserve SR1/SR2/SR6/SR7 acceptance;
  Core/Torii/daemon native tests and four-validator workflow closure remain open.
- Native test compilation exposes three migrated-test API mismatches (a required
  validator resume method, a private block-hash writer helper, and removed archive
  fields in a refusal pattern); each is corrected without changing refusal
  semantics. Compiler diagnostics also identify obsolete archive reservation
  APIs. Those and the old archive-only Kura lease helpers are removed completely;
  replacement tests use the actual current index writer, certified chain and
  insertion implementation, including release notification and partial-I/O retry.
  A fresh native compile and execution remain required after these corrections.
- The first executable Core query pass runs 704 tests: 691 pass and 13 fail.
  Six failures are new fixture setup errors (foreign-network Kura rebinding and
  five missing orderbook/reserve policy setups); their fixtures are corrected
  while preserving the refusal checks and using signed genesis policy grants.
  Seven broader asset-query fixtures fail with `FASTPQ source has no retained
  producer invocation`; those are outside the SoraFS changes. Three current
  executor archive tests reach the same missing-policy setup error, also fixed.
  All corrected cases require rebuilt execution; these results do not close SR8.
- Independent review finds that commit retries reset backoff during re-prepare.
  The driver now retains failure counts after durable append until commit succeeds,
  with regressions for exponential delay/cap, failed re-prepare and reset on the
  next block. The restart fixture now joins its executor and releases all archive
  owners before reopening, and tests partial startup capture followed by exact retry.
