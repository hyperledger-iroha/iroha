# Kotodama Calling Convention and Register Allocation

This document describes the provisional calling convention used by the Kotodama compiler and the basic register allocation strategy implemented in `src/kotodama/regalloc.rs`.

## Register Usage

- **Argument registers:** `r10`–`r17` are used to pass up to eight function arguments.
- **Return register:** `r10` carries the first return value.
- **Return value:** `r10` is also used for the first return value.
- **Stack pointer:** `r31` acts as the stack pointer (grows downward).
- **Frame pointer:** `r30` is reserved as an optional frame pointer.
- **Caller saved:** `r1`–`r15` (except the argument registers in use) are treated as caller saved.
- **Callee saved:** `r18`–`r29` must be preserved by the callee.
- **`r0`:** reads as zero and is never allocated.

Only the first 32 registers are currently used to keep the encoding simple. Future updates may extend the allocator to the full 9‑bit register space.

## Stack Frame Layout

Every function may allocate a stack frame to spill temporaries and save callee saved registers. The frame is aligned to 16 bytes. Offsets are measured relative to the value of the stack pointer after the prologue.

```
| higher addresses ...           |
| saved callee registers         |
| spilled temporaries / locals   |
| return address (optional)      |
<- sp after prologue             |
```

The stack pointer is decremented by the total frame size in the prologue and restored in the epilogue. When a frame pointer is used, `r30` is set to the value of the stack pointer after allocation to allow stable addressing of locals.

## Register Allocation

The current allocator assigns each IR temporary to a distinct register until `r29` is reached. Further temporaries are spilled to the stack in 8‑byte slots. The resulting mapping and frame size are returned from `allocate()` so the code generator can emit prologue/epilogue sequences. When a function returns a value, codegen moves it into `r10` before emitting `HALT` (temporary single‑function ABI).
