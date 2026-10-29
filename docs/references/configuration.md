# Runtime configuration

## Node essentials

A node file needs only `chain`, `public_key`, `trusted_peers`, `trusted_peers_pop`,
`network.address`, `network.public_address`, `torii.address` and `[genesis]`, plus
`data_dir`, plus any consensus-bound settings in `nexus`, `gov`, `oracle`, `content`
and `settlement` that the network's genesis was signed
with. `kagami node init` writes the per-node part and takes the rest from `network.toml`
with `extends`; `kagami network create` writes that shared file for the collected
validator roster. Everything else has a working default:

- `data_dir` completes every state path (`kura`, `snapshot`, `tiered_state`, Torii and
  its DA spools, SoraFS, SoraNet revocations, streaming sessions, Soracloud) under
  `<data_dir>/state/` and reads `validator.key`, `transport.key` and `streaming.key`
  from `<data_dir>/secrets/`; the transport and streaming public keys derive from
  those private keys.
- Socket addresses are plain `host:port`, `ip:port` or `[ipv6]:port`; the checked
  `addr:host:port#CRC` literal that tools emit is also accepted and still verified.
- `streaming.codec.rans_tables_path` is optional. Omitted, the node uses the canonical
  `SignedRansTablesV1` artefact (`codec/rans/tables/rans_seed0.toml`) compiled into
  `iroha3d`, so no table file has to ship with the node.
- Omitted governance and `network.soranet_vpn.operator_account_id` accounts keep their
  default accounts for any `chain_discriminant`. An explicitly configured account
  literal must use the node's prefix.
- Without `nexus.storage.local_budget_bytes`, the node derives its storage budget from
  the free space of each storage filesystem minus a headroom of 20% of its capacity,
  capped at 64 GiB, and refuses to start when nothing remains.

## Native execution-proof transport

`network.max_frame_bytes_connect` sets the dedicated reliable Connect P2P topic's
encoded frame bound (default 128 KiB). It is independent of the Health topic bound.
The optional `configs/soranexus/execution-proof-transport.toml` overlay configures
8 MiB Connect and transaction-gossip frames, 4 MiB + 4,096 byte Connect WebSocket
messages, and bounded session buffering/counts for execution-proof transport
qualification. Merge it with a complete reviewed runtime configuration and apply
compatible limits to participating peers, proxies and wallets. The overlay does
not qualify proof profiles or modify consensus settlement rules.

## Hardware acceleration

The `[accel]` section controls optional hardware acceleration for IVM and helpers. All
accelerated paths have deterministic CPU fallbacks; if a backend fails a golden
self‑test at runtime it is disabled automatically and execution continues on CPU.

- `enable_cuda` (default: true) – Use CUDA when compiled and available.
- `enable_metal` (default: true) – Use Metal on macOS when available.
- `max_gpus` (optional) – IVM device-selection cap; omission uses the available qualified devices and `0` explicitly opts out.
- `merkle_min_leaves_gpu` (default: 8192) – Minimum leaves to offload Merkle
  leaf hashing to GPU. Lower only for unusually fast GPUs.
- Advanced (optional; usually inherit sensible defaults):
  - `merkle_min_leaves_metal` (default: inherit `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (default: inherit `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (default: 32768) – Prefer CPU SHA‑2 up to this many leaves on ARMv8 with SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (default: 32768) – Prefer CPU SHA‑NI up to this many leaves on x86/x86_64.
  - Omitted optional thresholds inherit the stated default; explicit zero remains zero.
- `[accel.resource_limits]` supplies finite process attempt limits. Defaults are 256 MiB ordinary host, 256 MiB pinned host, 1 GiB requested device backing, 16 concurrent work owners, 16 MiB variable metadata, 16 observed device records, 64 discovery probes, 304 modules, 16 streams, and 16 MiB per artifact. Zero denies admission of that resource. These limits are distinct from caller-owned State execution memory.

Notes
- Determinism first: acceleration never changes observable outputs; backends
  run golden tests on init and fall back to scalar/SIMD when mismatches are detected.
- Configure via `iroha_config`; avoid environment variables in production.
