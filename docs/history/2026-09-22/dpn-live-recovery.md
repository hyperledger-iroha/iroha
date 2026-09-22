# DPN developer feedback and recovery — 2026-09-22

## Completed feedback change

The existing `taira_release.py check --focus-regression` diagnostic partitions
its exact requested selections. MV/Concread metadata, code generation, copied
artifact custody, test census and selected runtime checks finish first. A failure
stops before the larger graph. The same coordinated warm target then checks
mandatory configuration and all remaining selected harnesses; configuration is
required even for portable-only requests. Both phases must pass before overall
success. Pending-Kura ordering within the later phase is unchanged.

Immutable preparation, complete release feature graphs, artifact identity,
independent checkpoints and production qualification remain unchanged. There is
no new CLI mode, feature override, target directory or compatibility branch.
Each phase reports its own Cargo graph. Earlier feedback does not establish a
shorter total build when feature unification changes dependencies.

## Current-source validation

Base HEAD: `b621e30ad6dcf7179c29210d2746dd4e97b34ac6`, branch `optimizations`.
Only `/Users/takemiyamakoto/dev/iroha` was used. The maintained LLVM18 Linux
runner reused `target/cargo-fast/dpn-devex-linux`, with six build jobs.

The five gate/test/documentation files subsequently entered signed commit
`b8a8875e2fb11285f05cedf34bc80687815f14b2` through concurrent repository work.
After the merge to `1b8e5f92b6dadb1dcdd16f945286560e31b347ff`, all 465
scoped source hashes still matched the completed diagnostic exactly.

- All 343 selected portable storage checks passed in 26.6 seconds, including
  current deletion/replacement payload panic and charge-lifetime controls.
  Portable metadata took 8.5 seconds and code generation 14.8 seconds.
- The canonical faucet-advertisement SDK regression and all four mandatory
  configuration checks passed. Total diagnostic time was 446.234 seconds.
  The second graph took 131.2 seconds for metadata and 281.0 for code generation.
- Seven copied native executables were independently checked and released.
  All portable copies closed before the second Cargo graph began.
- The Python runner passed 225 tests on macOS; Linux passed 223 tests with two
  platform-specific skips. Added cases cover execution exactly once, first-phase
  failures, completed artifact custody, mandatory final configuration and HEAD
  drift. The paired run retained 177 identical source files across both hosts.
- HEAD and index stayed unchanged. All 465 recorded gate, storage and direct
  SDK/config inputs stayed unchanged. Other tasks changed FASTPQ/Halo2 sources
  and readiness/status records during execution; the full before/after receipt
  preserves those paths. No whole-checkout source-stability claim is made.

Local diagnostic receipts (ignored generated evidence):

| Receipt | SHA-256 |
| --- | --- |
| `target/dpn-devex/native-portable-feedback-validation-complete-20260922.json` | `23baff76374f6612248200e361fddccbe875e5f41ff9f32745d84d479adeda21` |
| `target/dpn-devex/portable-feedback-paired-python-20260922/complete.json` | `ef5f6fe958d1daebe23bcfa4838a1656c11c33e9f997e85d8ac636ece27a8bee` |

These results validate the focused diagnostic change. They do not qualify a
shipping binary, current Core execution, a four-validator release or deployment.

## Remaining live recovery

Fresh public `/status`, `/v1/accounts/faucet/policy` and `/v1/mcp` probes returned
HTTP 502 at 2026-09-22 05:49:55 UTC. No deployment or shared-ledger replacement
occurred in this change. The MacStadium host, guest, retained ledger and previous
failed candidate evidence remain preserved. The earlier 24.38 GB log clearing
and installed rotation are recorded in the September 20 incident record.

Current-source integration still lacks a production `CarrierValidator`, original
Validate-to-Apply prepared-owner transfer, concrete complete World allocation
admission, and production native lane decisions. Existing admitted storage
insert/remove/replacement/capture/restoration controls are already present and
passing. The next concrete boundary is scoped fallible acquisition for the real
World storage inventory; passing standalone storage controls is not that cutover.

## Exact prior current-status paragraph

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Explicit prepaid Storage insertion transactions now retain both original checkpoints and admit current/first-preimage edits together with exact sorted touch-buffer growth and policy-owned key copies from one original pool. Abort restores both parent roots; caught edit or cleanup panic prevents partial publication. The unchanged-source Linux diagnostic passes all 236 selected native controls and four mandatory configuration controls. Registered MV test names now receive a source check before Cargo compilation. The maintained gate passes 220 Python tests on Mac and 218 executed cases with two macOS-only skips on Linux. These component APIs do not activate prepaid World execution. Candidate30301 remains the failed four-validator release; complete model/control/aggregate admission, original Validate-to-Apply custody and the native producer cutover remain required before immutable release qualification and DPN deployment. See the [dated implementation and validation record](docs/history/2026-09-21/dpn-live-recovery.md).
