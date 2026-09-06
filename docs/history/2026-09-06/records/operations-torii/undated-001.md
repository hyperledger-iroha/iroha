# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-bb62a9c10c6414df9abac2fdd5ac35ac308aef6d395bcb406bad05bd651ea9ec"></a>

<!-- Original context: Roadmap / Security-audit release qualification -->
## Security-audit release qualification



<a id="record-a1ae51f88e8611bde343ca2d186323a6801f9b72094f428de0ef62e519af89dd"></a>

- Settle the 21 source-level dispositions in
  [`docs/audit_closeout_matrix.md`](../../../../audit_closeout_matrix.md) into one
  immutable candidate. Rerun every named focused regression from that matrix,
  the panic-recovery and release-feature source guards, formatting, strict
  all-target Clippy, and the workspace test gate. Record blocked, timed-out, or
  deferred commands as non-passes.


<a id="record-ad8cff81d4d30c072bb375f2b1383c71a8f748da37c3f7ec4f49326af23b93a0"></a>

- Before sealing that candidate, require the panic/source guard to reject macro
  and thread aliases, custom Cargo-root module escapes, symlink indirection,
  Python import shadowing, and mixed-read inventory races. Any canonical
  prebuilt binary must be copied from a provenance-authenticated snapshot bound
  to its reviewed manifest digest, source commit, lockfile hash, target, release
  profile, feature set, package mapping, executable bit, size, and content hash.


<a id="record-c9d05fc86026f49d222a0f3af716d9417dd4e7e8f42c09e96e8bbd8ac75d1716"></a>

- From that same candidate, regenerate and compare the route catalog, served
  OpenAPI mirrors, fixtures, and Rust/CLI, Kotlin, mirrored Java, Swift, Python,
  and JavaScript projections. Keep `Cargo.lock` unchanged and do not promote a
  dirty-tree generated artifact as release evidence. Before qualification,
  require the root and IVM fuzz lock graphs to agree with their manifests; the
  security-remediation lane must not normalize either lockfile on behalf of an
  unrelated dependency change.


<a id="record-a916bb97459f36b393d5880d30fcd1a653a44e48e35565fd540f0756f7858fa6"></a>

- Validate the four affected public-documentation routes in English and all 20
  maintained translations (21 locale routes) in the sibling `iroha-docs`
  repository. Publish contract lifecycle, data-trigger scope/capacity,
  moderation-challenge economics and settlement, Torii
  visibility/cursor/full-block restrictions, ISO participant and replay rules,
  public local SoraFS gateways, and relay verifier-roster operations only from
  the release-qualified candidate.



<a id="record-6d2d0f9a90238235a87760f3c8ba31eebafeca4b9e3b51e23107303d62931f32"></a>

<!-- Original context: Roadmap / Telemetry first-release closure -->
## Telemetry first-release closure



<a id="record-756425326ae8abc4c9bcb6d337e58e16a98a1917e0c96992bb42955168ecc23a"></a>

- Qualify integrity journaling and optional-exporter supervision under repeated
  process crashes, collector half-closes, full disks, and checkpoint permission
  failures on every shipping host platform. Preserve exact-record retry and
  keep exporter failure isolated from validator liveness.


<a id="record-377b866aa095a5b418c99a1db1cfcf65e5b1cbf114518d715789d6e43d288a90"></a>

- Benchmark status encoding, Prometheus gathering, logger field filtering, and
  exporter persistence/reconnect paths. Add allocation and latency ceilings
  before introducing caching or concurrency, while preserving deterministic
  payloads and bounded memory.



<a id="record-942a5982e3c8d37f0e54ab3fddb54b3eae2a915e6ec6018dc070e7f05a9244ee"></a>

<!-- Original context: Roadmap / Mochi first-release closure -->
## Mochi first-release closure



<a id="record-5c6955a79788bf36b79cbbd912856d45c46909984ca69aaa275187ea1f89cdec"></a>

- Qualify owner-only vault/bootstrap custody and atomic publication on every
  shipping host platform. Keep platforms without equivalent no-follow,
  ownership/ACL, single-link, and directory-durability guarantees fail closed.
  Replace pathname revalidation/publication with descriptor-relative no-follow
  traversal before claiming resistance to concurrent ancestor replacement.


<a id="record-de1652a64f0328b4e1e563341ce6cb9e0f077eb442b2a8d20008fd903b2cab22"></a>

