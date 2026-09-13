# Kotodama V1 validation — 2026-09-10

C9 remains open. This record distinguishes retained executions from the current
repair. No commit or clean-Git release certification is claimed.

## Recovered terminal builds

Apple attempt three completed on isolated source
`961db6d836ea3c53e911b84c0b09ae6fed9d7a731ebc962d7f903ba11bcbeb9d`,
HEAD `ff22bda4c42219ccc055a69d7b5ce86e25b299e8`. Its five normal release target
boundaries and C-consumer links pass. Only the native host executes the ABI,
SHA3/SHAKE, ML-DSA and ML-KEM checks; other targets provide cross-link evidence.
The independent audit passes 35 controls over outputs, identities and preserved
inputs. Apple qualification SHA256 is
`8e8fa7b876930feb79e3ac58dcf95ed0229cb30cb0e6f1d1164207437ec54cc2`.
The source loader still contains its previous three fallback pins; the external
projection does not publish those edits.

The separate ordinary-dev message-control daemon build also succeeds on 961,
with all 12 capture controls. Its frozen executable is 1,182,064,880 bytes,
SHA256 `7d0cb38630933a02c01f60bc4a8af48aefd7927b35fe092d7c71278154ae6e9f`.
Run SHA256 is `be8ffb3f268a0fe832ebd368ebb6852983a7d78c4c80e8bc36286a063fc1344e`.
The first independent recovery audit incorrectly expected identically named
features in child crates; its 42/44 result is retained. The 11-check corrective
addendum follows the actual Cargo feature mappings and passes, SHA256
`88db82031ecda34774ddc20ba9369f58202e4f352b444810ada100874480bdcc`.

The main checkout's earlier ignored target directory is absent on resumption.
Its missing receipts are not re-created or silently treated as available.
Retained PRIVATE/external records and current source/index comparisons provide
the evidence reported here. Both source snapshots matched the previous checkpoint.

## Actual four-validator executions

Both selectors ran against the exact retained normal release daemon and test
harness on 961, using four validators and mandatory signed RS16 availability.
There were no competing builds, stack overrides, owner deadlines or signals.
The test harness owned shutdown and the single cold restart.

| Scenario | Result | Run SHA256 |
| --- | --- | --- |
| Genesis-registered companion | 0 passed, 1 failed; native 101; 234.26 seconds | `69c37ad4ca9b4124e7d8b9aebeb3ab9846d895f37590c9a4bb1794c8e856ce56` |
| Ordinary upload/register/deploy | 0 passed, 1 failed; native 101; 345.12 seconds | `7140a5398378e02a71967db4a8a2323cc6e3bb728927019b45e99bef074976b3` |

Both passed `hajimari` and `verify` Applied waits, all four validators' view/state
readbacks, and the later deployment-bound RBC checks. Each reached the second
incarnation of the selected validator, whose Kura initialization rejected
`latest_certified_frontier.norito` with “certified/bundle capacity identity
conflicts with the durable certified frontier”. Restart readback was not reached.
This later failure is distinct from the earlier verify-wait/SigningGuard failures.

All eight non-success controls per run pass; the combined independent terminal
audit passes 38 controls, SHA256
`6cbae7ec28e7045735eee65d23cbf917361673da500e7d2e03fa2f167bdc2f04`.
Both native handles closed, and no owned validator remained.

## Applied recovery repair

Startup already authenticates persisted historical bundle bytes against their
certificate, execution input and durable source. It then incorrectly requested
new publication capacity, which rejects an older certificate against the newer
frontier. The repaired branch consumes only an existing exact bundle-component
obligation. This also preserves append-intent recovery: deleting the accounting
step would leave that obligation outstanding. Missing-bundle reconstruction and
normal incoming-publication admission keep their existing checks.

The production change and two primary-lane cold-restart regressions are applied
in both trees. All 22 previous capacity tests remain unchanged. The positive
checks both historical sources, unchanged latest frontier bytes/value, zero
reservation and a subsequent height-three publication. The negative corrupts
the older bundle and requires the exact InvalidData reason/path with unchanged
storage. The first 24-case normal-profile/default-stack run returns native 101: 19 pass
and five fail in 171.13 seconds. Its run SHA256 is
`14fa59e02a6caabe184be0ab2da80d516ab501d27a173512f7a756bae17aa045`.
Three failures in unchanged tests reproduce on the exact frozen 961 executable:
the two active-slot-reset READY fixtures and the reconstructed CertifiedPair
reservation-map comparison. The baseline returns native 101, 0/3 passing, in
9.93 seconds; receipt SHA256 is
`ea7ac5b856d6b9b804a3f762feb5265e000a75e4fb3e47dd4fa4df776efe7ba2`.
These remain failures, distinct from the new-test fixture errors.

The new positive fixture failed startup's required signed lifecycle-cursor
check. It now uses the existing signed reservation-identity, process-generation
and Live-cursor/CAS helpers before certification. The negative received the
intended corruption error but compared a noncanonical temporary path; it now
uses Kura's canonical root for the same exact kind/path/reason assertion. All
22 existing test bodies remain unchanged. The fixture patch SHA256 is
`d51316316535fa6ec4b9e4c99c9748e0b83447e74f870ea49b4c7995da95ae65`;
application SHA256 `1b4708a0f5ed3cc0615db2ee3f8c4ab2d2026b76ba1c4de9cbafb1033dcce59d`.
Both trees preserve all eight guards. PRIVATE becomes
`d12accbd7085fce44f7c390ce1677cc5eebeb3de3fe936315ab201462b7ffdfb`;
MAIN becomes `4f5e63c23f698b8aae34383d0b2c0bbb47d910540f01935638dd2a3b5197651e`
before these ledger edits. The complete Core24 rerun on d12 returns native 101:
21 pass, the same three pre-change failures remain, zero ignored, in 187.95
seconds. Both new tests pass, including normal cold startup, future publication,
and exact historical-corruption rejection without storage mutation. Run SHA256
is `62a470920e64086f595e7bce9e01525cc474dfac89b3a1d9e2ddb995929dce73`;
the independent outcome/source audit passes 22 controls, SHA256
`55969be25b3f75fc9c8b7aeff7678bed5c78225514d833794d8942128852e519`.
The whole 24-case suite remains failing; no failure is skipped or waived. The
normal daemon rebuild now qualifies; the harness and complete network reruns follow.
The fresh capacity static contract and workspace format check both
return zero, with ten controls passing; receipt SHA256 is
`35d484cfdf248ece2d56ffb4c7858572608f65c8725d04aeacd146d948348792`.

Patch SHA256 is `1e4b7a837d9a75f568f3e9f1e1e21cb22efcb2cb4632195d44191d9588bc8953`.
Application SHA256 is `3db5cf20f460f7b9a1ea3c4fa19995e16f68d6d9ab304f7fdcf6fdaa0200f638`;
all eight controls per tree pass. PRIVATE becomes
`9cd0858148835e586a365775ea71f4becc282ba6d539fe036c157324c62012e5`.
MAIN becomes `4896b257fece3a1f0b377660e6c0f4c160fb022cae653d11e1f3004228377bbd`
before these ledger edits. Both actual indexes, HEADs and Cargo.lock remain unchanged.
The recovery-capacity static contract passes before the change (receipt
`b34d45aa145f050b0bc3f4a29f23673441d12dc654aff367bc9e72eeffe3af6d`)
and afterward. Workspace formatting also passes afterward; combined receipt
`cfd025eb3225dd99dec7b82f3cc473e7f0e11903adc3acf88079d5b2747d3dd6`.
These static checks do not replace Core or network execution.

