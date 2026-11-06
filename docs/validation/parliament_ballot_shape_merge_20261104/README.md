# Parliament Shape stack-merge diagnosis

The remaining `Shape::new_as` inconsistent-SP merge is a conservative analyzer
precision limit: the analyzer resumes the caller after a direct callee whose
retained ordinary CFG has no return path. It is not caused by missing
instructions, an incorrect function boundary, or a variable-sized allocation.
This diagnosis does not change the analyzer or any stack-v4 result. All complete
physical stack bounds remain unknown.

The [diagnostic receipt](diagnosis.json) contains exact paths, original function
results, a minimal reproduction and before/after hashes for all 35 stack-v4
cache/archive files. They were unchanged. The diagnostic reads only retained
disassembly and evaluates a small synthetic CFG using the existing analyzer;
it executes no worker, Rust build, benchmark or pytest suite.

## Exact retained paths

Both images contain the same 93-instruction function. Every instruction is
present and visited, and the boundaries agree with `LC_FUNCTION_STARTS`.

| Item | Portable address | Accelerated address |
| --- | --- | --- |
| `Shape::new_as` entry | `0x100015abc` | `0x10001a65c` |
| Next function start | `0x100015c30` | `0x10001a7d0` |
| Ordinary branch into shared block | `0x100015bf0` | `0x10001a790` |
| Fixed 16-byte save | `0x100015c00` | `0x10001a7a0` |
| Call to panic helper | `0x100015c10` | `0x10001a7b0` |
| Shared block / reported merge | `0x100015c14` | `0x10001a7b4` |
| Direct callee entry | `0x10005a228` | `0x1000584a8` |
| Callee's terminal instruction | `0x10005a244` | `0x1000584c4` |

One branch enters the shared block with SP equal to its entry value. The panic
path executes `stp x29, x30, [sp, #-0x10]!`, sets FP and calls
`panic_const_rem_by_zero`. The analyzer assumes ordinary calls preserve SP/FP
and may return, so it also queues the immediately following shared block with
SP at entry minus 16. The two offsets trigger the existing fail-closed merge
rule. Its generic “loop/branch bound unproved” message does not establish that
this site contains a stack-allocation loop.

The only SP operands in this function are that fixed save and `mov x29, sp`.
See the exact [portable](portable-shape.txt) and
[accelerated](accel-shape.txt) instruction excerpts and the per-path traces in
the receipt.

## Why the continuation is conservative

The direct callee's first eight instructions are a fixed save, register/address
preparation, `bl panic_fmt`, and `udf #0x0`. There is no conditional path,
ordinary return, indirect transfer or tail exit before that trap. Even if the
child call returns normally, the next instruction traps. This conclusion comes
from the exact [portable](portable-terminal-callee.txt) and
[accelerated](accel-terminal-callee.txt) instructions, without inferring
non-return from a Rust symbol name or an unavailable historical source file.

Within the analyzer's ordinary-CFG assumptions, this callee cannot return to
the shared caller block. Native costs, trap handling, nonlocal unwinding,
signals and recursive panic behavior remain separate unresolved obligations.
The callee and its descendants must not be assigned zero cost or removed from
the resource graph.

## Minimal reproduction and next step

The diagnostic runs the following neutral-name fixture through the unchanged
analyzer. The result retains one inconsistent-SP merge, one native edge and one
exceptional terminal, with a `null` complete bound:

```text
fixture_root:
  1000: cbz w0, 0x1010
  1004: stp x29, x30, [sp, #-16]!
  1008: mov x29, sp
  100c: bl 0x2000
  1010: ret
fixture_terminal:
  2000: bl 0x3000       # opaque native endpoint
  2004: udf #0
```

A possible next step is a separately reviewed function-local
*no-ordinary-return* fact derived from exact CFG terminal instructions. Only a
proved fact could suppress the caller's continuation. The direct callee edge
and every native, trap, indirect, recursive and hidden-unwind limitation would
remain. Unknown targets, missing instructions, an actual return on any path,
or an unproved outward transfer must prevent that fact. Negative controls must
cover each case; a symbol-name allowlist or guessed native cost is insufficient.

No such returnability mechanism, state-lattice rewrite, source-bound
unreachability assumption or new bound is implemented here. Reproduce this
diagnosis with the retained [driver](diagnose_shape_merge.py.template), pointing
`--root` at this checkout, `--cache` at the Parliament persistent cache, and
`--output` at a new directory. `SHA256SUMS` covers every artifact here except
itself.