- Benchmark dashboard refresh, bounded Explorer decoding, one-pass snapshot
  inventory/copy/rehash, restore recovery, readiness smoke, and supervisor
  startup. Preserve deterministic output and add explicit allocation/latency
  budgets before introducing caches or wider concurrency.



<a id="record-46b04aeb6273d60e87594bc65bb4fac8ce1b6d151165f7c5675ac13d8200c63f"></a>

<!-- Original context: Roadmap / Iroha configuration first-release closure -->
## Iroha configuration first-release closure



<a id="record-2bcb4588ee2ddb2ce5d36ab93bcab6ce9527e84ab627eb09f84b594570c39ffc"></a>

- Split the large user/actual parameter modules along existing ownership and
  validation boundaries. Keep a single definition of each first-release
  label, default, and constraint, and add production-source reachability guards
  so aliases, duplicate serializers, ambient-environment reads, and unowned
  config/DTO/metric fields cannot return.


<a id="record-9c38d61b1d6b65167fc3d373373c0184e26fe1b550df4064a7c9c8a20584c5ae"></a>

<!-- Original context: Roadmap / Torii first-release closure -->
## Torii first-release closure



<a id="record-51c8f28d8ddae5f0285344e5cb3a7168cb6627710faa92cac4ad80a6c11770a8"></a>

- Remove Torii's crate-wide `dead_code`, `unused_imports`, and `unused_async`
  allowances. Delete or feature-gate every resulting production-only finding,
  and replace trait-check helper functions with compile-time assertions that do
  not need dead-code exemptions.


<a id="record-1413ff9457cc887f65508d67c9d982b5d07d4b8c18ce68264d7e8f528f998f28"></a>

- Audit the remaining one-way configuration parser spellings against
  first-release deployment owners. Delete aliases with no production owner
  instead of carrying compatibility branches.



<a id="record-99384def184ef5fcc74be96cf3df2c8b4c7e71fe80d3e0e24ed576ed25f79a3f"></a>

<!-- Original context: Roadmap / First-release hard-cut closeout -->
## First-release hard-cut closeout

The current candidate has one canonical release profile and no `BuildLine` or
top-level Nexus enablement selector. Nexus routing is mandatory for both the
canonical single-lane catalog and custom multilane catalogs; optional elastic
capacity is controlled only by the subordinate `nexus.autoscale.enabled`
policy. The six-field lifecycle status and byte-identical OpenAPI mirrors are
recorded in `status.md`. For this hard-cut tranche, remaining work is limited
to:



<a id="record-abdc16eb2fd16e48fe920c359783f4daa453d680b52efc74e23c93496669b5c1"></a>

- Close the remaining aggregate source objective without raising the currently
  reconciled per-file ratchets merely to absorb merge growth.


<a id="record-8d0e43285a4c348c8b50608097a8cceb3ce78f717f5b5bdd3b26829d282cb4cd"></a>

- Regenerate the exact context/genesis goldens from that candidate and attach
  the required authorized signatures, including clean OpenAPI provenance.


<a id="record-3fb13f389bd11f48f503cd11dfb39eb750103477da6237c0d6858ca06044e05c"></a>

- Recompute and verify the source, formal, and release seals from one immutable
  tree; mutable-tree checks are not release evidence.


<a id="record-0a3ff80c9b989defb939c018fab167e7911e998132d32c6142b26bc39eefd32e"></a>

- After that candidate is frozen, obtain the four owner-held protected approval
  files for the exact `23/38/7/8` operation plans. The validator and receipt
  writer now derive those counts from the candidate-bound contract and reject
  an unplanned eighth `network-scale-soak` operation; this source repair is not
  a substitute for operator approval.




<a id="record-f89bd70bef801d6eba6626b96f8c050a75481f30a6582d689eb631b74cb47f22"></a>

<!-- Original context: Roadmap / Nexus topology model closure -->
- Before a Taira release claims `dpn`, `is`, `is2`, or `cbsi` as a live physical
  dataspace, archive deployment evidence for its distinct validator/server
  cohort, storage boundary, and dataspace manifest. A catalog entry or repeated
  lane-manifest roster is insufficient evidence.


<a id="record-991e61dab22d004b492a7cb9bba4349695bc6f68336e930f3df65f74b07e3956"></a>