## Repaired daemon qualification

The ordinary locked/offline release build on d12 completes with native zero at
10:31:58 UTC after 4,426.399182 seconds. Its terminal capture passes all nine
checks; receipt SHA256 is
`25f1feb1bddeff993f1391bad60606f3350d0a4735135ec7617e72476a0ee3ca`.
The frozen `iroha3d` is 343,282,304 bytes, mode 0700, SHA256
`c81b776f57083420f638609d9d9609e8785ba70fd5dacf94ea197580811f75a0`.
The terminal audit passes all 16 checks, including current source, both actual
indices/locks, ordinary release profile, frozen bytes/log and build-process
closure; audit SHA256 is
`6407ae8bc838d40f3bfa54a3c59c750dad45be6ee3b93d686ec3d111069bf875`.
This is a build result, with no test-count claim. The normal-profile integration
harness build starts at 10:33:03 UTC using the existing target cache. Both full
four-validator scenarios remain pending; the qualified daemon alone does not
close restart/runtime acceptance.

The retired-codec guard also returns zero on exact d12, with all ten source,
index, lock and tool-preservation checks passing under IP/source-write denial.
Receipt SHA256 is
`773443aed2fca7335fc26504e2b0acda86c3dcff5926a5e38d99f47899812f78`.
An initial runner preflight failed on an absent `rg` path before source capture
or guard execution; the successful run uses the observed bundled executable.
No dependency, codec or source change was needed for that runner correction.

## Remaining consumers and tools

The packaged VSIX matches the current client/configuration/grammar source and
contains the LanguageClient runtime; archive SHA256 is
`47018c3051214f7669b605b330790701ae1a850059180d9d2fbd816d995f54b6`.
Generated settings and `koto lsp --project` wiring agree. The real packaged-client
smoke passes all 29 assertions and 14 preservation controls in official VS Code
1.137.0, isolated externally with IP access and writes outside the disposable
run denied. Activation, semantic snippets/signatures, hover, definitions,
references, parameter-label rename, Japanese/emoji UTF-16 diagnostics/recovery,
and formatting equal to frozen Koto pass. The protocol independently confirms
rename document version 4 and an exact empty second-format response.
Report SHA256 is `cccb6dac5f6ea75c1dd2675790880b4b688e956cbf689b38dcc4ff929109b58e`;
independent readback SHA256 is
`986904d1affe2f4ba88e84eb06903fff0bb4918bd446531963f0f17693221cf0`.
PRIVATE source 9cd, the five relevant MAIN editor files, both indices/locks,
VSIX/frozen compiler bytes and generated project files remain unchanged. Later
Core test-only edits do not alter these editor inputs. All owned host processes
closed naturally. Attempts a–c retain launcher-name, nested-sandbox and VS Code
API-result expectation failures; the last had 28 assertions passing and a valid
empty protocol response. The final run keeps the outer sandbox and disables only
the incompatible inner Chromium sandbox in the disposable test invocation.

The previously prepared MAIN Python runner is unavailable. A replacement uses
retained exact 222+9 case inventories (229 unique cases), fresh installed wheels
and the ordinary Maturin release build. Its offline preparation passes; no new
Python execution is claimed. Cached bytecode is excluded from copied dependencies.
The .NET 8.0.419 prerequisite is recovered into an external tools directory.
Microsoft's archive SHA512 and all four previously pinned binary records match;
`--version`/`--info` succeed under the unchanged global.json. Recovery report
SHA256 is `151b279dc3c6c01ea99bd31af6697df0e9d8a83626e3026a1335eb8c3dece79e`.
The proposed C# plan changes only 13 SDK paths and preserves all 42 selected
cases. No new managed/native consumer execution is claimed.

The production Kura edit requires refreshed affected binaries/native evidence;
the successful 961 Apple products do not qualify that new input. After native
qualification, deliver all three Swift fallback pins and verify the official
normalized closure and unchanged artifacts for that pin-only transition. Retain
all previous failures, remaining maintained-SDK checks and the unattempted full
workspace/build/strict-Clippy gates. Publication certification remains separate.

## State registry restart failure and applied boundary fix

The d12 normal-profile harness build completed with native 0; run SHA256
`ddaec6f1d667c6506800a2b5ea5e847459c3981c1817ac6dfe78240c6117224f`.
The initial pair audit's 15/16 result incorrectly assumed debug level 2; the
8-check profile addendum verifies the canonical debug-0 profile and qualifies
the pair without changing the build. Addendum SHA256 is
`f7808d3646a2e57a1bc97da05c4508f325f7b7c79bd512b34d004b23a0892a4e`.

Both complete four-validator scenarios then ran on the exact d12 pair:

| Scenario | Result | Run SHA256 |
| --- | --- | --- |
| Genesis-registered companion | 0 passed, 1 failed; native 101; 231.75 seconds | `8b8bbff53c08cf67408d3ab43d8d7ff2ba7dab7c6f24553b797f76a95cbd6bde` |
| Ordinary upload/register/deploy | 0 passed, 1 failed; native 101; 347.45 seconds | `857dd9351e71705446109f814df390911c08bc247916b007974d453c89db6857` |

The unchanged sequential scenarios reached restart after deployment/call Applied
checks, state 7 and readbacks on all four validators, and RBC progress. Each
restarted validator failed State hydration before replay with the embedded
reservation-key binding diagnostic. Post-restart readiness/readback was not
reached. The earlier Kura frontier-capacity rejection was absent. The combined
terminal audit passes 51 controls, preserving both failed outcomes, exact
sources/artifacts/indices/locks and natural process closure; SHA256
`9a0e88b2826910fffe74f04e6e1b1d411b498ca234cc9d1679e69696d9bd908b`.

The 18-check forensic review establishes that both snapshot-disabled constructors
start at State height 0 without admission markers, but historical preflight
requires an exact registry owner before earlier admission carriers replay.
This identifies a failing predicate; it does not claim every other embedded
reservation equality was decoded independently from runtime bytes. Report SHA256
is `217350f9fca4793c4f2ec9cffba6e45700d090fe5335a5fea38d0f52d4181c1c`.

The reviewed three-path fix is now applied in both trees: only future historical
carriers defer that WSV lookup. Restored-prefix history retains exact ownership,
ordered live execution requires pending admission, and immutable reservation,
route/session and payload checks remain unconditional. Application SHA256 is
`d7771fabca2a306fdf66a55088dc0fad21f7841305c4ea56d9948fdb906e3ffb`,
with all eight guards passing per tree. PRIVATE becomes
`b814ea7fc369f5ed12e64d2f24a01f99cc318c7c5fe6cad224de76f7d0d63ece`;
MAIN becomes `9d8ec600427010f506b4a52aeebdff15add5393a7210c039faf66aff5d826344`
before this ledger update. The 29-case Core capture has started on b814
(`20260910T111640Z-core-future-carrier-registry-29-4147f67c4c`); its terminal result
is pending. No regression pass is claimed. Native producers, maintained
SDK/Python consumers and workspace gates remain held pending fresh binary and
complete network qualification. All earlier failures retain their recorded scope.

