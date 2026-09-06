# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-d6c73ba56a767841746ca05cdd86363d353de861e112a14a0d83260bd694e0d0"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- Keep EVM route-canary live evidence bound to the receipt block: the live
  helper now checks receipt block number/hash against `eth_getBlockByNumber`,
  requires a non-zero block `receiptsRoot`, rejects duplicate matching
  `MessageProofAccepted` events at the supplied log index, and refuses imported
  full-TOML summaries whose route-canary block verification metadata was
  forged. Strict release-bundle verification now also owns regressions for
  positive receipt block numbers, non-zero receipt block hashes, non-zero block
  `receiptsRoot` values, receipt-block hash-role separation, and direct helper
  parity with the runtime's finality-height hash-role rejection. Runtime and
  builder-side canary checks also pin the EVM proof transcript to target-domain
  ETH/BSC, proof version `1`, proof source-domain SORA, and a consumed
  `usedMessageProofs(messageId)` replay guard.


<a id="record-17b7b34b63a7dd2e136819a14bc8d1632972438fd211c302b1cdbee439230954"></a>

- Keep Python UI witness-provider inputs isolated from app-owned mutable
  objects; the SDK snapshot path now clones accepted non-string sequence inputs
  before user-provided witness resolvers run, so portal/mobile witness
  preparation cannot mutate the original proof request that the UI displays.


<a id="record-461f996228ec2bdd5af62b6112fa6e4c45aeb2e9ebd9ebfe57c01869e8c07763"></a>

- The all-lanes readiness and release-bundle verifier now derive a required
  deployment records. Ready release bundles must carry that gate in the
  proofs the same machine-audited gate surface as the Solana, TON, and TRON
  JSON `toml_ready` false and refuses production TOML unless the governed
  runtime-storage gate hash is supplied and matches, so source material plus
  preflight now imports that same
  instead of treating a locally recomputed value as sufficient release evidence.
  Direct ETH/BSC source-evidence renderers now apply the same preimage rule to
  production TOML: hash-only source bridge code metadata remains diagnostic
  JSON, while `--toml` and JSON `toml_ready` require
  `--source-bridge-runtime-bytecode-hex` or
  `--source-bridge-runtime-bytecode-file` so the Keccak-256 runtime code hash is
  replayable from operator evidence.


<a id="record-d469a59e43acf4e573213c721612db2568de29fb5f85c1c2392aa73c8cfed781"></a>

- The all-lanes evidence preflight now emits an explicit `release_checklist`
  that separates required lane records, governed deployment evidence, route
  allowlist binding, live route canary evidence, and unresolved blockers for
  release automation.


<a id="record-cdbf81a94cd07537df92e1a12c942ecf6d49d06f05c06a8b2e251a94a74e042d"></a>

- Continue dependency, documentation, and release hygiene work required by LF
  Decentralized Trust project expectations.

**Next checkpoints:** governed deployment evidence and live canary evidence for
operator-provided rollout bundles.



<a id="record-075af270a1e4bf1f6b0da420a3deb0136428681048b55f6b17552766f7461d09"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Autoscale localnet expansion/contraction evidence now derives live
  elastic-lane evidence from retained validator tenures rather than lifecycle
  labels, while terminal audit rows exposed by Torii do not fake expansion
  progress or block contraction. Adversarial harness coverage pins terminal
  audit rows, revivable validator rows, and terminal-count/activation-watermark
  noise beside an already-live validator baseline. The same localnet harness now parses
  structured `lane` fields from autoscale transition logs before using
  deterministic scale-out/scale-in quorum evidence, and requires the producer's
  structured `height`, `active_lanes`, and `autoscale_capacity_lanes` fields on
  the same transition line after ANSI normalization. Scale-out evidence must
  also carry `out_latency_ratio_permille` and
  `out_utilization_p95_permille`; scale-in evidence must carry
  `in_latency_ratio_permille` and `in_utilization_p95_permille`. Transition
  markers for a different elastic lane, missing or wrong-direction producer
  fields, duplicated height/capacity/ratio fields, prefixed field name, multiple
  or conflicting standalone `lane` fields, stale contextual lane text beside a
  structured event, or suffixed lane-looking token such as a decimal,
  hyphenated, or leading-zero lane value cannot satisfy public-profile
  expansion or contraction checks. Malformed duplicate producer fields such as
  `height=2 height=bogus`, non-ASCII/control numeric separators, and required
  producer fields or transition markers carried inside unrelated quoted strings,
  keyed bracket/brace/paren detail values, non-message fields, or extended with
  forged suffix text, are also rejected before a transition line can count
  toward quorum evidence.
  Public-profile expansion evidence now also pins relay-height progress to the
  target elastic lane, rejecting wrong-lane relay progress and stale
  same-height relay records. Storage fallback evidence now requires each peer
  to expose the exact expanded contiguous lane-id profile
  plus the exact autoscale elastic `lane_NNN_elastic_lane_N` storage segment, so
  prefix-spoofed or wrong-slug storage directories cannot supply elastic-lane
  progress, duplicate elastic-lane directories cannot hide missing base-lane
  storage, and extra malformed or duplicate lane directories cannot satisfy an
  otherwise complete profile. Expansion/contraction status evidence also now
  requires a unique `teu_lane_commit` lane-status row per lane, so duplicate
  active rows cannot fake elastic-lane expansion or keep base lanes eligible for
  contraction.


<a id="record-a29747c5dd583c4f82954b08f29904f6027fb650ea607ac70a1ac90243d16c11"></a>

