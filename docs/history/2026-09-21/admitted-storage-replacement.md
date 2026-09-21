# Admitted Storage replacement

`Storage::try_block_and_revert_admitted` restores the preceding block through
the original prepaid B+tree engine. It acquires undo before current, reserves
both writer shells together, and borrows the exact held undo generation.
Each restoration admits the current edit and explicit incoming key/value copies
before allocation. Removing a prior insertion uses the same closed removal
operation. Clearing undo happens only after all restoration succeeds.

Every normal refusal drops both unpublished writers and preserves their
committed roots and publication identity. This includes capacity exhaustion
after a restored prefix, a payload planning refusal, a changed second plan and
final undo-clear refusal. The caller retries after releasing its enclosing
owners; replacement never waits while holding its writers. Successful capture
retains `Replace` mode and records subsequent preimages from the restored state.
Factory cleanup finishes while the private writers can still abort. A panic
while either writer remains held poisons that original writer.

Ten new tests cover the boundaries above, physical contention and exact wake
ownership, detached publication abort/retry, empty undo, copy/factory panic,
retained readers and complete original-credit release. Allocator instrumentation
matches every observed replacement allocation to an original layout charge and
requires actual deallocation before refund. Nested payloads reject ordinary
`Clone`; copies must pass through their funded policy.

The complete MV suite passes in both default and skinny tree layouts: each
runs 244 runtime tests and one documentation test without compiler warnings.
Checkpoint134 records source captures, artifacts and remaining checks in
`dist/sumeragi-main-work/validation134.json`. Earlier checkpoints retain their
own evidence scope; these changes do not establish network or release readiness.

Production World stores still need concrete payload policies, funded
construction/restore/history, local-refusal propagation and an aggregate
execution memory/work policy. Per-edit admission alone cannot activate the
retained production validator. The original `V2ApplyService` already survives
the recovery-to-worker transfer; the missing consumer must retain execution
through Apply and rebuild unfinished-height owners before exposing recovered
validation markers. The new Native batch owns its sealed application markers;
it does not require the retired participant representation. L1–L6 and unchanged
real four/seven-validator fault qualification remain open.
