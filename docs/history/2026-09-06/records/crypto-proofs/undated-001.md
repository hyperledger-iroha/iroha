# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-a1f5bc5283b3f5c0e21a76c9f5d7b065bf607b32d5032556dbbe5065475d301f"></a>

<!-- Original context: Roadmap / Digital-signature release qualification -->
## Digital-signature release qualification



<a id="record-1e6b78f39afb04ecea5ab95d19561dccc9a0d692b31a76cef31655980c991f2c"></a>

- Run the mixed-torsion Ed25519 regression with and without `ecc-batch`, followed
  by the full workspace test and strict all-target Clippy matrices from one
  settled candidate. The unrelated `iroha_crypto` SoraNet test-target
  compilation failures have been repaired.


<a id="record-d545f503dfec200e016344b706beb6511ebaddbbfb8078ee309ab123c452eb33"></a>

<!-- Original context: Roadmap / Iroha crypto first-release closure -->
## Iroha crypto first-release closure



<a id="record-159eff666eca0bbb848e29cd758bb8627dce8907b49b52df3a9f7b5d3830a806"></a>

- Benchmark canonical key parsing/formatting, BLS key generation/signing/aggregation, VRF proof
  construction, and threshold/timed-OVN arithmetic. Add deterministic output checks and allocation
  budgets so later optimizations cannot reintroduce transient secret copies or hardware-dependent
  results.


<a id="record-bb3a05332efebbf3d95671cf85941b0556d010def82d0560e558e163c815779f"></a>

- Complete an independent cryptographic and side-channel review of the adaptive threshold-BLS and
  timed-OVN implementations, including complaint handling, transcript binding, secret custody and
  erasure, pairing validation, corruption assumptions, and the scalar/accelerated target matrix.


<a id="record-2a6621e6380ec156d6b1aa71d29fda78e36a79bd7dd5c633a3b586c7706c2ad9"></a>

- Resolve the remaining production Clippy findings and bounded `dead_code` or lint exemptions in
  adaptive threshold-BLS, timed-OVN, TLE, GOST, SM, and SoraNet handshake/PQ modules after proving
  production reachability. Keep only release algorithms with owned callers, exact fixtures, and
  deterministic cross-platform validation.



<a id="record-f3012909d473aa467f9ae7ec710ccc4076697a599198f3620b2ac3ec01333f9e"></a>

<!-- Original context: Roadmap / Merged-candidate compile recovery -->
- Type the two large Halo2 tail-publication ledger equations explicitly so the
  normal lint policy accepts their values without wrapping or lint overrides.


<a id="record-2619311276e1ec58aab7dfebe54b2549e0ee4b6bd61e75fd6c7dcf990a14611c"></a>

<!-- Original context: Roadmap / ZK algorithm release qualification -->
## ZK algorithm release qualification



<a id="record-1f3357eb1c117a38cb05d698f6c9f72b3045f93b699209cb90384f9eae9ce649"></a>

- Resolve FASTPQ's verifier architecture before qualification. The current
  verifier deterministically rebuilds the full trace, FFT/LDE data, row hashes,
  AIR composition, and roots from the supplied batch while also authenticating
  an unquotiented FRI composition polynomial. That is prover-scale replay with
  redundant probabilistic proof plumbing, not succinct verification. Either
  implement the quotient/zerofier and degree argument needed to verify from
  bounded openings without replay, or deliberately reduce the format to one
  clearly documented full-replay artifact; benchmark the chosen single path.


<a id="record-8b4265e1d15678ed3508571b643d7e5976f67859dd2c7f63ee7edd5bcc1c7929"></a>

- Keep FASTPQ ledger qualification blocked until independent review connects
  the implemented arithmetic bound to the complete protocol adversary and
  validates the final-artifact multi-target digest accounting. V1 now pins six
  independently domain-separated Poseidon-x7 Goldilocks lanes for 384-bit
  commitments/transcript state, degree-four FRI challenges, binary folds, an
  eightfold LDE, zero grinding, and 136 queries. Its exact dyadic qROM
  calculator passes the 128-bit aggregate target across 54 release artifacts
  under the declared `Q ≤ 2^32` oracle-query bound, but intentionally reports
  production qualification unavailable until the protocol-specific reduction
  is reviewed. Cross-check the dense-MDS permutation and six-lane construction
  against an independent implementation and bind that review to final artifact
  digests before clearing the gate.


<a id="record-d326e8f8d8124e9fe3f5fb22fd95bbe977eb3104b56bd515c997c5194d9351a8"></a>

