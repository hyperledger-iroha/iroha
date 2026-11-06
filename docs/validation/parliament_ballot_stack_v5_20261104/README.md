# Parliament retained-image stack-v5 diagnostic

The bounded no-ordinary-return rule refines ordinary control flow in both retained
November 4 worker images. **All ten complete stack bounds remain `null`. Every
v4 call and frontier retains its exact original address and caller depth; the
resource graph and positive-weight recursive components remain unchanged.** This
is artifact-bound static evidence, with no qualification uplift.

The rule requires a complete, contiguous, aligned interval between two exact
`LC_FUNCTION_STARTS` entries. Before its first UDF/BRK, the interval permits only
known-effect exact straight-line mnemonics and direct BL calls to known function
or native-stub entries. SP must remain known, nonpositive and 16-byte aligned.
After that terminal instruction, only decoded UDF/BRK padding is accepted.
The final function, missing words, malformed operands, unknown stack effects,
RET, branches, indirect calls, HLT, dotted prefixes and non-trap trailing words
refuse a fact. Facts bind the exact interval plus full disassembly, starts,
stubs, binary, tool and analyzer identities. They are rederived before use.

Direct calls may return only under the existing ordinary ABI assumption. There
is no symbol-name inference, native cost assignment or transitive fixed point.
The caller retains its call edge and depth; terminal callees retain their full
conservative analysis. Only the impossible ordinary continuation is omitted.
Every removed baseline call or frontier remains machine-active through deferred
obligations, including native `__Unwind_Resume`, traps, function fallthroughs,
recursive calls and inconsistent-SP merges. Deferred frontiers prevent complete
bounds. Original maximum frame decrements are preserved.

Each image yields 25 facts and 151 omitted ordinary continuations. Six internal
calls per image remain active as deferred calls. The ordinary-only frame-error
count is zero, while the active frame-error count remains one: the Shape merge.
Its ordinary-CFG explanation is separate from its retained resource refusal.
The [prior read-only diagnosis](../parliament_ballot_shape_merge_20261104/README.md)
and [v4 capture](../parliament_ballot_stack_v4_20261104/README.md) are unchanged.

| Deferred frontier | Portable | Accelerated |
| --- | ---: | ---: |
| Function fallthrough | 36 | 34 |
| Exceptional terminal | 42 | 44 |
| Native edge | 5 | 5 |
| Inconsistent SP merge | 1 | 1 |

These are ordinary-to-deferred classifications, not discharged obligations.
The complete per-function before/after mapping, responsible facts and omitted
call sites are in [portable-v4-comparison.json](portable-v4-comparison.json) and
[accel-v4-comparison.json](accel-v4-comparison.json). Every function is checked
against its exact v4 baseline; all original calls and frontiers are conserved as
multisets with original depths. All five roots per image retain identical
frontiers, positive-weight SCCs and direct resource reachability.

| Selected root | Portable region, bytes | Accelerated region, bytes | Complete bound |
| --- | ---: | ---: | --- |
| `Reservation::verify` | 15,808 | 12,864 | Unknown |
| `workspace::worker` | 16,000 | 13,056 | Unknown |
| `fatal::exhausted` | 144 | 144 | Unknown |
| Registration proof verifier | 9,744 | 6,800 | Unknown |
| Vote proof verifier | 9,744 | 6,800 | Unknown |

These unchanged region figures are ordinary-ABI conditional diagnostics. Native
costs, indirect targets, trap handling, positive recursion, hidden unwind,
exception and signal paths and memory aliases remain unresolved. The fatal
prefix retains its 16-byte frame, modeled 128-byte red zone, native exit and
possible UDF continuation. There is no all-input or whole physical stack bound.

Primary terminal semantics: [Arm DDI 0602, 2025-03](https://documentation-service.arm.com/static/67e40f3398aa3c3b6eea6a85),
printed pages 90 (BRK) and 1056 (UDF). The retained reference identity is in
[arm-reference-download.json](arm-reference-download.json).

The focused suite passes **244 tests**. New controls cover exact ranges and
identity changes, missing/misaligned instructions, malformed terminals, all
rejected control transfers, invalid SP/FP states, recursion and non-propagation,
and active retention of deferred native, trap, fallthrough, recursive and merge
obligations. The positive-stack-loop adversary retains its original merge at
`0x1004` and depth 16 after an omitted continuation. `git diff --check` passes
for the analyzer and tests. The [independent review](independent-review.json)
records the preservation and initial-pinning findings and their fixes.

The [capture driver](capture_retained_stack_v5.py.template) guards reviewed
helper identities, exact parsed receipt bytes, retained binary and LLVM tool
hashes, and every prior capture/diagnosis file before and after. All eight raw
LLVM outputs equal v4 byte for byte. No Rust build, worker execution, timing
campaign or historical-suite rerun was performed. Latest-source qualification
remains separate because the prior review observed rustup/Cargo.lock drift.
The historical 160-node suite remains 47 passed and 113 failed.

[summary.json](summary.json) maps archived bytes to their original cache paths
and hashes. Large mode JSON and disassembly files use lossless deterministic
gzip; exact comparison JSON is directly readable. `SHA256SUMS` covers every file
here except itself. Binary, tool and reference bytes remain in the persistent
cache, which is not permanent release custody. No historical capture was repinned.