## Core29 result and genesis-domain fixture correction

Core29 on b814 completes with 27 passed, two failed, zero ignored and native 101
in 1,046.87 seconds. Run SHA256 is
`c7d077d5d706987039571aeaa2e30f669dcf5b4d089f09777073f770336a0735`.
The existing `exact_merge_carrier_replay_burns_settlement_once` case fails because
the committed block lacks its required AXT policy snapshot. An exact rerun on
the frozen d12 executable reproduces that same guard: 0 passed, one failed,
zero ignored, native 101; all ten preservation/baseline checks pass. Baseline run
SHA256 is `7fcae6bf019d5c25a572bb06603353d101fa0e8cfb702ccb2e0b150aa6da776e`.
This reproduces the failure without waiving it.

The new `cold_state_merge_hydration_preserves_registry_replay_boundary` case
passes the public cold constructor, then actual ordered replay fails because
the fixture omitted the genesis-domain owner. The reviewed two-test-path
correction seeds that existing authority before the first State and signed
commitments, only for this case, and supplies the matching cold World. Old
callers and all replay, canonical-state and rejection assertions remain intact.
The correction is applied in both trees with eight guards passing per tree;
application SHA256 is
`74021b810216bf0b000b188287f16b3ac25057c0a683f00d51384f86b20b8bb2`.
PRIVATE becomes `b11bac245575553ce6c6cb1a91187ba4c65ed9b4cfd24ec7ca15824246ece4cf`;
MAIN becomes `ef161e288b4c21a89a79c856532dcbdeafbc1fce60762adf339598d78649cfd5`
before this ledger update. Production State code remains unchanged. The full
29-case retry has no qualified result yet; no regression pass is predicted.
Fresh daemon/harness and both complete networks still require qualification.
Native producers, maintained SDK/Python consumers and workspace gates remain
held. All earlier failures and execution source scopes remain recorded.

## Core29 retry and startup runtime configuration

The full Core29 retry on b11 returns 27 passed, two failed, zero ignored and
native 101 in 1,041.51 seconds. Run SHA256 is
`b674da5fa696c2fee84696428dae29eaa31bdfcc513dcf5ce32d98926d192bb7`.
The existing settlement case still fails at its required AXT policy snapshot.
The new cold-start case passes public construction and the genesis-owner lookup,
then block-2 replay fails because lane 0/dataspace 0 requires four validators but
its canonical pool is empty. This is later than the prior fixture failure;
neither full Core29 result is a pass, and the reproduced settlement failure
remains unwaived.

KuraSeed restores canonical State without the fixture’s process-local manifests
or static dataspace configuration. Normal daemon startup reattaches both before
replay. The reviewed correction adds 42 lines only inside the new fixture’s
restore helper, using its actual Nexus configuration and frozen four-validator
manifest registry. It retains snapshot lane topology/configuration/cooldown and
the immutable configured baseline, with canonical-state preservation assertions.
All previous callers, replay and rejection assertions remain; no production
source changes. The correction is applied in both trees with eight guards per
tree passing; application SHA256 is
`5d1cf8c4039aebeffd4a151f9b7c0b6232dbbf40f87fa1441a42bc9ead3a0968`.
PRIVATE becomes `86c6671a8fc4ebd34d1c1b4c43ea55d0189be20c66f963d7c7d06ecc756acd4e`;
MAIN becomes `39fff832d3a0f2e8b1fc5dd316db3b554d10fd582a4a76f5a9d39eaa05254bf6`
before this ledger update. The exact cold-start follow-up is pending. Fresh
four-validator network qualification remains required; native producers,
maintained SDK/Python consumers and workspace gates remain held. Earlier
execution scopes and failures remain recorded.

## Exact cold-start follow-up and restored geometry

The 86c exact cold-start case compiles successfully, then returns 0 passed,
one failed, zero ignored and native 101 in 114.35 seconds. Run SHA256 is
`15f8a52964d233acf6a707f2a1ab89460fbc26e66cd45c3d49671c5191a47349`.
Failure occurs before block replay, during `set_nexus_from_config`: configured
catalog publication has no matching authenticated primary geometry anchor.
This result does not qualify the preceding configuration/manifest correction.

The reviewed correction adds 18 lines only to the same restore helper. It first
calls `prepare_restored_configured_primary_geometry_anchor` with the actual
configured baseline, then `restore_kura_lane_segments_from_nexus`, before the
existing configuration merge, setter and frozen manifests. This matches normal
daemon startup. The canonical-state hash is captured before the entire sequence;
every prior assertion and the full strict recovery audits remain. Both trees
receive the correction, with eight application guards passing per tree;
application SHA256 is
`495d3983e437ba3184c1a59f9fe04237457809cd254e270e75dff81fbf7a6660`.
PRIVATE becomes `e1af491cfc66d1dd8ae4ce9c868eae9a7d55c5b0ba0a85737b2b462f6e36bbe0`;
MAIN becomes `6305cc1d98f7f46f1a4a725aa37ff4ba497ff6775dd1dcfe37632cc37833abc7`
before this ledger update. No production code changes. The same exact case on
e1af is pending; no pass is predicted. Both complete four-validator scenarios
still require qualification. Native producers, maintained SDK/Python consumers
and workspace gates remain held; all earlier outcomes and source scopes remain.

## Cold replay checkpoint and replayable admission

The e1af exact cold-start case compiles in 6 minutes 53 seconds, then returns
0 passed, one failed, zero ignored and native 101 in 126.48 seconds. Run SHA256 is
`e8d59a6518273822327bb080bce8f54e52808aaeafabf9f280ce5b8839c577ca`.
Runtime attachment and canonical-state preservation assertions pass. Actual
block-2 replay reaches the strict WSV checkpoint check and fails: committed hash
`2b9d72d200e3648af43ce4ad707f8119c9a65e2548f426833fc8205ee0a77ee7`
differs from replayed hash
`3b1e31ba70a84fd2a419212e59293bef8270c7f23c50fd001ee005dad49461bb`.

The fixture’s reservation helper inserted canonical QueuePlan registry owners,
pending obligations and their indexes directly into State after the height-1
snapshot. Its signed source carrier contained autonomous payloads but no admission
certificates. Live application therefore started from extra canonical writes that
replay could not reproduce; both paths passed the execution commitment check.
The approved test-only proposal separates enqueue from reservation and carries
real signed admission certificates at height 2, source payloads at 3 and the merge
at 4. It preserves exact queue bindings, all nine existing fixture cases and
strict checkpoints, with snapshots 1/2/3 replayed to 4 and exact negatives at 3/4.
This correction is not yet applied or executed. PRIVATE remains e1af; the MAIN
pre-update source is `e9eefa832d6b2db7076809603b04d266391e8ce917bb582a347b8c1da0a368b6`.
Earlier failures and execution scopes remain recorded, including the reproduced
settlement failure. Both complete four-validator scenarios still require fresh
qualification; native producers, maintained SDK/Python consumers and workspace
gates remain held.