- Keep the rotating Byzantine 30 TPS NPoS soak in the stabilization corridor:
  the snapshot-enabled strict 7,200 second 4-peer transfer run now passes under
  the broadened `conflicting-ready`, `duplicate-inits`, and
  `drop-validator-chunks` matrix with `snapshot_mode=read_write`,
  `snapshot_create_every_ms=30000`, 72 non-overlapping fault windows evenly
  split across the three fault kinds, `submit_elapsed=7200.110828917s`,
  `submitted=216000`, final `min_approved=216016`, `max_rejected=0`, final
  queue size `0`, final convergence at height `1576`,
  `load_submitted_tps=29.999538`, `load_committed_tps=29.951370`, and final
  committed TPS `29.774000`. The matching snapshot-enabled 900 second bounded
  recovery gate also passes with
  `IROHA_REALISTIC_30TPS_RECOVERY_BOUND_SECS=120`; the full gate recorded 72
  successful restart recoveries with max status wait `30691ms`, max total
  restart/status duration `39070ms`, complete snapshot bundles after every
  shutdown/status recovery, and max strict convergence wait `63648ms`. This
  resolves the earlier multi-minute full-replay startup tail; remaining
  measured latency is bounded restarted-peer catch-up/convergence while the
  healthy quorum continues committing. Replay validation still avoids
  replay-only FASTPQ witness/transcript generation without weakening committed
  result or root verification, and replay catch-up persists the query-index
  journal once per Kura replay range while logging per-phase timings. Core
  replay validation also now replays multiple route-sensitive legacy blocks
  without embedded execution context into a fresh state and compares canonical
  WSV snapshot bytes against the originally committed WSV despite an
  adversarially drifted final WSV checkpoint sidecar. The
  remaining checkpoint for this corridor is keeping the broadened strict soak
  in regression rotation while release packaging and operator runbooks
  converge.


<a id="record-85bb8b5265b2647edd37606c16afa0e740080d8518243100a32d42fc0ffcd6cf"></a>

<!-- Original context: Roadmap / IVM, Kotodama, and Norito -->
## IVM, Kotodama, and Norito

**Status:** external release evidence remains.



<a id="record-a22222a330127927cd4e5ac29521f4ba40cdb4aee3bfe4cd2660536082e6fe8b"></a>

- If a future development-overlay profile must support an arbitrary non-sticky
  attacker-writable lookup directory or a hostile direct final parent, add a
  protected private anchor or a stronger directory-identity protocol. V1 now
  retains directory handles, performs handle-relative creation and publication,
  rejects unsupported parents, and fails stop after ambiguous persistence or
  host rollback failure; same-UID, privileged, and ACL-authorized writers remain
  explicit operator trust boundaries rather than supported adversaries.


<a id="record-f9d48dd4f97e50880e7f34b3239cdb13eef518ac5b793d77c5b42daeb726272d"></a>

- Keep strict SSA as Kotodama's single optimization authority. The dormant analysis/fuzz
  interpreter, duplicate transport optimizer, manual-access storage, and wide-encoding facade are
  removed; do not restore them as compatibility or comparison paths. Remaining compiler
  performance qualification is the source-and-lock-authenticated benchmark evidence below.


<a id="record-6475c162d5789cae311a68ac95ca071525cdc26dcc1c1b8f8f575a5571c1bcdd"></a>

- Run the candidate and the approved, source-and-lock-authenticated benchmark
  baseline through `.github/workflows/kotodama_perf.yml` on the same quiet,
  pinned runner and retain the workflow evidence for the `<=5%` threshold. The
  workflow now creates a source-, lockfile-, and Criterion-sample-bound JSON
  receipt, checksums the complete base/candidate Criterion tree, and uploads it
  with the runner/toolchain record; no accepted run exists yet. The
  unauthenticated July predecessor is not a V1 input and must not be recovered,
  rebuilt, or reintroduced as compatibility evidence. Until the controlled
  workflow completes, no reproducible 5% comparison is claimed.


<a id="record-5256e8198b03cb70f9c58bd4f3d71f11ae8251f87a2fc016acd78cd7d79a00e9"></a>

- Execute `.github/workflows/numeric_v1_calibration.yml` for both mandatory
  release profiles and retain its attested archives: Apple M1 Ultra
  (`Mac13,2`, arm64) and AWS Graviton3 `c7g.4xlarge`. The general Kotodama
  `<=5%` comparison is not a substitute for either Numeric V1 calibration
  archive, and neither release-profile archive is currently available.


<a id="record-d62d8da97aead03742014abf3b6f083b9f5a19b5d2a7eeaa0f7dd6c9514d4296"></a>

- Current-source Numeric filters pass in Rust 30/30, JavaScript 7/7, Python
  Numeric-plus-Quantity 25/25, Java 4/4, and Kotlin 4/4. SDK admission/manifest
  gates pass in JavaScript 8/8, Python 185/185, Java 10 checks, and Kotlin
  11/11; generated syntax freshness covers 18 files. The golden-fixture
  SHA-256 is
  `b5f11fd22856f69165a0c2a6906cea5beeaa46a1aee6c322ed18abc0acefee72`.


<a id="record-9c7a5bcc01ce3a4081322cb21cdbd17bf5591291d07b08d590f6cfe7f98bd779"></a>

- Swift's direct current-source Numeric harness passes 6/6. A local
  source-fingerprint-bound ABI-23 five-slice XCFramework also passes the full
  SwiftPM suite at 1729/1729; because it was emitted outside the repository and
  was not signed or published, the checked-in release inventory and
  cross-machine replay remain open.


<a id="record-49ccd84f766ecdd510d5545bb0bea269a5ab92955920ebf010e1093bf1babbac"></a>

- Fresh C# compilation is now qualified with the pinned .NET SDK 8.0.419:
  solution restore succeeds, Release builds with zero warnings or errors, and
  the unit and integration suites pass 5439/5439 and 2/2. Windows-native package
  production, signing, publication, and replay from the final candidate remain
  external release evidence.


