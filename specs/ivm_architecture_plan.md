# IVM architecture and remaining first-release work

IVM is the single concrete execution engine. Kotodama lives in
`crates/kotodama_lang` and compiles only IVM bytecode. This record describes
implementation boundaries; the [completion goals](kotodama_ivm_completion.md)
own current evidence and the remaining release gates. ABI V1 replaces unfinished
interfaces directly, with no compatibility engine, decoder or calling path.

## Implemented boundaries

- `ivm::runtime` groups `IvmBuilder`, `IvmConfig`, acceleration and stack policies.
  Lifecycle methods remain on concrete `IVM`; no facade trait or second engine is
  required. Hosts own transaction scheduling and publication; the VM has no
  second block scheduler.
- `runtime::SyscallDispatcher` wraps ordinary and shared hosts. `IVM::set_host`,
  default construction install that dispatcher. The interpreter's
  syscall owner enforces admission, deterministic charging and privacy checks;
  typed host adapters validate their pointer envelopes and semantic payloads.
  Custom instrumentation must preserve those checks.
- `kotodama_lang` owns parsing, semantic analysis, IR/lowering, register allocation
  and bytecode emission. `ivm_abi` owns shared calling, schema and syscall contracts;
  artifact admission owns static bytecode/interface validation.
- All compiled calls use bounded caller-owned tables. Cohesive call-frame,
  execution and gas modules enforce the final V1 convention. Core and SDKs consume
  canonical argument records and authenticated exact return counts.
- Runtime acceleration preferences come from `iroha_config` and are passed
  explicitly through configuration APIs. `IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`
  and similar environment controls are debug/test aids; shipping release binaries
  ignore them. Configuration remains operator-visible and defaults enabled.

## Remaining ownership and qualification

Continue separating touched interpreter, compiler and host responsibilities into
cohesive modules inside their existing dependency boundaries. Do not introduce
facade-only crates or duplicate semantics. Keep shared numeric algorithms and
recursive equality identical between constant folding and execution.

Complete allocation admission before VM construction/growth, root and nested
execution, and host scratch. Preserve one original pool through live borrowers,
configuration changes and callback-driven release. Retention and active execution
have separate admission limits; local capacity changes scheduling, never ledger
validity or gas.

Finish automatic target packaging, independent device/kernel qualification,
authenticated public workload cost selection and qualified fallback/quarantine.
The process-owned Metal pipelines and per-kernel completion receipts are component
boundaries, not evidence for every device or fastest-path selection. CUDA release
artifacts still require all ten reproducible PTX families and signed provenance.

Complete the exhaustive State authority codecs, derivation checks, range witnesses
and atomic persistence/recovery root owner before the private execution relation
and anchored AXT cutover. Binding-only proofs plus native replay do not satisfy
that execution relation. Keep production private-input and incomplete remote-spend
gates until sound replacements and every producer/consumer are qualified.

The completion record also tracks Musubi's concrete publication adapters and the
unchanged candidate's language, memory, proof, native/SDK, hardware and mandatory
four-validator DA/RBC evidence. Physical runners and signing/deployment custody
are required inputs; unavailable evidence remains an open gate.