## Real admission fixture and focused validation

The reviewed two-file test correction is applied in both trees, with eight
guards passing per tree; application SHA256 is
`5c4242806eecbe14209e43ff75b9fb2f72b92ef2b6ab1993668bc8ea3dcd5afd`.
PRIVATE becomes `a39f27bdb8ae47b7433fdaeada8df13fafb39906dec631a72d116ba316abeafd`;
MAIN becomes `92f4bbdfb8f40c8d8cd31e21a13e98b1ac57baddb54826a15e3e8774bad3bfce`
before this ledger update. Production State code is unchanged.

The new cold-start fixture now carries real signed QueuePlan admission controls
at height 2 before reserving its unchanged queued claims for source height 3
and merge height 4. Certificates use the exact frozen four-validator authority;
normal signing, CommitQC and RS16 paths remain. The nine existing fixture cases
retain their original helper behavior. The new case preserves strict WSV
checkpoints, complete startup attachment and canonical-state guards, and checks
snapshots 1/2/3 replayed to 4 plus exact live/restored negatives at 3/4.

The directly affected run passes all 11 tests, zero failed or ignored, native 0
in 1,161.63 seconds: the cold-start case, nine existing historical-fixture callers
and the existing plain-successor archive case. All 15 capture/preservation checks
pass and a39f source remains unchanged. Run SHA256 is
`ea9255f3868203f128e0c6b2499a5ce043212489c0b1aba1d07708e9eef4dbf9`.
Earlier Core29, exact cold-start and reproduced baseline failures remain recorded
without waivers; this focused pass does not qualify full Core or the workspace.

After that result, the approved ordinary Python release rebuild and installed
consumer run starts on a39f using a separate Cargo target and fresh external
output. Its 222 + 9 selected cases represent 231 executions and 229 unique tests;
no terminal result is claimed. It may run alongside Apple production and daemon
builds. Fresh daemon/harness, both complete four-validator scenarios and maintained
native/SDK delivery still require qualification. Workspace gates remain held
pending network qualification.

## Fresh Python pass and SDK diagnostic proposal

The ordinary a39f Python release rebuild and fresh installed-consumer run closes
with native 0. Its 222 + 9 cases pass: 231 executions and 229 unique tests, zero
failures or skips. All 12 subprocesses exit 0 and all 15 preservation checks pass;
PRIVATE source, HEAD, staging, raw index, Cargo.lock, tools and dependencies remain
unchanged. Report SHA256 is
`a9540c057f6f545f2e456207f0e84c104f57a765fd8346cf2bae1ed45115ee57`.
The installed extension passes the official ABI-23 probe and has SHA256
`1dc6b5c5759a8ec5642a0c6c2a4ae10f90f59bbc27c4ec702f666d8b22a3eaaf`;
its 2,109 compiler-input records and exact collected case membership are retained.
This is local installed-artifact evidence, not clean-Git publication certification.

A separate six-file Q-only proposal fixes four stale diagnostics across Python,
Swift decode/encode and Kotlin; the accepted policy already permits only `take`
and `page`. Plain `range` gains exact rejection assertions; all old assertions
remain. Python adds one case and naturally renames two diagnostic-based IDs:
223 + 9 cases would be 232 executions/230 unique. These are proposed counts,
not executed results; the existing 231-execution pass is unchanged. Patch SHA256 is
`b01add6372462f41f9d08bf80e7e5a27572e614c6741608181975bb32aa9ed36`.

The six files are absent from the retained Python and completed Apple-host Rust
dependency files. Swift's `ToriiClient.swift` is nevertheless explicitly included
in the official Apple source seal, and its contents are not normalized. After
the current producer closes and the proposal is applied, a fresh ordinary
five-target producer must establish the new fingerprint before Swift pin delivery
and SDK consumers. No source-seal exception or native reuse is qualified by the
Rust input exclusion. Both complete four-validator scenarios and workspace gates
remain open; earlier failures and receipts retain their exact scopes.

## a39 daemon and four-validator results

The normal release daemon and normal no-run integration harness qualify on
unchanged a39f, native 0, with all nine capture guards each. Daemon build time is
2,726.58 seconds; harness build time is 560.01 seconds. Their run SHA256 values are
`71ecc46bc25dd689fb906a70fc94a6877a3fbfbefbd17a43b78ba5cae39f2816` and
`e5e8b7946860296aa2229340963c8b29fb26bbd8f9d23373e5ccbfed3c6e7eef`.

| Exact scenario | Actual terminal result | Run SHA256 |
| --- | --- | --- |
| Genesis-registered companion | 1 passed, 0 failed, 0 ignored; native 0; 233.95 seconds | `f2132eadc6219c069679c235acca6fb0f5f784ac4b6a91c15d54ac759828680d` |
| Original deployment | 1 passed, 0 failed, 0 ignored; native 0; 354.73 seconds | `b3f90fd274268340c9967dafe74337bc6711e4d0df8fc685f68039155223ce30` |

Each run uses exactly four owned validators and one attempt, with no retry or
scenario/assertion changes. All ten capture guards pass per run, including exact
source, raw index, Cargo.lock, frozen daemon/harness and public-call stages.
Both tests reach authoritative restart and exact view/state readbacks. The
original command log also records one unlabelled cleanup peer exit 1. Bounded
review cannot identify its peer or cause: all five retained stderr files are
empty and no fatal, panic or shutdown diagnostic explains the exit. TLS errors
occur at the intentional disconnect and affected peers log subsequent activity.
Forensic report SHA256 is
`271c042e3808154e7bf980fb7e8684af7f2f0002e5989ebb6b4a18dc471d3de7`.
The test PASS stands; this separate teardown status remains unexplained and
unclassified, with no all-peer clean-shutdown claim. This review does not establish an additional source fix. Earlier d12 hydration
failures, earlier network failures and Core/baseline results remain unchanged.

Recorded Apple progress includes three completed Cargo targets; the complete
producer/artifact result and maintained SDK consumers remain pending. The six-file
SDK diagnostic correction is still unapplied. Its Python 223 + 9 selection remains
prospective (232 executions/230 unique); the actual a39f Python pass remains
231 executions/229 unique. C9 still requires that source transition, fresh ordinary
Apple production, Swift pins, SDK consumers, refreshed CLI/message-control
prerequisites and ordinary workspace gates.

## a39 CLI and targeted network repeat

The fresh a39f CLI/Koto build and the complete 10 scaffold/nine debug selections
qualify, zero failed or ignored. Each of the three captures passes all nine
capture guards and its 17-check audit. The frozen CLI/Koto bytes are retained.
The offline generated project passes 12 command checks and all three exact tests;
its IP-network denial control passes, and expected overwrite refusal returns 1
while preserving the project. The official golden check performs two independent
renders and matches all 57 bytecodes and four compiler manifests, with zero
changes. All eight external-workflow checks pass. The completed evidence index is
`a39-cli-source-output-successor-0ad3je7_/EXECUTION.completed.json`, SHA256
`b53e658946c3533ac05ccde0ffa3f520488092d09a76f2e99e4d1551d2354296`.
This qualification belongs to a39f; it does not cover the later applied SDK edits.

