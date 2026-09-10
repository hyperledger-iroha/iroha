---
title: SoraFS V1 Implementation Goals
summary: Dependency-ordered execution goals for the accepted first-release completion plan.
---

# SoraFS V1 Implementation Goals

Accepted scope: the **SoraFS V1 Production-Readiness Completion Plan** supplied
on 2026-09-06. The active implementation goal is to complete that plan, including
genuine reference-deployment qualification. This document orders the work; the
[closure ledger](v1_closure_ledger.md) remains the single index of implementation,
tests, documentation, blockers, and rollout evidence. Goals are not attestations.

## Design and acceptance decisions

- Ship one canonical V1 wire, state, API, and implementation per capability.
  Delete obsolete pre-release implementations, formats, compatibility branches,
  aliases, fallback APIs, and migration shims. Discard and reseed obsolete
  development state; do not design a migration path for it. Canonical account
  aliases remain part of the account-label model, not compatibility aliases.
- Use native committed ledger authority for orderbook, reserve/rent, repair,
  and moderation. Existing implementations must be reviewed and completed;
  do not create a second authoritative service database or ledger implementation.
- Use Norito binary and JSON envelopes, bounded inputs and retained state,
  finalized-block cursors, idempotency keys, durable outboxes/dead letters,
  rotation/revocation, payload-free logs, and deterministic integer/fixed-point
  computation. Production behavior comes from `iroha_config` with sensible
  defaults and explicit injection. File keys and environment overrides are
  development/test inputs only.
- The accepted reference deployment requires HSM-backed signing and PKCS#11/KMS
  key wrapping where specified by the plan. The present software-only SoraFS
  promotion contract is a known implementation gap, not an accepted exception.
  G02 replaces that acceptance profile and its producers/consumers together;
  do not retain a software-only compatibility profile for SoraFS promotion.
  This scope does not redesign unrelated node or consensus key custody.
- Preserve unrelated working-tree changes. This goal does not change
  `Cargo.lock`; avoid new crates unless an audited primitive is unavailable
  internally. Any resulting unresolved dependency prerequisite stays explicit.
- Automatic hedge execution remains disabled until venue, key, limit, and
  reconciliation policies are approved. HTTP/3 is non-applicable to V1.
- Keep keys, credentials, tokens, holder secrets, private evidence, PII, and
  model secrets runtime-only. Reference deployment and evidence qualification
  do not authorize Taira or Minamoto live mutation or public cutover.

## Ordered goals

`active` means work has begun, `queued` means authorized work awaits its
dependencies, and `complete` requires the stated result and recorded evidence.
Local tests cannot complete a goal whose result requires deployed execution.
Dependency lists govern closure; independent source work can proceed in parallel.

