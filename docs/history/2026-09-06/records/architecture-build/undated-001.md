# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-4a4da9ee30c092c1a0b804972b1d7a7372c778c639d6d9febd685be8dff0c0a9"></a>

<!-- Original context: Status / First-release architecture redesign -->
## First-release architecture redesign

The approved structural redesign is in progress. The deterministic SoraNet
reward calculator and payout ledger now live in `soranet_incentives`, and
Rust consumers use that crate directly. Removing the orchestrator dependency
on Core and moving CLI compilation remove Core, FastPQ, IVM, and forbidden Halo2
execution features from the current Rust SDK normal/build graph. Node
configuration, telemetry, and storage implementations still require extraction.
All 13 incentives unit tests and 82 CI routing/workflow tests pass.
The configuration wire records now live in `iroha_torii_shared`, with 22 wire,
3 node conversion, and 14 Core configuration tests passing. JSON key hashing uses one implementation
across compile-time, scalar, and accelerated paths; focused parity tests pass.
The compiled Torii inventory contains 655 explicit routes, including 573 in the
current generated SDK projection; 8 inventory tests pass. File/provenance
policy schema 2 removes the active aggregate line target and retains historical
evidence, with 128 focused guard/workflow tests passing. The complete source
budget still has 249 findings; limits and exceptions were not expanded.
SDK/CLI configuration checks passed. Shared status records preserve 32 captured
named wire/JSON fixtures; 93 telemetry tests pass. The compiler-unit profiler
passes 135 focused tests, including source sealing and exact compiler-artifact
identity. The frozen cold model baseline validates all 338 compiler units; its
heavy compilation peaks at 13,270,646,784 bytes. The 25%-reduction ceiling is
9,952,985,088 bytes for that profile. The cold SDK baseline validates all 498 compiler units. Remaining baseline
surfaces and candidate measurements are still pending. All 10 CLI toolkit and 7 governance-audit tests pass. The Kotlin-owned JVM parity gate passes 89 runtime tests, including all 55 Java
consumer tests; its 14 script tests pass. The old Java Norito harness was removed,
and transparent-wrapper and missing-field validation gaps were fixed in Kotlin.
Core/Torii, SDK/CLI, test-network, schema-generator and Mochi status consumers
compile, as do the three integration harnesses. The full Kotlin JVM run has
1,207 tests and 33 failures: six lack the Rust fixture generator, 26 lack native
bridge capabilities, and one expects a removed matrix category. Native and
device qualification, Java implementation/publication retirement and the
complete candidate remain pending. Implementation and
remaining acceptance are tracked in
[the architecture record](../../../../../specs/first_release_architecture_redesign.md).



<a id="record-1997a07734bef10975baa4b887499c5df120a6538dfc8d7bc44e56b0a9f4c1c1"></a>

<!-- Original context: Roadmap / First-release architecture redesign -->
## First-release architecture redesign

Complete the approved model/service crate splits, immutable asynchronous Rust
SDK and consumer migration, capability-owned Core/Torii components, architecture
and build-memory budgets, conditional CI binary production, Kotlin-only JVM
implementation, and current documentation views. See the
[implementation and acceptance record](../../../../../specs/first_release_architecture_redesign.md).



<a id="record-3dd449171c9b45b2bfd3845fa08e8d0846ebda55afefd55806e90612b66a291e"></a>

<!-- Original context: Roadmap / Merged-candidate compile recovery -->
## Merged-candidate compile recovery



<a id="record-4848031a85c520d3363dbbeb8e0babf81768b72b485c68efc99f6e652dcde584"></a>

- After the merge is recorded as a signed, clean source commit, replay the
  Torii OpenAPI owner and synchronize the complete five-file artifact bundle.
  The current embedded, authored, and versioned-current specifications are
  synchronized with the reconciled router, and both dirty, unsigned manifests
  bind those current bytes. Regenerate the complete provenance bundle from the
  clean commit and sign it rather than hand-editing dirty development metadata.



<a id="record-9c4ba9401f4b7e72114fd3b95e94bedcda8831dfe3a6c30790948a715e21a85f"></a>

