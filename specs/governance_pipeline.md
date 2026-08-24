% Governance Pipeline (Iroha 2 and SORA Parliament)

The broader target doctrine, economic model, and mechanism maturity matrix are
defined in [`sora_adversarial_constitution.md`](./sora_adversarial_constitution.md).
This document records current runtime behavior; aspirational mechanisms in the
constitution are not active until implemented and separately enacted.

# Current state (v1)
- Validation-fee governance runs as: an operator-bound native proposal →
  deterministic Parliament body election → timed-private OVN ballot attempts →
  one canonical Parliament certificate → enactment at the certified due height.
  No validation-fee policy or payout-lifecycle preimage accepts a PLAIN voting
  mode, public electorate, referendum window, or public finalization evidence.
- Every validation-fee proposal fingerprints its canonical execution authority
  as `proposal_operator`. The operator must equal the governance record's
  proposer and the operator retained with the protected registry certificate;
  proposal reuse, restoration, projection, and enactment fail closed on any
  mismatch. Transaction ordering therefore cannot attach an identical policy
  payload to a different retained operator.
- Parliament attempts use consensus-derived, domain-separated identifiers for
  the proposal content, governance attempt, body election, body instance,
  ballot attempt, TLE key session, and release session. Registration,
  survivor, timed-commitment, release, and retry windows are deterministic
  block-height intervals. Retry and accepted-corpus bounds are consensus-hashed
  configuration, not environment-variable switches.
- The timed-private ballot certificate retains the exact sortition request,
  roster/assignment/result roots, ballot-attempt and TLE bindings,
  registration/dropout/survivor/corpus/no-recovery/commitment/opening roots,
  release pulse, aggregate tally, and terminal outcome. The no-recovery root is
  mandatory: there is no public-ballot or plaintext fallback path when a timed
  release attempt fails.
- A validation-fee authorization retains the complete certificate and its
  canonical `GovernanceCertificateId`. Validation rejects a structurally
  invalid certificate, a non-canonical certificate id, a certificate whose
  proposal-content id differs from the exact operator-bound proposal
  fingerprint, or enactment at a height other than the certificate's due
  height. Summary APIs may expose the id and heights; proof/detail APIs retain
  the complete certificate for independent validation.
- An epoch council roster is immutable after its first successful persistence.
  Replaying the exact same roster is idempotent and cannot consume another
  service slot or extend a cooldown. A citizen cannot release the citizenship
  bond while seated in a current or scheduled council/body roster, retained by
  an active proposal electorate, or holding an active referendum ballot.
- An enacted validation-fee policy is valid only when
  `effective_from_height = enacted_at_height + 120,960`; an earlier or later
  activation, or height overflow, is rejected. The proof-bearing current-policy
  flow rechecks that relation and projects the exact proposal operator,
  certificate id, complete certificate, certification height, and enactment-due
  height. Clients must complete every bounded proof page and verify the finality
  chain, registry witness, chain/genesis binding, policy-chain genesis, and both
  certificates before enabling fees.
- Validator misconduct is acted on via the evidence pipeline (`/v1/sumeragi/evidence*`, CLI helpers) with joint-consensus hand-offs enforced by `NextMode` + `ModeActivationHeight`.
- Protected namespaces, runtime-upgrade hooks, and governance manifest admission are documented in `governance_api.md` and covered by telemetry (`governance_manifest_*`, `governance_protected_namespace_total`).

# In-flight / backlog
- Complete end-to-end retention of issued certificates through every governance
  execution and recovery path, then add multi-peer timed-release/retry fixtures.
- Add operational telemetry for attempt transitions and deterministic deadline
  misses without exposing private ballot contents.
- Misconduct slashing for high-risk bodies and cooldowns between service slots
  require configuration plumbing plus tests.