| Goal | Outcome and completion criteria | Owners / closure anchors | Depends on | State |
| --- | --- | --- | --- | --- |
| G01 | Reconcile every active plan, status, roadmap, and source marker with the closure ledger; verify the release baseline and inventory actual gaps. Freeze one V1 contract and remove remaining competing production paths. Re-run current-tree guards rather than relying on historical pass counts. | SoraFS maintainers; V1-C01–C04; source-marker and competing-authority audit; release automation. | — | active |
| G02 | Replace the SoraFS software-only release/custody profile with the required authenticated HSM/PKCS#11/KMS design. Update runtime qualification, config, adapters, evidence schemas, signers/verifiers, fixtures, and docs atomically. Prove non-exportable-key operation, authority binding, rotation/revocation, outage/recovery, and rejection of incompatible keys or downgraded evidence. A changed metadata label is insufficient. | Crypto, runtime-provider and release owners; V1-C02/C05/C06; all signer/key-wrapper runtime blockers. | G01 | active |
| G03 | Qualify and finish native orderbook, reserve/rent, repair, and moderation ISIs, queries, events, and builders. Prove bounded price-time matching with expected revision, admission sequence, quantities/states, atomic escrow partition/channel/trade/expiry/refund, governed authorities and receipt signers; atomic reserve custody/debt/cap/interest/lifecycle/appeal; repair lease/outcome/slash/appeal uniqueness; moderation finality. Remove remaining local authority. | Data model, Core, Torii and SDK owners; V1-C04; orderbook/reserve/repair/moderation rows. | G01 | active |
| G04 | Complete supervised matcher/settlement/expiry and retry-safe forwarding; live pricing feeds, exposure, billing, statements, acknowledgement/reconciliation, alerts, and governed hedge intents/execution adapter. Verify exact-once custody and publication with automatic execution disabled. | Orderbook, reserve and finance owners; `orderbook`, `reserve_rent`, `hedging_billing`, `appeal_finance`. | G02, G03 | active |
| G05 | Qualify authenticated PDP challenge/work/proof/status/export APIs and challenge-ID-bound PDP streaming; final dual-signed PoTR receipts with atomic tracker and governed provider ML-DSA binding; PoR latency/VRF/seed emitters. All failures converge on one native repair task and execute only under its finalized lease. | Proof/provider and repair owners; SF-2/SF-2c/SF-4; `pdp`, `por`, `potr`, `repair`. | G02, G03 | active |
| G06 | Finish authenticated finalized provider-ingest/reputation sources, durable ingest/outboxes and retention, external threshold signing material, and runtime adapters. Qualify the SFM-1 authority join's exact committed-state cache identity, single-flight rebuild, resource limits, metrics, and byte-identical replica results. | Provider, reputation and orchestrator owners; provider-ingest/reputation runtime blockers; SFM-1/SF-1; `reputation`. | G02, G03, G05 | active |
| G07 | Complete PoP deployment adapters and encrypted enrollment, dual approval, HSM issuance, registry submission/reconciliation/revocation, wallet custody, rollback-safe root synchronization, and local proof generation. Prove no credentials, witnesses, holder secrets, attestations, or PII enter the ledger or logs. | PoP, wallet, privacy and runtime-provider owners; PoP runtime blocker; `pop_credentials`. | G02, G03 | active |
| G08 | Complete signed screening/committee admission and authenticated chunked ChaCha20-Poly1305 quarantine with per-object keys, unique nonces, metadata AAD, PKCS#11/KMS wrapping, rewrap, atomic persistence/recovery, and range decryption. Prove quorum uniqueness, model/policy/signer/subject/evidence/score binding, freshness and replay rejection. | AI, crypto and quarantine owners; quarantine runtime blocker; `ai_prescreen`. | G02, G03 | active |
| G09 | Qualify the finalized-chain moderation lifecycle and exactly-once HSM-backed appeal settlement. Complete notification/publication/archival adapters and evidence case/session/manifest/segment/event/audit APIs. Enforce current assignment or explicit auditor/legal authority, WebAuthn, purpose-bound rotating grants, sessions ≤15 minutes, strict CSP/no-cache, visible watermarks, signed hash chains, retention/erasure and legal holds. | Moderation, appeal, viewer and runtime-provider owners; moderation/viewer and appeal-settlement blockers; `moderation_panel`, `appeal_finance`. | G02, G03, G07, G08 | active |
| G10 | Qualify the compliance controller, finalized hold/appeal producers, authenticated feed adapters, catalogs/acknowledgements, scoped controls, atomic promotion and last-known-good rollback. Prove HTTPS allowlists, SSRF/DNS-rebinding/pin/redirect/decompression/size/time defenses, deterministic normalization, threshold predecessor binding and precedence: legal/safety hold → accepted appeal → baseline. | Gateway, compliance and security owners; gateway-controller blocker; `gateway_compliance`. | G02, G03, G09 | queued |
| G11 | Complete finalized moderation/PoP/finance/viewer/compliance/hold/redaction producers and transparency leader leases, HSM signing, DAG anchoring, replicas, proofs, pagination/ETags and explorer hardening. Qualify per-subject clipping, k-suppression, rational epsilon, delta zero, exact integer discrete-Laplace sampling, durable composition budgets and hidden threshold-PRF cycle randomness with commitment-only publication. | Transparency, privacy and governance owners; `transparency`. | G02, G04, G07, G09, G10 | queued |
| G12 | Package genuine runtime backends and supervised two-instance Governance DAG/Kubo/head ingress; exercise authenticated CAS/failover, signer rotation, checkpoint/archive recovery, rollback and mirrors. Retain bounded authenticated JSON unless SF-12 capacity fails. Connect all real metric emitters, bounded labels, Prometheus rule tests, Grafana/query lint, alert routing, SLO burn alerts and runbooks. | DAG, deployment and observability owners; DAG runtime blocker; `governance_dag`; all lane metric contracts. | G02, G06, G11 | active |
| G13 | Complete public APIs/builders and identical positive/negative `ValidationOutcomeV1` results across Rust, JS/TS, Python, Swift, Kotlin/JVM, Java Android and C#, including native reference and DAG block/head validation. Regenerate canonical signed fixture inventory twice byte-identically. Produce clean pinned signed OpenAPI with both copies/version maps/clients, five-target binaries and mandatory smokes, strict Ed25519 signing, full-SHA workflow pins, deterministic archives, checksums, SBOM/scans, OIDC/cosign provenance, install tests and rollback/yank instructions. | SDK, API and release owners; V1-C01/C05; `reference_sdk_release`. | G02–G12 | active |
| G14 | Run the complete local, adversarial and distributed validation matrix; provision and qualify the genuine reference topology, 1,000-stream cold/warm/mixed load and 24-hour soak, independent security review and disaster-recovery rehearsal. Close every critical/high finding and release blocker against one reviewed candidate. | All component owners, independent security reviewers and reference operators; test/security/operations closure; `gateway_load`. | G03–G13 | active |
| G15 | Collect 17 fresh lane summaries and the trusted monotonic signed nine-prerequisite foundational envelope with one production context and active anchors. Run promotion, byte-identical replay and tamper/stale/missing/duplicate/predecessor/signature negatives, then the final promotion-bundle conjunction. Require `status=ready`, both summary counts =17, empty error lists and available rollback. Only separately authorized public-cutover work remains in the SoraFS roadmap. | Release/evidence owners; V1-C06/C07; L1/L2 promotion gates. | G01–G14 | queued |

