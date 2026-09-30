# Taira readiness and interrupted beacon activation — September 30, 2026

Fresh pinned SSH observations supersede the earlier source42 outage. Four
validators run `d431ce73b05a21f073e1e436e3806d96f9f25196`; all agree at height 10,
with a four-validator CommitQC containing three votes and empty queues.
Public HTTPS health, liveness, account capabilities, faucet policy and native MCP
respond successfully. All four direct `/readyz` checks return HTTP 503.
The deployed basic doctor omits readiness and passes despite this failure.

The exact retained beacon installation transaction
`5ba739ee2bf4e81f0e6ce932ec145a17f6391c16e4d6b5df86bd5baedc0c1519`
freshly resolves through curated MCP to global, state-resolved `Applied` at
block 8. The original journal remains `recovery_pending` at the submitted
beacon-install cursor; all four provider actions remain prepared. The native
installation proof and provider activation markers are absent. Public Applied
status alone does not replace the coordinator's authenticated installation proof.

One same-revision native read-only recovery attempt supplied the operator key
but failed with `RestartProof recovery requires --validator-operator-key`.
Its journal and live services remained unchanged. The deployed CLI discards
this argument for Canary recovery. Current source already forwards the key for
both phases, with regression coverage, in signed commit
`647f77a34be09efe435f57bb741b99cbff97288b`. A successor CLI cannot consume the
d431 inventory: current admission binds the inventory revision to its compiled
source even during recovery. The original forward lease has expired; repeating
apply after reaching a forward disposition can initiate rollback. No lease was
extended, no envelope was replaced, and no reset or new ledger write was performed.

The diagnostic correction requires exact plain-text `Ready` from `/readyz`
and bounds failed response bodies and machine error codes. It is not a live
repair. The current observer uses `SumeragiFinalityProof` and its native
CommitCertificate; d431 emits the retired V2 finality artifact. Its beacon bundle
also differs from the native phase-proof contract. A current coordinator cannot
authenticate the old install without retired protocol support. A qualified fresh
cutover is required.

## Deployment policy

On September 30, 2026, the operator authorized public Taira cutover and removed
the fixed 24-hour fault-test requirement. On-chain governance owns deployment
policy. Fixed fault-test durations and missing or failing soak verdicts do not
authorize or block a cutover and are not Sumeragi protocol or node-admission
rules. Authenticated native control authority, genesis and committee validation,
safety-record custody, and live readiness/write/restart checks remain in force.
No 24-hour fault run was started for this recovery.

Current policy and procedure are owned by the
[Sumeragi specification](../../specs/sumeragi.md) and
[Taira reset runbook](../../specs/runbooks/sumeragi_taira_reset.md). This incident
records the observed September 30 state; it is not a claim about current live
readiness.
