# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-25f319674cefd05581f3ec264a32d5b26c473ec61e51e39e41c58e4eaae57d30"></a>

<!-- Original context: Roadmap / Musubi first-release registry and developer ecosystem reset -->
- Execute focused model/Core/Torii/CLI/publication/cache tests from the final
  source, then strict workspace clippy, serialization guards, Kotlin/Java/Swift
  suites, and the full workspace suite. The first deterministic cache crash
  matrix now terminates a child process at nine payload, source-file, source-tree,
  publication, and directory-sync boundaries, then requires exact verification
  or retry convergence after reopen. Execute it from a coherent build lane and
  extend it to the required crash-at-every-write, malformed-plan, race, secret,
  disk-full, and bounded-memory fuzz campaigns.


<a id="record-525f78ee464f62e594618a8f21fd5a18bb437709faf531bad65aa7ab86d7b887"></a>

- Qualify the bounded pristine pre-ingress recovery now exposed as
  `publish --recover OPERATION_ID`. It rebuilds the clean package, proves exact
  equality with the revision-one Validation journal, and under the operation
  lock idempotently installs the immutable plan before the CAR; ordinary
  `publish --resume` continues to avoid reconstructing unpublished workspace
  state. Execute crash-at-every-write and storage-fault coverage, and add a safe
  descriptor-relative way to identify and clean only an operation-owned
  temporary hard-link residue when interruption after target linking but before
  temporary unlink leaves an exact destination with link count two. Do not
  replace that work with a broad path scan or deletion heuristic.


<a id="record-fbc4ba2e7b3d4ab4a6737fec82a2209c27f062ed27cc2af2f9b8cc42c1d054d1"></a>

- Run the four-peer devnet, five-to-ten-namespace Taira allowlist/two-week soak,
  and 30-day invite beta. Open admission only after zero critical/high findings,
  recovery drills, load/chaos success, and sustained SLO evidence.

The implementation-coupled contract is [`specs/musubi.md`](../../../../../specs/musubi.md).



<a id="record-bc44a443c81bb8a210fc9661190c1ff93345a9e226062d7b92cc5cdf2053d5af"></a>

<!-- Original context: Roadmap / Privacy, ZK, and FHE -->
- Keep Soracloud FHE multi-input behavior covered at the source level while the
  BFV-RNS implementation is still pending. The current Rust corridor covers
  deterministic Add/Multiply folds, malformed late-operand rejection,
  multi-input admission/output projection, output commitment order binding, and
  shared Add/Multiply/RotateLeft/Bootstrap operation-output vectors with
  pinned public-key and evaluation-key bundle metadata, per-entry
  relinearization component digests, Galois automorphism key counts and
  per-entry component digests, rotation/bootstrap refresh `c0`/`c1` component
  digests, and adversarial refresh-material rejection for the public
  Galois/rotation/bootstrap keys.


<a id="record-45cc36f8d96c0f88d824a33feecf71328252578f0a70f2aeb348d7adda213fb1"></a>

- Keep Soracloud FHE governance parameter fixtures runtime-bound instead of
  descriptor-only. The canonical parameter-set, execution-policy, governance
  bundle, and job-spec fixtures now target the registered `bfv-default`
  RAM-LFE BFV profile; core admission consumes the shared bundle and rejects
  backend, polynomial-degree, slot-count, plaintext-width, ciphertext-chain, and
  parameter digest drift. Parameter-set descriptors now also carry the
  domain-separated registered BFV RNS modulus-chain digest, and core admission
  rejects RNS descriptor drift before FHE jobs can run. The registered RNS
  chain selector now preflights exact-addition and exact negacyclic-product
  coverage plus the concrete negacyclic NTT root table before returning the
  production chain or digest, and the RNS key-switch bridge applies the same
  exact-evaluator chain preflight before consuming key-switch material. Public
  RNS exact evaluator entry points now also preflight their required chain
  coverage before invalid refresh rounds, no-op packed rotations, or
  key-switch schedules can short-circuit validation, and indexed Bootstrap
  refresh helpers preflight requested round capacity before malformed
  ciphertext shapes enter the addition path. Exact and bounded plaintext-mask
  selector products are now pinned in the same preflight corridor tests, so
  packed-rotation public mask products cannot hide a too-narrow product chain
  behind malformed ciphertext diagnostics.
  Bounded exact-RNS ciphertext multiplication now uses the same exact
  evaluator-chain preflight before operand or relinearization-key shape checks.
  Bounded target-limb basis-extension execution wrappers now share one
  rounded-capacity plus decomposition/evaluator prefix corridor, and Bootstrap
  refresh rejects structurally valid non-prefix decomposition chains before
  malformed refresh-key or ciphertext shapes.
  Refresh transcript digest assembly now returns structured shape errors for
  missing or unmatched rotation transcript seeds instead of relying on a
  post-validation panic invariant.
  Execution policies now also
  pin the domain-separated BFV evaluation-key bundle digest, and
  `RunSoracloudFheJob` rejects structurally valid but ungoverned key bundles
  before output state is emitted. The shared operation fixture also carries
  sample residue-arithmetic hashes, Galois key-switch component digests, scalar
  plus packed Galois switch execution vectors, a runtime packed `RotateLeft`
  half-rotation vector, a one-step packed `RotateLeft` mask-and-sum schedule
  vector, and bounded one-/two-round bootstrap refresh vectors so SDK release
  vectors can reject fixture drift before the full BFV-RNS evaluator lands.