G02 and G03 can proceed concurrently after the baseline inventory. G04–G12
can be divided by existing component ownership, with shared wire/config changes
coordinated before consumers change. SDK and release-tooling development can run
alongside service work, but G13 closes only against the final canonical surface.

## Required lane coverage

The exact 17 lanes remain owned by the aggregate checker and closure ledger:

| Lane | Primary execution goals |
| --- | --- |
| `ai_prescreen` | G02, G08 |
| `appeal_finance` | G02, G04, G09 |
| `gateway_compliance` | G10 |
| `gateway_load` | G14 |
| `governance_dag` | G02, G12 |
| `hedging_billing` | G04 |
| `moderation_panel` | G03, G09 |
| `orderbook` | G03, G04, G06 |
| `pdp` | G05 |
| `pop_credentials` | G02, G07 |
| `por` | G05 |
| `potr` | G02, G05 |
| `reference_sdk_release` | G02, G13 |
| `repair` | G03, G05 |
| `reputation` | G06 |
| `reserve_rent` | G03, G04 |
| `transparency` | G02, G11 |

Every lane also depends on G14 qualification and G15 final acceptance. The
foundational envelope order is exactly `SFM-1, SF-1, SF-2, SF-2c, SF-3, SF-4,
SF-5b, SF-6, SF-8a`; its exact nine-to-17 partition remains in the closure ledger.

## Validation and external prerequisites

