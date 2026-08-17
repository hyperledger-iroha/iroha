# Kotodama Compiler Error Codes

The Kotodama compiler emits stable error codes so that tooling and CLI users can
quickly understand the cause of a failure. Use `koto explain <code>`
to print the corresponding hint.

The canonical registry data lives in the versioned
`kotodama_lang/src/assets/diagnostics_v1/diagnostic_explanations_v1.tsv` asset;
the build emits the same public `kotodama_lang::diagnostic` registry, which
`koto explain` reads directly so CLI guidance cannot drift from compiler codes.
The typed linker, shared build driver, `koto build`, and `musubi build` carry the
same `DiagnosticBundle` through human, JSON, and SARIF rendering. Module-call
and ambiguous-export diagnostics therefore retain exact primary and related
source spans instead of flattening the link failure into text.

| Code range | Phase | Meaning |
|------------|-------|---------|
| `K0000`–`K0100` | lex/budget | Source I/O, size, token count, nesting, diagnostic fanout, or invalid-token failures. |
| `K1001`, `K1004` | parse | The source does not match the V1 grammar, or a source graph exceeds its frontend budget. |
| `K2002`, `K2099` | resolve | A name cannot be bound, or resolved-HIR integrity validation failed. |
| `K2001`, `K2003`–`K2098`, `K2100` | semantic | Type, authorization, state, recursion, ABI-representation, or deterministic-profile failures. |
| `K2008` | semantic | A named value type exceeds the 256-level or 250,000-conceptual-expanded-node V1 resolution budget; accepted references reuse one canonical shared graph. |
| `K3001`–`K3099` | lowering | Typed-HIR, SSA-MIR, code generation, or assembly failures. |
| `K4001`–`K4003` | artifact | Compiler-owned header, manifest, deployable artifact construction, or standalone-module build failures. |
| `K5001`–`K5099` | semantic warning | Unified `koto check` lints for unused/unreachable declarations and operations whose access behavior is not statically transparent; `K5099` is the fail-closed adapter for a finding awaiting a dedicated code. |
| `E_PACKAGE_BUDGET` | parse | A typed-module package graph exceeds its fixed source-count or aggregate-byte budget. |
| `E_MULTIPLE_SEIYAKU_ROOTS` | resolve | One diagnostics request supplied more than one deployable `seiyaku`/`誓約` root; every root and additional declaration is spanned. |
| `E_PROJECT_MANIFEST_REQUIRED`, `E_PROJECT_MANIFEST` | resolve | Module linking lacked an explicit versioned graph, or that graph was malformed/unsafe. Positional source order never grants import/export authority. |
| `E_ROOT_*`, `E_DEPENDENCY_*`, `E_*PACKAGE*`, `E_*MODULE*`, `E_*IMPORT*`, `E_*EXPORT*` | resolve | Typed-module root, dependency, package, module, import, and export resolution failures. |
| `E_DUPLICATE_DECLARATION`, `E_RESERVED_DECLARATION`, `E_LOCAL_SHADOWING` | resolve | A declaration or binding is duplicate, reserved, or shadows another visible symbol. |
| `E_INVALID_SOURCE_PATH`, `E_DUPLICATE_SOURCE`, `E_DUPLICATE_SOURCE_ID`, `E_DUPLICATE_HIR_ID` | resolve | A normalized source graph or compiler-owned identity is invalid or ambiguous. |
| `E_INTERNAL_RESOLUTION` | resolve | Resolved-HIR metadata is incomplete or inconsistent when typed analysis consumes it. |
| `E_ACCESS_INCOMPLETE` | semantic | Static access derivation is incomplete, so runtime scheduling must be conservative. |
| `E_UNBOUNDED_LOOP`, `E_UNBOUNDED_ITERATION` | semantic | A loop or collection traversal lacks a compiler-proven V1 bound. |
| `E_INT_OVERFLOW` | semantic/runtime | Checked arithmetic overflowed; wrapping requires an explicit operation. |
| `E_INTERNAL_BUILTIN` | semantic | Source attempted to use allocation, pointers, raw syscalls, or another compiler-only capability. |
| `E_SECRET_*` | semantic | Secret information-flow policy rejected a public sink, control-flow dependency, or unapproved operation. |

For the precise summary and remediation of any registered code, run:

```sh
koto explain K2002
```
