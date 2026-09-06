# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-38679ba9eb1a575a025153a1dd61678beb0a31dcb313ebc8fd56be5004f84167"></a>

<!-- Original context: Roadmap / Inrou V1 release qualification -->
## Inrou V1 release qualification



<a id="record-d474183f88f17af01d9a0283210f1e75e9dcd5e7844a64c0a3b7dc353db7d596"></a>

- From a clean committed candidate, run the remaining full workspace test and
  strict all-target Clippy matrices, finish maintained SDK/native parity, and
  attach authorized signed provenance. The focused first-release regressions,
  retired-surface scan, deterministic local daemon/CLI builds, and three-pass
  byte-identical OpenAPI replay are complete; the current dirty-tree unsigned
  manifest is development evidence, not a signed release record.


<a id="record-4c749180518939b892da182e11ba0f8cf23b0a481d186b3ecd2b32924be3d9fc"></a>

- On a same-revision Linux/AArch64/KVM host, run the privileged real-guest smoke
  and adversarially qualify the private mount/network/IPC/UTS/PID/cgroup
  launcher and authenticated bounded bridge. Archive kernel-observed CPU,
  memory, swap, pids, I/O, tmpfs, identity, SSH, listener, launch, teardown,
  and escape-negative evidence.


<a id="record-39791e3945962a6dc7d517104a36f2f649ecf85fc6c01928406b737f8efcf891"></a>

- Run the mandatory disposable four-validator Taira corridor with the exact
  four locked Inrou identities, files-only NSS/subid policy, `/dev/kvm` custody,
  pinned ext4 V1 write-lease mount/reuse/recovery and disk reclaim, anonymous
  QMP, fail-secure firewall cleanup, converged finality, and four-replica guest
  workload canary. The current macOS/arm64 host
  cannot supply this Linux/KVM evidence; static/mock orchestration tests are not
  a substitute.


<a id="record-402b1d7bc34b2f4738ee35413a0a6d9d2db57ce78f545e71da4a4ce53b374826"></a>

- Restore authorized public reachability and runtime custody: public HTTPS
  ingress currently returns HTTP 502 for `/status`, `/v1/mcp`, and
  `/api/v1/health`; SSH authentication and owner-private reset/canary inputs
  remain unavailable. Before remote cleanup or mutation, provide the exact
  four-validator inventory, explicit runtime-only authorization, SSH and canary
  inputs, and the trusted compiled dispatcher/reset guard on the admitted host.


<a id="record-f50b77537b7cf5f7adabc9164970b976766e97296470935a5315902b26299aa6"></a>

- With those gates satisfied, build the same-revision `iroha` evidence binary
  using `--profile release`, run only `iroha taira public-reset preflight` and
  `iroha taira public-reset apply`, and archive cleanup and reset evidence. Then
  require four-peer convergence, all seven exact child outcomes, same-revision
  doctor evidence, four distinct Inrou receipts, bounded restart proof, guest
  workload qualification, and controlled edge cutover before calling public
  Taira ready.



<a id="record-9c6eed8499b14f862d833c15c3fd3c3514f58e9898508a82c76c38278e471ff7"></a>

<!-- Original context: Roadmap / SoraNet first-release security qualification -->
## SoraNet first-release security qualification



<a id="record-5f4a46cc30e051a08acda59a5fe47523dfd279356c7ba52beaa826653fbb8f0c"></a>

- Exercise stateless VPN quotes across Torii restart and load-balanced nodes,
  then saturate the optional process session cache and prove an exact paid,
  active WSV lease still creates the same canonical session and helper ticket.


<a id="record-c6413cf4304533819abeedda0ae8534ae081485d86b2bfb13d9a765af483daa1"></a>

- Exercise the hardened relay, puzzle, DNS, VPN backend, and privileged helper
  boundaries on Linux with real TUN devices, pidfds, owner-private runtime
  secrets, DNS rollback, process replacement, partial writes, and hostile local
  peers. Archive multi-relay loss/replay/rotation evidence and resource-limit
  telemetry from the release build.


<a id="record-19cc30ba3ed7ae1979472197fea921a9974a9d68a43db604d39e44779f1e90ef"></a>

- Run the full workspace test and strict all-target Clippy matrices, extended
  handshake/record/FFI fuzzing, and independent cryptographic and deployment
  review from the immutable release source. Focused security suites are green;
  they do not substitute for this release qualification.



<a id="record-489ea8b4cf41a1814eb82bc195900df281a3db6d25557380e4bc9a2513569b0d"></a>

<!-- Original context: Roadmap / Disposable Taira-compatible development networks -->
- Move the last trusted-Inrou guest binding out of Python's small textual TOML
  append and into a typed Rust config writer that consumes the staged artifact
  identity. Kagami now owns storage and egress directly, so do not restore the
  removed generic-localnet overlay or its invalid guest-less intermediate.


<a id="record-63b3d62d82b2d25d4dba35a7479abd5832e7024c697eda65b6feabd5a48fc67a"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Lane relay admission now reports missing dataspace catalog entries as
  `unknown_dataspace` instead of folding them into validator-roster failures,
  keeping operator diagnostics and telemetry aligned with routing/catalog drift.
  Emergency override registration coverage now also pins the current commit
  topology boundary: registered peers with live consensus keys but absent from
  the transaction's current commit topology are rejected before any override row
  is stored, and stale stored overrides cannot fill runtime relay committees
  with peers that have since fallen out of the current topology or whose
  consensus keys have expired by the relay height, and removed world peers
  remain ineligible even if stale keys or topology entries survive.

