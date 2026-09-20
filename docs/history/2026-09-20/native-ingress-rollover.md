# Native physical ingress across global rollover

Native messages now have a process-lived source identity in the original fair
ingress queue. Native and other authenticated sources share the existing finite
source pool, byte limits and scheduling class. A Native source does not inherit
global validator privileges or a global lifecycle ordinal. This is physical
custody, not permission to vote for a Native instance.

Both global retirement and roster replacement preserve the original Native
message allocations, canonical bytes, occurrence IDs, coalescing entries and
source rotation. Successor binding accepts retained Native custody only after
global custody is gone. Opening includes retained messages in its protected-slot
calculation. If successor geometry exceeds capacity, admission stays closed and
the retained messages remain available to drain. Full shutdown still requires
the entire queue to drain.

The production Native entrypoint remains closed. Its sole process-lived consumer,
the retained Validate-to-Apply execution owner and retirement of the old signing
path must be connected together. Existing production rollover loops still require
full drainage; they must distinguish the retiring global height from independently
live Native instances when that consumer is installed. No fallback or second
scheduler is introduced.

Shared capacity alone does not prove a Native committee's admission opportunity.
Activation must account for its frozen peers before accepting their instances and
show that continuously replenished authenticated traffic cannot occupy every
source slot indefinitely. The current physical tests establish bounds and retained
custody, not that stronger scheduling property.

Epoch93 compiled the seven-package harness selection in 443.86 seconds. Its
physical-ingress selection passed 55 of 56 tests. The new rollover test inherited
`u64::MAX` from a maximum-wire fixture and overflowed when advancing the height.
The correction uses height 41 consistently in the proposal, manifest, timeout
certificate and Prepare certificates, preserving a real successor and disjoint
four-validator rosters. No production behavior was changed for that correction.

The epoch93 Rust input hashes and captured executable stayed identical, but HEAD
and index changed during the run. Its receipts retain that identity drift and the
failed test; they are not an unchanged-candidate pass. Subsequent source-bound
results must qualify the corrected fixture separately. These tests use structural
canonical Native messages and actual durable global lifecycle gates; they are
not Native cryptographic-quorum or live-network qualification.

Epoch94 passes all 56 selected physical-ingress and global-retirement tests on
unchanged Rust inputs, HEAD and index, using the captured Core executable with
SHA-256 `8f2c422922b4aa52ca9076b9f86efbdfedf7b3a6c43746aa5e546a43a257b4a1`.
The new canonical ingress contract checks the public gate, shared source budget,
both retention cuts, original occurrence evidence and strict full shutdown drain.
Its 33 positive/mutation controls and the aligned canonical preflight pass. The
combined build and final source-bound formal/guard results belong to their
separate epoch94 receipts under `dist/sumeragi-main-work/`; no previous epoch's
publication or network result is added to this selection.
