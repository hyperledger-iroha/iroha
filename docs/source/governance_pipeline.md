% Governance Pipeline (Iroha 2 and SORA Parliament)

# Current state (v1)
- Governance proposals run as: proposer → Parliament stage ballots → enactment. Referendum records remain as scheduling/audit envelopes, but SORA Parliament decisions are made by equal signed ballots from seated body members.
- Parliament selection uses deterministic, domain-separated draws over bonded citizens; the citizenship bond is an anti-Sybil/collateral floor and does not increase draw odds above the minimum. When no persisted roster exists, Torii derives fallback rosters from the bonded citizen registry.
- Legacy referendum voting modes remain available outside the Parliament decision path: ZK (default, requires `Active` VK with inline bytes) and Plain (quadratic weight). Parliament policy-jury decisions use equal signed citizen ballots instead of token-lock weight.
- Validator misconduct is acted on via the evidence pipeline (`/v1/sumeragi/evidence*`, CLI helpers) with joint-consensus hand-offs enforced by `NextMode` + `ModeActivationHeight`.
- Protected namespaces, runtime-upgrade hooks, and governance manifest admission are documented in `governance_api.md` and covered by telemetry (`governance_manifest_*`, `governance_protected_namespace_total`).

# In-flight / backlog
- Publish VRF draw artifacts (seed, proof, ordered roster, alternates) and codify replacement rules for no-shows; add golden fixtures for the draw and replacements.
- Stage-SLA enforcement for the Parliament bodies (rules → agenda → interest → review → policy jury → oversight/FMA → enact) needs explicit timers, escalation paths, and telemetry counters.
- Policy-jury secret/commit–reveal voting and associated bribery-resistance audits remain a hardening item on top of the signed clear-ballot path.
- Misconduct slashing for high-risk bodies and cooldowns between service slots require configuration plumbing plus tests.
- Governance lane sealing and referenda window/turnout gates are tracked in `gov.md`/`status.md`; keep the roadmap entries updated as the remaining acceptance tests land.