- Qualify the simplified accelerator boundary on release-class Apple and NVIDIA
  hardware. Compile the native Metal and CUDA sources, inject copy, launch,
  event, stream, and timeout failures to confirm process-lifetime quarantine,
  and archive CPU/Metal/CUDA root parity plus throughput and peak-memory results.
  A Rust feature build that selects runtime Metal source fallback does not
  replace native shader compilation or an `nvcc` build.


<a id="record-bae24f782bba0e9c0f2730f6c0be6a6dbfa0959e1b6ddc74e993b801bcaeacc9"></a>

- Keep non-null IVM `AXT_VERIFY_DS_PROOF` admission fail closed until the exact
  source roots and transaction set are matched to an authoritative finalized/QC
  source-state statement. Before releasing handle-backed remote spend, either
  make the issuer authentication cover the exact intent, proof, and effective
  amount or give those facts an independently anchored binding. Commitment
  width cannot compensate for facts omitted from the authenticated statement.


<a id="record-5e4dc38fded945763ddedf5885a7491633c52e0b0e20c530991089631c41277d"></a>

- Keep ZK-ACE proving, verification, and activation unavailable until its four
  public commitment words come from independent domain-separated invocations
  (or an equivalently strong replacement) with at least 128-bit collision
  binding. Regenerate the wider AIR schedule, trace domain, masks, FRI profile,
  profile digest, proof fixtures, and quantitative certificate before removing
  the fail-closed engine flag.


<a id="record-d30464cddbd8430cde8185084bdf629b8ce7c86695ae5c61737056e3d79134d9"></a>

- Add an explicit degree bound and bounded-degree terminal polynomial check to
  private generic native-STARK AIR profiles. `blowup_log2` must affect verified
  geometry rather than only transcript metadata. Keep the exact-root public
  Binding profile capped at `n_log2 = 12` in the meantime.


<a id="record-82b3479eb1b2e23b5e4dc68d4f53b354e2a85cb2429ad6948727010ca8d1f6eb"></a>

- Keep BFV public-padding-only native verification disabled until commitments
  and degree proofs for every hidden trace column are bound into the sampled AIR
  relation. Continue using governed full-material replay, which reconstructs
  and exactly matches the complete trace and composition roots.


<a id="record-765c1c1e28281aa9c17661033dd0892642a263efcec50f0fccbb6eb60d03db4c"></a>

- Run the focused native-STARK and FASTPQ regressions, then the full workspace
  test and strict all-target Clippy matrices from one settled candidate.


<a id="record-a873fba2489b27d0c3818f97fa8b8fd3600591902c71c574457dc4f69c274f75"></a>

- On CUDA- and Metal-capable release hosts, compile the corrected BN254 kernels
  and archive direct-evaluation, CPU/GPU parity, FFT/LDE, and benchmark evidence
  from the same immutable source revision.



<a id="record-0ff775b0d366b78314daec5e6ebbfe5a3ac44dba9793224bfba471d5a7e34911"></a>

<!-- Original context: Roadmap / MKHE and Figure 9 evidence-gated completion -->
## MKHE and Figure 9 evidence-gated completion



<a id="record-750572e11f79c53b52e6c657c4bde0f21bc909c7b8516dc49d0122f618d714bc"></a>

- Join the implemented 40-limb qPCS/FRI, private RLWE/source-statement,
  source-to-terminal/cross-field, terminal cross-basis, and authenticated
  zero-padding prerequisites inside the production RNS-native composite. The
  remaining verifier must prove the complete source mapping, terminal
  materialization and packing seals, 40-limb cross-field/global lookup, and
  ownership of every padding lane as one atomic path. Until then the composite
  returns `StageUnavailable` and emits no receipt or readiness authority; no
  parked 38-limb decoder or rejected scale-four proof may become an alternate
  path.


<a id="record-ddfd17c5de261f2d4bcd1f8b9b9b02786aa8de85751c3bfcf42ad62ee9336f26"></a>

- Qualify that MKHE composite with the exact estimator transcript,
  instantiated soundness/ZK review, canonical positive/adversarial wire
  fixtures, full-size KAT, eight-party end-to-end execution, and authenticated
  RSS/I/O/work measurements. Placeholder booleans, arbitrary nonzero digests,
  static formulas, and mutable-tree unit tests are not release evidence.


<a id="record-b98cad81c6b5481aaf2fcd5f82bc55259c848b7622ea3db74486bd084c60a2d1"></a>

