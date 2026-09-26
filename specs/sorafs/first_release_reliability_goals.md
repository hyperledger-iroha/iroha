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
| SR1 | One ordered durable storage index commit path; ordinary reads never rewrite the global index. Immutable metadata is shared without inventory-sized read clones. | Storage | Barrier-controlled read-A/ingest-B and read-A/evict-B interleavings survive restart; acknowledged manifests remain present; evicted manifests never reappear; failed/uncertain writes preserve explicit durability semantics. Read cost and persistence do not scale with the global inventory. | active |
| SR2 | Expired manifests retire independently of repeated/shared content. | Storage/GC | Two expired manifests sharing bytes are reclaimed; repeated chunks within one manifest do not block GC; an unexpired sharing manifest remains readable; capacity/refcounts/audits remain exact after restart. | active |
| SR3 | Discovery consumes current governed finalized admission, renewal and revocation state. | Core/DataModel/Torii | Production constructor and same-State reader accept valid governed admission, observe live renewal/revocation, evict stale adverts, reject substituted/stale heads and preserve revocation across restart. Local envelope directories are not a second authority. | active |
| SR4 | Fresh production tokens and authenticated publication complete through real authority. | Core/Torii/daemon/publisher | Reserve, Complete and challenged Check evidence authenticates exact execution and finality; issuer produces a usable token through production constructors. Authenticated initial-source staging breaks the empty-provider bootstrap cycle. Publisher waits for exact finalized registration, assignments and provider completion, then verifies every asset before declaring success. Recovery never double-spends or repeats a completed signature. | active |
| SR5 | Payload corruption permits authenticated degraded startup and bounded repair. | Storage/repair | Corrupt/missing chunks quarantine only affected manifests; healthy objects serve; corrupted objects cannot serve or produce successful proofs. Finalized lease-bound repair restores bytes and verifies original commitments before clearing quarantine. Metadata/authority corruption still rejects startup. | active |
| SR6 | Retrieval retains bounded working memory and incrementally verifies canonical content. | CAR/orchestrator | Consuming sink and bounded reorder window release committed chunks; memory is bounded independently of total object size; chunk/root/plan/CAR integrity and deterministic output remain identical; cancellation, truncation, corruption and sink failure release resources. Public SDK/CLI consumers use the canonical new path. | active |
| SR7 | Gateway throttling and policy errors preserve typed meaning across scheduling. | CAR/gateway/orchestrator | Actual HTTP 429 honors bounded Retry-After and byte/request budgets; throttling does not mark a provider unhealthy; retry deadlines and cancellation remain bounded; full-path policy-denial tests retain structured evidence. | active |
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
