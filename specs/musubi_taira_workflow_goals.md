# Musubi to Taira first-release workflow goals

Status: blocked. Started 2026-09-13; blocked 2026-09-13. Owner: Musubi / native contract deployment / Taira onboarding.

The user requested all findings in `dist/kotodama-musubi-demo/ux-review.md` be fixed at their roots, with no backward-compatibility aliases, wrappers or duplicate implementation, and a real end-to-end demo. This ledger records acceptance criteria, not release qualification.

| Goal | Outcome | Completion evidence | Status |
| --- | --- | --- | --- |
| G1 | Runnable contract scaffold and contract-only packages | Default contract template checks/tests/builds; no dummy library; exact library dependency semantics; public view boundary tested; source example matches scaffold | Complete locally |
| G2 | Useful output, explicit profiles, stable workspace locks | Named tests and full failures in human/JSON; artifact/interface/code identity and resolved profile; offline read-only source listing; publication evidence separate from workspace lock; no inert release flags | Complete locally |
| G3 | One package-aware deployment/view workflow | Shared native deployment owner; redundant standalone helper retired and consumers migrated; exact target/artifact/fee/permission preflight; immutable signed plan and status recovery; Applied receipt with height/scope and exact code/address readback | Implemented; live verification pending |
| G4 | Executable Taira onboarding and discoverable reads | Current network identity, faucet and fee instructions; actionable registrar authorization; no weakened governance permissions; read-only view MCP capability; English and maintained locales verified | Implemented; public rollout unverified |
| G5 | Fresh end-to-end Taira recording | Supported account path, real deployment Applied evidence and deployed quote(3)=30; reproducible source, visible edits/scaffold, readable short video; runtime keys excluded | Local video complete; on-chain recording pending |

## Design constraints

- Musubi.toml is the source/build authority. A public named target selects exact runtime client configuration and contract alias; signing custody remains outside the repository.
- A registry publication is independent of contract deployment. Consumer locks are not overwritten with publication-only graphs.
- Native orchestration lives in a focused service boundary shared by consumers, not copied into frontends or added to the network SDK as a filesystem/journal runtime.
- Account funding does not imply registrar or alias permission. Preflight must report the actual missing capability and supported way to obtain it. Never relax authorization or reset shared Taira to make a demonstration pass.
- Submission is not completion. Retain exact hashes/signed envelopes through ambiguity; final success requires state-resolved Applied and contract/code readback.
- Read-only contract views are not new finalized transactions or credits to balances.
- Local checks and previously captured footage do not qualify live deployment or the full workspace suite.

## Current qualification