- Preserve the schema-closed manifest validator-account/`PeerId`/Torii binding
  projection now exposed in operator status. The V1 Taira gate requires exact
  roster-to-binding equality, stable same-dataspace and advancing-sample
  projections, and no account, peer, or canonical HTTPS origin reuse across
  dataspaces. This proves disjoint declared identities and routes, not disjoint
  physical machines; release qualification still needs independently archived
  deployment evidence for the physical server and storage boundaries.



<a id="record-64cef74910761fcdb2aeabd78751777a5b5afd2a2b445438dacdc7c5272c7a7f"></a>

<!-- Original context: Roadmap / First-release security remediation validation -->
## First-release security remediation validation


Do not promote the private audit ledger to complete until those source-bound
commands and the final reconciliation pass are recorded.

The OpenAPI default generator and canonical CI dependency bootstrap are now
executable, and `.cargo/config.toml` is bound into the exact source-input tree.
The two detached replays now use the tracked Cargo-lock provisioner, receive an
exact verified copy of the pinned installed Node dependency graph, and clean up
safely under Bash 3.2 even when preflight rejects the checkout.
Closure still requires a clean double regeneration, synchronized manifests and
version maps, an explicit generated-client versus hand-maintained-SDK policy,
an independently administered external software Ed25519 envelope, and a
production gate run with
`OPENAPI_REQUIRE_SIGNED=1` and the approved operator allowlist.



<a id="record-3457d6f720b76711d9d8bdfcb375d8e55b0a1141508e36425110b0850b31c8a3"></a>

<!-- Original context: Roadmap / Disposable Taira-compatible development networks -->
## Disposable Taira-compatible development networks



<a id="record-0e17e48dcc2192c4a6ab416ccc4f6bfe9c5a3aabf33fbc06c2c83928e32255bb"></a>

- Keep the disposable Taira path one command and four peers:
  `python3 scripts/taira_devnet.py up --inrou-canary-dir <owner-only-workspace>`.
  It must generate fresh keys, canonical Taira chain identity, NPoS Nexus
  configuration, validate all four daemon configs with the current binaries,
  start only its owned cohort, and require readiness, typed `Applied` status
  for its signed transaction, converged advancing heights, semantic MCP
  initialization, and the exact four-replica guest workload canary before
  returning success. There is no startup-only success mode.


<a id="record-3054a25856cd3a2cc42ac1986f381e13a55e4ed2cb32d4b4558d69351b1fb3a0"></a>

- Keep `check` read-only and `down` scoped to the marked disposable directory.
  `check` reports configured capacity and current cohort health and must require
  the owner-only exact V1 guest qualification record emitted by `up`. It must
  not claim to repeat KVM, source/binary identity, signed finality, or the
  mutating Inrou deployment.


<a id="record-0bc5596d3c28b82e27755e449a0a2075626df65cbd0dfb25e6ce57dbb066bfe3"></a>

- Once peer shutdown and the cleanup-directory identity are proven, `down` must
  destroy the complete generated `network/` tree, including configs, state,
  logs, every peer/operator/onboarding signer, and the onboarding token. Failed
  startup may print bounded log tails before applying the same destruction;
  unproven shutdown or identity drift must preserve the tree.
  Do not restore release authorities, source-seal handoffs, publication
  receipts, LaunchAgents, systemd validator units, predecessor rollback, or
  24-hour soak requirements to this path.


<a id="record-d94f0800d7fd59dc358fa1bb29f3580c3149fa87a83e1532c4723606e120cbe0"></a>

- Replace generated stop-script PID-file/argv matching with a Linux process
  identity that cannot be reused between proof and signal (pidfd plus pinned
  start-time/executable/config identity). Keep the current fail-closed residual
  cohort checks until that narrower authority is implemented.


<a id="record-d90f744c34b0ada7740d59e2d27de606168a271680d1711a3f44b4b3b6b3e9a6"></a>

- Treat public Taira product-surface qualification as a separate operator
  activity. The compiled `iroha taira doctor` remains the standalone read-only
  diagnostic; the broad doctor is opt-in for disposable networks.
  `write-canary` and `inrou-canary` are low-level coordinator primitives, not
  one-shot operator flows: each invocation selects exactly one child and one
  prepare, retained-envelope submit, or read-only recovery action. Authorized
  public mutation must use the compiled `public-reset` coordinator.



<a id="record-546855dbada73ecf30e753da8ea22b52b8ac8a22bfb4a687aba263d079377279"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
## SORA Nexus and Taira

**Status:** active pre-release hardening.