<!-- Original context: Roadmap / First-release hard-cut closeout -->
- Run the focused and workspace Rust Cargo tests and strict all-target Clippy
  from the settled candidate.


<a id="record-4a3dbbd41d761574bb331c487b1defba81947d4ef5e89c3f5a1d54ee081738f3"></a>

<!-- Original context: Roadmap / Workspace review closure -->
## Workspace review closure



<a id="record-644abf4c46c6f9abc5def60a9bd7294f2d262ba34fc0e171cd2a5513eda1d92e"></a>

- Run the remaining release-wide workspace matrix against the regenerated
  six-lane 384-bit FastPQ proof fixture, the BLAKE2b-256 ordering/metadata
  commitments, and the first-release AXT hard cut. Retired single-field metadata
  proofs, pre-incarnation handles, and pre-ratchet snapshots must not be
  accepted or migrated by relabelling; pre-transition-set block results and
  fixtures must be regenerated rather than defaulting the required set to
  empty.



<a id="record-b39bdd4d92a547e11839128ff277a27e0cefadb958613fa40bd1399e82b7a890"></a>

- TODO: define one workspace-wide global ordering/pagination contract for every
  routed list fanout. Torii now fetches the exact admitted global prefix from
  each shard and applies the requested page once for the repaired directory
  path; add bounded multi-page shard retrieval before permitting a larger
  offset window, and make every remaining fanout list prove the same property
  with cross-shard fixtures.


<a id="record-6967abc79817f906b5150b255634f8b27fb5eb4c1f6d8792ed13f9ae1ea988fe"></a>

- TODO: add an authenticated cancellation/rebind protocol for an exact pending
  QueuePlan whose fresh dataspace/role topology changes (for example after an
  SNS lease transition). Candidate assembly now defers such work without
  poisoning proposals, but the immutable obligation otherwise remains pending
  until canonical application.


<a id="record-b14e46ff691c69f9db2ffa82705dbc664890032519f34f223eccebf2c83239b9"></a>

- TODO: specify and implement authenticated, irreversible AXT family retirement,
  deterministic family-budget compaction, and per-issuer admission quotas using
  the permanent dataspace generation and exact asset incarnation. V1 retains
  every cumulative family record until a consensus retirement proof and
  resource policy are specified and audited. Capacity-only or unaudited expiry
  eviction is not acceptable because it can reset remaining allowance without
  proving that the exact signed family is permanently unusable; a future
  strict-after expiry rule may participate only as one authenticated input to
  that irreversible retirement proof.


<a id="record-ae3e6c8c68d6117d2342b7ce6bb2a9b17771dbd73e39715d3ff51630ed0e96b7"></a>

- Validate the bounded high-view pacemaker and corrected selected-Serve
  late-Fetch regression in the four-validator loss/hold/heal corridor. The
  promotion canary must submit real work, observe it commit within the finite
  timeout envelope, then prove the empty queue leaves total and non-empty block
  heights stable; empty blocks are not a liveness mechanism.


<a id="record-67195b0d8a39deb3296935df414c8a224b2fe39186a1bc79999a2b430e444e35"></a>

- Freeze the current revision-4 candidate only after the physical ChainEpoch
  proof shards, exact four-validator fault corridor, and remaining authoritative
  formal and workspace-wide validation are complete. Historical mutable-tree
  snapshots are not release receipts for the current source tree.


<a id="record-a8f8777782774c8f81fead59a61b02e2c132ad314f9bf2db18b0e062311dbd28"></a>

- After that freeze, use the official workflows to regenerate and check the
  environment inventory, JavaScript current-Rust-contract fixture, and
  generated-artifact registry. Preserve the green SDK release guard after its
  end-to-end closure repin and the completed JavaScript receipt-header repair.


<a id="record-61321d21f58415cc0cc734b8128e182c98ba2440ca14b6cf0b0a64ed7c164c44"></a>

- For OpenAPI, first commit the final repaired inputs, regenerate the bundle
  from that clean exact commit with truthful provenance, and commit the
  generated outputs in a descendant. Refresh SF1 rows and projections only
  with an authorized council re-sign over the current manifest digest.


