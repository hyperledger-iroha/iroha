# Hyperledger Iroha v2 Threat Model

_Last reviewed: 2025-11-07 - Next scheduled review: 2026-02-05_

Maintenance cadence: Security Working Group with component owners (<=90 days). Every revision is summarised in `status.md` with ticket links for open risks.

## Assumptions and Non-Goals

- **Assumptions:** permissioned validator network; peers mutually authenticate via mTLS with key pinning. Clients are untrusted. Operator compromise and Internet-scale DoS are in scope.
- **Non-goals:** full supply-chain provenance (tracked in build-hardening docs), quantum-resistance, UI/UX phishing. Runtime-relevant supply-chain gates appear here when they affect execution or consensus safety.

## Scope

- Runtime upgrades and manifest handling
- Sumeragi consensus (leader election, voting, RBC, view-change telemetry)
- Torii application APIs (REST/WebSocket ingress, authN/Z, rate limiting)
- Attachments and payload handling (transactions, manifests, large blobs)
- Zero-knowledge pipeline (proof generation, verification, circuit governance)
- Key and identity management (validator, operator, release signing)
- Network and transport (P2P/ingress handshake, connection management, DoS)
- Telemetry and logging (export, privacy, integrity)
- Membership and governance (validator set changes, eviction, configuration drift)

Threat actors: Byzantine peers, malicious clients, compromised operators, Internet-scale DoS actors. Supply-chain and compiler threats are handled in build-hardening; runtime enforcement hooks are tracked here.

## Asset Classification

Impact scale (used below): **Critical** - safety/liveness break or irreversible ledger corruption; **High** - liveness or trust-boundary break needing operator intervention; **Moderate** - degraded service or bounded exposure; **Low** - minimal impact.

| Asset | Description | Integrity | Availability | Confidentiality | Owner |
| --- | --- | --- | --- | --- | --- |
| Ledger state (WSV and blocks) | Canonical replicated history | Critical | Critical | Moderate | Core WG |
| Upgrade manifests and release artifacts | Scripts, binaries, attestations | Critical | High | High | Runtime WG |
| Release-signing keys | Authorise manifests/releases | Critical | High | High | Security WG |
| Consensus keys | Validator voting/commitment keys | Critical | High | High | Validators |
| Client credentials | API keys/tokens/multisig contexts | High | High | Critical | Torii WG |
| Peer membership registry | On-chain/off-chain peer list per epoch | Critical | High | Moderate | Consensus WG |
| Attachment storage | Off-chain blobs + on-chain refs | High | High | Moderate | Runtime WG |
| Norito/Kotodama codecs | Serialization rules | Critical | High | Moderate | Data Model WG |
| ZK proving material | Circuits, verification keys, witnesses | High | Moderate | High | ZK WG |
| Telemetry and audit logs | Metrics, health, security logs | Moderate | High | Low/Moderate | Observability WG |
| Node configuration and secrets | mTLS certs, tokens, config | High | High | High | Ops |

## Trust Boundaries

- Peer boundary (authenticated P2P; peers may act Byzantine)
- Client ingress boundary (Torii REST/WebSocket/CLI exposed to untrusted clients)
- Upgrade control boundary (operators with manifest/signing/config access)
- Attachment ingress boundary (attachments via Torii/gossip)
- ZK workload boundary (off-chain proving infrastructure)
- Telemetry export boundary (metrics/logs shipped externally)
- Membership governance boundary (processes that add/evict validators)

## Risk Rubric

- **Likelihood:** Rare / Unlikely / Possible / Likely / Almost certain
- **Severity:** Low / Moderate / High / Critical (per asset scale)
- **Risk level:** Matrix product. Critical or High risks require mitigation before GA unless formally accepted by Security WG chair + owning WG; accepted risks are tracked in `status.md`.
- **Response targets:** P1 <=7 days, P2 <=30 days, P3 <=90 days unless accepted.

## Threat Scenarios

Each area lists **Current controls** (implemented today) and **Outstanding gaps** (tracked in the residual-risk table).

### Runtime Upgrades

**Current controls**
- Detached manifest signature verification using release keys defined in governance manifests.
- Admission compares `abi_hash` and `code_hash`; mismatches reject deployment (tested in `crates/iroha_core/tests/runtime_upgrade_admission.rs`).
- Upgrade rollouts require governance approval and support deterministic rollback to the prior build.
- Config pinning via `iroha_config` prevents unexpected ABI versions during admission.

**Outstanding gaps**
- Release-signing keys still co-located with validator ops environments (**see residual risks: Release-signing key separation**).
- Supply-chain attestations (SBOM/SLSA) enforced only in CI, not during runtime admission (**see residual risks: Upgrade SBOM provenance gap**).

### Sumeragi Consensus