- Core authorization and exact committed block replay pass ten focused tests, including both Initial and bundled executors and both parallel apply modes. The full default-executor suite passes 160 tests. The generated four-peer Taira genesis/config seed test passes.
- The native deployment service passes 32 tests, including durable progress ordering, exact recovery, historical-authority inspection, cancellation, HTTP pagination and incomplete-route rejection. SDK exact finality classification passes five. Root CLI presentation/network checks cover artifact substitution, fee bounds, canonical target names, network-file size, progress redaction and one-document JSON output.
- The new read-only MCP view tool passes three focused tests. Authored onboarding, Musubi and contract guides pass the maintained 20-locale and content-policy checks. All nine exact local Musubi tutorial commands pass with the rebuilt binary and authored source blocks.
- The final integrated Musubi suite passes 460 tests with one existing ignore. The full Kotodama library suite passes 1,093 tests, and all 52 focused IVM invocation/return tests pass. Public calls use the canonical argument record and current-caller host dispatch; the retired JSON override is removed. Ordinary Taira onboarding passes 16 tests, the native faucet/reset regression filter passes 17, and the retained native contract CLI suite passes 33. Default-directory `init` and exact onboarding fee review regressions pass.
- The fresh 60-second local video at `dist/kotodama-musubi-demo/local-workflow/coffee-rewards-local.mp4` uses the final rebuilt binary and actual `new`, `cd`, `check`, `test` and `build` output. All four public quote cases pass. Source, lock and compiled artifacts match byte-for-byte across two workspace roots. The neutral capture excludes home paths from visible output; H.264 playback, all eight delivery-file hashes and source-ZIP integrity pass. Captions, transcript and exact provenance accompany the video. Edited pacing is explicit.
- Read-only Taira inspection during the review returned HTTP 200 (`Healthy`) for MCP health. The default client file and documented owner-only runtime client directories are absent on this host; the user has been asked for a client configuration path. No live deployment or updated on-chain video is claimed.
- The next goal turn revalidated those missing client paths and completed a fresh public audit at 03:31–03:33 UTC on 2026-09-13: health, stateless MCP discovery and tools-list returned HTTP 502. The rebuilt native CLI independently confirmed nine public route failures at approximately 03:38:44 UTC. This establishes public ingress/upstream degradation, not whole-network failure, signer error or missing tools. Raw bounded evidence and exact live prerequisites are in `dist/kotodama-musubi-demo/taira-live-audit/`. The previous goal turn is classified as progress (implemented fixes, tests and recorded artifacts); this continuation also made progress by building the native CLI, gathering fresh public evidence and correcting the diagnostic producer/verifier to use `mcp_discovery` for stateless discovery, with all 16 focused doctor tests passing and no compatibility alias. The missing runtime client input remains the same blocker, now observed across two consecutive goal turns.
- The full source-budget gate remains failing with 244 findings across the candidate and recorded baseline, including pre-existing overages and reduced files requiring baseline ratchets. New local-workflow tests are extracted below the 3,000-line limit; no budget exceptions are expanded. Scoped formatting, workspace target inventory, codec and historical-archive checks pass; full-workspace runtime qualification is not claimed.

## Blocked audit and resumption

The third consecutive goal turn confirmed at 03:42:20 UTC on 2026-09-13 that the default and documented runtime client paths remain absent, and no client configuration path has been supplied. A fresh native doctor run again returned HTTP 502 for all nine public routes. Evidence is retained in `dist/kotodama-musubi-demo/taira-live-audit/blocked-audit-inputs.json`, `doctor-recheck.json` and `doctor-recheck-stderr.txt`. Independent review found no remaining local action that can establish the required live deployment without this runtime input. The goal is blocked, not complete; the local video does not substitute for the on-chain acceptance criteria.

Resume requires the owner-only runtime client configuration file path and usable Taira ingress. Keep signing inputs outside repository files and messages. Verify exact network identity, account funding, registrar and alias permissions before preparing deployment; obtain any required live-write authorization against the concrete plan. Completion still requires exact Applied receipt/readback, deployed `quote(3) = 30`, and the on-chain recording. No live write or public reset was performed or inferred from this audit.

`Musubi.networks.toml` pins each named target's exact `network-id` and
`chain-discriminant`, with a public reference to the runtime `config` file and an explicit `fee`
payer selection. Native SDK fee quotes supply the exact signed charge limits. Contract aliases are keyed by `namespace/package::target` to keep workspace
members independent. The bindings and their lock never enter source packages.
Registry publication writes `target/package/Musubi.publish.lock`; local builds
retain `Musubi.lock`. Deployment journals retain exact signed envelopes, terminal
failure or Applied evidence, and code readback under `target/deploy/`. Explicit
cancellation is limited to fully unattempted local plans; attempted ambiguity
always retains exact-hash recovery. Read-only inspection authenticates historical
signed plans independently of the current reader authority, on the same exact
network; executing or resuming still requires the original signing authority.
Human deployment progress shows the authenticated plan before dispatch, exact
stage hashes during submit or recovery, durable Applied evidence and readback.
Progress uses the same redaction boundary as final output; JSON remains one final
document. Progress cannot change replay or completion decisions.
