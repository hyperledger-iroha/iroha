# Private VM lifecycle cleanup

`IVM::reset` and `IVM::set_zk_mode` return `Result<(), VMError>`. Callers must handle cleanup errors; no infallible aliases are provided. Raw, artifact and prepared program loads propagate the same cleanup error.

The exclusive memory owner validates every tagged HEAP/STACK range before mutation. Ordinary cleanup needs no new write-log allocation: it zeroizes private bytes, updates the existing Merkle dirty and runtime-modified bitmaps, then retires privacy tags and clears private histories. Public bytes are preserved. Private register cleanup updates the register commitment without allocating register-proof events.

When local memory diagnostics are attached, all zero-fill rows are checked and published under one recorder lock before erasure. `PrivateReset` rows contain the original bytes and zero results; they remain local private diagnostic data. Insufficient capacity for the complete private scrub returns the original operational deferral without changing private bytes, tags, registers, mode, gas or PC. Invalid ranges return `PrivacyViolation` before any range is erased. Program loaders may still fail a later CODE/HEAP/OUTPUT diagnostic preflight after successful private cleanup; this boundary does not promise whole-loader rollback.

Runtime-template reset restores memory before resetting transient execution state. Its existing program, lineage, geometry and allocation checks remain mandatory. Core, executor and daemon runtime pools admit a VM only after successful template reset. Shared immutable templates retain their own bytes and tags; cleaning one VM cannot erase a template borrower.

This lifecycle boundary does not establish complete private-witness custody: final-owner destruction of memory/register backing and independently retained trace/diagnostic copies require their own verified cleanup. Production raw-private input and incomplete proof admission remain closed.
