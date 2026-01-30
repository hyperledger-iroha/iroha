---
lang: pt
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2026-01-03T18:08:00.913452+00:00"
translation_last_reviewed: 2026-01-30
---

% Governance Pipeline (Iroha 2 and SORA Parliament)

# Current state (v1)
- Governance proposals run as: proposer → referendum → tally → enactment. Referendum windows and turnout/approval thresholds are enforced as described in `gov.md`; locks are extend-only and unlock on expiry.
- Parliament selection uses VRF-based draws with deterministic ordering and term bounds; when no persisted roster exists, Torii derives a fallback using `gov.parliament_*` config. Council gating and quorum checks are exercised in `gov_parliament_bodies` / `gov_pipeline_sla` tests.
- Voting modes: ZK (default, requires `Active` VK with inline bytes) and Plain (quadratic weight). Mode mismatches are rejected; lock creation/extension is monotonic in both modes with regression tests for ZK and plain re-votes.
- Validator misconduct is acted on via the evidence pipeline (`/v1/sumeragi/evidence*`, CLI helpers) with joint-consensus hand-offs enforced by `NextMode` + `ModeActivationHeight`.
- Protected namespaces, runtime-upgrade hooks, and governance manifest admission are documented in `governance_api.md` and covered by telemetry (`governance_manifest_*`, `governance_protected_namespace_total`).

# In-flight / backlog
- Publish VRF draw artifacts (seed, proof, ordered roster, alternates) and codify replacement rules for no-shows; add golden fixtures for the draw and replacements.
- Stage-SLA enforcement for the Parliament bodies (rules → agenda → study → review → jury → enact) needs explicit timers, escalation paths, and telemetry counters.
- Policy-jury secret/commit–reveal voting and associated bribery-resistance audits are still to be implemented.
- Role-bond multipliers, misconduct slashing for high-risk bodies, and cooldowns between service slots require configuration plumbing plus tests.
- Governance lane sealing and referenda window/turnout gates are tracked in `gov.md`/`status.md`; keep the roadmap entries updated as the remaining acceptance tests land.
