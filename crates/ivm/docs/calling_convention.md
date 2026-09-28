# Kotodama Calling Convention and Register Allocation

This document describes the first-release calling convention used by the
Kotodama compiler and the register allocation strategy implemented in
`crates/kotodama_lang/src/regalloc.rs`.

## Argument and result tables

Every Kotodama function uses the authenticated V1 word-table convention in
`ivm_abi::call`. Each eight-byte-aligned table contains at most 8,192 words
(64 KiB). The callable metadata binds its instruction entry, frame size, exact
argument count, exact result count, and representation/privacy role for every
slot. Tuple and struct fields flatten in declaration order; Option, Result,
List, cursor, and typed pointer values each occupy one handle word. Unit is one
public zero word. An empty nominal struct also occupies one public zero word;
its schema identity remains distinct from Unit. There is no register-window
call convention.

On entry, `r10` identifies the argument table, `r11` is its exact word count,
`r12` identifies the caller-owned result table, and `r13` is its exact capacity.
An empty argument table uses address zero and count zero. Argument and result
tables must be disjoint. On return, `r10` identifies the same result allocation
and `r11` reports its exact initialized count; the runtime checks both against
its protected frame state.

The host selects an authenticated public entrypoint and prepares root tables in
owned HEAP storage from the canonical schema-bound argument record. Guest
register descriptors cannot replace this host authority. Internal calls place
both outgoing tables in the immediate caller's live stack frame; the argument
bytes must already be initialized. A callee may read its argument table and
write its result table, but cannot write arguments, read the result table, or
access unrelated ancestor frames. Its own stack reads also require initialized
bytes. The memory owner tracks successful writes, and return validation requires
every result byte to have been written, the exact declared roles and privacy
tags, and a restored stack pointer.

A successful root return publishes interpreter-owned completion state. Core
reads `IVM::call_result_word_count()` and `IVM::public_call_result_word(index)`;
it checks the exact signed return-schema width before collecting any payload.
An unfinished or failed call has no return table to decode. Later changes to
`r10`/`r11` cannot redirect the collector. Private result slots and private
pointed payloads fail the public boundary; diagnostics identify their owning
zero-based table word, including values nested behind a sum or list handle.

The test-only nested entrypoint invocation syscall stages actor, selector and
payload in `r10`, `r11` and `r12`; `r13` is an owned result-table address and `r14`
is its exact capacity. The host copies the completed child results according to
the authenticated return schema, then publishes the table base/count in
`r10`/`r11`. It uses the same 8,192-word bound and has no pointer mask or register
result window. Compiler-owned test sidecars authenticate their real callable
roots; successful tests return to the runtime-owned end-of-code target.

Syscalls retain their individual register conventions. Their use of `r10+` does
not define a function-call value window; see [`syscalls.md`](syscalls.md).

## Register usage

- **Call descriptors:** `r10`–`r13` hold the table descriptors described above.
- **Stack pointer:** `r31` grows downward.
- **Link register:** `r1` carries the architectural return address of direct calls.
- **Frame pointer:** `r30` is reserved as an optional frame pointer.
- **Caller saved:** the allocation pool `r10`–`r22` may be overwritten by calls,
  syscalls, and fixed-register operations. A non-leaf callee saves `r1`.
- **Callee saved allocation pool:** `r2`–`r9` and `r23`–`r24` are preserved by
  callees that use them.
- **Compiler scratch:** `r25`–`r26` serve arithmetic/literal sequences, while
  `r27`–`r29` serve spill shuttling and wide frame addresses.
- **`r0`:** reads as zero and is never allocated.

The allocator uses the first 32 of the VM's 256 registers. This allocation
choice does not limit the size of an argument or result table.

## Stack frame layout

Every function reserves saved argument/result base slots. The frame also owns
its spills, used callee-saved registers, optional nested-call return address,
aggregate-state scratch, and maximum outgoing argument/result tables. The
complete reservation is aligned to 16 bytes, reported as `frame_bytes`, and
bound by authenticated callable metadata. Even a call-free identity leaf saves
its table bases and reads its parameter from the argument table.

The prologue decrements the stack pointer by this fixed reservation; the
epilogue restores it. All offsets are relative to the post-prologue pointer.
Eight-byte spill slots are reused only for non-overlapping live intervals.
Tables and preserved values remain inside the owning frame throughout a call.

## Return-address integrity

Deployable Kotodama artifacts execute with a protected return stack inaccessible
to bytecode. Direct `JAL` linking `r1`, or `JALS`, records its fallthrough PC.
The indirect return `JALR r0, r1, 0` must match the protected top entry before
returning. Its table ownership, initialized results, role tags, and restored
stack pointer must also pass validation. Corrupting a saved architectural `r1`
therefore traps instead of redirecting execution.

Invocation setup installs the host-owned end-of-code return sentinel for the
public root. Internal callable roots cannot be selected as production public
entrypoints. Call state is cleared on reset, program replacement, a new
invocation, and failure. Protected nested-call depth is bounded at 1,024;
Kotodama V1 rejects source recursion.

The assembler relaxes calls to `JAL` or `JALS`, using direct `JMP` trampoline
islands for exceptionally large images. Trampolines preserve the original link
so a source-level call has one protected return entry.

## Register Allocation

The allocator computes deterministic live intervals from CFG liveness,
including backedges and loop-carried values, then performs linear scan in
deterministic position/temporary order. Register classes are selected for each
interval rather than once for the whole function. Values that cross an internal
call or host-ABI clobber use the callee-saved pool or a spill slot. Values born
after, consumed by, or confined between clobbers prefer `r10`–`r22`. Host-call
operands remain in preserved homes until their multi-step ABI staging is
complete. Entry `LoadVar` temporaries load from their declared argument-table
slots into the selected register or spill home.

Internal-call operands are stored in outgoing argument-table slots before the
callee starts; returned values are loaded from the caller-owned result table
after successful completion. Syscall register staging uses parallel assignments
and a reserved scratch register to break cycles. Only callee-saved registers
actually used by a function are saved and restored.

When peak pressure exceeds the selected pool, the temporary receives a stable
eight-byte stack home. Stack-slot colouring reuses a physical slot for disjoint
full intervals. Code generation writes every definition of a spilled
temporary to that canonical home.

The compiler then performs deterministic live-interval splitting as a
second-chance pass. Repeated runtime uses of an initially spilled temporary are
grouped into short, position-indexed segments within one basic block, one
definition epoch, and one ABI-clobber region. A segment may occupy only a
register hole that does not overlap a normal home interval or an earlier split.
It never evicts an allocated home. Clobber-local segments prefer caller-saved
holes; other segments reuse only a callee-saved register the function already
preserves, so the optimization cannot introduce new prologue/epilogue traffic.

At the first use position, code generation reloads the canonical stack home
once; every use through the end of that segment reads the same physical
register. Split segments are deliberately read-only. They require no store on
exit, never cross a definition, and reload independently after CFG joins and on
each loop re-entry. This reduces repeated spill traffic without edge copies or
path-dependent allocation state and keeps emitted bytecode deterministic.

Returns are written into the saved caller-owned result table before publishing
its address and exact count in `r10`/`r11`. Source functions return through the
protected `r1` convention; a validated root return completes the invocation.