One unchanged original four-validator scenario is repeated specifically to
investigate the earlier unexplained cleanup exit. It passes 1 test, 0 failed,
0 ignored, native 0 in 360.37 seconds, with exactly four validators, one attempt
and no retry. All ten capture guards pass. Its run SHA256 is
`d468ea9144e66d396446e1d3477a6f15b04ca5b77bf48cab7a162b58b1a1250d`.
All five logged node exits are 0, all five retained stderr files are empty, and
no exact frozen network processes remain after natural closure. The bounded
context report passes 15 checks, SHA256
`e929042661c4bffdbc6b54cda755e42c34941ccae05a0d79f0c95a8d53a88c6f`.
No source, owner, configuration, deadline or assertion changes were made; the
capture runner sent no signals to the harness. Existing exit rows have no peer
identity. The prior exit 1 did not recur in this single repeat; its peer, cause and classification remain
unresolved. Both earlier PASS receipts and all failed receipts remain unchanged.

The separate a39f message-control daemon build then closes with native 0 and all
12 capture guards passing. It uses the ordinary dev profile with opt level 0,
debug info 0, debug assertions and overflow checks enabled, and the explicit
`test-network-message-control` feature. Its frozen executable is distinct from
the normal-release daemon; no Parliament feature or workspace test execution is
claimed. Run SHA256 is
`64624a7cf769c5d7c866d3dbcc0c605de34d9eddae36b665fe6c112242fc299b`;
frozen executable SHA256 is
`a722a452e4bb881e0ae44fcdd1f0b5826fd2d34d23a112e263ed3bd641556760`.
All source/index/lock guards pass on unchanged a39f. Final-source qualification
must be rebound for the applied SDK source and subsequent generated-pin transition.

The ordinary a39f Apple producer then completes all five release target builds
and C links, host ABI/crypto execution, and official projection/provenance readback,
with native 0. Cross targets provide link evidence only. All seven source guards,
actual raw-index preservation and prior-artifact checks pass. Its qualification
SHA256 is `a3c1978ce297a6cb3c87ac9e103a57bb4cc7ba0e7a797021f02bd2071cf8ac71`.
This is local source-fingerprinted evidence, not clean-Git publication certification.

After that producer closes, the approved six-file SDK diagnostic patch is applied
in both roots with all eight application guards per root passing. The exact
postimages fix the four stale messages and add the intended plain `range` negatives;
Cargo.lock, HEAD, staging, raw index and unrelated source are preserved.
Application report SHA256 is
`89e1c222d373f736351a94394703ce2b6b094e99f203ae343d492a60acb7dba2`.
PRIVATE changes from a39f to
`9a454d1bce7afe6804547233a5ac18d2b6654c592afedf1f06472cecba12bd0a`;
MAIN changes from 66552ed6 to
`0deca08d4d170f1ce33bda5f1c38c95004d2241f967db3c1cedc5d767134c8f6`
before this ledger update. Swift's changed runtime source changes the official
Apple fingerprint; the successful a39f producer is not relabelled for 9a45.
A fresh ordinary five-target producer and its generated Swift pins remain required.

Python's 223 + 9 selection on 9a45 is still pending: 232 executions/230 unique
are planned, while the actual a39f pass remains 231 executions/229 unique.
Maintained SDK/native consumers, final-source binary bindings and ordinary
workspace gates also remain pending. Earlier Core, formal, packaging and network
failures retain their exact scopes.

## Apple pins, Python and C# fixture outcomes

The ordinary Apple attempt five completes on 9a45: all five release target builds
and C links pass, with host ABI/crypto execution and official artifact readback.
Cross targets supply link evidence only. All seven source guards and actual
raw-index preservation pass. Qualification SHA256 is
`6257d014aad413674e3246d502f998a7564479b9ce48327eaca3f6d9f9af5fee`.
The three manifest-derived Swift hashes are applied in both roots with eight
guards per root; application SHA256 is
`ae36ce5d3d8521de1d65e9f42fb0f3965438d613d87748aefb8a644536b09e6d`.
PRIVATE moves from 9a45 to
`1252ee46a6902aeadee427dc2f40ad2a4944e86b23893e83d814c7134a9a162e`.
The official live-pin, provenance and normalized-fingerprint commands all exit 0.
Binding SHA256 is
`b76ac48e478c8d7a2f954a0481350e7cc1c56ef4bcf62c7b91492488c5806d20`;
normalized Apple fingerprint is
`d35bff7692579fce5963223d4600402095c97ce0e8a699faf49b34424fe4239f`.
The producer and consumer retain distinct whole-source identities. No clean-Git
publication certification is claimed.

The first Python launch contains an agent transcription error: its source argument
has 61 characters instead of 64. Admission rejects it before setup, native build
or tests, with zero executed steps. Its terminal receipt remains failed, including
15 unavailable final guards because preparation never populated their inputs;
SHA256 is `60eb4dcae3886e05f0e61c1acd1f069a78c8ec6942df0dadde5a8d099eaf5f9f`.
The corrected launch reads the source directly from the verified pin-application
receipt and uses a fresh external output directory. On unchanged 1252, all 223 + 9
cases pass: 232 executions/230 unique, zero failures or skips, all 12 subprocesses
exit 0 and all 15 preservation guards pass. Qualification SHA256 is
`943ddea0554afcfa9e63757239958eb6f0ee446206dd3dee2b4985ec3c67ac0d`.
The ordinary Maturin command takes 36.52 seconds; Cargo reports Core fresh and
compiles `iroha_python_rs`. The fresh wheel installs normally and passes the
official ABI-23 probe; extension bytes match the prior `1dc6b5c5…` native artifact.
The earlier a39f 231-execution pass and failed first launch remain unchanged.

The C# attempt on 1252 restores and builds successfully, stages the qualified
Apple host library and reads its required exports. Actual tests then exit 1:
25 failed cases, zero passed or skipped, one framework metadata error and
17 missing parameterized rows. Qualification SHA256 is
`2224e9789fde20c3729e2cf270b12e5a4a6b9d21da632631961008fe42e384d2`.
The retained XML traces test-class initialization to `TestAccountId`, which fills
32 bytes with a discriminator and passes them as an Ed25519 public key. Native
canonical validation rejects that key before the intended contract assertions.
The framework cannot materialize the remaining rows; this attempt is not a C#42 pass.
Source/index, tool/cache, host artifact and pin-transition guards remain true.

The one-file fixture correction is applied in both roots with all eight guards
per root passing, application SHA256
`cccca8ece03fc4db30941edf73b63869ae68d8323c5c89cf01604f1dffbbf3f2`.
It uses the existing discriminator bytes as deterministic seeds and derives real
Ed25519 public keys with the existing SDK helper. All 31 callers, assertions and
test names are preserved; no production validator or ABI changes. PRIVATE becomes
`7ce7a7679b267c1f7d8fc7a4876975c162adc75d34de4beb1f7605947069ccf6`;
MAIN becomes `54ee73150586acb00c6796e6cae2f47b95ec0927b241da04ce58ff1ea999ea54`
before this ledger update. Official live-pin, provenance and normalized-fingerprint
checks bind this fixture transition to the same Apple5 artifacts: all three
commands exit 0, all eight source guards and raw-index/transition checks pass,
with unchanged `d35bff76…` fingerprint. Binding SHA256 is
`2400b78174cc940c249c74dd8c94d47cc4cb6c48d8b1a30a4ee9f3b0c910f140`.

