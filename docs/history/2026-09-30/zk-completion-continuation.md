# ZK completion continuation — September 30

Reviewed October 1, 2026. ZK01 and ZK02 retain implementation status; ZK03 through
ZK08 remain unfinished. The [goal tracker](../../../specs/zk_first_release_goals.md)
owns acceptance. This evidence record does not grant release or signing approval.

## Candidate and evidence custody

All work uses `/Users/takemiyamakoto/devstuff/iroha`, branch `optimizations`, at
HEAD `3618455551ace739bc102694b6bb14306d5126e1` plus reviewed working-tree edits.
The last completed native cohort is epoch9, source-v2 SHA-256
`e9a9f04d54e17a50da92b9fcc792823bfee90ec936d395f86c2c40a866538232`.
The reviewed successor changes source, fixtures and packaging inventories and
requires fresh normal builds, exact compiler admission and native execution.
Evidence paths below are relative to `dist/zk-remediation/2026-09-30/`.
No previous pass qualifies a subsequently changed candidate.

Earlier failures remain retained. This includes the epoch8 SwiftPM marker
incident and incomplete IVM runner. Epoch9 proof and quick-IVM receipts instead
record unchanged source and compiled inputs; later success does not rewrite
those earlier failed receipts.

## Last completed epoch9 native results

| Evidence | Actual result | Remaining boundary |
| --- | --- | --- |
| `epoch9-core-complete-union1/result.json` | 1,146 executions, 1,088 distinct, all pass, no skips | Changed-source regression and workspace/network qualification |
| `ivm-epoch9-native-quick1/result.json` | 114 controls pass | Full relation/finalized authority unavailable; memory/heavy selection awaits fixture repair |
| `epoch9-fastpq-ordinary-and-retained1/result.json` | 1,298 pass, ten fixture failures, no skips; all four authentic KATs pass | Corrected fixture reruns; four current maximum producer/replay pairs and CPU/Metal parity |
| `epoch9-core-privacy-fastpq-build4/privacy-native/focused-summary.json` | 147 executions, 146 distinct, all pass, no skips | Successor transform/sample-commit parity and full proof |
| `epoch9-workspace-check1/result.json` | Normal all-targets check fails; 34 rendered diagnostic spans | Corrected APIs/imports and normal readmission of 53 separately recovered historical compiler records |
| `epoch9-four-validator-sumeragi1/result.json` | Four real daemons fail startup on MCP descriptor byte limit | Corrected descriptor and successful Sumeragi/lanes/application runs |

The 53 warm records now have exact historical fresh=false compiler rows and
matching historical, pre-check, current and retained bytes under
`workspace-warm-artifact-reconciliation1/`. This repairs missing cache metadata
for the next normal check; both failed workspace outcomes remain unchanged.

The separate X509 source-contract selection has two passes and two stale-boundary
failures (`epoch9-x509-source-contract1`, `3`, `4`, `5`). The failed guessed selector
in `2` was refused before native execution. Reviewed repairs retain ordering and
closed-path assertions rather than deleting them.

## Complete X509 proof

`epoch9-core-privacy-fastpq-build4/privacy-native/maximum-proof-assessment.json`
records a real 9,420,938-byte proof against the 9,437,184-byte limit. Producer
self-check, public verification, wrong-genesis and tampered-proof controls pass.
Peak RSS is 9,983,410,176 bytes, below 12 GiB. Proving takes 2,405.926166 seconds
against 300 seconds, so the maximum test fails and activation remains unavailable.
The public proof SHA-256 is
`8a58f948a3e57d29375fac0706012c891ae1f096f533042abb8039099276ab90`;
normal optimized binary SHA-256 is
`163a5873ccfeecb43b855b3b135a42f10fe4966802825a2a399b3017bfd6dfeb`.
`x509-epoch9-independent-replay1/result.json` records one separate native verifier
pass over retained bytes without regeneration in 15.36281358 seconds.

Nested phase observations include composition 674.464309 seconds, query openings
584.534058 and DEEP/FRI 354.586716. Sample-source construction totals 163.654266
seconds over 5,811 columns. Concurrent load and nested timers prevent adding
these observations or asserting a causal speedup. Reviewed bounded private FFT
and initial source-reuse changes require actual native parity and a complete
proof within unchanged coverage, byte, RSS and time limits.

## SDK and workflow evidence

Epoch9 authentic Rust generation and all 57 official Kotodama mappings pass;
`epoch9-kotodama-goldens-check2/result.json` also checks four manifests. Compiler
closure changes in the successor require new genuine producer admission and
JavaScript provenance, not simply updated source hashes.

Under `sdk-qualification/`, epoch9 Kotlin has 1,556 passes, Android host consumers
291 and the two native host suites 62 against ABI-25. Python's installed native
consumer runs pass five genuine full-tree wallet controls; its broader suites
pass 4,370 tests plus 281 subtests with no skips. C# records 6,005 passes and
three typed multisig fixture failures, with a reviewed repair awaiting adopted
consumer reruns. Six additional held Kotlin full-tree controls pass natively;
source-bound readmission is still needed after adoption.

`epoch9-apple-native1/native-build-result.json` records normal compilation of all
five Apple static slices but failed packaging: the maintained KAGEMUSHA inventory
omits the existing coordinator install export. The reviewed correction adds it
to three exact inventories. Independent C-header and missing/extra-symbol tests,
and exact shell checks over all five retained archives, support this diagnosis;
they do not constitute new package/cap or Swift host qualification. Caps, export
set equality and ABI-25 requirements remain unchanged. The current device
inventory is empty; physical-device runs and signed release artifacts remain open.

## Qualification and next candidate

Independent soundness and ideal-QROM hiding derivations are conditional on their
explicit field, oracle, query, attempt and entropy assumptions. They do not prove
concrete Keccak security, physical entropy or side-channel behavior. Changed
source requires delta review before applying any artifact-bound conclusion.

TODO: validate the reviewed successor cohort through normal builds and focused
regressions, genuine fixture production, current maximum proofs, maintained SDK
packaging, workspace checks and real four-validator workflows. Preserve exact
`3f + 1` committees, `n - f` votes and signed RS16 custody. Secure RAM-LFE encryption
and its full program relation, and the complete native IVM transition relation
with finalized State authority, remain unavailable. Planners, diagnostic traces,
leaf proofs and component tests do not complete these capabilities. All six
unfinished goals remain open.
