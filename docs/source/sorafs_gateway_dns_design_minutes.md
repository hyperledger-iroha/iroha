---
title: SoraFS Gateway & DNS Design Kickoff Minutes
summary: Decision, evidence, and close-out record for the 2025-03-03 SoraFS gateway and DNS kickoff.
---

# Gateway & DNS Kickoff - 2025-03-03

- **Moderator:** Networking TL
- **Scribe:** Docs/DevRel Observer
- **Attendees:** Networking TL, Ops Lead, Storage Team Rep, Tooling WG Rep,
  Governance delegate, QA Guild Lead, Torii Platform Rep, Security Engineering,
  Docs/DevRel Observer
- **Evidence bundle:** `artifacts/sorafs_gateway_dns/20250302/runbook_bundle/`
- **Related runbook:** `docs/source/sorafs_gateway_dns_design_runbook.md`

## Agenda Checkpoints

1. Deterministic DNS host derivation and SoraDNS automation ownership.
2. GAR policy enforcement, telemetry, and incident escalation.
3. Gateway hardening, direct mode, TLS/ECH automation, and self-cert evidence.
4. SF-5a conformance coverage, attestation output, and CI entry points.
5. Owner assignments and post-kickoff documentation updates.

## Decisions

1. **DNS automation baseline:** use the checked-in SoraDNS `xtask` helpers for
   host derivation, GAR template generation, binding headers, ACME SAN plans,
   and signed RAD/directory releases. Ops owns execution; Networking owns
   deterministic host policy review.
2. **GAR policy engine:** keep GAR evaluation in the shared
   `sorafs_manifest::gateway` policy surface and distribute signed Norito
   artifacts to operators instead of introducing a separate per-operator
   policy schema.
3. **Configuration ownership:** production gateway behavior must be surfaced
   through configuration bundles and documented operator guides. Runtime-only
   secrets stay out of repository files.
4. **TLS/ECH hand-off:** SF-5b TLS/ECH automation is handled by
   `docs/source/sorafs_gateway_tls_automation.md`; gateways capture evidence
   with `scripts/sorafs_gateway_self_cert.sh` and
   `cargo xtask sorafs-gateway-probe`.
5. **Conformance acceptance:** SF-5a requires fixture replay, negative cases,
   load evidence, and signed attestation verification through
   `ci/check_sorafs_gateway_conformance.sh` and
   `cargo xtask sorafs-gateway-attest --verify`.
6. **Escalation evidence:** GAR violations, probe summaries, TLS/ECH state, and
   conformance reports must be archived with hashes before operator
   self-certification.

## Action Items

| Owner | Due | Status | Action |
|-------|-----|--------|--------|
| Networking TL | 2025-03-04 | Complete | Publish deterministic host decisions and route-label expectations to the SoraDNS docs. |
| Ops Lead | 2025-03-04 | Complete | Archive the mock-run evidence bundle and record hashes in governance storage. |
| Tooling WG | 2025-03-06 | Complete | Keep conformance CI and attestation verification aligned with the fixture bundle. |
| QA Guild | 2025-03-06 | Complete | Maintain negative and load coverage expectations in the gateway conformance plan. |
| Docs/DevRel | 2025-03-06 | Complete | Replace future-tense pre-read wording with an outcome brief and link these minutes. |
| Security Engineering | 2025-03-07 | Complete | Confirm TLS/ECH fallback and GAR escalation hooks are captured in operator playbooks. |

## Evidence

- Runbook snapshot: `docs/source/sorafs_gateway_dns_design_runbook.md` section
  9 records the 2025-03-02 mock run.
- GAR telemetry appendix:
  `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
- Attendance and owner register:
  `docs/source/sorafs_gateway_dns_design_attendance.md`.
- TLS/ECH operator hand-off:
  `docs/source/sorafs_gateway_tls_automation.md`.
- Conformance plan: `docs/source/sorafs_gateway_conformance.md`.

## Open Follow-ups

- Anonymous and blinded-CID access remain tied to SNNet-2 governance and
  telemetry decisions.
- Live environment cutovers must append evidence to `status.md` and the
  relevant operator runbooks.
- Any future conformance or gateway-probe regression should be linked from this
  file and the attendance/action tracker.

## Close-out

The 2025-03-04 close-out confirmed that attendee follow-ups, owner assignments,
mock-run evidence, TLS/ECH hand-off, and conformance responsibilities were
captured in the repository. The old pre-read remains as
`docs/source/sorafs_gateway_dns_design_pre_read.md`, but its body is now an
outcome brief so readers do not treat the kickoff as pending work.