The fresh corrected C# run on 7ce7 passes all 42 cases with zero failures, skips
or framework errors, including all 17 previously missing parameterized rows.
All four commands exit 0; source, raw-index, managed/native artifact, tool/cache
and pin-transition guards pass. Qualification SHA256 is
`fef14ac7b821381d29e15ee30662a9c7c260bcb8bf5d03c7d3159ae975b89ef0`;
XML SHA256 is `cffbbc39b77317a1815353980d3aaf9ff6a361ac931ee7fc1251829adf84d75d`.
No assertions or cases are excluded. The earlier failed receipt remains intact.
At that checkpoint, Kotlin17, Swift24, JS31, final-source binary bindings and
workspace gates remain pending.

During earlier Apple preparation, an agent ran `git status` without
`--no-optional-locks`. Git refreshed PRIVATE's index and its file mode changed;
the strict raw-index guard failed before the native binder ran. The observed
index and failed preparation are retained. Independent comparison of all 19,354
entries identifies 34 changed bytes: timestamps for the tracked network lock
file and the resulting index checksum. Paths, blobs, entry modes/flags, extension
sections and staged records are identical. Comparison SHA256 is
`c46ccd8e731971b97fcaeea9c893da67fcc342b8d2f2510f7dc2ae7916f262b8`.
An initial restoration attempt stops before mutation; the corrected recovery
restores the exact original PRIVATE index bytes and mode, while MAIN is unchanged.
All nine guards per root pass, including source, HEAD, staging and Cargo.lock.
Recovery SHA256 is
`7337bc8d8ce88c7a40e6b86e8d1cef7f0e56af85c30a0b1bc6bd6f775175d2ee`.
This was an observed interruption and exact recovery, not uninterrupted raw-index
preservation. Earlier network failures, the unexplained original cleanup exit 1,
Core/formal failures and separate publication limits remain recorded; C9 is open.

## SDK index recovery and highlighted documentation

Kotlin17 on 7ce7 passes every case, including the Java consumer, with zero failures
or ignored cases and all source/index/host guards. Receipt SHA256:
`216510a12274deb3e4cf1dca7dca1903c46fcde169a3c3175ab35b5863654ea0`.
The first Swift capture passes 24 native cases but remains unqualified because
PRIVATE's raw index changes from 8d7a/0644 to b04a/0600; receipt
`74410ebad81d021b7521a1a520d0f238e08f376e8be78879783df5391fe993ab`.
Forensic receipt `8e85d5525f123618e2b169bb1adf0e339982a8125124945f35939f3301327692`
identifies only one entry's timestamp fields and checksum; source, staging and
Cargo.lock remain equal. It does not identify the writer. The overlapping daemon
receipt `deaf4abc3d32c1ecd8c71a0cf8060ee2ae7786d6037cd734b1de556611640cbf`
records native 0 and generic `qualified=true`, but `raw_index_bytes_changed=true`
is a strict qualification failure; matching frozen bytes do not waive it.

Root recovery `abdb92d216d2eeedbaf8919b23fbe591e4e83059da5f9994d7327c351d99f7bc`
restores exact original PRIVATE index bytes/mode with all nine guards per root;
MAIN remains unchanged. The subsequent Swift run sets `GIT_OPTIONAL_LOCKS=0`
and qualifies all 24 cases, zero failed or ignored, both commands 0 and exact
source/index/lock/artifact preservation. Receipt SHA256:
`5f9ab0318af4261f9c1db6491af9e2d6261b971dbe2b4746976ea7420ae8b69c`.
The JS attempt instead stops before starting any child: pinned Node 26.8.1 is
absent. Receipt `bbd7493a080b3f2dcb7565dd556299865ca6e36a68f755ae1a3c47dc632d47a2`
retains that failure with all preservation guards. The subsequent installed-26.8.2
attempt completes normal native build, provenance, local artifact staging and
ABI-23 probing, all native 0. It executes 29 cases: 28 pass, one fails and the
final two contract-call cases are not reached. Qualification remains failed:
`d1ed22efeb669ec3711df5b0fb93ee391c8ac654319ffbf644fe5385589326a1`.
The failed deployment-provenance test expects a schema-hash mismatch but receives
an earlier authority TypeError. Source review finds `smartContractDeployment.js`
reading obsolete `_controller`, while `AccountAddress` uses private WeakMap state;
the fixture authority is valid. The production correction and complete JS31
qualification are pending. Source/index/lock/pin/tool/artifact and prior-evidence
guards all pass; no retry or source edit belongs to this capture. New final-source
daemon/binary captures are in progress; no workspace test/build/Clippy gate has executed.

The older sibling-docs 12 GiB receipt could not be located in the bounded retained
search, so its logs were not reconstructed. A fresh six-command run on e2a passes
161 tests and produces 5,997 outputs with all five preservation guards; receipt
`8db68cdd9137f24a881c6cc07ce5391a4c9b686dc9377f5fd10fe6699b69ebb4`.
The first attempt failed before content assertions because its blanket network
sandbox denied TSX Unix IPC (`5af7dd68…`). The corrected policy proves IP denial
and local Unix IPC, then runs the unchanged validators and tests offline.

That build exposed unhighlighted new tutorial `ko` fences. Six docs-only paths
now vendor exact canonical grammar
`0efdae17609b959d4b72375d1fdcb0d62921ece482fde77dcc59fae2ee65f8b8`,
register local Kotodama/ko and record pending signed-source provenance. No package,
lock or Iroha source changed, and builds do not read the sibling Iroha checkout.
The first renderer test's standalone-span assumption fails (`cfb1214d…`); the
corrected actual renderer checks equivalent Romanized/Japanese keyword styles and
named labels against ordinary identifiers, passing 3/3 with all guards (`d3eecd48…`).
The docs source is `7e397d5184a6cc3803bb49fa1892bd998e9994632b0ecd3993ea94c6de96c9f0`.

Fresh qualification `817a6e4b48484a4a9889f19debcb8fb56fc593a211162d5fdfa82a3c862cea25`
passes all six offline commands: validation, 164 tests in ten files, typecheck,
explicit 12 GiB VitePress build, links and all 21 locale roots/revision checks.
The build takes 96.48 seconds and creates 5,998 outputs; all five source/index/
dependency/existing-dist/tool preservation guards pass. Built-output readback
`0fe7dc2e58403e93b0f4834cdd23a577e056bce91670917312241a99658a0e32`
confirms all six tutorial blocks have colored spans and no `ko` fallback warning.
Other deprecation/chunk warnings remain. The package's earlier 6 GiB OOM is not a
`pnpm build` pass. Earlier docs attempts and original historical records remain intact.

