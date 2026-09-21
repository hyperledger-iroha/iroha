# World capture custody

The two original checkpoint153 controls reproduce premature notification in
ordinary and replacement World capture: releasing parameters wakes its original
waiter while the peers writer is still physically busy. Each uses actual native
release observations and nonblocking preparation of the original peers journal.
This establishes the ordering defect, not a measured production deadlock by itself.

## Ownership

`mv::BlockCapture` now keeps each original Cell or Storage block in a caller-owned
slot through operability checks and retention admission. An attached phase retains
its returned admission. Successful capture moves the original current/undo
allocations and publication metadata into the captured phase and retains the two
original native notifications in `CaptureCleanup`. Refusal or unwind leaves the
same attached owner available for terminal release. Failed map cursors can only
abandon their private state. Standalone and prepaid capture delegate to this kernel.

World derives all capture slots from its sole field inventory. Every inert slot
exists before any field capture begins. Aggregate Drop releases every remaining
physical writer before field cleanup. Successful capture completes every field
before materializing retained wrappers or delivering notifications. The original
extras and admission outlive pending captured payloads, including a callback panic
during final materialization. Borrowed, separately outlined fill and finish calls
keep their large temporary frames out of native capture work.

TriggerSet uses the same rule for all ten stores. Its attached, capturing and
captured phases retain original ownership through checks and admission. It keeps
all ten child notification owners until the enclosing World releases later fields.
Standalone `SetBlock::try_detach` delegates to this same slot.

## Validation scope

Original failing controls and development compiler outputs are retained under
`dist/sumeragi-main-work/generation153-core`. The final joined receipt must match
complete source keysets, emitted test binaries and exact runtime inventories;
source-binding checks do not establish production liveness. Native/MV capture
controls cover ordinary/replacement identity and allocation retention, admission
refusal/panic, failed map cursors, actual raw-writer poison and cleanup panic.
Core controls include actual World and TriggerSet success, nested trigger-to-later-
World-field notification order, and World admission refusal/panic. Existing
review regressions and default-stack retained-validation controls remain required.

## Remaining boundary

This change addresses Cell/Storage and World/TriggerSet capture ownership.
Consuming commit, enclosing State/runtime acquisition and capture, complete
resource admission, the production retained Validate/Apply cutover and unchanged
four/seven-validator qualification remain open. Callback-local destructors remain
the callback's responsibility. All L1–L6 remain active.