**Current controls**
- Quorum assembly records double-vote evidence (`Evidence::DoublePrevote` / `Evidence::DoublePrecommit`).
- RBC payload hashes and size clamps; READY/DELIVER gating validated by tests in `crates/iroha_core/tests`.
- Pacemaker timers bounded via `iroha_config::sumeragi.timers` (see `status.md`, Sep 10 2025 update).
- Collector path verifies ExecWitness roots before commit; evidence triggers peer quarantine.

**Outstanding gaps**
- Automatic membership reconciliation alerting is being addressed by the
  `sumeragi_membership_view_hash` telemetry gauge (hashes of the active view + epoch),
  emitted every round and scraped by the Ops dashboard to flag divergent peers.
  Rollout is scheduled alongside the April 2026 observability bundle (see residual
  risks: Membership registry reconciliation).
- Compromised peer eviction playbook lacks automated tooling for epoch reconfiguration (**see residual risks: Membership registry reconciliation**).

### Torii Application APIs

**Current controls**
- Norito JSON schema validation with depth and size caps; per-route body limits (tests under `crates/iroha_torii/tests`).
- Route registration isolated in builder helpers; unsupported feature combinations rejected in tests (status.md Sep 10 2025).
- Rate limiting per account and credential; adaptive backoff for repeated failures (see `crates/iroha_torii/src`).
- Operator endpoints can require mTLS; audit trails keyed to `AccountId`/`DomainId`.

**Outstanding gaps**
- Connection-level handshake throttling and circuit breakers are not yet implemented (**see residual risks: Pre-auth DoS controls**).
- WebAuthn-based operator auth remains gated behind development flag (**see residual risks: Torii operator auth hardening**).

### Attachments and Payloads

**Current controls**
- Size caps on attachments (`handle_post_attachment` enforces max bytes from config or 4 MiB fallback).
- Deterministic ID (Blake2b-32) and TTL-based GC for stored attachments.
- Content type recorded for auditing; attachments stored under dedicated directory with atomic writes.

**Outstanding gaps**
- No magic-byte sniffing or sandboxed parsing; decompression guards limited to size caps (**see residual risks: Hardware-accelerated hashing** for verification). Sanitisation plan: add Norito attachment metadata hashing + deterministic magic-byte validation in Torii (reject mismatched MIME/ext), route opaque payloads through a Firecracker jail with time/CPU caps for decompression, and land nightly ClamAV scans over persisted attachments. Work tracked under OPS-2217 / targeted for M5 (May 2026).
- Export sanitisation and format-specific safeguards pending design.

### Zero-Knowledge Pipeline

**Current controls**
- Circuit hash validation via manifest metadata; hosts reject unknown circuits (see `crates/iroha_core/tests/ivm_manifest_abi_reject.rs`).
- Proof queue sizing and worker isolation enforced in Torii ZK module; metrics cover queue depth and rejects.
- Witness material stored encrypted on disk with TLS between node and prover (configurable in `iroha_torii`).

**Outstanding gaps**
- Trusted-setup artifact governance formalisation pending (**see residual risks: ZK circuit governance**).
- Automatic witness shredding verification and audit logs still manual. Remediation plan: extend the witness retention service to emit a Norito `WitnessPurgeReportV1` with `{circuit_id, witness_hash, shredder_node, purge_timestamp}` for every deletion, back the payload with Dilithium3 signatures, and add a nightly auditor job that samples 5 % of purge events, replays the deletion, and stores attestations in `kura.audit`. Tracking ticket OPS-2244, targeting rollout with the June 2026 privacy milestone.

### Key and Identity Management

**Current controls**
- Validators support key rotation through governance actions; consensus keys stored via configured key providers.
- Client credentials scoped per account/domain; CLI and Torii enforce token expiry.
- Secrets scanning in CI (`scripts/inventory_serde_usage.py` pipeline reused for secret detection).

**Outstanding gaps**
- Broad HSM adoption for validator keys is incomplete (**see residual risks: Validator key HSM adoption**).
- Release-signing key separation from validator infrastructure outstanding (**see residual risks: Release-signing key separation**).

### Network and Transport

**Current controls**
- mTLS with certificate pinning for validators; TLS renegotiation disabled.
- Gossip ingress limited via per-peer quotas configured in `iroha_p2p` (status.md Sep 10 2025 updates on RBC).

**Outstanding gaps**
- No dedicated connection-gating/handshake budget enforcement at Torii ingress (**residual risks: Pre-auth DoS controls**).
- Peer churn telemetry exported via `p2p_peer_churn_total{event}`; operators can alert on sustained `disconnected` spikes.

### Telemetry and Logging

**Current controls**
- Structured logging with Norito JSON writers; sensitive fields redacted per logging guidelines (`docs/source/telemetry.md`).
- Prometheus endpoints served over TLS; scrape tokens configurable in `iroha_config`.