The closing acceptance supplement retains all 35 original matrix rows and its
1,753 executions at original scopes, without summing overlapping later suites.
Current bytes match 4,502 compiler/Koto/ABI records and six editor/package records;
the 7ce7 read-only syntax check passes 18 outputs with all nine guards, receipt
`1f6fc745b9cb737e14c0361b3fcef417c2b1460935a46fa023e247778f4200ae`.
C9 remains open for the JavaScript authority fix and full JS31 selection,
final-source bindings and ordinary workspace gates;
Core/formal failures, earlier network failures and the unexplained prior cleanup
exit 1 retain their exact scopes and classifications.

## JavaScript controller repair and preserved-index daemon build

The ordinary release-daemon recapture completes on unchanged source 7ce7, from
17:31:34 to 18:16:47 UTC on September 10. Native exit is 0; all nine capture checks
pass, `changed_source_paths` is empty and `raw_index_bytes_changed` is false.
Receipt `6046cd76ff509a161c69387d96e5c6d43816a8153a14e4dd8da06946323c1073`
binds the normal opt-3/debug-0/non-test binary: 343,283,728 bytes, mode 0700,
SHA256 `e6abc762ded9c5e485cd00d661232539c13f27d0bf78122b1b138a80983ddac5`.
The earlier `deaf4abc…` capture retains its failed index guard; matching binary
bytes do not turn the interrupted preservation interval into an uninterrupted one.

The reviewed 17-path JavaScript correction is then applied in both roots, with
all ten guards per root passing. Application receipt:
`80c2fdf44b0d7cfef3d9320d8329e99532b817d5600c59f28ec1b1a060ad251d`.
MAIN becomes `70e2eb4308a6141229940861a0d0059c4de269e584b8385003d931065b80f2aa`;
PRIVATE becomes `231de5ae18c4ccfe9ba0e5e11f92525fbd022d0f35ac1f05a494e64918af5f47`.
Only JavaScript SDK source, declarations and tests change; Rust, other SDKs,
Cargo.lock, HEAD, staging and exact index bytes remain preserved.

`controllerInfo()` returns normalized single-key/multisig records from private
state. Records and member arrays are frozen; each public key is a fresh byte
array. Six public consumers replace obsolete `_controller` reads without an
admission fallback. New tests retain all existing assertions and cover borrowed
receivers, nested key isolation, strict root/subpath TypeScript declarations,
valid multisig/non-Ed25519 restrictions and independent compact receipt bytes.
The independent reference reproduces the existing Rust receipt digest before
pinning the multisig variant. The complete 72-case selection retains every
original JS31 case and the additional multisig contract-call helper caller; actual
execution is pending. The official Apple transition readback
`47442f83afa4e227ae2c6e2f53663b88488efb6b628e957023e1304dae3f0abe`
passes all three commands and eight preservation plus raw-index/transition guards
on 231de5ae, retaining the d35b native fingerprint. Python stays scoped to 1252;
C#/Kotlin/Swift and the failed JS attempt stay scoped to 7ce7. Earlier JS29
failure, SDK/native source scopes, final-source bindings and unexecuted workspace gates remain
explicit. No release-completion or clean-teardown conclusion follows.


## JavaScript 72-case qualification

The complete native JavaScript selection passes on unchanged source 231de5ae:
72 passed, zero failed or ignored across 14 groups, with all 21 subprocesses
exiting 0. Receipt:
`60bf644c894f73ffc3f3f35199dc397be51acca779631cb53b34330c5d1fe056`.
The capture includes ordinary dist/native build, ABI-23 probing, all original
31 cases, strict TypeScript/controller tests, three browser parity cases,
all 13 deployment cases and the multisig helper's public caller. The earlier
failing deployment test and both previously unreached contract-call cases pass.
All source, exact index, lock, pin, Node/runtime, built-dist and retained-artifact
guards pass. The `d1ed22ef…` failure remains unchanged at its original 7ce7 scope;
Python remains scoped to 1252 and C#/Kotlin/Swift to 7ce7.

Complete build-input continuity for retained daemon/CLI outputs is being checked
before current workspace capture. SDK-only changes do not justify repeating
Rust/CLI execution when their complete build inputs remain unchanged. No
workspace test/build/strict-Clippy result or release completion is claimed.


## Retained native input continuity and workspace start

Input review `65e7974f0b2a97a9d6528123cdd2c86b08fd5a119924f45f8897d59fb0bc5952`
passes 22 checks: exact 17/25 SDK-only deltas do not intersect 2,663 actual
dependency-source inputs; 61 manifests, nine build scripts and eight proc-macro
trees are unchanged, and optional Norito SDK-sync was not enabled. Current
dependency records corroborate retained logs/frozen outputs, not capture-time
snapshots or new executions. Admission
`5ae25b17d796da16670f76530d049a178e6e506f6f9039d53bfdd062657b029c`
passes 14 checks and three previews with 84 immutable observations; the five
original a39f CLI/message-control and 7ce7 release identities remain unchanged.
The ordinary workspace test starts on 231de5ae at 19:09 UTC on September 10,
with source/index admission passed: `20260910T190950Z-workspace-test-18bbe76ea4`,
runner PID 98135/Cargo PID 98161. It is running without a terminal result; build
and strict Clippy remain unexecuted. Prior failures and limits are unchanged.


## Workspace CID test compilation failure

The ordinary workspace test on 231de5ae closes with native exit 101 before any
tests execute, receipt
`72fe897efa6cfa9f1e123913cd86f3e49831fcee01991c251e8f3f8382def9c6`.
All 11 preservation checks pass. `irohad` library tests fail E0599 because
`server_source_tests.rs` calls `.as_bytes()` on the `Result` returned by
`ManifestRootCid::from_blake3_digest`. Lineage receipt
`ce5bbb013df1e84f3931d2d69853d7057e15eef3124a7ca5c06630481ada1a5e`
shows both complete files unchanged from retained f899 through 231de snapshots;
it does not establish an authenticated original dirty-worktree baseline or
attribute the defect to this task. The one-line test-only correction adds
`.expect("nonzero test manifest digest")` in both roots; the production
constructor is unchanged. Validated application
`1b34807ab68b58655cefbe8fcc5c299cb021cd006132e257ce307c4df2d831b5`
passes all 16 guards, producing PRIVATE
`cb32e4125347e45c2e877d1f15e94fdfa602ba41fa51dfb750de2bf43d3c7bc5`
and MAIN `185614fb9246f8f953b9777c80857f9171245e41a4477af4b3a96728de6ca829`.
The initial application report `6ad86c36…` remains retained: its status guard
incorrectly required unchanged worktree status after the authorized edit; the
validated record checks the exact one-file status delta. Source, index, staging,
modes, HEAD and lock guards are preserved. The resumed workspace test, build
and strict Clippy remain pending; no validation pass is claimed.


## Workspace alias-bootstrap compilation failure

