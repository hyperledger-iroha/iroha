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
| SR8 | Integrated first-release validation closes the seven findings. | Integration/release | Focused affected-crate tests, canonical wire roundtrips, codec guard, formatting and applicable lint pass. Four validators with mandatory signed RS16 DA/RBC exercise publish/replicate/retrieve/restart/repair/revoke through production constructors; no fixture adapter stands in for the authority under test. Broader workspace results and limitations are recorded separately. | queued |

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
