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

Current-source doctor now requires exact plain-text `Ready` from `/readyz` in
both scopes and bounds failed response bodies and machine error codes. All 26
focused doctor tests pass, including the two new readiness regressions. This diagnostic correction is not a live repair.
The current observer uses `SumeragiFinalityProof` and its native CommitCertificate;
d431 emits the retired V2 finality artifact. Its beacon bundle also differs from
the current native phase-proof contract. A current coordinator cannot authenticate
the old install without retired protocol support. No compatibility path was added.
A qualified fresh cutover is required. A separate clean candidate worktree at
signed source `0993e1c83fe3a06e0f0b0fbf503fe79caeba3b7f` is being prepared;
this source identity is a base, not a completed or approved release. Offline
first-boot initialization, one-shot assertion custody, and reset storage-namespace
corrections are implemented; daemon compilation and 118 retry-wrapper tests pass.
Native Rust execution and the required 24-hour Linux fault soaks at committees 4
and 22 remain pending. An isolated local Linux qualification VM is provisioned;
production validators remain on the approved MacStadium host. The live
ledger remains preserved until a concrete replacement decision is authorized.

## Superseded status paragraph, retained verbatim

Public Taira remains bound to source42 (`d085418a382e831875731144436c174d8621cdd6`); the last funding-policy probe returned HTTP 502. Verified off-host archives precede inactive source85–93 and selected binary retirement; current runtime, ledger, configuration and history remain preserved. The user explicitly authorized clearing a 24.38 GB IPFS stderr log on the same Mac. Exact held-inode truncation increased measured Mac free space from 2.66 GB to 27.05 GB, preserving the active append writer and permissions without restart. A dedicated logrotate job now checks the exact log every minute at a 64 MiB threshold with seven gzip archives. Nine installed-utility controls and two natural launchd runs passed; the same Kubo process and log inode remain active, with 26.90 GB free at the scheduled observation. Deployment remains gated on native qualification, concrete fresh inputs and an explicit shared-ledger replacement decision. See the [incident record](docs/history/2026-09-20/dpn-live-recovery.md).