The workspace retry on cb32e412 compiles the corrected CID caller, then closes
with native 101 before any tests run, receipt
`ac837f2b96fc6761fab02b14608519e692e1202732991662d03c6e6ce45c1243`.
All 11 preservation checks pass. Ten diagnostics arise from three test-only
`alias_registry_bootstrap_network.rs` sites: fallible query error conversion and
two nested `Cow` arguments. Application
`51ba2e491010170ed09fb00aaf6e189ece1c950c79ac1e9e93da30c70d8b22c1`
corrects those callers in both roots with all 16 guards and rustfmt passing;
PRIVATE becomes `d13c73bdc859af2c60c3890929a6f83f10919ef68b9fbc85b87574e38bcb7749`,
MAIN `9202c01de1dacadd1fba11d9c1fa31a69723f5e90c30d6252fc55e65638a579e`.
Scope/lineage receipt
`e3df15ece5d5b66d2f9a85228280a441903764f7be9979985553b2da88d3993e`
shows the failing preimage matches retained f899 and MAIN committed HEAD;
PRIVATE committed HEAD is older, and original dirty-worktree attribution remains
unproven. Retained production/SDK and Kotodama harness inputs exclude this
separate test target. Assertions, topology and production APIs are unchanged.
The resumed workspace test, build and strict Clippy remain pending.


## Workspace SoraFS callers and current Apple provenance

The d13c workspace attempt closes native 101 before tests execute, with both
prior fixture errors compiled and all 11 preservation checks passing. Receipt:
`5ce96b1ddacf2d35269f3d8019d4105138d373b0a182264b57fef2b2d076765f`.
Three SoraFS callers are corrected in both roots: two readers pass explicit byte
slices, including one production CLI-bin function, and the canonical test uses
final `load_prepared_storage_payload` with real directory/manifest commitments
and all prior assertions. Application
`5168aa06aae6bb3112026d3e5421ca10ff89c51335ae9922021aa6b2f1911476`
records three sequential 16-guard transitions and rustfmt exit 0; PRIVATE becomes
`65021e4dd261ad2fcca057c215e07cfedbdd399f63070168456a5b67dbd1f71c`,
MAIN `9f0c9adef9b39c80a1e1b312452bc3d5825078e3a93b45c865a56800a351ee66`.
Scope review `fdd38ee749b04ebd6bada1982a35aef562d518517a32bea6c18ddc174b6ceb2c`
passes ten checks: the exact three BIN/helper paths are excluded from retained
program, SDK and Kotodama harness inputs, while the shared library is distinct.
All three preimages match retained f899 and MAIN HEAD; this does not prove the
original dirty-worktree baseline.

The official Apple whole-package selection includes these paths, requiring a
fresh ordinary five-target producer. Apple6 qualifies on actual 65021, receipt
`129f016b0af2e56bbcc6286304f4b80def5ea286848440cbb5f36e0a1e99c1e6`.
All five release targets/C links, host ABI/crypto and official readbacks pass,
with all preservation guards. Manifest
`d4282688b93632cf2c566f3a071b561900cd529024110ba42fa9f8b2828fe6d8`
records current fingerprint
`dd1683a14a8c249a1f0ea98794686941691828a77560a3df72e3301fc18544bd`.
All three native archives and the host match Apple5 bytes; projected loader
`8900ad2b…` equals both shipping loaders, so no pin delta or SDK rerun is needed.
Independent readback `ec1dd475df872338d8acf2043753c22e78e0795502c28451399ccecff7d4bffb`
passes 11 checks. Apple5/d35 remains historical, as do Python1252, C#/Kotlin/Swift7ce
and JS72/231 executions. The five-path transition adds no redesigned-behavior gap.

## Workspace test storage-startup outcome

Current 65021 workspace test closes native 101, receipt
`a4cb1fb25788ddb5ca21615d690401730f8e75b42f9d3536d2861f757124ce6b`:
1,512 passed, three failed and 27 ignored in 11 reported summaries; all 11
preservation checks pass. The failed integration sandbox cases are
`serialized_network_drop_completes_on_current_thread_runtime`,
`start_network_async_or_skip_includes_trusted_peer_pops` and
`start_network_blocking_or_skip_includes_trusted_peer_pops`. Each is refused
startup before block 1 by the existing nonzero automatic storage-budget reserve.
Bounded observation
`8e37ef27c6a7b1aae11a80417680f9f1603824c04133622419110b5ec89dff47`
and terminal readback
`118692d9410bd0f88beb87aa07276add538149bfe2893217c2c2577801ae8ab5`
attribute this execution to insufficient available storage under unchanged
policy/source, not an authenticated original dirty-baseline classification.
Normal Cargo stops at this failed suite; unexecuted later targets are not
counted. No runtime budget or assertion was weakened. The separate ordinary
workspace build and strict Clippy outcomes follow below; focused SoraFS9 remains pending.
All 35 redesigned-behavior rows retain scoped focused passes, while these broad
workspace outcomes and earlier failures remain explicit.


## Workspace build and strict Clippy outcomes

The ordinary workspace build passes on unchanged 65021 with native 0, all
17 checks, 839 Cargo artifact rows and 1,482 recorded outputs in 1,169.449118
seconds. Receipt:
`5d166c203296d6a3044e579aada47ad3077a7467ad877a252b85766e09b2839b`.
This is build evidence, not an added test count. Strict Clippy then exits 101
in 16.464173 seconds at `clippy::option_if_let_else`,
`iroha_primitives/src/conststr.rs:490–493`; all 11 preservation checks pass.
Receipt `b0d32f705cb92e6342b7a9bab4a9773ce5276c38ba03f586ec2026643586018a`
records the actual first error and stop at primitives. Later FASTPQ/tool warnings
are not recast as actual Clippy failures. Authoritative classification
`24a4f7836861db76c0fb90829ad6136c9df304e8d378062ce5c6bb1f4605239d`
confirms the complete file matches retained f899/a39/7ce/current source and MAIN
committed HEAD. PRIVATE HEAD differs; original dirty-baseline origin is unproven.
The initial `5b1e6052…` report's erroneous both-HEAD wording remains preserved
and is superseded by that narrow classification. No unrelated lint repair or
whole-workspace pass is claimed. Focused SoraFS9 and final acceptance recording
remain pending.


## Completed redesign acceptance and remaining workspace health

Focused SoraFS9 passes on unchanged 65021: exactly nine passed, zero failed or
ignored, 100 filtered, in 0.81 seconds. Both native commands exit 0 and all
16 preservation/input/executable checks pass. Receipt:
`ba8d686e67f664a799983b989e1dbc0133736369df170c58988b24de6e0f4c8d`.
The actual source-bound executable covers all eight deployment-integrity cases
and the canonical prepared-payload ordering case, retaining every assertion.

Kotodama V1 implementation and scoped acceptance are complete: all 35 redesigned
behavior rows have focused passing evidence; final syntax/ABI assets, editor,
onboarding, highlighted tutorial/translations, maintained SDK consumers and
four-validator scenarios are qualified at their recorded inputs. Apple6 qualifies
current 65021 native provenance without changing artifacts or shipping pins.
The ordinary workspace build passes. Required broad test and strict-Clippy
gates were also executed: the test run retains its 1,512/3/27 storage-startup
outcome and unexecuted later targets; Clippy retains the unrelated primitives
lint and its limited source attribution. These are remaining workspace-health
outcomes, not missing redesigned-behavior evidence. Historical matrix totals
and source labels are unchanged; no aggregate count, full-workspace success,
clean teardown or release-readiness claim is inferred. Earlier failures, exact
index restoration incidents and the original unexplained cleanup exit remain
retained.