<a id="record-db6b68f7e28805437270456791a4fe6f83e56a6ed876430a5e3d8927c1c5d33c"></a>

- Finish borrowed, cache-/scratch-audited admission verification for default
  w3f BLS, GOST, and SM2, and replace PQClean's heap-backed SHAKE workspace.
  Extend the source-proven ordinary iterable adapter beyond `FindPeers` only
  with producer-specific semantic parity and cold-Kura bounds. Then rerun the
  focused Rust suites and strict all-target Clippy from the frozen candidate.


<a id="record-6491d9de098e123bbf423e8b9ed3ffb9ebda8778390531d8d0a40cbc630ee460"></a>

<!-- Original context: Roadmap / Build-efficiency closeout -->
## Build-efficiency closeout



<a id="record-8b56cc24888c8ddb735aa92bd9a80443b420d111163c3947223ee5cc3cd4e765"></a>

- Preserve the reviewed 5,067,263-line first-party Rust baseline, the current
  5,014,603-line active ratchet, the 4,540,000-line hard ceiling, and the
  4,500,000-line working target. The current mutable-tree checkpoint counts
  4,975,825 lines, 435,825 above the hard ceiling, with 18 per-file findings
  remaining. Recompute the exact count and gap after source freeze; do not
  redefine the baseline,
  count moved test code as a physical reduction, or weaken required runtime,
  security, consensus, SDK, or release-evidence behavior to close it. Keep
  deterministic oversized-file exceptions as exact ratchets and the
  source-budget gate finding-free.


<a id="record-d29aa215edfca3e33fb087b144885c32375792aa836a441ab9dbdcfa2ec37876"></a>

- Keep every reviewed production module and newly introduced companion source
  in the eventual signed commit, then rerun the strict SDK source-closure guard
  from that immutable candidate. Keep the staged merge-resolution and
  optimization changes reviewable until the signed commit is prepared.


<a id="record-09aacc69dd5e843b2fc99787020dd2212fa66c7e32e75d78f9ca5d318155b66d"></a>

- Use the unchanged protected workspace lockfile now that the incompatible
  prototype manifest edge is withdrawn. From that immutable candidate, run the
  full locked workspace build and tests, strict all-target/all-feature
  Clippy, ABI and canonical wire goldens, focused Sumeragi/Kura recovery suites,
  and representative four-validator deterministic consensus tests.


<a id="record-b5077d216462cfaa8384f2e085b965fcd9345a90c9c0a9c6abdc21813b23d1b0"></a>

- Capture comparable `valid: true` cold/warm and compiler-work reports for the
  baseline and candidate with identical source, lock, toolchain, arguments,
  environment, and host. Expand compile-unit coverage beyond the current
  data-model graph to the daemon, CLI, and workspace release lanes before
  claiming reduced compiler work or runtime performance neutrality.



<a id="record-a602bda12c3f3a0f415b06de0c3d38ad3ea6d82160477e6a01ee2ffd6c0e4f09"></a>

<!-- Original context: Roadmap / Repository structure follow-ups -->
## Repository structure follow-ups



<a id="record-e6a68f0630449b698546406db33c74e30331805fad755350901ce218b5fa3f1a"></a>

- Continue extracting cohesive production modules from the exact source-budget
  exceptions, prioritizing Kura, Torii routing/API, core state, and other files
  still above 20,000 lines. Preserve public facades and wire behavior; every
  split must ratchet the checked-in line count downward.


<a id="record-7dcf7aaac3cd3dbb150fb16d145e33d5cc280d4c43cc5050d844060e89be7f2d"></a>

<!-- Original context: Roadmap / Memory-containment follow-ups -->
## Memory-containment follow-ups



<a id="record-72763559901a075b64c5e6e9af13b72fb72405f32a4dc82411ddc8c3cf8a62ba"></a>

- Complete mobile-device concurrency and memory-pressure evidence for init,
  append, verify, and redeem operations. The matrix must demonstrate bounded
  peak memory, retry after the native busy result, transient verifier/prover
  sequencing, and no partially advanced wallet lifecycle when a proof worker
  is busy.