**Outstanding gaps**
- Formal redaction lint/CI checks and tamper-evident shipping not yet implemented (**see residual risks: Telemetry redaction policy**).

### Time and Randomness

**Current controls**
- Monotonic clocks used for pacemaker timers (see `status.md` Sep 1 2025 NTS update).
- Nodes rely on OS CSPRNG; no custom RNG fallbacks.

**Outstanding gaps**
- NTS-backed or multi-source time validation not enforced at runtime (**see residual risks: Time and NTP hardening**).

## Residual Risks and Tracking

| Risk | Status | Mitigation Plan | Owner | Target |
| --- | --- | --- | --- | --- |
| Upgrade SBOM provenance gap | Open | Integrate SLSA Level 3 attestations into runtime admission (`SEC-147`) | Security WG | 2025-11-30 |
| Aggregator fairness audit | Open | Commission third-party review; publish before Milestone 2 GA (`SUM-203`) | Consensus WG | 2025-12-15 |
| Torii operator auth hardening | Open | Ship WebAuthn behind feature flag; evaluate rollout (`TOR-118`) | Torii WG | 2025-11-15 |
| Hardware-accelerated hashing | Open | Implement multiversion hashing with deterministic fallback (`RNT-092`) | Runtime WG | 2025-12-01 |
| ZK circuit governance | Open | Draft governance protocol and tooling (`ZK-077`) | ZK WG | 2025-11-20 |
| Validator key HSM adoption | Open | Define policy and reference deployment (tracked via `roadmap.md` entry *Security & privacy hardening — SNNet-15H*) | Security WG | 2025-11-15 |
| Release-signing key separation | Open | Offline root with threshold signing (tracked via `roadmap.md` Milestone R3 release runbook) | Security WG | 2025-10-31 |
| Membership registry reconciliation | Open | Enforce view-hash checks and halt on mismatch (`SUM-203` follow-up) | Consensus WG | 2025-10-25 |
| Pre-auth DoS controls | Open | Connection gating and handshake caps implemented (`preauth_*` config, `torii_pre_auth_reject_total`), continue tuning via follow-up ticket | Torii WG & Core WG | 2025-10-31 |
| Telemetry redaction policy | Open | Redaction lints and CI checks (see `status.md` — Latest Updates, Nov 28 2025) | Observability WG | 2025-10-20 |
| Time and NTP hardening | Open | NTS or multi-source bounds (status tracked in `status.md` Time Service section) | Runtime WG & Ops | 2025-11-10 |
| Membership mismatch telemetry | Open | `sumeragi_membership_mismatch_total` metric landed; wire alerting and operational runbook | Consensus WG | 2025-10-15 |
| Attachment sanitisation | Open | Design magic-byte sniffing, sandboxing, export guards (align with `docs/source/security_hardening_requirements.md`) | Runtime WG | 2025-11-30 |
| Witness retention audit | Open | Automate witness shredding verification (requirements captured in `security_hardening_requirements.md`) | ZK WG | 2025-11-05 |
| Peer churn telemetry | In progress | Metric `p2p_peer_churn_total` shipped; wire alert thresholds and dashboards via observability runbook | Core WG | 2025-10-25 |

## Review Process

1. Security WG runs a review every <=90 days and before each release candidate.
2. Component owners update mitigations, telemetry coverage, and risk status.
3. Document is amended; `status.md` gains a summary with ticket links.
4. Incidents or near misses trigger an addendum within seven days of discovery.

## Sign-off Checklist

| Working Group | Point of Contact | Status | Notes |
| --- | --- | --- | --- |
| Security WG | security@iroha | Pending | Circulate document; collect acknowledgements and ticket references by 2025-10-05. |
| Core WG | core@iroha | Pending | Confirm pre-auth DoS roadmap and churn telemetry alerting coverage. |
| Runtime WG | runtime@iroha | Pending | Confirm attachment sanitisation and SBOM gate actions. |
| Torii WG | torii@iroha | Pending | Validate auth hardening (`TOR-118`) and connection gating schedule. |
| Consensus WG | consensus@iroha | Pending | Provide fairness audit schedule and membership telemetry plan. |
| Data Model WG | data-model@iroha | Pending | Confirm Norito/Kotodama coverage and fuzz corpus maintenance. |
| ZK WG | zk@iroha | Pending | Verify circuit governance (`ZK-077`) and witness retention audit plan. |
| Observability WG | observability@iroha | Pending | Redaction policy enforcement and tamper-evident logging roadmap. |
| Ops | ops@iroha | Pending | HSM rollout for validators and NTP/NTS implementation plan. |

## References

- `docs/source/new_pipeline.md`
- `status.md`
- `roadmap.md` Milestone 0
- Build-hardening documentation (supply-chain provenance, SBOM, attestations)
