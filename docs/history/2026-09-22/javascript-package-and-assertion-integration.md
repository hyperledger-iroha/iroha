# JavaScript package and assertion integration — 2026-09-22

All work used `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`.
The observed branch tip was `bdadfae6175a89e5fdb56292a5a64b93b44a231f`;
the changes and observations below are uncommitted component work, not a frozen
release candidate. All fourteen overall release goals remain open. No backward
compatibility implementation or HSM requirement was introduced.

## Implementation

The [JavaScript content contract](../../../specs/sorafs/javascript_original_archives_v1.md)
now joins an original package to complete captured source, the original lock,
the pinned package/build recipes and a separately supplied checksum manifest.
It enforces exact source-to-dist bytes, literal and implicit package members,
0644 publication modes, all 23 required build outputs and six consumer
entrypoints. Reviewed module paths reject npm-ignored names and unowned
selection files. Source count, file/aggregate bytes, checksum and shared path
inventories are bounded. The archive and projection use one namespace owner;
refused admission permanently invalidates it. Character admission precedes
UTF-8 allocation and prefix construction.

The six SoraFS source test entrypoints now select one shared assertion owner
each. Their complete statement tails preserve all **172 assertion expressions**,
**46 top-level cases**, **nine nested cases**, and seven native requirement
helper functions. Explicit contexts supply the concrete selected subjects and
fixture/temp roots. The pure native requirement factory replaces the old
embedded implementation; the eager source helper binds that factory after the
same actual load attempt. No public SDK export was added. Orderbook mocks and
orchestrator local-provider fixtures retain their original limited scope.

The native cache observer identifies an addon already loaded by an ordinary
installed SDK operation. It refuses preexisting, missing, multiple or
substituted native records and changed module/export descriptors. It never
creates a load or a second snapshot. SameValue comparison distinguishes signed
zero and handles unchanged NaN; a caught reentrant refusal permanently poisons
the outer observation. Inventories cap cache entries at 4,096, export names at
1,024, cache-key length at 4,096 code units and export-name length at 256.
This is mutable process-state consistency, not file custody, native execution
attestation or mapped-memory proof. The actual snapshot-file owner is pending.

Release workflow watches and test inventories cover the new owners. Existing
workflow controls now read the canonical shared assertion/failure bodies;
source entrypoints, case names, skip refusals and original assertions remain.

## Validation

Two final root runs cover **1,756 distinct component test identities**:

- **1,596 Python tests passed**, zero failures/errors/skips, in 77.77 seconds
  (78.09 seconds including the driver's original-package replay). The driver
  observed 634 scoped input files with no byte drift. Packet:
  `target/first-release-javascript-package-validation-20260922/`.
  Its `source-observation.json` SHA-256 is
  `97f41577e3a441de054f474b5e6ee86319f3bf244ee1caeff8f4a54b907c1654`.
- **160 Node tests passed**, zero failures/cancellations/skips/todo, using host
  Node 26.9.0. These comprise 140 inert cache-record controls and 20 assertion
  structure/registration/profile controls. The driver observed 820 scoped
  input files, including TypeScript and fixture inputs, with no byte drift.
  Packet: `target/first-release-javascript-node-validation-20260922/`.
  Its `source-observation.json` SHA-256 is
  `f25772a8778b3d3312efb11175645c0a92fe10fd615ea89ea0113a96a08806c7`.

The 380 focused content controls, 49 workflow controls and earlier 20 Node
controls are subsets of these final runs, not additional passes. Node controls
register the preserved native suites but do not execute their SDK/native
assertion callbacks. No addon was loaded. Host Node 26.9.0 does not qualify
the release's Node/platform matrix.

The existing inert npm pack replays all **199 members** against the corrected
owner: 953,019 compressed bytes and 4,947,456 inflated tar bytes, SHA-256
`f5d57c908118629b7b342a8d5580e41a8457a37474a336221ad70d23e8cabdc8`.
Every member's digest/mode is retained in `original-package-replay.json`.
The pack's dirty native checksum metadata remains unqualified. The original
0600 checksum-mode pack rejects; only the public staged copy is 0644, without
changing original custody permissions.

Agent review packets preserve preimages, AST comparisons, controls and failure
history under `target/first-release-javascript-{package-tests,shared-suites,native-cache-tests}-20260922/`.
Initial package controls exposed missing mandatory outputs and npm-ignored
paths; canonical corrections preserve every current source file. The shared
suite packet retains an erroneous broad partial-mirror test selection, then
the corrected exact-selector run. The final root run executes all 49 workflow
controls against the complete checkout. Native cache equality/reentrancy
mutation controls are deliberately altered component tests, not historical
native execution. These agent reviews are not independent release audits.

## Remaining work

F12/SF11 still require the fixed installed JavaScript producer and original-index
adapter, authenticated complete source census, pinned offline Node/npm inputs,
complete installed/transitive-module observations, actual retained loader
snapshot and ABI-23 execution, all original assertion results, signed aggregate
authority and matching-candidate platform runs. The Node >=18 advertisement
still conflicts with the nested dependency's >=20.19.0 requirement. A separate
runtime-floor proposal must update recipe custody and regenerate package
observations before integration; this checkpoint does not qualify that change.

The source-budget guard still reports 276 existing findings, none in the new
JavaScript owners/tests; no limit was raised. Other privacy, SoraFS, runtime,
formal, distributed, hardware, independent audit and promotion requirements
remain open in their component ledgers.

Release-automation validation, shell syntax and diff whitespace pass. The
historical archive verifies 64,736 records and 67,311 occurrences. Logs are
`target/first-release-javascript-package-{automation,history}-20260922.log`;
the final source-budget report is
`target/first-release-source-budget-after-javascript-package-final-20260922.json`.
