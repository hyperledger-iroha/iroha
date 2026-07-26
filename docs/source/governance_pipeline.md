% Governance Pipeline (Iroha 2 and SORA Parliament)

The broader target doctrine, economic model, and mechanism maturity matrix are
defined in [`sora_adversarial_constitution.md`](./sora_adversarial_constitution.md).
This document records current runtime behavior; aspirational mechanisms in the
constitution are not active until implemented and separately enacted.

# Current state (v1)
- Validation-fee governance runs as: bonded citizen proposal → immutable
  proposal-time seven-body Parliament approvals → PLAIN citizen referendum →
  approved close → enactment. A citizen ballot cannot open or bypass the
  Parliament gate.
- Parliament selection uses deterministic, independently domain-separated
  draws over bonded citizens; the citizenship bond is an anti-Sybil/collateral
  floor and does not increase draw odds above the minimum. Each proposal body
  uses `min(configured target, eligible citizens)` members, rejects only the
  zero-candidate case, and computes quorum from the actual immutable roster.
  The same citizen may serve in all seven bodies.
- The first validation-fee release supports PLAIN finalization only. `h_end` is
  inclusive; close/tally executes at `h_end + 1`, evidence is anchored to
  `h_end`, and ballot locks must remain active through that height. Other
  governance paths may retain ZK behavior where explicitly configured. The
  Taira validation-fee profile retains an exact 3,600-block window, so
  `h_end = h_start + 3,599`.
- At the `h_start` boundary, after all seven proposal-local Parliament bodies
  have reached quorum, consensus freezes the complete PLAIN electorate. The
  snapshot binds the proposal id and operator, Parliament approval-gate height,
  capture height, canonical member records, member count, and a
  domain-separated roster root. It is absent before opening and mandatory once
  the referendum is open; later citizen-registry changes cannot add, remove, or
  alter eligible voters. Ballot admission and live/final tallying fail closed
  on a non-member or a lock that differs from the retained rules.
- Every validation-fee proposal fingerprints and retains its exact
  `plain_electorate_rules`. Ballots use the retained asset, amount, duration,
  conviction, turnout, approval threshold, member cap, and citizen gate even
  if live governance configuration later changes. The typed
  `/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft` route derives
  these immutable fields and rejects duplicate effective ballots; clients
  supply only the owner and closed AYE/NAY/ABSTAIN direction.
- An enacted validation-fee policy is valid only when
  `effective_from_height = enacted_at_height + 120,960`; an earlier or later
  activation, or height overflow, is rejected. The proof-bearing current-policy
  flow rechecks that relation and projects the exact proposal rules together
  with the frozen-roster root, member count, capture height, and approval-gate
  height. Clients must complete every bounded proof page and verify the finality
  chain, registry witness, chain/genesis binding, and policy-chain genesis
  before enabling fees.
- Validator misconduct is acted on via the evidence pipeline (`/v1/sumeragi/evidence*`, CLI helpers) with joint-consensus hand-offs enforced by `NextMode` + `ModeActivationHeight`.
- Protected namespaces, runtime-upgrade hooks, and governance manifest admission are documented in `governance_api.md` and covered by telemetry (`governance_manifest_*`, `governance_protected_namespace_total`).

# In-flight / backlog
- Publish VRF draw artifacts (seed, proof, ordered roster, alternates) and codify replacement rules for no-shows; add golden fixtures for the draw and replacements.
- Stage-SLA enforcement for the Parliament bodies (rules → agenda → interest → review → policy jury → oversight/FMA → enact) needs explicit timers, escalation paths, and telemetry counters.
- Policy-jury secret/commit–reveal voting and associated bribery-resistance audits remain a hardening item on top of the signed clear-ballot path.
- Misconduct slashing for high-risk bodies and cooldowns between service slots require configuration plumbing plus tests.
- Governance lane sealing and referenda window/turnout gates are tracked in `gov.md`/`status.md`; keep the roadmap entries updated as the remaining acceptance tests land.
