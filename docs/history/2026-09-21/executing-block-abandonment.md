# Executing Storage block abandonment

The original executing Block owned current and undo ReleaseGuards separately.
Default destruction could notify an undo waiter while the current writer remained
locked. A native wake panic could then poison the still-held current mutex or
leave its notification reporting callback unwind instead of physical lock poison.
The same gap existed during reset, replacement copying and admitted restoration.

Storage now uses one private StorageWriters owner for the original pair from the
instant both acquisitions succeed through ordinary or prepaid execution and
prepared publication. It retains the original Storage reference, from which
publication and admitted pool authority are derived. There is no second map
engine, heap wrapper, unsafe extraction or compatibility path. Opening keeps this
owner through predecessor capture, next-identity creation and fallible reset or
replacement; admitted snapshot copying also retains it until publication.

Abandonment consumes both raw writers through the previously qualified paired
release operation. Both physical destructors finish before callbacks, and actual
mutex poison is sampled before either native notification. A payload panic after
unlock and a later wake panic cannot substitute for that physical verdict. Child
transactions borrow the same original checkpoints. Their dirty/failure and undo
semantics remain unchanged. Capture and publication consume the original pair
once. Refusal during partial acquisition still names only the actual held writer.

Three new required regressions cover executing abandonment with normal drop,
caller unwind, either private payload panic and first-waker unwind; replacement
copy failure before Block construction; and admitted ordinary/replacement
callback refusal, callback panic and native wake panic after an applied child
transaction. They verify both releases, real/native poison agreement, original
committed current/undo images, unchanged predecessor and healthy owner reuse.
Existing reset/refusal, replacement, snapshot and transaction controls remain.

## Scoped validation

Only `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`, was edited. Both
complete MV layouts pass 283 runtime tests and five doctests each with zero
compiler diagnostics. Exact executable/source inventories preserve all 272 HEAD
cases and the eleven subsequent cases. All 7,275 captured Rust build inputs match.
The release harness passes 220 cases plus 2,698 subtests after correcting two
stale per-module counts. Its mandatory ownership selection is 267: MV 65,
admitted-map 64, map 24, EBR 5 and Concread 109. The basic/full selectors contain
1,379/1,557 cases, plus five on Linux.

Evidence lives under `dist/sumeragi-main-work/generation139-core`,
`generation139-release-census` and `generation139-formal`. The final
`dist/sumeragi-main-work/validation139.json` receipt records completed Core
compilation/runtime, shipping checks and canonical binding checks separately.
The first preliminary MV build overlapped formatting and is not immutable-source
evidence. The first release suite failed its stale census assertion; the corrected
complete suite is retained separately. A runner call missing its output-directory
argument executed no tests. These attempts are not represented as passing gates.

This closes the executing Storage pair's abandonment boundary. Whole-State
physical preparation must still acquire every fallible lock before any field
transfer and retain actual retirement/notification custody through its outer
fences. Concrete World payload/native control policy, aggregate admission,
production retained Validate-to-Apply, full workspace and unchanged real four-
and seven-validator fault/restart/final-transaction qualification remain open.
L1–L6 remain active; these scoped checks do not establish release readiness.
