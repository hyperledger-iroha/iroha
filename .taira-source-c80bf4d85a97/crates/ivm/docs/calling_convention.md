# Kotodama Calling Convention and Register Allocation

This document describes the first-release calling convention used by the
Kotodama compiler and the register allocation strategy implemented in
`crates/kotodama_lang/src/regalloc.rs`.

## Register Usage

- **Argument registers:** `r10`–`r22` pass up to thirteen flattened argument
  words.
- **Return registers:** `r10`–`r22` carry up to thirteen flattened return words;
  `r10` is the first return value.
- **Stack pointer:** `r31` acts as the stack pointer (grows downward).
- **Link register:** `r1` carries the architectural return address written by direct calls.
- **Frame pointer:** `r30` is reserved as an optional frame pointer.
- **Caller saved:** the argument/return window `r10`–`r22` may be overwritten by
  a call or host ABI operation. `r1` is overwritten by a linking call and is
  saved by a non-leaf callee.
- **Callee saved allocation pool:** `r2`–`r9` and `r23`–`r24` are preserved by
  callees that use them.
- **Compiler scratch:** `r25`–`r26` are reserved for checked-arithmetic and
  literal-materialization sequences; `r27`–`r29` are reserved for spill
  shuttling and wide frame addresses.
- **`r0`:** reads as zero and is never allocated.

Only the first 32 registers are currently used to keep the encoding simple. Future updates may extend the allocator to the full 9‑bit register space.

## Stack Frame Layout

Functions allocate a frame only when they spill, preserve callee-saved
registers, save a nested-call return address, or need aggregate-state scratch
space. The complete frame is aligned to 16 bytes after accounting for the
return address, spills, saved registers, aggregate-state scratch, and final
padding. Compile reports include that padding in `frame_bytes`. Offsets are
measured relative to the value of the stack pointer after the prologue.

```
| higher addresses ...           |
| aggregate-state scratch        |
| saved callee registers in use  |
| spilled temporaries            |
| return address (optional)      |
<- sp after prologue             |
```

The stack pointer is decremented by the total frame size in the prologue and
restored in the epilogue. Spill slots are eight bytes and are reused when their
complete live intervals do not overlap. A call-free identity leaf therefore
uses its precoloured argument register directly and emits no frame or stack
traffic.

## Return-address integrity

Deployable Kotodama artifacts execute with a protected return stack that is not
addressable by contract bytecode. A direct `JAL` linking `r1`, or `JALS`, pushes
its fallthrough PC. The only permitted indirect return is
`JALR r0, r1, 0`, and its target must equal the protected top entry before that
entry is popped. The architectural `r1` save/restore in a non-leaf stack frame
remains part of the calling convention, but corrupting that memory can only
cause an `AssertionFailed` trap; it cannot redirect execution.

The protected stack is cleared for each invocation and whenever a warmed VM is
reset. Its deterministic maximum depth is 1,024. Kotodama V1 rejects recursion,
so reaching the bound indicates malformed or adversarial bytecode. A directly
selected internal entrypoint begins with an empty protected stack. At invocation
start the VM captures the aligned value of `r1`; the outer return must match that
captured value exactly, and the value must identify an appended `HALT` or the
host-provided end-of-code sentinel. Changing `r1` to a different valid `HALT`
therefore traps instead of skipping intervening contract code.

The Kotodama assembler relaxes ordinary calls to one-word `JAL` or `JALS`.
Exceptionally large code images use direct-`JMP` trampoline islands between the
linking call and its callee. Trampoline hops never relink `r1`, so the protected
return stack still contains exactly one frame for the source-level call.

## Register Allocation

The allocator computes deterministic live intervals from CFG liveness,
including backedges and loop-carried values, then performs linear scan in
deterministic position/temporary order. Register classes are selected for each
interval rather than once for the whole function. Values that cross an internal
call or host-ABI clobber use the callee-saved pool or a spill slot. Values born
after, consumed by, or confined between clobbers prefer `r10`–`r22`. Host-call
operands remain in preserved homes until their multi-step ABI staging is
complete. Entry `LoadVar` temporaries are precoloured to their incoming ABI
register only when their complete interval is safe there.

Internal-call arguments and multi-value returns are parallel assignments.
Code generation emits acyclic moves first and breaks register cycles through a
reserved scratch register, so reordered parameters and tuple returns cannot
overwrite a value that has not been copied yet. A call-local interval can
therefore occupy the full argument window without adding callee-save traffic.
Only callee-saved registers actually used by a function are saved and restored;
a simple identity leaf still emits no frame.

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

Returns are moved into `r10`–`r22` as needed. Source functions return through
the protected `r1` convention; compiler-owned public-entry wrappers terminate
the outer invocation with `HALT`.
