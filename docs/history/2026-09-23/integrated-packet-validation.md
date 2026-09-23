# Integrated first-release packet validation

Work stayed in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations` at
`1b8e5f92b6dadb1dcdd16f945286560e31b347ff`. The exact-preimage
application receipt is
`target/first-release-reviewed-packet-application-20260923.json` (SHA-256
`4d7bb29c22c88113c87d5661a617fc8fcb7c25aaa26d928a657aa0ee966e82a6`).
It joins four reviewed changes: prepaid MV buffer custody; MKHE source-receipt
identity through coordinator capture and final replay read; a current audit-head
fence before release-manifest signing; and bounded direct-ancestor `@rpath`
candidate accounting for the Node 24 runtime-input relation. The changes do
not activate the Core membership tip, MKHE production composite, protected
SoraFS signer roles or actual Node runtime approval.

The focused source-observed receipt is
`target/first-release-focused-packets-20260923/receipt.json` (SHA-256
`da0ab007761d8a4802122deb1da0e1381f31b9d52eef4eec39507693710a0b61`).
All four suites passed on unchanged source: 25 MV buffer-custody tests, 13 MKHE
source tests, nine MKHE pretranscript tests and 20 release-manifest signer
tests. The expanded MV composition executable subsequently passed 28/28 after
three test-only checks of actual Shared-header/backing retirement and deferred
refund ordering. The focused JavaScript runtime-input suite now passes 124/124,
including a two-requester regression that refuses omitted executable-ancestor
slots on either shared `@rpath` edge. The actual recorded 20-image manifest
remains rejected for four omitted claims; an in-memory corrected manifest
passes only the pure content relation and has no independent authority pin.
Rust formatting, the retired-codec guard and `git diff --check` passed after
the four production packets were applied.

The combined selected Core run built its actual test binary, passed the former
ordinary-stack publication overflow case, and completed all 763 selected tests
without stalling. It passed 754 and failed nine fixture setup cases. Its receipt
is `target/first-release-core-history-combined-20260923/receipt.json`; the
captured 9,534 source inputs, branch, HEAD and selector file had no drift.
Seven replay tests attempted to write a blank Kura before authenticating the
configured initial lane geometry. Two lane-body tests installed a second lane
only after opening a default pre-genesis Kura. Those failures preceded each
test's intended adversarial assertion. The corrected fixtures first bind the
exact configured geometry and construct State against the final pre-genesis
catalog. All 28 tests in the three affected modules pass with no source drift:
`target/first-release-core-fixture-recheck-20260923/receipt.json` (SHA-256
`49b8554f421d33c228c43da028b316d94f30970a726ee5909c56018c4090406b`).
The full selected rerun then completed **763/763 passed, zero failures** on an
ordinary worker stack, with no source drift or stall. Its original
`target/first-release-core-history-combined-rerun-20260923/receipt.json`
reports a wrapper failure after the successful test command: Rust displays
` - should panic` after two passing test names, while the wrapper compared
them literally with discovery names. The separate
`target/first-release-core-history-combined-rerun-20260923/posthoc_verification.json`
(SHA-256 `81fa8acd36cbd63d1bf4a49272700f2f76e4a125ce3acf1aae93f8e55b7b5b82`)
verifies the unchanged 9,534 source inputs, binary and log hashes, exact
763-name selection after normalizing only those two display suffixes, 14
required tests, zero failures, and the original receipt's failed state. This
is a scoped Core regression pass, not release qualification. The first three
replay failures also appeared in the earlier pre-packet run, which stopped
later on a separate test self-deadlock. The corrected publication test and
full publication module passed separately on the ordinary stack before this
run.

The actual 20-image Node artifact remains rejected for omitted ancestor-slot
claims; the input relation alone does not establish loader, process or native
execution authority. The release-manifest ceremony still uses test-only
operation-state and key-provider implementations; protected roles 11, 13, 14
and 15 remain disabled. All first-release completion goals and final privacy,
SoraFS and multilane release gates remain open. Authenticated software custody
has no HSM prerequisite, and no compatibility implementation is admitted.
