# Kotodama Compiler Error Codes

The Kotodama compiler emits stable error codes so that tooling and CLI users can
quickly understand the cause of a failure. Use `koto explain <code>`
to print the corresponding hint.

The canonical registry lives in `kotodama_lang::diagnostic`; `koto explain`
reads that registry directly, so CLI guidance cannot drift from compiler codes.

| Code range | Phase | Meaning |
|------------|-------|---------|
| `K0000`–`K0100` | lex/budget | Source I/O, size, token count, nesting, diagnostic fanout, or invalid-token failures. |
| `K1001` | parse | The source does not match the V1 grammar. |
| `K2001`–`K2100` | semantic | Duplicate or unknown symbols, type, authorization, state, recursion, or deterministic-profile failures. |
| `K2008` | semantic | A named value type exceeds the 256-level or 250,000-conceptual-expanded-node V1 resolution budget; accepted references reuse one canonical shared graph. |
| `K3001`–`K3099` | lowering | Typed-HIR, SSA-MIR, code generation, or assembly failures. |
| `K4001`–`K4003` | artifact | Compiler-owned header, manifest, deployable artifact construction, or standalone-module build failures. |
| `K5001`–`K5099` | semantic warning | Unified `koto check` lints for unused/unreachable declarations and operations whose access behavior is not statically transparent; `K5099` is the fail-closed adapter for a finding awaiting a dedicated code. |
| `E_ACCESS_INCOMPLETE` | semantic | Static access derivation is incomplete, so runtime scheduling must be conservative. |
| `E_UNBOUNDED_LOOP`, `E_UNBOUNDED_ITERATION` | semantic | A loop or collection traversal lacks a compiler-proven V1 bound. |
| `E_INT_OVERFLOW` | semantic/runtime | Checked arithmetic overflowed; wrapping requires an explicit operation. |
| `E_INTERNAL_BUILTIN` | semantic | Source attempted to use allocation, pointers, raw syscalls, or another compiler-only capability. |
| `E_SECRET_*` | semantic | Secret information-flow policy rejected a public sink, control-flow dependency, or unapproved operation. |

For the precise summary and remediation of any registered code, run:

```sh
koto explain K2003
```