- Provision the exact governed full-shape Figure 9 PK/VK artifacts and an
  independent full-shape proof vector, then run the completed first-party
  semantic, random-Nova, relaxed-Spartan, final-opening, and self-verification
  pipeline against them and cross-conform the output. The public prover and any
  verifier without the installed governed key remain fail closed until those
  artifacts and evidence are authenticated.


<a id="record-46bb184fbe5ba1541d75da4b0268bbee61626f17d0783b5b58327a3893eeeb47"></a>

- Run focused and workspace tests, strict all-target Clippy, four-validator
  activation/restart/replay, deterministic rebuild parity, fuzz thresholds,
  and every ignored release-size KAT from one immutable candidate before any
  readiness or activation gate can close.



<a id="record-6b079387ff46cb1c5735704380bf89fcb7e00d32f095d1a0e11d9b14f8dc95b7"></a>

<!-- Original context: Roadmap / Workspace review closure -->
- Rerun the focused Core/Torii ZK lifecycle, transcript, column-shape,
  reserved-STARK-alias, and Goldilocks rejection tests, followed by the
  workspace ZK feature matrix.
  Ship the corrected IPA parameter derivation and Poseidon byte framing as one
  first-release fixture/proof hard cut; do not mix artifacts generated under
  the retired derivations with corrected nodes.


<a id="record-89661126f586e1693c2b5af1ef9a26a13951fa8a4a052b3586611d80793bff59"></a>

- Keep T256/MKHE acceleration qualification fail closed: the fixed T256 suite
  disables generic Rayon fan-out and the MKHE NTT remains fixed-order scalar.
  Qualify no parallel, SIMD/NEON, Metal, or CUDA/GPU backend until fixed-shape
  output/KAT parity, secret-handling review, and implementation-derived peak-RSS
  evidence close; no hardware acceleration is currently claimed.


<a id="record-203e3ef2e6d29966202ac1b938a2e7faef99633af4c2ac27fb6e8e4e61abc638"></a>

- Preserve the lock-compatible parking of the ZKP crate's obsolete direct
  confidential-spool dependency. Reconnect the private source through a
  Core-owned adapter over the already locked
  `iroha_crypto::confidential_spool` primitive, with a move-only authenticated
  provider trait at the proof boundary. Do not add a new crate or alter
  `Cargo.lock`; keep the path fail closed until source lineage, secret
  lifecycle, and measured residency evidence are complete.


<a id="record-508aab62fc91c820111a56cbcd6299f56366ff219592761faef208e43454bed8"></a>

- Add a shared typed multisignature-witness construction and signing flow to
  the primary SDKs. Rust currently hashes requests and bounded-encodes completed
  witnesses; JavaScript and the `iroha_python` SoraFS reputation helpers can
  forward an externally produced witness. The standalone `iroha_torii_client`
  and the typed mobile/C# clients are intentionally signer-only; none of those
  surfaces should be described as end-to-end multisignature construction yet.



<a id="record-0b8f913eab34f2e387e7dc4c4ac4541bbcf62607da89147f797c089c9031cc77"></a>

<!-- Original context: Roadmap / Nexus topology model closure -->
- Move physical validator/server membership, replication, privacy, DA, and
  governance ownership into a dataspace-level manifest. The current V1 runtime
  still projects several of these fields through lane configuration and lane
  manifests; until that migration lands, enforce that every lane in one
  dataspace has an identical projection of its owning dataspace's roster and
  security policy.


<a id="record-a58552b75091ca8a26b9037c9a7954ed3b9ce9866f82a94aee01d32e26ad9a31"></a>

<!-- Original context: Roadmap / ZK-ACE JavaScript signed-transaction parity -->
## ZK-ACE JavaScript signed-transaction parity



<a id="record-f6fc6a3b646b8f9468119b6e7cc78188785cd98feebc7ab52fa4789b3796d9c9"></a>

- Design and implement a new typed JavaScript API for the canonical governed
  ZK-ACE action. It must accept a validated `PrivacyZkAcePolicyRecordV1`,
  canonical genesis hash, exact transaction context and fee intent, private
  non-serializable witness material, and signing key, then return the signed
  `SubmitPrivacyProofV1` transaction and typed effect metadata.


<a id="record-5cdacfdc40acfe17b55c501886671537a2e3f73ed9d2a589c8c6470f116caa15"></a>

- Mirror the Rust/Python two-pass intent binding and witness-erasure guarantees.
  Do not restore or alias the retired proof-attachment builders, caller-selected
  verifier keys/backends, serializable witness/public-input types, or direct
  ZK-ACE instruction wires.


