# Dynamic Dispatch Integration Tasks

This document breaks down the work required to integrate a SIMD dispatch layer
for field operations and the Poseidon hash functions. The goal is to detect the
best available CPU features at runtime and route field arithmetic through the
matching implementation transparently.

## Task List

1. **Define a Dispatch Trait**
   - Ensure a `FieldArithmetic` trait abstracts `add`, `sub` and `mul` for
     BN254 field elements.
   - Provide structs for each backend (Scalar, SSE2, AVX2, AVX-512, NEON) and
     implement the trait for them.
   - On non-x86 builds, the SSE2/AVX implementations fall back to the scalar
     code so the library compiles on macOS/ARM.
   - Reference: `src/field_dispatch.rs` already defines this interface and
     uses `OnceLock` to store the chosen implementation.【F:src/field_dispatch.rs†L1-L53】

2. **Implement SIMD Backends**
   - Complete the SSE2, AVX2, AVX-512 and NEON intrinsic routines in
     `src/bn254_vec.rs`. These should match the trait methods and be callable
     safely via `unsafe` blocks where needed.
   - The scalar path serves as the reference implementation.

3. **Runtime Feature Detection**
   - Update `vector::simd_choice()` to detect the host CPU features and return
     a `SimdChoice` enum indicating the best available option.
   - The field dispatch layer reads this choice on first use and caches the
     backend pointer with `OnceLock` so subsequent calls avoid any overhead.

4. **Poseidon Integration**
   - Modify `src/poseidon.rs` so all internal field operations call through the
     `field_impl()` function. This ensures Poseidon permutations automatically
     use the selected SIMD backend without changing the API used by the rest of
     the VM or by Halo2 integrations.

5. **Thread-Safe Initialization and Testing**
   - Use `std::sync::Once` or `OnceLock` to initialize the dispatch exactly once
     even in multi-threaded scenarios. This is partially implemented in
     `field_impl()` but should be reviewed for race conditions.
   - Add unit tests that force each backend (e.g. by mocking `simd_choice`) and
     confirm the correct implementation is invoked. Tests should also ensure two
     threads accessing the dispatch concurrently do not panic or race.

6. **Documentation and Maintenance Guidelines**
   - Document the dispatch design in the repository so future contributors know
     how to add new SIMD implementations. Update `performance_tasks.md` with a
     brief summary and link to this document.
   - Explain how the dispatch integrates with Halo2 and how developers can
     override the detection for benchmarking.

\nThe above tasks are now implemented. SIMD backends are selected by `field_dispatch::field_impl()` using `vector::simd_choice()`.
