# Combined-source file-budget gate — 2026-09-23

On the existing `optimizations` checkout, `python3 scripts/check_source_file_budget.py`
initially failed with 279 findings across 12,804 checked files (168 configured
exceptions). Splitting the reserve tests into a code-adjacent file reduced this
to 278 findings. A mechanical shared-test-fixture extraction brought
`crates/iroha_core/src/privacy_state.rs` from 15,783 to 14,331 lines and
lowered its 14,920-line exception to that exact new count; the check then
reported 277 findings. The new reserve regressions live in
`crates/iroha_core/src/smartcontracts/isi/asset/privacy_public_reserve_tests.rs`;
`core_numeric_mutation_tests.rs` is 2,949 lines. Neither extracted file now
appears in the budget findings.

After the external merge finalized on `optimizations`, a fresh JSON diagnostic
reported 277 findings across 12,837 files. I ratcheted 34 other exception
entries: 32 baselines fell to the exact smaller source counts, and two were
removed because those sources are now below the default 5,000-line limit. No
baseline or limit was raised and no new exception was added. After the F02
source settled, I also lowered `crates/iroha_core/src/block.rs` from 30,778 to
its exact 29,085 lines. A fresh check reports **242 findings across 12,841
files and 166 exceptions**. The diagnostic JSON under `target/` is
local working evidence, not a release receipt; the remaining growth and
over-limit findings still block the gate.
The subsequent test-module splits moved the unchanged bridge and beacon
regressions into `bridge/tests.rs` and `beacon/tests.rs`, and moved ten
governance-rooted-filesystem platform/ACL tests into
`governance_rooted_fs/platform_security_tests.rs`. All new test files fit the
3,000-line limit. The F02 move-only merge-beacon helper reduced `block.rs` to
29,083 lines, and I lowered that exact exception again. A fresh combined check
reports **239 findings across 12,848 files and 166 exceptions**. Focused Cargo
validation of these mechanical test moves and the new F02 helper is pending.
The budget checker's focused Python suite passed 51/51 after the downward
ratchets.

A byte-for-byte test move split two SCCP replay cases into
`IrohaSwift/Tests/IrohaSwiftTests/SccpReplayV1Tests.swift`, reducing
`SccpV1Tests.swift` from 3,019 to 2,800 lines; the new file has 225 lines.
Swift parsing passed and all 28 original test names remain. The fresh checker
reports **238 findings across 12,853 files and 166 exceptions**. Swift runtime
tests remain pending. The moved Core beacon tests passed 29/29, SoraFS node
platform/ACL tests passed 7/7 on this host, and the move-only merge-beacon
test passed 1/1. The signed epoch-one fixture repair subsequently passed the
fresh Core bridge selector at 77/77; see the separate SCCP fixture record.
Moving 13 byte-for-byte explorer parsing tests into
`IrohaSwift/Tests/IrohaSwiftTests/ToriiExplorerParsingTests.swift` reduced
`ToriiClientTests.swift` from 20,918 to 20,278 lines. Its exact no-growth
exception was lowered from 20,350 to 20,278. All 444 test names remain and
Swift parsing passes; execution is pending. The fresh checker reports **237
findings across 12,854 files and 166 exceptions**. No limit or baseline was
raised.
Moving the internal 92-line `NativeBridgeError` enum verbatim into its own
Swift source file reduced `NativeBridge.swift` from 8,609 to 8,516 lines; its
exact no-growth exception was lowered from 8,537 to 8,516. Both files parse,
but runtime/typecheck validation remains pending. The fresh checker reports
**236 findings across 12,855 files and 166 exceptions**.
The two new FASTPQ diagnostic/test scripts bring the current check to 12,857
files with the same 236 findings and 166 exceptions.
A byte-for-byte move of the Taira `RetireLiveReferenceTests` class into
`scripts/tests/taira_retry_live_reference_test.py` reduced the original retry
test from 3,088 to 2,928 lines. Both modules pass 129/129 pytest cases and
preserve every test name. The checker now reports **235 findings across 12,858
files and 166 exceptions**; neither Taira file appears in the findings.
The F02 pristine-effects owner moved unchanged to a code-adjacent include,
reducing `block.rs` to 29,061 lines. Its exact exception was lowered from
29,083 to 29,061. With that new file, the checker then reports
**235 findings across 12,859 files and 166 exceptions**.
The [Sumeragi release-bootstrap test split](sumeragi-release-bootstrap-test-budget-split.md)
moves the five direct cases into the existing lexical component pattern. The
canonical module is now 2,891 lines and the component 273 lines; the current
checker reports **234 findings across 12,863 files and 166 exceptions**. The
full bootstrap module still fails, and this file-budget gate remains open.

`swiftc -frontend -parse` passed on all six changed and new Swift files.
`swift test --filter SccpReplayV1Tests` stopped before package compilation:
`Package.swift` requires `dist/NoritoBridge.xcframework`, and neither that
artifact nor the allowed local `target/norito-bridge-local/artifacts` copy
exists. The repository's local-integration bridge build is a separate
prerequisite; no Swift runtime test pass is claimed.

The file-budget gate remains **OPEN** alongside full formatting, strict
Clippy, workspace, SDK and release validation. The findings must be resolved
against the final combined source, not waived by treating a current checkout
as a qualified candidate.

After the role-11 native instruction, Core source-binding tests, BFV negative
test and X509 geometry-screen edits, a fresh checker reports **235 findings
across 12,874 files and 166 exceptions**. The checker still exits nonzero; no
baseline or production limit was raised by these edits.
