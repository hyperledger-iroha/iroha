# Acceleration

The `[accel]` section controls optional hardware acceleration for IVM and helpers. All
accelerated paths have deterministic CPU fallbacks; if a backend fails a golden
self‑test at runtime it is disabled automatically and execution continues on CPU.

- `enable_cuda` (default: true) – Use CUDA when compiled and available.
- `enable_metal` (default: true) – Use Metal on macOS when available.
- `max_gpus` (default: 0) – Maximum GPUs to initialize; `0` means auto/no cap.
- `merkle_min_leaves_gpu` (default: 8192) – Minimum leaves to offload Merkle
  leaf hashing to GPU. Lower only for unusually fast GPUs.
- Advanced (optional; usually inherit sensible defaults):
  - `merkle_min_leaves_metal` (default: inherit `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (default: inherit `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (default: 32768) – Prefer CPU SHA‑2 up to this many leaves on ARMv8 with SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (default: 32768) – Prefer CPU SHA‑NI up to this many leaves on x86/x86_64.

Notes
- Determinism first: acceleration never changes observable outputs; backends
  run golden tests on init and fall back to scalar/SIMD when mismatches are detected.
- Configure via `iroha_config`; avoid environment variables in production.