Start with focused unit/property/model tests for modified behavior and the
release-helper suite. Broaden to `cargo build --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo fmt --all -- --check`, `cargo test --workspace --locked`,
`pytest pytests/scripts`, the SoraFS release/fixture guards and
`scripts/check_no_legacy_codec.sh`. Run Swift, Kotlin/JVM, Java Android,
JavaScript/TypeScript, Python and C# matrices with rebuilt matching native
artifacts. The four mandatory targets are Linux and macOS x86_64/aarch64;
Windows x86_64 is an additional artifact. Missing targets or skipped native
execution do not count as passes.

Adversarial qualification includes noncanonical/deep/oversized Norito,
overflow, duplicate/reordered records, replay/equivocation, stale/future time,
revoked/forged signers, damaged checkpoints, symlink/hardlink/TOCTOU and rename
crashes, poisoned storage, exhaustion/floods, escrow/settlement/repair races,
fork recovery, screening substitution, AEAD/key/tag/AAD/chunk failures, PoP
rollback/nullifier replay, viewer IDOR/WebAuthn replay, hostile compliance feeds,
split catalogs, privacy differencing and budget exhaustion.

Operators supply four voting validators with mandatory signed RS16 DA/RBC,
multiple admitted providers, two independently administered regional gateways,
two Governance DAG/Kubo instances, monitoring/alerting, HSM/KMS custody,
WebAuthn devices, signed trained model artifacts and runtime-only credentials.
Prove authenticated partition/loss acknowledgement before healing, view changes,
cross-peer simultaneous submissions, restart and key/root rotation, exactly one
repair/settlement, and identical recovered queries/balances/roots/proofs/bytes.
Load qualification requires ≥1,000 concurrent range streams in cold/warm/mixed
profiles with 1% injected corruption, advert rotation/revocation, failover,
malformed floods and rate-limit/denylist pressure. The subsequent 24-hour soak
must meet SF-5a/SLO limits with no proof failures, critical alerts or sensitive-log
leakage. Rehearse backup restoration, signer rotation, gateway/DAG failover and
rollback; independently review security and reject critical/high findings.

Each completed slice records its changed paths, exact validation command/result,
candidate identity and remaining deployment prerequisites in the closure ledger.
Missing external infrastructure does not stop independent local work. Synthetic
fixtures, local broker mocks, historical logs and metadata claims cannot supply
genuine deployment evidence. Operational checklists and future V2 guidance are
not active implementation markers.

## Initial checkpoint — 2026-09-06

- The release dependency expectation already includes `blake3==1.0.9` in
  `scripts/tests/check_sorafs_release_automation_test.py`, matching
  `scripts/requirements.txt`; no duplicate pin fix is needed. Current release
  regression progress and the first security fixes are recorded in the
  [2026-09-06 closure checkpoint](v1_closure_ledger.md#2026-09-06-execution-checkpoint).
- Existing native orderbook, reserve, repair and moderation instructions and
  finalized event queries mean Step 2 starts with validation/gap closure.
- Quarantine AEAD/range/rewrap, exact integer privacy accounting, the C# native
  reference wrapper and DAG block/head validators across the SDKs are present.
  Their goals require reviewed integration, negative tests and matching native
  execution, not replacement implementations merely because the input plan
  describes them as missing.
- `sorafs_external_software_signer` is a concrete packaged executable with
  native transaction, Governance, PoTR, billing, viewer and token signer
  adapters. It does not satisfy G02 or supply every required sealed-CAS,
  archive, notification, publication and key-wrapper backend.
- Four-peer moderation anchor/bond tests exist. Four-peer native orderbook,
  reserve and repair submissions and the full moderation orchestration corridor
  remain explicit test implementation work under G03/G14.
- `scripts/check_sorafs_production_promotion_bundle.py` currently requires
  `signing_backend=software` and `signer_qualification=software-key-qualified`.
  Related lane-inventory, resilience and runtime contracts must be changed and
  qualified together under G02. Their current success cannot close the new
  HSM-backed reference-release requirement.
- Both checked-in OpenAPI manifests currently contain `generator_dirty=true`;
  G13 must regenerate and sign them from the actual clean candidate rather than
  editing that metadata flag.
- No new production evidence or release readiness is asserted by goal creation.
