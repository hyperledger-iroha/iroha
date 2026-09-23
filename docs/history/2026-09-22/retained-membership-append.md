# Retained membership append prerequisite

This change extends the fixed membership records with one retained append attempt.
It does not enable Native Validate-to-Apply or replace production State checkpoints.

## Ownership and bounded work

The State owner retains the actual prepared transaction identity, authenticated
current/rollback roots, one fixed workspace, one pending 184-byte frame, and the
original Kura range. Exact allocation charges cover the concrete boxed owner and
the codec constructor's declared schema-name scratch. Demand visits the actual
net changes, with a conservative height-plus-257-nodes bound per changed key;
it never reconstructs historical membership for a hot update.

Repeated preparation checks original identity before I/O. Completed records must
match their exact expected bytes, including physical references. Short writes,
errors and unwind retry the same pending slot. Sealing requires complete replay;
sync retries retain the sealed candidate without repeating logical preparation.

Kura owns one generation-1 descriptor under its original store-root lock.
`[kura.membership_storage]` declares finite physical and allocation limits.
Actual segment bytes join the fixed physical inventory and disk scans; pending
bytes join the checked sum of existing publication reservations. The formal
checker, ledger and mutation controls bind that defining sum together.

The range keeps the original prune/canonical notification batches through every
operation and error. Completed cleanup transfers those batches and the original
range event outside all enclosing State writers. Memory refunds remain under the
original allocation pool's enclosing refund scope. No synthetic wake source or
new retry owner substitutes for these original controls.

Post-write metadata failure retains the exact bounded native request and original
physical mutation. Recovery authenticates only its possible prefix on the same
descriptor. It leaves the disk-scan operation free, then publishes the retained
physical delta and invalidates cached totals to avoid counting a prior rescan twice.
An unfinished dropped range retains its uncertain capacity and requires recovery.

## Validation

The final Core/Config/Torii library test-target build passes with **7,571 unchanged
compiler inputs** and no compiler diagnostics. Current runtime evidence contains
**1,248 Core controls**, the **full 651-test Config library**, and the **21 retained
Torii admission controls**, with no failed or ignored selected tests. Core includes
all 1,004 previous selected controls, every current membership control, all ten
new Kura range controls, and additional actual disk/capacity/inventory regressions.
The new State and Kura controls pass on the default harness stack.

The 18 focused Native preparation/accounting mutation and positive controls pass
on unchanged sources; 1,187 unrelated controls in that file are deselected. The
full canonical multilane structural checker, formatting, retired-codec guard,
history verification and both working/staged whitespace checks also pass.
These structural checks do not establish production trace extraction or fresh
four/seven-validator network behavior.

Commands and exact selections are captured under
`dist/sumeragi-main-work/generation166-append/`. The compilation command is
`scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test -p iroha_core -p iroha_config -p iroha_torii --lib --no-run --locked --offline`.
The final source/artifact receipt is `checkpoint-verification.json` in that directory.

The first build window includes review-time changes and is diagnostic evidence.
The Core runtime window includes two independently corrected Torii DA files;
its raw source-window result remains false, while all 1,248 individual tests pass.
Reuse is justified by the final unchanged compiler capture emitting the **identical
Core executable**, not by overlooking changed inputs. Config likewise joins its
exact executable; Torii admission is rerun on the corrected final executable.
Earlier preliminary/formal runs and the interrupted full-file pytest invocation
remain recorded separately and are not substituted for final validation.

The independent SDK/DA task also changed adjacent Torii diagnostics and performs
its own DA/CLI runtime qualification. Those changes and results are not counted
as new consensus implementation or extra Core controls here.

## Remaining acceptance boundary

The current production transaction-membership map is unchanged. Existing files
require authenticated recovery rather than adoption by length. Root installation,
complete State restoration, incomplete-range recovery, authenticated reclamation,
both incremental Apply checkpoints and the retained production consumer remain
required before this finite store can replace resident history. A finite quota
or abandoned range is not an automatically retryable resource condition.

The original five review findings have their prior scoped regressions; this record
adds append preparation and physical custody evidence only. L1–L6 and the unchanged
four/seven-validator fault/restart/final-transaction acceptance candidate remain
open. No backward-compatibility decoder or alternate production path is added.
